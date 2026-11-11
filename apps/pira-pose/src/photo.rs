//! The reference photo behind the figures, and posing a figure like the person or hands in it
//! (as PIRA's `scene3d.matchPhoto` and `scene3d.refitPhoto`, `engine/src/cmd/scene3d_cmds/photo.rs`). The photo is the canvas:
//! its pixels are canvas pixels.

use photocraft_doc::Scene3d;
use photocraft_posetrack::{self as posetrack, Image};
use photocraft_scene3d::glam::{Vec2, Vec3};
use photocraft_scene3d::retarget;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The longest side a photo is kept at (bigger ones are scaled down when opened).
pub const MAX_PHOTO_SIDE: u32 = 2400;
/// The longest side the photo is tracked at: the detectors look at a 192–224 px view of it, so a
/// bigger copy only costs time.
const MAX_TRACK_SIDE: usize = 1600;

pub const AVAILABLE: bool = posetrack::AVAILABLE;

pub struct Photo {
    pub name: String,
    pub size: [usize; 2],
    /// sRGB, straight alpha, 8 bits per channel.
    pub rgba: Arc<Vec<u8>>,
}

impl Photo {
    pub fn decode(name: &str, bytes: &[u8]) -> Result<Photo, String> {
        let img = image::load_from_memory(bytes).map_err(|e| format!("{name}: can't read this picture ({e})"))?;
        let img = if img.width().max(img.height()) > MAX_PHOTO_SIDE {
            img.resize(MAX_PHOTO_SIDE, MAX_PHOTO_SIDE, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        if size[0] == 0 || size[1] == 0 {
            return Err(format!("{name}: the picture is empty"));
        }
        Ok(Photo { name: name.to_string(), size, rgba: Arc::new(rgba.into_raw()) })
    }
}

/// Which part to match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Body,
    /// Each hand found onto the figure's hand of the same side.
    Hands,
    /// The clearest hand found onto one of the figure's hands (true: the right).
    Hand(bool),
}

/// What the tracker found.
pub enum Found {
    /// The landmarks in 3D, their visibility and where they are on the canvas.
    Body { world: Vec<Vec3>, visible: Vec<f32>, image: Vec<Vec2> },
    /// Each hand's landmarks and whether it is a right hand.
    Hands(Vec<(Vec<Vec3>, bool)>),
}

/// Tracks `part` in the photo (on the calling thread: it takes a second or more).
pub fn track(rgba: &[u8], size: [usize; 2], part: Part, accurate: bool) -> Result<Found, String> {
    if !AVAILABLE {
        return Err(posetrack::UNAVAILABLE.into());
    }
    let [w, h] = size;
    // Over white, shrunk by a whole factor (averaging each block).
    let k = w.max(h).div_ceil(MAX_TRACK_SIDE).max(1);
    let (sw, sh) = (w.div_ceil(k), h.div_ceil(k));
    let mut rgb = vec![[0.0f32; 3]; sw * sh];
    for (i, o) in rgb.iter_mut().enumerate() {
        let (x0, y0) = ((i % sw) * k, (i / sw) * k);
        let mut acc = [0.0f32; 3];
        let mut n = 0.0f32;
        for y in y0..(y0 + k).min(h) {
            for x in x0..(x0 + k).min(w) {
                if let Some(p) = rgba.get((y * w + x) * 4..(y * w + x) * 4 + 4) {
                    let a = f32::from(p[3]) / 255.0;
                    for c in 0..3 {
                        acc[c] += f32::from(p[c]) / 255.0 * a + (1.0 - a);
                    }
                    n += 1.0;
                }
            }
        }
        *o = acc.map(|v| v / n.max(1.0));
    }
    let img = Image { width: sw, height: sh, rgb: &rgb };
    match part {
        Part::Body => {
            let quality = if accurate { posetrack::Quality::Accurate } else { posetrack::Quality::Quick };
            let b = posetrack::body_with(img, quality)?.ok_or("No person found in the photo")?;
            let image = b.image.iter().map(|p| Vec2::new(p.x, p.y) * k as f32).collect();
            Ok(Found::Body { world: b.world, visible: b.visible, image })
        }
        Part::Hands | Part::Hand(_) => {
            let hands = posetrack::hands(img)?;
            if hands.is_empty() {
                return Err("No hand found in the photo".into());
            }
            Ok(Found::Hands(hands.into_iter().map(|h| (h.world.clone(), h.is_right().0)).collect()))
        }
    }
}

/// The last body matched to the photo, kept so it can be corrected.
#[derive(Clone, Debug, PartialEq)]
pub struct PhotoMatch {
    pub figure: usize,
    /// Where the tracker put each landmark on the canvas (pixels).
    pub tracked: Vec<Vec2>,
    /// The landmarks the user has dragged, and where to.
    pub moved: BTreeMap<usize, Vec2>,
    pub world: Vec<Vec3>,
    pub visible: Vec<f32>,
    /// Per `retarget::SEGMENTS` entry: the user's depth choice (`None`: the tracker's).
    pub toward: [Option<bool>; 8],
    /// The depth each segment has now (true: its far end is nearer the camera).
    pub now: [bool; 8],
}

impl PhotoMatch {
    /// Each landmark where it is now: the user's place for it, or the tracker's.
    pub fn points(&self) -> Vec<Vec2> {
        self.tracked.iter().enumerate().map(|(i, p)| self.moved.get(&i).copied().unwrap_or(*p)).collect()
    }
}

/// Poses figure `fi` from what was found. Returns the sides matched and, for a body, the match
/// to correct later.
pub fn apply(scene: &mut Scene3d, fi: usize, part: Part, found: Found) -> Result<(Vec<&'static str>, Option<PhotoMatch>), String> {
    let camera = scene.camera.clone();
    let fig = scene.figures.get_mut(fi).ok_or("That figure is gone")?;
    let mut matched = Vec::new();
    match (&found, part) {
        (Found::Body { world, visible, .. }, _) => {
            if !retarget::match_body(fig, &camera, world, visible) {
                return Err("The person in the photo is too unclear to match".into());
            }
            matched.push("body");
        }
        (Found::Hands(hands), Part::Hand(to_right)) => {
            // Prefer a hand of the same side; otherwise the clearest one, mirrored across.
            let pick = hands.iter().find(|(_, r)| *r == to_right).or_else(|| hands.first());
            if let Some((world, is_right)) = pick
                && retarget::match_hand(fig, &camera, world, *is_right, to_right, true)
            {
                matched.push(if to_right { "right" } else { "left" });
            }
        }
        (Found::Hands(hands), _) => {
            for (world, is_right) in hands {
                let side = if *is_right { "right" } else { "left" };
                if !matched.contains(&side) && retarget::match_hand(fig, &camera, world, *is_right, *is_right, true) {
                    matched.push(side);
                }
            }
        }
    }
    if matched.is_empty() {
        return Err("The hand in the photo is too unclear to match".into());
    }
    let m = match found {
        Found::Body { world, visible, image } => {
            let depth = |i: usize| world.get(i).map_or(0.0, |p| p.z);
            let now = std::array::from_fn(|k| retarget::SEGMENTS.get(k).is_some_and(|&(_, a, b)| depth(b) - depth(a) >= 0.0));
            Some(PhotoMatch { figure: fi, tracked: image, moved: BTreeMap::new(), world, visible, toward: [None; 8], now })
        }
        Found::Hands(_) => None,
    };
    Ok((matched, m))
}

/// Poses the match's figure again from the photo's joints, with the user's corrections.
pub fn refit(scene: &mut Scene3d, m: &mut PhotoMatch) -> Result<(), String> {
    let points = m.points();
    let mut visible = m.visible.clone();
    for i in m.moved.keys() {
        if let Some(v) = visible.get_mut(*i) {
            *v = 1.0;
        }
    }
    let (world, now) =
        retarget::fit_body(&points, &m.tracked, &m.world, &visible, &m.toward).ok_or("Too little of the person is clear in the photo to fix the match")?;
    m.now = now;
    let camera = scene.camera.clone();
    let fig = scene.figures.get_mut(m.figure).ok_or("That figure is gone")?;
    if retarget::match_body(fig, &camera, &world, &visible) { Ok(()) } else { Err("The person in the photo is too unclear to match".into()) }
}
