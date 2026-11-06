//! `scene3d.matchPhoto`: poses a 3D figure from a photo in the document. The photo is the canvas
//! as it looks without its 3D layers (or one layer's pixels), the body or hands are tracked with
//! `photocraft-posetrack` on a background job, and the result is one undo step.

use photocraft_doc::{Document, LayerContent, LayerId};
use photocraft_posetrack::{self as posetrack, Image};
use photocraft_scene3d::glam::{Vec2, Vec3};
use photocraft_scene3d::retarget;
use serde_json::{Value, json};
use std::sync::Arc;

use super::{bad, figure_index, layer_id, scene_of, with_scene};
use crate::{EngineError, Result, Session, jobs};

pub(super) const CMD: &str = "scene3d.matchPhoto";

/// The longest side the photo is tracked at: the detectors look at a 192–224 px view of it, so a
/// bigger copy only costs time.
const MAX_SIDE: usize = 1600;

/// Which part to match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Body,
    /// Each hand found onto the figure's hand of the same side.
    Hands,
    /// The clearest hand found onto one of the figure's hands (true: the right).
    Hand(bool),
}

impl Part {
    fn of(p: &Value) -> Result<Part> {
        match p.get("part").and_then(Value::as_str).unwrap_or("body") {
            "body" => Ok(Part::Body),
            "hands" => Ok(Part::Hands),
            "left" => Ok(Part::Hand(false)),
            "right" => Ok(Part::Hand(true)),
            _ => Err(bad(CMD, "`part` must be body, hands, left or right")),
        }
    }
}

/// The last body matched to a photo, kept so the user can correct it (`scene3d.refitPhoto`).
#[derive(Clone, Debug, PartialEq)]
pub struct PhotoMatch {
    pub layer: LayerId,
    pub figure: usize,
    /// Where the tracker put each landmark on the canvas (pixels).
    pub tracked: Vec<Vec2>,
    /// The landmarks the user has dragged, and where to.
    pub moved: std::collections::BTreeMap<usize, Vec2>,
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

    fn json(&self) -> Value {
        json!({
            "layer": self.layer.0,
            "figure": self.figure,
            "points": self.points().iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
            "toward": self.now,
        })
    }
}

/// What the tracker found.
enum Found {
    /// The landmarks in 3D, their visibility and where they are on the canvas.
    Body { world: Vec<Vec3>, visible: Vec<f32>, image: Vec<Vec2> },
    /// Each hand's landmarks and whether it is a right hand.
    Hands(Vec<(Vec<Vec3>, bool)>),
}

/// The photo as RGB over white, at most [`MAX_SIDE`] on its long side: `source`'s pixels, or the
/// canvas without its 3D layers.
fn photo(doc: &Document, source: Option<LayerId>) -> Result<(Vec<[f32; 3]>, usize, usize, usize)> {
    let area = doc.bounds();
    let (w, h) = (area.width() as usize, area.height() as usize);
    if w == 0 || h == 0 {
        return Err(bad(CMD, "the document is empty"));
    }
    let rgba = match source {
        Some(id) => {
            let layer = doc.layer(id).ok_or(EngineError::NoLayer(id))?;
            let surf = layer.surface().ok_or_else(|| bad(CMD, format!("layer {} has no pixels to track", id.0)))?;
            let mut px = vec![[0.0f32; 4]; w * h];
            surf.read_rgba_into(area, &mut px);
            px
        }
        None => {
            let mut clean = doc.clone();
            let figures: Vec<LayerId> =
                doc.walk().into_iter().filter(|(_, _, l)| matches!(l.content, LayerContent::Scene3d(_))).map(|(_, _, l)| l.id).collect();
            for id in figures {
                if let Some(l) = clean.layer_mut(id) {
                    l.visible = false;
                }
            }
            photocraft_compose::render(&clean, area).into_px()
        }
    };
    let over_white = |p: [f32; 4]| {
        let a = p[3].clamp(0.0, 1.0);
        [0, 1, 2].map(|i| (p[i].clamp(0.0, 1.0) * a + (1.0 - a)).clamp(0.0, 1.0))
    };
    // Shrink by a whole factor, averaging each block.
    let k = w.max(h).div_ceil(MAX_SIDE).max(1);
    let (sw, sh) = (w.div_ceil(k), h.div_ceil(k));
    let mut out = vec![[0.0f32; 3]; sw * sh];
    for (i, o) in out.iter_mut().enumerate() {
        let (x0, y0) = ((i % sw) * k, (i / sw) * k);
        let mut acc = [0.0f32; 3];
        let mut n = 0.0f32;
        for y in y0..(y0 + k).min(h) {
            for x in x0..(x0 + k).min(w) {
                if let Some(&p) = rgba.get(y * w + x) {
                    let c = over_white(p);
                    acc = [acc[0] + c[0], acc[1] + c[1], acc[2] + c[2]];
                    n += 1.0;
                }
            }
        }
        *o = acc.map(|v| v / n.max(1.0));
    }
    Ok((out, sw, sh, k))
}

/// Tracks `part` in the photo (`k`: canvas pixels per photo pixel).
fn track(part: Part, quality: posetrack::Quality, rgb: &[[f32; 3]], w: usize, h: usize, k: usize) -> Result<Found> {
    let img = Image { width: w, height: h, rgb };
    match part {
        Part::Body => {
            let b = posetrack::body_with(img, quality).map_err(EngineError::Other)?.ok_or_else(|| EngineError::Other("No person found in the image".into()))?;
            let image = b.image.iter().map(|p| Vec2::new(p.x, p.y) * k as f32).collect();
            Ok(Found::Body { world: b.world, visible: b.visible, image })
        }
        Part::Hands | Part::Hand(_) => {
            let hands = posetrack::hands(img).map_err(EngineError::Other)?;
            if hands.is_empty() {
                return Err(EngineError::Other("No hand found in the image".into()));
            }
            Ok(Found::Hands(hands.into_iter().map(|h| (h.is_right().0, h.world)).map(|(r, w)| (w, r)).collect()))
        }
    }
}

/// `scene3d.matchPhoto`.
pub(super) fn match_photo(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_id(s, p)?;
    let part = Part::of(p)?;
    let quality = match p.get("model").and_then(Value::as_str).unwrap_or("quick") {
        "quick" => posetrack::Quality::Quick,
        "accurate" => posetrack::Quality::Accurate,
        _ => return Err(bad(CMD, "`model` must be quick or accurate")),
    };
    let wrist = match p.get("wrist") {
        None | Some(Value::Null) => true,
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "`wrist` must be true or false"))?,
    };
    let source = match p.get("source") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_u64().map(LayerId).ok_or_else(|| bad(CMD, "`source` must be a layer id"))?),
    };
    let scene = scene_of(s, id, CMD)?;
    let fi = figure_index(p, &scene, CMD)?;
    if !posetrack::AVAILABLE {
        return Err(EngineError::Other(posetrack::UNAVAILABLE.into()));
    }
    let doc: Arc<Document> = s.active().ok_or(EngineError::NoDocument)?.doc.clone();
    if let Some(src) = source {
        doc.layer(src).ok_or(EngineError::NoLayer(src))?;
    }
    let label = match part {
        Part::Body => "Match Pose to Photo",
        _ => "Match Hands to Photo",
    };
    jobs::run(
        s,
        label,
        true,
        move |ctx| {
            ctx.progress(0.1, "Reading the image");
            let (rgb, w, h, k) = photo(&doc, source)?;
            ctx.check()?;
            ctx.progress(0.3, "Finding the pose");
            let started = std::time::Instant::now();
            let found = track(part, quality, &rgb, w, h, k)?;
            let seconds = started.elapsed().as_secs_f64();
            ctx.check()?;
            ctx.progress(1.0, "Posing the figure");
            Ok((found, seconds))
        },
        move |s, (found, seconds)| {
            let mut out = apply(s, id, fi, part, wrist, found, label)?;
            if let Some(o) = out.as_object_mut() {
                o.insert("seconds".into(), json!((seconds * 100.0).round() / 100.0));
                o.insert("model".into(), json!(if quality == posetrack::Quality::Accurate { "accurate" } else { "quick" }));
            }
            Ok(out)
        },
    )
}

fn apply(s: &mut Session, id: LayerId, fi: usize, part: Part, wrist: bool, found: Found, label: &str) -> Result<Value> {
    let mut matched: Vec<&str> = Vec::new();
    let p = json!({ "layer": id.0 });
    let out = with_scene(s, &p, CMD, label, |scene, _, _| {
        let camera = scene.camera.clone();
        let fig = scene.figures.get_mut(fi).ok_or_else(|| bad(CMD, format!("the layer has no figure {fi}")))?;
        match (&found, part) {
            (Found::Body { world, visible, .. }, _) => {
                if !retarget::match_body(fig, &camera, world, visible) {
                    return Err(EngineError::Other("The person in the image is too unclear to match".into()));
                }
                matched.push("body");
            }
            (Found::Hands(hands), Part::Hand(to_right)) => {
                // Prefer a hand of the same side; otherwise the clearest one, mirrored across.
                let pick = hands.iter().find(|(_, r)| *r == to_right).or_else(|| hands.first());
                if let Some((world, is_right)) = pick
                    && retarget::match_hand(fig, &camera, world, *is_right, to_right, wrist)
                {
                    matched.push(if to_right { "right" } else { "left" });
                }
            }
            (Found::Hands(hands), _) => {
                for (world, is_right) in hands {
                    let side = if *is_right { "right" } else { "left" };
                    if !matched.contains(&side) && retarget::match_hand(fig, &camera, world, *is_right, *is_right, wrist) {
                        matched.push(side);
                    }
                }
            }
        }
        if matched.is_empty() {
            return Err(EngineError::Other("The hand in the image is too unclear to match".into()));
        }
        Ok(())
    })?;
    if let Found::Body { world, visible, image } = found {
        let now = std::array::from_fn(|k| retarget::SEGMENTS.get(k).is_some_and(|&(_, a, b)| depth(&world, b) - depth(&world, a) >= 0.0));
        let m = PhotoMatch { layer: id, figure: fi, tracked: image, moved: Default::default(), world, visible, toward: [None; 8], now };
        s.photo_match = Some(m);
    }
    let mut out = out;
    if let Some(o) = out.as_object_mut() {
        o.insert("figure".into(), json!(fi));
        o.insert("matched".into(), json!(matched));
        if let Some(m) = s.photo_match.as_ref().filter(|m| m.layer == id && m.figure == fi && matched.contains(&"body")) {
            o.insert("points".into(), m.json()["points"].clone());
        }
    }
    Ok(out)
}

fn depth(world: &[Vec3], i: usize) -> f32 {
    world.get(i).map_or(0.0, |p| p.z)
}

const REFIT: &str = "scene3d.refitPhoto";

/// `scene3d.refitPhoto`: applies a correction (a moved joint, a limb's depth, or a reset) to the
/// last body matched to a photo and poses the figure again from the photo's joints. One undo step
/// per call (a drag coalesces its calls).
pub(super) fn refit_photo(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_id(s, p)?;
    let scene = scene_of(s, id, REFIT)?;
    let fi = figure_index(p, &scene, REFIT)?;
    let Some(mut m) = s.photo_match.clone().filter(|m| m.layer == id && m.figure == fi) else {
        return Err(EngineError::Other("Match the pose to a photo first".into()));
    };
    if p.get("reset").and_then(Value::as_bool).unwrap_or(false) {
        m.moved.clear();
        m.toward = [None; 8];
    }
    if let Some(mv) = p.get("move").filter(|v| !v.is_null()) {
        let point = mv.get("point").and_then(Value::as_u64).map(|v| v as usize).filter(|i| retarget::HANDLES.contains(i));
        let to = mv.get("to").and_then(Value::as_array).and_then(|a| Some(Vec2::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)));
        let (Some(point), Some(to)) = (point, to) else {
            return Err(bad(REFIT, "`move` needs a `point` (a joint's landmark index) and `to` ([x, y])"));
        };
        m.moved.insert(point, to);
    }
    if let Some(d) = p.get("depth").filter(|v| !v.is_null()) {
        let seg = d.get("segment").and_then(Value::as_u64).map(|v| v as usize).filter(|k| *k < m.toward.len());
        let Some(seg) = seg else { return Err(bad(REFIT, "`depth` needs a `segment` from 0 to 7")) };
        if let Some(slot) = m.toward.get_mut(seg) {
            *slot = d.get("toward").and_then(Value::as_bool);
        }
    }
    let points = m.points();
    let mut visible = m.visible.clone();
    for i in m.moved.keys() {
        if let Some(v) = visible.get_mut(*i) {
            *v = 1.0;
        }
    }
    let (world, now) = retarget::fit_body(&points, &m.tracked, &m.world, &visible, &m.toward)
        .ok_or_else(|| EngineError::Other("Too little of the person is clear in the photo to fix the match".into()))?;
    m.now = now;
    with_scene(s, p, REFIT, "Fix Pose Match", |scene, _, _| {
        let camera = scene.camera.clone();
        let fig = scene.figures.get_mut(fi).ok_or_else(|| bad(REFIT, format!("the layer has no figure {fi}")))?;
        if retarget::match_body(fig, &camera, &world, &visible) {
            Ok(())
        } else {
            Err(EngineError::Other("The person in the image is too unclear to match".into()))
        }
    })?;
    let out = m.json();
    s.photo_match = Some(m);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_scene3d::skeleton::Pose;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 700, "height": 900, "background": "white"})).unwrap();
        s
    }

    fn active_scene(s: &Session) -> photocraft_doc::Scene3d {
        let d = s.active().unwrap();
        scene_of(s, d.active_layer.unwrap(), CMD).unwrap()
    }

    /// How far below level the figure's left upper arm points (1 = straight down).
    fn arm_drop(scene: &photocraft_doc::Scene3d) -> f32 {
        let fig = &scene.figures[0];
        let (skel, _) = photocraft_scene3d::model(fig.body);
        let w = Pose::of(skel, fig).world(skel);
        let i = skel.find("upper_arm.L").unwrap();
        -w[i].transform_vector3(skel.joints[i].tail).normalize().y
    }

    #[test]
    fn a_figure_takes_the_pose_of_the_person_in_the_picture() {
        let mut s = session();
        // The "photo": a mannequin with its arms out, flattened to pixels.
        s.execute("layer.new.figure3d", json!({"pose": "T-pose"})).unwrap();
        s.execute("layer.rasterize.scene3d", json!({})).unwrap();
        // A figure with its arms down, to be matched to it.
        s.execute("layer.new.figure3d", json!({"pose": "Standing"})).unwrap();
        assert!(arm_drop(&active_scene(&s)) > 0.8, "starts with its arms down");
        let r = s.execute(CMD, json!({})).unwrap();
        assert_eq!(r["matched"], json!(["body"]));
        let drop = arm_drop(&active_scene(&s));
        assert!(drop.abs() < 0.45, "the arms came up to the photo's: {drop}");
        // One undo step.
        s.execute("edit.undo", json!({})).unwrap();
        assert!(arm_drop(&active_scene(&s)) > 0.8);
    }

    #[test]
    fn the_accurate_model_matches_the_same_pose() {
        let mut s = session();
        s.execute("layer.new.figure3d", json!({"pose": "T-pose"})).unwrap();
        s.execute("layer.rasterize.scene3d", json!({})).unwrap();
        for model in ["quick", "accurate", "quick", "accurate"] {
            s.execute("layer.new.figure3d", json!({"pose": "Standing"})).unwrap();
            let r = s.execute(CMD, json!({"model": model})).unwrap();
            assert_eq!(r["matched"], json!(["body"]));
            assert_eq!(r["model"], json!(model));
            let drop = arm_drop(&active_scene(&s));
            assert!(drop.abs() < 0.45, "{model}: the arms came up to the photo's: {drop}");
            // The first run of each also loads its model; the second is the steady time.
            eprintln!("{model}: {} s", r["seconds"]);
            s.execute("layer.delete", json!({})).unwrap();
        }
        assert!(s.execute(CMD, json!({"model": "huge"})).is_err());
    }

    #[test]
    fn a_match_can_be_corrected_by_dragging_a_joint_and_flipping_a_limb() {
        let mut s = session();
        s.execute("layer.new.figure3d", json!({"pose": "T-pose"})).unwrap();
        s.execute("layer.rasterize.scene3d", json!({})).unwrap();
        s.execute("layer.new.figure3d", json!({"pose": "Standing"})).unwrap();
        // Nothing to fix before a match.
        assert!(s.execute(REFIT, json!({})).unwrap_err().to_string().contains("Match the pose"));
        let r = s.execute(CMD, json!({})).unwrap();
        let points = r["points"].as_array().unwrap().clone();
        assert_eq!(points.len(), retarget::BODY_POINTS);
        let before = arm_drop(&active_scene(&s));
        // Drag the left wrist (15) well below the shoulder: the arm swings down.
        let shoulder = &points[11];
        let to = [shoulder[0].as_f64().unwrap() + 20.0, shoulder[1].as_f64().unwrap() + 150.0];
        let r = s.execute(REFIT, json!({"move": {"point": 13, "to": to}})).unwrap();
        assert_eq!(r["points"][13][0].as_f64().unwrap().round(), to[0].round());
        let after = arm_drop(&active_scene(&s));
        assert!(after > before + 0.3, "the arm came down: {before} → {after}");
        // One undo step back to the match.
        s.execute("edit.undo", json!({})).unwrap();
        assert!((arm_drop(&active_scene(&s)) - before).abs() < 1e-3);
        // A depth flip answers with the new choice; a reset forgets every fix.
        let t0 = r["toward"][0].as_bool().unwrap();
        let r = s.execute(REFIT, json!({"depth": {"segment": 0, "toward": !t0}})).unwrap();
        assert_eq!(r["toward"][0].as_bool().unwrap(), !t0);
        let r = s.execute(REFIT, json!({"reset": true})).unwrap();
        assert_eq!(r["points"], json!(points));
        assert!(s.execute(REFIT, json!({"move": {"point": 99, "to": [0, 0]}})).is_err());
        assert!(s.execute(REFIT, json!({"depth": {"segment": 9}})).is_err());
    }

    #[test]
    fn nobody_in_the_picture_is_an_error_and_changes_nothing() {
        let mut s = session();
        s.execute("layer.new.figure3d", json!({"pose": "Standing"})).unwrap();
        let before = active_scene(&s);
        let e = s.execute(CMD, json!({})).unwrap_err().to_string();
        assert!(e.contains("No person found"), "{e}");
        assert!(s.execute(CMD, json!({"part": "hands"})).unwrap_err().to_string().contains("No hand found"));
        assert_eq!(active_scene(&s), before);
        assert!(s.execute(CMD, json!({"part": "feet"})).is_err());
        assert!(s.execute(CMD, json!({"figure": 3})).is_err());
        assert!(s.execute(CMD, json!({"source": 999_999})).is_err());
    }
}
