//! PIRA's first-launch defaults. The app shell calls [`first_launch`] once, on a fresh install
//! (the Android app keeps a marker file for it), right after creating the app: the PIRA theme,
//! the painting layout and a pressure- and tilt-ready pencil. Nothing here runs on later
//! launches, so the user's own choices always win, and the PhotoCraft defaults the tests rely
//! on stay as they are.

use serde_json::json;

use crate::PhotocraftApp;

/// Apply PIRA's defaults over a freshly created app. Every step is best effort: a failure keeps
/// that part at its ordinary default and is noted in the status bar.
pub fn first_launch(app: &mut PhotocraftApp) {
    for (path, value) in [("interface.darkTheme", "pira"), ("interface.lightTheme", "piraLight"), ("interface.appearanceMode", "dark")] {
        if let Err(e) = app.run("prefs.set", json!({ "path": path, "value": value })) {
            app.ui.status = e;
        }
    }
    crate::dock::first_launch_layout(app);
    crate::last_brush::first_launch_brush(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_engine::prefs::{DarkTheme, LightTheme};

    #[test]
    fn a_fresh_install_starts_on_the_pira_theme_layout_and_pencil() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        first_launch(&mut app);
        let i = &app.session.prefs().interface;
        assert_eq!(i.dark_theme, DarkTheme::Pira);
        assert_eq!(i.light_theme, LightTheme::PiraLight);
        assert_eq!(app.ui.workspace, "Painting");
        assert!(app.ui.panels.brushes && app.ui.panels.color && app.ui.panels.layers && !app.ui.panels.properties);
        assert_eq!(app.session.tools.current_preset.as_deref(), Some(photocraft_paint::presets::FIRST_BRUSH));
        assert!(app.session.tools.brush.tilt_shading.enabled);
    }
}
