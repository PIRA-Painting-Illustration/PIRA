//! The app: PIRA's Object tool and 3D panel on a canvas of their own. What a drag does, the
//! handles and the overlays come from `photocraft-scene3d-egui`, shared with PIRA's Object tool
//! (`crates/ui-egui/src/tools/object_tool.rs`); keep the options bar and 3D panel in step with it.
//!
//! A drag on a body part swings it about its joint in the canvas plane (with Twist on, or Alt, it
//! turns about its own bone instead); a drag on the hips slides the figure across its floor (with
//! Twist, turns it). A drag on empty canvas orbits the camera; Shift pans and Ctrl dollies, and the
//! options bar can make a plain drag do either, or place the light. Two fingers pan and pinch to
//! zoom. Each drag is one undo step.
//!
//! The 3D panel has the pose library, hand poses, matching a pose to a photo, and the figures.
//! With a photo open, the photo is the canvas and the figures are drawn over it.

use crate::dock::{self, PanelId};
use crate::files::{Event, Files, Location, Purpose};
use crate::photo::{self, Part, Photo, PhotoMatch};
use crate::scene::{self, FILE_EXTENSION, History};
use crate::view::{self, View};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use photocraft_doc::{Body3d, Figure3d, Scene3d, Shading3d};
use photocraft_scene3d::{self as s3d, camera, library, light, pose};
use photocraft_scene3d_egui::{self as pose_ui, DragKind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

mod gravity_ui;
use gravity_ui::Gravity;

const TOUCH: bool = cfg!(target_os = "android");
/// About a fingertip (or a comfortable mouse target) on screen, in points.
const REACH: f32 = if TOUCH { 26.0 } else { 16.0 };
const ACCENT: Color32 = Color32::from_rgb(92, 160, 255);

/// What a plain drag does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DragMode {
    /// Pose the part under the pen; empty canvas orbits the camera.
    #[default]
    Pose,
    Orbit,
    Pan,
    Zoom,
    /// Place the main light: the sun goes where you drag.
    Light,
    /// Tap a part to pin it for Gravity (or unpin it); empty canvas orbits.
    Pin,
}

struct Drag {
    kind: DragKind,
    /// The scene before the drag (for its undo step).
    before: Scene3d,
    last: [f32; 2],
    moved: bool,
}

/// Canvas pixels ↔ screen points: `screen = origin + canvas * scale`.
#[derive(Clone, Copy, Debug)]
struct Xform {
    origin: Pos2,
    scale: f32,
}

impl Xform {
    fn to_screen(self, p: [f32; 2]) -> Pos2 {
        self.origin + Vec2::new(p[0], p[1]) * self.scale
    }

    fn to_canvas(self, p: Pos2) -> [f32; 2] {
        let v = (p - self.origin) / self.scale;
        [v.x, v.y]
    }
}

struct MatchJob {
    rx: mpsc::Receiver<Result<photo::Found, String>>,
    part: Part,
    figure: usize,
    started: Instant,
}

pub struct App {
    pub scene: Scene3d,
    /// The canvas colour behind the figures (sRGB, 0–1), saved with the scene.
    pub background: [f32; 3],
    history: History<Scene3d>,
    pub mode: DragMode,
    pub twist: bool,
    pub joints: bool,
    /// The selected part: figure, joint.
    pub selected: Option<(usize, usize)>,
    category: String,
    search: String,
    hands: String,
    accurate: bool,
    /// The last body match is being fixed (its joints show on the canvas).
    fixing: bool,
    drag: Option<Drag>,
    /// Two fingers are down: they pan and zoom until all are lifted.
    gesture: bool,
    view: View,
    photo: Option<Photo>,
    photo_tex: Option<egui::TextureHandle>,
    photo_opacity: f32,
    photo_match: Option<PhotoMatch>,
    job: Option<MatchJob>,
    thumbs: HashMap<String, egui::TextureHandle>,
    files: Box<dyn Files>,
    doc: Option<Location>,
    doc_name: String,
    modified: bool,
    generation: u64,
    status: Option<(String, Instant)>,
    /// The panels' arrangement (craft-ui docking), and where it is saved.
    pub workspace: craft_ui::docking::Workspace<PanelId>,
    workspace_file: Option<PathBuf>,
    light_picker: bool,
    gravity: Gravity,
    /// Which light a Light-mode drag away from both suns places (the Light menu's choice).
    light_target: light::Which,
    canvas: (u32, u32),
    framed: bool,
    autosave: Option<PathBuf>,
    autosaved: (u64, Instant),
    title: String,
}

fn starting_scene() -> Scene3d {
    Scene3d { figures: vec![s3d::new_figure(Body3d::Male, None)], ..Scene3d::default() }
}

impl App {
    pub fn new(cc: &eframe::CreationContext, files: Box<dyn Files>, autosave: Option<PathBuf>, workspace_file: Option<PathBuf>) -> App {
        style(&cc.egui_ctx);
        let mut app = App::with_files(files, autosave);
        app.workspace = dock::load(workspace_file.as_deref());
        app.workspace_file = workspace_file;
        app
    }

    /// The app without a window (tests).
    pub fn with_files(files: Box<dyn Files>, autosave: Option<PathBuf>) -> App {
        let restored = autosave.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| scene::from_bytes(&b).ok());
        let was_restored = restored.is_some();
        let (restored, background) = restored.map_or((None, scene::DEFAULT_BACKGROUND), |(s, bg)| (Some(s), bg));
        App {
            scene: restored.unwrap_or_else(starting_scene),
            background,
            history: History::default(),
            mode: DragMode::Pose,
            twist: false,
            joints: false,
            selected: None,
            category: library::BASIC.to_string(),
            search: String::new(),
            hands: "both".into(),
            accurate: false,
            fixing: false,
            drag: None,
            gesture: false,
            view: View::default(),
            photo: None,
            photo_tex: None,
            photo_opacity: 1.0,
            photo_match: None,
            job: None,
            thumbs: HashMap::new(),
            files,
            doc: None,
            doc_name: "Untitled".into(),
            modified: was_restored,
            generation: 0,
            status: was_restored.then(|| ("Restored your last session".to_string(), Instant::now())),
            workspace: dock::default_workspace(),
            workspace_file: None,
            light_picker: false,
            gravity: Gravity::new(),
            light_target: light::Which::Main,
            canvas: (1200, 1600),
            framed: was_restored,
            autosave,
            autosaved: (0, Instant::now()),
            title: String::new(),
        }
    }

    fn say(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::info!("{text}");
        self.status = Some((text, Instant::now()));
    }

    fn changed(&mut self) {
        self.modified = true;
        self.generation += 1;
    }

    /// One undo step: `f` edits the scene (it gets the canvas size). A fall being shown is kept
    /// first, so the edit starts from the pose on screen.
    pub fn edit(&mut self, f: impl FnOnce(&mut Scene3d, (u32, u32))) {
        self.keep_fall();
        let before = self.scene.clone();
        f(&mut self.scene, self.canvas);
        if self.scene != before {
            self.history.push(before);
            self.changed();
        }
    }

    pub fn undo(&mut self) {
        // Undo while a fall shows takes the figure back to before it.
        if self.gravity.fall.is_some() {
            self.cancel_fall();
            return;
        }
        if let Some(prev) = self.history.undo(self.scene.clone()) {
            self.scene = prev;
            self.drag = None;
            self.changed();
        }
    }

    pub fn redo(&mut self) {
        if self.gravity.fall.is_some() {
            return;
        }
        if let Some(next) = self.history.redo(self.scene.clone()) {
            self.scene = next;
            self.drag = None;
            self.changed();
        }
    }

    /// The figure the panel and options bar act on.
    pub fn figure(&self) -> usize {
        self.selected.map(|s| s.0).filter(|&f| f < self.scene.figures.len()).unwrap_or(0)
    }

    // ---- Commands (PIRA's scene3d commands) ---------------------------------------------------

    pub fn apply_pose(&mut self, id: &str, sides: pose::Sides) {
        let fi = self.figure();
        let mut err = None;
        self.edit(|s, _| {
            if let Some(f) = s.figures.get_mut(fi)
                && let Err(e) = library::apply(f, id, sides)
            {
                err = Some(e);
            }
        });
        if let Some(e) = err {
            self.say(e);
        }
    }

    pub fn add_figure(&mut self, body: Body3d) {
        if self.scene.figures.len() >= 64 {
            self.say("A scene holds at most 64 figures");
            return;
        }
        self.edit(|s, _| {
            let mut fig = s3d::new_figure(body, None);
            // Beside the others, on the camera's right.
            let right = s.figures.iter().map(|f| f.position[0]).fold(f32::MIN, f32::max);
            if right > f32::MIN {
                fig.position = [right + 0.7, 0.0, s.figures.first().map_or(0.0, |f| f.position[2])];
            }
            s.figures.push(fig);
        });
        self.selected = Some((self.scene.figures.len() - 1, 0));
    }

    pub fn remove_figure(&mut self, fi: usize) {
        if fi >= self.scene.figures.len() {
            return;
        }
        self.edit(|s, _| {
            s.figures.remove(fi);
        });
        self.selected = None;
        if self.photo_match.as_ref().is_some_and(|m| m.figure >= fi) {
            self.photo_match = None;
        }
    }

    fn figure_edit(&mut self, f: impl FnOnce(&mut Figure3d)) {
        let fi = self.figure();
        self.edit(|s, _| {
            if let Some(fig) = s.figures.get_mut(fi) {
                f(fig);
            }
        });
    }

    pub fn frame_figures(&mut self) {
        self.edit(|s, (w, h)| s3d::frame(s, w, h, 0.82));
    }

    fn new_scene(&mut self) {
        let before = std::mem::replace(&mut self.scene, starting_scene());
        self.history.push(before);
        self.changed();
        let (w, h) = self.canvas;
        s3d::frame(&mut self.scene, w, h, 0.82);
        self.selected = None;
        self.photo_match = None;
        self.background = scene::DEFAULT_BACKGROUND;
        self.doc = None;
        self.doc_name = "Untitled".into();
        self.modified = false;
    }

    // ---- Files ------------------------------------------------------------------------------

    fn save(&mut self, save_as: bool) {
        let name = format!("{}.{FILE_EXTENSION}", self.doc_name);
        let to = if save_as { None } else { self.doc.clone() };
        self.files.save(Purpose::Save, &name, scene::to_bytes(&self.scene, self.background), to.as_ref());
    }

    fn export_png(&mut self, under: Under) {
        let size = self.canvas;
        let fill;
        let photo = match under {
            Under::Nothing => None,
            Under::Background => {
                let [r, g, b] = self.background.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
                fill = [r, g, b, 255].repeat(size.0 as usize * size.1 as usize);
                Some(fill.as_slice())
            }
            Under::Photo => self.photo.as_ref().map(|p| p.rgba.as_slice()),
        };
        let rgba = view::flatten(&self.scene, size, photo);
        let Some(img) = image::RgbaImage::from_raw(size.0, size.1, rgba) else { return };
        let mut png = std::io::Cursor::new(Vec::new());
        if let Err(e) = img.write_to(&mut png, image::ImageFormat::Png) {
            self.say(format!("couldn't make the PNG: {e}"));
            return;
        }
        self.files.save(Purpose::Export, &format!("{}.png", self.doc_name), png.into_inner(), None);
    }

    pub fn take_file(&mut self, purpose: Purpose, name: &str, bytes: &[u8], location: Option<Location>) {
        match purpose {
            Purpose::Photo => match Photo::decode(name, bytes) {
                Ok(p) => {
                    self.photo_tex = None;
                    self.photo_match = None;
                    self.fixing = false;
                    self.canvas = (p.size[0] as u32, p.size[1] as u32);
                    self.photo = Some(p);
                    self.say(format!("{name}: pick a figure, then Match photo › Body or Hands"));
                }
                Err(e) => self.say(e),
            },
            _ => match scene::from_bytes(bytes) {
                Ok((s, background)) => {
                    self.history.push(std::mem::replace(&mut self.scene, s));
                    self.background = background;
                    self.selected = None;
                    self.photo_match = None;
                    self.doc = location;
                    self.doc_name = name.trim_end_matches(&format!(".{FILE_EXTENSION}")).to_string();
                    self.changed();
                    self.modified = false;
                    self.say(format!("Opened {name}"));
                }
                Err(e) => self.say(format!("{name}: {e}")),
            },
        }
    }

    fn poll_files(&mut self) {
        while let Some(ev) = self.files.poll() {
            match ev {
                Event::Opened { purpose, name, bytes, location } => self.take_file(purpose, &name, &bytes, location),
                Event::Saved { purpose: Purpose::Save, name, location } => {
                    self.doc = Some(location);
                    self.doc_name = name.trim_end_matches(&format!(".{FILE_EXTENSION}")).to_string();
                    self.modified = false;
                    self.say(format!("Saved {name}"));
                }
                Event::Saved { name, .. } => self.say(format!("Exported {name}")),
                Event::Failed(e) => self.say(e),
                Event::Cancelled => {}
            }
        }
    }

    fn autosave_now(&mut self) {
        let Some(path) = self.autosave.clone() else { return };
        if self.autosaved.0 == self.generation || self.autosaved.1.elapsed() < Duration::from_secs(2) || self.drag.is_some() {
            return;
        }
        let tmp = path.with_extension("tmp");
        match std::fs::write(&tmp, scene::to_bytes(&self.scene, self.background)).and_then(|()| std::fs::rename(&tmp, &path)) {
            Ok(()) => self.autosaved = (self.generation, Instant::now()),
            Err(e) => {
                log::warn!("autosave: {e}");
                self.autosaved.1 = Instant::now();
            }
        }
    }

    // ---- Photo match ------------------------------------------------------------------------

    fn match_photo(&mut self, part: Part) {
        let Some(p) = &self.photo else {
            self.say("Open a photo first");
            self.files.open(Purpose::Photo);
            return;
        };
        if self.job.is_some() {
            return;
        }
        let (rgba, size, accurate) = (p.rgba.clone(), p.size, self.accurate);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(photo::track(&rgba, size, part, accurate));
        });
        self.job = Some(MatchJob { rx, part, figure: self.figure(), started: Instant::now() });
        if part == Part::Body {
            self.fixing = true;
        }
    }

    fn poll_match(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("The pose finder stopped".into()),
        };
        let (part, fi, secs) = (job.part, job.figure, job.started.elapsed().as_secs_f32());
        self.job = None;
        let before = self.scene.clone();
        match result.and_then(|found| photo::apply(&mut self.scene, fi, part, found)) {
            Ok((matched, m)) => {
                self.history.push(before);
                self.changed();
                if let Some(m) = m {
                    self.photo_match = Some(m);
                }
                self.say(format!("Matched {} in {secs:.1} s", matched.join(" and ")));
            }
            Err(e) => {
                self.scene = before;
                if part == Part::Body {
                    self.fixing = false;
                }
                self.say(e);
            }
        }
    }

    /// The photo match, while the Match photo tab shows it (its joints are draggable).
    fn shown_match(&self) -> Option<&PhotoMatch> {
        if !dock::in_front(&self.workspace.layout, PanelId::Photo) || !self.fixing || self.photo.is_none() {
            return None;
        }
        self.photo_match.as_ref().filter(|m| m.figure < self.scene.figures.len())
    }

    fn refit(&mut self, change: impl FnOnce(&mut PhotoMatch)) {
        let Some(mut m) = self.photo_match.clone() else { return };
        change(&mut m);
        let before = self.scene.clone();
        match photo::refit(&mut self.scene, &mut m) {
            Ok(()) => {
                self.history.push(before);
                self.changed();
                self.photo_match = Some(m);
            }
            Err(e) => {
                self.scene = before;
                self.say(e);
            }
        }
    }

    // ---- Frame ------------------------------------------------------------------------------

    pub fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll_files();
        self.poll_match(&ctx);
        self.shortcuts(&ctx);
        self.dropped_files(&ctx);
        self.tick_gravity(&ctx);

        let title = format!("{}{} - PIRA Pose", self.doc_name, if self.modified { "*" } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("options").show(ui, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| ui.horizontal(|ui| self.options_bar(ui)));
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_gray(38))).show(ui, |ui| self.dock_ui(ui));

        if self.autosave.is_some() && self.autosaved.0 != self.generation {
            self.autosave_now();
            ctx.request_repaint_after(Duration::from_secs(2));
        }
        if self.files.busy() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
        if let Some((_, at)) = &self.status {
            let left = Duration::from_secs(5).saturating_sub(at.elapsed());
            if left.is_zero() {
                self.status = None;
            } else {
                ctx.request_repaint_after(left);
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let cmd = Modifiers::COMMAND;
        let (undo, redo, save, save_as, open) = ctx.input_mut(|i| {
            (
                i.consume_key(cmd, Key::Z),
                i.consume_key(cmd | Modifiers::SHIFT, Key::Z) || i.consume_key(cmd, Key::Y),
                i.consume_key(cmd, Key::S),
                i.consume_key(cmd | Modifiers::SHIFT, Key::S),
                i.consume_key(cmd, Key::O),
            )
        });
        if redo {
            self.redo();
        } else if undo {
            self.undo();
        }
        if save_as {
            self.save(true);
        } else if save {
            self.save(false);
        }
        if open {
            self.files.open(Purpose::Open);
        }
    }

    fn dropped_files(&mut self, ctx: &egui::Context) {
        for f in ctx.input(|i| i.raw.dropped_files.clone()) {
            let path = f.path().to_path_buf();
            if path.as_os_str().is_empty() {
                continue;
            }
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let purpose = if name.to_ascii_lowercase().ends_with(FILE_EXTENSION) { Purpose::Open } else { Purpose::Photo };
                    self.take_file(purpose, &name, &bytes, Some(Location(path.to_string_lossy().into_owned())));
                }
                Err(e) => self.say(format!("couldn't read {name}: {e}")),
            }
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New scene").clicked() {
                    self.new_scene();
                    ui.close();
                }
                if ui.button("Open…").clicked() {
                    self.files.open(Purpose::Open);
                    ui.close();
                }
                if ui.button("Save").clicked() {
                    self.save(false);
                    ui.close();
                }
                if ui.button("Save As…").clicked() {
                    self.save(true);
                    ui.close();
                }
                ui.separator();
                if ui.button("Open photo…").clicked() {
                    self.files.open(Purpose::Photo);
                    ui.close();
                }
                if ui.add_enabled(self.photo.is_some(), egui::Button::new("Remove photo")).clicked() {
                    self.photo = None;
                    self.photo_tex = None;
                    self.photo_match = None;
                    self.fixing = false;
                    ui.close();
                }
                ui.separator();
                if ui.button("Export PNG…").on_hover_text("The figures on the background colour, at the canvas size").clicked() {
                    self.export_png(Under::Background);
                    ui.close();
                }
                if ui.button("Export PNG, transparent…").on_hover_text("The figures alone, on a transparent background").clicked() {
                    self.export_png(Under::Nothing);
                    ui.close();
                }
                if ui.add_enabled(self.photo.is_some(), egui::Button::new("Export PNG with photo…")).clicked() {
                    self.export_png(Under::Photo);
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(self.history.can_undo(), egui::Button::new("Undo")).clicked() {
                    self.undo();
                    ui.close();
                }
                if ui.add_enabled(self.history.can_redo(), egui::Button::new("Redo")).clicked() {
                    self.redo();
                    ui.close();
                }
            });
            ui.menu_button("Window", |ui| {
                for panel in PanelId::all().into_iter().filter(|p| *p != PanelId::Canvas) {
                    let mut on = dock::shown(&self.workspace.layout, panel);
                    if ui.checkbox(&mut on, panel.label()).clicked() {
                        self.show_panel(panel, on);
                    }
                }
                ui.separator();
                if ui.button("Reset panels").on_hover_text("Put every panel back where it starts").clicked() {
                    self.workspace.reset(&dock::default_layout());
                    self.workspace.hidden.clear();
                    dock::save(self.workspace_file.as_deref(), &self.workspace);
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                ui.strong(format!("PIRA Pose {}", env!("CARGO_PKG_VERSION")));
                ui.label(HELP);
                ui.separator();
                ui.weak("Motion-capture poses from mocap.cs.cmu.edu. The database was created with funding from NSF EIA-0196217.");
                ui.weak("Photo matching uses Google's MediaPipe models (Apache 2.0).");
            });
        });
    }

    /// PIRA's Object tool options bar.
    fn options_bar(&mut self, ui: &mut egui::Ui) {
        let modes = [
            (DragMode::Pose, "Pose", "Pose: drag a part to turn it, empty canvas to orbit"),
            (DragMode::Orbit, "Orbit", "Orbit the camera"),
            (DragMode::Pan, "Pan", "Pan the camera (Shift)"),
            (DragMode::Zoom, "Zoom", "Dolly the camera (Ctrl)"),
            (DragMode::Light, "Light", "Light: drag to place the sun (inside the ring lights the front, outside lights from behind)"),
            (DragMode::Pin, "Pin", "Pin: tap a part to pin it for Gravity, tap again to unpin"),
        ];
        for (m, label, tip) in modes {
            if ui.selectable_label(self.mode == m, label).on_hover_text(tip).clicked() {
                self.mode = m;
            }
        }
        ui.separator();
        ui.toggle_value(&mut self.twist, "Twist").on_hover_text("Twist: turn parts about their own bone (Alt)");
        ui.toggle_value(&mut self.joints, "Joints").on_hover_text("Joints: drag the head, elbows, wrists, knees and ankles to where they should be");
        ui.separator();
        ui.menu_button("View", |ui| {
            for (id, label) in s3d::CAMERA_VIEWS {
                if ui.button(label).clicked() {
                    self.edit(|s, (w, h)| {
                        if let Some(c) = s3d::camera_preset(&s.camera, id) {
                            s.camera = c;
                        }
                        s3d::frame(s, w, h, 0.82);
                    });
                    ui.close();
                }
            }
            ui.separator();
            let parallel = self.scene.camera.parallel;
            if ui.selectable_label(parallel, "Parallel projection").clicked() {
                self.edit(|s, _| s.camera.parallel = !parallel);
                ui.close();
            }
        });
        if ui.button("Fit").on_hover_text("Fit the figures to the canvas").clicked() {
            self.frame_figures();
        }
        if ui.button("Floor").on_hover_text("Place on floor").clicked() {
            self.figure_edit(|f| {
                pose::place_on_floor(f);
                f.position[1] = 0.0;
            });
        }
        ui.menu_button("Mirror", |ui| {
            if ui.button("Left side onto right").clicked() {
                self.figure_edit(|f| pose::mirror(f, true));
                ui.close();
            }
            if ui.button("Right side onto left").clicked() {
                self.figure_edit(|f| pose::mirror(f, false));
                ui.close();
            }
            if ui.button("Flip pose").clicked() {
                self.figure_edit(pose::flip);
                ui.close();
            }
        });
        ui.menu_button("Figure", |ui| {
            let fi = self.figure();
            let body = self.scene.figures.get(fi).map_or(Body3d::Male, |f| f.body);
            for b in [Body3d::Male, Body3d::Female] {
                if ui.selectable_label(body == b, b.label()).clicked() {
                    self.figure_edit(|f| f.body = b);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Reset pose").clicked() {
                self.apply_pose("Standing", pose::Sides::Both);
                ui.close();
            }
            if ui.button("Add male figure").clicked() {
                self.add_figure(Body3d::Male);
                ui.close();
            }
            if ui.button("Add female figure").clicked() {
                self.add_figure(Body3d::Female);
                ui.close();
            }
            if self.scene.figures.len() > 1 && ui.button("Delete this figure").clicked() {
                self.remove_figure(fi);
                ui.close();
            }
        });
        ui.menu_button("Look", |ui| {
            for s in Shading3d::ALL {
                if ui.selectable_label(self.scene.look.shading == s, s.label()).clicked() {
                    self.edit(|sc, _| sc.look.shading = s);
                    ui.close();
                }
            }
            ui.separator();
            let shadow = self.scene.look.floor_shadow;
            if ui.selectable_label(shadow, "Floor shadow").clicked() {
                self.edit(|sc, _| sc.look.floor_shadow = !shadow);
                ui.close();
            }
            ui.separator();
            ui.label("Outline");
            for (px, label) in [(0.0, "None"), (1.5, "Thin"), (3.0, "Medium"), (6.0, "Thick")] {
                if ui.selectable_label((self.scene.look.outline_px - px).abs() < 0.01, label).clicked() {
                    self.edit(|sc, _| sc.look.outline_px = px);
                    ui.close();
                }
            }
            ui.separator();
            // The colours open as sub-menus with the picker inside: a colour button's own popup
            // would count as a click outside the menu and close it.
            ui.menu_button("Body colour", |ui| {
                let before = self.scene.clone();
                if let Some((c, start)) = inline_color(ui, self.scene.look.body_color) {
                    self.scene.look.body_color = c;
                    // One undo step per press in the picker.
                    if start || !self.history.can_undo() {
                        self.history.push(before);
                    }
                    self.changed();
                }
            });
            ui.menu_button("Background", |ui| {
                if let Some((c, _)) = inline_color(ui, self.background) {
                    self.background = c;
                    self.changed();
                }
                if ui.add_enabled(self.background != scene::DEFAULT_BACKGROUND, egui::Button::new("Neutral grey")).clicked() {
                    self.background = scene::DEFAULT_BACKGROUND;
                    self.changed();
                }
            });
        });
        ui.separator();
        // Show or hide the panels (they dock, tab and float: drag a panel by its tab).
        for (panel, tip) in
            [(PanelId::Light, "The Light panel"), (PanelId::Poses, "The pose library"), (PanelId::Gravity, "Gravity: drop, slump or hang the figure")]
        {
            let on = dock::shown(&self.workspace.layout, panel);
            if ui.selectable_label(on, panel.label()).on_hover_text(tip).clicked() {
                self.show_panel(panel, !on);
            }
        }
        if ui.add_enabled(self.history.can_undo(), egui::Button::new("Undo")).clicked() {
            self.undo();
        }
        if ui.add_enabled(self.history.can_redo(), egui::Button::new("Redo")).clicked() {
            self.redo();
        }
    }
}

/// An sRGB colour picker drawn in place (no popup), for menus. The new colour when it changed,
/// and whether this frame's press started the change (for one undo step per press).
fn inline_color(ui: &mut egui::Ui, current: [f32; 3]) -> Option<([f32; 3], bool)> {
    let rgb = current.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
    let mut hsva = egui::ecolor::Hsva::from_srgb(rgb);
    let pressed = ui.input(|i| i.pointer.any_pressed());
    let r = ui.scope(|ui| egui::color_picker::color_picker_hsva_2d(ui, &mut hsva, egui::color_picker::Alpha::Opaque));
    r.inner.then(|| (hsva.to_srgb().map(|v| f32::from(v) / 255.0), pressed && r.response.contains_pointer()))
}

impl App {
    /// The canvas colour: a colour button, and back to neutral grey.
    fn background_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Background");
            let mut c = self.background.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            if ui.color_edit_button_srgb(&mut c).changed() {
                self.background = c.map(|v| f32::from(v) / 255.0);
                self.changed();
            }
            if ui.add_enabled(self.background != scene::DEFAULT_BACKGROUND, egui::Button::new("Neutral grey")).clicked() {
                self.background = scene::DEFAULT_BACKGROUND;
                self.changed();
            }
        });
    }

    /// The Light panel: presets, direction, strength, colour, ambient, shadow, the second light.
    fn light_menu(&mut self, ui: &mut egui::Ui) {
        let placing = self.mode == DragMode::Light;
        if ui.selectable_label(placing && self.light_target == light::Which::Main, "Place the light on the canvas").clicked() {
            self.mode = DragMode::Light;
            self.light_target = light::Which::Main;
        }
        if self.scene.look.light2 && ui.selectable_label(placing && self.light_target == light::Which::Second, "Place the second light on the canvas").clicked()
        {
            self.mode = DragMode::Light;
            self.light_target = light::Which::Second;
        }
        ui.separator();
        ui.label("Light from");
        ui.horizontal_wrapped(|ui| {
            for (id, label, _) in light::PRESETS {
                if ui.button(label).clicked() {
                    self.edit(|s, _| {
                        light::apply_preset(s, id);
                    });
                }
            }
        });
        ui.separator();
        // Each slider drag (or colour edit) is one undo step.
        let before = self.scene.clone();
        let mut start = false;
        let mut picker = self.light_picker;
        {
            let look = &mut self.scene.look;
            let mut slider = |ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, suffix: &str| {
                let r = ui.add(egui::Slider::new(v, range).text(label).suffix(suffix));
                if r.drag_started() || r.gained_focus() || (r.changed() && !r.dragged()) {
                    start = true;
                }
            };
            let mut yaw = (look.light_yaw.to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
            let mut pitch = look.light_pitch.to_degrees();
            let (y0, p0) = (yaw, pitch);
            slider(ui, "Direction", &mut yaw, -180.0..=180.0, "°");
            slider(ui, "Height", &mut pitch, -85.0..=85.0, "°");
            if yaw != y0 {
                look.light_yaw = yaw.to_radians();
            }
            if pitch != p0 {
                look.light_pitch = pitch.to_radians();
            }
            slider(ui, "Strength", &mut look.light_strength, 0.0..=2.0, "");
            slider(ui, "Ambient", &mut look.ambient, 0.0..=1.0, "");
            ui.label("Light colour");
            let current = look.light_color.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            ui.horizontal_wrapped(|ui| {
                for (h, tip) in pose_ui::LIGHT_SWATCHES {
                    let c = Color32::from_hex(h).unwrap_or(Color32::WHITE);
                    let (rect, r) = ui.allocate_exact_size(Vec2::splat(if TOUCH { 30.0 } else { 22.0 }), egui::Sense::click());
                    ui.painter().rect_filled(rect, 4.0, c);
                    let on = current == [c.r(), c.g(), c.b()];
                    let edge = if on { Stroke::new(2.0, ACCENT) } else { Stroke::new(1.0, Color32::from_black_alpha(90)) };
                    ui.painter().rect_stroke(rect, 4.0, edge, egui::StrokeKind::Inside);
                    if r.on_hover_text(tip).clicked() {
                        look.light_color = [c.r(), c.g(), c.b()].map(|v| f32::from(v) / 255.0);
                        start = true;
                    }
                }
                if ui.selectable_label(picker, "Custom…").clicked() {
                    picker = !picker;
                }
            });
            if picker {
                let mut hsva = egui::ecolor::Hsva::from_srgb(current);
                let pressed = ui.input(|i| i.pointer.any_pressed());
                let r = ui.scope(|ui| egui::color_picker::color_picker_hsva_2d(ui, &mut hsva, egui::color_picker::Alpha::Opaque));
                if pressed && r.response.contains_pointer() {
                    start = true;
                }
                if r.inner {
                    look.light_color = hsva.to_srgb().map(|v| f32::from(v) / 255.0);
                }
            }
        }
        self.light_picker = picker;
        if self.scene != before {
            if start || !self.history.can_undo() {
                self.history.push(before);
            }
            self.changed();
        }
        // The second light (shared with PIRA's Light menu).
        ui.separator();
        let before = self.scene.clone();
        let e = pose_ui::second_light_ui(ui, &mut self.scene, &|s| s);
        if e.changed {
            if e.started || !self.history.can_undo() {
                self.history.push(before);
            }
            self.changed();
        }
        ui.separator();
        let fixed = self.scene.look.light_fixed;
        if ui.selectable_label(!fixed, "Light turns with the camera").on_hover_text("Off: the light stays put in the scene while you orbit").clicked() {
            self.edit(|s, _| light::set_fixed(s, !fixed));
        }
        let shadow = self.scene.look.floor_shadow;
        if ui.selectable_label(shadow, "Floor shadow").clicked() {
            self.edit(|s, _| s.look.floor_shadow = !shadow);
        }
        ui.separator();
        self.background_ui(ui);
    }

    // ---- 3D panel ---------------------------------------------------------------------------

    /// Shows or hides a panel (a shown one comes back where it was, or beside the 3D panels).
    pub fn show_panel(&mut self, panel: PanelId, on: bool) {
        let fallback = [PanelId::Poses, PanelId::Figures, PanelId::Light].into_iter().find(|p| *p != panel && dock::shown(&self.workspace.layout, *p));
        match self.workspace.set_visible(panel, on, fallback, dock::permissions) {
            Ok(()) => dock::save(self.workspace_file.as_deref(), &self.workspace),
            Err(e) => log::warn!("panel {panel:?}: {e:?}"),
        }
    }

    /// The canvas and the panels, as the user has arranged them.
    fn dock_ui(&mut self, ui: &mut egui::Ui) {
        let mut style = craft_ui::docking::DockStyle::from_ui(ui);
        style.new_dock_zones = true;
        if TOUCH {
            style.tab_height = 40.0;
        }
        let layout = self.workspace.layout.clone();
        let output = craft_ui::docking::DockArea::new(egui::Id::unique("pira-pose-dock")).show_customized(
            ui,
            &layout,
            &style,
            |p| p.label().to_string(),
            dock::permissions,
            dock::limits,
            &mut Panels { app: self },
        );
        if let Some(e) = output.error {
            log::warn!("panel layout: {e:?}; back to the default");
            self.workspace = dock::default_workspace();
        }
        if !output.actions.is_empty() {
            match self.workspace.apply_all(output.actions, dock::permissions) {
                Ok(_) => dock::save(self.workspace_file.as_deref(), &self.workspace),
                Err(e) => log::warn!("panel move refused: {e:?}"),
            }
        }
    }

    fn hands_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Pose");
            for (id, label) in [("both", "Both hands"), ("left", "Left"), ("right", "Right")] {
                if ui.selectable_label(self.hands == id, label).clicked() {
                    self.hands = id.to_string();
                }
            }
        });
        ui.add_space(6.0);
        let sides = pose::Sides::from_id(&self.hands).unwrap_or(pose::Sides::Both);
        let mut picked = None;
        ui.horizontal_wrapped(|ui| {
            for e in library::entries().iter().filter(|e| e.hands_only) {
                if ui.button(e.name.as_str()).clicked() {
                    picked = Some(e.id.clone());
                }
            }
        });
        if let Some(id) = picked {
            self.apply_pose(&id, sides);
        }
    }

    fn photo_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("Pose a figure like the person or hands in a photo.");
        ui.horizontal_wrapped(|ui| {
            if ui.button(if self.photo.is_some() { "Change photo…" } else { "Open photo…" }).clicked() {
                self.files.open(Purpose::Photo);
            }
            if let Some(p) = &self.photo {
                ui.weak(&p.name);
            }
        });
        if self.photo.is_some() {
            ui.add(egui::Slider::new(&mut self.photo_opacity, 0.0..=1.0).text("Photo opacity"));
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Body model");
            if ui.selectable_label(!self.accurate, "Quick").clicked() {
                self.accurate = false;
            }
            if ui.selectable_label(self.accurate, "Accurate").clicked() {
                self.accurate = true;
            }
        });
        ui.weak(if self.accurate {
            "Accurate: a larger model, steadier on hard poses, about three times slower."
        } else {
            "Quick: fast. Try Accurate if the pose comes out wrong."
        });
        ui.add_space(6.0);
        let mut part = None;
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(self.job.is_none(), |ui| {
                for (p, label) in [(Part::Body, "Body"), (Part::Hands, "Both hands"), (Part::Hand(false), "Left hand"), (Part::Hand(true), "Right hand")] {
                    if ui.button(label).clicked() {
                        part = Some(p);
                    }
                }
            });
        });
        if let Some(p) = part {
            self.match_photo(p);
        }
        if self.job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Finding the pose…");
            });
        }
        ui.add_space(6.0);
        ui.weak("The first match takes a few seconds longer while the models load.");
        // Fixing the match: drag joints on the canvas, set each limb's depth here.
        let fi = self.figure();
        let shown = self.photo_match.clone().filter(|m| m.figure == fi && self.photo.is_some());
        if shown.is_some() && !self.fixing {
            ui.add_space(10.0);
            ui.separator();
            if ui.button("Fix the match").clicked() {
                self.fixing = true;
            }
        }
        let Some(m) = shown.filter(|_| self.fixing) else { return };
        ui.add_space(10.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Fix the match");
            if ui.button("Done").clicked() {
                self.fixing = false;
            }
        });
        ui.weak("Drag the coral dots on the photo onto the person's joints. White dots are ones you've moved.");
        ui.weak("If a limb points the wrong way, say which end is nearer the camera:");
        ui.add_space(4.0);
        let mut depth = None;
        for (k, (name, _, _)) in s3d::retarget::SEGMENTS.iter().enumerate() {
            let now = m.now.get(k).copied().unwrap_or(true);
            let set_by_user = m.toward.get(k).copied().flatten().is_some();
            ui.horizontal(|ui| {
                let text = if set_by_user { egui::RichText::new(*name).strong() } else { egui::RichText::new(*name) };
                ui.add_sized([110.0, 18.0], egui::Label::new(text));
                for (toward, label) in [(true, "Toward me"), (false, "Away")] {
                    if ui.selectable_label(now == toward, label).clicked() && now != toward {
                        depth = Some((k, toward));
                    }
                }
            });
        }
        if let Some((k, toward)) = depth {
            self.refit(|m| m.toward[k] = Some(toward));
        }
        ui.add_space(4.0);
        let fixes = !m.moved.is_empty() || m.toward.iter().any(Option::is_some);
        if ui.add_enabled(fixes, egui::Button::new("Undo all fixes")).clicked() {
            self.refit(|m| {
                m.moved.clear();
                m.toward = [None; 8];
            });
        }
    }

    fn figures_tab(&mut self, ui: &mut egui::Ui) {
        ui.label("Add a figure");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Add male").clicked() {
                self.add_figure(Body3d::Male);
            }
            if ui.button("Add female").clicked() {
                self.add_figure(Body3d::Female);
            }
        });
        ui.add_space(8.0);
        ui.label("Figures");
        let fi = self.figure();
        let mut pick = None;
        let mut toggle = None;
        let mut delete = None;
        for (i, f) in self.scene.figures.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut vis = f.visible;
                if ui.checkbox(&mut vis, "").on_hover_text("Show or hide").changed() {
                    toggle = Some(i);
                }
                if ui.selectable_label(i == fi, format!("{} {}", f.body.label(), i + 1)).clicked() {
                    pick = Some(i);
                }
                if self.scene.figures.len() > 1 && ui.small_button("Delete").clicked() {
                    delete = Some(i);
                }
            });
        }
        if let Some(i) = pick {
            self.selected = Some((i, 0));
        }
        if let Some(i) = toggle {
            self.edit(|s, _| s.figures[i].visible ^= true);
        }
        if let Some(i) = delete {
            self.remove_figure(i);
        }
        ui.add_space(8.0);
        if let Some(f) = self.scene.figures.get(fi).cloned() {
            ui.label("This figure");
            let mut yaw = (f.yaw.to_degrees() + 180.0).rem_euclid(360.0) - 180.0;
            let mut scale = f.scale;
            let r1 = ui.add(egui::Slider::new(&mut yaw, -180.0..=180.0).text("Turn").suffix("°"));
            let r2 = ui.add(egui::Slider::new(&mut scale, 0.2..=3.0).text("Size"));
            if r1.changed() || r2.changed() {
                let before = self.scene.clone();
                if let Some(g) = self.scene.figures.get_mut(fi) {
                    if r1.changed() {
                        g.yaw = yaw.to_radians();
                    }
                    if r2.changed() {
                        g.scale = scale.clamp(0.05, 20.0);
                    }
                }
                if r1.drag_started() || r2.drag_started() || !(r1.dragged() || r2.dragged()) {
                    self.history.push(before);
                }
                self.changed();
            }
        }
    }

    /// The Poses tab: category, search and the thumbnail grid. A tap poses the selected figure.
    fn pose_grid(&mut self, ui: &mut egui::Ui) {
        let entries = library::entries();
        let cats: Vec<String> = library::categories().into_iter().filter(|c| c != library::HANDS).collect();
        if !cats.contains(&self.category) {
            self.category = library::BASIC.to_string();
        }
        ui.horizontal(|ui| {
            craft_ui::ComboBox::from_id_salt("pose-category").selected_text(self.category.as_str()).show_ui(ui, |ui| {
                for c in &cats {
                    let n = entries.iter().filter(|e| &e.category == c).count();
                    ui.selectable_value(&mut self.category, c.clone(), format!("{c} ({n})"));
                }
            });
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search poses").desired_width(f32::INFINITY));
        });
        ui.weak("Motion-capture poses from mocap.cs.cmu.edu (NSF EIA-0196217).");
        ui.add_space(4.0);
        let search = self.search.trim().to_lowercase();
        let shown: Vec<&library::Entry> = entries
            .iter()
            .filter(|e| !e.hands_only)
            .filter(|e| if search.is_empty() { e.category == self.category } else { e.name.to_lowercase().contains(&search) })
            .collect();
        if shown.is_empty() {
            ui.weak("No poses match.");
            return;
        }
        let cell = Vec2::new(THUMB.0 as f32 * 0.8 + 6.0, THUMB.1 as f32 * 0.8 + 6.0);
        let cols = ((ui.available_width() / cell.x).floor() as usize).max(1);
        let rows = shown.len().div_ceil(cols);
        let ctx = ui.ctx().clone();
        let mut budget = THUMBS_PER_FRAME;
        let mut picked: Option<String> = None;
        let thumbs = &mut self.thumbs;
        egui::ScrollArea::vertical().id_salt("pose-grid").auto_shrink([false, false]).show_rows(ui, cell.y, rows, |ui, range| {
            for row in range {
                ui.horizontal(|ui| {
                    for e in shown.iter().skip(row * cols).take(cols) {
                        let (rect, resp) = ui.allocate_exact_size(cell - Vec2::splat(4.0), egui::Sense::click());
                        let fill = if resp.hovered() { Color32::from_gray(70) } else { Color32::from_gray(52) };
                        ui.painter().rect_filled(rect, 4.0, fill);
                        if !thumbs.contains_key(&e.id) && budget > 0 {
                            budget -= 1;
                            let tex = ctx.load_texture(format!("pose-{}", e.id), pose_ui::thumb_image(&e.id), egui::TextureOptions::LINEAR);
                            thumbs.insert(e.id.clone(), tex);
                        }
                        if let Some(tex) = thumbs.get(&e.id) {
                            ui.painter().image(tex.id(), rect.shrink(2.0), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                        } else {
                            ctx.request_repaint();
                        }
                        if resp.on_hover_text(e.name.as_str()).clicked() {
                            picked = Some(e.id.clone());
                        }
                    }
                });
            }
        });
        if budget == 0 {
            ctx.request_repaint();
        }
        if let Some(id) = picked {
            self.apply_pose(&id, pose::Sides::Both);
        }
    }

    // ---- Canvas -----------------------------------------------------------------------------

    /// The canvas's size and where it sits in `rect`: the photo's size, fitted; or the whole area.
    fn layout(&self, rect: Rect, ppp: f32) -> ((u32, u32), Xform) {
        match &self.photo {
            Some(p) => {
                let (w, h) = (p.size[0] as f32, p.size[1] as f32);
                let scale = (rect.width() / w).min(rect.height() / h).max(1e-4);
                let origin = rect.center() - Vec2::new(w, h) * scale * 0.5;
                ((p.size[0] as u32, p.size[1] as u32), Xform { origin, scale })
            }
            None => {
                let size = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
                (size, Xform { origin: rect.min, scale: 1.0 / ppp })
            }
        }
    }

    /// The photo-match joint within reach of canvas point `at`, nearest first.
    fn photo_handle_at(&self, at: [f32; 2], reach: f32) -> Option<usize> {
        pose_ui::photo_handle_at(&self.shown_match()?.points(), at, reach)
    }

    /// The figure joint (Joints) within reach of canvas point `at`, nearest first.
    fn joint_handle_at(&self, at: [f32; 2], reach: f32) -> Option<(usize, usize)> {
        if !self.joints {
            return None;
        }
        pose_ui::joint_handle_at(&self.scene, self.canvas, at, reach)
    }

    /// A press on the canvas (canvas pixels): decides what the drag does.
    pub fn press(&mut self, at: [f32; 2], mods: egui::Modifiers, reach: f32) {
        let before = self.scene.clone();
        // While a fall shows, the canvas only moves the camera.
        if self.gravity.fall.is_some() {
            let kind = if mods.shift {
                DragKind::Pan
            } else if mods.command || mods.ctrl {
                DragKind::Dolly
            } else {
                match self.mode {
                    DragMode::Pan => DragKind::Pan,
                    DragMode::Zoom => DragKind::Dolly,
                    _ => DragKind::Orbit,
                }
            };
            self.drag = Some(Drag { kind, before, last: at, moved: false });
            return;
        }
        if self.mode == DragMode::Pin && !mods.shift && !mods.command && !mods.ctrl && self.toggle_pin_at(at) {
            return;
        }
        if let Some(point) = self.photo_handle_at(at, reach) {
            self.drag = Some(Drag { kind: DragKind::PhotoPoint(point), before, last: at, moved: false });
            return;
        }
        if let Some((figure, joint)) = self.joint_handle_at(at, reach) {
            self.selected = Some((figure, joint));
            self.drag = Some(Drag { kind: DragKind::Reach(figure, joint), before, last: at, moved: false });
            return;
        }
        let camera_kind = if mods.shift {
            Some(DragKind::Pan)
        } else if mods.command || mods.ctrl {
            Some(DragKind::Dolly)
        } else {
            match self.mode {
                DragMode::Pose => None,
                DragMode::Orbit => Some(DragKind::Orbit),
                DragMode::Pan => Some(DragKind::Pan),
                DragMode::Zoom => Some(DragKind::Dolly),
                DragMode::Pin => Some(DragKind::Orbit),
                // The sun under the pen, else the light picked in the Light menu.
                DragMode::Light => Some(pose_ui::light_drag_at(&self.scene, self.canvas, at, reach, self.light_target)),
            }
        };
        let twist = self.twist || mods.alt;
        let kind = match camera_kind {
            Some(k) => k,
            None => {
                let (hit, kind) = pose_ui::pose_drag_at(&self.scene, self.canvas, at, twist);
                if hit.is_some() {
                    self.selected = hit;
                }
                kind
            }
        };
        let mut d = Drag { kind, before, last: at, moved: false };
        // The light jumps to the pen straight away.
        if matches!(d.kind, DragKind::Light | DragKind::Light2) && pose_ui::step(&mut self.scene, &d.kind, self.canvas, at, at) {
            d.moved = true;
            self.generation += 1;
        }
        self.drag = Some(d);
    }

    pub fn drag_to(&mut self, at: [f32; 2]) {
        let Some(mut d) = self.drag.take() else { return };
        if matches!(d.kind, DragKind::PhotoPoint(_)) {
            // The dot follows the pen; refitting waits for the release.
            d.moved |= at != d.last;
        } else if pose_ui::step(&mut self.scene, &d.kind, self.canvas, d.last, at) {
            d.moved = true;
            self.generation += 1;
        }
        d.last = at;
        self.drag = Some(d);
    }

    pub fn release(&mut self, at: [f32; 2]) {
        let Some(mut d) = self.drag.take() else { return };
        if let DragKind::PhotoPoint(point) = d.kind {
            // A tap without a move leaves the match as it is.
            if d.moved || at != d.last {
                let to = s3d::glam::Vec2::new(at[0], at[1]);
                self.refit(|m| {
                    m.moved.insert(point, to);
                });
            }
            return;
        }
        d.moved |= pose_ui::step(&mut self.scene, &d.kind, self.canvas, d.last, at);
        // Watching a fall: the camera moves with it, and Keep makes the one undo step.
        if d.moved && self.gravity.fall.is_none() {
            self.history.push(d.before);
            self.changed();
        }
    }

    fn canvas_ui(&mut self, ui: &mut egui::Ui) {
        let area = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(area, egui::Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        let (size, xf) = self.layout(area, ppp);
        self.canvas = size;
        if !self.framed {
            s3d::frame(&mut self.scene, size.0, size.1, 0.82);
            self.framed = true;
        }
        let reach = REACH / xf.scale;
        let (multi, mods, any_down) = ui.input(|i| (i.multi_touch(), i.modifiers, i.pointer.any_down()));

        // Two fingers: pan and pinch-zoom the camera (one undo step), nothing else until they lift.
        if let Some(mt) = multi.filter(|m| m.num_touches >= 2) {
            if !self.gesture {
                self.gesture = true;
                let before = match self.drag.take() {
                    Some(d) => {
                        self.scene = d.before.clone();
                        d.before
                    }
                    None => self.scene.clone(),
                };
                self.history.push(before);
            }
            let (w, h) = size;
            let view = camera::View::new(&self.scene.camera, w, h);
            let d = mt.translation_delta / xf.scale;
            camera::pan(&mut self.scene.camera, s3d::glam::Vec2::new(d.x, d.y), &view);
            camera::dolly(&mut self.scene.camera, mt.zoom_delta);
            self.changed();
        } else if self.gesture && !any_down {
            self.gesture = false;
        }

        if !self.gesture {
            if resp.drag_started()
                && let Some(p) = ui.input(|i| i.pointer.press_origin())
            {
                self.press(xf.to_canvas(p), mods, reach);
            }
            if resp.dragged()
                && let Some(p) = resp.interact_pointer_pos()
            {
                self.drag_to(xf.to_canvas(p));
            }
            if resp.drag_stopped() {
                let at = resp.interact_pointer_pos().map(|p| xf.to_canvas(p)).or(self.drag.as_ref().map(|d| d.last));
                if let Some(at) = at {
                    self.release(at);
                }
            }
            if resp.clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                // A tap: picks the part under it (and places the light in Light mode).
                let at = xf.to_canvas(p);
                self.press(at, mods, reach);
                self.release(at);
            }
            if resp.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    camera::dolly(&mut self.scene.camera, (scroll * 0.002).exp());
                    self.changed();
                }
            }
        }

        // Draw: paper or photo, the figures, then the handles.
        let painter = ui.painter_at(area);
        let canvas_rect = Rect::from_min_max(xf.to_screen([0.0, 0.0]), xf.to_screen([size.0 as f32, size.1 as f32]));
        let background = srgb(self.background);
        match &self.photo {
            Some(p) => {
                let tex = self.photo_tex.get_or_insert_with(|| {
                    ui.ctx().load_texture("photo", egui::ColorImage::from_rgba_unmultiplied(p.size, &p.rgba), egui::TextureOptions::LINEAR)
                });
                painter.rect_filled(canvas_rect, 0.0, background);
                painter.image(tex.id(), canvas_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE.gamma_multiply(self.photo_opacity));
            }
            None => {
                painter.rect_filled(canvas_rect, 0.0, background);
            }
        }
        let dragging = self.drag.is_some() || self.gesture || any_down;
        if let Some(shown) = self.view.image(ui.ctx(), &self.scene, size, dragging) {
            let r = shown.rect;
            let rect = Rect::from_min_max(xf.to_screen([r.x0 as f32, r.y0 as f32]), xf.to_screen([r.x1 as f32, r.y1 as f32]));
            painter.image(shown.texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        self.draw_overlay(&painter, xf);

        let hint = if TOUCH {
            "Drag a part to pose it · drag empty space to orbit · two fingers to pan and zoom"
        } else {
            "Drag a part to pose it · drag empty space to orbit · Shift: pan · Ctrl or scroll: zoom · Alt: twist"
        };
        // Readable on any background: dark text on a light one, light on a dark one.
        let light_bg = self.background[0] * 0.3 + self.background[1] * 0.59 + self.background[2] * 0.11 > 0.45;
        let hint_color = if light_bg { Color32::from_black_alpha(150) } else { Color32::from_white_alpha(170) };
        painter.text(area.left_bottom() + Vec2::new(10.0, -10.0), Align2::LEFT_BOTTOM, hint, FontId::proportional(12.0), hint_color);
        if let Some((text, _)) = &self.status {
            let pos = area.center_top() + Vec2::new(0.0, 14.0);
            let galley = painter.layout_no_wrap(text.clone(), FontId::proportional(15.0), Color32::WHITE);
            let bg = Rect::from_center_size(pos + Vec2::new(0.0, galley.size().y / 2.0), galley.size() + Vec2::new(20.0, 10.0));
            painter.rect_filled(bg, 6.0, Color32::from_black_alpha(190));
            painter.galley(bg.min + Vec2::new(10.0, 5.0), galley, Color32::WHITE);
        }
    }

    /// The selected part (its bone and pivot), dots on the main joints, the Joints handles, the
    /// light disc, and the photo match's joints.
    fn draw_overlay(&self, painter: &egui::Painter, xf: Xform) {
        let (w, h) = self.canvas;
        let scene = &self.scene;
        let to_screen = |p: [f32; 2]| xf.to_screen(p);
        // Match photo: the photo's joints as the match read them, to drag where they belong.
        if let Some(m) = self.shown_match() {
            let mut points = m.points();
            let held = self.drag.as_ref().and_then(|d| if let DragKind::PhotoPoint(i) = d.kind { Some((i, d.last)) } else { None });
            if let Some((i, at)) = held
                && let Some(p) = points.get_mut(i)
            {
                *p = s3d::glam::Vec2::new(at[0], at[1]);
            }
            pose_ui::draw_photo_points(painter, &points, &|i| m.moved.contains_key(&i), held.map(|(i, _)| i), &to_screen);
            return;
        }
        self.draw_pins(painter, &to_screen);
        if self.mode == DragMode::Light {
            pose_ui::draw_light(painter, scene, (w, h), &to_screen);
            return;
        }
        pose_ui::draw_joint_dots(painter, scene, (w, h), &to_screen);
        if self.joints {
            let held = self.drag.as_ref().and_then(|d| if let DragKind::Reach(f, j) = d.kind { Some((f, j)) } else { None });
            pose_ui::draw_reach_handles(painter, scene, (w, h), &to_screen, held);
        }
        if let Some(sel) = self.selected {
            pose_ui::draw_selected(painter, scene, (w, h), &to_screen, sel, (ACCENT, Color32::from_gray(30)));
        }
    }
}

/// What goes under the figures in an exported PNG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Under {
    Nothing,
    Background,
    Photo,
}

fn srgb(c: [f32; 3]) -> Color32 {
    let [r, g, b] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
    Color32::from_rgb(r, g, b)
}

/// The dock's view of the app: each panel's body.
struct Panels<'a> {
    app: &'a mut App,
}

impl craft_ui::docking::DockContent<PanelId> for Panels<'_> {
    fn body(&mut self, ui: &mut egui::Ui, panel: &PanelId) {
        if *panel == PanelId::Canvas {
            self.app.canvas_ui(ui);
            return;
        }
        egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(ui, |ui| match panel {
            PanelId::Poses => self.app.pose_grid(ui),
            PanelId::Hands => {
                egui::ScrollArea::vertical().id_salt("hands").auto_shrink([false, true]).show(ui, |ui| self.app.hands_panel(ui));
            }
            PanelId::Photo => {
                egui::ScrollArea::vertical().id_salt("match-photo").auto_shrink([false, true]).show(ui, |ui| self.app.photo_tab(ui));
            }
            PanelId::Figures => {
                egui::ScrollArea::vertical().id_salt("figures").auto_shrink([false, true]).show(ui, |ui| self.app.figures_tab(ui));
            }
            PanelId::Light => {
                egui::ScrollArea::vertical().id_salt("light").auto_shrink([false, true]).show(ui, |ui| self.app.light_menu(ui));
            }
            PanelId::Gravity => {
                egui::ScrollArea::vertical().id_salt("gravity").auto_shrink([false, true]).show(ui, |ui| self.app.gravity_panel(ui));
            }
            PanelId::Canvas => {}
        });
    }

    fn logical_label(&self, panel: &PanelId) -> String {
        panel.label().to_string()
    }
}

use pose_ui::THUMB;
/// Pose thumbnails rendered per frame (each takes a few milliseconds).
const THUMBS_PER_FRAME: usize = 3;

const HELP: &str = "Drag a body part to pose it; drag the hips to move the figure. Twist (or Alt) turns a part about its own bone. Joints shows handles to drag a hand, foot or the head to where it should be.\n\nThe 3D panel has a library of poses, hand poses, and Match photo: open a photo and pose a figure like the person in it.\n\nSave keeps a .pirapose scene; Export PNG writes the figures as a picture.";

fn style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    if TOUCH {
        ctx.global_style_mut(|s| {
            s.spacing.interact_size = Vec2::new(44.0, 34.0);
            s.spacing.button_padding = Vec2::new(10.0, 7.0);
            s.spacing.item_spacing = Vec2::new(10.0, 8.0);
            s.spacing.slider_width = 150.0;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::Nowhere;

    fn app() -> App {
        let mut a = App::with_files(Box::<Nowhere>::default(), None);
        a.canvas = (900, 1200);
        s3d::frame(&mut a.scene, 900, 1200, 0.82);
        a.framed = true;
        a
    }

    /// The middle of a part's bone on the canvas, where a tap pins it.
    fn part_px(a: &App, joint: &str) -> [f32; 2] {
        let j = pose::joint_index(&a.scene.figures[0], joint).unwrap();
        let (pivot, tip) = pose::joint_px(&a.scene, 900, 1200, 0, j).unwrap();
        [(pivot[0] + tip[0]) / 2.0, (pivot[1] + tip[1]) / 2.0]
    }

    #[test]
    fn a_pinned_hand_drops_hangs_and_keeps_in_one_undo_step() {
        let mut a = app();
        let start = a.scene.clone();
        // Pin mode: a tap pins the part, a second tap unpins it.
        a.mode = DragMode::Pin;
        let at = part_px(&a, "thigh.L");
        a.press(at, egui::Modifiers::NONE, 20.0);
        a.release(at);
        assert_eq!(a.gravity.pins, vec![(0, "thigh.L".to_string())]);
        a.press(at, egui::Modifiers::NONE, 20.0);
        a.release(at);
        assert!(a.gravity.pins.is_empty());
        a.press(at, egui::Modifiers::NONE, 20.0);
        a.release(at);
        assert_eq!(a.scene, start, "pinning changes nothing in the scene");
        a.drop_figure();
        assert_eq!(a.mode, DragMode::Pose, "dropping leaves Pin mode");
        // While it shows, a drag on the figure moves the camera, not the figure.
        let p = a.gravity.fall.as_mut().unwrap();
        p.playing = false;
        p.time = p.fall.seconds();
        a.show_frame();
        let fallen = a.scene.figures[0].clone();
        assert_ne!(fallen, start.figures[0]);
        let at = part_px(&a, "chest");
        a.press(at, egui::Modifiers::NONE, 20.0);
        a.drag_to([at[0] + 80.0, at[1]]);
        a.release([at[0] + 80.0, at[1]]);
        assert_eq!(a.scene.figures[0], fallen);
        assert_ne!(a.scene.camera, start.camera);
        a.keep_fall();
        assert!(a.gravity.fall.is_none());
        a.undo();
        assert_eq!(a.scene.figures[0], start.figures[0], "one undo step back to standing");
    }

    #[test]
    fn cancel_and_undo_put_the_figure_back() {
        let mut a = app();
        let start = a.scene.figures[0].clone();
        a.drop_figure();
        a.gravity.fall.as_mut().unwrap().time = 1.0;
        a.show_frame();
        assert_ne!(a.scene.figures[0], start);
        a.cancel_fall();
        assert_eq!(a.scene.figures[0], start);
        a.drop_figure();
        a.gravity.fall.as_mut().unwrap().time = 1.0;
        a.show_frame();
        a.undo();
        assert_eq!(a.scene.figures[0], start);
        assert!(!a.history.can_undo(), "nothing to undo was added");
    }

    /// The Look menu's colours are picked in place: a press in the picker changes the colour and
    /// starts one undo step, with no popup that would close the menu.
    #[test]
    fn the_menu_colour_picker_works_in_place() {
        let ctx = egui::Context::default();
        let frame = |events: Vec<egui::Event>| {
            let mut out = (egui::Rect::NOTHING, None);
            let _ = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
                let r = ui.scope(|ui| inline_color(ui, [0.80, 0.64, 0.47]));
                out = (r.response.rect, r.inner);
            });
            out
        };
        let (rect, none) = frame(Vec::new());
        assert!(none.is_none() && rect.width() > 50.0, "{rect:?}");
        // The saturation-value square, at the bottom (the top row only shows the colour).
        let at = egui::pos2(rect.left() + rect.width() * 0.2, rect.bottom() - 20.0);
        let press = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE };
        let (_, picked) = frame(vec![egui::Event::PointerMoved(at), press]);
        let (c, start) = picked.expect("the press picks a colour");
        assert!(start, "and starts an undo step");
        assert_ne!(c, [0.80, 0.64, 0.47]);
    }

    #[test]
    fn dragging_a_forearm_poses_it_in_one_undo_step() {
        let mut a = app();
        let start = a.scene.clone();
        // Grab the left forearm halfway along and swing it.
        let fig = &a.scene.figures[0];
        let j = pose::joint_index(fig, "forearm.L").unwrap();
        let (pivot, tip) = pose::joint_px(&a.scene, 900, 1200, 0, j).unwrap();
        let p = [(pivot[0] + tip[0]) / 2.0, (pivot[1] + tip[1]) / 2.0];
        a.press(p, egui::Modifiers::NONE, 20.0);
        assert!(matches!(a.drag.as_ref().unwrap().kind, DragKind::Swing(0, _)), "{:?}", a.drag.as_ref().map(|d| &d.kind));
        for k in 1..=10 {
            a.drag_to([p[0] + 6.0 * k as f32, p[1] - 12.0 * k as f32]);
        }
        a.release([p[0] + 60.0, p[1] - 120.0]);
        assert_ne!(a.scene, start);
        a.undo();
        assert_eq!(a.scene, start);
    }

    #[test]
    fn dragging_empty_canvas_orbits_and_poses_apply() {
        let mut a = app();
        let yaw = a.scene.camera.yaw;
        a.press([5.0, 5.0], egui::Modifiers::NONE, 20.0);
        a.drag_to([105.0, 5.0]);
        a.release([105.0, 5.0]);
        assert_ne!(a.scene.camera.yaw, yaw);
        let id = library::entries().into_iter().find(|e| !e.hands_only && e.name != "Standing").unwrap().id;
        let before = a.scene.figures[0].clone();
        a.apply_pose(&id, pose::Sides::Both);
        assert_ne!(a.scene.figures[0], before);
        a.add_figure(Body3d::Female);
        assert_eq!(a.scene.figures.len(), 2);
        assert_eq!(a.figure(), 1);
        a.remove_figure(1);
        assert_eq!(a.scene.figures.len(), 1);
    }

    #[test]
    fn a_saved_scene_opens_and_autosave_restores_it() {
        let dir = std::env::temp_dir().join(format!("pira-pose-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("autosave.pirapose");
        let mut a = App::with_files(Box::<Nowhere>::default(), Some(path.clone()));
        a.add_figure(Body3d::Female);
        a.autosaved.1 = Instant::now() - Duration::from_secs(5);
        a.autosave_now();
        let b = App::with_files(Box::<Nowhere>::default(), Some(path));
        assert_eq!(b.scene, a.scene);
        let mut c = app();
        c.take_file(Purpose::Open, "y.pirapose", &scene::to_bytes(&a.scene, [0.1, 0.2, 0.3]), None);
        assert_eq!(c.scene.figures.len(), 2);
        assert_eq!(c.doc_name, "y");
        assert_eq!(c.background, [0.1, 0.2, 0.3], "the background comes with the scene");
        c.new_scene();
        assert_eq!(c.background, scene::DEFAULT_BACKGROUND, "a new scene is on neutral grey");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn exported_pictures_show_the_figures() {
        let mut s = app().scene;
        s3d::frame(&mut s, 90, 120, 0.8);
        let px = view::flatten(&s, (90, 120), None);
        assert_eq!(px.len(), 90 * 120 * 4);
        assert!(px.chunks(4).any(|p| p[3] > 200), "the figure shows");
        assert!(px.chunks(4).any(|p| p[3] == 0), "the background is clear");
    }

    #[test]
    fn a_photo_of_a_posed_figure_poses_another_like_it() {
        if !photo::AVAILABLE {
            return;
        }
        // The "photo": a mannequin in a T-pose, flattened over white.
        let mut src = Scene3d { figures: vec![s3d::new_figure(Body3d::Male, Some("T-pose"))], ..Scene3d::default() };
        s3d::frame(&mut src, 600, 800, 0.82);
        let mut rgba = vec![255u8; 600 * 800 * 4];
        let fig = view::flatten(&src, (600, 800), None);
        for (o, p) in rgba.chunks_mut(4).zip(fig.chunks(4)) {
            let a = f32::from(p[3]) / 255.0;
            for c in 0..3 {
                o[c] = (f32::from(p[c]) * a + 255.0 * (1.0 - a)) as u8;
            }
        }
        let found = match photo::track(&rgba, [600, 800], Part::Body, false) {
            Ok(f) => f,
            Err(e) => panic!("tracking failed: {e}"),
        };
        let mut scene = src.clone();
        library::apply(&mut scene.figures[0], "Standing", pose::Sides::Both).unwrap();
        let (matched, m) = photo::apply(&mut scene, 0, Part::Body, found).unwrap();
        assert_eq!(matched, vec!["body"]);
        assert!(m.is_some());
    }
}
