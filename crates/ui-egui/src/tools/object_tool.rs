//! Object tool and the 3D panel: posing the figures of a 3D layer on the canvas.
//!
//! On a 3D layer, a drag on a body part swings it about its joint in the canvas plane (with
//! Twist on, or ⌥, it turns about its own bone instead); a drag on the hips slides the figure
//! across its floor (with Twist, turns it). A drag on empty canvas orbits the camera; ⇧ pans and
//! ⌘/Ctrl dollies, and the options bar can make a plain drag do either. Each drag is one undo
//! step. While dragging, the figures are drawn as a quick low-resolution preview over the canvas
//! (`s3d::preview`) and the layer keeps no image (`scene3d.set` with `hide`); the release sends
//! the scene once more with a full render, coalesced into the same undo step. (Rendering the
//! layer on every move took a third of a second a step on a tablet with a photo-sized canvas.)
//!
//! The 3D panel (beside Brushes) adds mannequins and applies poses from the pose library.
//!
//! What a drag does, the handles and the overlays come from `photocraft-scene3d-egui`, shared with
//! PIRA Pose (`apps/pira-pose`), so the two pose the same way.
//!
//! After Match photo › Body, the photo's joints show as coral dots while the Match photo tab is
//! open, until Done: dragging one moves that joint (the dot follows the pen; the figure refits
//! once, on release, with `scene3d.refitPhoto`), and the tab sets each limb towards or away from
//! the camera.
//!
//! Joints (options bar) shows the same kind of dots on every figure's head, elbows, wrists, knees
//! and ankles: dragging one puts that joint under the pen (`pose::reach`: a wrist or ankle bends
//! its arm or leg to get there).

use egui::{Color32, Pos2, Stroke};
use photocraft_doc::{Body3d, LayerContent, LayerId, Scene3d};
use photocraft_scene3d::{self as s3d, library, light};
use photocraft_scene3d_egui as pose_ui;
pub use photocraft_scene3d_egui::DragKind;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::canvas::{ToolEvent, ViewXform};
use crate::state::Tool;
use crate::theme::Tokens;

mod gravity;
pub use gravity::{cancel_fall, drop_figure, keep_fall, tick as tick_gravity};

/// What a plain drag does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DragMode {
    /// Pose the part under the pen; empty canvas orbits the camera.
    #[default]
    Pose,
    Orbit,
    Pan,
    Zoom,
    /// Place the main light: the sun goes where you drag (the disc's rim is side-on, beyond it is
    /// behind the figures).
    Light,
    /// Tap a part to pin it (or unpin it) for Gravity.
    Pin,
}

/// The 3D panel's tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelTab {
    /// The pose library (the grid gets the panel's full height).
    #[default]
    Poses,
    Hands,
    Photo,
    Figures,
    Gravity,
}

/// Object tool and 3D panel state (serde: readable and settable through the control channel).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectToolUi {
    pub mode: DragMode,
    /// Drags turn parts about their own bone (and the hips turn the figure).
    pub twist: bool,
    /// The selected part: layer, figure, joint.
    pub selected: Option<(u64, usize, usize)>,
    /// 3D panel: the pose category shown, and the search text.
    pub category: String,
    pub search: String,
    /// 3D panel: which hands a hand pose changes ("both", "left", "right").
    pub hands: String,
    /// 3D panel: the tab shown.
    pub tab: PanelTab,
    /// 3D panel › Photo: match bodies with the accurate (Heavy) model.
    pub accurate: bool,
    /// 3D panel › Photo: the last body match is being fixed (its joints show on the canvas).
    #[serde(skip)]
    pub fixing: bool,
    /// Joints: the figures' main joints show as dots to drag into place.
    pub joints: bool,
    #[serde(skip)]
    pub drag: Option<ObjectDrag>,
    #[serde(skip)]
    drags: u64,
    /// Bumped as each Light-menu slider drag starts, so each drag is one undo step.
    #[serde(skip)]
    light_edits: u64,
    /// The Light menu's colour picker is open (inline: a popup would close the menu).
    #[serde(skip)]
    light_picker: bool,
    /// Which light a Light-mode drag away from both suns places (the menu's choice).
    #[serde(skip)]
    light_target: light::Which,
    /// 3D panel › Gravity: its settings, pins and the fall being shown.
    #[serde(skip)]
    pub gravity: gravity::GravityUi,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectDrag {
    layer: LayerId,
    kind: DragKind,
    scene: Scene3d,
    size: (u32, u32),
    last: [f64; 2],
    moved: bool,
    key: String,
    /// The layer's own image is hidden while the drag shows `preview`.
    hidden: bool,
    preview: Option<std::sync::Arc<s3d::Preview>>,
}

/// Most pixels a drag preview renders for the whole canvas.
const PREVIEW_PIXELS: u64 = 350_000;

/// Hides the layer's image (once per drag) and renders the drag's preview of `d.scene`.
fn show_preview(app: &mut PhotocraftApp, d: &mut ObjectDrag) {
    if !d.hidden {
        let p = json!({"layer": d.layer.0, "scene": d.scene, "hide": true, "coalesce": d.key});
        let r = app.run("scene3d.set", p);
        status(app, r);
        d.hidden = true;
    }
    let (w, h) = d.size;
    d.preview = Some(std::sync::Arc::new(s3d::preview(&d.scene, w, h, PREVIEW_PIXELS)));
}

/// The active layer's id and scene, when it is a 3D layer, with the canvas size.
fn active_scene(app: &PhotocraftApp) -> Option<(LayerId, Scene3d, (u32, u32))> {
    let st = app.session.active()?;
    let id = st.active_layer?;
    match &st.doc.layer(id)?.content {
        LayerContent::Scene3d(sc) => Some((id, sc.scene.clone(), (st.doc.size.width, st.doc.size.height))),
        _ => None,
    }
}

fn status(app: &mut PhotocraftApp, r: Result<Value, String>) {
    if let Err(e) = r {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}

fn f32p(p: [f64; 2]) -> [f32; 2] {
    [p[0] as f32, p[1] as f32]
}

/// Applies one step of a drag from `a` to `b` (canvas pixels) to the scene.
fn step(d: &mut ObjectDrag, a: [f64; 2], b: [f64; 2]) -> bool {
    pose_ui::step(&mut d.scene, &d.kind, d.size, f32p(a), f32p(b))
}

/// The photo match of `layer`, while the Match photo tab shows it (its joints are draggable).
fn shown_photo_match(app: &PhotocraftApp, layer: LayerId) -> Option<&photocraft_engine::scene3d_cmds::PhotoMatch> {
    if app.ui.object_tool.tab != PanelTab::Photo || !app.ui.object_tool.fixing {
        return None;
    }
    app.session.photo_match.as_ref().filter(|m| m.layer == layer)
}

/// The photo-match joint within reach of canvas point `at`, nearest first.
fn photo_handle_at(app: &PhotocraftApp, layer: LayerId, at: [f64; 2]) -> Option<usize> {
    let m = shown_photo_match(app, layer)?;
    // About a fingertip on screen, whatever the zoom.
    let reach = 24.0 / app.current_zoom().max(0.01);
    pose_ui::photo_handle_at(&m.points(), f32p(at), reach)
}

/// The figure joint (Joints) within reach of canvas point `at`, nearest first.
fn joint_handle_at(app: &PhotocraftApp, scene: &Scene3d, size: (u32, u32), at: [f64; 2]) -> Option<(usize, usize)> {
    if !app.ui.object_tool.joints {
        return None;
    }
    let reach = 24.0 / app.current_zoom().max(0.01);
    pose_ui::joint_handle_at(scene, size, f32p(at), reach)
}

fn refit_drag(app: &mut PhotocraftApp, d: &ObjectDrag, point: usize, at: [f64; 2], draft: bool) {
    let fig = app.session.photo_match.as_ref().map_or(0, |m| m.figure);
    let p = json!({"layer": d.layer.0, "figure": fig, "move": {"point": point, "to": at}, "draft": draft, "coalesce": d.key});
    let r = app.run("scene3d.refitPhoto", p);
    status(app, r);
}

fn send(app: &mut PhotocraftApp, d: &ObjectDrag, draft: bool) {
    let p = json!({"layer": d.layer.0, "scene": d.scene, "draft": draft, "coalesce": d.key});
    let r = app.run("scene3d.set", p);
    status(app, r);
}

/// Pointer input for the Object tool. Returns true when the event was its own.
pub fn pointer(app: &mut PhotocraftApp, ev: ToolEvent, mods: egui::Modifiers) -> bool {
    if app.ui.tool != Tool::Object {
        return false;
    }
    match ev {
        ToolEvent::Down { x, y, .. } => {
            let Some((layer, scene, size)) = active_scene(app) else {
                app.ui.status = tl!("Select a 3D layer to pose (Layer › New › 3D Figure…)").to_string();
                return true;
            };
            // While a fall shows, the canvas waits for Keep or Cancel.
            if gravity::showing(app) {
                app.ui.status = tl!("Keep or cancel the fall in the 3D panel's Gravity tab first").to_string();
                return true;
            }
            if app.ui.object_tool.mode == DragMode::Pin && !(mods.shift || mods.command || mods.ctrl) {
                gravity::toggle_pin_at(app, layer, &scene, size, f32p([x, y]));
                return true;
            }
            // A photo-match joint under the pen is dragged onto the photo.
            if let Some(point) = photo_handle_at(app, layer, [x, y]) {
                app.ui.object_tool.drags += 1;
                let key = format!("object-tool-{}", app.ui.object_tool.drags);
                app.ui.object_tool.drag =
                    Some(ObjectDrag { layer, kind: DragKind::PhotoPoint(point), scene, size, last: [x, y], moved: false, key, hidden: false, preview: None });
                return true;
            }
            // A joint dot (Joints) is dragged to where the joint should be.
            if let Some((figure, joint)) = joint_handle_at(app, &scene, size, [x, y]) {
                app.ui.object_tool.selected = Some((layer.0, figure, joint));
                app.ui.object_tool.drags += 1;
                let key = format!("object-tool-{}", app.ui.object_tool.drags);
                app.ui.object_tool.drag = Some(ObjectDrag {
                    layer,
                    kind: DragKind::Reach(figure, joint),
                    scene,
                    size,
                    last: [x, y],
                    moved: false,
                    key,
                    hidden: false,
                    preview: None,
                });
                return true;
            }
            let ui = &app.ui.object_tool;
            let camera_kind = if mods.shift {
                Some(DragKind::Pan)
            } else if mods.command || mods.ctrl {
                Some(DragKind::Dolly)
            } else {
                match ui.mode {
                    DragMode::Pose | DragMode::Pin => None,
                    DragMode::Orbit => Some(DragKind::Orbit),
                    DragMode::Pan => Some(DragKind::Pan),
                    DragMode::Zoom => Some(DragKind::Dolly),
                    // The sun under the pen, else the light picked in the Light menu.
                    DragMode::Light => Some(pose_ui::light_drag_at(&scene, size, f32p([x, y]), 24.0 / app.current_zoom().max(0.01), ui.light_target)),
                }
            };
            let twist = ui.twist || mods.alt;
            let kind = match camera_kind {
                Some(k) => k,
                None => {
                    let (hit, kind) = pose_ui::pose_drag_at(&scene, size, f32p([x, y]), twist);
                    if let Some((figure, joint)) = hit {
                        app.ui.object_tool.selected = Some((layer.0, figure, joint));
                    }
                    kind
                }
            };
            app.ui.object_tool.drags += 1;
            let key = format!("object-tool-{}", app.ui.object_tool.drags);
            let mut d = ObjectDrag { layer, kind, scene, size, last: [x, y], moved: false, key, hidden: false, preview: None };
            // The light jumps to the pen straight away.
            if matches!(d.kind, DragKind::Light | DragKind::Light2) && step(&mut d, [x, y], [x, y]) {
                d.moved = true;
                show_preview(app, &mut d);
            }
            app.ui.object_tool.drag = Some(d);
        }
        ToolEvent::Move { x, y, .. } => {
            let Some(mut d) = app.ui.object_tool.drag.take() else { return true };
            // The dot follows the pen; refitting and re-rendering the figure waits for the release.
            if let DragKind::PhotoPoint(_) = d.kind {
                d.moved |= [x, y] != d.last;
                d.last = [x, y];
                app.ui.object_tool.drag = Some(d);
                return true;
            }
            let a = d.last;
            if step(&mut d, a, [x, y]) {
                d.moved = true;
                show_preview(app, &mut d);
            }
            d.last = [x, y];
            app.ui.object_tool.drag = Some(d);
        }
        ToolEvent::Up { x, y } => {
            let Some(mut d) = app.ui.object_tool.drag.take() else { return true };
            if let DragKind::PhotoPoint(point) = d.kind {
                // A tap without a move leaves the match as it is.
                if d.moved || [x, y] != d.last {
                    refit_drag(app, &d, point, [x, y], false);
                }
                return true;
            }
            let a = d.last;
            d.moved |= step(&mut d, a, [x, y]);
            if d.moved {
                send(app, &d, false);
            }
        }
    }
    true
}

/// The selected part (its bone and pivot) and dots on the main joints of the active 3D layer.
pub fn draw_overlay(app: &PhotocraftApp, painter: &egui::Painter, xf: &ViewXform) {
    let Some((layer, scene, (w, h))) = active_scene(app) else { return };
    // A fall plays over its layer whichever tool is out; the pins show with the Object tool.
    let shown = gravity::showing(app);
    if shown || app.ui.tool == Tool::Object {
        gravity::draw(app, painter, &|p: [f32; 2]| xf.to_screen(p[0], p[1]), layer, &scene, (w, h));
    }
    if app.ui.tool != Tool::Object || shown {
        return;
    }
    // A pose drag shows its own copy of the scene; a photo-joint drag refits through the engine.
    let scene = app.ui.object_tool.drag.as_ref().filter(|d| d.layer == layer && !matches!(d.kind, DragKind::PhotoPoint(_))).map_or(scene, |d| d.scene.clone());
    let t = Tokens::get(painter.ctx());
    let to_screen = |p: [f32; 2]| -> Pos2 { xf.to_screen(p[0], p[1]) };
    // A drag's quick preview of the figures, in place of the layer's image.
    if let Some(d) = app.ui.object_tool.drag.as_ref().filter(|d| d.layer == layer)
        && let Some(p) = d.preview.as_ref().filter(|p| !p.rgba.is_empty())
    {
        draw_preview(painter, &to_screen, p);
    }
    // The Match photo tab: the photo's joints as the match read them, to drag where they belong.
    if let Some(m) = shown_photo_match(app, layer) {
        let mut points = m.points();
        // The joint being dragged is drawn under the pen.
        if let Some(d) = app.ui.object_tool.drag.as_ref().filter(|d| d.layer == layer)
            && let DragKind::PhotoPoint(i) = d.kind
            && let Some(p) = points.get_mut(i)
        {
            *p = s3d::glam::Vec2::new(d.last[0] as f32, d.last[1] as f32);
        }
        let held = app.ui.object_tool.drag.as_ref().and_then(|d| if let DragKind::PhotoPoint(i) = d.kind { Some(i) } else { None });
        pose_ui::draw_photo_points(painter, &points, &|i| m.moved.contains_key(&i), held, &to_screen);
        return;
    }
    if app.ui.object_tool.mode == DragMode::Light {
        pose_ui::draw_light(painter, &scene, (w, h), &to_screen);
        return;
    }
    pose_ui::draw_joint_dots(painter, &scene, (w, h), &to_screen);
    // Joints: the main joints as coral dots on a thin skeleton, to drag into place.
    if app.ui.object_tool.joints {
        let held = app.ui.object_tool.drag.as_ref().and_then(|d| if let DragKind::Reach(f, j) = d.kind { Some((f, j)) } else { None });
        pose_ui::draw_reach_handles(painter, &scene, (w, h), &to_screen, held);
    }
    let Some((_, fi, j)) = app.ui.object_tool.selected.filter(|s| s.0 == layer.0) else { return };
    pose_ui::draw_selected(painter, &scene, (w, h), &to_screen, (fi, j), (t.accent, t.text));
}

/// Draws a drag preview where it belongs on the canvas (uploaded once per new preview).
fn draw_preview(painter: &egui::Painter, to_screen: &dyn Fn([f32; 2]) -> Pos2, p: &std::sync::Arc<s3d::Preview>) {
    let id = egui::Id::unique("object-tool-preview");
    let key = std::sync::Arc::as_ptr(p) as usize;
    let ctx = painter.ctx();
    let cached: Option<(usize, egui::TextureHandle)> = ctx.data(|d| d.get_temp(id));
    let tex = match cached {
        Some((k, tex)) if k == key => tex,
        _ => {
            let img = egui::ColorImage::from_rgba_unmultiplied(p.size, &p.rgba);
            let tex = ctx.load_texture("object-tool-preview", img, egui::TextureOptions::LINEAR);
            ctx.data_mut(|d| d.insert_temp(id, (key, tex.clone())));
            tex
        }
    };
    let r = p.rect;
    let rect = egui::Rect::from_two_pos(to_screen([r.x0 as f32, r.y0 as f32]), to_screen([r.x1 as f32, r.y1 as f32]));
    painter.image(tex.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
}

/// The options bar's Light menu: presets, direction, strength, colour, ambient, shadow.
fn light_menu(app: &mut PhotocraftApp, ui: &mut egui::Ui, layer: LayerId, scene: &Scene3d, cmd: &mut Option<(&'static str, Value)>) {
    let look = &scene.look;
    let set = |extra: Value| -> Option<(&'static str, Value)> {
        let mut p = json!({"layer": layer.0});
        if let (Some(o), Value::Object(e)) = (p.as_object_mut(), extra) {
            o.extend(e);
        }
        Some(("scene3d.look", p))
    };
    ui.set_min_width(240.0);
    let placing = app.ui.object_tool.mode == DragMode::Light;
    let target = app.ui.object_tool.light_target;
    if ui.selectable_label(placing && target == light::Which::Main, tl!("Place the light on the canvas")).clicked() {
        app.ui.object_tool.mode = DragMode::Light;
        app.ui.object_tool.light_target = light::Which::Main;
    }
    if look.light2 && ui.selectable_label(placing && target == light::Which::Second, tl!("Place the second light on the canvas")).clicked() {
        app.ui.object_tool.mode = DragMode::Light;
        app.ui.object_tool.light_target = light::Which::Second;
    }
    ui.separator();
    ui.label(tl!("Light from"));
    ui.horizontal_wrapped(|ui| {
        for (id, label, _) in light::PRESETS {
            if ui.button(tl!(label)).clicked() {
                *cmd = set(json!({"lightPreset": id}));
            }
        }
    });
    ui.separator();
    // Each slider drag (or colour edit) is one undo step.
    let key = format!("object-light-{}", app.ui.object_tool.light_edits);
    let mut bump = false;
    let mut slider = |ui: &mut egui::Ui, label: &str, value: f32, range: std::ops::RangeInclusive<f32>, suffix: &str| -> Option<f32> {
        let mut v = value;
        let r = ui.add(egui::Slider::new(&mut v, range).text(label).suffix(suffix));
        if r.drag_started() || r.gained_focus() {
            bump = true;
        }
        r.changed().then_some(v)
    };
    let mut change: Option<Value> = None;
    let yaw = look.light_yaw.to_degrees();
    if let Some(v) = slider(ui, tl!("Direction"), (yaw + 180.0).rem_euclid(360.0) - 180.0, -180.0..=180.0, "°") {
        change = Some(json!({"lightYaw": v}));
    }
    if let Some(v) = slider(ui, tl!("Height"), look.light_pitch.to_degrees(), -85.0..=85.0, "°") {
        change = Some(json!({"lightPitch": v}));
    }
    if let Some(v) = slider(ui, tl!("Strength"), look.light_strength, 0.0..=2.0, "") {
        change = Some(json!({"lightStrength": v}));
    }
    if let Some(v) = slider(ui, tl!("Ambient"), look.ambient, 0.0..=1.0, "") {
        change = Some(json!({"ambient": v}));
    }
    // Light colour: swatches and an inline picker (a colour popup would close the menu).
    ui.label(tl!("Light colour"));
    let current = pose_ui::hex(look.light_color);
    ui.horizontal_wrapped(|ui| {
        for (h, tip) in pose_ui::LIGHT_SWATCHES {
            let c = Color32::from_hex(h).unwrap_or(Color32::WHITE);
            let (rect, r) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
            ui.painter().rect_filled(rect, 4.0, c);
            let on = current == h;
            let edge = if on { Stroke::new(2.0, ui.visuals().selection.stroke.color) } else { Stroke::new(1.0, Color32::from_black_alpha(90)) };
            ui.painter().rect_stroke(rect, 4.0, edge, egui::StrokeKind::Inside);
            if r.on_hover_text(tl!(tip)).clicked() {
                *cmd = set(json!({"lightColor": h}));
            }
        }
        let open = app.ui.object_tool.light_picker;
        if ui.selectable_label(open, tl!("Custom…")).clicked() {
            app.ui.object_tool.light_picker = !open;
        }
    });
    if app.ui.object_tool.light_picker {
        let mut hsva = egui::ecolor::Hsva::from_srgb(look.light_color.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
        let pressed = ui.input(|i| i.pointer.any_pressed());
        let r = ui.scope(|ui| egui::color_picker::color_picker_hsva_2d(ui, &mut hsva, egui::color_picker::Alpha::Opaque));
        if pressed && r.response.contains_pointer() {
            bump = true;
        }
        if r.inner {
            change = Some(json!({"lightColor": pose_ui::hex(hsva.to_srgb().map(|v| f32::from(v) / 255.0))}));
        }
    }
    if let Some(mut c) = change {
        if let Some(o) = c.as_object_mut() {
            o.insert("coalesce".into(), Value::String(key));
        }
        *cmd = set(c);
    }
    if bump {
        app.ui.object_tool.light_edits += 1;
    }
    // The second light (shared with PIRA Pose): edited on a copy, sent as one command.
    ui.separator();
    let mut edited = scene.clone();
    let e = pose_ui::second_light_ui(ui, &mut edited, &|s| crate::i18n::t(s));
    if e.started {
        app.ui.object_tool.light_edits += 1;
    }
    if e.changed {
        let l = &edited.look;
        let mut p = json!({
            "light2": l.light2,
            "light2Yaw": l.light2_yaw.to_degrees(),
            "light2Pitch": l.light2_pitch.to_degrees(),
            "light2Strength": l.light2_strength,
            "light2Color": pose_ui::hex(l.light2_color),
        });
        if let Some(o) = p.as_object_mut() {
            o.insert("coalesce".into(), Value::String(format!("object-light-{}", app.ui.object_tool.light_edits)));
        }
        *cmd = set(p);
    }
    ui.separator();
    let fixed = look.light_fixed;
    if ui.selectable_label(!fixed, tl!("Light turns with the camera")).on_hover_text(tl!("Off: the light stays put in the scene while you orbit")).clicked() {
        *cmd = set(json!({"lightFixed": !fixed}));
    }
    let shadow = look.floor_shadow;
    if ui.selectable_label(shadow, tl!("Floor shadow")).clicked() {
        *cmd = set(json!({"floorShadow": !shadow}));
    }
}

fn selected_figure(app: &PhotocraftApp, layer: LayerId, scene: &Scene3d) -> usize {
    app.ui.object_tool.selected.filter(|s| s.0 == layer.0 && s.1 < scene.figures.len()).map_or(0, |s| s.1)
}

/// The Object tool's options bar.
pub fn options_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, tool: Tool) -> bool {
    if tool != Tool::Object {
        return false;
    }
    let modes = [
        (DragMode::Pose, "pointer", tl!("Pose: drag a part to turn it, empty canvas to orbit")),
        (DragMode::Orbit, "rotate-cw", tl!("Orbit the camera")),
        (DragMode::Pan, "hand", tl!("Pan the camera (⇧)")),
        (DragMode::Zoom, "zoom-in", tl!("Dolly the camera (⌘)")),
        (DragMode::Light, "sun", tl!("Light: drag to place the sun (inside the ring lights the front, outside lights from behind)")),
        (DragMode::Pin, "pin", tl!("Pin: tap parts to pin them for Gravity (3D panel)")),
    ];
    for (m, icon, tip) in modes {
        if crate::icons::button(ui, icon, 26.0, app.ui.object_tool.mode == m, tip).clicked() {
            app.ui.object_tool.mode = m;
        }
    }
    let twist = app.ui.object_tool.twist;
    if crate::icons::button(ui, "rotate-ccw", 26.0, twist, tl!("Twist: turn parts about their own bone (⌥)")).clicked() {
        app.ui.object_tool.twist = !twist;
    }
    let joints = app.ui.object_tool.joints;
    if crate::icons::button(ui, "spline", 26.0, joints, tl!("Joints: drag the head, elbows, wrists, knees and ankles to where they should be")).clicked() {
        app.ui.object_tool.joints = !joints;
    }
    crate::widgets::vline(ui, 22.0);
    let Some((layer, scene, _)) = active_scene(app) else {
        if ui.button(tl!("New 3D Figure")).clicked() {
            let r = app.run("layer.new.figure3d", json!({}));
            status(app, r);
        }
        ui.label(tl!("Select a 3D layer to pose it."));
        return true;
    };
    let fig = selected_figure(app, layer, &scene);
    let base = json!({"layer": layer.0, "figure": fig});
    let with = |extra: Value| {
        let mut p = base.clone();
        if let (Some(o), Value::Object(e)) = (p.as_object_mut(), extra) {
            o.extend(e);
        }
        p
    };
    let mut cmd: Option<(&str, Value)> = None;
    ui.menu_button(tl!("View"), |ui| {
        for (id, label) in s3d::CAMERA_VIEWS {
            if ui.button(tl!(label)).clicked() {
                cmd = Some(("scene3d.camera", json!({"layer": layer.0, "view": id})));
                ui.close();
            }
        }
        ui.separator();
        let parallel = scene.camera.parallel;
        if ui.selectable_label(parallel, tl!("Parallel projection")).clicked() {
            cmd = Some(("scene3d.camera", json!({"layer": layer.0, "parallel": !parallel})));
            ui.close();
        }
    });
    if crate::icons::button(ui, "maximize-2", 26.0, false, tl!("Fit the figures to the canvas")).clicked() {
        cmd = Some(("scene3d.camera", json!({"layer": layer.0, "frame": true})));
    }
    if crate::icons::button(ui, "align-edge-bottom", 26.0, false, tl!("Place on floor")).clicked() {
        cmd = Some(("scene3d.placeOnFloor", base.clone()));
    }
    ui.menu_button(tl!("Mirror"), |ui| {
        if ui.button(tl!("Left side onto right")).clicked() {
            cmd = Some(("scene3d.mirrorPose", with(json!({"from": "left"}))));
            ui.close();
        }
        if ui.button(tl!("Right side onto left")).clicked() {
            cmd = Some(("scene3d.mirrorPose", with(json!({"from": "right"}))));
            ui.close();
        }
        if ui.button(tl!("Flip pose")).clicked() {
            cmd = Some(("scene3d.flipPose", base.clone()));
            ui.close();
        }
    });
    ui.menu_button(tl!("Figure"), |ui| {
        let body = scene.figures.get(fig).map_or(Body3d::Male, |f| f.body);
        for b in [Body3d::Male, Body3d::Female] {
            if ui.selectable_label(body == b, tl!(b.label())).clicked() {
                cmd = Some(("scene3d.figure", with(json!({"body": b.id()}))));
                ui.close();
            }
        }
        ui.separator();
        if ui.button(tl!("Reset pose")).clicked() {
            cmd = Some(("scene3d.applyPose", with(json!({"pose": "Standing"}))));
            ui.close();
        }
        if ui.button(tl!("Add male figure")).clicked() {
            cmd = Some(("scene3d.addFigure", json!({"layer": layer.0, "body": "male"})));
            ui.close();
        }
        if ui.button(tl!("Add female figure")).clicked() {
            cmd = Some(("scene3d.addFigure", json!({"layer": layer.0, "body": "female"})));
            ui.close();
        }
        if scene.figures.len() > 1 && ui.button(tl!("Delete this figure")).clicked() {
            cmd = Some(("scene3d.removeFigure", base.clone()));
            ui.close();
        }
    });
    ui.menu_button(tl!("Look"), |ui| {
        for s in photocraft_doc::Shading3d::ALL {
            if ui.selectable_label(scene.look.shading == s, tl!(s.label())).clicked() {
                cmd = Some(("scene3d.look", json!({"layer": layer.0, "shading": s.id()})));
                ui.close();
            }
        }
        ui.separator();
        let shadow = scene.look.floor_shadow;
        if ui.selectable_label(shadow, tl!("Floor shadow")).clicked() {
            cmd = Some(("scene3d.look", json!({"layer": layer.0, "floorShadow": !shadow})));
            ui.close();
        }
        ui.separator();
        ui.label(tl!("Outline"));
        for (px, label) in [(0.0, "None"), (1.5, "Thin"), (3.0, "Medium"), (6.0, "Thick")] {
            if ui.selectable_label((scene.look.outline_px - px).abs() < 0.01, tl!(label)).clicked() {
                cmd = Some(("scene3d.look", json!({"layer": layer.0, "outline": px})));
                ui.close();
            }
        }
    });
    let light_btn = ui.button(tl!("Light")).on_hover_text(tl!("The 3D Light panel: drag its tab to move it, dock it or float it"));
    if light_btn.clicked() {
        toggle_light_panel(app);
    }
    if ui.button(tl!("Poses")).on_hover_text(tl!("Open the 3D panel: poses, hands, match to photo and new figures")).clicked() {
        let _ = app.run("window.panel.activate", json!({"panel": "materials3d"}));
    }
    if ui.button(tl!("Rasterize")).on_hover_text(tl!("Turn the 3D layer into pixels")).clicked() {
        cmd = Some(("layer.rasterize.scene3d", json!({"layer": layer.0})));
    }
    if let Some((id, p)) = cmd {
        let r = app.run(id, p);
        status(app, r);
    }
    true
}

/// Light opens the 3D Light panel, or closes it when it is showing, wherever it was last docked
/// or floated: by default an icon under the 3D panel's. A workspace saved before the panel
/// existed gets it there too, as a group of its own below the 3D panel.
fn toggle_light_panel(app: &mut PhotocraftApp) {
    let placed = app.ui.docking.as_ref().is_none_or(|w| w.layout.contains(&"light3d".to_string()) || w.hidden.iter().any(|(p, _)| p == "light3d"));
    let beside_3d = app.ui.docking.as_ref().is_some_and(|w| w.layout.contains(&"materials3d".to_string()));
    let r = app.run("window.toggle.light3d", json!({})).and_then(|v| {
        if placed || !beside_3d {
            return Ok(v);
        }
        app.run("window.panel.move", json!({"panel": "light3d", "anchor": "materials3d", "zone": "below"}))
    });
    status(app, r);
}

/// The 3D Light panel: the light settings of the active 3D layer. It scrolls, so a second
/// light's settings never run off the screen.
pub fn light_panel(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let Some((layer, scene, _)) = active_scene(app) else {
        ui.weak(tl!("Select a 3D layer to light it."));
        return;
    };
    let mut cmd: Option<(&str, Value)> = None;
    light_menu(app, ui, layer, &scene, &mut cmd);
    if let Some((id, p)) = cmd {
        let r = app.run(id, p);
        status(app, r);
    }
}

// ----------------------------------------------------------------------------- 3D panel

use pose_ui::THUMB;
/// Pose thumbnails rendered per frame (each takes a few milliseconds).
const THUMBS_PER_FRAME: usize = 3;

#[derive(Clone, Default)]
struct Thumbs(std::collections::HashMap<String, egui::TextureHandle>);

/// The 3D panel: tabs for the pose library, hand poses, matching to a photo and new figures.
/// One section at a time, so the pose grid gets the panel's whole height.
pub fn materials(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let target = active_scene(app).map(|(layer, scene, _)| (layer, selected_figure(app, layer, &scene)));
    let mut cmd: Option<(&str, Value)> = None;
    let mut photo_cmd: Option<Value> = None;
    let mut tabs = vec![(PanelTab::Poses, "Poses"), (PanelTab::Hands, "Hands")];
    if photocraft_engine::scene3d_cmds::PHOTO_TRACKING {
        tabs.push((PanelTab::Photo, "Match photo"));
    }
    tabs.push((PanelTab::Figures, "Figures"));
    tabs.push((PanelTab::Gravity, "Gravity"));
    if !tabs.iter().any(|(t, _)| *t == app.ui.object_tool.tab) {
        app.ui.object_tool.tab = PanelTab::Poses;
    }
    ui.horizontal_wrapped(|ui| {
        for (tab, label) in &tabs {
            if crate::widgets::pill_tab(ui, label, app.ui.object_tool.tab == *tab).clicked() {
                app.ui.object_tool.tab = *tab;
            }
        }
    });
    ui.add_space(6.0);
    let no_layer = |ui: &mut egui::Ui| {
        ui.weak(tl!("Select a 3D layer first, or make one in Figures."));
    };
    match app.ui.object_tool.tab {
        PanelTab::Poses => cmd = pose_grid(app, ui, target),
        PanelTab::Hands => match target {
            Some((layer, fig)) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(tl!("Pose"));
                    let hands = if app.ui.object_tool.hands.is_empty() { "both".to_string() } else { app.ui.object_tool.hands.clone() };
                    for (id, label) in [("both", "Both hands"), ("left", "Left"), ("right", "Right")] {
                        if crate::widgets::pill_tab(ui, label, hands == id).clicked() {
                            app.ui.object_tool.hands = id.to_string();
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for e in library::entries().iter().filter(|e| e.hands_only) {
                        if ui.button(tl!(e.name.as_str())).clicked() {
                            let sides = if app.ui.object_tool.hands.is_empty() { "both" } else { app.ui.object_tool.hands.as_str() };
                            cmd = Some(("scene3d.applyPose", json!({"layer": layer.0, "figure": fig, "pose": e.id, "sides": sides})));
                        }
                    }
                });
            }
            None => no_layer(ui),
        },
        PanelTab::Photo => match target {
            Some((layer, fig)) => {
                // Scrolls: the fixes below the buttons run past a short floating panel.
                egui::ScrollArea::vertical().id_salt("match-photo").auto_shrink([false, true]).show(ui, |ui| {
                    ui.label(tl!("Pose this figure like the person or hands in a photo."));
                    ui.weak(tl!("Put the photo on a layer under the 3D layer, then pick what to match."));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(tl!("Body model"));
                        let accurate = &mut app.ui.object_tool.accurate;
                        if crate::widgets::pill_tab(ui, tl!("Quick"), !*accurate).clicked() {
                            *accurate = false;
                        }
                        if crate::widgets::pill_tab(ui, tl!("Accurate"), *accurate).clicked() {
                            *accurate = true;
                        }
                    });
                    ui.weak(if app.ui.object_tool.accurate {
                        tl!("Accurate: a larger model, steadier on hard poses, about three times slower.")
                    } else {
                        tl!("Quick: fast. Try Accurate if the pose comes out wrong.")
                    });
                    ui.add_space(6.0);
                    let model = if app.ui.object_tool.accurate { "accurate" } else { "quick" };
                    ui.horizontal_wrapped(|ui| {
                        for (part, label) in [("body", "Body"), ("hands", "Both hands"), ("left", "Left hand"), ("right", "Right hand")] {
                            if ui.button(tl!(label)).clicked() {
                                photo_cmd = Some(json!({"layer": layer.0, "figure": fig, "part": part, "model": model}));
                                if part == "body" {
                                    app.ui.object_tool.fixing = true;
                                }
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.weak(tl!("The first match takes a few seconds longer while the models load."));
                    // Fixing the match: drag joints on the canvas, set each limb's depth here.
                    let shown = app.session.photo_match.clone().filter(|m| m.layer == layer && m.figure == fig);
                    if let Some(m) = shown.as_ref().filter(|_| !app.ui.object_tool.fixing) {
                        ui.add_space(10.0);
                        ui.separator();
                        let _ = m;
                        if ui.button(tl!("Fix the match")).clicked() {
                            app.ui.object_tool.fixing = true;
                        }
                    }
                    if let Some(m) = shown.filter(|_| app.ui.object_tool.fixing) {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(tl!("Fix the match")).strong());
                            // Done: the pose stays as it is and the joints leave the canvas.
                            if crate::widgets::primary_button(ui, tl!("Done"), 64.0).clicked() {
                                app.ui.object_tool.fixing = false;
                            }
                        });
                        ui.weak(tl!("With the Object tool, drag the coral dots on the photo onto the person's joints. White dots are ones you've moved."));
                        ui.weak(tl!("If a limb points the wrong way, say which end is nearer the camera:"));
                        ui.add_space(4.0);
                        for (k, (name, _, _)) in s3d::retarget::SEGMENTS.iter().enumerate() {
                            let now = m.now.get(k).copied().unwrap_or(true);
                            let set_by_user = m.toward.get(k).copied().flatten().is_some();
                            ui.horizontal(|ui| {
                                let label = tl!(name);
                                ui.add_sized(
                                    [110.0, 18.0],
                                    egui::Label::new(if set_by_user { egui::RichText::new(label).strong() } else { egui::RichText::new(label) }),
                                );
                                for (toward, text) in [(true, "Toward me"), (false, "Away")] {
                                    if crate::widgets::pill_tab(ui, tl!(text), now == toward).clicked() && now != toward {
                                        cmd = Some(("scene3d.refitPhoto", json!({"layer": layer.0, "figure": fig, "depth": {"segment": k, "toward": toward}})));
                                    }
                                }
                            });
                        }
                        ui.add_space(4.0);
                        let fixes = !m.moved.is_empty() || m.toward.iter().any(Option::is_some);
                        if ui.add_enabled(fixes, egui::Button::new(tl!("Undo all fixes"))).clicked() {
                            cmd = Some(("scene3d.refitPhoto", json!({"layer": layer.0, "figure": fig, "reset": true})));
                        }
                    }
                });
            }
            None => no_layer(ui),
        },
        PanelTab::Figures => {
            ui.label(tl!("New 3D layer"));
            ui.horizontal_wrapped(|ui| {
                if ui.button(tl!("New male")).clicked() {
                    cmd = Some(("layer.new.figure3d", json!({"body": "male"})));
                }
                if ui.button(tl!("New female")).clicked() {
                    cmd = Some(("layer.new.figure3d", json!({"body": "female"})));
                }
            });
            if let Some((layer, _)) = target {
                ui.add_space(6.0);
                ui.label(tl!("Add to this 3D layer"));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tl!("Add male")).clicked() {
                        cmd = Some(("scene3d.addFigure", json!({"layer": layer.0, "body": "male"})));
                    }
                    if ui.button(tl!("Add female")).clicked() {
                        cmd = Some(("scene3d.addFigure", json!({"layer": layer.0, "body": "female"})));
                    }
                });
            }
        }
        PanelTab::Gravity => gravity::panel(app, ui, target),
    }
    if let Some((id, p)) = cmd {
        let r = app.run(id, p);
        status(app, r);
    }
    // Tracking takes a moment (longer the first time, while the models load): a background job,
    // with its progress in the status bar.
    if let Some(p) = photo_cmd {
        let r = crate::jobs_ui::run(app, "scene3d.matchPhoto", p);
        status(app, r);
    }
}

/// The Poses tab: category, search and the thumbnail grid (filling the rest of the panel). A tap
/// gives the command to run: pose the selected figure, or make a new 3D layer in that pose.
fn pose_grid(app: &mut PhotocraftApp, ui: &mut egui::Ui, target: Option<(LayerId, usize)>) -> Option<(&'static str, Value)> {
    let t = Tokens::get(ui.ctx());
    let entries = library::entries();
    let mut cmd = None;
    let cats: Vec<String> = library::categories().into_iter().filter(|c| c != library::HANDS).collect();
    if !cats.contains(&app.ui.object_tool.category) {
        app.ui.object_tool.category = library::BASIC.to_string();
    }
    ui.horizontal(|ui| {
        craft_ui::ComboBox::from_id_salt("pose-category").selected_text(tl!(app.ui.object_tool.category.as_str())).show_ui(ui, |ui| {
            for c in &cats {
                let n = entries.iter().filter(|e| &e.category == c).count();
                ui.selectable_value(&mut app.ui.object_tool.category, c.clone(), format!("{} ({n})", tl!(c.as_str())));
            }
        });
        ui.add(egui::TextEdit::singleline(&mut app.ui.object_tool.search).hint_text(tl!("Search poses")).desired_width(f32::INFINITY));
    });
    ui.weak(tl!("Motion-capture poses from mocap.cs.cmu.edu (NSF EIA-0196217)."));
    ui.add_space(4.0);
    let search = app.ui.object_tool.search.trim().to_lowercase();
    let shown: Vec<&library::Entry> = entries
        .iter()
        .filter(|e| !e.hands_only)
        .filter(|e| if search.is_empty() { e.category == app.ui.object_tool.category } else { e.name.to_lowercase().contains(&search) })
        .collect();
    if shown.is_empty() {
        ui.weak(tl!("No poses match."));
        return None;
    }
    let cell = egui::vec2(THUMB.0 as f32 * 0.8 + 6.0, THUMB.1 as f32 * 0.8 + 6.0);
    let cols = ((ui.available_width() / cell.x).floor() as usize).max(1);
    let rows = shown.len().div_ceil(cols);
    let ctx = ui.ctx().clone();
    let thumbs_id = egui::Id::unique("scene3d-pose-thumbs");
    let mut thumbs: Thumbs = ctx.data_mut(|d| d.get_temp::<Thumbs>(thumbs_id)).unwrap_or_default();
    let mut budget = THUMBS_PER_FRAME;
    egui::ScrollArea::vertical().id_salt("pose-grid").auto_shrink([false, false]).show_rows(ui, cell.y, rows, |ui, range| {
        for row in range {
            ui.horizontal(|ui| {
                for e in shown.iter().skip(row * cols).take(cols) {
                    let (rect, resp) = ui.allocate_exact_size(cell - egui::vec2(4.0, 4.0), egui::Sense::click());
                    let hovered = resp.hovered();
                    ui.painter().rect_filled(rect, t.radius_sm, if hovered { t.hover } else { t.field });
                    if !thumbs.0.contains_key(&e.id) && budget > 0 {
                        budget -= 1;
                        let tex = ctx.load_texture(format!("pose-{}", e.id), pose_ui::thumb_image(&e.id), egui::TextureOptions::LINEAR);
                        thumbs.0.insert(e.id.clone(), tex);
                    }
                    if let Some(tex) = thumbs.0.get(&e.id) {
                        ui.painter().image(tex.id(), rect.shrink(2.0), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                    } else {
                        ctx.request_repaint();
                    }
                    let resp = resp.on_hover_text(e.name.as_str());
                    if resp.clicked() {
                        cmd = Some(match target {
                            Some((layer, fig)) => ("scene3d.applyPose", json!({"layer": layer.0, "figure": fig, "pose": e.id})),
                            None => ("layer.new.figure3d", json!({"pose": e.id})),
                        });
                    }
                }
            });
        }
    });
    if budget == 0 {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(thumbs_id, thumbs));
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_scene3d::pose;

    fn app_with_figure() -> PhotocraftApp {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({"width": 600, "height": 800})).unwrap();
        app.run("layer.new.figure3d", json!({"pose": "T-pose"})).unwrap();
        app.ui.tool = Tool::Object;
        app
    }

    fn scene(app: &PhotocraftApp) -> Scene3d {
        active_scene(app).unwrap().1
    }

    #[test]
    fn light_opens_and_closes_its_own_panel_under_the_3d_one() {
        let mut app = app_with_figure();
        crate::panel_docking::use_pira_workspace(&mut app);
        let showing = |app: &PhotocraftApp| app.ui.docking.as_ref().unwrap().layout.contains(&"light3d".to_owned());
        let own_icon = |app: &PhotocraftApp| crate::panel_docking::group_members(&app.ui.docking.as_ref().unwrap().layout, "light3d") == ["light3d"];
        assert!(showing(&app) && own_icon(&app), "an icon in PIRA's layout");
        toggle_light_panel(&mut app);
        assert!(!showing(&app), "a click on Light closes it");
        toggle_light_panel(&mut app);
        assert!(showing(&app) && own_icon(&app), "and the next one opens it again: {}", app.ui.status);
        // A workspace saved before the panel existed gets it under the 3D panel, not as a Brushes tab.
        app.run("window.panel.close", json!({"panel": "light3d"})).unwrap();
        app.ui.docking.as_mut().unwrap().hidden.retain(|(p, _)| p != "light3d");
        toggle_light_panel(&mut app);
        assert!(showing(&app) && own_icon(&app), "{:?}", app.ui.docking.as_ref().unwrap().layout);
        // It floats and docks like any panel, and comes back where it was left.
        toggle_light_panel(&mut app);
        app.run("window.panel.float", json!({"panel": "light3d", "x": 600, "y": 200})).unwrap();
        toggle_light_panel(&mut app);
        toggle_light_panel(&mut app);
        assert!(app.ui.docking.as_ref().unwrap().layout.floating.iter().any(|w| w.contains(&"light3d".to_owned())));
        // The panel draws with and without a 3D layer.
        let ctx = egui::Context::default();
        PhotocraftApp::setup_context(&ctx, Default::default());
        for _ in 0..2 {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| light_panel(&mut app, ui));
            })
            .drop_without_applying_deltas();
            app.run("layer.new.layer", json!({})).unwrap();
        }
    }

    #[test]
    fn dragging_an_arm_poses_it_in_one_undo_step() {
        let mut app = app_with_figure();
        let before = scene(&app);
        let arm = pose::joint_index(&before.figures[0], "forearm.L").unwrap();
        let (pivot, tip) = pose::joint_px(&before, 600, 800, 0, arm).unwrap();
        let mid = [f64::from(pivot[0] + tip[0]) / 2.0, f64::from(pivot[1] + tip[1]) / 2.0];
        let mods = egui::Modifiers::NONE;
        assert!(pointer(&mut app, ToolEvent::Down { x: mid[0], y: mid[1], pressure: 1.0 }, mods));
        assert_eq!(app.ui.object_tool.selected.map(|s| s.2), Some(arm));
        for k in 1..=5 {
            let k = f64::from(k);
            assert!(pointer(&mut app, ToolEvent::Move { x: mid[0], y: mid[1] + 10.0 * k, pressure: 1.0 }, mods));
        }
        assert!(pointer(&mut app, ToolEvent::Up { x: mid[0], y: mid[1] + 60.0 }, mods));
        let after = scene(&app);
        assert_ne!(after.figures[0].joints.get("forearm.L"), before.figures[0].joints.get("forearm.L"));
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(scene(&app).figures[0].joints, before.figures[0].joints);
    }

    #[test]
    fn dragging_a_photo_joint_refits_the_match_in_one_undo_step() {
        let mut app = app_with_figure();
        app.run("layer.rasterize.scene3d", json!({})).unwrap();
        app.run("layer.new.figure3d", json!({"pose": "Standing"})).unwrap();
        app.session.execute("scene3d.matchPhoto", json!({})).unwrap();
        let layer = app.session.active().unwrap().active_layer.unwrap();
        // Hidden unless the Match photo tab is open.
        app.ui.object_tool.tab = PanelTab::Poses;
        let elbow = app.session.photo_match.as_ref().unwrap().points()[13];
        assert!(photo_handle_at(&app, layer, [elbow.x as f64, elbow.y as f64]).is_none());
        app.ui.object_tool.tab = PanelTab::Photo;
        assert!(photo_handle_at(&app, layer, [elbow.x as f64, elbow.y as f64]).is_none(), "not fixing");
        app.ui.object_tool.fixing = true;
        assert_eq!(photo_handle_at(&app, layer, [elbow.x as f64 + 3.0, elbow.y as f64]), Some(13));
        let before = scene(&app);
        let (x, y) = (elbow.x as f64, elbow.y as f64);
        pointer(&mut app, ToolEvent::Down { x, y, pressure: 1.0 }, egui::Modifiers::NONE);
        pointer(&mut app, ToolEvent::Move { x: x + 10.0, y: y + 60.0, pressure: 1.0 }, egui::Modifiers::NONE);
        assert_eq!(scene(&app), before, "moves only move the dot");
        pointer(&mut app, ToolEvent::Up { x: x + 20.0, y: y + 120.0 }, egui::Modifiers::NONE);
        let m = app.session.photo_match.as_ref().unwrap();
        assert!((m.points()[13].y - (elbow.y + 120.0)).abs() < 0.5, "the dot is where it was dropped");
        assert_ne!(scene(&app), before, "the figure was refitted");
        app.session.execute("edit.undo", json!({})).unwrap();
        assert_eq!(scene(&app).figures, before.figures, "one undo step");
        // Done hides the joints.
        app.ui.object_tool.fixing = false;
        assert!(photo_handle_at(&app, layer, [elbow.x as f64, elbow.y as f64]).is_none());
    }

    #[test]
    fn with_joints_on_dragging_a_wrist_puts_it_under_the_pen() {
        let mut app = app_with_figure();
        let (_, sc, (w, h)) = active_scene(&app).unwrap();
        let hand = pose::joint_index(&sc.figures[0], "hand.L").unwrap();
        let (wrist, _) = pose::joint_px(&sc, w, h, 0, hand).unwrap();
        let (x, y) = (wrist[0] as f64, wrist[1] as f64);
        assert!(joint_handle_at(&app, &sc, (w, h), [x, y]).is_none(), "off until Joints is on");
        app.ui.object_tool.joints = true;
        assert_eq!(joint_handle_at(&app, &sc, (w, h), [x + 4.0, y]), Some((0, hand)));
        pointer(&mut app, ToolEvent::Down { x, y, pressure: 1.0 }, egui::Modifiers::NONE);
        pointer(&mut app, ToolEvent::Move { x: x - 30.0, y: y + 40.0, pressure: 1.0 }, egui::Modifiers::NONE);
        pointer(&mut app, ToolEvent::Up { x: x - 60.0, y: y + 80.0 }, egui::Modifiers::NONE);
        let sc2 = scene(&app);
        let (now, _) = pose::joint_px(&sc2, w, h, 0, hand).unwrap();
        assert!((now[0] - (wrist[0] - 60.0)).abs() < 2.0 && (now[1] - (wrist[1] + 80.0)).abs() < 2.0, "{now:?}");
        app.session.execute("edit.undo", json!({})).unwrap();
        assert_eq!(scene(&app).figures, sc.figures, "one undo step");
    }

    #[test]
    fn dragging_empty_canvas_orbits_and_shift_pans() {
        let mut app = app_with_figure();
        let before = scene(&app).camera;
        let mods = egui::Modifiers::NONE;
        pointer(&mut app, ToolEvent::Down { x: 5.0, y: 5.0, pressure: 1.0 }, mods);
        pointer(&mut app, ToolEvent::Move { x: 60.0, y: 5.0, pressure: 1.0 }, mods);
        pointer(&mut app, ToolEvent::Up { x: 60.0, y: 5.0 }, mods);
        let orbited = scene(&app).camera;
        assert!((orbited.yaw - before.yaw).abs() > 0.1);
        let shift = egui::Modifiers { shift: true, ..Default::default() };
        pointer(&mut app, ToolEvent::Down { x: 5.0, y: 5.0, pressure: 1.0 }, shift);
        pointer(&mut app, ToolEvent::Up { x: 80.0, y: 5.0 }, shift);
        assert_ne!(scene(&app).camera.target, orbited.target);
    }

    #[test]
    fn other_tools_and_other_layers_are_left_alone() {
        let mut app = app_with_figure();
        app.ui.tool = Tool::Brush;
        assert!(!pointer(&mut app, ToolEvent::Down { x: 5.0, y: 5.0, pressure: 1.0 }, egui::Modifiers::NONE));
        app.ui.tool = Tool::Object;
        app.run("layer.new.layer", json!({})).unwrap();
        assert!(pointer(&mut app, ToolEvent::Down { x: 5.0, y: 5.0, pressure: 1.0 }, egui::Modifiers::NONE));
        assert!(app.ui.object_tool.drag.is_none());
    }

    fn cache_tiles(app: &PhotocraftApp) -> usize {
        let st = app.session.active().unwrap();
        match &st.doc.layer(st.active_layer.unwrap()).unwrap().content {
            LayerContent::Scene3d(sc) => sc.cache.as_ref().map_or(0, |c| c.tile_count()),
            _ => 0,
        }
    }

    /// Plays a fall to its end, as the canvas would frame by frame.
    fn play_out(app: &mut PhotocraftApp) {
        let ctx = egui::Context::default();
        for _ in 0..2000 {
            if !app.ui.object_tool.gravity.fall.as_ref().is_some_and(|p| p.playing) {
                break;
            }
            app.ui.object_tool.gravity.fall.as_mut().unwrap().time += 0.1;
            tick_gravity(app, &ctx);
        }
    }

    #[test]
    fn a_pinned_part_drops_and_keeps_in_one_undo_step() {
        let mut app = app_with_figure();
        let before = scene(&app);
        let layer = active_scene(&app).unwrap().0;
        // Pin mode: a tap on the left thigh pins it.
        app.ui.object_tool.mode = DragMode::Pin;
        let thigh = pose::joint_index(&before.figures[0], "thigh.L").unwrap();
        let (a, b) = pose::joint_px(&before, 600, 800, 0, thigh).unwrap();
        let (x, y) = (f64::from(a[0] + b[0]) / 2.0, f64::from(a[1] + b[1]) / 2.0);
        let mods = egui::Modifiers::NONE;
        pointer(&mut app, ToolEvent::Down { x, y, pressure: 1.0 }, mods);
        pointer(&mut app, ToolEvent::Up { x, y }, mods);
        assert_eq!(app.ui.object_tool.gravity.pins, vec![(layer.0, 0, "thigh.L".to_string())]);
        assert_eq!(scene(&app), before, "pinning changes nothing in the scene");
        drop_figure(&mut app, layer, 0);
        assert!(app.ui.object_tool.gravity.fall.is_some());
        assert_eq!(cache_tiles(&app), 0, "the layer's image is hidden while the fall plays over it");
        // The canvas waits while it plays.
        pointer(&mut app, ToolEvent::Down { x, y, pressure: 1.0 }, mods);
        pointer(&mut app, ToolEvent::Up { x, y }, mods);
        assert_eq!(app.ui.object_tool.gravity.pins.len(), 1);
        play_out(&mut app);
        keep_fall(&mut app);
        let after = scene(&app);
        assert_ne!(after.figures, before.figures, "it fell");
        assert!(cache_tiles(&app) > 0, "kept: rendered again");
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(scene(&app).figures, before.figures, "one undo step");
        assert!(cache_tiles(&app) > 0);
    }

    #[test]
    fn cancel_or_undo_puts_the_figure_back() {
        let mut app = app_with_figure();
        let before = scene(&app);
        let layer = active_scene(&app).unwrap().0;
        drop_figure(&mut app, layer, 0);
        play_out(&mut app);
        cancel_fall(&mut app);
        assert!(app.ui.object_tool.gravity.fall.is_none());
        assert_eq!(scene(&app), before);
        assert!(cache_tiles(&app) > 0);
        // An undo while a fall shows ends it, the figure as it was.
        drop_figure(&mut app, layer, 0);
        app.run("edit.undo", json!({})).unwrap();
        tick_gravity(&mut app, &egui::Context::default());
        assert!(app.ui.object_tool.gravity.fall.is_none());
        assert_eq!(scene(&app), before);
        assert!(cache_tiles(&app) > 0);
    }

    #[test]
    fn light_mode_puts_the_sun_under_the_pen_in_one_undo_step() {
        let mut app = app_with_figure();
        app.ui.object_tool.mode = DragMode::Light;
        let before = scene(&app).look;
        let (c, r) = light::disc(&scene(&app), 600, 800);
        let mods = egui::Modifiers::NONE;
        let at = |k: f32| (f64::from(c.x - r * 0.3 * k), f64::from(c.y - r * 0.2 * k));
        let (x, y) = at(1.0);
        pointer(&mut app, ToolEvent::Down { x, y, pressure: 1.0 }, mods);
        for k in 2..=4 {
            let (x, y) = at(k as f32);
            pointer(&mut app, ToolEvent::Move { x, y, pressure: 1.0 }, mods);
        }
        let end = at(4.0);
        pointer(&mut app, ToolEvent::Up { x: end.0, y: end.1 }, mods);
        let s = scene(&app);
        let (sun, _) = light::handle(&s, 600, 800);
        assert!((f64::from(sun[0]) - end.0).abs() < 2.0 && (f64::from(sun[1]) - end.1).abs() < 2.0, "{sun:?} vs {end:?}");
        assert_eq!(s.figures, scene(&app).figures);
        app.run("edit.undo", json!({})).unwrap();
        let undone = scene(&app).look;
        assert_eq!((undone.light_yaw, undone.light_pitch), (before.light_yaw, before.light_pitch));
    }

    #[test]
    fn a_thumbnail_shows_the_figure() {
        let img = pose_ui::thumb_image("Walking");
        assert!(img.pixels.iter().filter(|p| p.a() > 200).count() > 500);
    }

    #[test]
    fn every_panel_tab_draws_with_and_without_a_3d_layer() {
        let ctx = egui::Context::default();
        PhotocraftApp::setup_context(&ctx, Default::default());
        for with_figure in [true, false] {
            let mut app = app_with_figure();
            if !with_figure {
                app.run("layer.new.layer", json!({})).unwrap();
            }
            for tab in [PanelTab::Poses, PanelTab::Hands, PanelTab::Photo, PanelTab::Figures, PanelTab::Gravity] {
                app.ui.object_tool.tab = tab;
                ctx.run_ui(egui::RawInput::default(), |ui| {
                    egui::CentralPanel::default().show(ui, |ui| materials(&mut app, ui));
                })
                .drop_without_applying_deltas();
                assert_eq!(app.ui.object_tool.tab, tab, "{tab:?} stays selected");
            }
        }
        assert_eq!(ObjectToolUi::default().tab, PanelTab::Poses, "the pose grid is the first thing you see");
    }
}
