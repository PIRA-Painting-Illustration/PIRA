//! Body and hand landmarks from a photo, with Google's MediaPipe models (Apache 2.0; the ONNX
//! conversions from OpenCV's model zoo, `assets/models/mediapipe`), run on the CPU with tract.
//! Two stages each, as MediaPipe does: a detector finds the palm or the person, then a landmark
//! model reads a turned square crop around it.
//!
//! The pre- and post-processing follows OpenCV's reference code, rewritten around one
//! turned-crop sampler instead of crop, pad, rotate and resize steps. Ported from PIRA Pose.
//!
//! The models are built in (about 25 MB) and loaded once, on first use (a second or two). The
//! browser build has no models: [`AVAILABLE`] is false and every call fails.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use glam::{Vec2, Vec3};

/// Whether this build can track (false in the browser).
pub const AVAILABLE: bool = cfg!(not(target_arch = "wasm32"));

/// Why tracking fails in the browser build.
pub const UNAVAILABLE: &str = "pose tracking isn't available in the browser version; use the desktop or Android app";

/// An RGB image, row by row, values 0–1 (sRGB).
#[derive(Clone, Copy, Debug)]
pub struct Image<'a> {
    pub width: usize,
    pub height: usize,
    pub rgb: &'a [[f32; 3]],
}

impl Image<'_> {
    fn px(&self, x: i64, y: i64) -> Option<[f32; 3]> {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        self.rgb.get(y as usize * self.width + x as usize).copied()
    }
}

/// A tracked hand: 21 points in image pixels and in metres (x right, y up, z towards the viewer).
#[derive(Clone, Debug)]
pub struct Hand {
    pub image: Vec<Vec2>,
    pub world: Vec<Vec3>,
    /// The model's handedness score (MediaPipe's, trained on mirrored selfies: near 1 means a
    /// left hand in an ordinary photo).
    pub right: f32,
    pub score: f32,
}

impl Hand {
    /// Whether this is a right hand, and whether that's a confident call.
    ///
    /// The model's own score comes first. MediaPipe trains it on mirrored (selfie) images, so for
    /// an ordinary photo a high score means a left hand. When the score is unsure, the hand's
    /// shape decides: given the thumb side and the fingers' direction, the palm is on opposite
    /// sides for left and right hands, and fingers curl towards the palm. The shape comes last
    /// because it rests on depth, which a single photo can get backwards.
    pub fn is_right(&self) -> (bool, bool) {
        if (self.right - 0.5).abs() > 0.25 {
            return (self.right < 0.5, true);
        }
        let p = |i: usize| self.world.get(i).copied().unwrap_or(Vec3::ZERO);
        let fwd = p(9) - p(0);
        let n = (p(5) - p(17)).cross(fwd).normalize_or_zero();
        let size = fwd.length().max(1e-4);
        // Left hands curl towards −n, right hands towards +n.
        let curl: f32 = [(5, 8), (9, 12), (13, 16), (17, 20)].iter().map(|&(a, b)| (p(b) - p(a)).dot(n)).sum::<f32>() / (4.0 * size);
        if curl.abs() > 0.15 { (curl > 0.0, true) } else { (self.right < 0.5, false) }
    }
}

/// A tracked body: MediaPipe's 33 landmarks, in image pixels and in metres (x right, y up,
/// z towards the viewer, origin between the hips), with how visible each one is (0–1).
#[derive(Clone, Debug)]
pub struct Body {
    pub image: Vec<Vec2>,
    pub world: Vec<Vec3>,
    pub visible: Vec<f32>,
    pub score: f32,
}

/// Lines between hand landmarks, for drawing what was tracked.
pub const HAND_LINKS: &[(usize, usize)] = &[
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 4),
    (0, 5),
    (5, 6),
    (6, 7),
    (7, 8),
    (5, 9),
    (9, 10),
    (10, 11),
    (11, 12),
    (9, 13),
    (13, 14),
    (14, 15),
    (15, 16),
    (13, 17),
    (0, 17),
    (17, 18),
    (18, 19),
    (19, 20),
];

/// Lines between body landmarks.
pub const BODY_LINKS: &[(usize, usize)] = &[
    (11, 12),
    (11, 13),
    (13, 15),
    (12, 14),
    (14, 16),
    (11, 23),
    (12, 24),
    (23, 24),
    (23, 25),
    (25, 27),
    (24, 26),
    (26, 28),
    (27, 31),
    (28, 32),
    (27, 29),
    (28, 30),
    (15, 19),
    (16, 20),
    (0, 11),
    (0, 12),
];

/// The hands in `img`, best first (at most two).
pub fn hands(img: Image<'_>) -> Result<Vec<Hand>, String> {
    imp::tracker()?.hands(img)
}

/// The most confident person in `img`, if any.
pub fn body(img: Image<'_>) -> Result<Option<Body>, String> {
    imp::tracker()?.body(img)
}

/// Loads the models now (the first call to [`hands`] or [`body`] otherwise does).
pub fn warm_up() -> Result<(), String> {
    imp::tracker().map(|_| ())
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::{Body, Hand, Image, UNAVAILABLE};

    pub struct Tracker;

    impl Tracker {
        pub fn hands(&self, _img: Image<'_>) -> Result<Vec<Hand>, String> {
            Err(UNAVAILABLE.into())
        }

        pub fn body(&self, _img: Image<'_>) -> Result<Option<Body>, String> {
            Err(UNAVAILABLE.into())
        }
    }

    pub fn tracker() -> Result<&'static Tracker, String> {
        Err(UNAVAILABLE.into())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::{Body, Hand, Image};
    use glam::{Vec2, Vec3};
    use std::sync::{Arc, OnceLock};
    use tract_onnx::prelude::*;

    type Model = Arc<TypedRunnableModel>;

    const PALM: &[u8] = include_bytes!("../../../assets/models/mediapipe/palm_detection_mediapipe_2023feb.onnx");
    const HAND: &[u8] = include_bytes!("../../../assets/models/mediapipe/handpose_estimation_mediapipe_2023feb.onnx");
    const PERSON: &[u8] = include_bytes!("../../../assets/models/mediapipe/person_detection_mediapipe_2023mar.onnx");
    const POSE: &[u8] = include_bytes!("../../../assets/models/mediapipe/pose_estimation_mediapipe_2023mar.onnx");

    pub struct Tracker {
        palm: Model,
        hand: Model,
        person: Model,
        pose: Model,
        palm_anchors: Vec<Vec2>,
        person_anchors: Vec<Vec2>,
    }

    /// The tracker, loaded on first use.
    pub fn tracker() -> Result<&'static Tracker, String> {
        static TRACKER: OnceLock<Result<Tracker, String>> = OnceLock::new();
        TRACKER.get_or_init(|| Tracker::load().map_err(|e| format!("could not load the pose models: {e}"))).as_ref().map_err(Clone::clone)
    }

    /// A square region of the image, turned so that `down` points down the crop.
    #[derive(Clone, Copy, Debug)]
    pub(super) struct Crop {
        pub center: Vec2,
        pub side: f32,
        pub down: Vec2,
    }

    impl Crop {
        pub fn right(self) -> Vec2 {
            Vec2::new(self.down.y, -self.down.x)
        }

        /// Crop coordinates in 0..1 → image pixels.
        pub fn to_image(self, n: Vec2) -> Vec2 {
            self.center + self.right() * ((n.x - 0.5) * self.side) + self.down * ((n.y - 0.5) * self.side)
        }

        /// A vector in the crop's axes → the image's axes.
        pub fn rotate(self, v: Vec2) -> Vec2 {
            self.right() * v.x + self.down * v.y
        }

        /// Samples the crop as `out`×`out` RGB, bilinear, black outside the image.
        fn sample(self, img: Image<'_>, out: usize, map: impl Fn(f32) -> f32) -> Vec<[f32; 3]> {
            let px = |x: i64, y: i64| -> [f32; 3] { img.px(x, y).map_or([map(0.0); 3], |p| p.map(&map)) };
            let mut buf = Vec::with_capacity(out * out);
            // Average a few samples per output pixel when shrinking a lot, so detail doesn't alias.
            let step = self.side / out as f32;
            let taps = (step / 1.5).ceil().clamp(1.0, 4.0) as usize;
            for v in 0..out {
                for u in 0..out {
                    let mut acc = [0.0f32; 3];
                    for ty in 0..taps {
                        for tx in 0..taps {
                            let n = Vec2::new(
                                (u as f32 + (tx as f32 + 0.5) / taps as f32) / out as f32,
                                (v as f32 + (ty as f32 + 0.5) / taps as f32) / out as f32,
                            );
                            let p = self.to_image(n) - Vec2::splat(0.5);
                            let (x0, y0) = (p.x.floor(), p.y.floor());
                            let (fx, fy) = (p.x - x0, p.y - y0);
                            let (x0, y0) = (x0 as i64, y0 as i64);
                            let [a, b, c, d] = [px(x0, y0), px(x0 + 1, y0), px(x0, y0 + 1), px(x0 + 1, y0 + 1)];
                            for (i, acc) in acc.iter_mut().enumerate() {
                                *acc += (a[i] * (1.0 - fx) + b[i] * fx) * (1.0 - fy) + (c[i] * (1.0 - fx) + d[i] * fx) * fy;
                            }
                        }
                    }
                    let n = (taps * taps) as f32;
                    buf.push(acc.map(|v| v / n));
                }
            }
            buf
        }
    }

    fn nhwc(pixels: &[[f32; 3]], size: usize) -> Tensor {
        tract_ndarray::Array4::from_shape_fn((1, size, size, 3), |(_, y, x, c)| pixels.get(y * size + x).map_or(0.0, |p| p[c])).into()
    }

    fn nchw(pixels: &[[f32; 3]], size: usize) -> Tensor {
        tract_ndarray::Array4::from_shape_fn((1, 3, size, size), |(_, c, y, x)| pixels.get(y * size + x).map_or(0.0, |p| p[c])).into()
    }

    /// SSD anchor centres as MediaPipe lays them out: per stride, every cell, `per` anchors each.
    pub(super) fn anchors(size: usize, groups: &[(usize, usize)]) -> Vec<Vec2> {
        let mut out = Vec::new();
        for &(stride, per) in groups {
            let fm = size / stride;
            for y in 0..fm {
                for x in 0..fm {
                    for _ in 0..per {
                        out.push(Vec2::new((x as f32 + 0.5) / fm as f32, (y as f32 + 0.5) / fm as f32));
                    }
                }
            }
        }
        out
    }

    fn sigmoid(x: f32) -> f32 {
        1.0 / (1.0 + (-x.clamp(-100.0, 100.0)).exp())
    }

    /// One SSD detection: score, box in crop 0..1 (x1, y1, x2, y2) and keypoints in crop 0..1.
    struct Detection {
        score: f32,
        bbox: [f32; 4],
        points: Vec<Vec2>,
    }

    /// Decodes an SSD head (`raw` per anchor: box cx, cy, w, h then keypoint pairs, all in input
    /// pixels relative to the anchor) and keeps the best boxes that don't overlap.
    fn decode(raw: &[f32], scores: &[f32], anchors: &[Vec2], input: f32, threshold: f32) -> Vec<Detection> {
        let stride = raw.len() / anchors.len().max(1);
        let mut found: Vec<Detection> = Vec::new();
        if stride < 4 {
            return found;
        }
        for (i, a) in anchors.iter().enumerate() {
            let score = sigmoid(scores.get(i).copied().unwrap_or(-100.0));
            if score < threshold {
                continue;
            }
            let Some(r) = raw.get(i * stride..(i + 1) * stride) else { continue };
            let c = Vec2::new(r[0], r[1]) / input + *a;
            let wh = Vec2::new(r[2], r[3]) / input;
            let points = r[4..].as_chunks::<2>().0.iter().map(|p| Vec2::new(p[0], p[1]) / input + *a).collect();
            found.push(Detection { score, bbox: [c.x - wh.x / 2.0, c.y - wh.y / 2.0, c.x + wh.x / 2.0, c.y + wh.y / 2.0], points });
        }
        found.sort_by(|a, b| b.score.total_cmp(&a.score));
        let iou = |a: &[f32; 4], b: &[f32; 4]| {
            let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
            let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
            let inter = w * h;
            let area = |r: &[f32; 4]| (r[2] - r[0]) * (r[3] - r[1]);
            inter / (area(a) + area(b) - inter).max(1e-9)
        };
        let mut kept: Vec<Detection> = Vec::new();
        for d in found {
            if kept.iter().all(|k| iou(&k.bbox, &d.bbox) < 0.3) {
                kept.push(d);
            }
        }
        kept
    }

    /// The whole image as a square crop (letterboxed), as the detectors expect.
    fn letterbox(img: Image<'_>) -> Crop {
        let (w, h) = (img.width as f32, img.height as f32);
        Crop { center: Vec2::new(w, h) / 2.0, side: w.max(h), down: Vec2::Y }
    }

    /// MediaPipe's image axes (y down, z away from the viewer) → ours (y up, z towards the viewer).
    fn to_view(v: Vec2, z: f32) -> Vec3 {
        Vec3::new(v.x, -v.y, -z)
    }

    fn load(bytes: &[u8], shape: &[usize]) -> TractResult<Model> {
        tract_onnx::onnx().model_for_read(&mut std::io::Cursor::new(bytes))?.with_input_fact(0, f32::fact(shape).into())?.into_optimized()?.into_runnable()
    }

    fn slice(t: &TValue) -> TractResult<&[f32]> {
        t.try_as_plain_ram()?.as_slice::<f32>()
    }

    /// The first output whose shape is exactly `shape`.
    fn output<'a>(out: &'a TVec<TValue>, shape: &[usize]) -> TractResult<&'a [f32]> {
        let t = out.iter().find(|t| t.shape() == shape).ok_or_else(|| TractError::msg(format!("no output shaped {shape:?}")))?;
        slice(t)
    }

    fn check(img: Image<'_>) -> Result<(), String> {
        if img.width < 8 || img.height < 8 || img.rgb.len() < img.width * img.height {
            return Err("the image is too small to track".into());
        }
        Ok(())
    }

    impl Tracker {
        fn load() -> TractResult<Self> {
            Ok(Tracker {
                palm: load(PALM, &[1, 192, 192, 3])?,
                hand: load(HAND, &[1, 224, 224, 3])?,
                person: load(PERSON, &[1, 3, 224, 224])?,
                pose: load(POSE, &[1, 256, 256, 3])?,
                palm_anchors: anchors(192, &[(8, 2), (16, 6)]),
                person_anchors: anchors(224, &[(8, 2), (16, 2), (32, 6)]),
            })
        }

        pub fn hands(&self, img: Image<'_>) -> Result<Vec<Hand>, String> {
            check(img)?;
            self.hands_in(img).map_err(|e| format!("hand tracking failed: {e}"))
        }

        pub fn body(&self, img: Image<'_>) -> Result<Option<Body>, String> {
            check(img)?;
            self.body_in(img).map_err(|e| format!("pose tracking failed: {e}"))
        }

        fn hands_in(&self, img: Image<'_>) -> TractResult<Vec<Hand>> {
            let frame = letterbox(img);
            let input = nhwc(&frame.sample(img, 192, |v| v), 192);
            let out = self.palm.run(tvec!(input.into()))?;
            let n = self.palm_anchors.len();
            let palms = decode(output(&out, &[1, n, 18])?, output(&out, &[1, n, 1])?, &self.palm_anchors, 192.0, 0.5);
            let mut hands = Vec::new();
            for palm in palms.iter().take(2) {
                let pts: Vec<Vec2> = palm.points.iter().map(|p| frame.to_image(*p)).collect();
                let (Some(&wrist), Some(&middle)) = (pts.first(), pts.get(2)) else { continue };
                // Turn the crop so the hand points up: from the wrist (0) to the middle finger's base (2).
                let down = (wrist - middle).normalize_or(Vec2::Y);
                let probe = Crop { center: wrist, side: 1.0, down };
                let local: Vec<Vec2> = pts.iter().map(|p| Vec2::new((*p - probe.center).dot(probe.right()), (*p - probe.center).dot(down))).collect();
                let lo = local.iter().fold(Vec2::splat(f32::MAX), |a, p| a.min(*p));
                let hi = local.iter().fold(Vec2::splat(f32::MIN), |a, p| a.max(*p));
                let wh = hi - lo;
                // OpenCV's numbers: shift up by 0.4 of the palm box, then 3× its larger side.
                let c = (lo + hi) / 2.0 + Vec2::new(0.0, -0.4 * wh.y);
                let crop = Crop { center: probe.center + probe.rotate(c), side: wh.max_element() * 3.0, down };
                if let Some(h) = self.hand_in(img, crop)? {
                    hands.push(h);
                }
            }
            Ok(hands)
        }

        fn hand_in(&self, img: Image<'_>, crop: Crop) -> TractResult<Option<Hand>> {
            let input = nhwc(&crop.sample(img, 224, |v| v), 224);
            let out = self.hand.run(tvec!(input.into()))?;
            // Two [1, 63] outputs: screen landmarks come first, then world ones.
            let mut sixty_three = Vec::new();
            let mut ones = Vec::new();
            for t in out.iter() {
                if t.shape() == [1, 63] {
                    sixty_three.push(slice(t)?);
                } else if t.shape() == [1, 1] {
                    ones.push(slice(t)?.first().copied().unwrap_or(0.0));
                }
            }
            let (Some(screen), Some(world), Some(&score), Some(&right)) = (sixty_three.first(), sixty_three.get(1), ones.first(), ones.get(1)) else {
                return Err(TractError::msg("unexpected hand model outputs"));
            };
            if score < 0.5 {
                return Ok(None);
            }
            let image = screen.as_chunks::<3>().0.iter().map(|p| crop.to_image(Vec2::new(p[0], p[1]) / 224.0)).collect();
            let world = world.as_chunks::<3>().0.iter().map(|p| to_view(crop.rotate(Vec2::new(p[0], p[1])), p[2])).collect();
            Ok(Some(Hand { image, world, right, score }))
        }

        fn body_in(&self, img: Image<'_>) -> TractResult<Option<Body>> {
            let frame = letterbox(img);
            let input = nchw(&frame.sample(img, 224, |v| v * 2.0 - 1.0), 224);
            let out = self.person.run(tvec!(input.into()))?;
            let n = self.person_anchors.len();
            let people = decode(output(&out, &[1, n, 12])?, output(&out, &[1, n, 1])?, &self.person_anchors, 224.0, 0.5);
            let Some(person) = people.first() else { return Ok(None) };
            let (Some(&hip), Some(&full)) = (person.points.first(), person.points.get(1)) else { return Ok(None) };
            let (hip, full) = (frame.to_image(hip), frame.to_image(full));
            let reach = (full - hip).length();
            let crop = Crop { center: hip, side: reach * 2.0, down: (hip - full).normalize_or(Vec2::Y) };
            let input = nhwc(&crop.sample(img, 256, |v| v), 256);
            let out = self.pose.run(tvec!(input.into()))?;
            let marks = output(&out, &[1, 195])?;
            let world = output(&out, &[1, 117])?;
            let score = output(&out, &[1, 1])?.first().copied().unwrap_or(0.0);
            if score < 0.5 {
                return Ok(None);
            }
            let marks: Vec<&[f32; 5]> = marks.as_chunks::<5>().0.iter().take(33).collect();
            Ok(Some(Body {
                image: marks.iter().map(|p| crop.to_image(Vec2::new(p[0], p[1]) / 256.0)).collect(),
                visible: marks.iter().map(|p| sigmoid(p[3])).collect(),
                world: world.as_chunks::<3>().0.iter().take(33).map(|p| to_view(crop.rotate(Vec2::new(p[0], p[1])), p[2])).collect(),
                score,
            }))
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::imp::{Crop, anchors};
    use super::*;

    #[test]
    fn anchor_counts_match_the_models() {
        assert_eq!(anchors(192, &[(8, 2), (16, 6)]).len(), 2016);
        assert_eq!(anchors(224, &[(8, 2), (16, 2), (32, 6)]).len(), 2254);
    }

    #[test]
    fn an_upright_crop_maps_corners_to_corners() {
        let c = Crop { center: Vec2::new(50.0, 40.0), side: 20.0, down: Vec2::Y };
        assert!((c.to_image(Vec2::ZERO) - Vec2::new(40.0, 30.0)).length() < 1e-5);
        assert!((c.to_image(Vec2::ONE) - Vec2::new(60.0, 50.0)).length() < 1e-5);
        // Turned a quarter: the crop's down is the image's left.
        let t = Crop { down: -Vec2::X, ..c };
        assert!((t.rotate(Vec2::new(1.0, 0.0)) - Vec2::new(0.0, 1.0)).length() < 1e-5);
    }

    #[test]
    fn the_models_load_and_a_blank_image_has_nobody_in_it() {
        let rgb = vec![[0.5f32; 3]; 320 * 240];
        let img = Image { width: 320, height: 240, rgb: &rgb };
        assert!(body(img).unwrap().is_none());
        assert!(hands(img).unwrap().is_empty());
        let tiny = Image { width: 2, height: 2, rgb: &rgb };
        assert!(body(tiny).is_err());
    }
}
