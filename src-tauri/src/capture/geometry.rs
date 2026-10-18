//! Coordinate math for region selection.
//!
//! Each monitor gets its own overlay window covering exactly that monitor, and the app is
//! per-monitor DPI aware, so drag points arrive in physical pixels relative to the
//! monitor's top-left: the same space as that monitor's frozen frame. Virtual-desktop
//! origins (which may be negative) and mixed DPI therefore never enter the crop math.

use crate::platform::MonitorInfo;

/// A crop rectangle in physical pixels within one monitor's frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Selections smaller than this (either side) are treated as a click, not a capture.
pub const MIN_SELECTION_PX: u32 = 4;

/// The crop for a drag from `a` to `b` (either direction) on a `frame_w` x `frame_h`
/// monitor. Points outside the monitor (the pointer is captured, so it can leave) are
/// clamped to its edges. Returns `None` for a click or a tiny accidental drag.
pub fn from_drag(a: (i32, i32), b: (i32, i32), frame_w: u32, frame_h: u32) -> Option<PhysicalRect> {
    let clamp = |v: i32, max: u32| v.clamp(0, i32::try_from(max).unwrap_or(i32::MAX)) as u32;
    let (x0, x1) = (clamp(a.0.min(b.0), frame_w), clamp(a.0.max(b.0), frame_w));
    let (y0, y1) = (clamp(a.1.min(b.1), frame_h), clamp(a.1.max(b.1), frame_h));
    let (width, height) = (x1 - x0, y1 - y0);
    (width >= MIN_SELECTION_PX && height >= MIN_SELECTION_PX).then_some(PhysicalRect { x: x0, y: y0, width, height })
}

/// The monitor containing a point on the virtual desktop (physical pixels).
pub fn monitor_at(monitors: &[MonitorInfo], x: i32, y: i32) -> Option<usize> {
    monitors.iter().position(|m| {
        let (x, y) = (i64::from(x), i64::from(y));
        let (mx, my) = (i64::from(m.x), i64::from(m.y));
        x >= mx && x < mx + i64::from(m.width) && y >= my && y < my + i64::from(m.height)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_maps_one_to_one() {
        let r = from_drag((10, 20), (610, 360), 1920, 1080).unwrap();
        assert_eq!(r, PhysicalRect { x: 10, y: 20, width: 600, height: 340 });
    }

    #[test]
    fn reversed_drag_is_normalized() {
        assert_eq!(from_drag((610, 360), (10, 20), 1920, 1080), from_drag((10, 20), (610, 360), 1920, 1080));
        assert_eq!(from_drag((610, 20), (10, 360), 1920, 1080), from_drag((10, 20), (610, 360), 1920, 1080));
    }

    #[test]
    fn clamps_to_monitor_when_pointer_leaves_it() {
        let r = from_drag((-50, -80), (5000, 9000), 3840, 2160).unwrap();
        assert_eq!(r, PhysicalRect { x: 0, y: 0, width: 3840, height: 2160 });
    }

    #[test]
    fn rejects_clicks_and_slivers() {
        assert!(from_drag((5, 5), (5, 5), 100, 100).is_none());
        assert!(from_drag((5, 5), (8, 80), 100, 100).is_none());
        assert!(from_drag((-30, -30), (-10, -10), 100, 100).is_none()); // entirely off-monitor
    }

    fn mon(id: u32, x: i32, y: i32, width: u32, height: u32) -> MonitorInfo {
        MonitorInfo { id, name: String::new(), x, y, width, height, scale_factor: 1.0, is_primary: id == 1 }
    }

    #[test]
    fn finds_monitor_with_negative_origin() {
        // Secondary 4K panel to the left of and above a 1080p primary.
        let ms = [mon(1, 0, 0, 1920, 1080), mon(2, -3840, -500, 3840, 2160)];
        assert_eq!(monitor_at(&ms, -1, 0), Some(1));
        assert_eq!(monitor_at(&ms, -3840, -500), Some(1));
        assert_eq!(monitor_at(&ms, 0, 0), Some(0));
        assert_eq!(monitor_at(&ms, 1919, 1079), Some(0));
        assert_eq!(monitor_at(&ms, 1920, 0), None);
        assert_eq!(monitor_at(&ms, -100, 1700), None);
    }
}
