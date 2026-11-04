//! `scene3d.matchPhoto`: poses a 3D figure from a photo in the document. The photo is the canvas
//! as it looks without its 3D layers (or one layer's pixels), the body or hands are tracked with
//! `photocraft-posetrack` on a background job, and the result is one undo step.

use photocraft_doc::{Document, LayerContent, LayerId};
use photocraft_posetrack::{self as posetrack, Image};
use photocraft_scene3d::glam::Vec3;
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

/// What the tracker found.
enum Found {
    Body { world: Vec<Vec3>, visible: Vec<f32> },
    /// Each hand's landmarks and whether it is a right hand.
    Hands(Vec<(Vec<Vec3>, bool)>),
}

/// The photo as RGB over white, at most [`MAX_SIDE`] on its long side: `source`'s pixels, or the
/// canvas without its 3D layers.
fn photo(doc: &Document, source: Option<LayerId>) -> Result<(Vec<[f32; 3]>, usize, usize)> {
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
            let figures: Vec<LayerId> = doc.walk().into_iter().filter(|(_, _, l)| matches!(l.content, LayerContent::Scene3d(_))).map(|(_, _, l)| l.id).collect();
            for id in figures {
                if let Some(l) = clean.layer_mut(id) {
                    l.visible = false;
                }
            }
            photocraft_compose::render(&clean, area).px
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
    Ok((out, sw, sh))
}

fn track(part: Part, rgb: &[[f32; 3]], w: usize, h: usize) -> Result<Found> {
    let img = Image { width: w, height: h, rgb };
    match part {
        Part::Body => {
            let b = posetrack::body(img).map_err(EngineError::Other)?.ok_or_else(|| EngineError::Other("No person found in the image".into()))?;
            Ok(Found::Body { world: b.world, visible: b.visible })
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
            let (rgb, w, h) = photo(&doc, source)?;
            ctx.check()?;
            ctx.progress(0.3, "Finding the pose");
            let found = track(part, &rgb, w, h)?;
            ctx.check()?;
            ctx.progress(1.0, "Posing the figure");
            Ok(found)
        },
        move |s, found| apply(s, id, fi, part, wrist, found, label),
    )
}

fn apply(s: &mut Session, id: LayerId, fi: usize, part: Part, wrist: bool, found: Found, label: &str) -> Result<Value> {
    let mut matched: Vec<&str> = Vec::new();
    let p = json!({ "layer": id.0 });
    let out = with_scene(s, &p, CMD, label, |scene, _, _| {
        let camera = scene.camera.clone();
        let fig = scene.figures.get_mut(fi).ok_or_else(|| bad(CMD, format!("the layer has no figure {fi}")))?;
        match (&found, part) {
            (Found::Body { world, visible }, _) => {
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
    let mut out = out;
    if let Some(o) = out.as_object_mut() {
        o.insert("figure".into(), json!(fi));
        o.insert("matched".into(), json!(matched));
    }
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
