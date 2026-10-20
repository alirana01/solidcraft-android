//! SolidCraft on Android.
//!
//! The same [`solidcraft_ui_egui::SolveApp`] as the desktop and browser apps, on eframe's
//! GameActivity runner (Vulkan, with GLES as a fallback). Build the native library with
//! `cargo ndk -t arm64-v8a`, then assemble `android/` with Gradle. The debug and release
//! packages share the debug signing key so a phone can install the release build.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_os = "android")]
mod picker;

#[cfg(target_os = "android")]
mod android {
    use std::path::PathBuf;

    use eframe::egui_wgpu::WgpuSetup;
    use solidcraft_engine::Session;
    use solidcraft_ui_egui::{Services, SolveApp};

    use crate::picker;

    struct App {
        solve: SolveApp,
        host: android_activity::AndroidApp,
    }

    impl eframe::App for App {
        fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            self.solve.logic(ctx);
        }
        fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
            if let Some(insets) = insets(&self.host, ctx, raw) {
                raw.safe_area_insets = Some(insets);
            }
            self.solve.raw_input_hook(raw);
        }
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.solve.ui(ui);
        }
        fn save(&mut self, storage: &mut dyn eframe::Storage) {
            storage.set_string(solidcraft_ui_egui::prefs::STORAGE_KEY, self.solve.prefs());
        }
    }

    /// Status and navigation bars, from GameActivity's content rectangle, in logical points.
    fn insets(host: &android_activity::AndroidApp, ctx: &egui::Context, raw: &egui::RawInput) -> Option<egui::SafeAreaInsets> {
        let screen = raw.screen_rect?;
        let native = raw.viewports.get(&raw.viewport_id).and_then(|v| v.native_pixels_per_point);
        let ppp = native
            .filter(|p| p.is_finite() && *p > 0.0)
            .unwrap_or_else(|| ctx.pixels_per_point())
            .clamp(0.5, 8.0);
        let rect = host.content_rect();
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return None;
        }
        let win_w = screen.width() * ppp;
        let win_h = screen.height() * ppp;
        let edge = |px: f32| (px / ppp).clamp(0.0, 160.0);
        let top = edge(rect.top as f32);
        let left = edge(rect.left as f32);
        let right = edge(win_w - rect.right as f32);
        let bottom = edge(win_h - rect.bottom as f32);
        if ![top, left, right, bottom].iter().all(|v| v.is_finite()) {
            return None;
        }
        Some(egui::SafeAreaInsets(egui::epaint::MarginF32 { top, left, right, bottom }))
    }

    fn services(data: Option<PathBuf>) -> Services {
        let save_dir = data.clone();
        Services {
            pick_open: Some(Box::new(|_exts: &[&str]| picker::open_future())),
            pick_save: Some(Box::new(move |name: &str, exts: &[&str]| {
                let path = save_dir.as_ref().map(|dir| picker::save_path(dir, name, exts));
                Box::pin(std::future::ready(path))
            })),
            ..Default::default()
        }
    }

    #[allow(unsafe_code)]
    #[unsafe(no_mangle)]
    fn android_main(app: android_activity::AndroidApp) {
        android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("solidcraft"));
        picker::install(app.clone());
        let data = app.internal_data_path();
        if let Some(dir) = &data {
            if let Err(e) = std::fs::create_dir_all(dir) {
                log::error!("data dir {}: {e}", dir.display());
            }
            let _ = solidcraft_engine::recovery::set_data_root(dir.clone());
        }
        let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
        if let WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
            setup.instance_descriptor.backends = eframe::wgpu::Backends::VULKAN | eframe::wgpu::Backends::GL;
        }
        let options = eframe::NativeOptions {
            android_app: Some(app.clone()),
            depth_buffer: 24,
            stencil_buffer: 8,
            multisampling: 1,
            persistence_path: data.as_ref().map(|d| d.join("app.ron")),
            persist_window: false,
            wgpu_options,
            viewport: egui::ViewportBuilder::default().with_title("SolidCraft"),
            ..Default::default()
        };
        let host = app;
        if let Err(e) = eframe::run_native(
            "SolidCraft",
            options,
            Box::new(move |cc| {
                solidcraft_ui_egui::fonts::setup_fonts(&cc.egui_ctx);
                let stored = cc.storage.and_then(|s| solidcraft_ui_egui::prefs::stored(|k| s.get_string(k)));
                let mut solve = SolveApp::new(Session::default(), services(data.clone()));
                solve.apply_off_thread = Some(solidcraft_ui_egui::dialogs::APPLY_AT_ONCE);
                if let Some(prefs) = stored {
                    solve.load_prefs(&prefs);
                } else {
                    solve.preferences.nav = solidcraft_ui_egui::prefs::TOUCH_NAV.into();
                }
                if solve.preferences.nav.is_empty() {
                    solve.preferences.nav = solidcraft_ui_egui::prefs::TOUCH_NAV.into();
                }
                if let Some(rs) = &cc.wgpu_render_state {
                    solve.set_wgpu(rs, 24, 8, rs.msaa_samples);
                    solidcraft_ui_egui::gpu::install_picture_renderer(rs);
                }
                if let Err(e) = solve.session.run_script(&solidcraft_engine::sample::script()) {
                    solve.set_status(format!("sample: {e}"), true);
                }
                solve.session.doc_mut().name = "Sample Plate".into();
                solve.session.mark_saved();
                if let Some(dir) = solidcraft_engine::recovery::default_dir() {
                    let every = if solve.autosave_minutes > 0.0 { solve.autosave_minutes * 60.0 } else { f64::MAX / 4.0 };
                    solidcraft_ui_egui::recovery_ui::start(&mut solve, &dir, std::time::Duration::from_secs_f64(every.min(1e9)));
                }
                if let Some(dir) = solidcraft_engine::crash::default_dir() {
                    let build = solidcraft_engine::crash::Build {
                        version: solidcraft_ui_egui::help::VERSION,
                        commit: solidcraft_ui_egui::help::COMMIT,
                        built: solidcraft_ui_egui::help::BUILD_DATE,
                    };
                    let crashed = solidcraft_engine::crash::install(dir, build);
                    solidcraft_ui_egui::crash_ui::announce(&mut solve, &crashed);
                }
                Ok(Box::new(App { solve, host }))
            }),
        ) {
            log::error!("SolidCraft failed to start: {e}");
        }
    }
}

#[cfg(not(target_os = "android"))]
pub fn android_only() {}
