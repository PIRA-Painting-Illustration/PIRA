//! Posing on the canvas: which part is under the pen, and turning parts by dragging.
//!
//! Positions are canvas pixels; the canvas is `width` × `height`.

use crate::camera::{View, ray_capsule};
use crate::skeleton::{Pose, Skeleton};
use crate::{figure_joints, finite_or, model, placement};
use glam::{Quat, Vec2, Vec3};
use photocraft_doc::{Figure3d, Scene3d};

/// Which hands a hand pose changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sides {
    Both,
    Left,
    Right,
}

impl Sides {
    pub fn suffixes(self) -> &'static [&'static str] {
        match self {
            Sides::Both => &[".L", ".R"],
            Sides::Left => &[".L"],
            Sides::Right => &[".R"],
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "both" => Some(Sides::Both),
            "left" => Some(Sides::Left),
            "right" => Some(Sides::Right),
            _ => None,
        }
    }
}

/// A part under the pen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub figure: usize,
    pub joint: usize,
    /// Distance from the camera (metres).
    pub distance: f32,
}

/// The nearest part of a visible figure under canvas point `px`.
pub fn pick(scene: &Scene3d, width: u32, height: u32, px: [f32; 2]) -> Option<Hit> {
    let view = View::new(&scene.camera, width, height);
    let (o, d) = view.ray(Vec2::from_array(px));
    let mut best: Option<Hit> = None;
    for (fi, fig) in scene.figures.iter().enumerate() {
        if !fig.visible {
            continue;
        }
        let (skel, mesh) = model(fig.body);
        let joints = figure_joints(skel, fig);
        let scale = finite_or(fig.scale, 1.0).clamp(0.05, 20.0);
        for h in &mesh.hits {
            let Some(m) = joints.get(h.joint) else { continue };
            let Some(t) = ray_capsule(o, d, m.transform_point3(h.a), m.transform_point3(h.b), h.radius * scale) else { continue };
            if best.is_none_or(|b| t < b.distance) {
                best = Some(Hit { figure: fi, joint: h.joint, distance: t });
            }
        }
    }
    best
}

/// The joint's pivot and the tip of its bone, on the canvas.
pub fn joint_px(scene: &Scene3d, width: u32, height: u32, figure: usize, joint: usize) -> Option<([f32; 2], [f32; 2])> {
    let fig = scene.figures.get(figure)?;
    let (skel, _) = model(fig.body);
    let m = *figure_joints(skel, fig).get(joint)?;
    let tail = skel.joints.get(joint)?.tail;
    let view = View::new(&scene.camera, width, height);
    let a = view.project(m.w_axis.truncate())?.px;
    let b = view.project(m.transform_point3(tail))?.px;
    Some((a.to_array(), b.to_array()))
}

/// The joint's name (`upper_arm.L`, …).
pub fn joint_name(fig: &Figure3d, joint: usize) -> Option<&'static str> {
    model(fig.body).0.joints.get(joint).map(|j| j.name.as_str())
}

/// The joint with this name.
pub fn joint_index(fig: &Figure3d, name: &str) -> Option<usize> {
    model(fig.body).0.find(name)
}

/// The user-facing name of a joint: "Left upper arm", "Right index finger 2", "Hips".
pub fn joint_label(name: &str) -> String {
    let (base, side) = match name.rsplit_once('.') {
        Some((b, "L")) => (b, "Left "),
        Some((b, "R")) => (b, "Right "),
        _ => (name, ""),
    };
    let digit = base.trim_end_matches(|c: char| c.is_ascii_digit());
    let num = &base[digit.len()..];
    let part = match digit {
        "upper_arm" => "upper arm".to_string(),
        "thumb" | "index" | "middle" | "ring" | "pinky" if !num.is_empty() => {
            let finger = if digit == "thumb" { "thumb".to_string() } else { format!("{digit} finger") };
            format!("{finger} {num}")
        }
        other => other.replace('_', " "),
    };
    let s = format!("{side}{part}");
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Turns `joint` by `turn` (a rotation in the scene's frame) about its pivot.
fn rotate_world(skel: &Skeleton, fig: &mut Figure3d, joint: usize, turn: Quat) -> bool {
    let mut pose = Pose::of(skel, fig);
    let place = Quat::from_rotation_y(finite_or(fig.yaw, 0.0));
    let world = pose.world_rot(skel);
    let parent = skel.joints.get(joint).and_then(|j| j.parent).and_then(|p| world.get(p).copied()).unwrap_or(Quat::IDENTITY);
    let parent = (place * parent).normalize();
    let Some(local) = pose.local.get_mut(joint) else { return false };
    let next = (parent.inverse() * turn * parent * *local).normalize();
    if !next.is_finite() {
        return false;
    }
    *local = next;
    pose.write_to(skel, fig);
    true
}

/// Swings `joint` in the canvas plane, as if its bone were dragged from `from` to `to` around
/// its pivot. Returns false when nothing changed.
pub fn swing(scene: &mut Scene3d, width: u32, height: u32, figure: usize, joint: usize, from: [f32; 2], to: [f32; 2]) -> bool {
    let Some((pivot, _)) = joint_px(scene, width, height, figure, joint) else { return false };
    let view = View::new(&scene.camera, width, height);
    let (p, a, b) = (Vec2::from_array(pivot), Vec2::from_array(from) - Vec2::from_array(pivot), Vec2::from_array(to) - Vec2::from_array(pivot));
    // Too close to the pivot to tell a direction.
    if a.length() < 2.0 || b.length() < 2.0 || !p.is_finite() {
        return false;
    }
    let angle = a.angle_to(b);
    if !angle.is_finite() || angle.abs() < 1e-5 {
        return false;
    }
    let (_, _, fwd) = view.axes();
    let Some(fig) = scene.figures.get_mut(figure) else { return false };
    let (skel, _) = model(fig.body);
    rotate_world(skel, fig, joint, Quat::from_axis_angle(fwd, angle))
}

/// Twists `joint` about its own bone by `angle` radians.
pub fn twist(fig: &mut Figure3d, joint: usize, angle: f32) -> bool {
    let (skel, _) = model(fig.body);
    let Some(j) = skel.joints.get(joint) else { return false };
    let m = figure_joints(skel, fig).get(joint).copied().unwrap_or_default();
    let axis = m.transform_vector3(j.tail).normalize_or(Vec3::Y);
    angle.is_finite() && rotate_world(skel, fig, joint, Quat::from_axis_angle(axis, angle))
}

/// Turns `joint` by `degrees` about its parent's x, y or z axis (a precise edit from a field).
pub fn turn_local(fig: &mut Figure3d, joint: usize, axis: char, degrees: f32) -> bool {
    let (skel, _) = model(fig.body);
    let mut pose = Pose::of(skel, fig);
    let Some(q) = pose.local.get_mut(joint) else { return false };
    let r = finite_or(degrees, 0.0).to_radians();
    let step = match axis {
        'x' => Quat::from_rotation_x(r),
        'y' => Quat::from_rotation_y(r),
        'z' => Quat::from_rotation_z(r),
        _ => return false,
    };
    *q = (step * *q).normalize();
    pose.write_to(skel, fig);
    true
}

/// Slides a figure across its floor as the pen moves from `from` to `to`.
pub fn slide(scene: &mut Scene3d, width: u32, height: u32, figure: usize, from: [f32; 2], to: [f32; 2]) -> bool {
    let view = View::new(&scene.camera, width, height);
    let Some(fig) = scene.figures.get(figure) else { return false };
    let (skel, _) = model(fig.body);
    let hips = figure_joints(skel, fig).first().map_or(Vec3::from_array(fig.position), |m| m.w_axis.truncate());
    let on_plane = |px: [f32; 2]| {
        let (o, d) = view.ray(Vec2::from_array(px));
        // A horizontal plane through the hips; seen edge-on, a vertical one facing the camera.
        if d.y.abs() > 0.08 {
            let t = (hips.y - o.y) / d.y;
            (t > 0.0).then(|| o + d * t)
        } else {
            let n = Vec3::new(d.x, 0.0, d.z).normalize_or(Vec3::Z);
            let t = (hips - o).dot(n) / d.dot(n);
            (t > 0.0).then(|| o + d * t)
        }
    };
    let (Some(a), Some(b)) = (on_plane(from), on_plane(to)) else { return false };
    let delta = b - a;
    if !delta.is_finite() || delta.length() > 100.0 {
        return false;
    }
    let Some(fig) = scene.figures.get_mut(figure) else { return false };
    fig.position[0] += delta.x;
    fig.position[2] += delta.z;
    true
}

/// Copies one side's pose onto the other, mirrored.
pub fn mirror(fig: &mut Figure3d, from_left: bool) {
    let (skel, _) = model(fig.body);
    let mut pose = Pose::of(skel, fig);
    pose.mirror(skel, from_left);
    pose.write_to(skel, fig);
}

/// Swaps the two sides' poses (a mirror image of the whole pose).
pub fn flip(fig: &mut Figure3d) {
    let (skel, _) = model(fig.body);
    let pose = Pose::of(skel, fig);
    let mut out = pose.clone();
    for i in 0..skel.joints.len() {
        let o = skel.mirror_of(i);
        if let (Some(slot), Some(q)) = (out.local.get_mut(o), pose.local.get(i)) {
            *slot = crate::skeleton::mirror_quat(*q);
        }
    }
    out.root.x = -out.root.x;
    out.write_to(skel, fig);
}

/// Sets a figure down so its lowest point rests on its floor.
pub fn place_on_floor(fig: &mut Figure3d) {
    let (skel, _) = model(fig.body);
    let mut pose = Pose::of(skel, fig);
    pose.ground(skel);
    pose.write_to(skel, fig);
}

/// Puts one joint back at rest.
pub fn reset_joint(fig: &mut Figure3d, joint: usize) {
    if let Some(name) = joint_name(fig, joint) {
        fig.joints.remove(name);
    }
}

/// The figure's facing on the floor, from its placement.
pub fn facing(fig: &Figure3d) -> Vec3 {
    placement(fig).transform_vector3(Vec3::Z).normalize_or(Vec3::Z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_figure;
    use photocraft_doc::Body3d;

    fn scene() -> Scene3d {
        let mut s = Scene3d { figures: vec![new_figure(Body3d::Male, Some("T-pose"))], ..Scene3d::default() };
        s.camera.yaw = 0.0;
        s.camera.pitch = 0.0;
        crate::frame(&mut s, 800, 1000, 0.8);
        s
    }

    #[test]
    fn picking_the_head_finds_the_head() {
        let s = scene();
        let head = joint_index(&s.figures[0], "head").unwrap();
        let (pivot, tip) = joint_px(&s, 800, 1000, 0, head).unwrap();
        let mid = [(pivot[0] + tip[0]) / 2.0, (pivot[1] + tip[1]) / 2.0];
        assert_eq!(pick(&s, 800, 1000, mid).map(|h| h.joint), Some(head));
        assert_eq!(pick(&s, 800, 1000, [5.0, 5.0]), None);
    }

    #[test]
    fn swinging_an_arm_moves_its_hand_towards_the_pen() {
        let mut s = scene();
        let arm = joint_index(&s.figures[0], "upper_arm.L").unwrap();
        let (pivot, tip) = joint_px(&s, 800, 1000, 0, arm).unwrap();
        // Drag the elbow straight down.
        let to = [pivot[0], pivot[1] + (tip[0] - pivot[0]).abs()];
        assert!(swing(&mut s, 800, 1000, 0, arm, tip, to));
        let (_, after) = joint_px(&s, 800, 1000, 0, arm).unwrap();
        assert!((after[0] - to[0]).abs() < 3.0 && (after[1] - to[1]).abs() < 3.0, "{after:?} vs {to:?}");
    }

    #[test]
    fn mirror_and_flip_are_symmetric() {
        let mut fig = new_figure(Body3d::Female, Some("Walking"));
        mirror(&mut fig, true);
        let l = fig.joints.get("thigh.L").copied().unwrap();
        let r = fig.joints.get("thigh.R").copied().unwrap();
        assert!((l[0] - r[0]).abs() < 1e-5 && (l[1] + r[1]).abs() < 1e-5 && (l[3] - r[3]).abs() < 1e-5);
        let before = fig.clone();
        flip(&mut fig);
        flip(&mut fig);
        for (k, v) in &before.joints {
            let w = fig.joints.get(k).unwrap();
            assert!(v.iter().zip(w).all(|(a, b)| (a - b).abs() < 1e-4), "{k}");
        }
    }

    #[test]
    fn sliding_moves_the_figure_along_the_floor() {
        let mut s = scene();
        assert!(slide(&mut s, 800, 1000, 0, [400.0, 500.0], [500.0, 500.0]));
        assert!(s.figures[0].position[0] > 0.05);
        assert!(s.figures[0].position[1].abs() < 1e-6);
    }

    #[test]
    fn joint_labels_read_naturally() {
        assert_eq!(joint_label("upper_arm.L"), "Left upper arm");
        assert_eq!(joint_label("index2.R"), "Right index finger 2");
        assert_eq!(joint_label("hips"), "Hips");
    }
}
