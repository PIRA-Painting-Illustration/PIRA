//! 3D layers: Layer › New › 3D Figure, posing (library poses, mirror, floor), the camera, the look
//! (shading, outline, light), Layer › Rasterize › 3D Layer, and `scene3d.set` for the Object tool,
//! which edits the scene directly and sends the whole of it back; `scene3d.matchPhoto` (in
//! `photo`) poses a figure from a photo in the document.
//!
//! Every edit re-renders the layer's pixels from its scene: a quick one-sample render with
//! `"draft": true` (while dragging), the smooth one otherwise.

use photocraft_doc::{Body3d, Document, Figure3d, Layer, LayerContent, LayerId, Scene3d, Scene3dLayer, Shading3d};
use photocraft_scene3d::{self as s3d, Quality, library, pose};
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

mod photo;

/// Whether `scene3d.matchPhoto` can track in this build (not in the browser).
pub const PHOTO_TRACKING: bool = photocraft_posetrack::AVAILABLE;

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

/// Re-renders a 3D layer's pixels for `doc` (canvas size, document pixel format).
pub fn refresh(doc: &Document, sc: &mut Scene3dLayer) {
    refresh_with(doc, sc, Quality::Final);
}

fn refresh_with(doc: &Document, sc: &mut Scene3dLayer, q: Quality) {
    sc.cache = Some(s3d::render_surface(&sc.scene, doc.bounds(), doc.pixel_format(), q));
}

fn layer_id(s: &Session, p: &Value) -> Result<LayerId> {
    match p.get("layer") {
        None | Some(Value::Null) => s.active().and_then(|d| d.active_layer).ok_or(EngineError::Other("no active layer".into())),
        Some(v) => v.as_u64().map(LayerId).ok_or_else(|| EngineError::Other("`layer` must be a layer id".into())),
    }
}

fn is_scene3d(s: &Session) -> std::result::Result<(), String> {
    let d = s.active().ok_or("no document open")?;
    let id = d.active_layer.ok_or("no active layer")?;
    match d.doc.layer(id).map(|l| &l.content) {
        Some(LayerContent::Scene3d(_)) => Ok(()),
        _ => Err("the active layer is not a 3D layer".into()),
    }
}

fn has_doc(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}

fn always(_: &Session) -> std::result::Result<(), String> {
    Ok(())
}

fn draft(p: &Value) -> Quality {
    if p.get("draft").and_then(Value::as_bool).unwrap_or(false) { Quality::Draft } else { Quality::Final }
}

/// The scene of 3D layer `id`.
fn scene_of(s: &Session, id: LayerId, cmd: &str) -> Result<Scene3d> {
    let d = s.active().ok_or(EngineError::NoDocument)?;
    match d.doc.layer(id).map(|l| &l.content) {
        Some(LayerContent::Scene3d(sc)) => Ok(sc.scene.clone()),
        Some(_) => Err(bad(cmd, format!("layer {} is not a 3D layer", id.0))),
        None => Err(EngineError::NoLayer(id)),
    }
}

/// Edits the scene of the 3D layer `p.layer` (default: the active layer) in one history step.
/// `f` gets the scene and the canvas size.
fn with_scene(s: &mut Session, p: &Value, cmd: &str, label: &str, f: impl FnOnce(&mut Scene3d, u32, u32) -> Result<()>) -> Result<Value> {
    let id = layer_id(s, p)?;
    let q = draft(p);
    s.edit(label, |doc, _| {
        let snapshot = doc.clone();
        let (w, h) = (doc.size.width, doc.size.height);
        let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
        let LayerContent::Scene3d(sc) = &mut l.content else { return Err(bad(cmd, format!("layer {} is not a 3D layer", id.0))) };
        f(&mut sc.scene, w, h)?;
        refresh_with(&snapshot, sc, q);
        Ok(())
    })?;
    Ok(json!({ "layer": id.0 }))
}

fn figure_index(p: &Value, scene: &Scene3d, cmd: &str) -> Result<usize> {
    let i = match p.get("figure") {
        None | Some(Value::Null) => 0,
        Some(v) => v.as_u64().ok_or_else(|| bad(cmd, "`figure` must be a figure index"))? as usize,
    };
    if i < scene.figures.len() { Ok(i) } else { Err(bad(cmd, format!("the layer has no figure {i}"))) }
}

fn body(p: &Value, cmd: &str) -> Result<Body3d> {
    match p.get("body") {
        None | Some(Value::Null) => Ok(Body3d::Male),
        Some(v) => v.as_str().and_then(Body3d::from_id).ok_or_else(|| bad(cmd, "`body` must be \"male\" or \"female\"")),
    }
}

fn pose_param<'a>(p: &'a Value, cmd: &str) -> Result<Option<&'a str>> {
    match p.get("pose") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let id = v.as_str().ok_or_else(|| bad(cmd, "`pose` must be a pose id"))?;
            if library::entries().iter().any(|e| e.id.eq_ignore_ascii_case(id)) { Ok(Some(id)) } else { Err(bad(cmd, format!("no pose \"{id}\""))) }
        }
    }
}

const NEW: &str = "layer.new.figure3d";

/// Layer › New › 3D Figure: a 3D layer holding one mannequin, framed to fill most of the canvas.
fn new_figure(s: &mut Session, p: &Value) -> Result<Value> {
    let body = body(p, NEW)?;
    let pose = pose_param(p, NEW)?;
    let name = match p.get("name") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_str().ok_or_else(|| bad(NEW, "`name` must be a string"))?.to_string()),
    };
    let id = s.edit("New 3D Figure", |doc, active| {
        let mut scene = Scene3d { figures: vec![s3d::new_figure(body, pose)], ..Scene3d::default() };
        s3d::frame(&mut scene, doc.size.width, doc.size.height, 0.82);
        let mut sc = Scene3dLayer::new(scene);
        refresh(doc, &mut sc);
        let name = name.clone().unwrap_or_else(|| doc.next_layer_name("3D Figure"));
        let layer = Layer::new(name, LayerContent::Scene3d(sc));
        let id = crate::layer_place::insert_new(doc, *active, layer, crate::layer_place::Place::Auto);
        *active = Some(id);
        Ok(id)
    })?;
    Ok(json!({ "layer": id.0 }))
}

fn add_figure(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "scene3d.addFigure";
    let body = body(p, CMD)?;
    let pose = pose_param(p, CMD)?;
    with_scene(s, p, CMD, "Add 3D Figure", |scene, _, _| {
        if scene.figures.len() >= 64 {
            return Err(bad(CMD, "a 3D layer holds at most 64 figures"));
        }
        let mut fig = s3d::new_figure(body, pose);
        // Beside the others, on the camera's right.
        let right = scene.figures.iter().map(|f| f.position[0]).fold(f32::MIN, f32::max);
        if right > f32::MIN {
            fig.position = [right + 0.7, 0.0, scene.figures.first().map_or(0.0, |f| f.position[2])];
        }
        scene.figures.push(fig);
        Ok(())
    })
}

fn remove_figure(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "scene3d.removeFigure";
    let id = layer_id(s, p)?;
    let i = figure_index(p, &scene_of(s, id, CMD)?, CMD)?;
    with_scene(s, p, CMD, "Delete 3D Figure", |scene, _, _| {
        scene.figures.remove(i);
        Ok(())
    })
}

/// The Object tool's edit: the whole scene, replaced.
fn set_scene(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "scene3d.set";
    let v = p.get("scene").ok_or_else(|| bad(CMD, "`scene` is required"))?;
    let next: Scene3d = serde_json::from_value(v.clone()).map_err(|e| bad(CMD, format!("`scene`: {e}")))?;
    if next.figures.len() > 64 {
        return Err(bad(CMD, "a 3D layer holds at most 64 figures"));
    }
    with_scene(s, p, CMD, "Edit 3D Layer", |scene, _, _| {
        *scene = next;
        Ok(())
    })
}

fn get_scene(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_id(s, p)?;
    let scene = scene_of(s, id, "scene3d.get")?;
    let figures: Vec<Value> = scene.figures.iter().map(|f| json!({"body": f.body.id(), "joints": f.joints.len()})).collect();
    Ok(json!({ "layer": id.0, "scene": scene, "summary": figures }))
}

fn apply_pose(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "scene3d.applyPose";
    let id = layer_id(s, p)?;
    let i = figure_index(p, &scene_of(s, id, CMD)?, CMD)?;
    let pose = pose_param(p, CMD)?.ok_or_else(|| bad(CMD, "`pose` is required"))?.to_string();
    let sides = match p.get("sides").and_then(Value::as_str) {
        None => pose::Sides::Both,
        Some(v) => pose::Sides::from_id(v).ok_or_else(|| bad(CMD, "`sides` must be both, left or right"))?,
    };
    with_scene(s, p, CMD, "Apply Pose", |scene, _, _| {
        let fig = scene.figures.get_mut(i).ok_or_else(|| bad(CMD, "no such figure"))?;
        library::apply(fig, &pose, sides).map_err(|e| bad(CMD, e))
    })
}

fn figure_edit(s: &mut Session, p: &Value, cmd: &str, label: &str, f: impl FnOnce(&mut Figure3d) -> Result<()>) -> Result<Value> {
    let id = layer_id(s, p)?;
    let i = figure_index(p, &scene_of(s, id, cmd)?, cmd)?;
    with_scene(s, p, cmd, label, |scene, _, _| f(scene.figures.get_mut(i).ok_or_else(|| bad(cmd, "no such figure"))?))
}

fn mirror_pose(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "scene3d.mirrorPose";
    let from_left = match p.get("from").and_then(Value::as_str) {
        None | Some("left") => true,
        Some("right") => false,
        Some(_) => return Err(bad(CMD, "`from` must be left or right")),
    };
    figure_edit(s, p, CMD, "Mirror Pose", |f| {
        pose::mirror(f, from_left);
        Ok(())
    })
}

fn flip_pose(s: &mut Session, p: &Value) -> Result<Value> {
    figure_edit(s, p, "scene3d.flipPose", "Flip Pose", |f| {
        pose::flip(f);
        Ok(())
    })
}

fn place_on_floor(s: &mut Session, p: &Value) -> Result<Value> {
    figure_edit(s, p, "scene3d.placeOnFloor", "Place on Floor", |f| {
        pose::place_on_floor(f);
        f.position[1] = 0.0;
        Ok(())
    })
}

fn figure_props(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "scene3d.figure";
    let num = |k: &str| -> Result<Option<f32>> {
        match p.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v.as_f64().map(|x| Some(x as f32)).filter(|x| x.is_some_and(f32::is_finite)).ok_or_else(|| bad(CMD, format!("`{k}` must be a number"))),
        }
    };
    let (yaw, scale) = (num("yawDegrees")?, num("scale")?);
    let position = match p.get("position") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let a: [f32; 3] = serde_json::from_value(v.clone()).map_err(|_| bad(CMD, "`position` must be [x, y, z] in metres"))?;
            Some(a.map(|x| x.clamp(-1000.0, 1000.0)))
        }
    };
    let visible = p.get("visible").and_then(Value::as_bool);
    let body = match p.get("body") {
        None | Some(Value::Null) => None,
        Some(_) => Some(body(p, CMD)?),
    };
    figure_edit(s, p, CMD, "3D Figure Properties", |f| {
        if let Some(y) = yaw {
            f.yaw = y.to_radians();
        }
        if let Some(sc) = scale {
            f.scale = sc.clamp(0.05, 20.0);
        }
        if let Some(pos) = position {
            f.position = pos;
        }
        if let Some(v) = visible {
            f.visible = v;
        }
        if let Some(b) = body {
            f.body = b;
        }
        Ok(())
    })
}

fn camera(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "scene3d.camera";
    let view = p.get("view").and_then(Value::as_str).map(str::to_string);
    let frame = p.get("frame").and_then(Value::as_bool).unwrap_or(view.is_some());
    let parallel = p.get("parallel").and_then(Value::as_bool);
    let fov = p.get("fov").and_then(Value::as_f64).map(|v| (v as f32).clamp(5.0, 120.0));
    if let Some(v) = &view
        && s3d::camera_preset(&photocraft_doc::Camera3d::default(), v).is_none()
    {
        let known: Vec<&str> = s3d::CAMERA_VIEWS.iter().map(|(id, _)| *id).collect();
        return Err(bad(CMD, format!("`view` must be one of {}", known.join(", "))));
    }
    with_scene(s, p, CMD, "3D Camera", |scene, w, h| {
        if let Some(v) = &view
            && let Some(c) = s3d::camera_preset(&scene.camera, v)
        {
            scene.camera = c;
        }
        if let Some(par) = parallel {
            scene.camera.parallel = par;
        }
        if let Some(f) = fov {
            scene.camera.fov_y_deg = f;
        }
        if frame {
            s3d::frame(scene, w, h, 0.82);
        }
        Ok(())
    })
}

fn look(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "scene3d.look";
    let o = p.as_object().ok_or_else(|| bad(CMD, "expected an object"))?;
    let num = |k: &str, lo: f32, hi: f32| -> Result<Option<f32>> {
        match o.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v.as_f64().map(|x| (x as f32).clamp(lo, hi)).filter(|x| x.is_finite()).map(Some).ok_or_else(|| bad(CMD, format!("`{k}` must be a number"))),
        }
    };
    let rgb = |k: &str| -> Result<Option<[f32; 3]>> {
        match o.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => {
                let s = v.as_str().ok_or_else(|| bad(CMD, format!("`{k}` must be \"#rrggbb\"")))?;
                hex(s).map(Some).ok_or_else(|| bad(CMD, format!("`{k}` must be \"#rrggbb\"")))
            }
        }
    };
    let shading = match o.get("shading") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_str().and_then(Shading3d::from_id).ok_or_else(|| bad(CMD, "`shading` must be shaded, toon, flat or silhouette"))?),
    };
    let (outline, ambient) = (num("outline", 0.0, 64.0)?, num("ambient", 0.0, 1.0)?);
    let (light_yaw, light_pitch) = (num("lightYaw", -360.0, 360.0)?, num("lightPitch", -85.0, 85.0)?);
    let (body_color, outline_color) = (rgb("bodyColor")?, rgb("outlineColor")?);
    let shadow = o.get("floorShadow").and_then(Value::as_bool);
    let (strength, light_color) = (num("lightStrength", 0.0, 2.0)?, rgb("lightColor")?);
    let fixed = match o.get("lightFixed") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_bool().ok_or_else(|| bad(CMD, "`lightFixed` must be true or false"))?),
    };
    let preset = match o.get("lightPreset") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let names = s3d::light::PRESETS.map(|p| p.0).join(", ");
            Some(v.as_str().filter(|id| s3d::light::PRESETS.iter().any(|p| p.0 == *id)).ok_or_else(|| bad(CMD, format!("`lightPreset` must be one of {names}")))?.to_string())
        }
    };
    with_scene(s, p, CMD, "3D Look", |scene, _, _| {
        if let Some(v) = fixed {
            s3d::light::set_fixed(scene, v);
        }
        if let Some(id) = &preset {
            s3d::light::apply_preset(scene, id);
        }
        let l = &mut scene.look;
        if let Some(v) = strength {
            l.light_strength = v;
        }
        if let Some(v) = light_color {
            l.light_color = v;
        }
        if let Some(v) = shading {
            l.shading = v;
        }
        if let Some(v) = outline {
            l.outline_px = v;
        }
        if let Some(v) = ambient {
            l.ambient = v;
        }
        if let Some(v) = light_yaw {
            l.light_yaw = v.to_radians();
        }
        if let Some(v) = light_pitch {
            l.light_pitch = v.to_radians();
        }
        if let Some(v) = body_color {
            l.body_color = v;
        }
        if let Some(v) = outline_color {
            l.outline_color = v;
        }
        if let Some(v) = shadow {
            l.floor_shadow = v;
        }
        Ok(())
    })
}

fn hex(s: &str) -> Option<[f32; 3]> {
    let h = s.strip_prefix('#').unwrap_or(s);
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let c = |i: usize| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok()).map(|v| f32::from(v) / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

fn poses(_: &mut Session, p: &Value) -> Result<Value> {
    let category = p.get("category").and_then(Value::as_str);
    let entries: Vec<Value> = library::entries()
        .into_iter()
        .filter(|e| category.is_none_or(|c| e.category == c))
        .map(|e| json!({"id": e.id, "name": e.name, "category": e.category, "handsOnly": e.hands_only}))
        .collect();
    Ok(json!({ "categories": library::categories(), "poses": entries }))
}

fn rasterize(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.rasterize.scene3d";
    let id = layer_id(s, p)?;
    scene_of(s, id, CMD)?;
    s.edit("Rasterize 3D Layer", |doc, _| {
        let snapshot = doc.clone();
        let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
        let LayerContent::Scene3d(sc) = &mut l.content else { return Err(bad(CMD, "not a 3D layer")) };
        let fmt = snapshot.pixel_format();
        let px = match sc.cache.take() {
            Some(c) if c.format() == fmt => c,
            Some(c) => c.convert(fmt),
            None => s3d::render_surface(&sc.scene, snapshot.bounds(), fmt, Quality::Final),
        };
        l.content = LayerContent::Raster(px);
        Ok(())
    })?;
    Ok(json!({ "layer": id.0 }))
}

macro_rules! spec {
    ($id:expr, $label:literal, [$($m:literal),*], $params:literal, $en:expr, $run:expr) => {
        CommandSpec { id: $id, label: $label, menu: &[$($m),*], shortcut: None, params: $params, enabled: $en, run: $run, journal: true }
    };
}

pub fn specs() -> Vec<CommandSpec> {
    let mut v = vec![
        spec!(
            NEW,
            "3D Figure…",
            ["Layer", "New"],
            r##"{"body":"male|female"="male","pose":pose id?="Standing" (see scene3d.poses),"name":string?}"##,
            has_doc,
            new_figure
        ),
        spec!("scene3d.addFigure", "Add Figure", [], r##"{"layer":id?,"body":"male|female"="male","pose":pose id?}"##, is_scene3d, add_figure),
        spec!("scene3d.removeFigure", "Delete Figure", [], r##"{"layer":id?,"figure":index=0}"##, is_scene3d, remove_figure),
        spec!(
            "scene3d.set",
            "Edit 3D Layer",
            [],
            r##"{"layer":id?,"scene":Scene3d (as scene3d.get returns it),"draft":bool=false} (draft: quick render while dragging)"##,
            is_scene3d,
            set_scene
        ),
        spec!(
            "scene3d.applyPose",
            "Apply Pose",
            [],
            r##"{"layer":id?,"figure":index=0,"pose":pose id,"sides":"both|left|right"="both" (hand poses)}"##,
            is_scene3d,
            apply_pose
        ),
        spec!("scene3d.mirrorPose", "Mirror Pose", [], r##"{"layer":id?,"figure":index=0,"from":"left|right"="left"}"##, is_scene3d, mirror_pose),
        spec!("scene3d.flipPose", "Flip Pose", [], r##"{"layer":id?,"figure":index=0}"##, is_scene3d, flip_pose),
        spec!("scene3d.placeOnFloor", "Place on Floor", [], r##"{"layer":id?,"figure":index=0}"##, is_scene3d, place_on_floor),
        spec!(
            "scene3d.figure",
            "3D Figure Properties",
            [],
            r##"{"layer":id?,"figure":index=0,"body":"male|female"?,"position":[x,y,z]? (metres),"yawDegrees":number?,"scale":0.05..20?,"visible":bool?}"##,
            is_scene3d,
            figure_props
        ),
        spec!(
            "scene3d.camera",
            "3D Camera",
            [],
            r##"{"layer":id?,"view":"front|threeQuarter|left|right|back|top|high|low|bottom"?,"frame":bool=true with a view,"parallel":bool?,"fov":5..120?}"##,
            is_scene3d,
            camera
        ),
        spec!(
            "scene3d.look",
            "3D Look",
            [],
            r##"{"layer":id?,"shading":"shaded|toon|flat|silhouette"?,"outline":0..64? (px),"outlineColor":"#rrggbb"?,"bodyColor":"#rrggbb"?,"lightYaw":degrees?,"lightPitch":degrees?,"lightFixed":bool? (true: stays in the scene; false: turns with the camera),"lightStrength":0..2?,"lightColor":"#rrggbb"?,"lightPreset":"upperLeft|upperRight|front|left|right|top|below|rimLeft|rimRight"?,"ambient":0..1?,"floorShadow":bool?}"##,
            is_scene3d,
            look
        ),
        spec!(
            "layer.rasterize.scene3d",
            "3D Layer",
            ["Layer", "Rasterize"],
            r##"{"layer":id?}"##,
            is_scene3d,
            rasterize
        ),
        spec!(
            "scene3d.matchPhoto",
            "Match Pose to Photo",
            [],
            r##"{"layer":id?,"figure":index=0,"part":"body|hands|left|right"="body" (hands: each hand found onto the figure's hand of that side; left/right: the clearest hand onto that hand),"wrist":bool=true (hands also take the photo's wrist angle),"source":layer id? (track that layer's pixels; default: the canvas without its 3D layers)} → {layer, figure, matched}; a background job"##,
            is_scene3d,
            photo::match_photo
        ),
    ];
    v.push(CommandSpec {
        id: "scene3d.get",
        label: "3D Scene",
        menu: &[],
        shortcut: None,
        params: r##"{"layer":id?}"##,
        enabled: is_scene3d,
        run: get_scene,
        journal: false,
    });
    v.push(CommandSpec {
        id: "scene3d.poses",
        label: "3D Poses",
        menu: &[],
        shortcut: None,
        params: r##"{"category":string?}"##,
        enabled: always,
        run: poses,
        journal: false,
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 600, "height": 800})).unwrap();
        s
    }

    fn scene(s: &Session) -> Scene3d {
        let d = s.active().unwrap();
        match &d.doc.layer(d.active_layer.unwrap()).unwrap().content {
            LayerContent::Scene3d(sc) => sc.scene.clone(),
            _ => panic!("not a 3D layer"),
        }
    }

    fn pixels(s: &Session) -> usize {
        let d = s.active().unwrap();
        let l = d.doc.layer(d.active_layer.unwrap()).unwrap();
        let r = l.surface().map_or(photocraft_geom::Rect::EMPTY, |c| c.content_bounds());
        (r.width() * r.height()) as usize
    }

    #[test]
    fn a_new_figure_is_a_rendered_3d_layer_that_undoes() {
        let mut s = session();
        let r = s.execute(NEW, json!({"body": "female", "pose": "Walking"})).unwrap();
        assert!(r["layer"].as_u64().is_some());
        assert_eq!(scene(&s).figures.len(), 1);
        assert!(pixels(&s) > 10_000);
        s.execute("edit.undo", json!({})).unwrap();
        assert!(s.active().unwrap().doc.walk().iter().all(|(_, _, l)| !matches!(l.content, LayerContent::Scene3d(_))));
    }

    #[test]
    fn poses_mirror_and_the_camera_change_the_scene() {
        let mut s = session();
        s.execute(NEW, json!({})).unwrap();
        let before = scene(&s);
        s.execute("scene3d.applyPose", json!({"pose": library::library()[3].id})).unwrap();
        assert_ne!(scene(&s).figures[0].joints, before.figures[0].joints);
        s.execute("scene3d.applyPose", json!({"pose": "Fist", "sides": "left"})).unwrap();
        s.execute("scene3d.mirrorPose", json!({"from": "left"})).unwrap();
        assert!(scene(&s).figures[0].joints.contains_key("index2.R"));
        s.execute("scene3d.camera", json!({"view": "back"})).unwrap();
        assert!((scene(&s).camera.yaw - std::f32::consts::PI).abs() < 1e-4);
        s.execute("scene3d.look", json!({"shading": "toon", "outline": 3, "outlineColor": "#102030"})).unwrap();
        assert_eq!(scene(&s).look.shading, Shading3d::Toon);
        s.execute("scene3d.addFigure", json!({"body": "female"})).unwrap();
        assert_eq!(scene(&s).figures.len(), 2);
        s.execute("scene3d.removeFigure", json!({"figure": 1})).unwrap();
        assert_eq!(scene(&s).figures.len(), 1);
    }

    #[test]
    fn set_replaces_the_scene_and_rasterize_keeps_the_pixels() {
        let mut s = session();
        s.execute(NEW, json!({})).unwrap();
        let mut sc = scene(&s);
        sc.camera.offset_px = [100.0, 0.0];
        s.execute("scene3d.set", json!({"scene": sc, "draft": true})).unwrap();
        assert_eq!(scene(&s).camera.offset_px, [100.0, 0.0]);
        let n = pixels(&s);
        s.execute("layer.rasterize.scene3d", json!({})).unwrap();
        let d = s.active().unwrap();
        let l = d.doc.layer(d.active_layer.unwrap()).unwrap();
        assert!(matches!(l.content, LayerContent::Raster(_)));
        assert_eq!(pixels(&s), n);
    }

    #[test]
    fn bad_params_are_errors_not_panics() {
        let mut s = session();
        assert!(s.execute("scene3d.applyPose", json!({"pose": "Standing"})).is_err());
        s.execute(NEW, json!({})).unwrap();
        for (cmd, p) in [
            (NEW, json!({"body": "dog"})),
            (NEW, json!({"pose": "nope"})),
            ("scene3d.applyPose", json!({"pose": 3})),
            ("scene3d.applyPose", json!({"pose": "Standing", "figure": 9})),
            ("scene3d.set", json!({"scene": {"figures": "x"}})),
            ("scene3d.set", json!({})),
            ("scene3d.look", json!({"outlineColor": "red"})),
            ("scene3d.camera", json!({"view": "sideways"})),
            ("scene3d.figure", json!({"position": [1, 2]})),
        ] {
            assert!(s.execute(cmd, p.clone()).is_err(), "{cmd} {p}");
        }
        let hostile = json!({"scene": {"figures": [{"scale": 1e30, "yaw": 1e30, "root": [1e30, 0, 0], "joints": {"hips": [0, 0, 0, 0]}}], "camera": {"distance": -1, "fov_y_deg": 1e9}}});
        s.execute("scene3d.set", hostile).unwrap();
    }

    #[test]
    fn the_light_can_be_aimed_tinted_and_fixed_in_the_scene() {
        let mut s = session();
        s.execute(NEW, json!({})).unwrap();
        let before = scene(&s).look;
        s.execute("scene3d.look", json!({"lightPreset": "rimLeft", "lightStrength": 1.6, "lightColor": "#ffaa66"})).unwrap();
        let look = scene(&s).look;
        assert_ne!((look.light_yaw, look.light_pitch), (before.light_yaw, before.light_pitch));
        assert!((look.light_strength - 1.6).abs() < 1e-6);
        assert!((look.light_color[1] - 0xaa as f32 / 255.0).abs() < 1e-3);
        let dir = s3d::light::direction(&scene(&s));
        s.execute("scene3d.look", json!({"lightFixed": true})).unwrap();
        assert!(scene(&s).look.light_fixed);
        assert!(s3d::light::direction(&scene(&s)).distance(dir) < 1e-4, "fixing the light keeps it where it is");
        s.execute("scene3d.camera", json!({"view": "back"})).unwrap();
        assert!(s3d::light::direction(&scene(&s)).distance(dir) < 1e-4, "a fixed light stays when the camera moves");
        assert!(s.execute("scene3d.look", json!({"lightPreset": "sideways"})).is_err());
        assert!(s.execute("scene3d.look", json!({"lightFixed": "yes"})).is_err());
    }

    #[test]
    fn the_pose_list_names_every_category() {
        let mut s = Session::new();
        let r = s.execute("scene3d.poses", json!({})).unwrap();
        assert!(r["poses"].as_array().unwrap().len() > 1000);
        assert!(r["categories"].as_array().unwrap().iter().any(|c| c == "Dance"));
    }
}
