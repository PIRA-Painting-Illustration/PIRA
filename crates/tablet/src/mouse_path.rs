//! The mouse's whole path on Windows. Windows keeps a single pending `WM_MOUSEMOVE`: while the
//! app draws a frame, newer positions replace older ones, so a quick mouse stroke reached the
//! brush as one position per frame and came out as a few long straight pieces. Windows still
//! records every position the mouse reported (`GetMouseMovePointsEx`, the last 64, newest
//! first); [`since`] picks out the ones after the last position already taken, so each frame
//! can draw through all of them (the Windows side is `windows::MousePath`).
//!
//! Plain logic, so it builds and is tested on every platform.

/// One entry of Windows' mouse history: a screen position (pixels) and its time (milliseconds,
/// `GetTickCount`'s clock).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
    pub time: u32,
}

/// `GMMP_USE_DISPLAY_POINTS` gives coordinates as 16-bit values, so a monitor left of or above
/// the main one (negative coordinates) comes back as 32768..65535: those are negative again.
pub fn unwrap(v: i32) -> i32 {
    if v > 32767 { v - 65536 } else { v }
}

/// The points of `history` (newest first, as `GetMouseMovePointsEx` returns them) that came after
/// `last`, oldest first. With no `last` (the first call), none: only later moves are new.
///
/// `last` is looked for by position and time. When it has dropped out of the history (more than
/// 64 moves since), every point newer than its time is new.
pub fn since(history: &[Point], last: Option<Point>) -> Vec<Point> {
    let Some(last) = last else { return Vec::new() };
    let newer = match history.iter().position(|p| *p == last) {
        Some(i) => &history[..i],
        None => {
            // Tick counts wrap after 49.7 days: compare by difference.
            let n = history.iter().take_while(|p| p.time.wrapping_sub(last.time) as i32 > 0).count();
            &history[..n]
        }
    };
    newer.iter().rev().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i32, time: u32) -> Point {
        Point { x, y: 0, time }
    }

    #[test]
    fn the_points_after_the_last_taken_come_oldest_first() {
        let history = [p(40, 13), p(30, 12), p(20, 11), p(10, 10)];
        assert_eq!(since(&history, Some(p(20, 11))), [p(30, 12), p(40, 13)]);
        assert_eq!(since(&history, Some(p(40, 13))), [], "nothing new");
    }

    #[test]
    fn the_first_call_takes_nothing() {
        assert_eq!(since(&[p(10, 10)], None), []);
    }

    #[test]
    fn a_last_point_gone_from_the_history_keeps_what_is_newer() {
        let history = [p(40, 13), p(30, 12), p(20, 11)];
        assert_eq!(since(&history, Some(p(5, 11))), [p(30, 12), p(40, 13)]);
        assert_eq!(since(&history, Some(p(5, 2))), [p(20, 11), p(30, 12), p(40, 13)]);
    }

    #[test]
    fn the_same_spot_at_another_time_is_another_point() {
        // The mouse went back to x 10: only the later visit is new.
        let history = [p(10, 12), p(20, 11), p(10, 10)];
        assert_eq!(since(&history, Some(p(10, 10))), [p(20, 11), p(10, 12)]);
    }

    #[test]
    fn times_compare_across_the_tick_count_wrap() {
        let history = [p(30, 5), p(20, u32::MAX)];
        assert_eq!(since(&history, Some(p(9, u32::MAX - 3))), [p(20, u32::MAX), p(30, 5)]);
    }

    #[test]
    fn display_points_left_of_the_main_monitor_are_negative() {
        assert_eq!(unwrap(65535), -1);
        assert_eq!(unwrap(65536 - 1920), -1920);
        assert_eq!(unwrap(1919), 1919);
    }
}
