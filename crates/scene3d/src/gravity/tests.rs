use super::*;
use crate::new_figure;

fn standing(body: Body3d) -> Figure3d {
    new_figure(body, None)
}

/// Every sphere of every part in a pose, in the figure's frame, with the part's lead joint.
fn spheres(fig: &Figure3d, pose: &Pose) -> Vec<(usize, Vec3, f32)> {
    let (skel, mesh) = model(fig.body);
    let world = pose.world(skel);
    mesh.hits
        .iter()
        .filter(|h| !DIGITS.iter().any(|d| skel.joints[h.joint].name.starts_with(d)))
        .flat_map(|h| {
            let m = world[h.joint];
            [(h.joint, m.transform_point3(h.a), h.radius), (h.joint, m.transform_point3(h.b), h.radius)]
        })
        .collect()
}

/// The mannequin's surface in a pose (its mesh vertices), in the figure's frame.
fn surface(fig: &Figure3d, pose: &Pose) -> Vec<Vec3> {
    let (skel, mesh) = model(fig.body);
    let world = pose.world(skel);
    mesh.vertices.iter().map(|v| world[v.joint as usize].transform_point3(Vec3::from_array(v.pos))).collect()
}

fn lowest(fig: &Figure3d, pose: &Pose) -> f32 {
    surface(fig, pose).iter().map(|p| p.y).fold(f32::MAX, f32::min)
}

fn highest(fig: &Figure3d, pose: &Pose) -> f32 {
    surface(fig, pose).iter().map(|p| p.y).fold(f32::MIN, f32::max)
}

/// A joint's bend about a hinge (radians), as the limits measure it.
fn hinge_angle(fig: &Figure3d, pose: &Pose, joint: &str, axis: Vec3) -> f32 {
    let (skel, _) = model(fig.body);
    let j = skel.find(joint).unwrap();
    let bone = skel.joints[j].tail.normalize();
    let d = pose.local[j] * bone;
    let p = (d - axis * d.dot(axis)).normalize();
    axis.dot(bone.cross(p)).atan2(bone.dot(p))
}

#[test]
fn a_limp_figure_collapses_onto_the_floor_and_comes_to_rest() {
    for body in [Body3d::Male, Body3d::Female] {
        let fig = standing(body);
        let fall = simulate(&fig, &Settings::default());
        assert!(fall.settled, "{body:?} came to rest in {:.1} s", fall.seconds());
        let end = fall.frames.last().unwrap();
        let (top, low) = (highest(&fig, end), lowest(&fig, end));
        assert!(top < 0.5, "{body:?} ends down on the floor, its highest point at {top:.2} m");
        assert!(low > -0.02 && low < 0.03, "{body:?} rests on the floor, not in it or above it: {low:.3} m");
        // It went down, not off sideways.
        let skel = &model(body).0;
        let hips = |p: &Pose| p.world(skel)[0].w_axis.truncate();
        let moved = hips(end) - hips(&fall.frames[0]);
        assert!(moved.y < -0.4 && moved.x.abs() < 1.0 && moved.z.abs() < 1.5, "{moved:?}");
    }
}

#[test]
fn the_first_frame_is_the_pose_it_started_in() {
    let fig = standing(Body3d::Male);
    let fall = simulate(&fig, &Settings::default());
    let (skel, _) = model(fig.body);
    let start = Pose::of(skel, &fig);
    let first = &fall.frames[0];
    assert!((first.root - start.root).length() < 1e-4);
    for (a, b) in first.local.iter().zip(&start.local) {
        assert!(a.angle_between(*b) < 1e-3);
    }
}

#[test]
fn knees_and_elbows_never_bend_the_wrong_way() {
    for push in Push::ALL {
        let fig = standing(Body3d::Male);
        let fall = simulate(&fig, &Settings { push, ..Settings::default() });
        let slack = 4f32.to_radians();
        for (k, p) in fall.frames.iter().enumerate() {
            for side in [".L", ".R"] {
                let knee = hinge_angle(&fig, p, &format!("shin{side}"), Vec3::X);
                assert!(knee > -slack && knee < 150f32.to_radians() + slack, "{push:?} frame {k}: knee{side} {:.0}°", knee.to_degrees());
                let axis = if side == ".L" { Vec3::Y } else { Vec3::NEG_Y };
                let elbow = hinge_angle(&fig, p, &format!("forearm{side}"), axis);
                assert!(elbow < slack && elbow > -150f32.to_radians() - slack, "{push:?} frame {k}: elbow{side} {:.0}°", elbow.to_degrees());
            }
        }
    }
}

#[test]
fn the_push_sends_it_that_way() {
    let fig = standing(Body3d::Male);
    let head = |push: Push| {
        let fall = simulate(&fig, &Settings { push, ..Settings::default() });
        let (skel, _) = model(fig.body);
        let j = skel.find("head").unwrap();
        fall.frames.last().unwrap().world(skel)[j].w_axis.truncate()
    };
    assert!(head(Push::Forward).z > 0.4, "{:?}", head(Push::Forward));
    assert!(head(Push::Back).z < -0.4, "{:?}", head(Push::Back));
    assert!(head(Push::Left).x > 0.3, "{:?}", head(Push::Left));
    assert!(head(Push::Right).x < -0.3, "{:?}", head(Push::Right));
}

#[test]
fn a_pinned_hand_holds_it_up() {
    let mut fig = standing(Body3d::Male);
    // Arms out (the T-pose's), so the hand is well off the floor.
    fig.joints.retain(|k, _| !k.contains("arm") && !k.contains("clavicle"));
    let (skel, _) = model(fig.body);
    let start = Pose::of(skel, &fig);
    let hand = skel.find("hand.L").unwrap();
    let pin = pin_point(skel, &start, hand).unwrap();
    let fall = simulate(&fig, &Settings { pins: vec!["hand.L".into()], ..Settings::default() });
    for (k, p) in fall.frames.iter().enumerate() {
        let now = pin_point(skel, p, hand).unwrap();
        assert!(now.distance(pin) < 0.02, "frame {k}: the hand moved {:.3} m", now.distance(pin));
    }
    // It hangs: the hips have dropped below the hand, and the body sags off the T.
    let end = fall.frames.last().unwrap();
    let hips = end.world(skel)[0].w_axis.truncate();
    assert!(hips.y < start.world(skel)[0].w_axis.truncate().y - 0.05, "{hips:?}");
    assert!(lowest(&fig, end) > -0.02);
}

#[test]
fn pinned_feet_stay_put_as_the_body_folds() {
    let fig = standing(Body3d::Male);
    let (skel, _) = model(fig.body);
    let start = Pose::of(skel, &fig);
    let pins: Vec<String> = ["foot.L", "foot.R"].map(String::from).to_vec();
    let fall = simulate(&fig, &Settings { pins: pins.clone(), push: Push::Back, ..Settings::default() });
    let end = fall.frames.last().unwrap();
    for name in &pins {
        let j = skel.find(name).unwrap();
        assert!(pin_point(skel, end, j).unwrap().distance(pin_point(skel, &start, j).unwrap()) < 0.02, "{name}");
    }
    assert!(highest(&fig, end) < highest(&fig, &start) - 0.3, "the body came down");
}

#[test]
fn parts_dont_pass_through_each_other() {
    for push in [Push::Forward, Push::Left] {
        let fig = standing(Body3d::Female);
        let fall = simulate(&fig, &Settings { push, ..Settings::default() });
        let (skel, _) = model(fig.body);
        let end = fall.frames.last().unwrap();
        // The two thighs, the two shins, and each forearm against the chest.
        for (a, b) in [("thigh.L", "thigh.R"), ("shin.L", "shin.R"), ("forearm.L", "chest"), ("forearm.R", "chest")] {
            let (ja, jb) = (skel.find(a).unwrap(), skel.find(b).unwrap());
            let s = spheres(&fig, end);
            for (_, ca, ra) in s.iter().filter(|(j, ..)| *j == ja) {
                for (_, cb, rb) in s.iter().filter(|(j, ..)| *j == jb) {
                    let overlap = ra + rb - ca.distance(*cb);
                    assert!(overlap < 0.06, "{push:?}: {a} sinks {overlap:.3} m into {b}");
                }
            }
        }
    }
}

#[test]
fn muscle_tone_keeps_more_of_the_pose() {
    let fig = standing(Body3d::Male);
    let (skel, _) = model(fig.body);
    let start = Pose::of(skel, &fig);
    let bend = |tone: f32| {
        let fall = simulate(&fig, &Settings { tone, ..Settings::default() });
        let end = fall.frames.last().unwrap().clone();
        end.local.iter().zip(&start.local).map(|(a, b)| a.angle_between(*b)).sum::<f32>()
    };
    let (limp, firm) = (bend(0.0), bend(0.9));
    assert!(firm < limp * 0.7, "limp {limp:.2} rad, firm {firm:.2} rad");
}

#[test]
fn the_same_fall_twice_is_the_same() {
    let fig = standing(Body3d::Male);
    let s = Settings { pins: vec!["hand.R".into()], tone: 0.3, ..Settings::default() };
    assert_eq!(simulate(&fig, &s), simulate(&fig, &s));
}

#[test]
fn apply_poses_the_figure_as_a_frame() {
    let mut fig = standing(Body3d::Male);
    let fall = simulate(&fig, &Settings::default());
    let (skel, _) = model(fig.body);
    fall.apply(usize::MAX, &mut fig);
    let now = Pose::of(skel, &fig);
    let last = fall.frames.last().unwrap();
    assert!((now.root - last.root).length() < 1e-4);
}
