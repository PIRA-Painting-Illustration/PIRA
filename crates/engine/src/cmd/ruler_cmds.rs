//! Special rulers (the Ruler tool's Special Ruler option): guides that
//! painting strokes snap to.
//!
//! - **Straight** and **Curve**: a stroke that starts near the ruler runs along it.
//! - **Parallel**: every stroke runs parallel to the ruler, from wherever it starts.
//! - **Radial**: strokes run towards or away from the centre (speed lines, sunbursts).
//! - **Concentric**: a stroke runs round the ellipse (or circle) through where it starts.
//! - **Fisheye**: a stroke runs along the arc through the two vanishing points and its start.
//!
//! Rulers belong to the document (`PiraDoc::rulers`, saved in `.pcraft`) and to a layer: each
//! shows and snaps only while its layer is active, unless it is set to show on every layer.
//! Adding, moving and deleting rulers are history steps; Snap and Show are view switches. Like
//! the perspective ruler, a stroke picks its guide from its first few points (the one whose
//! direction there best matches where the stroke heads), then stays on it.

use crate::{CommandSpec, EngineError, Result, Session};
use photocraft_doc::{RulerShape, SpecialRuler};
use serde_json::{Value, json};

/// The special rulers' switches for a document (view state).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RulersView {
    /// Painting strokes snap to the rulers.
    pub snap: bool,
    /// The rulers are drawn over the canvas.
    pub visible: bool,
}

impl Default for RulersView {
    fn default() -> Self {
        Self { snap: true, visible: true }
    }
}

impl crate::DocState {
    /// The rulers that show on the active layer, with their indices in the document's list.
    pub fn rulers_here(&self) -> Vec<(usize, &SpecialRuler)> {
        self.doc.pira.rulers.iter().enumerate().filter(|(_, r)| r.layer.is_none() || r.layer == self.active_layer).collect()
    }
}

/// A line or curve a stroke has locked onto.
#[derive(Clone, Debug, PartialEq)]
pub enum Guide {
    /// The line through `origin` along unit `dir`.
    Line { origin: [f64; 2], dir: [f64; 2] },
    /// An ellipse: centre, half-axes and the angle of the first one.
    Ellipse { center: [f64; 2], radii: [f64; 2], angle: f64 },
    /// A circle.
    Circle { center: [f64; 2], r: f64 },
    /// A polyline (a sampled curve).
    Path(Vec<[f64; 2]>),
}

impl Guide {
    /// The point of the guide nearest `p` (for ellipses, the one on the same ray from the
    /// centre: close enough to nearest for drawing, and exact for circles).
    pub fn project(&self, p: [f64; 2]) -> [f64; 2] {
        match self {
            Guide::Line { origin, dir } => {
                let t = (p[0] - origin[0]) * dir[0] + (p[1] - origin[1]) * dir[1];
                [origin[0] + dir[0] * t, origin[1] + dir[1] * t]
            }
            Guide::Ellipse { center, radii, angle } => {
                let (u, v) = axes(*angle);
                let d = [p[0] - center[0], p[1] - center[1]];
                let (lx, ly) = (dot(d, u), dot(d, v));
                let t = (ly / radii[1]).atan2(lx / radii[0]);
                let (x, y) = (radii[0] * t.cos(), radii[1] * t.sin());
                [center[0] + u[0] * x + v[0] * y, center[1] + u[1] * x + v[1] * y]
            }
            Guide::Circle { center, r } => match unit([p[0] - center[0], p[1] - center[1]]) {
                Some(d) => [center[0] + d[0] * r, center[1] + d[1] * r],
                None => [center[0] + r, center[1]],
            },
            Guide::Path(pts) => nearest_on_path(pts, p).map_or(p, |(q, _)| q),
        }
    }

    /// The unit direction of the guide at the point nearest `p`.
    pub fn tangent(&self, p: [f64; 2]) -> Option<[f64; 2]> {
        match self {
            Guide::Line { dir, .. } => Some(*dir),
            Guide::Ellipse { center, radii, angle } => {
                let (u, v) = axes(*angle);
                let d = [p[0] - center[0], p[1] - center[1]];
                let t = (dot(d, v) / radii[1]).atan2(dot(d, u) / radii[0]);
                let (x, y) = (-radii[0] * t.sin(), radii[1] * t.cos());
                unit([u[0] * x + v[0] * y, u[1] * x + v[1] * y])
            }
            Guide::Circle { center, .. } => unit([p[0] - center[0], p[1] - center[1]]).map(|d| [-d[1], d[0]]),
            Guide::Path(pts) => nearest_on_path(pts, p).and_then(|(_, i)| unit([pts[i + 1][0] - pts[i][0], pts[i + 1][1] - pts[i][1]])),
        }
    }

    /// How far `p` is from the guide.
    pub fn distance(&self, p: [f64; 2]) -> f64 {
        let q = self.project(p);
        (p[0] - q[0]).hypot(p[1] - q[1])
    }
}

/// The guides `ruler` offers a stroke starting at `s`: Straight and Curve only within `range`
/// of the ruler, the others anywhere.
pub fn guide(shape: &RulerShape, s: [f64; 2], range: f64) -> Option<Guide> {
    let near = |g: Guide| (g.distance(s) <= range).then_some(g);
    match shape {
        RulerShape::Straight { a, b } => near(Guide::Line { origin: *a, dir: unit([b[0] - a[0], b[1] - a[1]])? }),
        RulerShape::Curve { points } => near(Guide::Path(curve_path(points))),
        RulerShape::Parallel { a, b } => Some(Guide::Line { origin: s, dir: unit([b[0] - a[0], b[1] - a[1]])? }),
        RulerShape::Radial { center } => Some(Guide::Line { origin: s, dir: unit([center[0] - s[0], center[1] - s[1]])? }),
        RulerShape::Concentric { center, radii, angle } => {
            let (u, v) = axes(*angle);
            let d = [s[0] - center[0], s[1] - center[1]];
            let k = (dot(d, u) / radii[0]).hypot(dot(d, v) / radii[1]);
            (k.is_finite() && k > 1e-6).then(|| Guide::Ellipse { center: *center, radii: [radii[0] * k, radii[1] * k], angle: *angle })
        }
        RulerShape::Fisheye { a, b } => Some(match circle_through(*a, *b, s) {
            Some((center, r)) => Guide::Circle { center, r },
            // On the line through the vanishing points: the eye level itself.
            None => Guide::Line { origin: s, dir: unit([b[0] - a[0], b[1] - a[1]])? },
        }),
    }
}

/// The guide a stroke from `s` heading towards `p` locks onto: of all the rulers' guides, the
/// one whose direction at `s` best matches the heading. None when the stroke hasn't moved or no
/// ruler offers one.
pub fn pick<'a>(rulers: impl IntoIterator<Item = &'a RulerShape>, s: [f64; 2], p: [f64; 2], range: f64) -> Option<Guide> {
    let heading = unit([p[0] - s[0], p[1] - s[1]])?;
    rulers
        .into_iter()
        .filter_map(|r| guide(r, s, range))
        .filter_map(|g| {
            let t = g.tangent(s)?;
            Some((dot(heading, t).abs(), g))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, g)| g)
}

/// A curve ruler's points as a smooth polyline (Catmull–Rom through every point).
pub fn curve_path(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    const STEPS: usize = 12;
    if points.len() < 3 {
        return points.to_vec();
    }
    let at = |i: isize| points[i.clamp(0, points.len() as isize - 1) as usize];
    let mut out = Vec::with_capacity((points.len() - 1) * STEPS + 1);
    for i in 0..points.len() as isize - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        for k in 0..STEPS {
            let t = k as f64 / STEPS as f64;
            let (t2, t3) = (t * t, t * t * t);
            let c = |a: f64, b: f64, c: f64, d: f64| 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3);
            out.push([c(p0[0], p1[0], p2[0], p3[0]), c(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
    out.extend(points.last());
    out
}

/// The nearest point on the polyline and the index of its segment.
fn nearest_on_path(pts: &[[f64; 2]], p: [f64; 2]) -> Option<([f64; 2], usize)> {
    pts.windows(2)
        .enumerate()
        .map(|(i, w)| {
            let (a, b) = (w[0], w[1]);
            let ab = [b[0] - a[0], b[1] - a[1]];
            let l2 = dot(ab, ab);
            let t = if l2 > 1e-12 { (dot([p[0] - a[0], p[1] - a[1]], ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
            ((p[0] - q[0]).hypot(p[1] - q[1]), q, i)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, q, i)| (q, i))
}

/// The circle through three points; None when they are (nearly) on one line.
fn circle_through(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Option<([f64; 2], f64)> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    let scale = (b[0] - a[0]).hypot(b[1] - a[1]).max(1.0);
    if d.abs() < 1e-6 * scale * scale {
        return None;
    }
    let sq = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let x = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
    let y = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
    let r = (a[0] - x).hypot(a[1] - y);
    (x.is_finite() && y.is_finite() && r.is_finite()).then_some(([x, y], r))
}

/// The unit axes of an ellipse turned by `angle`.
fn axes(angle: f64) -> ([f64; 2], [f64; 2]) {
    let (s, c) = angle.sin_cos();
    ([c, s], [-s, c])
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// `v` scaled to length 1; None when it is zero or not finite.
fn unit(v: [f64; 2]) -> Option<[f64; 2]> {
    let l = v[0].hypot(v[1]);
    (l.is_finite() && l > 1e-9).then(|| [v[0] / l, v[1] / l])
}

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

/// A ruler as the commands report it.
pub fn inspect(index: usize, r: &SpecialRuler) -> Value {
    let mut v = serde_json::to_value(&r.shape).unwrap_or(Value::Null);
    v["index"] = json!(index);
    v["layer"] = r.layer.map_or(Value::Null, |l| json!(l.0));
    v
}

/// A shape from a command's params (`{"type": …, …}`), checked against the canvas.
fn shape(s: &Session, cmd: &str, p: &Value) -> Result<RulerShape> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let shape: RulerShape = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, format!("a ruler: {e}")))?;
    if !shape.ok(f64::from(st.doc.size.width), f64::from(st.doc.size.height)) {
        return Err(bad(cmd, "the ruler's points must be finite, near the canvas and apart (a curve has 2 to 64, an ellipse non-zero radii)"));
    }
    Ok(shape)
}

/// The layer a new ruler belongs to: the active one, or every layer with `allLayers`.
fn layer_for(s: &Session, p: &Value) -> Result<Option<photocraft_doc::LayerId>> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    Ok(if p.get("allLayers").and_then(Value::as_bool).unwrap_or(false) { None } else { st.active_layer })
}

/// `ruler.add`.
fn add(s: &mut Session, p: &Value) -> Result<Value> {
    let shape = shape(s, "ruler.add", p)?;
    let layer = layer_for(s, p)?;
    let label = match shape.kind() {
        "straight" => "Straight Ruler",
        "curve" => "Curve Ruler",
        "concentric" => "Concentric Circle Ruler",
        "parallel" => "Parallel Line Ruler",
        "radial" => "Radial Line Ruler",
        _ => "Fisheye Ruler",
    };
    let ruler = SpecialRuler { shape, layer };
    let r = ruler.clone();
    s.edit(label, move |doc, _| {
        doc.pira.rulers.push(r);
        Ok(())
    })?;
    let st = s.active_mut().ok_or(EngineError::NoDocument)?;
    st.rulers_view = RulersView::default();
    let index = st.doc.pira.rulers.len() - 1;
    Ok(inspect(index, &ruler))
}

/// The `index` param, checked against the document's rulers.
fn index(s: &Session, cmd: &str, p: &Value) -> Result<usize> {
    let n = s.active().ok_or(EngineError::NoDocument)?.doc.pira.rulers.len();
    p.get("index").and_then(Value::as_u64).map(|i| i as usize).filter(|i| *i < n).ok_or_else(|| bad(cmd, format!("index: 0 to {}", n.saturating_sub(1))))
}

/// `ruler.set`: replaces a ruler's shape (`shape`) and which layers it shows on (`allLayers`).
fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let i = index(s, "ruler.set", p)?;
    let mut ruler = s.active().ok_or(EngineError::NoDocument)?.doc.pira.rulers[i].clone();
    if let Some(sh) = p.get("shape") {
        ruler.shape = shape(s, "ruler.set", sh)?;
    }
    if let Some(all) = p.get("allLayers") {
        let all = all.as_bool().ok_or_else(|| bad("ruler.set", "allLayers: true or false"))?;
        ruler.layer = if all { None } else { ruler.layer.or(s.active().and_then(|st| st.active_layer)) };
    }
    if s.active().is_some_and(|st| st.doc.pira.rulers[i] == ruler) {
        return Ok(inspect(i, &ruler));
    }
    let r = ruler.clone();
    s.edit("Edit Ruler", move |doc, _| {
        if let Some(slot) = doc.pira.rulers.get_mut(i) {
            *slot = r;
        }
        Ok(())
    })?;
    Ok(inspect(i, &ruler))
}

/// `ruler.delete`.
fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    let i = index(s, "ruler.delete", p)?;
    s.edit("Delete Ruler", move |doc, _| {
        if i < doc.pira.rulers.len() {
            doc.pira.rulers.remove(i);
        }
        Ok(())
    })?;
    Ok(json!({}))
}

/// `ruler.clear`: deletes the rulers that show on the active layer.
fn clear(s: &mut Session) -> Result<Value> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let gone: Vec<usize> = st.rulers_here().into_iter().map(|(i, _)| i).collect();
    if gone.is_empty() {
        return Ok(json!({"deleted": 0}));
    }
    let n = gone.len();
    s.edit("Clear Rulers", move |doc, _| {
        let mut k = 0;
        doc.pira.rulers.retain(|_| {
            k += 1;
            !gone.contains(&(k - 1))
        });
        Ok(())
    })?;
    Ok(json!({"deleted": n}))
}

/// `ruler.list`.
fn list(s: &Session) -> Result<Value> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let here: Vec<usize> = st.rulers_here().into_iter().map(|(i, _)| i).collect();
    let rulers: Vec<Value> = st
        .doc
        .pira
        .rulers
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut v = inspect(i, r);
            v["shown"] = json!(here.contains(&i));
            v
        })
        .collect();
    Ok(json!({"rulers": rulers, "snap": st.rulers_view.snap, "visible": st.rulers_view.visible}))
}

/// `ruler.{snap,show}`: flips one of the view switches.
fn toggle(s: &mut Session, f: fn(&mut RulersView) -> &mut bool) -> Result<Value> {
    let st = s.active_mut().ok_or(EngineError::NoDocument)?;
    if st.doc.pira.rulers.is_empty() {
        return Err(EngineError::Other("no special rulers".into()));
    }
    let b = f(&mut st.rulers_view);
    *b = !*b;
    Ok(json!({"snap": st.rulers_view.snap, "visible": st.rulers_view.visible}))
}

fn has_doc(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}

fn has_rulers(s: &Session) -> std::result::Result<(), String> {
    s.active().filter(|st| !st.doc.pira.rulers.is_empty()).map(|_| ()).ok_or_else(|| "no special rulers".into())
}

/// The special ruler commands, registered with the engine.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "ruler.add",
            label: "Add Special Ruler",
            menu: &[],
            shortcut: None,
            params: r#"{"type":"straight"|"curve"|"concentric"|"parallel"|"radial"|"fisheye", straight/parallel/fisheye: "a":[x,y],"b":[x,y]; curve: "points":[[x,y],…] (2–64); concentric: "center":[x,y],"radii":[rx,ry],"angle":radians; radial: "center":[x,y]; "allLayers":bool=false (else it belongs to the active layer)}"#,
            enabled: has_doc,
            run: add,
            journal: true,
        },
        CommandSpec {
            id: "ruler.set",
            label: "Edit Special Ruler",
            menu: &[],
            shortcut: None,
            params: r#"{"index":n,"shape":{as ruler.add}?,"allLayers":bool?}"#,
            enabled: has_rulers,
            run: set,
            journal: true,
        },
        CommandSpec {
            id: "ruler.delete",
            label: "Delete Special Ruler",
            menu: &[],
            shortcut: None,
            params: r#"{"index":n}"#,
            enabled: has_rulers,
            run: delete,
            journal: true,
        },
        CommandSpec {
            id: "ruler.clear",
            label: "Clear Special Rulers",
            menu: &[],
            shortcut: None,
            params: "{} — deletes the rulers shown on the active layer",
            enabled: has_rulers,
            run: |s, _| clear(s),
            journal: true,
        },
        CommandSpec {
            id: "ruler.snap",
            label: "Snap to Special Rulers",
            menu: &[],
            shortcut: None,
            params: "{} — toggle: painting strokes snap to the special rulers",
            enabled: has_rulers,
            run: |s, _| toggle(s, |r| &mut r.snap),
            journal: false,
        },
        CommandSpec {
            id: "ruler.show",
            label: "Show Special Rulers",
            menu: &[],
            shortcut: None,
            params: "{} — toggle: draw the special rulers",
            enabled: has_rulers,
            run: |s, _| toggle(s, |r| &mut r.visible),
            journal: false,
        },
        CommandSpec {
            id: "ruler.list",
            label: "Special Rulers Info",
            menu: &[],
            shortcut: None,
            params: "{} → {rulers:[{index,type,…,layer,shown}],snap,visible}",
            enabled: has_doc,
            run: |s, _| list(s),
            journal: false,
        },
    ]
}

#[cfg(test)]
mod tests;
