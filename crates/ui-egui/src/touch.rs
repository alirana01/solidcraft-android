//! Phone layout and the Touch navigation preset's one-finger mode.
//!
//! Wide desktop windows keep the docked browser, timeline and mouse presets. A narrow window,
//! and every Android build, uses larger targets, a scrolling toolbar and overlay drawers.
//! Camera motion itself stays in [`crate::navigation`].

use egui::{Align2, Pos2, Rect, Sense, vec2};

use crate::SolveApp;
use crate::viewport::NavMode;

/// Logical width below which the desktop chrome is replaced by the phone layout.
pub const COMPACT_WIDTH: f32 = 720.0;

/// What one finger does while the Touch preset is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Orbit,
    Pan,
    Select,
}

/// Narrow window, or the Android build (a tablet in landscape included).
pub(crate) fn compact(ctx: &egui::Context) -> bool {
    if cfg!(target_os = "android") {
        return true;
    }
    let width = ctx.content_rect().width();
    width > 1.0 && width < COMPACT_WIDTH
}

/// Larger hit targets on a coarse pointer. Desktop spacing is put back when the window is wide.
pub(crate) fn apply_spacing(ctx: &egui::Context, compact: bool) {
    ctx.global_style_mut(|style| {
        if compact {
            style.spacing.interact_size = vec2(44.0, 44.0);
            style.spacing.button_padding = vec2(10.0, 8.0);
        } else {
            style.spacing.interact_size = vec2(40.0, 18.0);
            style.spacing.button_padding = vec2(6.0, 3.0);
        }
    });
}

/// One-finger mode. A sketch or a feature dialog selects until the chip is tapped; an empty
/// modelling view orbits. Leaving that context drops the chip's choice.
pub(crate) fn sync_tool(app: &mut SolveApp) {
    let selecting = app.session.active_sketch.is_some() || app.dialog.is_some();
    if app.viewport.touch_selecting != selecting {
        app.viewport.touch_selecting = selecting;
        app.viewport.touch_tool = None;
    }
}

pub(crate) fn tool(app: &SolveApp) -> Tool {
    app.viewport.touch_tool.unwrap_or(if app.viewport.touch_selecting { Tool::Select } else { Tool::Orbit })
}

/// Left-button navigation for the Touch preset. Geometry keeps the select/drag path.
pub(crate) fn primary_mode(tool: Tool, on_empty: bool) -> Option<NavMode> {
    match tool {
        Tool::Orbit if on_empty => Some(NavMode::Orbit),
        Tool::Pan => Some(NavMode::Pan),
        Tool::Select | Tool::Orbit => None,
    }
}

/// Orbit / Pan / Select, over the view. Only the Touch preset shows it.
pub(crate) fn chip(app: &mut SolveApp, ui: &mut egui::Ui, rect: Rect) {
    if app.preferences.nav != crate::prefs::TOUCH_NAV || !rect.is_positive() {
        return;
    }
    sync_tool(app);
    let current = tool(app);
    let t = crate::theme::Tokens::get();
    let labels = [(Tool::Orbit, "Orbit"), (Tool::Pan, "Pan"), (Tool::Select, "Select")];
    let width = labels.len() as f32 * 76.0 + 8.0;
    let bar = Rect::from_min_size(egui::pos2(rect.left() + 8.0, rect.top() + 8.0), vec2(width, 40.0));
    if !rect.contains_rect(bar) && rect.width() < bar.width() {
        return;
    }
    let painter = ui.painter().clone();
    painter.rect_filled(bar, 8.0, t.overlay);
    painter.rect_stroke(bar, 8.0, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    for (i, (mode, label)) in labels.into_iter().enumerate() {
        let br = Rect::from_min_size(egui::pos2(bar.left() + 4.0 + i as f32 * 76.0, bar.top() + 4.0), vec2(72.0, 32.0));
        let resp = ui.interact(br, ui.id().with(("touch_tool", label)), Sense::click());
        if current == mode {
            painter.rect_filled(br, 6.0, t.accent_soft);
        } else if resp.hovered() {
            painter.rect_filled(br, 6.0, t.hover);
        }
        painter.text(br.center(), Align2::CENTER_CENTER, app.tr_label(label), egui::FontId::proportional(13.0), t.text);
        if resp.clicked() {
            app.viewport.touch_tool = Some(mode);
        }
    }
}

/// Browser, timeline and the command search, along the bottom of a narrow window.
pub(crate) fn bottom_bar(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get();
    let bottom = ui.ctx().input(|i| i.safe_area_insets().0.bottom).clamp(0.0, 64.0);
    egui::Panel::bottom("sc_touch_bar")
        .exact_size(52.0 + bottom)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(t.app_bar))
        .show(ui, |ui| {
            let r = ui.max_rect();
            let y = r.top() + 26.0;
            let items = [("Browser", app.viewport.touch_browser), ("Timeline", app.viewport.touch_timeline), ("Commands", false)];
            let step = (r.width() / items.len() as f32).max(44.0);
            for (i, (label, on)) in items.into_iter().enumerate() {
                let br = Rect::from_center_size(egui::pos2(r.left() + step * (i as f32 + 0.5), y), vec2(step - 8.0, 44.0).min(vec2(160.0, 44.0)));
                let resp = ui.interact(br, ui.id().with(("touch_bar", label)), Sense::click());
                if on || resp.hovered() {
                    ui.painter().rect_filled(br, 8.0, if on { t.accent_soft } else { t.hover });
                }
                ui.painter().text(br.center(), Align2::CENTER_CENTER, app.tr_label(label), egui::FontId::proportional(14.0), t.app_bar_text);
                if resp.clicked() {
                    match label {
                        "Browser" => app.viewport.touch_browser = !app.viewport.touch_browser,
                        "Timeline" => app.viewport.touch_timeline = !app.viewport.touch_timeline,
                        _ => crate::shortcut_box::open(app, Some(Pos2::new(r.center().x, r.top() - 8.0))),
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{Tool, primary_mode};
    use crate::viewport::NavMode;

    #[test]
    fn one_finger_orbits_empty_space_and_selects_geometry() {
        assert_eq!(primary_mode(Tool::Orbit, true), Some(NavMode::Orbit));
        assert_eq!(primary_mode(Tool::Orbit, false), None);
        assert_eq!(primary_mode(Tool::Select, true), None);
        assert_eq!(primary_mode(Tool::Select, false), None);
        assert_eq!(primary_mode(Tool::Pan, false), Some(NavMode::Pan));
    }

    #[test]
    fn a_sketch_selects_until_the_chip_is_tapped() {
        let mut app = crate::SolveApp::new(solidcraft_engine::Session::default(), crate::Services::default());
        super::sync_tool(&mut app);
        assert_eq!(super::tool(&app), Tool::Orbit);
        app.session.active_sketch = Some(1);
        super::sync_tool(&mut app);
        assert_eq!(super::tool(&app), Tool::Select);
        app.viewport.touch_tool = Some(Tool::Orbit);
        assert_eq!(super::tool(&app), Tool::Orbit);
        app.session.active_sketch = None;
        super::sync_tool(&mut app);
        assert_eq!(super::tool(&app), Tool::Orbit);
        assert!(app.viewport.touch_tool.is_none());
    }

    #[test]
    fn a_held_touch_is_a_long_press() {
        use egui::{Event, PointerButton, TouchDeviceId, TouchId, TouchPhase, pos2, vec2};
        let ctx = egui::Context::default();
        ctx.options_mut(|o| o.input_options.max_click_duration = 0.4);
        let at = pos2(120.0, 80.0);
        let mut time = 0.0;
        let mut saw = false;
        // The widget has to exist on the frame before the press, or the hit test misses it.
        let mut events = vec![Event::PointerMoved(at)];
        for step in 0..16 {
            time += 0.05;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 700.0))),
                    time: Some(time),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    let resp = ui.interact(ui.max_rect(), ui.id().with("hold"), egui::Sense::click_and_drag());
                    saw |= resp.long_touched() || resp.secondary_clicked();
                },
            );
            out.textures_delta.clear();
            events = if step == 0 {
                vec![
                    Event::PointerMoved(at),
                    Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
                    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(1), phase: TouchPhase::Start, pos: at, force: None },
                ]
            } else {
                Vec::new()
            };
        }
        assert!(saw, "a still touch past the click duration opens the context path");
    }
}
