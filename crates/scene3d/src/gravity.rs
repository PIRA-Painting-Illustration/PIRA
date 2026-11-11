//! Gravity: the mannequin as a ragdoll that falls, slumps or hangs.
//!
//! Each main part (hips, spine, chest, head, upper and lower arms, hands, thighs, shins, feet) is
//! a solid body with a person's share of the weight; collarbones, the head, fingers and toes ride
//! rigidly on the part they hang from. The parts are linked at their joints, and every joint
//! bends only as far as a body does: elbows and knees one way, shoulders and hips within a cone,
//! each bone twisting a little. Parts land on the floor with friction, don't pass through each
//! other, and can be pinned where they are (a hand to hang from, a foot that stays put). Muscle
//! tone pulls each joint back towards its pose: none drops like a doll, more slumps and buckles.
//!
//! Solved with substepped XPBD (Müller et al., "Detailed Rigid Body Simulation with Extended
//! Position Based Dynamics", 2020): the same figure and settings always fall the same way.
//! Everything happens in the figure's own frame (metres, floor at y = 0, +Z the way it faces).

use crate::model;
use crate::skeleton::{DIGITS, Pose, Skeleton};
use glam::{Mat3, Quat, Vec3};
use photocraft_doc::{Body3d, Figure3d};

/// Frames per second of a [`Fall`].
pub const FPS: f32 = 30.0;
/// Simulation steps per recorded frame, and substeps per step.
const STEPS_PER_FRAME: usize = 2;
const SUBSTEPS: usize = 40;
const STEP: f32 = 1.0 / (FPS * STEPS_PER_FRAME as f32);
const GRAVITY: f32 = 9.81;
const FRICTION: f32 = 0.7;
/// Velocity damping per second: the air, and the soft tissue at every joint.
const LINEAR_DAMPING: f32 = 0.2;
const ANGULAR_DAMPING: f32 = 0.6;
const JOINT_DAMPING: f32 = 8.0;
/// The figure is at rest when, over this many frames, its hips moved less than `STILL.0` metres
/// and no joint turned more than `STILL.1` radians (net: a part trembling in place still counts).
const STILL: (f32, f32) = (0.01, 0.05);
const STILL_FRAMES: usize = 15;
/// Extra damping (per second) for a part lying on the floor: it rolls to a stop.
const FLOOR_DAMPING: (f32, f32) = (4.0, 15.0);
/// Joint stiffness at full muscle tone (N·m per radian).
const FULL_TONE: f32 = 400.0;

/// Which way the figure is nudged as it starts to fall.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Push {
    #[default]
    None,
    Forward,
    Back,
    Left,
    Right,
}

impl Push {
    pub const ALL: [Push; 5] = [Push::None, Push::Forward, Push::Back, Push::Left, Push::Right];

    pub fn label(self) -> &'static str {
        match self {
            Push::None => "None",
            Push::Forward => "Forward",
            Push::Back => "Back",
            Push::Left => "Left",
            Push::Right => "Right",
        }
    }

    /// The push in the figure's frame (+X is its left, +Z the way it faces).
    fn dir(self) -> Vec3 {
        match self {
            Push::None => Vec3::ZERO,
            Push::Forward => Vec3::Z,
            Push::Back => Vec3::NEG_Z,
            Push::Left => Vec3::X,
            Push::Right => Vec3::NEG_X,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Muscle tone, 0–1: 0 is limp, like a dropped doll; 1 holds the pose stiffly as it topples.
    pub tone: f32,
    pub push: Push,
    /// Joints whose parts are pinned where they are now (by joint name; a finger pins its hand).
    pub pins: Vec<String>,
    /// The longest fall simulated (seconds).
    pub max_seconds: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { tone: 0.0, push: Push::Forward, pins: Vec::new(), max_seconds: 8.0 }
    }
}

/// A simulated fall: the figure's pose [`FPS`] times a second, from the pose it started in.
#[derive(Clone, Debug, PartialEq)]
pub struct Fall {
    pub frames: Vec<Pose>,
    /// The figure came to rest (rather than running out of time).
    pub settled: bool,
}

impl Fall {
    /// Poses `fig` as frame `i`, or the last frame past the end.
    pub fn apply(&self, i: usize, fig: &mut Figure3d) {
        let (skel, _) = model(fig.body);
        if let Some(p) = self.frames.get(i).or(self.frames.last()) {
            p.write_to(skel, fig);
        }
    }

    pub fn seconds(&self) -> f32 {
        self.frames.len().saturating_sub(1) as f32 / FPS
    }
}

/// Lets `fig` fall from the pose it is in.
pub fn simulate(fig: &Figure3d, settings: &Settings) -> Fall {
    let (skel, mesh) = model(fig.body);
    let pose = Pose::of(skel, fig);
    let scale = crate::finite_or(fig.scale, 1.0).clamp(0.05, 20.0);
    let mut sim = Sim::new(skel, &mesh.hits, &pose, fig.body, settings);
    // A bigger figure falls as slowly as its size suggests on screen.
    sim.gravity = GRAVITY / scale;
    sim.run(settings.max_seconds.clamp(0.1, 60.0))
}

/// Where a pin on `joint` holds the part: the middle of its bone, in the figure's frame.
pub fn pin_point(skel: &Skeleton, pose: &Pose, joint: usize) -> Option<Vec3> {
    let world = pose.world(skel);
    let m = world.get(joint)?;
    Some(m.transform_point3(skel.joints.get(joint)?.tail * 0.5))
}

/// How far a joint bends, in the frame of the joint it hangs from (rest is the T-pose).
#[derive(Clone, Copy, Debug)]
enum Limit {
    /// The bone stays within `half` of `centre`.
    Cone { centre: Vec3, half: f32 },
    /// The bone turns about `axis` from `min` to `max`, straying at most `slack` out of plane.
    Hinge { axis: Vec3, min: f32, max: f32, slack: f32 },
    /// The bone twists about itself at most this far either way.
    Twist(f32),
}

/// The main parts: everything else rides on the nearest of these above it.
const PARTS: [&str; 10] = ["hips", "spine", "chest", "neck", "upper_arm", "forearm", "hand", "thigh", "shin", "foot"];

/// A part's share of the body's weight (after de Leva's segment tables).
fn mass_share(part: &str) -> f32 {
    match part {
        "hips" => 0.142,
        "spine" => 0.139,
        "chest" => 0.216,
        "neck" => 0.081,
        "upper_arm" => 0.028,
        "forearm" => 0.016,
        "hand" => 0.006,
        "thigh" => 0.1,
        "shin" => 0.0465,
        _ => 0.0145,
    }
}

fn base_name(name: &str) -> &str {
    name.split('.').next().unwrap_or(name)
}

/// The left side's limits; the right side's are their mirror images.
fn limits(name: &str) -> Vec<Limit> {
    let d = f32::to_radians;
    let forward = |deg: f32| Vec3::new(0.0, d(deg).cos(), d(deg).sin());
    let mut out = match base_name(name) {
        "spine" => vec![Limit::Cone { centre: forward(10.0), half: d(35.0) }, Limit::Twist(d(30.0))],
        "chest" => vec![Limit::Cone { centre: forward(10.0), half: d(30.0) }, Limit::Twist(d(30.0))],
        "neck" => vec![Limit::Cone { centre: forward(10.0), half: d(50.0) }, Limit::Twist(d(70.0))],
        "upper_arm" => vec![Limit::Cone { centre: Vec3::new(1.0, -0.25, 0.45).normalize(), half: d(105.0) }, Limit::Twist(d(80.0))],
        "forearm" => vec![Limit::Hinge { axis: Vec3::Y, min: d(-150.0), max: 0.0, slack: d(4.0) }, Limit::Twist(d(85.0))],
        "hand" => vec![Limit::Cone { centre: Vec3::X, half: d(65.0) }, Limit::Twist(d(10.0))],
        "thigh" => vec![Limit::Cone { centre: Vec3::new(0.0, -d(50.0).cos(), d(50.0).sin()), half: d(72.0) }, Limit::Twist(d(40.0))],
        "shin" => vec![Limit::Hinge { axis: Vec3::X, min: 0.0, max: d(150.0), slack: d(3.0) }, Limit::Twist(d(10.0))],
        "foot" => vec![Limit::Hinge { axis: Vec3::X, min: d(-25.0), max: d(50.0), slack: d(15.0) }, Limit::Twist(d(15.0))],
        _ => Vec::new(),
    };
    if name.ends_with(".R") {
        for l in &mut out {
            match l {
                Limit::Cone { centre, .. } => centre.x = -centre.x,
                Limit::Hinge { axis, .. } => *axis = Vec3::new(axis.x, -axis.y, -axis.z),
                Limit::Twist(_) => {}
            }
        }
    }
    out
}

#[derive(Clone, Debug)]
struct Body {
    x: Vec3,
    q: Quat,
    v: Vec3,
    w: Vec3,
    x_prev: Vec3,
    q_prev: Quat,
    inv_mass: f32,
    /// Inverse inertia in the body's frame.
    inv_inertia: Mat3,
    /// The centre of mass in the lead joint's frame.
    com: Vec3,
    /// Collision spheres: centre (from the centre of mass, body frame) and radius.
    spheres: Vec<(Vec3, f32)>,
    /// Reaches every sphere from the centre of mass.
    reach: f32,
    /// Touched the floor this substep.
    touching: bool,
    lead: usize,
    link: Option<Link>,
}

/// How a body hangs from its parent body.
#[derive(Clone, Debug)]
struct Link {
    parent: usize,
    /// The joint's pivot, from each body's centre of mass, in that body's frame.
    at_child: Vec3,
    at_parent: Vec3,
    /// The frame the joint bends in (its skeleton parent's), relative to the parent body.
    frame: Quat,
    /// The pose's own bend here, which muscle tone holds.
    rest: Quat,
    /// The bone's direction in its own frame.
    bone: Vec3,
    limits: Vec<Limit>,
}

impl Body {
    fn inv_inertia_world(&self) -> Mat3 {
        let r = Mat3::from_quat(self.q);
        r * self.inv_inertia * r.transpose()
    }

    /// Generalised inverse mass for a push along `n` at offset `r` (world) from the centre.
    fn weight(&self, r: Vec3, n: Vec3) -> f32 {
        let rn = r.cross(n);
        self.inv_mass + rn.dot(self.inv_inertia_world() * rn)
    }

    fn turn(&mut self, dw: Vec3) {
        let dq = Quat::from_xyzw(dw.x, dw.y, dw.z, 0.0) * self.q;
        let q = self.q;
        self.q = Quat::from_xyzw(q.x + 0.5 * dq.x, q.y + 0.5 * dq.y, q.z + 0.5 * dq.z, q.w + 0.5 * dq.w).normalize();
    }
}

struct Pin {
    body: usize,
    /// From the centre of mass, body frame.
    at: Vec3,
    target: Vec3,
}

struct Sim<'a> {
    skel: &'a Skeleton,
    bodies: Vec<Body>,
    /// The body each joint rides on.
    body_of: Vec<usize>,
    /// Every joint's bend when the fall started (riders keep theirs).
    start: Vec<Quat>,
    pins: Vec<Pin>,
    /// Sphere pairs that may touch: (body, sphere, body, sphere).
    pairs: Vec<(usize, usize)>,
    allowed: std::collections::HashSet<(usize, usize, usize, usize)>,
    stiffness: f32,
    gravity: f32,
}

impl<'a> Sim<'a> {
    fn new(skel: &'a Skeleton, hits: &[crate::mannequin::Hit], pose: &Pose, body: Body3d, settings: &Settings) -> Self {
        let world = pose.world(skel);
        let rot = pose.world_rot(skel);
        let total = if body == Body3d::Female { 60.0 } else { 75.0 };
        let mut body_of = Vec::with_capacity(skel.joints.len());
        let mut bodies: Vec<Body> = Vec::new();
        for (j, joint) in skel.joints.iter().enumerate() {
            let is_part = PARTS.contains(&base_name(&joint.name)) || joint.parent.is_none();
            let parent_body = joint.parent.and_then(|p| body_of.get(p).copied());
            match (is_part, parent_body) {
                (false, Some(b)) => body_of.push(b),
                _ => {
                    body_of.push(bodies.len());
                    bodies.push(Body {
                        x: Vec3::ZERO,
                        q: rot.get(j).copied().unwrap_or(Quat::IDENTITY),
                        v: Vec3::ZERO,
                        w: Vec3::ZERO,
                        x_prev: Vec3::ZERO,
                        q_prev: Quat::IDENTITY,
                        inv_mass: 1.0 / (mass_share(base_name(&joint.name)) * total),
                        inv_inertia: Mat3::IDENTITY,
                        com: Vec3::ZERO,
                        spheres: Vec::new(),
                        reach: 0.0,
                        touching: false,
                        lead: j,
                        link: None,
                    });
                }
            }
        }
        // Each body's shape: its riders' capsules, in the lead joint's frame (fingers left out:
        // they're too fine to land on).
        for hit in hits {
            let (Some(&b), Some(m), Some(joint)) = (body_of.get(hit.joint), world.get(hit.joint), skel.joints.get(hit.joint)) else { continue };
            if DIGITS.iter().any(|d| joint.name.starts_with(d)) {
                continue;
            }
            let Some(body) = bodies.get_mut(b) else { continue };
            let Some(lead) = world.get(body.lead) else { continue };
            let to_lead = lead.inverse() * *m;
            let (a, e) = (to_lead.transform_point3(hit.a), to_lead.transform_point3(hit.b));
            // The torso's parts are flatter than round: their spheres sit a little inside.
            let r = if matches!(base_name(skel.joints.get(body.lead).map_or("", |j| j.name.as_str())), "hips" | "spine" | "chest") {
                hit.radius * 0.8
            } else {
                hit.radius
            };
            let n = ((a.distance(e) / r.max(1e-3)).ceil() as usize).clamp(1, 6);
            for k in 0..=n {
                body.spheres.push((a.lerp(e, k as f32 / n as f32), r));
            }
        }
        for b in &mut bodies {
            mass_properties(b);
            let (o, r) = world.get(b.lead).map_or((Vec3::ZERO, Quat::IDENTITY), |m| (m.w_axis.truncate(), b.q));
            b.x = o + r * b.com;
        }
        let links: Vec<Option<Link>> = bodies.iter().map(|b| link(skel, &world, &rot, pose, &bodies, &body_of, b)).collect();
        for (b, l) in bodies.iter_mut().zip(links) {
            b.link = l;
        }
        let mut sim = Sim {
            skel,
            bodies,
            body_of,
            start: pose.local.clone(),
            pins: Vec::new(),
            pairs: Vec::new(),
            allowed: Default::default(),
            stiffness: 0.0,
            gravity: GRAVITY,
        };
        sim.stiffness = FULL_TONE * settings.tone.clamp(0.0, 1.0).powi(2);
        sim.pins = settings.pins.iter().filter_map(|name| sim.pin(pose, name)).collect();
        sim.collision_pairs();
        sim.start_moving(settings);
        sim
    }

    fn pin(&self, pose: &Pose, name: &str) -> Option<Pin> {
        let j = self.skel.find(name)?;
        let b = *self.body_of.get(j)?;
        let target = pin_point(self.skel, pose, j)?;
        let body = self.bodies.get(b)?;
        Some(Pin { body: b, at: body.q.inverse() * (target - body.x), target })
    }

    /// Body pairs that aren't joined, and the sphere pairs already touching in the pose (left
    /// alone: the torso's wide spheres overlap the thighs' at the hips).
    fn collision_pairs(&mut self) {
        let n = self.bodies.len();
        for a in 0..n {
            for b in a + 1..n {
                let joined = |x: usize, y: usize| self.bodies.get(x).and_then(|bd| bd.link.as_ref()).is_some_and(|l| l.parent == y);
                if joined(a, b) || joined(b, a) {
                    continue;
                }
                self.pairs.push((a, b));
                let (Some(ba), Some(bb)) = (self.bodies.get(a), self.bodies.get(b)) else { continue };
                for (i, (sa, ra)) in ba.spheres.iter().enumerate() {
                    for (k, (sb, rb)) in bb.spheres.iter().enumerate() {
                        if (ba.x + ba.q * *sa).distance(bb.x + bb.q * *sb) < ra + rb {
                            self.allowed.insert((a, i, b, k));
                        }
                    }
                }
            }
        }
    }

    /// The push, and (unless the muscles hold it) straight knees starting to buckle, as a
    /// person's do when they go limp.
    fn start_moving(&mut self, settings: &Settings) {
        let top = self.bodies.iter().map(|b| b.x.y).fold(0.1f32, f32::max);
        let push = settings.push.dir();
        for b in &mut self.bodies {
            b.v = push * 1.2 * (b.x.y / top).clamp(0.0, 1.0);
        }
        let limp = 1.0 - settings.tone.clamp(0.0, 1.0);
        for i in 0..self.bodies.len() {
            let Some(name) = self.bodies.get(i).and_then(|b| self.skel.joints.get(b.lead)).map(|j| j.name.clone()) else { continue };
            if base_name(&name) != "shin" {
                continue;
            }
            if let Some(axis) = self.hinge_axis_world(i)
                && let Some(b) = self.bodies.get_mut(i)
            {
                b.w += axis * 2.5 * limp;
            }
        }
    }

    fn hinge_axis_world(&self, i: usize) -> Option<Vec3> {
        let b = self.bodies.get(i)?;
        let l = b.link.as_ref()?;
        let p = self.bodies.get(l.parent)?;
        l.limits.iter().find_map(|lim| match lim {
            Limit::Hinge { axis, .. } => Some(p.q * l.frame * *axis),
            _ => None,
        })
    }

    fn run(&mut self, max_seconds: f32) -> Fall {
        let mut frames = vec![self.pose()];
        let max_frames = (max_seconds * FPS).ceil() as usize;
        while frames.len() <= max_frames {
            for _ in 0..STEPS_PER_FRAME {
                for _ in 0..SUBSTEPS {
                    self.substep(STEP / SUBSTEPS as f32);
                }
            }
            if !self.bodies.iter().all(|b| b.x.is_finite() && b.q.is_finite()) {
                return Fall { frames, settled: false };
            }
            frames.push(self.pose());
            if frames.len() > STILL_FRAMES && still(&frames) {
                return Fall { frames, settled: true };
            }
        }
        Fall { frames, settled: false }
    }

    fn substep(&mut self, h: f32) {
        for b in &mut self.bodies {
            b.touching = false;
            b.x_prev = b.x;
            b.q_prev = b.q;
            b.v.y -= self.gravity * h;
            b.x += b.v * h;
            let w = b.w * h;
            b.turn(w);
        }
        for i in 0..self.bodies.len() {
            self.hold_joint(i, h);
            self.bend_within_limits(i, h);
            self.hold_tone(i, h);
        }
        for k in 0..self.pins.len() {
            self.hold_pin(k, h);
        }
        for i in 0..self.bodies.len() {
            self.floor(i, h);
        }
        self.collide(h);
        for b in &mut self.bodies {
            b.v = (b.x - b.x_prev) / h;
            let dq = b.q * b.q_prev.inverse();
            let w = Vec3::new(dq.x, dq.y, dq.z) * (2.0 / h);
            b.w = if dq.w < 0.0 { -w } else { w };
            let (lin, ang) = if b.touching { (LINEAR_DAMPING + FLOOR_DAMPING.0, ANGULAR_DAMPING + FLOOR_DAMPING.1) } else { (LINEAR_DAMPING, ANGULAR_DAMPING) };
            b.v /= 1.0 + lin * h;
            b.w /= 1.0 + ang * h;
        }
        self.damp_joints(h);
    }

    /// Moves point `ra` of body `a` (from its centre, world) by `corr` relative to point `rb` of
    /// body `b` (or the fixed world), sharing the move by their weights.
    fn positional(&mut self, a: usize, ra: Vec3, b: Option<(usize, Vec3)>, corr: Vec3, h: f32, compliance: f32) {
        let c = corr.length();
        if c < 1e-9 {
            return;
        }
        let n = corr / c;
        let wa = self.bodies.get(a).map_or(0.0, |x| x.weight(ra, n));
        let wb = b.and_then(|(b, rb)| self.bodies.get(b).map(|x| x.weight(rb, n))).unwrap_or(0.0);
        let denom = wa + wb + compliance / (h * h);
        if denom <= 1e-12 {
            return;
        }
        let p = n * (c / denom);
        if let Some(x) = self.bodies.get_mut(a) {
            x.x += p * x.inv_mass;
            let dw = x.inv_inertia_world() * ra.cross(p);
            x.turn(dw);
        }
        if let Some((b, rb)) = b
            && let Some(x) = self.bodies.get_mut(b)
        {
            x.x -= p * x.inv_mass;
            let dw = x.inv_inertia_world() * rb.cross(p);
            x.turn(-dw);
        }
    }

    /// Turns body `a` by `angle` about world `axis` relative to body `b`.
    fn angular(&mut self, a: usize, b: usize, axis: Vec3, angle: f32, h: f32, compliance: f32) {
        let Some(n) = axis.try_normalize() else { return };
        if angle.abs() < 1e-7 || !angle.is_finite() {
            return;
        }
        let w = |i: usize| self.bodies.get(i).map_or(0.0, |x| n.dot(x.inv_inertia_world() * n));
        let denom = w(a) + w(b) + compliance / (h * h);
        if denom <= 1e-12 {
            return;
        }
        let impulse = n * (angle / denom);
        if let Some(x) = self.bodies.get_mut(a) {
            let dw = x.inv_inertia_world() * impulse;
            x.turn(dw);
        }
        if let Some(x) = self.bodies.get_mut(b) {
            let dw = x.inv_inertia_world() * impulse;
            x.turn(-dw);
        }
    }

    /// Keeps a body's pivot on its parent's.
    fn hold_joint(&mut self, i: usize, h: f32) {
        let Some(b) = self.bodies.get(i) else { return };
        let Some(l) = b.link.as_ref() else { return };
        let Some(p) = self.bodies.get(l.parent) else { return };
        let (rc, rp) = (b.q * l.at_child, p.q * l.at_parent);
        let corr = (p.x + rp) - (b.x + rc);
        let parent = l.parent;
        self.positional(i, rc, Some((parent, rp)), corr, h, 0.0);
    }

    /// Bends a joint back inside its limits (each limit reads the bodies as the last left them).
    fn bend_within_limits(&mut self, i: usize, h: f32) {
        let Some(l) = self.bodies.get(i).and_then(|b| b.link.as_ref()) else { return };
        let (parent, bone, frame_in_parent, limits) = (l.parent, l.bone, l.frame, l.limits.clone());
        for limit in limits {
            let (Some(b), Some(p)) = (self.bodies.get(i), self.bodies.get(parent)) else { return };
            let frame = p.q * frame_in_parent;
            let d = frame.inverse() * (b.q * bone);
            match limit {
                Limit::Cone { centre, half } => {
                    let ang = d.angle_between(centre);
                    if ang > half
                        && let Some(axis) = d.cross(centre).try_normalize()
                    {
                        self.angular(i, parent, frame * axis, ang - half, h, 0.0);
                    }
                }
                Limit::Hinge { axis, min, max, slack } => self.hinge(i, parent, (frame, bone, d), (axis, min, max, slack), h),
                Limit::Twist(max) => {
                    let twist = twist_about(frame.inverse() * b.q, bone);
                    let held = twist.clamp(-max, max);
                    if held != twist {
                        let axis = b.q * bone;
                        self.angular(i, parent, axis, held - twist, h, 0.0);
                    }
                }
            }
        }
    }

    /// An elbow, knee or ankle: back into its plane, then within its range.
    fn hinge(&mut self, i: usize, parent: usize, (frame, bone, d): (Quat, Vec3, Vec3), (axis, min, max, slack): (Vec3, f32, f32, f32), h: f32) {
        let s = d.dot(axis).clamp(-1.0, 1.0);
        let off = s.asin();
        if off.abs() > slack
            && let Some(n) = d.cross(axis).try_normalize()
        {
            self.angular(i, parent, frame * n, -off.signum() * (off.abs() - slack), h, 0.0);
        }
        let Some(p) = (d - axis * s).try_normalize() else { return };
        let ang = axis.dot(bone.cross(p)).atan2(bone.dot(p));
        let held = ang.clamp(min, max);
        if held != ang {
            self.angular(i, parent, frame * axis, held - ang, h, 0.0);
        }
    }

    /// Muscle tone: a spring at every joint towards the pose it started in.
    fn hold_tone(&mut self, i: usize, h: f32) {
        if self.stiffness <= 0.0 {
            return;
        }
        let Some(b) = self.bodies.get(i) else { return };
        let Some(l) = b.link.as_ref() else { return };
        let Some(p) = self.bodies.get(l.parent) else { return };
        let target = p.q * l.frame * l.rest;
        let mut dq = target * b.q.inverse();
        if dq.w < 0.0 {
            dq = -dq;
        }
        let (axis, angle) = dq.to_axis_angle();
        let parent = l.parent;
        self.angular(i, parent, axis, angle, h, 1.0 / self.stiffness);
    }

    fn hold_pin(&mut self, k: usize, h: f32) {
        let Some(pin) = self.pins.get(k) else { return };
        let Some(b) = self.bodies.get(pin.body) else { return };
        let r = b.q * pin.at;
        let corr = pin.target - (b.x + r);
        let body = pin.body;
        self.positional(body, r, None, corr, h, 0.0);
    }

    /// Lifts any part below the floor back onto it, with friction against sliding.
    fn floor(&mut self, i: usize, h: f32) {
        let Some(n) = self.bodies.get(i).map(|b| b.spheres.len()) else { return };
        for s in 0..n {
            let Some(b) = self.bodies.get(i) else { return };
            let Some(&(c, r)) = b.spheres.get(s) else { continue };
            let centre = b.x + b.q * c;
            let depth = r - centre.y;
            if depth <= 0.0 {
                continue;
            }
            let local = c - b.q.inverse() * (Vec3::Y * r);
            let at = b.q * local;
            self.positional(i, at, None, Vec3::Y * depth, h, 0.0);
            if let Some(b) = self.bodies.get_mut(i) {
                b.touching = true;
            }
            let Some(b) = self.bodies.get(i) else { return };
            let now = b.x + b.q * local;
            let before = b.x_prev + b.q_prev * local;
            let slide = now - before;
            let slide = Vec3::new(slide.x, 0.0, slide.z);
            let len = slide.length();
            if len > 1e-9 {
                let at = b.q * local;
                self.positional(i, at, None, -slide * (FRICTION * depth / len).min(1.0), h, 0.0);
            }
        }
    }

    /// Pushes apart parts that run into each other.
    fn collide(&mut self, h: f32) {
        for k in 0..self.pairs.len() {
            let Some(&(a, b)) = self.pairs.get(k) else { continue };
            let (Some(ba), Some(bb)) = (self.bodies.get(a), self.bodies.get(b)) else { continue };
            if ba.x.distance(bb.x) > ba.reach + bb.reach {
                continue;
            }
            let (na, nb) = (ba.spheres.len(), bb.spheres.len());
            for i in 0..na {
                for j in 0..nb {
                    if self.allowed.contains(&(a, i, b, j)) {
                        continue;
                    }
                    let (Some(ba), Some(bb)) = (self.bodies.get(a), self.bodies.get(b)) else { return };
                    let (Some(&(sa, ra)), Some(&(sb, rb))) = (ba.spheres.get(i), bb.spheres.get(j)) else { continue };
                    let (ca, cb) = (ba.x + ba.q * sa, bb.x + bb.q * sb);
                    let gap = ca.distance(cb);
                    let depth = ra + rb - gap;
                    if depth <= 0.0 {
                        continue;
                    }
                    let n = (cb - ca).try_normalize().unwrap_or(Vec3::Y);
                    let (pa, pb) = (ca + n * ra - ba.x, cb - n * rb - bb.x);
                    self.positional(a, pa, Some((b, pb)), -n * depth, h, 0.0);
                }
            }
        }
    }

    /// Soft tissue: each joint's bending slows down.
    fn damp_joints(&mut self, h: f32) {
        let k = (JOINT_DAMPING * h).min(1.0);
        for i in 0..self.bodies.len() {
            let Some(b) = self.bodies.get(i) else { continue };
            let Some(parent) = b.link.as_ref().map(|l| l.parent) else { continue };
            let Some(p) = self.bodies.get(parent) else { continue };
            let rel = (b.w - p.w) * k;
            let Some(n) = rel.try_normalize() else { continue };
            let (wb, wp) = (n.dot(b.inv_inertia_world() * n), n.dot(p.inv_inertia_world() * n));
            if wb + wp <= 1e-12 {
                continue;
            }
            let (fb, fp) = (wb / (wb + wp), wp / (wb + wp));
            if let Some(b) = self.bodies.get_mut(i) {
                b.w -= rel * fb;
            }
            if let Some(p) = self.bodies.get_mut(parent) {
                p.w += rel * fp;
            }
        }
    }

    /// The figure's pose now: each part's turn, riders keeping their own bend.
    fn pose(&self) -> Pose {
        let n = self.skel.joints.len();
        let mut world = vec![Quat::IDENTITY; n];
        let mut local = self.start.clone();
        for (j, joint) in self.skel.joints.iter().enumerate() {
            let parent = joint.parent.and_then(|p| world.get(p).copied()).unwrap_or(Quat::IDENTITY);
            let body = self.body_of.get(j).and_then(|&b| self.bodies.get(b));
            match body.filter(|b| b.lead == j) {
                Some(b) => {
                    if let Some(w) = world.get_mut(j) {
                        *w = b.q;
                    }
                    if let Some(l) = local.get_mut(j) {
                        *l = (parent.inverse() * b.q).normalize();
                    }
                }
                None => {
                    let own = local.get(j).copied().unwrap_or(Quat::IDENTITY);
                    if let Some(w) = world.get_mut(j) {
                        *w = (parent * own).normalize();
                    }
                }
            }
        }
        let hips = self.bodies.first();
        let origin = hips.map_or(Vec3::ZERO, |b| b.x - b.q * b.com);
        let offset = self.skel.joints.first().map_or(Vec3::ZERO, |j| j.offset);
        Pose { root: origin - offset, local }
    }
}

/// The last [`STILL_FRAMES`] frames barely differ from each other (net, end to end).
fn still(frames: &[Pose]) -> bool {
    let (Some(now), Some(then)) = (frames.last(), frames.len().checked_sub(STILL_FRAMES + 1).and_then(|i| frames.get(i))) else { return false };
    now.root.distance(then.root) < STILL.0 && now.local.iter().zip(&then.local).all(|(a, b)| a.angle_between(*b) < STILL.1)
}

/// How far rotation `q` twists about `axis` (radians, −π to π).
fn twist_about(q: Quat, axis: Vec3) -> f32 {
    let proj = Vec3::new(q.x, q.y, q.z).dot(axis);
    let t = 2.0 * proj.atan2(q.w);
    if t > std::f32::consts::PI {
        t - std::f32::consts::TAU
    } else if t < -std::f32::consts::PI {
        t + std::f32::consts::TAU
    } else {
        t
    }
}

/// Mass centre, inertia and reach from a body's spheres (each as heavy as its volume).
fn mass_properties(b: &mut Body) {
    if b.spheres.is_empty() {
        b.spheres.push((Vec3::ZERO, 0.03));
    }
    let mass = 1.0 / b.inv_mass;
    let volume: f32 = b.spheres.iter().map(|(_, r)| r.powi(3)).sum();
    let com = b.spheres.iter().map(|(c, r)| *c * r.powi(3)).fold(Vec3::ZERO, |a, v| a + v) / volume.max(1e-12);
    let mut inertia = Mat3::ZERO;
    for (c, r) in &b.spheres {
        let m = mass * r.powi(3) / volume.max(1e-12);
        let p = *c - com;
        inertia += Mat3::from_diagonal(Vec3::splat(m * p.length_squared() + 0.4 * m * r * r)) - Mat3::from_cols(p * (m * p.x), p * (m * p.y), p * (m * p.z));
    }
    b.inv_inertia = if inertia.determinant().abs() > 1e-15 { inertia.inverse() } else { Mat3::from_diagonal(Vec3::splat(1.0 / (0.4 * mass * 0.01))) };
    for (c, _) in &mut b.spheres {
        *c -= com;
    }
    b.reach = b.spheres.iter().map(|(c, r)| c.length() + r).fold(0.0, f32::max);
    b.com = com;
}

/// How body `b` hangs from the body its lead joint's skeleton parent rides on.
fn link(skel: &Skeleton, world: &[glam::Mat4], rot: &[Quat], pose: &Pose, bodies: &[Body], body_of: &[usize], b: &Body) -> Option<Link> {
    let joint = skel.joints.get(b.lead)?;
    let sp = joint.parent?;
    let parent = *body_of.get(sp)?;
    let pb = bodies.get(parent)?;
    let pivot = world.get(b.lead)?.w_axis.truncate();
    let parent_origin = world.get(pb.lead)?.w_axis.truncate();
    Some(Link {
        parent,
        at_child: -b.com,
        at_parent: pb.q.inverse() * (pivot - parent_origin) - pb.com,
        frame: (pb.q.inverse() * *rot.get(sp)?).normalize(),
        rest: pose.local.get(b.lead).copied().unwrap_or(Quat::IDENTITY),
        bone: joint.tail.try_normalize().unwrap_or(Vec3::Y),
        limits: limits(&joint.name),
    })
}

#[cfg(test)]
mod tests;
