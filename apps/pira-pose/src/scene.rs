//! The `.pirapose` file (a scene of posed figures, as in a PIRA 3D layer, and the canvas's
//! background colour) and undo history.

use photocraft_doc::Scene3d;
use serde::{Deserialize, Serialize};

pub const FILE_EXTENSION: &str = "pirapose";
const FORMAT: &str = "pira-pose";
const VERSION: u32 = 1;

/// The canvas colour a new scene gets: a neutral mid grey (sRGB #808080).
pub const DEFAULT_BACKGROUND: [f32; 3] = [128.0 / 255.0; 3];

#[derive(Serialize, Deserialize)]
struct File {
    format: String,
    version: u32,
    scene: Scene3d,
    /// sRGB, 0–1. Files from before it was saved get [`DEFAULT_BACKGROUND`].
    #[serde(default = "default_background")]
    background: [f32; 3],
}

fn default_background() -> [f32; 3] {
    DEFAULT_BACKGROUND
}

pub fn to_bytes(scene: &Scene3d, background: [f32; 3]) -> Vec<u8> {
    serde_json::to_vec_pretty(&File { format: FORMAT.into(), version: VERSION, scene: scene.clone(), background }).unwrap_or_default()
}

/// A `.pirapose` file (its scene and background), or a bare scene as PIRA's `scene3d.get` gives
/// it (on the default background).
pub fn from_bytes(bytes: &[u8]) -> Result<(Scene3d, [f32; 3]), String> {
    let (scene, background) = match serde_json::from_slice::<File>(bytes) {
        Ok(f) if f.format == FORMAT => {
            if f.version > VERSION {
                return Err(format!("this scene needs a newer PIRA Pose (file version {})", f.version));
            }
            (f.scene, f.background.map(|c| if c.is_finite() { c.clamp(0.0, 1.0) } else { 0.5 }))
        }
        _ => (serde_json::from_slice::<Scene3d>(bytes).map_err(|_| "not a PIRA Pose scene".to_string())?, DEFAULT_BACKGROUND),
    };
    if scene.figures.len() > 64 {
        return Err("a scene holds at most 64 figures".into());
    }
    Ok((scene, background))
}

/// Undo and redo: whole-scene snapshots (scenes are small).
pub struct History<T: Clone> {
    undo: Vec<T>,
    redo: Vec<T>,
}

impl<T: Clone> Default for History<T> {
    fn default() -> Self {
        History { undo: Vec::new(), redo: Vec::new() }
    }
}

impl<T: Clone> History<T> {
    const LIMIT: usize = 200;

    /// Remembers `state` (as it was before a change).
    pub fn push(&mut self, state: T) {
        self.undo.push(state);
        if self.undo.len() > Self::LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, current: T) -> Option<T> {
        let prev = self.undo.pop()?;
        self.redo.push(current);
        Some(prev)
    }

    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_doc::Body3d;
    use photocraft_scene3d as s3d;

    #[test]
    fn scenes_round_trip_through_the_file() {
        let scene = Scene3d { figures: vec![s3d::new_figure(Body3d::Female, Some("Walking")), s3d::new_figure(Body3d::Male, None)], ..Scene3d::default() };
        let bg = [0.2, 0.3, 0.4];
        assert_eq!(from_bytes(&to_bytes(&scene, bg)).unwrap(), (scene.clone(), bg));
        // A bare scene (from PIRA) opens too, on neutral grey.
        assert_eq!(from_bytes(&serde_json::to_vec(&scene).unwrap()).unwrap(), (scene.clone(), DEFAULT_BACKGROUND));
        // A file saved before the background was: neutral grey.
        let old = format!("{{\"format\":\"pira-pose\",\"version\":1,\"scene\":{}}}", serde_json::to_string(&scene).unwrap());
        assert_eq!(from_bytes(old.as_bytes()).unwrap().1, DEFAULT_BACKGROUND);
        assert!(from_bytes(b"{\"format\":\"pira-pose\",\"version\":99,\"scene\":{}}").is_err());
    }

    #[test]
    fn history_undoes_and_redoes() {
        let mut h = History::default();
        h.push(1);
        h.push(2);
        assert_eq!(h.undo(3), Some(2));
        assert_eq!(h.undo(2), Some(1));
        assert_eq!(h.undo(1), None);
        assert_eq!(h.redo(1), Some(2));
        h.push(5);
        assert!(!h.can_redo());
    }
}
