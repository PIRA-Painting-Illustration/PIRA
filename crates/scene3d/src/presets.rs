//! Ready-made poses. Each is written for the left side and middle; the right side mirrors it
//! unless the pose sets it.
//!
//! Turns are degrees about the joint's parent axes, applied in order. From the T-pose: an arm
//! comes down with −Z (left), an elbow bends forward with −Y, a hip flexes with −X, a knee
//! bends with +X, and the fingers curl with −Z.

use crate::skeleton::{Pose, Skeleton, mirror_quat};
use glam::Quat;

type Turns = &'static [(char, f32)];

pub struct Preset {
    pub name: &'static str,
    /// Only touches the hands.
    pub hands_only: bool,
    left: &'static [(&'static str, Turns)],
    right: &'static [(&'static str, Turns)],
}

fn turns(t: Turns) -> Quat {
    t.iter().fold(Quat::IDENTITY, |q, &(axis, deg)| {
        let r = deg.to_radians();
        let step = match axis {
            'x' => Quat::from_rotation_x(r),
            'y' => Quat::from_rotation_y(r),
            _ => Quat::from_rotation_z(r),
        };
        (step * q).normalize()
    })
}

const NONE: Turns = &[];
const CURL: Turns = &[('z', -80.0)];
const CURL_TIP: Turns = &[('z', -65.0)];
const RELAX: Turns = &[('z', -18.0)];
const THUMB_TUCK: Turns = &[('y', 30.0), ('z', -20.0)];
const THUMB_IN: Turns = &[('y', 25.0)];
const THUMB_REST: Turns = &[('y', 10.0), ('z', -10.0)];

macro_rules! fingers {
    ($t1:expr, $t2:expr) => {
        [
            ("index1", $t1), ("index2", $t2), ("index3", $t2),
            ("middle1", $t1), ("middle2", $t2), ("middle3", $t2),
            ("ring1", $t1), ("ring2", $t2), ("ring3", $t2),
            ("pinky1", $t1), ("pinky2", $t2), ("pinky3", $t2),
        ]
    };
}

const FIST: [(&str, Turns); 15] = {
    let f = fingers!(CURL, CURL_TIP);
    [f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11], ("thumb1", THUMB_TUCK), ("thumb2", THUMB_IN), ("thumb3", THUMB_IN)]
};
const RELAXED: [(&str, Turns); 15] = {
    let f = fingers!(RELAX, RELAX);
    [f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11], ("thumb1", THUMB_REST), ("thumb2", &[('y', 8.0)]), ("thumb3", &[('y', 8.0)])]
};
const POINT: [(&str, Turns); 15] = {
    let f = fingers!(CURL, CURL_TIP);
    [
        ("index1", &[('z', -4.0)]), ("index2", &[('z', -4.0)]), ("index3", &[('z', -2.0)]),
        f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11],
        ("thumb1", THUMB_TUCK), ("thumb2", THUMB_IN), ("thumb3", THUMB_IN),
    ]
};
const OPEN: [(&str, Turns); 15] = {
    let f = fingers!(NONE, NONE);
    [f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8], f[9], f[10], f[11], ("thumb1", NONE), ("thumb2", NONE), ("thumb3", NONE)]
};

macro_rules! with_hands {
    ($hands:expr; $($j:expr),* $(,)?) => {{
        const H: [(&str, Turns); 15] = $hands;
        &[$($j,)* H[0], H[1], H[2], H[3], H[4], H[5], H[6], H[7], H[8], H[9], H[10], H[11], H[12], H[13], H[14]]
    }};
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "T-pose", hands_only: false, left: with_hands!(OPEN;), right: &[] },
    Preset {
        name: "Standing",
        hands_only: false,
        left: with_hands!(RELAXED; ("upper_arm", &[('z', -76.0), ('x', -4.0)]), ("forearm", &[('y', -12.0)]), ("thigh", &[('z', 2.0)])),
        right: &[],
    },
    Preset {
        name: "Contrapposto",
        hands_only: false,
        left: with_hands!(RELAXED;
            ("hips", &[('z', 5.0), ('y', -6.0)]), ("spine", &[('z', -3.0)]), ("chest", &[('z', -5.0), ('y', 6.0)]), ("neck", &[('z', 3.0)]),
            ("upper_arm", &[('z', -72.0), ('x', 6.0)]), ("forearm", &[('y', -20.0)]),
            ("thigh", &[('z', -3.0), ('x', -4.0)]), ("shin", &[('x', 4.0)]),
        ),
        right: &[
            ("upper_arm", &[('z', 80.0), ('x', -3.0)]), ("forearm", &[('y', 10.0)]),
            ("thigh", &[('z', -6.0), ('x', -14.0)]), ("shin", &[('x', 24.0)]), ("foot", &[('x', -8.0), ('y', -12.0)]),
        ],
    },
    Preset {
        name: "Walking",
        hands_only: false,
        left: with_hands!(RELAXED;
            ("hips", &[('y', 6.0)]), ("chest", &[('y', -10.0)]),
            ("upper_arm", &[('z', -78.0), ('x', 28.0)]), ("forearm", &[('y', -14.0)]),
            ("thigh", &[('x', -26.0)]), ("shin", &[('x', 10.0)]), ("foot", &[('x', 12.0)]),
        ),
        right: &[
            ("upper_arm", &[('z', 78.0), ('x', -30.0)]), ("forearm", &[('y', 34.0)]),
            ("thigh", &[('x', 18.0)]), ("shin", &[('x', 32.0)]), ("foot", &[('x', -10.0)]),
        ],
    },
    Preset {
        name: "Running",
        hands_only: false,
        left: with_hands!(FIST;
            ("spine", &[('x', 10.0)]), ("chest", &[('x', 6.0), ('y', -14.0)]), ("neck", &[('x', -12.0)]),
            ("upper_arm", &[('z', -72.0), ('x', 45.0)]), ("forearm", &[('y', -95.0)]),
            ("thigh", &[('x', -62.0)]), ("shin", &[('x', 80.0)]), ("foot", &[('x', -15.0)]),
        ),
        right: &[
            ("upper_arm", &[('z', 70.0), ('x', -40.0)]), ("forearm", &[('y', 90.0)]),
            ("thigh", &[('x', 22.0)]), ("shin", &[('x', 70.0)]), ("foot", &[('x', 25.0)]),
        ],
    },
    Preset {
        name: "Sitting",
        hands_only: false,
        left: with_hands!(RELAXED;
            ("spine", &[('x', 6.0)]), ("neck", &[('x', -4.0)]),
            ("upper_arm", &[('z', -72.0), ('x', -28.0)]), ("forearm", &[('y', -62.0)]), ("hand", &[('z', -10.0)]),
            ("thigh", &[('x', -88.0), ('z', 4.0)]), ("shin", &[('x', 86.0)]),
        ),
        right: &[],
    },
    Preset {
        name: "Reaching up",
        hands_only: false,
        left: with_hands!(OPEN;
            ("spine", &[('x', -4.0)]), ("chest", &[('x', -6.0)]), ("neck", &[('x', -18.0)]),
            ("upper_arm", &[('z', 70.0), ('x', -10.0)]), ("forearm", &[('y', -8.0)]),
        ),
        right: &[("upper_arm", &[('z', 78.0), ('x', -30.0)]), ("forearm", &[('y', 20.0)])],
    },
    Preset { name: "Fist", hands_only: true, left: &FIST, right: &[] },
    Preset { name: "Relaxed hand", hands_only: true, left: &RELAXED, right: &[] },
    Preset { name: "Pointing", hands_only: true, left: &POINT, right: &[] },
    Preset { name: "Open hand", hands_only: true, left: &OPEN, right: &[] },
];

impl Preset {
    /// Applies the preset. A hand preset only changes the hand(s) in `sides` (".L", ".R").
    pub fn apply(&self, skel: &Skeleton, pose: &mut Pose, sides: &[&str]) {
        if !self.hands_only {
            *pose = Pose::rest(skel);
        }
        let set = |pose: &mut Pose, name: &str, q: Quat| {
            if let Some(slot) = skel.find(name).and_then(|i| pose.local.get_mut(i)) {
                *slot = q;
            }
        };
        for &(joint, t) in self.left {
            let q = turns(t);
            if skel.find(joint).is_some() {
                // A middle joint (hips, spine, …).
                if !self.hands_only {
                    set(pose, joint, q);
                }
                continue;
            }
            for side in sides {
                set(pose, &format!("{joint}{side}"), if *side == ".L" { q } else { mirror_quat(q) });
            }
        }
        for &(joint, t) in self.right {
            if sides.contains(&".R") {
                set(pose, &format!("{joint}.R"), turns(t));
            }
        }
        if !self.hands_only {
            pose.ground(skel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_doc::Body3d as Body;

    #[test]
    fn every_preset_names_real_joints() {
        let skel = Skeleton::new(Body::Male);
        for p in PRESETS {
            for (j, _) in p.left.iter().chain(p.right) {
                assert!(skel.find(j).is_some() || skel.find(&format!("{j}.L")).is_some(), "{}: {j}", p.name);
            }
        }
    }

    #[test]
    fn standing_puts_the_hands_by_the_hips() {
        let skel = Skeleton::new(Body::Male);
        let mut pose = Pose::rest(&skel);
        PRESETS.iter().find(|p| p.name == "Standing").unwrap().apply(&skel, &mut pose, &[".L", ".R"]);
        let w = pose.world(&skel);
        for side in [".L", ".R"] {
            let hand = w[skel.find(&format!("hand{side}")).unwrap()].w_axis;
            assert!(hand.y < 1.0 && hand.y > 0.6, "{side} hand at {hand}");
            assert!(hand.x.abs() < 0.4);
        }
    }
}
