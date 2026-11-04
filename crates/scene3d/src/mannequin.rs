//! The wooden artist's mannequin: rounded segments and ball joints built in code, each part fixed
//! to one joint of the skeleton (no skinning: the parts move rigidly, like a real mannequin).

use crate::skeleton::Skeleton;
use glam::Vec3;
use photocraft_doc::Body3d as Body;

pub const MAT_WOOD: u32 = 0;
pub const MAT_BALL: u32 = 1;

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub joint: u32,
    pub material: u32,
}

/// A rough shape of a part for picking: a capsule from `a` to `b` in the joint's frame.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub joint: usize,
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub hits: Vec<Hit>,
}

// Coarser than a GPU mesh would be: the CPU renderer shades every pixel, and the parts are small.
const RINGS: usize = 14;
const SIDES: usize = 16;

impl Mesh {
    /// A rounded segment from `a` to `b`: radius `ra` near `a` easing to `rb` near `b`, with its
    /// cross-section squashed to `depth` × the width across `width_axis`.
    #[allow(clippy::too_many_arguments)]
    fn segment(&mut self, joint: usize, a: Vec3, b: Vec3, ra: f32, rb: f32, depth: f32, width_axis: Vec3, material: u32) {
        self.shaped(joint, a, b, [ra, rb], depth, width_axis, material, 0.72);
    }

    /// `segment` with its squareness: 1 is an ellipsoid, lower is fuller and boxier.
    #[allow(clippy::too_many_arguments)]
    fn shaped(&mut self, joint: usize, a: Vec3, b: Vec3, [ra, rb]: [f32; 2], depth: f32, width_axis: Vec3, material: u32, e: f32) {
        let axis = (b - a).normalize_or(Vec3::Y);
        let len = (b - a).length();
        let mut u = (width_axis - axis * width_axis.dot(axis)).normalize_or(axis.any_orthonormal_vector());
        if u.length_squared() < 0.5 {
            u = axis.any_orthonormal_vector();
        }
        let v = axis.cross(u);
        let start = self.vertices.len() as u32;
        // A superellipsoid along the axis: fuller than an ellipsoid, rounded at both ends.
        let spow = |x: f32| x.signum() * x.abs().powf(e);
        for i in 0..=RINGS {
            let th = std::f32::consts::PI * i as f32 / RINGS as f32;
            let along = 0.5 * (1.0 - spow(th.cos()));
            let r = (ra + (rb - ra) * along) * spow(th.sin()).max(0.0);
            let center = a + axis * (len * along);
            for k in 0..SIDES {
                let ph = std::f32::consts::TAU * k as f32 / SIDES as f32;
                let p = center + u * (r * spow(ph.cos())) + v * (r * depth * spow(ph.sin()));
                self.vertices.push(Vertex { pos: p.to_array(), normal: [0.0; 3], joint: joint as u32, material });
            }
        }
        for i in 0..RINGS as u32 {
            for k in 0..SIDES as u32 {
                let k1 = (k + 1) % SIDES as u32;
                let p00 = start + i * SIDES as u32 + k;
                let p01 = start + i * SIDES as u32 + k1;
                let p10 = start + (i + 1) * SIDES as u32 + k;
                let p11 = start + (i + 1) * SIDES as u32 + k1;
                self.indices.extend_from_slice(&[p00, p10, p11, p00, p11, p01]);
            }
        }
        self.hits.push(Hit { joint, a, b, radius: ra.max(rb) });
    }

    fn ball(&mut self, joint: usize, c: Vec3, r: f32) {
        // A sphere is a segment through its own centre with no squash.
        let start = self.vertices.len() as u32;
        for i in 0..=RINGS {
            let th = std::f32::consts::PI * i as f32 / RINGS as f32;
            for k in 0..SIDES {
                let ph = std::f32::consts::TAU * k as f32 / SIDES as f32;
                let n = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
                self.vertices.push(Vertex { pos: (c + n * r).to_array(), normal: n.to_array(), joint: joint as u32, material: MAT_BALL });
            }
        }
        for i in 0..RINGS as u32 {
            for k in 0..SIDES as u32 {
                let k1 = (k + 1) % SIDES as u32;
                let p00 = start + i * SIDES as u32 + k;
                let p01 = start + i * SIDES as u32 + k1;
                let p10 = start + (i + 1) * SIDES as u32 + k;
                let p11 = start + (i + 1) * SIDES as u32 + k1;
                self.indices.extend_from_slice(&[p00, p11, p10, p00, p01, p11]);
            }
        }
        self.hits.push(Hit { joint, a: c, b: c, radius: r });
    }

    /// Smooth normals from the triangles, for every vertex that doesn't have one yet.
    fn finish_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.vertices.len()];
        for t in self.indices.as_chunks::<3>().0 {
            let p = |i: u32| self.vertices.get(i as usize).map_or(Vec3::ZERO, |v| Vec3::from_array(v.pos));
            let [a, b, c] = [t[0], t[1], t[2]].map(p);
            let n = (b - a).cross(c - a);
            for &i in t {
                if let Some(s) = acc.get_mut(i as usize) {
                    *s += n;
                }
            }
        }
        for (v, n) in self.vertices.iter_mut().zip(acc) {
            if v.normal == [0.0; 3] {
                v.normal = n.normalize_or(Vec3::Y).to_array();
            }
        }
    }
}

/// Builds the mannequin for `skel`.
pub fn build(skel: &Skeleton) -> Mesh {
    let mut m = Mesh::default();
    let female = skel.body == Body::Female;
    let s = if female { 0.93 } else { 1.0 };
    // Every name below is a joint of the skeleton (tested); a missing one would hang its part on
    // the hips rather than fail.
    let j = |name: &str| skel.find(name).unwrap_or(0);
    let tail = |name: &str| skel.joints.get(j(name)).map_or(Vec3::ZERO, |b| b.tail);
    let y = |v: f32| Vec3::new(0.0, v * s, 0.0);

    // Torso.
    let (pelvis_w, waist_w, chest_w) = if female { (0.165, 0.115, 0.145) } else { (0.155, 0.13, 0.175) };
    m.shaped(j("hips"), y(-0.1), y(0.11), [pelvis_w * s, pelvis_w * 0.85 * s], 0.68, Vec3::X, MAT_WOOD, 0.62);
    m.shaped(j("spine"), y(-0.02), y(0.16), [waist_w * s, waist_w * 1.05 * s], 0.72, Vec3::X, MAT_WOOD, 0.7);
    m.shaped(j("chest"), y(-0.03), y(0.24), [chest_w * 0.92 * s, chest_w * s], 0.62, Vec3::X, MAT_WOOD, 0.62);
    if female {
        for x in [-1.0f32, 1.0] {
            m.ball_wood(j("chest"), Vec3::new(0.055 * x, 0.12, 0.07) * s, 0.06 * s);
        }
    }
    m.segment(j("neck"), y(-0.02), y(0.11), 0.048 * s, 0.042 * s, 1.0, Vec3::X, MAT_WOOD);
    m.shaped(j("head"), Vec3::new(0.0, -0.01, 0.012) * s, Vec3::new(0.0, 0.235, -0.005) * s, [0.07 * s, 0.092 * s], 1.18, Vec3::X, MAT_WOOD, 0.92);
    // A small nose, so the face's direction reads at a glance.
    m.segment(j("head"), Vec3::new(0.0, 0.085, 0.085) * s, Vec3::new(0.0, 0.1, 0.112) * s, 0.012 * s, 0.01 * s, 1.0, Vec3::X, MAT_WOOD);

    let k = if female { 0.88 } else { 1.0 };
    for side in [".L", ".R"] {
        let n = |b: &str| format!("{b}{side}");
        let along = |b: &str| tail(&n(b));
        // Arms.
        m.segment(j(&n("clavicle")), Vec3::ZERO, along("clavicle") * 0.9, 0.03 * s * k, 0.035 * s * k, 0.8, Vec3::Z, MAT_WOOD);
        m.ball(j(&n("upper_arm")), Vec3::ZERO, 0.052 * s * k);
        m.segment(j(&n("upper_arm")), along("upper_arm") * 0.06, along("upper_arm") * 0.96, 0.047 * s * k, 0.037 * s * k, 0.9, Vec3::Z, MAT_WOOD);
        m.ball(j(&n("forearm")), Vec3::ZERO, 0.034 * s * k);
        m.segment(j(&n("forearm")), along("forearm") * 0.05, along("forearm") * 0.97, 0.039 * s * k, 0.026 * s * k, 0.8, Vec3::Z, MAT_WOOD);
        m.ball(j(&n("hand")), Vec3::ZERO, 0.024 * s * k);
        let palm = along("hand");
        m.segment(j(&n("hand")), palm * 0.08, palm * 1.08, 0.038 * s * k, 0.047 * s * k, 0.36, Vec3::Z, MAT_WOOD);
        for (d, digit) in crate::skeleton::DIGITS.iter().enumerate() {
            let r = if d == 0 { 0.012 } else { 0.0095 } * s * k;
            for seg in 1..=3 {
                let name = n(&format!("{digit}{seg}"));
                let t = tail(&name);
                if seg > 1 || d == 0 {
                    m.ball(j(&name), Vec3::ZERO, r * 0.95);
                }
                m.segment(j(&name), t * 0.08, t * 1.02, r, r * 0.9, 0.85, Vec3::Z, MAT_WOOD);
            }
        }
        // Legs.
        m.ball(j(&n("thigh")), Vec3::ZERO, 0.075 * s);
        m.segment(j(&n("thigh")), along("thigh") * 0.04, along("thigh") * 0.97, 0.085 * s, 0.052 * s, 0.92, Vec3::X, MAT_WOOD);
        m.ball(j(&n("shin")), Vec3::ZERO, 0.05 * s);
        m.segment(j(&n("shin")), along("shin") * 0.04, along("shin") * 0.97, 0.055 * s, 0.036 * s, 0.9, Vec3::X, MAT_WOOD);
        m.ball(j(&n("foot")), Vec3::ZERO, 0.036 * s);
        m.segment(j(&n("foot")), Vec3::new(0.0, -0.035, -0.045) * s, along("foot") * 1.02, 0.042 * s * k, 0.045 * s * k, 0.55, Vec3::X, MAT_WOOD);
        m.segment(j(&n("toes")), Vec3::new(0.0, 0.0, -0.01) * s, along("toes"), 0.042 * s * k, 0.038 * s * k, 0.45, Vec3::X, MAT_WOOD);
    }
    m.finish_normals();
    m
}

impl Mesh {
    /// A wooden sphere (part of a body segment rather than a joint).
    fn ball_wood(&mut self, joint: usize, c: Vec3, r: f32) {
        let from = self.vertices.len();
        self.ball(joint, c, r);
        for v in self.vertices.iter_mut().skip(from) {
            v.material = MAT_WOOD;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_bodies_build_with_valid_indices() {
        for body in [Body::Male, Body::Female] {
            let s = Skeleton::new(body);
            let m = build(&s);
            assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
            assert!(m.vertices.iter().all(|v| (v.joint as usize) < s.joints.len()));
            assert!(m.vertices.iter().all(|v| Vec3::from_array(v.normal).is_finite()));
        }
    }
}
