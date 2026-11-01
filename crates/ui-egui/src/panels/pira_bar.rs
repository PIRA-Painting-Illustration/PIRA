//! PIRA's touch chrome (the PIRA themes, [`crate::theme::Tokens::touch`]): a slim top bar in
//! place of the desktop menu row and title bar, and a size and opacity strip beside the tool
//! rail in place of the brush tools' options bar.
//!
//! The bar: the P app menu ([`crate::menus::pira_menu`]) and the document name at the start; the
//! brush pill (preset, size, opacity; a tap opens the brush picker) in the middle; undo, redo,
//! free transform, brush settings, layers and the foreground colour at the end.

use super::*;

/// Height of the PIRA bar (points): roomy enough for a fingertip.
pub(crate) const BAR_HEIGHT: f32 = 46.0;
/// Size of the bar's icon buttons.
const ICON: f32 = 34.0;
/// Width of the size and opacity strip.
pub(crate) const STRIP_WIDTH: f32 = 40.0;

/// Tools whose options the brush pill and the strip cover, so the options bar steps aside.
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

/// The brush pill's text: the preset (or tool) and, for the pill tools, size and opacity.
pub(crate) fn pill_text(app: &PhotocraftApp) -> String {
    let tool = app.ui.tool;
    if !pill_tool(tool) {
        return tool.label().to_string();
    }
    let b = &app.session.tools.brush;
    let name = app.session.tools.current_preset.clone().unwrap_or_else(|| tool.label().to_string());
    format!("{name}  ·  {} px  ·  {}%", b.size.round(), (b.opacity * 100.0).round())
}

/// The PIRA bar (drawn by [`title_bar`] in the touch themes).
pub fn pira_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    // A brush picked anywhere keeps the Pen, Pencil and Brush buttons in step.
    crate::drawing_kit::follow_preset(app);
    let t = Tokens::get(ui.ctx());
    let (caption_left, caption_right) = crate::titlebar::reserved(app, ui.ctx());
    let margin = |reserved: f32| if reserved > 0.0 { 0 } else { 10 };
    egui::Panel::top("title_bar")
        .exact_size(crate::titlebar::bar_height(app, ui.ctx(), BAR_HEIGHT))
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: margin(caption_left), right: margin(caption_right), top: 0, bottom: 0 }))
        .show(ui, |ui| {
            let full = ui.max_rect();
            ui.horizontal_centered(|ui| {
                if caption_left > 0.0 {
                    ui.add_space(caption_left + 4.0);
                }
                ui.spacing_mut().item_spacing.x = 8.0;
                crate::menus::pira_menu::app_menu(app, ui);
                // The document, with its size; the tabs below name every open one.
                let title = match app.session.active() {
                    Some(d) => {
                        let (w, h) = (d.doc.size.width, d.doc.size.height);
                        format!("{}{}  ·  {w}×{h}", d.doc.name, if d.is_dirty() { " •" } else { "" })
                    }
                    None => "PIRA".to_string(),
                };
                ui.add(egui::Label::new(egui::RichText::new(title).color(t.text_dim).size(13.0)).truncate());
                let start_end = ui.min_rect().right();
                // The end group, laid out from the bar's end inwards.
                let end = egui::Rect::from_min_max(egui::pos2(start_end, full.top()), egui::pos2(full.right() - caption_right, full.bottom()));
                let mut end_ui = ui.new_child(egui::UiBuilder::new().max_rect(end).layout(egui::Layout::right_to_left(egui::Align::Center)));
                let group_start = {
                    let ui = &mut end_ui;
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if caption_right > 0.0 {
                        ui.add_space(4.0);
                    }
                    colour_chip(app, ui);
                    ui.add_space(4.0);
                    if icons::button(ui, "layers", ICON, app.ui.panels.layers, tl!("Layers")).clicked() {
                        app.ui.panels.layers = !app.ui.panels.layers;
                    }
                    if icons::button(ui, "sliders-horizontal", ICON, app.ui.panels.brush_settings, tl!("Brush Settings")).clicked() {
                        app.ui.panels.brush_settings = !app.ui.panels.brush_settings;
                    }
                    if icons::button(ui, "scaling", ICON, false, tl!("Free Transform")).clicked() {
                        run(app, ui.ctx(), "edit.freeTransform");
                    }
                    ui.add_space(6.0);
                    if icons::button(ui, "redo-2", ICON, false, tl!("Redo")).clicked() {
                        run(app, ui.ctx(), "edit.redo");
                    }
                    if icons::button(ui, "undo-2", ICON, false, tl!("Undo")).clicked() {
                        run(app, ui.ctx(), "edit.undo");
                    }
                    ui.add_space(10.0);
                    // Pen, Pencil and Brush, laid out from the end inwards so Pen reads first.
                    let active = crate::drawing_kit::active(app);
                    for kit in crate::drawing_kit::Kit::ALL.into_iter().rev() {
                        let label = kit.label();
                        let b = crate::brush_picker::named(icons::button(ui, kit.icon(), ICON, active == Some(kit), tl!(label)), label);
                        if b.clicked() {
                            crate::drawing_kit::switch(app, kit);
                        }
                    }
                    ui.min_rect().left()
                };
                // The pill, centred in the room between the two groups.
                let room = egui::Rect::from_x_y_ranges((start_end + 12.0)..=(group_start - 12.0).max(start_end + 12.0), full.y_range());
                brush_pill(app, ui, room);
            });
        });
}

/// The brush pill, centred in `room`: a tap opens the brush picker under it.
fn brush_pill(app: &mut PhotocraftApp, ui: &mut egui::Ui, room: egui::Rect) {
    let t = Tokens::get(ui.ctx());
    if room.width() < 120.0 {
        return;
    }
    let text = pill_text(app);
    let galley = ui.painter().layout_no_wrap(text, egui::FontId::proportional(13.0), t.text);
    let width = (galley.size().x + 48.0).min(room.width());
    let rect = egui::Rect::from_center_size(room.center(), egui::vec2(width, 32.0));
    let response = ui.interact(rect, ui.scope_id().with("pira-brush-pill"), Sense::click()).on_hover_text(tl!("Brushes"));
    let fill = if response.hovered() { t.hover } else { t.card };
    ui.painter().rect(rect, 16.0, fill, egui::Stroke::new(1.0, t.card_border), egui::StrokeKind::Inside);
    // The tool's colour dot, then the text.
    let dot = egui::pos2(rect.left() + 18.0, rect.center().y);
    ui.painter().circle_filled(dot, 5.0, t.accent);
    let text_pos = egui::pos2(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().with_clip_rect(rect.shrink(4.0)).galley(text_pos, galley, t.text);
    if response.clicked() && pill_tool(app.ui.tool) {
        crate::brush_picker::open(&mut app.ui, ui.ctx(), [rect.left(), rect.bottom() + 4.0]);
    }
}

/// The foreground colour as a ring; a tap opens its picker.
fn colour_chip(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let [r, g, b, _] = app.session.tools.foreground;
    let fg = Color32::from_rgb((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ICON, ICON), Sense::click());
    let response = response.on_hover_text(tl!("Foreground Color"));
    ui.painter().circle_filled(rect.center(), 12.0, fg);
    ui.painter().circle_stroke(rect.center(), 13.5, egui::Stroke::new(3.0, t.primary_bg));
    if response.clicked() {
        crate::color_picker_ui::open(app, "foreground");
    }
}

/// The size and opacity strip beside the tool rail (pill tools in the touch themes): two
/// capsules, size in the accent colour on top, opacity in the warning colour below. Dragging
/// along one sets it; a stroke of the strip is one brush change, as with the options bar.
pub fn pira_strip(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::left("pira_strip").exact_size(STRIP_WIDTH).resizable(false).frame(egui::Frame::NONE.fill(t.chrome)).show(ui, |ui| {
        let full = ui.max_rect().shrink2(egui::vec2(9.0, 14.0));
        let gap = 14.0;
        let h = ((full.height() - gap) / 2.0).max(40.0);
        let size_rect = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), h));
        let opacity_rect = egui::Rect::from_min_size(egui::pos2(full.left(), size_rect.bottom() + gap), egui::vec2(full.width(), h));
        let before = app.session.tools.brush.clone();
        let mut brush = before.clone();
        // Size runs 1..500 px on a square-root scale, so small pencils get most of the travel.
        let max = 500.0_f32;
        let mut s = ((brush.size.clamp(1.0, max) - 1.0) / (max - 1.0)).sqrt();
        if capsule(ui, "pira-size", size_rect, &mut s, t.accent, tl!("Size")) {
            brush.resize_with_dual_brush((1.0 + s * s * (max - 1.0)).round().max(1.0));
        }
        let mut o = brush.opacity.clamp(0.01, 1.0);
        if capsule(ui, "pira-opacity", opacity_rect, &mut o, t.warning, tl!("Opacity")) {
            brush.opacity = (o * 100.0).round().clamp(1.0, 100.0) / 100.0;
        }
        crate::brush_panel::commit_gesture(app, ui.ctx(), &before, &brush);
    });
}

/// One vertical capsule slider: `value` 0..1 fills from the bottom. Returns whether it changed.
fn capsule(ui: &mut egui::Ui, id: &str, rect: egui::Rect, value: &mut f32, fill: Color32, tip: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let response = ui.interact(rect, ui.scope_id().with(id), Sense::click_and_drag()).on_hover_text(tip);
    let mut changed = false;
    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        let v = ((rect.bottom() - p.y) / rect.height()).clamp(0.0, 1.0);
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            changed = true;
        }
    }
    let radius = rect.width() / 2.0;
    ui.painter().rect_filled(rect, radius, t.field);
    let filled = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - rect.height() * value.clamp(0.0, 1.0)), rect.max);
    if filled.height() > 0.5 {
        ui.painter().rect_filled(filled, radius, fill.gamma_multiply(0.85));
    }
    // The handle: a cream bar where the fill ends.
    let y = filled.top().clamp(rect.top() + 3.0, rect.bottom() - 3.0);
    ui.painter().line_segment([egui::pos2(rect.left() + 5.0, y), egui::pos2(rect.right() - 5.0, y)], egui::Stroke::new(3.0, t.text));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pill_names_the_brush_size_and_opacity() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.ui.tool = Tool::Brush;
        app.session.tools.current_preset = Some("2B Pencil".into());
        app.session.tools.brush.size = 7.0;
        app.session.tools.brush.opacity = 0.5;
        assert_eq!(pill_text(&app), "2B Pencil  ·  7 px  ·  50%");
        app.ui.tool = Tool::Move;
        assert_eq!(pill_text(&app), Tool::Move.label());
        assert!(pill_tool(Tool::Pencil) && pill_tool(Tool::Eraser) && !pill_tool(Tool::Move));
    }
}
