//! PIRA Pose: PIRA's posable 3D mannequins as an app of their own, for Android tablets and Windows.
//!
//! It poses with PIRA's own crates: `photocraft-scene3d` (mannequins, poses, renderer),
//! `photocraft-scene3d-egui` (the Object tool's drags, handles and overlays, shared with PIRA's
//! `ui-egui`) and `photocraft-posetrack` (photo matching). A change to posing in those crates
//! reaches both apps.

pub mod app;
pub mod dock;
pub mod files;
pub mod photo;
pub mod scene;
pub mod view;

#[cfg(target_os = "android")]
mod android;

pub use app::App;

/// Runs the desktop app (Windows, Linux, macOS).
#[cfg(not(target_os = "android"))]
pub fn run_desktop() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_title("PIRA Pose").with_inner_size([1400.0, 900.0]).with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native("PIRA Pose", options, Box::new(|cc| Ok(Box::new(Shell(App::new(cc, Box::<files::Desktop>::default(), None, dock::desktop_file()))))))
}

/// eframe's app trait, kept apart so the app can be built without a window in tests.
pub struct Shell(pub App);

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.0.frame(ui);
    }
}
