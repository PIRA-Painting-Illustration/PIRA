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
    studio_layout(app);
    crate::last_brush::first_launch_brush(app);
}

/// PIRA's studio layout, after the painting apps artists already know: the tools in two
/// columns with the colour chips under them, the size and opacity sliders beside them, the
/// brushes as an icon on the left edge, a full-height Layers panel on the right and the other
/// panels as icons beside it. A fresh install starts on it; the Android app switches an older
/// one once, after its saved layout has loaded.
pub fn studio_layout(app: &mut PhotocraftApp) {
    crate::dock::first_launch_layout(app);
    app.ui.panels.toolbar = true;
    app.ui.panels.toolbar_double = true;
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
        assert!(app.ui.panels.brushes && app.ui.panels.color && app.ui.panels.layers && app.ui.panels.properties);
        assert!(app.ui.panels.toolbar_double, "two tool columns");
        let layout = &app.ui.docking.as_ref().expect("PIRA's docking layout").layout;
        assert!(crate::panel_docking::valid(layout));
        // Brushes at the start edge; everything else at the end edge, Layers expanded and the
        // rest as icons.
        let column = |panel: &str| layout.location(&panel.to_owned()).ok().and_then(|at| at.group).and_then(|g| g.column);
        assert_eq!(column("brushes").map(|c| c.side), Some(craft_ui::docking::Side::Start));
        for panel in ["layers", "channels", "paths", "properties", "color", "swatches", "history", "navigator"] {
            assert_eq!(column(panel).map(|c| c.side), Some(craft_ui::docking::Side::End), "{panel}");
        }
        let iconic = |panel: &str| column(panel).map(|c| c.iconic);
        assert_eq!(iconic("layers"), Some(false), "Layers stays open");
        for panel in ["brushes", "properties", "color", "history"] {
            assert_eq!(iconic(panel), Some(true), "{panel} is an icon");
        }
        assert_eq!(app.session.tools.current_preset.as_deref(), Some(photocraft_paint::presets::FIRST_BRUSH));
        assert!(app.session.tools.brush.tilt_shading.enabled);
    }

    #[test]
    fn the_studio_layout_keeps_its_places_once_drawn() {
        use egui_kittest::Harness;
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
        studio_layout(&mut app);
        let mut h = Harness::builder().with_size(egui::vec2(1100.0, 720.0)).build_ui_state(
            |ui, app: &mut PhotocraftApp| {
                let ctx = ui.ctx().clone();
                if !ctx.fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                    return;
                }
                crate::panels::dock_area(app, ui);
                app.ui.status = "drawn".into();
            },
            app,
        );
        PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::Pira);
        // The first frame applies the saved theme, as the app does at launch.
        let ctx = h.ctx.clone();
        h.state_mut().apply_theme(&ctx, crate::theme::ThemeKind::Pira);
        h.run_steps(12);
        assert_eq!(h.state().ui.status, "drawn");
        // Saved and loaded again at the next launch.
        let saved = crate::dock::snapshot(h.state());
        crate::dock::apply(h.state_mut(), &saved);
        let layout = &h.state().ui.docking.as_ref().expect("still customized").layout;
        let column = |panel: &str| layout.location(&panel.to_owned()).ok().and_then(|at| at.group).and_then(|g| g.column);
        let brushes = column("brushes").expect("brushes docked");
        assert_eq!((brushes.side, brushes.iconic), (craft_ui::docking::Side::Start, true), "{layout:#?}");
        assert_eq!(column("layers").map(|c| c.iconic), Some(false));
    }
}
