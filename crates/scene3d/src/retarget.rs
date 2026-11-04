//! Poses a figure from tracked landmarks: MediaPipe's 33 body points or 21 hand points, in metres
//! with x right, y up and z towards the camera (as `photocraft-posetrack` gives them). Ported from
//! PIRA Pose.
//!
//! Each bone is aimed down the matching landmark segment with the smallest turn from its parent
//! (so no twist is guessed). Twist that can be read from the landmarks is set from a full frame:
//! the hips from the hip line, the chest from the shoulders, the head from the ears and nose.
//!
//! The landmarks are as the photo's camera saw them; [`match_body`] and [`match_hand`] turn the
//! result to face the 3D layer's camera the same way, so the figure looks like the photo from
//! where you're looking (upright: only the camera's turn is used, not its tilt).

use crate::skeleton::{Pose, Skeleton, mirror_quat};
use crate::{finite_or, model};
use glam::{Mat3, Quat, Vec3};
use photocraft_doc::{Camera3d, Figure3d};

/// The rotation whose X and Y axes point along `x` and (as near as possible) `y`.
fn frame(x: Vec3, y: Vec3) -> Option<Quat> {
    let x = x.try_normalize()?;
    let z = x.cross(y).try_normalize()?;
    let y = z.cross(x);
    Some(Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize())
}

/// Sets joint `j`'s local rotation so its bone points along `dir` (world), given its parent's
/// world rotation, and returns the joint's new world rotation.
fn aim(skel: &Skeleton, pose: &mut Pose, j: usize, parent: Quat, dir: Vec3) -> Quat {
    let Some(joint) = skel.joints.get(j) else { return parent };
    let rest = joint.tail.normalize_or(Vec3::Y);
    if let (Some(d), Some(slot)) = (dir.try_normalize(), pose.local.get_mut(j)) {
        *slot = Quat::from_rotation_arc(rest, parent.inverse() * d);
    }
    (parent * pose.local.get(j).copied().unwrap_or(Quat::IDENTITY)).normalize()
}

fn set(pose: &mut Pose, j: usize, q: Quat) {
    if let Some(slot) = pose.local.get_mut(j) {
        *slot = q;
    }
}

/// Splits a turn between two joints in a chain: the first takes half, the second the rest.
fn split(pose: &mut Pose, first: usize, second: usize, turn: Quat) {
    let half = Quat::IDENTITY.slerp(turn, 0.5);
    set(pose, first, half);
    set(pose, second, (half.inverse() * turn).normalize());
}

// MediaPipe body landmarks.
const NOSE: usize = 0;
const EAR: [usize; 2] = [7, 8];
const SHOULDER: [usize; 2] = [11, 12];
const ELBOW: [usize; 2] = [13, 14];
const WRIST: [usize; 2] = [15, 16];
const PINKY: [usize; 2] = [17, 18];
const INDEX: [usize; 2] = [19, 20];
const HIP: [usize; 2] = [23, 24];
const KNEE: [usize; 2] = [25, 26];
const ANKLE: [usize; 2] = [27, 28];
const TOE: [usize; 2] = [31, 32];

/// Body landmarks MediaPipe gives (and [`body`] needs).
pub const BODY_POINTS: usize = 33;
/// Hand landmarks MediaPipe gives (and [`hand`] needs).
pub const HAND_POINTS: usize = 21;

/// The head's nose-from-ears line sits about this far below level when looking straight ahead.
pub const NOSE_DROP: f32 = 0.3;

/// Poses the whole body from tracked landmarks (`world`, with each one's visibility 0–1), facing
/// the way they do (towards +Z is towards the camera). Limbs whose landmarks are hidden keep their
/// pose. False (and nothing changed) without the hips and shoulders.
pub fn body(skel: &Skeleton, pose: &mut Pose, world: &[Vec3], visible: &[f32]) -> bool {
    if world.len() < BODY_POINTS {
        return false;
    }
    let p = |i: usize| world.get(i).copied().unwrap_or(Vec3::ZERO);
    let seen = |ids: &[usize]| ids.iter().all(|&i| visible.get(i).copied().unwrap_or(0.0) > 0.5);
    let j = |name: &str| skel.find(name).unwrap_or(usize::MAX);
    let mid = |a: [usize; 2]| (p(a[0]) + p(a[1])) * 0.5;
    // Index 0 is the person's left, which faces the camera as our +X.
    let up = mid(SHOULDER) - mid(HIP);

    let Some(hips) = frame(p(HIP[0]) - p(HIP[1]), up) else { return false };
    set(pose, j("hips"), hips);
    let chest = frame(p(SHOULDER[0]) - p(SHOULDER[1]), up).unwrap_or(hips);
    split(pose, j("spine"), j("chest"), (hips.inverse() * chest).normalize());

    if seen(&[NOSE, EAR[0], EAR[1]]) {
        let ears = p(EAR[0]) - p(EAR[1]);
        let fwd = p(NOSE) - mid(EAR);
        if let Some(head) = frame(ears, fwd.cross(ears)) {
            let head = head * Quat::from_rotation_x(-NOSE_DROP);
            split(pose, j("neck"), j("head"), (chest.inverse() * head).normalize());
        }
    }

    for (side, s) in [(".L", 0), (".R", 1)] {
        let n = |b: &str| j(&format!("{b}{side}"));
        set(pose, n("clavicle"), Quat::IDENTITY);
        let mut rot = chest;
        if seen(&[SHOULDER[s], ELBOW[s]]) {
            rot = aim(skel, pose, n("upper_arm"), rot, p(ELBOW[s]) - p(SHOULDER[s]));
            if seen(&[WRIST[s]]) {
                rot = aim(skel, pose, n("forearm"), rot, p(WRIST[s]) - p(ELBOW[s]));
                if seen(&[INDEX[s], PINKY[s]]) {
                    let knuckles = (p(INDEX[s]) + p(PINKY[s])) * 0.5;
                    aim(skel, pose, n("hand"), rot, knuckles - p(WRIST[s]));
                }
            }
        }
        let mut rot = hips;
        if seen(&[HIP[s], KNEE[s]]) {
            rot = aim(skel, pose, n("thigh"), rot, p(KNEE[s]) - p(HIP[s]));
            if seen(&[ANKLE[s]]) {
                rot = aim(skel, pose, n("shin"), rot, p(ANKLE[s]) - p(KNEE[s]));
                if seen(&[TOE[s]]) {
                    aim(skel, pose, n("foot"), rot, p(TOE[s]) - p(ANKLE[s]));
                }
            }
        }
    }
    pose.ground(skel);
    true
}

/// Poses one hand's fingers from tracked hand landmarks. The landmarks are made left-handed first
/// (`is_right` mirrors them), the fingers are solved for the left hand, then mirrored onto the
/// right hand if that's the target. With `wrist`, the hand's angle is set too: `view` turns the
/// landmarks' axes into the figure's. False (and nothing changed) for a degenerate hand.
pub fn hand(skel: &Skeleton, pose: &mut Pose, world: &[Vec3], is_right: bool, to_right: bool, wrist: Option<Quat>) -> bool {
    if world.len() < HAND_POINTS {
        return false;
    }
    let flip = |v: Vec3| if is_right { Vec3::new(-v.x, v.y, v.z) } else { v };
    let p: Vec<Vec3> = world.iter().map(|v| flip(*v)).collect();
    let at = |i: usize| p.get(i).copied().unwrap_or(Vec3::ZERO);
    // Hand frame: fingers along +X, thumb side +Z, back of the hand +Y (left hand, palm down).
    let fwd = at(9) - at(0);
    let side = at(5) - at(17);
    let Some(raw) = frame(fwd, side.cross(fwd)) else { return false };
    // The same frame measured on the rest pose, so an open flat hand reads as no turn at all.
    let hand = (raw * rest_hand_frame(skel).inverse()).normalize();
    let mut solved = Pose::rest(skel);
    let j = |name: &str| skel.find(name).unwrap_or(usize::MAX);
    let chains: [(&str, [usize; 4]); 5] =
        [("thumb", [1, 2, 3, 4]), ("index", [5, 6, 7, 8]), ("middle", [9, 10, 11, 12]), ("ring", [13, 14, 15, 16]), ("pinky", [17, 18, 19, 20])];
    for (digit, ids) in chains {
        // Work in the hand's own frame, so the result doesn't depend on how the photo was held.
        let mut rot = Quat::IDENTITY;
        for k in 0..3 {
            let dir = hand.inverse() * (at(ids[k + 1]) - at(ids[k]));
            rot = aim(skel, &mut solved, j(&format!("{digit}{}.L", k + 1)), rot, dir);
        }
    }
    let to = if to_right { ".R" } else { ".L" };
    for (digit, _) in chains {
        for k in 1..=3 {
            let q = solved.local.get(j(&format!("{digit}{k}.L"))).copied().unwrap_or(Quat::IDENTITY);
            set(pose, j(&format!("{digit}{k}{to}")), if to_right { mirror_quat(q) } else { q });
        }
    }
    if let Some(view) = wrist {
        // Seen from the camera the hand's frame is a rotation in the landmarks' axes: turn it into
        // the figure's, then make it the hand joint's own.
        let hj = j(&format!("hand{to}"));
        let world_rot = pose.world_rot(skel);
        let parent = skel.joints.get(hj).and_then(|jt| jt.parent).and_then(|p| world_rot.get(p).copied()).unwrap_or(Quat::IDENTITY);
        // `hand` is the left-handed version: mirrored back for the right hand.
        let target = view * if to_right { mirror_quat(hand) } else { hand };
        set(pose, hj, (parent.inverse() * target).normalize());
    }
    true
}

fn rest_hand_frame(skel: &Skeleton) -> Quat {
    let w = Pose::rest(skel).world(skel);
    let at = |n: &str| skel.find(n).and_then(|i| w.get(i)).map_or(Vec3::ZERO, |m| m.transform_point3(Vec3::ZERO));
    let fwd = at("middle1.L") - at("hand.L");
    let side = at("index1.L") - at("pinky1.L");
    frame(fwd, side.cross(fwd)).unwrap_or(Quat::IDENTITY)
}

/// The turn from the photo camera's axes (x right, y up, z towards the camera) into `fig`'s own,
/// so a pose seen from the front in the photo is seen from the front by `camera`. Only the
/// camera's turn about the vertical is used, so the figure stays upright.
pub fn view_turn(camera: &Camera3d, fig: &Figure3d) -> Quat {
    Quat::from_rotation_y(finite_or(camera.yaw, 0.0) - finite_or(fig.yaw, 0.0))
}

/// Poses `fig`'s body from tracked body landmarks, facing `camera` as the person faced the photo's
/// camera. Its fingers and place are kept. False when the landmarks are unusable.
pub fn match_body(fig: &mut Figure3d, camera: &Camera3d, world: &[Vec3], visible: &[f32]) -> bool {
    let (skel, _) = model(fig.body);
    let mut pose = Pose::of(skel, fig);
    if !body(skel, &mut pose, world, visible) {
        return false;
    }
    if let Some(hips) = skel.find("hips").and_then(|i| pose.local.get_mut(i)) {
        *hips = (view_turn(camera, fig) * *hips).normalize();
    }
    pose.ground(skel);
    pose.write_to(skel, fig);
    true
}

/// Poses one of `fig`'s hands (`to_right`) from tracked hand landmarks of a left or right hand
/// (`is_right`). With `wrist`, the hand's angle is set as the photo shows it from `camera`.
pub fn match_hand(fig: &mut Figure3d, camera: &Camera3d, world: &[Vec3], is_right: bool, to_right: bool, wrist: bool) -> bool {
    let (skel, _) = model(fig.body);
    let mut pose = Pose::of(skel, fig);
    let view = wrist.then(|| view_turn(camera, fig));
    if !hand(skel, &mut pose, world, is_right, to_right, view) {
        return false;
    }
    pose.write_to(skel, fig);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::PRESETS;
    use photocraft_doc::Body3d;

    /// Landmarks read off the skeleton itself, as if a camera tracked it perfectly.
    fn fake_hand(skel: &Skeleton, pose: &Pose, side: &str) -> Vec<Vec3> {
        let w = pose.world(skel);
        let at = |n: &str| w[skel.find(&format!("{n}{side}")).unwrap()].transform_point3(Vec3::ZERO);
        let tip = |n: &str| {
            let i = skel.find(&format!("{n}{side}")).unwrap();
            w[i].transform_point3(skel.joints[i].tail)
        };
        let mut world = vec![at("hand")];
        for d in ["thumb", "index", "middle", "ring", "pinky"] {
            world.extend([at(&format!("{d}1")), at(&format!("{d}2")), at(&format!("{d}3")), tip(&format!("{d}3"))]);
        }
        world
    }

    fn fake_body(skel: &Skeleton, posed: &Pose) -> Vec<Vec3> {
        let w = posed.world(skel);
        let at = |n: &str| w[skel.find(n).unwrap()].transform_point3(Vec3::ZERO);
        let tip = |n: &str| {
            let i = skel.find(n).unwrap();
            w[i].transform_point3(skel.joints[i].tail)
        };
        let mut world = vec![Vec3::ZERO; BODY_POINTS];
        world[NOSE] = tip("head") * 0.5 + at("head") * 0.5 + Vec3::new(0.0, -0.02, 0.1);
        world[EAR[0]] = at("head") + Vec3::new(0.07, 0.08, 0.0);
        world[EAR[1]] = at("head") + Vec3::new(-0.07, 0.08, 0.0);
        for (s, side) in [(0, ".L"), (1, ".R")] {
            let n = |b: &str| format!("{b}{side}");
            world[SHOULDER[s]] = at(&n("upper_arm"));
            world[ELBOW[s]] = at(&n("forearm"));
            world[WRIST[s]] = at(&n("hand"));
            world[INDEX[s]] = tip(&n("hand"));
            world[PINKY[s]] = tip(&n("hand"));
            world[HIP[s]] = at(&n("thigh"));
            world[KNEE[s]] = at(&n("shin"));
            world[ANKLE[s]] = at(&n("foot"));
            world[TOE[s]] = tip(&n("foot"));
        }
        world
    }

    #[test]
    fn a_tracked_fist_comes_back_as_a_fist() {
        let skel = Skeleton::new(Body3d::Male);
        let mut posed = Pose::rest(&skel);
        for d in ["index", "middle", "ring", "pinky"] {
            for k in 1..=3 {
                posed.local[skel.find(&format!("{d}{k}.L")).unwrap()] = Quat::from_rotation_z(-1.2);
            }
        }
        let h = fake_hand(&skel, &posed, ".L");
        let mut out = Pose::rest(&skel);
        assert!(hand(&skel, &mut out, &h, false, false, None));
        // Twist along a bone can't be seen, so compare where the joints end up.
        let (wa, wb) = (posed.world(&skel), out.world(&skel));
        for d in ["thumb", "index", "middle", "ring", "pinky"] {
            for k in 1..=3 {
                let i = skel.find(&format!("{d}{k}.L")).unwrap();
                let tip = skel.joints[i].tail;
                let gap = (wa[i].transform_point3(tip) - wb[i].transform_point3(tip)).length();
                assert!(gap < 0.002, "{d}{k} tip is {gap} m off");
            }
        }
        // The same landmarks onto the right hand bend its fingers the mirrored way.
        assert!(hand(&skel, &mut out, &h, false, true, None));
        let r = skel.find("index2.R").unwrap();
        assert!(out.local[r].angle_between(Quat::from_rotation_z(1.2)) < 0.1);
    }

    #[test]
    fn a_tracked_body_reproduces_bent_limbs() {
        let skel = Skeleton::new(Body3d::Female);
        let mut posed = Pose::rest(&skel);
        let set = |p: &mut Pose, n: &str, q: Quat| p.local[skel.find(n).unwrap()] = q;
        set(&mut posed, "upper_arm.L", Quat::from_rotation_z(-1.2));
        set(&mut posed, "forearm.L", Quat::from_rotation_y(-1.0));
        set(&mut posed, "thigh.R", Quat::from_rotation_x(-0.8));
        set(&mut posed, "shin.R", Quat::from_rotation_x(1.1));
        let world = fake_body(&skel, &posed);
        let mut out = Pose::rest(&skel);
        assert!(body(&skel, &mut out, &world, &[1.0; BODY_POINTS]));
        let (w, w2) = (posed.world(&skel), out.world(&skel));
        for n in ["forearm.L", "hand.L", "shin.R", "foot.R"] {
            let i = skel.find(n).unwrap();
            let a = w[i].transform_point3(Vec3::ZERO) - w[0].transform_point3(Vec3::ZERO);
            let b = w2[i].transform_point3(Vec3::ZERO) - w2[0].transform_point3(Vec3::ZERO);
            assert!((a - b).length() < 0.03, "{n}: {a} vs {b}");
        }
    }

    #[test]
    fn a_matched_figure_faces_the_layer_camera_as_the_person_faced_theirs() {
        let skel = Skeleton::new(Body3d::Male);
        // A person facing their camera head-on.
        let world = fake_body(&skel, &Pose::rest(&skel));
        let mut fig = crate::new_figure(Body3d::Male, Some("Standing"));
        fig.yaw = 0.4;
        let camera = Camera3d { yaw: 1.2, ..Camera3d::default() };
        assert!(match_body(&mut fig, &camera, &world, &[1.0; BODY_POINTS]));
        // The figure's chest faces the layer's camera.
        let joints = crate::figure_joints(&skel, &fig);
        let chest = joints[skel.find("chest").unwrap()].transform_vector3(Vec3::Z).normalize();
        let toward_camera = Vec3::new(camera.yaw.sin(), 0.0, camera.yaw.cos());
        assert!(chest.dot(toward_camera) > 0.95, "{chest} vs {toward_camera}");
        // Too few landmarks: unchanged.
        let before = fig.clone();
        assert!(!match_body(&mut fig, &camera, &world[..10], &[1.0; 10]));
        assert_eq!(fig, before);
    }

    #[test]
    fn a_hand_matched_from_a_right_hand_onto_the_left_mirrors_it() {
        let skel = Skeleton::new(Body3d::Male);
        let mut posed = Pose::rest(&skel);
        PRESETS.iter().find(|p| p.name == "Fist").unwrap().apply(&skel, &mut posed, &[".R"]);
        let h = fake_hand(&skel, &posed, ".R");
        let mut fig = crate::new_figure(Body3d::Male, Some("Standing"));
        assert!(match_hand(&mut fig, &Camera3d::default(), &h, true, false, false));
        let i = skel.find("index2.L").unwrap();
        let got = Pose::of(&skel, &fig).local[i];
        let want = mirror_quat(posed.local[skel.find("index2.R").unwrap()]);
        assert!(got.angle_between(want) < 0.15, "{got} vs {want}");
    }
}
