//! PIRA's touch chrome (the PIRA themes, [`crate::theme::Tokens::touch`]), laid out like the
//! painting apps artists already know: a menu row (File, Edit, Layer…) across the top, a command
//! bar of large icon buttons under it, and size and opacity sliders beside the tool columns in
//! place of the brush tools' options bar.
//!
//! The command bar: New, Open and Save; Undo and Redo; Clear, Free Transform, Deselect and
//! Invert Selection; then Brush Settings. Pen, Pencil and Brush head the tool columns
//! (`toolbar::kit_buttons`).

use super::*;

/// Height of the menu row (points).
pub(crate) const MENU_HEIGHT: f32 = 36.0;
/// Height of the command bar: roomy enough for a fingertip.
pub(crate) const BAR_HEIGHT: f32 = 46.0;
/// Size of the command bar's icon buttons.
const ICON: f32 = 38.0;
/// Width of the size and opacity sliders.
pub(crate) const STRIP_WIDTH: f32 = 48.0;

/// Tools whose options the strip covers, so the options bar steps aside.
pub(crate) fn pill_tool(tool: Tool) -> bool {
    matches!(tool, Tool::Brush | Tool::Pencil | Tool::Eraser)
}

/// Run a menu command from the bar, reporting a failure in the status bar.
fn run(app: &mut PhotocraftApp, ctx: &egui::Context, id: &str) {
    if let Err(e) = crate::menus::invoke(app, ctx, id, json!({})) {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}

/// The command bar's buttons in groups: (icon, name, command).
const COMMANDS: [&[(&str, &str, &str)]; 3] = [
    &[("file-plus", "New", "file.new"), ("folder-open", "Open", "file.open"), ("save", "Save", "file.save")],
    &[("undo-2", "Undo", "edit.undo"), ("redo-2", "Redo", "edit.redo")],
    &[("trash", "Clear", "edit.clear"), ("scaling", "Free Transform", "edit.freeTransform"), ("square-dashed", "Deselect", "select.deselect"), ("squares-subtract", "Invert Selection", "select.inverse")],
];

/// A thin divider between the command bar's groups.
fn divider(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    let (r, _) = ui.allocate_exact_size(egui::vec2(1.0, ICON - 12.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.card_border);
    ui.add_space(6.0);
}

/// The menu row and the command bar (drawn by [`title_bar`] in the touch themes).
pub fn pira_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    // A brush picked anywhere keeps the Pen, Pencil and Brush buttons in step.
    crate::drawing_kit::follow_preset(app);
    let t = Tokens::get(ui.ctx());
    let (caption_left, caption_right) = crate::titlebar::reserved(app, ui.ctx());
    let margin = |reserved: f32| if reserved > 0.0 { 0 } else { 8 };
    egui::Panel::top("title_bar")
        .exact_size(crate::titlebar::bar_height(app, ui.ctx(), MENU_HEIGHT))
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: margin(caption_left), right: margin(caption_right), top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                if caption_left > 0.0 {
                    ui.add_space(caption_left + 4.0);
                }
                let (mark, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::hover());
                crate::brand::paint_mark(ui, mark);
                ui.add_space(6.0);
                crate::menus::menu_bar(app, ui);
            });
        });
    egui::Panel::top("pira_commands")
        .exact_size(BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin::symmetric(8, 0)))
        .show(ui, |ui| {
            let full = ui.max_rect();
            ui.painter().hline(full.x_range(), full.top(), egui::Stroke::new(1.0, t.card_border));
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, group) in COMMANDS.iter().enumerate() {
                    if i > 0 {
                        divider(ui);
                    }
                    for (icon, name, id) in group.iter() {
                        if icons::button(ui, icon, ICON, false, tl!(name)).clicked() {
                            run(app, ui.ctx(), id);
                        }
                    }
                }
                divider(ui);
                if icons::button(ui, "sliders-horizontal", ICON, app.ui.panels.brush_settings, tl!("Brush Settings")).clicked() {
                    app.ui.panels.brush_settings = !app.ui.panels.brush_settings;
                }
            });
        });
}

/// The size as the slider's bubble shows it: one decimal below 10 px.
pub(crate) fn size_label(size: f32) -> String {
    if size < 9.95 { format!("{size:.1}") } else { format!("{}", size.round()) }
}

/// The size and opacity sliders beside the tool columns (pill tools in the touch themes): each a
/// bubble with its value over a thin track with a round handle, size on top and opacity below.
/// Dragging along a track sets it; a stroke of the strip is one brush change, as with the
/// options bar.
pub fn pira_strip(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::left("pira_strip").exact_size(STRIP_WIDTH).resizable(false).frame(egui::Frame::NONE.fill(t.chrome)).show(ui, |ui| {
        let full = ui.max_rect().shrink2(egui::vec2(4.0, 10.0));
        let gap = 12.0;
        let h = ((full.height() - gap) / 2.0).max(80.0);
        let size_rect = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), h));
        let opacity_rect = egui::Rect::from_min_size(egui::pos2(full.left(), size_rect.bottom() + gap), egui::vec2(full.width(), h));
        let before = app.session.tools.brush.clone();
        let mut brush = before.clone();
        // Size runs 1..500 px on a square-root scale, so small pencils get most of the travel.
        let max = 500.0_f32;
        let mut s = ((brush.size.clamp(1.0, max) - 1.0) / (max - 1.0)).sqrt();
        if slider(ui, "pira-size", size_rect, &mut s, [&size_label(brush.size), "px"], tl!("Size")) {
            brush.resize_with_dual_brush((1.0 + s * s * (max - 1.0)).round().max(1.0));
        }
        let mut o = brush.opacity.clamp(0.01, 1.0);
        if slider(ui, "pira-opacity", opacity_rect, &mut o, [&format!("{}", (brush.opacity * 100.0).round()), "%"], tl!("Opacity")) {
            brush.opacity = (o * 100.0).round().clamp(1.0, 100.0) / 100.0;
        }
        crate::brush_panel::commit_gesture(app, ui.ctx(), &before, &brush);
    });
}

/// One vertical slider in `rect`: a bubble with the value's two lines at the top, then a thin
/// track whose top is 1. Returns whether `value` changed.
fn slider(ui: &mut egui::Ui, id: &str, rect: egui::Rect, value: &mut f32, lines: [&str; 2], tip: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let bubble = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 40.0));
    let knob = 11.0;
    let track = egui::Rect::from_x_y_ranges(rect.x_range(), (bubble.bottom() + 8.0 + knob)..=(rect.bottom() - knob));
    let response = ui.interact(rect, ui.scope_id().with(id), Sense::click_and_drag()).on_hover_text(tip);
    let mut changed = false;
    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
        && track.height() > 1.0
    {
        let v = ((track.bottom() - p.y) / track.height()).clamp(0.0, 1.0);
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            changed = true;
        }
    }
    let p = ui.painter();
    p.rect_filled(bubble, 8.0, t.field);
    p.text(egui::pos2(bubble.center().x, bubble.top() + 13.0), egui::Align2::CENTER_CENTER, lines[0], egui::FontId::proportional(13.0), t.text);
    p.text(egui::pos2(bubble.center().x, bubble.top() + 28.0), egui::Align2::CENTER_CENTER, lines[1], egui::FontId::proportional(11.0), t.text_dim);
    let x = track.center().x;
    let line = |top: f32, bottom: f32, color: Color32| {
        p.rect_filled(egui::Rect::from_x_y_ranges((x - 2.0)..=(x + 2.0), top..=bottom), 2.0, color);
    };
    line(track.top() - knob, track.bottom() + knob, t.field);
    let y = track.bottom() - track.height() * value.clamp(0.0, 1.0);
    line(y, track.bottom() + knob, t.text_faint);
    p.circle(egui::pos2(x, y), knob, t.text_dim, egui::Stroke::new(1.5, t.chrome));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_size_bubble_keeps_a_decimal_for_fine_brushes() {
        assert_eq!(size_label(11.6), "12");
        assert_eq!(size_label(3.24), "3.2");
        assert_eq!(size_label(9.99), "10");
        assert!(pill_tool(Tool::Pencil) && pill_tool(Tool::Eraser) && !pill_tool(Tool::Move));
    }
}
