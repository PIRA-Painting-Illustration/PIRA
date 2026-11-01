//! The drawing tool popup (touch themes): a second tap on the Pen, Pencil or Brush in the tool
//! columns opens it beside them, the way painting apps on tablets let you change brushes. Its top
//! half lists the tool's brushes, each as a stroke with its name; a tap switches to that brush
//! and leaves the popup open. Its bottom half holds the brush's everyday settings: size,
//! opacity, stabilization and, for textured brushes, the texture's density. The full editor is
//! one button away (Brush Settings). A press anywhere else, or Esc, closes it.
//!
//! Brush picks run `tools.setBrush` and slider changes go through
//! [`crate::brush_panel::commit_gesture`], so every change is a journaled command.

use egui::{Color32, Rect, Sense, pos2, vec2};
use serde_json::json;

use crate::PhotocraftApp;
use crate::drawing_kit::{self, Kit};
use crate::theme::Tokens;

/// Width of the popup's contents.
const WIDTH: f32 = 300.0;
/// Height of a brush row.
const ROW: f32 = 40.0;
/// Width of a row's stroke preview.
const STROKE_W: f32 = 150.0;
/// Height of a setting row (label and value, then its track).
const SETTING: f32 = 46.0;
/// Largest size the size slider reaches (bigger sizes stay settable in Brush Settings).
const MAX_SIZE: f32 = 500.0;

fn id() -> egui::Id {
    egui::Id::unique("pira-kit-popup")
}

/// Is the popup open?
pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Rect>(id())).is_some()
}

/// Open the popup beside `anchor` (the tool's button), or close it if it is open.
pub fn toggle(ctx: &egui::Context, anchor: Rect) {
    if is_open(ctx) {
        close(ctx);
    } else {
        ctx.data_mut(|d| d.insert_temp(id(), anchor));
    }
}

pub fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Rect>(id()));
}

/// A setting as its slider shows it, 0..1 along the track, and back.
fn size_to_track(size: f32) -> f32 {
    ((size.clamp(1.0, MAX_SIZE) - 1.0) / (MAX_SIZE - 1.0)).sqrt()
}

fn track_to_size(v: f32) -> f32 {
    (1.0 + v * v * (MAX_SIZE - 1.0)).round().max(1.0)
}

/// Show the popup if it is open. Run once a frame, after the tool columns.
pub fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(anchor) = ctx.data(|d| d.get_temp::<Rect>(id())) else { return };
    let Some(kit) = drawing_kit::active(app) else {
        close(ctx);
        return;
    };
    let t = Tokens::get(ctx);
    let (pivot, pos) = if crate::direction::is_rtl() {
        (egui::Align2::RIGHT_TOP, anchor.left_top() - vec2(8.0, 0.0))
    } else {
        (egui::Align2::LEFT_TOP, anchor.right_top() + vec2(8.0, 0.0))
    };
    let presets = drawing_kit::picker_presets(app).unwrap_or_default();
    let current = app.session.tools.current_preset.clone();
    let before = app.session.tools.brush.clone();
    let mut brush = before.clone();
    let mut pick: Option<String> = None;
    let mut settings = false;
    let area = egui::Area::new(id().with("area")).order(egui::Order::Foreground).pivot(pivot).fixed_pos(pos).constrain(true).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.card).inner_margin(8.0).show(ui, |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
            caption(ui, tl!("Tool Group"));
            group_tab(ui, kit);
            ui.add_space(4.0);
            egui::ScrollArea::vertical().id_salt("kit-popup-brushes").max_height(ROW * 6.5).show(ui, |ui| {
                for p in &presets {
                    if brush_row(ui, p, current.as_deref() == Some(p.name.as_str())) {
                        pick = Some(p.name.clone());
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let b = crate::brush_picker::named(crate::icons::button(ui, "sliders-horizontal", 30.0, false, tl!("Brush Settings")), "Brush Settings");
                settings = b.clicked();
            });
            ui.add_space(6.0);
            let line = ui.allocate_exact_size(vec2(WIDTH, 1.0), Sense::hover()).0;
            ui.painter().rect_filled(line, 0.0, t.card_border);
            ui.add_space(4.0);
            caption(ui, tl!("Tool Settings"));
            sample(ui, current.as_deref().unwrap_or(kit.label()), &brush);
            ui.add_space(4.0);
            let mut s = size_to_track(brush.size);
            if setting(ui, tl!("Brush Size"), &crate::panels::pira_bar::size_label(brush.size), &mut s) {
                brush.resize_with_dual_brush(track_to_size(s));
            }
            let mut o = brush.opacity.clamp(0.0, 1.0);
            if setting(ui, tl!("Opacity"), &format!("{}", (brush.opacity * 100.0).round()), &mut o) {
                brush.opacity = (o * 100.0).round().clamp(1.0, 100.0) / 100.0;
            }
            if brush.texture.enabled {
                let mut d = brush.texture.depth.clamp(0.0, 1.0);
                if setting(ui, tl!("Texture Density"), &format!("{}", (brush.texture.depth * 100.0).round()), &mut d) {
                    brush.texture.depth = (d * 100.0).round() / 100.0;
                }
            }
            let mut st = if brush.smoothing.enabled { brush.smoothing.amount.clamp(0.0, 1.0) } else { 0.0 };
            if setting(ui, tl!("Stabilization"), &format!("{}", (st * 100.0).round()), &mut st) {
                brush.smoothing.amount = (st * 100.0).round() / 100.0;
                brush.smoothing.enabled = brush.smoothing.amount > 0.0;
            }
        });
    });
    if let Some(name) = pick
        && current.as_deref() != Some(name.as_str())
    {
        if let Err(e) = app.run("tools.setBrush", json!({ "preset": name })) {
            app.ui.status = e;
        }
        drawing_kit::follow_preset(app);
    } else {
        crate::brush_panel::commit_gesture(app, ctx, &before, &brush);
    }
    if settings {
        crate::brush_picker::open_settings(app, ctx);
    }
    // A press outside the popup and its button closes it; so does Esc.
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.press_origin().is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p)));
    if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close(ctx);
    }
}

/// A small centred section title.
fn caption(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(WIDTH, 20.0), Sense::hover());
    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(12.0), t.text_dim);
}

/// The tool's group as a tab: its icon and name.
fn group_tab(ui: &mut egui::Ui, kit: Kit) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(WIDTH, 34.0), Sense::hover());
    let tab = Rect::from_min_size(r.min, vec2(WIDTH * 0.62, r.height()));
    ui.painter().rect_filled(tab, t.radius_sm, t.accent_soft);
    crate::icons::paint(ui, Rect::from_center_size(pos2(tab.left() + 18.0, tab.center().y), vec2(20.0, 20.0)), kit.icon(), 18.0, t.text);
    ui.painter().text(pos2(tab.left() + 36.0, tab.center().y), egui::Align2::LEFT_CENTER, tl!(kit.label()), egui::FontId::proportional(14.0), t.text);
}

/// A brush row: its stroke, its name at the end. Returns whether it was tapped.
fn brush_row(ui: &mut egui::Ui, p: &photocraft_engine::paint::BrushPreset, current: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, current, &p.name));
    if current {
        ui.painter().rect_filled(r, t.radius_sm, t.accent_soft);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, t.radius_sm, t.hover);
    }
    if ui.is_rect_visible(r) {
        let stroke = crate::brush_preview::stroke_texture(ui.ctx(), &format!("kit-stroke:{}", p.name), &p.brush, STROKE_W as u32, 32, t.text);
        let sr = Rect::from_min_size(pos2(r.left() + 6.0, r.center().y - 16.0), vec2(STROKE_W, 32.0));
        ui.painter().image(stroke.id(), sr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        ui.painter().with_clip_rect(r).text(
            pos2(r.right() - 8.0, r.center().y),
            egui::Align2::RIGHT_CENTER,
            &p.name,
            egui::FontId::proportional(13.0),
            if current { t.text } else { t.text_dim },
        );
    }
    resp.clicked()
}

/// The current brush's name over a sample of its stroke.
fn sample(ui: &mut egui::Ui, name: &str, brush: &photocraft_engine::BrushSettings) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(WIDTH, 56.0), Sense::hover());
    ui.painter().rect_filled(r, t.radius_sm, t.field);
    let stroke = crate::brush_preview::stroke_texture(ui.ctx(), "kit-sample", brush, (WIDTH - 20.0) as u32, 40, t.text);
    ui.painter().image(stroke.id(), Rect::from_min_size(r.min + vec2(10.0, 12.0), vec2(WIDTH - 20.0, 40.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    ui.painter().text(r.min + vec2(8.0, 4.0), egui::Align2::LEFT_TOP, name, egui::FontId::proportional(12.0), t.text_dim);
}

/// One setting: its name and value on a line, a track under them; dragging or tapping along the
/// row sets `value` (0..1). Returns whether it changed.
fn setting(ui: &mut egui::Ui, label: &str, shown: &str, value: &mut f32) -> bool {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(WIDTH, SETTING), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, label));
    let track = Rect::from_x_y_ranges((r.left() + 10.0)..=(r.right() - 10.0), (r.bottom() - 14.0)..=(r.bottom() - 8.0));
    let mut changed = false;
    if (resp.dragged() || resp.clicked())
        && let Some(p) = resp.interact_pointer_pos()
    {
        let v = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            changed = true;
        }
    }
    let p = ui.painter();
    p.text(pos2(r.left() + 10.0, r.top() + 12.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.0), t.text);
    p.text(pos2(r.right() - 10.0, r.top() + 12.0), egui::Align2::RIGHT_CENTER, shown, egui::FontId::proportional(13.0), t.text);
    p.rect_filled(track, 3.0, t.field);
    let filled = Rect::from_min_max(track.min, pos2(track.left() + track.width() * value.clamp(0.0, 1.0), track.bottom()));
    p.rect_filled(filled, 3.0, t.accent);
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_size_track_round_trips() {
        for size in [1.0, 3.0, 12.0, 80.0, 500.0] {
            assert_eq!(track_to_size(size_to_track(size)), size);
        }
        assert_eq!(size_to_track(5000.0), 1.0, "past the end stays at the end");
    }
}
