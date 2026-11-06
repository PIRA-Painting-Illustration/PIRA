//! Smart Shape: hold the pen still at the end of a Brush, Pencil or Eraser stroke and the stroke
//! snaps to the shape it looks like: a straight line, an ellipse or a rectangle. Nothing happens
//! when it looks like none of them. After the snap the stroke stays put until the pen lifts.

use crate::PhotocraftApp;
use crate::state::Tool;

/// How long the pen must stay still (milliseconds).
pub const HOLD_MS: f64 = 550.0;
/// How far (screen points) the pen may wander and still count as still.
const STILL_PT: f64 = 4.0;

/// Where and since when the pen has been still during the current drag, whether this drag
/// snapped to a shape, and whether this stillness was already looked at.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hold {
    pub at: Option<([f64; 2], f64)>,
    pub snapped: bool,
    pub checked: bool,
}

/// Tools Smart Shape works with.
pub fn applies(tool: Tool) -> bool {
    matches!(tool, Tool::Brush | Tool::Pencil | Tool::Eraser)
}

/// A pointer sample at document point `p` (time `now_ms`, view zoom `zoom`): restart the still
/// timer when the pen has moved.
pub fn track(hold: &mut Hold, p: [f64; 2], now_ms: f64, zoom: f64) {
    match hold.at {
        Some((at, _)) if (at[0] - p[0]).hypot(at[1] - p[1]) * zoom <= STILL_PT => {}
        _ => {
            hold.at = Some((p, now_ms));
            hold.checked = false;
        }
    }
}

/// Once per frame while a stroke is held: snap it when the pen has been still long enough.
/// Returns whether it snapped.
pub fn check(app: &mut PhotocraftApp, now_ms: f64) -> bool {
    if !app.ui.tool_options.smart_shape || app.smart_hold.snapped || app.smart_hold.checked {
        return false;
    }
    let Some((_, since)) = app.smart_hold.at else { return false };
    if now_ms - since < HOLD_MS {
        return false;
    }
    let Some(d) = app.drag.as_mut().filter(|d| applies(d.tool)) else { return false };
    let Some(shape) = fit(&d.points) else {
        // Looked at once: no shape, and it won't become one until the pen moves again.
        app.smart_hold.checked = true;
        return false;
    };
    app.stylus.respread(shape.len());
    d.points = shape;
    app.smart_hold.snapped = true;
    crate::canvas::restart_live_stroke(app);
    true
}

/// The shape `points` (x, y, pressure) look like, as a polyline at their mean pressure, or
/// `None` when they look like no shape.
pub fn fit(points: &[[f64; 3]]) -> Option<Vec<[f64; 3]>> {
    if points.len() < 6 {
        return None;
    }
    let pressure = points.iter().map(|p| p[2]).sum::<f64>() / points.len() as f64;
    let length: f64 = points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
    if length < 12.0 {
        return None;
    }
    let (a, b) = (points[0], points[points.len() - 1]);
    let chord = (b[0] - a[0]).hypot(b[1] - a[1]);
    let at = |x: f64, y: f64| [x, y, pressure];
    // A line: every point close to the chord, and the path about as long as it.
    if chord > 0.6 * length {
        let (ux, uy) = ((b[0] - a[0]) / chord, (b[1] - a[1]) / chord);
        let off = points.iter().map(|p| ((p[0] - a[0]) * uy - (p[1] - a[1]) * ux).abs()).fold(0.0, f64::max);
        if off <= (0.05 * chord).max(2.0) {
            return Some((0..=16).map(|i| at(a[0] + (b[0] - a[0]) * f64::from(i) / 16.0, a[1] + (b[1] - a[1]) * f64::from(i) / 16.0)).collect());
        }
        return None;
    }
    // A closed shape: the ends meet.
    if chord > 0.12 * length {
        return None;
    }
    // Its own axes (principal components), so tilted shapes fit too.
    let n = points.len() as f64;
    let (cx, cy) = (points.iter().map(|p| p[0]).sum::<f64>() / n, points.iter().map(|p| p[1]).sum::<f64>() / n);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in points {
        let (dx, dy) = (p[0] - cx, p[1] - cy);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let (c, s) = (theta.cos(), theta.sin());
    let local: Vec<(f64, f64)> = points.iter().map(|p| ((p[0] - cx) * c + (p[1] - cy) * s, -(p[0] - cx) * s + (p[1] - cy) * c)).collect();
    let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(u, v) in &local {
        (u0, u1, v0, v1) = (u0.min(u), u1.max(u), v0.min(v), v1.max(v));
    }
    let (hu, hv) = ((u1 - u0) / 2.0, (v1 - v0) / 2.0);
    if hu.min(hv) < 3.0 {
        return None;
    }
    let (mu, mv) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0);
    // And it goes all the way round its centre (a back-and-forth scribble doesn't).
    let mut angles: Vec<f64> = local.iter().map(|&(u, v)| ((v - mv) / hv).atan2((u - mu) / hu)).collect();
    angles.sort_by(f64::total_cmp);
    let wrap = angles[0] + std::f64::consts::TAU - angles[angles.len() - 1];
    if angles.windows(2).map(|w| w[1] - w[0]).fold(wrap, f64::max) > 1.0 {
        return None;
    }
    let world = |u: f64, v: f64| at(cx + (mu + u) * c - (mv + v) * s, cy + (mu + u) * s + (mv + v) * c);
    // How far each fit is from the points, relative to the shape's size.
    let size = hu.min(hv);
    let ellipse_err = local.iter().map(|&(u, v)| (((u - mu) / hu).hypot((v - mv) / hv) - 1.0).abs()).sum::<f64>() / n;
    let rect_err = local
        .iter()
        .map(|&(u, v)| {
            let (du, dv) = (hu - (u - mu).abs(), hv - (v - mv).abs());
            du.min(dv).abs() / size
        })
        .sum::<f64>()
        / n;
    if rect_err < 0.08 && rect_err < ellipse_err {
        let corners = [(-hu, -hv), (hu, -hv), (hu, hv), (-hu, hv), (-hu, -hv)];
        let mut out = Vec::new();
        for w in corners.windows(2) {
            for i in 0..8 {
                let t = f64::from(i) / 8.0;
                out.push(world(w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t));
            }
        }
        out.push(world(corners[4].0, corners[4].1));
        return Some(out);
    }
    if ellipse_err < 0.12 {
        let steps = 72;
        return Some(
            (0..=steps)
                .map(|i| {
                    let t = std::f64::consts::TAU * f64::from(i) / f64::from(steps);
                    world(hu * t.cos(), hv * t.sin())
                })
                .collect(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wobble(i: usize) -> f64 {
        // A small, repeatable hand tremor.
        ((i * 7919) % 13) as f64 / 13.0 - 0.5
    }

    #[test]
    fn a_shaky_line_becomes_straight() {
        let pts: Vec<[f64; 3]> = (0..40).map(|i| [10.0 + i as f64 * 5.0, 50.0 + wobble(i) * 3.0, 0.5]).collect();
        let s = fit(&pts).expect("a line");
        assert!(s.iter().all(|p| (p[1] - 50.0 + (p[0] - 10.0) * 0.0).abs() < 2.0));
        assert_eq!(s.first().map(|p| p[0]), Some(10.0));
        assert!(s.iter().all(|p| (p[2] - 0.5).abs() < 1e-9), "mean pressure");
    }

    #[test]
    fn a_rough_circle_becomes_an_ellipse() {
        let pts: Vec<[f64; 3]> = (0..60)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / 58.0;
                let r = 40.0 + wobble(i) * 3.0;
                [100.0 + r * t.cos(), 100.0 + r * t.sin(), 0.7]
            })
            .collect();
        let s = fit(&pts).expect("an ellipse");
        assert_eq!(s.len(), 73);
        for p in &s {
            let r = (p[0] - 100.0).hypot(p[1] - 100.0);
            assert!((r - 40.0).abs() < 3.0, "{r}");
        }
    }

    #[test]
    fn a_rough_box_becomes_a_rectangle() {
        let mut pts = Vec::new();
        let corners = [(0.0, 0.0), (80.0, 0.0), (80.0, 50.0), (0.0, 50.0), (0.0, 1.0)];
        for w in corners.windows(2) {
            for i in 0..15 {
                let t = i as f64 / 15.0;
                pts.push([w[0].0 + (w[1].0 - w[0].0) * t + wobble(pts.len()), w[0].1 + (w[1].1 - w[0].1) * t + wobble(pts.len() + 3), 0.5]);
            }
        }
        let s = fit(&pts).expect("a rectangle");
        assert_eq!(s.len(), 33);
        // Its corners land near the drawn ones.
        for (x, y) in [(0.0, 0.0), (80.0, 0.0), (80.0, 50.0), (0.0, 50.0)] {
            assert!(s.iter().any(|p| (p[0] - x).hypot(p[1] - y) < 3.0), "corner {x},{y}");
        }
    }

    #[test]
    fn a_scribble_stays_a_scribble() {
        let pts: Vec<[f64; 3]> = (0..40).map(|i| [i as f64 * 3.0, if i % 4 < 2 { 0.0 } else { 30.0 }, 0.5]).collect();
        assert!(fit(&pts).is_none());
        assert!(fit(&pts[..4]).is_none(), "too short");
    }

    #[test]
    fn the_still_timer_restarts_when_the_pen_moves() {
        let mut h = Hold::default();
        track(&mut h, [0.0, 0.0], 0.0, 1.0);
        track(&mut h, [2.0, 0.0], 300.0, 1.0);
        assert_eq!(h.at.map(|a| a.1), Some(0.0), "a little drift is still");
        track(&mut h, [10.0, 0.0], 400.0, 1.0);
        assert_eq!(h.at.map(|a| a.1), Some(400.0));
    }
}
