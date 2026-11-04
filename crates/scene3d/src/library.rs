//! Ready-made poses: a few built-in ones (standing, walking, hand shapes, …) and 1,072 poses
//! taken from the CMU Graphics Lab Motion Capture Database (see `ATTRIBUTION.md`).

use crate::pose::Sides;
use crate::presets::PRESETS;
use crate::skeleton::Pose;
use crate::{model, skeleton};
use photocraft_doc::Figure3d;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::OnceLock;

/// The motion-capture poses, as gzipped JSON.
static CMU: &[u8] = include_bytes!("../../../assets/poses/cmu-poses.json.gz");

/// One pose of the library.
#[derive(Clone, Debug, Deserialize)]
pub struct LibraryPose {
    pub id: String,
    pub name: String,
    pub category: String,
    /// Where it came from (take and frame).
    pub source: String,
    pub root: [f32; 3],
    pub joints: BTreeMap<String, [f32; 4]>,
}

/// A pose as listed in the 3D materials: built-in poses first, then the library.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub category: String,
    /// Only changes the hands.
    pub hands_only: bool,
}

/// The built-in poses' categories.
pub const BASIC: &str = "Basic";
pub const HANDS: &str = "Hands";

/// The motion-capture library (decoded on first use; empty if the bundled data were damaged).
pub fn library() -> &'static [LibraryPose] {
    static LIB: OnceLock<Vec<LibraryPose>> = OnceLock::new();
    LIB.get_or_init(|| {
        let mut json = String::new();
        if flate2::read::GzDecoder::new(CMU).read_to_string(&mut json).is_err() {
            return Vec::new();
        }
        serde_json::from_str(&json).unwrap_or_default()
    })
}

/// Every pose, built-in first.
pub fn entries() -> Vec<Entry> {
    let builtin = PRESETS.iter().map(|p| Entry {
        id: p.name.to_string(),
        name: p.name.to_string(),
        category: if p.hands_only { HANDS } else { BASIC }.to_string(),
        hands_only: p.hands_only,
    });
    let lib = library().iter().map(|p| Entry { id: p.id.clone(), name: p.name.clone(), category: p.category.clone(), hands_only: false });
    builtin.chain(lib).collect()
}

/// The categories in the order the materials list them.
pub fn categories() -> Vec<String> {
    let mut out = vec![BASIC.to_string(), HANDS.to_string()];
    for p in library() {
        if !out.contains(&p.category) {
            out.push(p.category.clone());
        }
    }
    out
}

fn is_hand(name: &str) -> bool {
    name.split('.').next().is_some_and(|b| skeleton::DIGITS.iter().any(|d| b.starts_with(d)))
}

/// Puts `fig` in the pose with this id (a built-in pose's name, or a library id such as
/// `cmu_05_06_00024`). Library poses keep the figure's finger positions (they record none); hand
/// poses change only the hands in `sides`. The figure is then set down on its floor.
pub fn apply(fig: &mut Figure3d, id: &str, sides: Sides) -> Result<(), String> {
    let (skel, _) = model(fig.body);
    let mut pose = Pose::of(skel, fig);
    if let Some(p) = PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(id)) {
        p.apply(skel, &mut pose, sides.suffixes());
    } else if let Some(p) = library().iter().find(|p| p.id == id) {
        let mut next = Pose::from_joints(skel, p.root, p.joints.iter().map(|(k, v)| (k.as_str(), *v)));
        for (i, j) in skel.joints.iter().enumerate() {
            if is_hand(&j.name)
                && let (Some(slot), Some(q)) = (next.local.get_mut(i), pose.local.get(i))
            {
                *slot = *q;
            }
        }
        next.ground(skel);
        pose = next;
    } else {
        return Err(format!("no pose \"{id}\""));
    }
    pose.write_to(skel, fig);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_doc::Body3d;

    #[test]
    fn the_library_decodes_every_pose() {
        assert_eq!(library().len(), 1072);
        assert!(entries().len() > 1072);
        assert!(categories().len() >= 10);
    }

    #[test]
    fn library_poses_keep_the_fingers() {
        let mut fig = crate::new_figure(Body3d::Male, Some("Fist"));
        let before = fig.joints.get("index2.L").copied();
        assert!(before.is_some());
        let id = library().first().map(|p| p.id.clone()).unwrap();
        apply(&mut fig, &id, Sides::Both).unwrap();
        assert_eq!(fig.joints.get("index2.L").copied(), before);
        assert!(apply(&mut fig, "no such pose", Sides::Both).is_err());
    }

    #[test]
    fn a_hand_pose_on_one_side_leaves_the_other() {
        let mut fig = crate::new_figure(Body3d::Female, Some("T-pose"));
        apply(&mut fig, "Fist", Sides::Right).unwrap();
        assert!(fig.joints.contains_key("index2.R"));
        assert!(!fig.joints.contains_key("index2.L"));
    }
}
