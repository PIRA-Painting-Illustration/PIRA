//! The mannequin's skeleton and its pose.
//!
//! The rest pose is a T-pose with every joint's rotation at identity, so a joint's local axes line
//! up with the world axes at rest: +X is the figure's left, +Y is up and +Z is the way it faces.
//! A pose is one local rotation per joint (relative to its parent) plus where the hips sit.

use glam::{Mat4, Quat, Vec3};
use photocraft_doc::{Body3d as Body, Figure3d};

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    /// Where the joint sits in its parent's frame at rest.
    pub offset: Vec3,
    /// The bone from this joint to the next one (or its tip), in this joint's frame at rest.
    pub tail: Vec3,
}

#[derive(Clone, Debug)]
pub struct Skeleton {
    pub body: Body,
    pub joints: Vec<Joint>,
}

/// The five digits, thumb first, each with three joints.
pub const DIGITS: [&str; 5] = ["thumb", "index", "middle", "ring", "pinky"];

impl Skeleton {
    pub fn new(body: Body) -> Self {
        // Lengths in metres for a 1.78 m man; the woman is scaled down and reshaped.
        let (s, shoulder, hip_x) = match body {
            Body::Male => (1.0, 0.165, 0.095),
            Body::Female => (0.93, 0.14, 0.1),
        };
        let mut b = Builder { joints: Vec::new() };
        let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * s;
        let hips = b.add("hips", None, v(0.0, 0.98, 0.0), v(0.0, 0.1, 0.0));
        let spine = b.add("spine", Some(hips), v(0.0, 0.1, 0.0), v(0.0, 0.14, 0.0));
        let chest = b.add("chest", Some(spine), v(0.0, 0.14, 0.0), v(0.0, 0.22, 0.0));
        let neck = b.add("neck", Some(chest), v(0.0, 0.22, 0.0), v(0.0, 0.09, 0.0));
        b.add("head", Some(neck), v(0.0, 0.09, 0.0), v(0.0, 0.22, 0.0));
        for (side, sx) in [(".L", 1.0f32), (".R", -1.0)] {
            let m = |p: Vec3| Vec3::new(p.x * sx, p.y, p.z);
            let clav = b.add(&format!("clavicle{side}"), Some(chest), m(v(0.02, 0.17, 0.0)), m(Vec3::new(shoulder, 0.01 * s, 0.0)));
            let upper = b.add(&format!("upper_arm{side}"), Some(clav), m(Vec3::new(shoulder, 0.01 * s, 0.0)), m(v(0.29, 0.0, 0.0)));
            let fore = b.add(&format!("forearm{side}"), Some(upper), m(v(0.29, 0.0, 0.0)), m(v(0.26, 0.0, 0.0)));
            let hand = b.add(&format!("hand{side}"), Some(fore), m(v(0.26, 0.0, 0.0)), m(v(0.09, 0.0, 0.0)));
            // Palm down, fingers along the arm, thumb forward.
            let digits: [(Vec3, [Vec3; 3]); 5] = [
                (v(0.022, -0.012, 0.028), [v(0.032, -0.006, 0.03), v(0.026, 0.0, 0.02), v(0.022, 0.0, 0.014)]),
                (v(0.09, 0.0, 0.03), [v(0.046, 0.0, 0.002), v(0.026, 0.0, 0.0), v(0.021, 0.0, 0.0)]),
                (v(0.093, 0.0, 0.008), [v(0.05, 0.0, 0.0), v(0.03, 0.0, 0.0), v(0.022, 0.0, 0.0)]),
                (v(0.089, 0.0, -0.013), [v(0.046, 0.0, -0.002), v(0.028, 0.0, 0.0), v(0.021, 0.0, 0.0)]),
                (v(0.08, -0.003, -0.032), [v(0.036, 0.0, -0.004), v(0.021, 0.0, 0.0), v(0.018, 0.0, 0.0)]),
            ];
            for (name, (base, bones)) in DIGITS.iter().zip(digits) {
                let mut parent = hand;
                let mut offset = m(base);
                for (k, tail) in bones.iter().enumerate() {
                    parent = b.add(&format!("{name}{}{side}", k + 1), Some(parent), offset, m(*tail));
                    offset = m(*tail);
                }
            }
            let thigh = b.add(&format!("thigh{side}"), Some(hips), Vec3::new(hip_x * sx, -0.06 * s, 0.0), v(0.0, -0.44, 0.0));
            let shin = b.add(&format!("shin{side}"), Some(thigh), v(0.0, -0.44, 0.0), v(0.0, -0.42, 0.0));
            let foot = b.add(&format!("foot{side}"), Some(shin), v(0.0, -0.42, 0.0), v(0.0, -0.06, 0.14));
            b.add(&format!("toes{side}"), Some(foot), v(0.0, -0.06, 0.14), v(0.0, 0.0, 0.06));
        }
        Skeleton { body, joints: b.joints }
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j.name == name)
    }

    /// The joint on the other side (`upper_arm.L` ↔ `upper_arm.R`), or itself on the middle line.
    pub fn mirror_of(&self, i: usize) -> usize {
        let Some(name) = self.joints.get(i).map(|j| &j.name) else { return i };
        let other = if let Some(n) = name.strip_suffix(".L") {
            format!("{n}.R")
        } else if let Some(n) = name.strip_suffix(".R") {
            format!("{n}.L")
        } else {
            return i;
        };
        self.find(&other).unwrap_or(i)
    }
}

struct Builder {
    joints: Vec<Joint>,
}

impl Builder {
    fn add(&mut self, name: &str, parent: Option<usize>, offset: Vec3, tail: Vec3) -> usize {
        self.joints.push(Joint { name: name.to_owned(), parent, offset, tail });
        self.joints.len() - 1
    }
}

/// Reflects a local rotation across the body's middle (X → −X).
pub fn mirror_quat(q: Quat) -> Quat {
    Quat::from_xyzw(q.x, -q.y, -q.z, q.w)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    /// Added to the hips' rest position.
    pub root: Vec3,
    pub local: Vec<Quat>,
}

impl Pose {
    pub fn rest(skel: &Skeleton) -> Self {
        Pose { root: Vec3::ZERO, local: vec![Quat::IDENTITY; skel.joints.len()] }
    }

    /// Each joint's frame in the world (parents always come before their children).
    pub fn world(&self, skel: &Skeleton) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(skel.joints.len());
        for (i, j) in skel.joints.iter().enumerate() {
            let local = Mat4::from_rotation_translation(self.local.get(i).copied().unwrap_or(Quat::IDENTITY), j.offset);
            let parent = j.parent.and_then(|p| out.get(p).copied()).unwrap_or(Mat4::from_translation(self.root));
            out.push(parent * local);
        }
        out
    }

    /// Each joint's rotation in the world.
    pub fn world_rot(&self, skel: &Skeleton) -> Vec<Quat> {
        let mut out: Vec<Quat> = Vec::with_capacity(skel.joints.len());
        for (i, j) in skel.joints.iter().enumerate() {
            let q = self.local.get(i).copied().unwrap_or(Quat::IDENTITY);
            out.push(match j.parent.and_then(|p| out.get(p)) {
                Some(p) => (*p * q).normalize(),
                None => q,
            });
        }
        out
    }

    /// Copies one side onto the other, mirrored. `from_left` copies .L onto .R.
    pub fn mirror(&mut self, skel: &Skeleton, from_left: bool) {
        let src = if from_left { ".L" } else { ".R" };
        for i in 0..skel.joints.len() {
            if skel.joints.get(i).is_some_and(|j| j.name.ends_with(src)) {
                let o = skel.mirror_of(i);
                if let Some(q) = self.local.get(i).copied()
                    && let Some(slot) = self.local.get_mut(o)
                {
                    *slot = mirror_quat(q);
                }
            }
        }
    }

    /// Moves the figure up or down so its lowest point rests on the floor: the feet when
    /// standing, but equally the hands in a handstand or the back when lying down.
    pub fn ground(&mut self, skel: &Skeleton) {
        let world = self.world(skel);
        let mut low = f32::MAX;
        for (i, j) in skel.joints.iter().enumerate() {
            // Roughly how far the body's surface sits from the bone here.
            let r = match j.name.split('.').next().unwrap_or("") {
                "foot" | "toes" => 0.025,
                "shin" => 0.045,
                "hips" | "spine" | "chest" => 0.1,
                "head" => 0.09,
                "neck" => 0.05,
                "thigh" => 0.07,
                "clavicle" | "upper_arm" | "forearm" => 0.04,
                "hand" => 0.02,
                _ => 0.01,
            };
            let Some(w) = world.get(i) else { continue };
            for p in [w.w_axis.truncate(), w.transform_point3(j.tail)] {
                low = low.min(p.y - r);
            }
        }
        if low.is_finite() {
            self.root.y -= low;
        }
    }

    /// The pose as stored in a [`Figure3d`] (only joints away from rest are written).
    pub fn write_to(&self, skel: &Skeleton, fig: &mut Figure3d) {
        fig.body = skel.body;
        fig.root = self.root.to_array();
        fig.joints = skel
            .joints
            .iter()
            .zip(&self.local)
            .filter(|(_, q)| q.angle_between(Quat::IDENTITY) > 1e-4)
            .map(|(j, q)| (j.name.clone(), q.to_array()))
            .collect();
    }

    /// The pose a [`Figure3d`] stores. Unknown joint names and broken rotations are ignored.
    pub fn of(skel: &Skeleton, fig: &Figure3d) -> Self {
        Self::from_joints(skel, fig.root, fig.joints.iter().map(|(k, v)| (k.as_str(), *v)))
    }

    /// A pose from a root offset and named joint rotations.
    pub fn from_joints<'a>(skel: &Skeleton, root: [f32; 3], joints: impl IntoIterator<Item = (&'a str, [f32; 4])>) -> Self {
        let mut p = Pose::rest(skel);
        let root = Vec3::from_array(root);
        if root.is_finite() {
            p.root = root.clamp(Vec3::splat(-10.0), Vec3::splat(10.0));
        }
        for (name, q) in joints {
            let q = Quat::from_array(q);
            if let Some(slot) = skel.find(name).and_then(|i| p.local.get_mut(i))
                && q.is_finite()
                && q.length_squared() > 1e-6
            {
                *slot = q.normalize();
            }
        }
        p
    }
}
