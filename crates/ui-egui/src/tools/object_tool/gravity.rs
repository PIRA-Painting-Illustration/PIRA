//! The 3D panel's Gravity tab: let a figure of the active 3D layer fall under its own weight,
//! slump, or hang from pinned parts (`photocraft_scene3d::gravity`, shared with PIRA Pose), play
//! the fall back on the canvas, and keep the moment you want.
//!
//! Drop hides the layer's image (`scene3d.set` with `hide`) and the fall plays as a quick preview
//! over the canvas, like an Object tool drag. Keep renders the frame shown into the same undo
//! step; Cancel puts the scene back and undoes that step. An undo while a fall shows ends it.

use super::{
    Color32, DragMode, LayerContent, LayerId, PREVIEW_PIXELS, PhotocraftApp, Pos2, Scene3d, Stroke, Tool, active_scene, draw_preview, json, s3d, status,
};
use photocraft_scene3d::gravity::{self, Fall, Push, Settings};
use photocraft_scene3d::pose;

/// The Gravity tab's settings, the pins and the fall being shown.
#[derive(Clone, Debug, PartialEq)]
pub struct GravityUi {
    /// Muscle tone, 0–1.
    pub tone: f32,
    pub push: Push,
    /// Pinned parts: layer, figure and joint name.
    pub pins: Vec<(u64, usize, String)>,
    pub fall: Option<Playback>,
    falls: u64,
}

impl Default for GravityUi {
    fn default() -> Self {
        GravityUi { tone: 0.0, push: Push::Forward, pins: Vec::new(), fall: None, falls: 0 }
    }
}

/// A simulated fall, shown over its layer until it is kept or cancelled.
#[derive(Clone, Debug, PartialEq)]
pub struct Playback {
    pub layer: LayerId,
    pub figure: usize,
    pub fall: Fall,
    /// The scene before the fall (what Cancel goes back to).
    pub before: Scene3d,
    /// The scene as the frame shown poses it.
    pub scene: Scene3d,
    size: (u32, u32),
    /// Seconds into the fall.
    pub time: f32,
    pub playing: bool,
    key: String,
    frame: usize,
    preview: Option<std::sync::Arc<s3d::Preview>>,
}

impl Playback {
    fn frame_at(&self) -> usize {
        (self.time * gravity::FPS).round().max(0.0) as usize
    }

    /// Poses the scene as the current moment and renders its preview when it changed.
    fn show(&mut self, force: bool) {
        let i = self.frame_at().min(self.fall.frames.len().saturating_sub(1));
        if i == self.frame && !force && self.preview.is_some() {
            return;
        }
        self.frame = i;
        if let Some(fig) = self.scene.figures.get_mut(self.figure) {
            self.fall.apply(i, fig);
        }
        let (w, h) = self.size;
        self.preview = Some(std::sync::Arc::new(s3d::preview(&self.scene, w, h, PREVIEW_PIXELS)));
    }
}

fn pins_of(app: &PhotocraftApp, layer: LayerId, figure: usize) -> Vec<String> {
    app.ui.object_tool.gravity.pins.iter().filter(|(l, f, _)| *l == layer.0 && *f == figure).map(|(_, _, j)| j.clone()).collect()
}

/// The fall shown, while its layer still has its image hidden for it (an undo ends it).
pub(super) fn showing(app: &PhotocraftApp) -> bool {
    app.ui.object_tool.gravity.fall.is_some()
}

/// The Gravity tab.
pub(super) fn panel(app: &mut PhotocraftApp, ui: &mut egui::Ui, target: Option<(LayerId, usize)>) {
    let Some((layer, fig)) = target else {
        ui.weak(tl!("Select a 3D layer first, or make one in Figures."));
        return;
    };
    // Scrolls, and Drop (or the playback) comes first: a short docked panel still shows it.
    egui::ScrollArea::vertical().id_salt("gravity").auto_shrink([false, true]).show(ui, |ui| {
        let shown = showing(app);
        if shown {
            playback_ui(app, ui);
        } else {
            ui.horizontal(|ui| {
                if crate::widgets::primary_button(ui, tl!("Drop"), 120.0).clicked() {
                    drop_figure(app, layer, fig);
                }
                ui.weak(tl!("Let the figure fall under its own weight."));
            });
        }
        ui.add_space(8.0);
        ui.add_enabled_ui(!shown, |ui| settings(app, ui, layer, fig));
        ui.add_space(6.0);
        ui.weak(tl!("Pin a hand to hang it from that hand, or the feet to keep them planted."));
    });
}

fn settings(app: &mut PhotocraftApp, ui: &mut egui::Ui, layer: LayerId, fig: usize) {
    ui.horizontal_wrapped(|ui| {
        ui.label(tl!("Push"));
        for p in Push::ALL {
            if crate::widgets::pill_tab(ui, tl!(p.label()), app.ui.object_tool.gravity.push == p).clicked() {
                app.ui.object_tool.gravity.push = p;
            }
        }
    });
    ui.add_space(4.0);
    let g = &mut app.ui.object_tool.gravity;
    ui.add(egui::Slider::new(&mut g.tone, 0.0..=1.0).text(tl!("Muscle tone")).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
    ui.weak(match g.tone {
        t if t < 0.15 => tl!("Limp: drops like a doll."),
        t if t < 0.5 => tl!("Slumps: the knees buckle and it sags."),
        t if t < 0.85 => tl!("Firm: it sinks down, holding much of the pose."),
        _ => tl!("Stiff: it topples, keeping the pose."),
    });
    ui.add_space(6.0);
    let pins = pins_of(app, layer, fig);
    ui.horizontal(|ui| {
        let pinning = app.ui.object_tool.mode == DragMode::Pin;
        let tip = tl!("With the Object tool, tap a part on the canvas to pin it where it is; tap it again to unpin");
        if crate::widgets::pill_tab(ui, tl!("Pin parts"), pinning).on_hover_text(tip).clicked() {
            app.ui.object_tool.mode = if pinning { DragMode::Pose } else { DragMode::Pin };
            if !pinning {
                app.ui.tool = Tool::Object;
            }
        }
        if ui.add_enabled(!pins.is_empty(), egui::Button::new(tl!("Clear pins"))).clicked() {
            app.ui.object_tool.gravity.pins.retain(|(l, f, _)| !(*l == layer.0 && *f == fig));
        }
    });
    if pins.is_empty() {
        ui.weak(tl!("No pins: it falls freely."));
    } else {
        let names: Vec<String> = pins.iter().map(|j| tl!(pose::joint_label(j).as_str()).to_string()).collect();
        ui.weak(format!("{} {}", tl!("Pinned:"), names.join(", ")));
    }
}

fn playback_ui(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let Some(p) = app.ui.object_tool.gravity.fall.as_mut() else { return };
    let total = p.fall.seconds().max(1.0 / gravity::FPS);
    ui.horizontal(|ui| {
        let label = if p.playing { tl!("Pause") } else { tl!("Play") };
        if ui.button(label).clicked() {
            if !p.playing && p.time >= total {
                p.time = 0.0;
            }
            p.playing = !p.playing;
        }
        let mut t = p.time;
        if ui.add(egui::Slider::new(&mut t, 0.0..=total).show_value(false)).changed() {
            p.time = t;
            p.playing = false;
            p.show(false);
        }
        ui.monospace(format!("{:.1} s", p.time.min(total)));
    });
    ui.weak(if p.fall.settled { tl!("Scrub to any moment of the fall, then keep it.") } else { tl!("It was still moving when the time ran out.") });
    ui.add_space(6.0);
    let mut keep = false;
    let mut cancel = false;
    let mut again = false;
    ui.horizontal(|ui| {
        keep = crate::widgets::primary_button(ui, tl!("Keep"), 64.0).on_hover_text(tl!("Keep the pose shown (one undo step)")).clicked();
        cancel = ui.button(tl!("Cancel")).clicked();
        again = ui.button(tl!("Drop again")).on_hover_text(tl!("Back to the start and fall with the settings as they are now")).clicked();
    });
    if keep {
        keep_fall(app);
    } else if cancel {
        cancel_fall(app);
    } else if again {
        let target = app.ui.object_tool.gravity.fall.as_ref().map(|p| (p.layer, p.figure));
        cancel_fall(app);
        if let Some((layer, fig)) = target {
            drop_figure(app, layer, fig);
        }
    }
}

/// Simulates the figure's fall from its pose now, hides the layer's image and starts playing.
pub fn drop_figure(app: &mut PhotocraftApp, layer: LayerId, figure: usize) {
    let Some((active, scene, size)) = active_scene(app) else { return };
    if active != layer {
        return;
    }
    let Some(fig) = scene.figures.get(figure) else { return };
    let g = &app.ui.object_tool.gravity;
    let settings = Settings { tone: g.tone, push: g.push, pins: pins_of(app, layer, figure), ..Settings::default() };
    let fall = gravity::simulate(fig, &settings);
    app.ui.object_tool.gravity.falls += 1;
    let key = format!("gravity-{}", app.ui.object_tool.gravity.falls);
    let r = app.run("scene3d.set", json!({"layer": layer.0, "scene": scene, "hide": true, "coalesce": key}));
    if r.is_err() {
        status(app, r);
        return;
    }
    if app.ui.object_tool.mode == DragMode::Pin {
        app.ui.object_tool.mode = DragMode::Pose;
    }
    app.ui.object_tool.drag = None;
    let mut p = Playback { layer, figure, fall, before: scene.clone(), scene, size, time: 0.0, playing: true, key, frame: 0, preview: None };
    p.show(true);
    app.ui.object_tool.gravity.fall = Some(p);
}

/// Advances a playing fall (each frame), and lets go of one an undo took back.
pub fn tick(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(layer) = app.ui.object_tool.gravity.fall.as_ref().map(|p| p.layer) else { return };
    let hidden = app.session.active().and_then(|st| st.doc.layer(layer)).is_some_and(|l| match &l.content {
        LayerContent::Scene3d(sc) => sc.cache.as_ref().is_some_and(|c| c.tile_count() == 0),
        _ => false,
    });
    if !hidden {
        app.ui.object_tool.gravity.fall = None;
        return;
    }
    // Another layer picked: the fall would play out of sight, so it is put back.
    if app.session.active().and_then(|st| st.active_layer) != Some(layer) {
        cancel_fall(app);
        return;
    }
    let Some(p) = app.ui.object_tool.gravity.fall.as_mut() else { return };
    if !p.playing {
        return;
    }
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
    p.time += dt;
    if p.time >= p.fall.seconds() {
        p.time = p.fall.seconds();
        p.playing = false;
    }
    p.show(false);
    ctx.request_repaint();
}

/// Keep: the pose shown, rendered into the fall's undo step.
pub fn keep_fall(app: &mut PhotocraftApp) {
    let Some(p) = app.ui.object_tool.gravity.fall.take() else { return };
    let r = app.run("scene3d.set", json!({"layer": p.layer.0, "scene": p.scene, "coalesce": p.key}));
    status(app, r);
}

/// Cancel: the scene as it was, and the fall's undo step taken back.
pub fn cancel_fall(app: &mut PhotocraftApp) {
    let Some(p) = app.ui.object_tool.gravity.fall.take() else { return };
    let r = app.run("scene3d.set", json!({"layer": p.layer.0, "scene": p.before, "coalesce": p.key})).and_then(|_| app.run("edit.undo", json!({})));
    status(app, r);
}

/// A tap in Pin mode: pins the part under the pen, or unpins it.
pub(super) fn toggle_pin_at(app: &mut PhotocraftApp, layer: LayerId, scene: &Scene3d, size: (u32, u32), at: [f32; 2]) {
    let (w, h) = size;
    let Some(hit) = pose::pick(scene, w, h, at) else {
        app.ui.status = tl!("Tap a part of the figure to pin it").to_string();
        return;
    };
    let Some(name) = scene.figures.get(hit.figure).and_then(|f| pose::joint_name(f, hit.joint)) else { return };
    let key = (layer.0, hit.figure, name.to_string());
    let pins = &mut app.ui.object_tool.gravity.pins;
    if let Some(k) = pins.iter().position(|p| *p == key) {
        pins.remove(k);
        app.ui.status = format!("{} {}", tl!("Unpinned:"), tl!(pose::joint_label(name).as_str()));
    } else {
        pins.push(key);
        app.ui.status = format!("{} {}", tl!("Pinned:"), tl!(pose::joint_label(name).as_str()));
    }
    app.ui.status_error = false;
    app.ui.object_tool.selected = Some((layer.0, hit.figure, hit.joint));
}

/// The fall's preview in place of the layer's image, and the pins where they hold their parts.
pub(super) fn draw(app: &PhotocraftApp, painter: &egui::Painter, to_screen: &dyn Fn([f32; 2]) -> Pos2, layer: LayerId, scene: &Scene3d, size: (u32, u32)) {
    let fall = app.ui.object_tool.gravity.fall.as_ref().filter(|p| p.layer == layer);
    if let Some(prev) = fall.and_then(|p| p.preview.as_ref()).filter(|p| !p.rgba.is_empty()) {
        draw_preview(painter, to_screen, prev);
    }
    // While a fall shows, its pins are where they were when it started.
    let scene = fall.map_or(scene, |p| &p.before);
    let (w, h) = size;
    for (l, figure, name) in &app.ui.object_tool.gravity.pins {
        if *l != layer.0 {
            continue;
        }
        let Some(j) = scene.figures.get(*figure).and_then(|f| pose::joint_index(f, name)) else { continue };
        let Some((pivot, tip)) = pose::joint_px(scene, w, h, *figure, j) else { continue };
        let at = to_screen([(pivot[0] + tip[0]) / 2.0, (pivot[1] + tip[1]) / 2.0]);
        let head = at + egui::vec2(0.0, -16.0);
        painter.line_segment([at, head], Stroke::new(2.5, Color32::from_gray(235)));
        painter.circle(head, 7.0, Color32::from_rgb(232, 70, 70), Stroke::new(1.5, Color32::from_gray(20)));
        painter.circle_filled(at, 2.5, Color32::from_gray(20));
    }
}
