//! The mouse's whole path during a drag. Windows keeps one pending mouse move, so while a frame
//! is drawn the moves in between are merged: a quick mouse stroke reached the brush as one
//! position per frame and came out as a few long straight pieces. The platform reads the
//! positions it recorded ([`crate::MousePathFn`], Windows' `GetMouseMovePointsEx`), and
//! [`splice`] puts them back as pointer moves, as Android's pen path does for pen samples.

use egui::{Event, Pos2, pos2};

/// How far (points) the recorded path may end from winit's latest move.
pub const MAX_DRIFT: f32 = 24.0;

/// Replace a frame's pointer moves with one per point of `path` (window pixels, `ppp` pixels per
/// point), ending on winit's latest move. Returns how many moves were put in (0: `events`
/// unchanged).
///
/// Only frames of plain moves are changed: a press, a release or a touch in the frame keeps
/// winit's moves, in order with it. A safety net: when the path's last point is far from winit's
/// latest move (more than [`MAX_DRIFT`] points), the two don't measure the same way (another
/// monitor's scaling, a stale history), and winit's moves are kept.
pub fn splice(events: &mut Vec<Event>, path: &[(f32, f32)], ppp: f32) -> usize {
    if path.is_empty() || !(ppp.is_finite() && ppp > 0.0) {
        return 0;
    }
    if events.iter().any(|e| matches!(e, Event::PointerButton { .. } | Event::Touch { .. } | Event::PointerGone)) {
        return 0;
    }
    let Some(at) = events.iter().position(|e| matches!(e, Event::PointerMoved(_))) else { return 0 };
    let Some(last) = events.iter().rev().find_map(|e| if let Event::PointerMoved(p) = e { Some(*p) } else { None }) else { return 0 };
    let mut points: Vec<Pos2> = path.iter().map(|&(x, y)| pos2(x / ppp, y / ppp)).collect();
    if points.last().is_none_or(|p| p.distance(last) > MAX_DRIFT) {
        return 0;
    }
    // The frame ends exactly where winit says the pointer is.
    if points.last() != Some(&last) {
        points.push(last);
    }
    let added = points.len();
    let mut out = Vec::with_capacity(events.len() + added);
    for (i, e) in events.drain(..).enumerate() {
        if i == at {
            out.extend(points.iter().map(|p| Event::PointerMoved(*p)));
        }
        if !matches!(e, Event::PointerMoved(_)) {
            out.push(e);
        }
    }
    *events = out;
    added
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xs(events: &[Event]) -> Vec<f32> {
        events.iter().filter_map(|e| if let Event::PointerMoved(p) = e { Some(p.x) } else { None }).collect()
    }

    #[test]
    fn every_recorded_position_becomes_a_move() {
        let mut events = vec![Event::PointerMoved(pos2(10.0, 0.0)), Event::Zoom(1.0), Event::PointerMoved(pos2(60.0, 0.0))];
        // Pixels at 2 pixels per point: 20, 40, 60 points.
        assert_eq!(splice(&mut events, &[(40.0, 0.0), (80.0, 0.0), (120.0, 0.0)], 2.0), 3);
        assert_eq!(xs(&events), [20.0, 40.0, 60.0]);
        assert!(events.contains(&Event::Zoom(1.0)), "other events stay");
    }

    #[test]
    fn the_frame_ends_on_winits_latest_move() {
        let mut events = vec![Event::PointerMoved(pos2(61.0, 0.0))];
        assert_eq!(splice(&mut events, &[(20.0, 0.0), (40.0, 0.0), (60.0, 0.0)], 1.0), 4);
        assert_eq!(xs(&events), [20.0, 40.0, 60.0, 61.0]);
    }

    #[test]
    fn a_path_measured_elsewhere_is_not_used() {
        let before = vec![Event::PointerMoved(pos2(30.0, 0.0))];
        let mut events = before.clone();
        assert_eq!(splice(&mut events, &[(29.0, 0.0), (130.0, 0.0)], 1.0), 0);
        assert_eq!(events, before);
    }

    #[test]
    fn a_frame_with_a_press_release_or_touch_keeps_its_moves() {
        let press = Event::PointerButton { pos: pos2(5.0, 0.0), button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() };
        let touch = Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(1), phase: egui::TouchPhase::Move, pos: pos2(5.0, 0.0), force: None };
        for other in [press, touch, Event::PointerGone] {
            let before = vec![Event::PointerMoved(pos2(5.0, 0.0)), other];
            let mut events = before.clone();
            assert_eq!(splice(&mut events, &[(1.0, 0.0), (5.0, 0.0)], 1.0), 0);
            assert_eq!(events, before);
        }
    }

    #[test]
    fn without_moves_or_a_path_nothing_changes() {
        let mut events = vec![Event::Zoom(1.0)];
        assert_eq!(splice(&mut events, &[(1.0, 0.0)], 1.0), 0);
        let mut events = vec![Event::PointerMoved(pos2(1.0, 0.0))];
        assert_eq!(splice(&mut events, &[], 1.0), 0);
        assert_eq!(splice(&mut events, &[(1.0, 0.0)], 0.0), 0);
    }
}
