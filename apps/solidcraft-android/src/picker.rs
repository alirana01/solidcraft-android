//! Open through the Storage Access Framework, then copy the chosen file into app-private
//! storage. Save As writes there directly: a content URI is not a path the design can keep.

use std::path::Path;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll};

use solidcraft_ui_egui::FilePickerFuture;

static HOST: OnceLock<android_activity::AndroidApp> = OnceLock::new();

enum Slot {
    Idle,
    Waiting,
    Done(Option<String>),
}

static SLOT: Mutex<Slot> = Mutex::new(Slot::Idle);

pub fn install(app: android_activity::AndroidApp) {
    let _ = HOST.set(app);
}

fn slot() -> std::sync::MutexGuard<'static, Slot> {
    SLOT.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn open_future() -> FilePickerFuture {
    {
        let mut slot = slot();
        *slot = Slot::Waiting;
    }
    if let Some(app) = HOST.get().cloned() {
        let posted = app.clone();
        app.run_on_java_main_thread(Box::new(move || {
            if let Err(e) = request_open(&posted) {
                log::error!("open document: {e}");
                *slot() = Slot::Done(None);
            }
        }));
    } else {
        *slot() = Slot::Done(None);
    }
    Box::pin(Wait)
}

struct Wait;

impl std::future::Future for Wait {
    type Output = Option<String>;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<String>> {
        let mut slot = slot();
        match &mut *slot {
            Slot::Done(path) => {
                let path = path.take();
                *slot = Slot::Idle;
                Poll::Ready(path)
            }
            Slot::Idle | Slot::Waiting => Poll::Pending,
        }
    }
}

/// A file name the app can create under its private directory.
pub fn save_path(dir: &Path, name: &str, exts: &[&str]) -> String {
    let ext = exts.first().copied().filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or("solidcraft");
    let stem: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(64).collect();
    let stem = if stem.is_empty() { "design" } else { stem.as_str() };
    dir.join(format!("{stem}.{ext}")).to_string_lossy().into_owned()
}

#[allow(unsafe_code)]
fn request_open(app: &android_activity::AndroidApp) -> Result<(), String> {
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = app.activity_as_ptr().cast::<jni::sys::_jobject>();
    if activity.is_null() {
        return Err("no activity".into());
    }
    let activity = unsafe { jni::objects::JObject::from_raw(activity) };
    let result = env.call_method(&activity, "openDocument", "()V", &[]);
    // The activity pointer is a global reference owned by android-activity.
    std::mem::forget(activity);
    result.map(|_| ()).map_err(|e| e.to_string())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_app_solidcraft_MainActivity_nativePickerResult(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    path: jni::objects::JString<'_>,
) {
    let text = match env.get_string(&path) {
        Ok(s) => std::borrow::Cow::<str>::from(&*s).into_owned(),
        Err(_) => String::new(),
    };
    *slot() = Slot::Done((!text.is_empty()).then_some(text));
}
