//! The window's panels, arranged with craft-ui's docking (as PIRA's are): every panel can be
//! dragged by its tab to another group (as a tab), beside it or under it, into a new column on
//! the left or right edge, or out to float; groups stack in dock columns; the canvas stays put.
//! The arrangement is saved and comes back on the next start.

use craft_ui::docking::{Column, Group, Layout, Node, PanelLimits, Permissions, Side, Workspace};
use craft_ui::layout::{SplitAxis, SplitSize};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A panel of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PanelId {
    /// The figures, on their canvas: always docked and open.
    Canvas,
    Poses,
    Hands,
    /// Match photo.
    Photo,
    Figures,
    Light,
}

impl PanelId {
    /// Every panel this build has (Match photo only where photo tracking is built in).
    pub fn all() -> Vec<PanelId> {
        let mut v = vec![PanelId::Canvas, PanelId::Poses, PanelId::Hands];
        if crate::photo::AVAILABLE {
            v.push(PanelId::Photo);
        }
        v.extend([PanelId::Figures, PanelId::Light]);
        v
    }

    pub fn label(self) -> &'static str {
        match self {
            PanelId::Canvas => "Canvas",
            PanelId::Poses => "Poses",
            PanelId::Hands => "Hands",
            PanelId::Photo => "Match photo",
            PanelId::Figures => "Figures",
            PanelId::Light => "Light",
        }
    }
}

/// The canvas stays docked and open and takes no tabs, but panels can split beside it.
pub fn permissions(panel: &PanelId) -> Permissions {
    if *panel == PanelId::Canvas { Permissions { accepts_splits: true, ..Permissions::PROTECTED } } else { Permissions::default() }
}

pub fn limits(panel: &PanelId) -> PanelLimits {
    match panel {
        PanelId::Canvas => PanelLimits { min: egui::vec2(240.0, 160.0), ..Default::default() },
        _ => PanelLimits { min: egui::vec2(220.0, 80.0), ..Default::default() },
    }
}

/// Width of the default dock column.
const COLUMN_WIDTH: f32 = 330.0;

/// The canvas, and at the right edge one dock column: the 3D panels as tabs, with Light below.
pub fn default_layout() -> Layout<PanelId> {
    let mut tabs = vec![PanelId::Poses, PanelId::Hands];
    if crate::photo::AVAILABLE {
        tabs.push(PanelId::Photo);
    }
    tabs.push(PanelId::Figures);
    let light = Group { height: Some(380.0), ..Group::new(vec![PanelId::Light], 0) };
    let dock = Node::Dock { side: Side::End, columns: vec![Column::new(vec![Group::new(tabs, 0), light], COLUMN_WIDTH)] };
    let root = Node::Split {
        axis: SplitAxis::Horizontal,
        size: SplitSize::FixedSecond(COLUMN_WIDTH),
        first: Box::new(Node::Tabs { panels: vec![PanelId::Canvas], active: 0 }),
        second: Box::new(dock),
    };
    Layout { root: Some(root), floating: Vec::new() }
}

pub fn default_workspace() -> Workspace<PanelId> {
    Workspace::new(default_layout())
}

/// The saved workspace at `path`, fitted to this build's panels; the default when there is none
/// or it can't be read.
pub fn load(path: Option<&Path>) -> Workspace<PanelId> {
    let default = default_workspace();
    let Some(text) = path.and_then(|p| std::fs::read(p).ok()) else { return default };
    let Ok(mut w) = serde_json::from_slice::<Workspace<PanelId>>(&text) else { return default };
    if w.version > Workspace::<PanelId>::VERSION || w.validate().is_err() {
        return default;
    }
    let all = PanelId::all();
    w.reconcile(|p| all.contains(p), permissions, &default);
    if w.validate().is_err() || !w.layout.panels().contains(&&PanelId::Canvas) {
        return default;
    }
    w
}

pub fn save(path: Option<&Path>, workspace: &Workspace<PanelId>) {
    let Some(path) = path else { return };
    let Ok(json) = serde_json::to_vec_pretty(workspace) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("tmp");
    if let Err(e) = std::fs::write(&tmp, json).and_then(|()| std::fs::rename(&tmp, path)) {
        log::warn!("saving the panel layout: {e}");
    }
}

/// True when `panel` is in the layout (docked or floating).
pub fn shown(layout: &Layout<PanelId>, panel: PanelId) -> bool {
    layout.panels().contains(&&panel)
}

/// True when `panel` is the open tab of its group (not behind another tab, minimized or closed).
pub fn in_front(layout: &Layout<PanelId>, panel: PanelId) -> bool {
    layout.root.as_ref().is_some_and(|n| front(n, panel)) || layout.floating.iter().any(|w| front(&w.root, panel))
}

fn front(node: &Node<PanelId>, panel: PanelId) -> bool {
    match node {
        Node::Tabs { panels, active } => panels.get(*active) == Some(&panel),
        Node::Split { first, second, .. } => front(first, panel) || front(second, panel),
        Node::Stack { entries } => entries.iter().any(|e| e.open && e.panel == panel),
        Node::Dock { columns, .. } => columns.iter().flat_map(|c| &c.groups).any(|g| !g.minimized && g.active_panel() == Some(&panel)),
        _ => false,
    }
}

/// Where the panel layout is kept on the desktop: the user's config folder.
#[cfg(not(target_os = "android"))]
pub fn desktop_file() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from).or_else(|| {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
    })?;
    Some(base.join("PIRA Pose").join("workspace.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_ui::docking::{Action, Placement};

    #[test]
    fn the_default_layout_is_valid_and_has_every_panel() {
        let w = default_workspace();
        assert!(w.validate().is_ok());
        for p in PanelId::all() {
            assert!(shown(&w.layout, p), "{p:?}");
        }
        assert!(in_front(&w.layout, PanelId::Poses) && in_front(&w.layout, PanelId::Light));
        assert!(!in_front(&w.layout, PanelId::Hands), "behind Poses");
    }

    #[test]
    fn panels_move_tab_and_float_and_the_canvas_stays() {
        let mut w = default_workspace();
        // Light joins the 3D group as a tab, then Figures floats.
        w.apply(Action::Move { panel: PanelId::Light, anchor: PanelId::Poses, placement: Placement::Tab { before: None } }, permissions).unwrap();
        assert!(w.validate().is_ok());
        w.apply(Action::Float { panel: PanelId::Figures, rect: [100.0, 100.0, 300.0, 400.0] }, permissions).unwrap();
        assert_eq!(w.layout.floating.len(), 1);
        // The canvas can't be closed or float.
        assert!(w.apply(Action::Close { panel: PanelId::Canvas }, permissions).is_err());
        assert!(shown(&w.layout, PanelId::Canvas));
        // Closing and showing a panel again.
        w.set_visible(PanelId::Hands, false, None, permissions).unwrap();
        assert!(!shown(&w.layout, PanelId::Hands));
        w.set_visible(PanelId::Hands, true, Some(PanelId::Poses), permissions).unwrap();
        assert!(shown(&w.layout, PanelId::Hands));
    }

    #[test]
    fn a_saved_layout_comes_back_and_a_broken_one_falls_back() {
        let dir = std::env::temp_dir().join(format!("pira-pose-dock-{}", std::process::id()));
        let path = dir.join("workspace.json");
        let mut w = default_workspace();
        w.apply(Action::Float { panel: PanelId::Light, rect: [50.0, 60.0, 280.0, 360.0] }, permissions).unwrap();
        save(Some(&path), &w);
        assert_eq!(load(Some(&path)).layout, w.layout);
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(load(Some(&path)).layout, default_layout());
        assert_eq!(load(None).layout, default_layout());
        let _ = std::fs::remove_dir_all(dir);
    }
}
