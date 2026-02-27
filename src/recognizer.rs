//! Gesture recognition engine for touch input events.
use std::collections::HashMap;
use std::time::Instant;

use strum::{Display, EnumString, IntoStaticStr};

use crate::config::ValidatedThresholds;

/// Supported gesture types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, IntoStaticStr)]
pub enum GestureType {
    #[strum(serialize = "swipe_left")]
    SwipeLeft,
    #[strum(serialize = "swipe_right")]
    SwipeRight,
    #[strum(serialize = "swipe_up")]
    SwipeUp,
    #[strum(serialize = "swipe_down")]
    SwipeDown,
    #[strum(serialize = "tap")]
    Tap,
    #[strum(serialize = "double_tap")]
    DoubleTap,
    #[strum(serialize = "long_press")]
    LongPress,
    #[strum(serialize = "pinch_in")]
    PinchIn,
    #[strum(serialize = "pinch_out")]
    PinchOut,
}

/// Represents a single touch point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    pub x: f64,
    pub y: f64,
    pub time: Instant,
    pub tracking_id: i32,
}

impl TouchPoint {
    fn distance_to(&self, other: &TouchPoint) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// Recognizes gestures from touch input events.
#[derive(Default)]
pub struct GestureRecognizer {
    pub thresholds: ValidatedThresholds,
    x_range: (f64, f64),
    y_range: (f64, f64),

    /// Current touch state - public for direct manipulation in tests.
    pub touch_start: Option<TouchPoint>,
    pub touch_current: Option<TouchPoint>,
    pub touch_points: Vec<TouchPoint>,
    pub active_touches: HashMap<i32, TouchPoint>,
    pub last_tap_time: Option<Instant>,
    pub last_tap_position: Option<(f64, f64)>,

    pending_x: Option<f64>,
    pending_y: Option<f64>,
    pending_tracking_id: i32,

    pub pending_tap: bool,
}

impl GestureRecognizer {
    pub fn new(thresholds: ValidatedThresholds, x_range: (f64, f64), y_range: (f64, f64)) -> Self {
        Self {
            thresholds,
            x_range,
            y_range,
            ..Default::default()
        }
    }

    /// Reset touch tracking.
    pub fn reset(&mut self) {
        self.touch_start = None;
        self.touch_current = None;
        self.touch_points.clear();
        self.active_touches.clear();
        self.pending_x = None;
        self.pending_y = None;
        self.pending_tracking_id = 0;
    }

    /// Buffer a pending X coordinate until `SYN_REPORT`.
    pub fn set_pending_x(&mut self, x: f64) {
        self.pending_x = Some(x);
    }

    /// Buffer a pending Y coordinate until `SYN_REPORT`.
    pub fn set_pending_y(&mut self, y: f64) {
        self.pending_y = Some(y);
    }

    /// Set the tracking ID for the next touch point.
    pub fn set_tracking_id(&mut self, id: i32) {
        self.pending_tracking_id = id;
    }

    /// Commit buffered X/Y as a complete `TouchPoint` on `SYN_REPORT`.
    pub fn flush_pending(&mut self) {
        if self.pending_x.is_none() && self.pending_y.is_none() {
            return;
        }

        let point = TouchPoint {
            x: self
                .pending_x
                .unwrap_or_else(|| self.touch_current.map_or(0.0, |tc| tc.x)),
            y: self
                .pending_y
                .unwrap_or_else(|| self.touch_current.map_or(0.0, |tc| tc.y)),
            time: Instant::now(),
            tracking_id: self.pending_tracking_id,
        };
        self.active_touches.insert(self.pending_tracking_id, point);
        self.touch_points.push(point);
        self.touch_start.get_or_insert(point);
        self.touch_current = Some(point);

        self.pending_x = None;
        self.pending_y = None;
    }

    /// Recognize gesture from recorded touch data.
    pub fn recognize_gesture(&mut self) -> Option<GestureType> {
        let start = self.touch_start?;
        let current = self.touch_current?;

        if self.active_touches.len() >= 2 {
            if let Some(pinch) = self.detect_pinch() {
                return Some(pinch);
            }
        }

        if let Some(swipe) = self.detect_swipe(start, current) {
            return Some(swipe);
        }

        self.detect_stationary(start, current)
    }

    fn detect_swipe(&self, start: TouchPoint, current: TouchPoint) -> Option<GestureType> {
        let dx = current.x - start.x;
        let dy = current.y - start.y;
        let dt = current.time.duration_since(start.time).as_secs_f64();
        let th = &self.thresholds;

        if dt >= th.swipe_time_max {
            return None;
        }

        let x_span = self.x_range.1 - self.x_range.0;
        let y_span = self.y_range.1 - self.y_range.0;

        // Horizontal swipe
        if dx.abs() >= x_span * th.swipe_distance_min_pct
            && dy.abs().atan2(dx.abs()).to_degrees() <= th.angle_tolerance_deg
        {
            return Some(if dx > 0.0 {
                GestureType::SwipeRight
            } else {
                GestureType::SwipeLeft
            });
        }

        // Vertical swipe
        if dy.abs() >= y_span * th.swipe_distance_min_pct
            && dx.abs().atan2(dy.abs()).to_degrees() <= th.angle_tolerance_deg
        {
            return Some(if dy > 0.0 {
                GestureType::SwipeDown
            } else {
                GestureType::SwipeUp
            });
        }

        None
    }

    /// Detect stationary gestures: long press, tap, or double-tap.
    fn detect_stationary(&mut self, start: TouchPoint, current: TouchPoint) -> Option<GestureType> {
        let dt = current.time.duration_since(start.time).as_secs_f64();
        let distance = start.distance_to(&current);

        if dt >= self.thresholds.long_press_time_min && distance < self.thresholds.tap_distance_max
        {
            return Some(GestureType::LongPress);
        }

        if dt >= self.thresholds.tap_time_max || distance >= self.thresholds.tap_distance_max {
            return None;
        }

        let now = Instant::now();
        if let (Some(last_time), Some((lx, ly))) = (self.last_tap_time, self.last_tap_position) {
            if now.duration_since(last_time).as_secs_f64() < self.thresholds.double_tap_interval
                && (current.x - lx).hypot(current.y - ly) < self.thresholds.double_tap_distance_max
            {
                self.pending_tap = false;
                self.last_tap_time = None;
                self.last_tap_position = None;
                return Some(GestureType::DoubleTap);
            }
        }

        self.last_tap_time = Some(now);
        self.last_tap_position = Some((current.x, current.y));
        self.pending_tap = true;
        None
    }

    fn detect_pinch(&self) -> Option<GestureType> {
        if self.touch_points.len() < 4 || self.active_touches.len() < 2 {
            return None;
        }

        let p1_first = self.touch_points.first()?;
        let p2_first = self.touch_points[1..]
            .iter()
            .find(|p| p.tracking_id != p1_first.tracking_id)?;
        let first_dist = p1_first.distance_to(p2_first);

        let p1_last = self.touch_points.last()?;
        let p2_last = self.touch_points[..self.touch_points.len() - 1]
            .iter()
            .rev()
            .find(|p| p.tracking_id != p1_last.tracking_id)?;
        let last_dist = p1_last.distance_to(p2_last);

        let threshold = first_dist * self.thresholds.pinch_threshold_pct;
        if last_dist < first_dist - threshold {
            Some(GestureType::PinchIn)
        } else if last_dist > first_dist + threshold {
            Some(GestureType::PinchOut)
        } else {
            None
        }
    }

    /// Check if a tap is pending.
    pub fn has_pending_tap(&self) -> bool {
        self.pending_tap
    }

    /// Check and consume a pending tap.
    pub fn get_pending_tap(&mut self) -> bool {
        std::mem::take(&mut self.pending_tap)
    }

    /// If a single tap is pending and the double-tap window has expired,
    /// consume it and return `GestureType::Tap`.
    pub fn check_pending_tap_expired(&mut self) -> Option<GestureType> {
        if !self.pending_tap {
            return None;
        }
        let elapsed = self.last_tap_time?.elapsed().as_secs_f64();
        if elapsed >= self.thresholds.double_tap_interval {
            self.pending_tap = false;
            Some(GestureType::Tap)
        } else {
            None
        }
    }
}
