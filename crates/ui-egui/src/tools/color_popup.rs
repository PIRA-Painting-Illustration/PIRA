//! The colour popup (touch themes): a tap on the colour chips under the tool columns opens it
//! beside them, the way painting apps on tablets pick colours. From the top:
//!
//! - the main and sub colour (foreground and background) as overlapping chips: a tap picks which
//!   one the popup edits, with Swap and an Eyedropper beside them, then the colour's H, S, V and
//!   hex;
//! - a hue ring around a saturation and brightness square: drag the ring to turn the hue, the
//!   square for the shade;
//! - the colours used most recently, newest first;
//! - a swatch set (picked from the library's groups) as large squares.
//!
//! Each drag is one history step: its `tools.setColors` calls share a `coalesce` key, as the HUD
//! colour picker's do. A press anywhere else, or Esc, closes it.

use std::f32::consts::TAU;

use egui::{Color32, Mesh, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2};
use serde_json::json;

use crate::PhotocraftApp;
use crate::panels::{hsva_color32, hsva_srgb, srgb_bytes, srgb_hsva};
use crate::theme::Tokens;

/// Width of the popup's contents.
const WIDTH: f32 = 280.0;
/// Outer diameter of the hue ring.
const WHEEL: f32 = 240.0;
/// Width of the hue ring.
const RING: f32 = 22.0;
/// Side of a swatch or recent colour.
const SWATCH: f32 = 28.0;
/// How many recent colours are kept.
pub const RECENT: usize = 16;

/// What the popup keeps between frames: where it opened, which colour it edits, the hue it is on
/// (so greys keep their hue), the drag in progress and the swatch set shown.
#[derive(Clone, Debug)]
struct State {
    anchor: Rect,
    background: bool,
    hsv: [f32; 3],
    /// The colour `hsv` was taken from: when the colour changes elsewhere, the hue is read again.
    seen: [f32; 4],
    drag: Option<Drag>,
    group: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Hue(u64),
    Shade(u64),
}

fn id() -> egui::Id {
    egui::Id::unique("pira-color-popup")
}

fn state(ctx: &egui::Context) -> Option<State> {
    ctx.data(|d| d.get_temp::<State>(id()))
}

pub fn is_open(ctx: &egui::Context) -> bool {
    state(ctx).is_some()
}

/// Open the popup beside `anchor` (the colour chips) on the main colour, or on the sub colour
/// with `background`; close it if it is open.
pub fn toggle(app: &PhotocraftApp, ctx: &egui::Context, anchor: Rect, background: bool) {
    if is_open(ctx) {
        close(ctx);
        return;
    }
    let c = color(app, background);
    let group = ctx.data(|d| d.get_temp::<usize>(id().with("group"))).unwrap_or(0);
    ctx.data_mut(|d| d.insert_temp(id(), State { anchor, background, hsv: hsv_of(c), seen: c, drag: None, group }));
}

pub fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<State>(id()));
}

fn color(app: &PhotocraftApp, background: bool) -> [f32; 4] {
    if background { app.session.tools.background } else { app.session.tools.foreground }
}

fn hsv_of(c: [f32; 4]) -> [f32; 3] {
    let h = srgb_hsva(c);
    [h.h, h.s, h.v]
}

fn rgba_of(hsv: [f32; 3]) -> [f32; 4] {
    hsva_srgb(egui::ecolor::Hsva::new(hsv[0], hsv[1], hsv[2], 1.0))
}

fn color32(c: [f32; 4]) -> Color32 {
    let [r, g, b] = srgb_bytes(c);
    Color32::from_rgb(r, g, b)
}

/// `#rrggbb` for a colour.
pub(crate) fn hex(c: [f32; 4]) -> String {
    let [r, g, b] = srgb_bytes(c);
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// Put `c` at the front of the recent colours (moving it there if it is already in them).
pub(crate) fn remember(recent: &mut Vec<[f32; 4]>, c: [f32; 4]) {
    let same = |a: &[f32; 4]| srgb_bytes(*a) == srgb_bytes(c);
    recent.retain(|r| !same(r));
    recent.insert(0, c);
    recent.truncate(RECENT);
}

/// Set the edited colour; `drag` coalesces a drag's edits into one history step.
fn set(app: &mut PhotocraftApp, background: bool, c: [f32; 4], drag: Option<u64>) {
    let target = if background { "background" } else { "foreground" };
    let mut p = json!({ target: c });
    if let Some(key) = drag {
        p["coalesce"] = json!(format!("color-popup:{key}"));
    }
    if let Err(e) = app.run("tools.setColors", p) {
        app.ui.status = e;
    }
}

/// The hue (0..1) at `p` on a ring centred on `c`, red at the top, turning clockwise.
pub(crate) fn hue_at(c: Pos2, p: Pos2) -> f32 {
    let d = p - c;
    (d.x.atan2(-d.y) / TAU).rem_euclid(1.0)
}

/// The point on a circle of `radius` round `c` for hue `h` (as [`hue_at`] reads it).
fn on_ring(c: Pos2, radius: f32, h: f32) -> Pos2 {
    let a = h * TAU;
    c + vec2(a.sin(), -a.cos()) * radius
}

/// Show the popup if it is open. Run once a frame, after the tool columns.
pub fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(mut st) = state(ctx) else { return };
    let t = Tokens::get(ctx);
    let now = color(app, st.background);
    if srgb_bytes(now) != srgb_bytes(st.seen) {
        let hsv = hsv_of(now);
        // A grey (or black) has no hue of its own: keep the one the ring is on.
        st.hsv = if hsv[1] <= 0.0 || hsv[2] <= 0.0 { [st.hsv[0], hsv[1], hsv[2]] } else { hsv };
        st.seen = now;
    }
    let (pivot, pos) = if crate::direction::is_rtl() {
        (egui::Align2::RIGHT_BOTTOM, st.anchor.left_bottom() - vec2(8.0, 0.0))
    } else {
        (egui::Align2::LEFT_BOTTOM, st.anchor.right_bottom() + vec2(8.0, 0.0))
    };
    let mut picked: Option<[f32; 4]> = None;
    let mut swatch: Option<String> = None;
    let mut eyedropper = false;
    let mut swap = false;
    let mut released = false;
    let groups: Vec<String> = app.session.presets.swatches.iter().map(|g| g.name.clone()).collect();
    let table = crate::swatches_ui::colors(app, ctx);
    let recent = app.ui.recent_colors.clone();
    let fg = app.session.tools.foreground;
    let bg = app.session.tools.background;
    st.group = st.group.min(groups.len().saturating_sub(1));
    let area = egui::Area::new(id().with("area")).order(egui::Order::Foreground).pivot(pivot).fixed_pos(pos).constrain(true).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.card).inner_margin(10.0).show(ui, |ui| {
            ui.set_width(WIDTH);
            ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
            // The chips, Swap, Eyedropper, and the values.
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(58.0, 52.0), Sense::hover());
                let back = Rect::from_min_size(r.min + vec2(20.0, 16.0), vec2(34.0, 34.0));
                let front = Rect::from_min_size(r.min + vec2(2.0, 2.0), vec2(34.0, 34.0));
                // The chip being edited sits on top.
                let (top, under) = if st.background { ((back, bg, true), (front, fg, false)) } else { ((front, fg, false), (back, bg, true)) };
                for (rect, c, is_bg) in [under, top] {
                    let resp = ui.interact(rect, id().with(("chip", is_bg)), Sense::click());
                    let edited = is_bg == st.background;
                    ui.painter().circle_filled(rect.center(), rect.width() / 2.0 + 2.0, t.card);
                    ui.painter().circle_filled(rect.center(), rect.width() / 2.0, color32(c));
                    ui.painter().circle_stroke(rect.center(), rect.width() / 2.0, Stroke::new(if edited { 2.5 } else { 1.0 }, if edited { t.accent_text } else { t.field_border }));
                    let label = if is_bg { "Sub Color" } else { "Main Color" };
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, edited, tl!(label)));
                    if resp.clicked() && !edited {
                        st.background = is_bg;
                        st.seen = [f32::NAN; 4];
                    }
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
                    ui.horizontal(|ui| {
                        if crate::brush_picker::named(crate::icons::button(ui, "arrow-left-right", 30.0, false, tl!("Swap Colors")), "Swap Colors").clicked() {
                            swap = true;
                        }
                        if crate::brush_picker::named(crate::icons::button(ui, "pipette", 30.0, false, tl!("Eyedropper")), "Eyedropper").clicked() {
                            eyedropper = true;
                        }
                        ui.label(egui::RichText::new(hex(color(app, st.background))).monospace().size(13.0).color(t.text));
                    });
                    let [h, s, v] = st.hsv;
                    ui.label(
                        egui::RichText::new(format!("H {}°   S {}%   B {}%", (h * 360.0).round() as i32 % 360, (s * 100.0).round(), (v * 100.0).round()))
                            .size(12.0)
                            .color(t.text_dim),
                    );
                });
            });
            // The hue ring and the shade square.
            let (r, resp) = ui.allocate_exact_size(vec2(WIDTH, WHEEL), Sense::click_and_drag());
            let c = r.center();
            let outer = WHEEL / 2.0;
            let inner = outer - RING;
            let side = (inner - 8.0) * std::f32::consts::SQRT_2;
            let square = Rect::from_center_size(c, Vec2::splat(side));
            paint_ring(ui.painter(), c, inner, outer);
            let hue = st.hsv[0];
            ui.painter().add(crate::panels::color_grid(square, 16, |x, y| hsva_color32(egui::ecolor::Hsva::new(hue, x, 1.0 - y, 1.0))));
            let marker = on_ring(c, (inner + outer) / 2.0, hue);
            ui.painter().circle_stroke(marker, RING / 2.0 - 1.0, Stroke::new(2.5, Color32::WHITE));
            ui.painter().circle_stroke(marker, RING / 2.0 + 0.5, Stroke::new(1.0, Color32::BLACK));
            let dot = pos2(square.left() + st.hsv[1] * square.width(), square.top() + (1.0 - st.hsv[2]) * square.height());
            ui.painter().circle_stroke(dot, 7.0, Stroke::new(2.5, Color32::WHITE));
            ui.painter().circle_stroke(dot, 8.5, Stroke::new(1.0, Color32::BLACK));
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, tl!("Color Wheel")));
            if resp.is_pointer_button_down_on() && st.drag.is_none() {
                if let Some(p) = resp.interact_pointer_pos() {
                    let key = ctx.cumulative_pass_nr();
                    let d = (p - c).length();
                    st.drag = if d >= inner - 4.0 && d <= outer + 8.0 {
                        Some(Drag::Hue(key))
                    } else if square.expand(8.0).contains(p) {
                        Some(Drag::Shade(key))
                    } else {
                        None
                    };
                }
            }
            if let (Some(drag), Some(p)) = (st.drag, resp.interact_pointer_pos()) {
                let before = st.hsv;
                match drag {
                    Drag::Hue(_) => st.hsv[0] = hue_at(c, p),
                    Drag::Shade(_) => {
                        st.hsv[1] = ((p.x - square.left()) / square.width()).clamp(0.0, 1.0);
                        st.hsv[2] = (1.0 - (p.y - square.top()) / square.height()).clamp(0.0, 1.0);
                    }
                }
                if st.hsv != before {
                    let rgba = rgba_of(st.hsv);
                    st.seen = rgba;
                    picked = Some(rgba);
                }
            }
            if st.drag.is_some() && !ui.input(|i| i.pointer.any_down()) {
                st.drag = None;
                released = true;
            }
            // The recent colours.
            if !recent.is_empty() {
                ui.label(egui::RichText::new(tl!("Recent")).size(12.0).color(t.text_dim));
                if let Some(c) = swatch_rows(ui, recent.iter().map(|c| color32(*c)).collect(), "recent") {
                    picked = recent.get(c).copied();
                    released = true;
                }
            }
            // The swatch set.
            if !groups.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tl!("Swatches")).size(12.0).color(t.text_dim));
                    egui::ComboBox::from_id_salt(id().with("set")).selected_text(&groups[st.group]).width(WIDTH - 90.0).show_ui(ui, |ui| {
                        for (i, name) in groups.iter().enumerate() {
                            ui.selectable_value(&mut st.group, i, name);
                        }
                    });
                });
                let colors = table.get(st.group).cloned().unwrap_or_default();
                egui::ScrollArea::vertical().id_salt(id().with("swatches")).max_height(SWATCH * 4.0 + 6.0).show(ui, |ui| {
                    if let Some(i) = swatch_rows(ui, colors, "set") {
                        swatch = app.session.presets.swatches.get(st.group).and_then(|g| g.items.get(i)).map(|s| s.name.clone());
                    }
                });
            }
        });
    });
    let key = match st.drag {
        Some(Drag::Hue(k) | Drag::Shade(k)) => Some(k),
        None => None,
    };
    if swap {
        if let Err(e) = app.run("tools.swapColors", json!({})) {
            app.ui.status = e;
        }
    }
    if let Some(c) = picked {
        set(app, st.background, c, key);
    }
    if let Some(name) = swatch {
        crate::swatches_ui::use_swatch(app, &name, st.background);
        released = true;
    }
    if released {
        let c = color(app, st.background);
        remember(&mut app.ui.recent_colors, c);
    }
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.press_origin().is_some_and(|p| !area.response.rect.contains(p) && !st.anchor.contains(p)));
    // A combo box's list is its own popup; a press in it isn't outside.
    let in_list = egui::Popup::is_any_open(ctx);
    ctx.data_mut(|d| d.insert_temp(id().with("group"), st.group));
    if eyedropper {
        crate::tool_memory::select(app, crate::state::Tool::Eyedropper);
        close(ctx);
    } else if (outside && !in_list) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close(ctx);
    } else {
        ctx.data_mut(|d| d.insert_temp(id(), st));
    }
}

/// The hue ring as a mesh between radii `inner` and `outer` round `c`.
fn paint_ring(p: &egui::Painter, c: Pos2, inner: f32, outer: f32) {
    let n = 120;
    let mut mesh = Mesh::default();
    for i in 0..=n {
        let h = i as f32 / n as f32;
        let col = hsva_color32(egui::ecolor::Hsva::new(h, 1.0, 1.0, 1.0));
        mesh.colored_vertex(on_ring(c, inner, h), col);
        mesh.colored_vertex(on_ring(c, outer, h), col);
    }
    for i in 0..n as u32 {
        let a = i * 2;
        mesh.add_triangle(a, a + 1, a + 3);
        mesh.add_triangle(a, a + 3, a + 2);
    }
    p.add(mesh);
}

/// Colours as rows of squares across the popup. Returns the index tapped.
fn swatch_rows(ui: &mut egui::Ui, colors: Vec<Color32>, salt: &str) -> Option<usize> {
    let t = Tokens::get(ui.ctx());
    let per_row = ((WIDTH + 2.0) / (SWATCH + 2.0)).floor().max(1.0) as usize;
    let mut hit = None;
    for (row, chunk) in colors.chunks(per_row).enumerate() {
        let (r, _) = ui.allocate_exact_size(vec2(WIDTH, SWATCH), Sense::hover());
        for (i, c) in chunk.iter().enumerate() {
            let cell = Rect::from_min_size(pos2(r.left() + i as f32 * (SWATCH + 2.0), r.top()), Vec2::splat(SWATCH));
            let index = row * per_row + i;
            let resp = ui.interact(cell, id().with((salt, index)), Sense::click());
            ui.painter().rect_filled(cell, 4.0, *c);
            ui.painter().rect_stroke(cell, 4.0, Stroke::new(1.0, if resp.hovered() { t.text } else { t.field_border }), egui::StrokeKind::Inside);
            if resp.clicked() {
                hit = Some(index);
            }
        }
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_reads_red_at_the_top_and_turns_clockwise() {
        let c = pos2(100.0, 100.0);
        assert!(hue_at(c, pos2(100.0, 50.0)) < 1e-4, "red at the top");
        assert!((hue_at(c, pos2(150.0, 100.0)) - 0.25).abs() < 1e-4, "a quarter turn to the right");
        for h in [0.1, 0.4, 0.75] {
            assert!((hue_at(c, on_ring(c, 40.0, h)) - h).abs() < 1e-4);
        }
    }

    #[test]
    fn recent_colours_move_to_the_front_without_repeats() {
        let mut recent = Vec::new();
        for i in 0..20 {
            remember(&mut recent, [i as f32 / 20.0, 0.0, 0.0, 1.0]);
        }
        assert_eq!(recent.len(), RECENT);
        let red = [1.0, 0.0, 0.0, 1.0];
        remember(&mut recent, red);
        remember(&mut recent, [0.0, 0.0, 1.0, 1.0]);
        remember(&mut recent, red);
        assert_eq!(recent[0], red);
        assert_eq!(recent.iter().filter(|c| **c == red).count(), 1);
        assert_eq!(hex(red), "#FF0000");
    }

    #[test]
    fn a_drag_on_the_popup_is_one_history_step_and_a_recent_colour() {
        use egui_kittest::{Harness, kittest::Queryable};
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.run("tools.setColors", json!({"foreground": "#ff0000"})).unwrap();
        let mut h = Harness::builder().with_size(vec2(800.0, 700.0)).with_step_dt(1.0 / 60.0).build_ui_state(
            |ui, app: &mut PhotocraftApp| {
                let ctx = ui.ctx().clone();
                if !ctx.fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                    return;
                }
                if !is_open(&ctx) && app.ui.status.is_empty() {
                    toggle(app, &ctx, Rect::from_min_size(pos2(10.0, 600.0), vec2(40.0, 40.0)), false);
                    app.ui.status = "opened".into();
                }
                show(app, &ctx);
            },
            app,
        );
        crate::PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::Pira);
        h.run_steps(4);
        assert!(is_open(&h.ctx));
        let wheel = h.get_by_label(tl!("Color Wheel")).rect();
        let j0 = h.state().session.journal.len();
        // Drag across the shade square, from its top right towards its bottom.
        let c = wheel.center();
        h.event(egui::Event::PointerMoved(c + vec2(40.0, -40.0)));
        h.event(egui::Event::PointerButton { pos: c + vec2(40.0, -40.0), button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
        h.run_steps(2);
        for k in 1..6 {
            h.event(egui::Event::PointerMoved(c + vec2(40.0, -40.0 + k as f32 * 10.0)));
            h.run_steps(1);
        }
        h.event(egui::Event::PointerButton { pos: c + vec2(40.0, 10.0), button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
        h.run_steps(3);
        let sets: Vec<&serde_json::Value> = h.state().session.journal[j0..].iter().filter(|(id, _)| id == "tools.setColors").map(|(_, p)| p).collect();
        assert!(sets.len() > 1, "the drag picks live: {sets:?}");
        let keys: Vec<&str> = sets.iter().map(|p| p["coalesce"].as_str().unwrap()).collect();
        assert!(keys.windows(2).all(|w| w[0] == w[1]), "one drag, one step: {keys:?}");
        let fg = h.state().session.tools.foreground;
        assert_ne!(srgb_bytes(fg), [255, 0, 0], "the colour moved");
        assert_eq!(h.state().ui.recent_colors.first().map(|c| srgb_bytes(*c)), Some(srgb_bytes(fg)), "the pick is a recent colour");
        assert!(is_open(&h.ctx), "the popup stays open while picking");
    }
}
