//! The Gravity panel: let the selected figure fall, slump or hang from pinned parts, play the
//! fall back, and keep the moment you want (one undo step).

use super::{App, DragMode};
use eframe::egui::{self, Color32, Pos2, Stroke};
use photocraft_doc::Scene3d;
use photocraft_scene3d::gravity::{self, Fall, Push, Settings};
use photocraft_scene3d::pose;

/// The panel's settings and the fall being shown.
#[derive(Default)]
pub struct Gravity {
    /// Muscle tone, 0–1.
    pub tone: f32,
    pub push: Push,
    /// Pinned parts: figure and joint name.
    pub pins: Vec<(usize, String)>,
    pub fall: Option<Playback>,
}

/// A simulated fall, shown on its figure until it is kept or cancelled.
pub struct Playback {
    pub figure: usize,
    pub fall: Fall,
    /// The scene before the fall (its undo step, and what Cancel goes back to).
    pub before: Scene3d,
    /// Seconds into the fall.
    pub time: f32,
    pub playing: bool,
}

impl Gravity {
    pub fn new() -> Self {
        Gravity { push: Push::Forward, ..Default::default() }
    }

    fn pins_of(&self, figure: usize) -> Vec<String> {
        self.pins.iter().filter(|(f, _)| *f == figure).map(|(_, j)| j.clone()).collect()
    }
}

impl App {
    /// The Gravity panel's body.
    pub(super) fn gravity_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Let the figure fall under its own weight.");
        ui.weak("Pin a hand to hang it from that hand, or the feet to keep them planted.");
        ui.add_space(8.0);
        let showing = self.gravity.fall.is_some();
        ui.add_enabled_ui(!showing, |ui| self.gravity_settings(ui));
        ui.add_space(10.0);
        if showing {
            self.playback_ui(ui);
        } else if ui.add_sized([ui.available_width().min(220.0), 36.0], egui::Button::new(egui::RichText::new("Drop").strong())).clicked() {
            self.drop_figure();
        }
    }

    fn gravity_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Push");
            for p in Push::ALL {
                if ui.selectable_label(self.gravity.push == p, p.label()).clicked() {
                    self.gravity.push = p;
                }
            }
        });
        ui.add_space(4.0);
        ui.add(egui::Slider::new(&mut self.gravity.tone, 0.0..=1.0).text("Muscle tone").custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
        ui.weak(match self.gravity.tone {
            t if t < 0.15 => "Limp: drops like a doll.",
            t if t < 0.5 => "Slumps: the knees buckle and it sags.",
            t if t < 0.85 => "Firm: it sinks down, holding much of the pose.",
            _ => "Stiff: it topples, keeping the pose.",
        });
        ui.add_space(8.0);
        let figure = self.figure();
        let pins = self.gravity.pins_of(figure);
        ui.horizontal(|ui| {
            let pinning = self.mode == DragMode::Pin;
            if ui.selectable_label(pinning, "Pin parts").on_hover_text("Tap a part on the canvas to pin it where it is; tap it again to unpin").clicked() {
                self.mode = if pinning { DragMode::Pose } else { DragMode::Pin };
            }
            if ui.add_enabled(!pins.is_empty(), egui::Button::new("Clear pins")).clicked() {
                self.gravity.pins.retain(|(f, _)| *f != figure);
            }
        });
        if pins.is_empty() {
            ui.weak("No pins: it falls freely.");
        } else {
            let names: Vec<String> = pins.iter().map(|j| pose::joint_label(j)).collect();
            ui.weak(format!("Pinned: {}", names.join(", ")));
        }
    }

    fn playback_ui(&mut self, ui: &mut egui::Ui) {
        let Some(p) = self.gravity.fall.as_mut() else { return };
        let total = p.fall.seconds().max(1.0 / gravity::FPS);
        ui.horizontal(|ui| {
            let label = if p.playing { "Pause" } else { "Play" };
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
            }
            ui.monospace(format!("{:.1} s", p.time.min(total)));
        });
        ui.weak(if p.fall.settled { "Scrub to any moment of the fall, then keep it." } else { "It was still moving when the time ran out." });
        ui.add_space(6.0);
        let mut keep = false;
        let mut cancel = false;
        let mut again = false;
        ui.horizontal(|ui| {
            keep = ui.add(egui::Button::new(egui::RichText::new("Keep").strong())).on_hover_text("Keep the pose shown (one undo step)").clicked();
            cancel = ui.button("Cancel").clicked();
            again = ui.button("Drop again").on_hover_text("Back to the start and fall with the settings as they are now").clicked();
        });
        self.show_frame();
        if keep {
            self.keep_fall();
        } else if cancel {
            self.cancel_fall();
        } else if again {
            self.cancel_fall();
            self.drop_figure();
        }
    }

    /// Simulates the selected figure's fall from its pose now and starts playing it.
    pub fn drop_figure(&mut self) {
        let figure = self.figure();
        let Some(fig) = self.scene.figures.get(figure) else { return };
        let settings = Settings { tone: self.gravity.tone, push: self.gravity.push, pins: self.gravity.pins_of(figure), ..Settings::default() };
        let fall = gravity::simulate(fig, &settings);
        if self.mode == DragMode::Pin {
            self.mode = DragMode::Pose;
        }
        self.drag = None;
        self.gravity.fall = Some(Playback { figure, fall, before: self.scene.clone(), time: 0.0, playing: true });
    }

    /// Advances a playing fall (each frame).
    pub(super) fn tick_gravity(&mut self, ctx: &egui::Context) {
        let Some(p) = self.gravity.fall.as_mut() else { return };
        if !p.playing {
            return;
        }
        let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 0.1);
        p.time += dt;
        if p.time >= p.fall.seconds() {
            p.time = p.fall.seconds();
            p.playing = false;
        }
        self.show_frame();
        ctx.request_repaint();
    }

    /// Poses the falling figure as the fall's current moment.
    pub(super) fn show_frame(&mut self) {
        let Some(p) = self.gravity.fall.as_ref() else { return };
        let i = (p.time * gravity::FPS).round().max(0.0) as usize;
        let Some(fig) = self.scene.figures.get_mut(p.figure) else { return };
        let before = fig.clone();
        p.fall.apply(i, fig);
        if *fig != before {
            self.generation += 1;
        }
    }

    pub fn keep_fall(&mut self) {
        if let Some(p) = self.gravity.fall.take()
            && self.scene != p.before
        {
            self.history.push(p.before);
            self.changed();
        }
    }

    pub fn cancel_fall(&mut self) {
        if let Some(p) = self.gravity.fall.take() {
            // The camera stays where it was moved to while watching.
            if let (Some(fig), Some(old)) = (self.scene.figures.get_mut(p.figure), p.before.figures.get(p.figure)) {
                *fig = old.clone();
            }
            self.generation += 1;
        }
    }

    /// A press in Pin mode: pins the part under the pen, or unpins it. False when no part is there.
    pub(super) fn toggle_pin_at(&mut self, at: [f32; 2]) -> bool {
        let (w, h) = self.canvas;
        let Some(hit) = pose::pick(&self.scene, w, h, at) else { return false };
        let Some(name) = self.scene.figures.get(hit.figure).and_then(|f| pose::joint_name(f, hit.joint)) else { return false };
        let key = (hit.figure, name.to_string());
        if let Some(k) = self.gravity.pins.iter().position(|p| *p == key) {
            self.gravity.pins.remove(k);
        } else {
            self.say(format!("Pinned the {}", pose::joint_label(name).to_lowercase()));
            self.gravity.pins.push(key);
        }
        self.selected = Some((hit.figure, hit.joint));
        true
    }

    /// The pins, drawn where they hold their parts.
    pub(super) fn draw_pins(&self, painter: &egui::Painter, to_screen: &dyn Fn([f32; 2]) -> Pos2) {
        let (w, h) = self.canvas;
        // While a fall shows, its pins are where they were when it started.
        let scene = self.gravity.fall.as_ref().map_or(&self.scene, |p| &p.before);
        for (figure, name) in &self.gravity.pins {
            let Some(j) = scene.figures.get(*figure).and_then(|f| pose::joint_index(f, name)) else { continue };
            let Some((pivot, tip)) = pose::joint_px(scene, w, h, *figure, j) else { continue };
            let at = to_screen([(pivot[0] + tip[0]) / 2.0, (pivot[1] + tip[1]) / 2.0]);
            let head = at + egui::vec2(0.0, -16.0);
            painter.line_segment([at, head], Stroke::new(2.5, Color32::from_gray(235)));
            painter.circle(head, 7.0, Color32::from_rgb(232, 70, 70), Stroke::new(1.5, Color32::from_gray(20)));
            painter.circle_filled(at, 2.5, Color32::from_gray(20));
        }
    }
}
