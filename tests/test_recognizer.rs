//! Tests for `bodgestr::recognizer` - `GestureType`, `TouchPoint`, and `GestureRecognizer`.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use bodgestr::config::ValidatedThresholds;
use bodgestr::recognizer::{GestureRecognizer, GestureType, TouchPoint};

/// Screen range used for all tests: 0–1000 in both axes.
const X_RANGE: (f64, f64) = (0.0, 1000.0);
const Y_RANGE: (f64, f64) = (0.0, 1000.0);

fn default_thresholds() -> ValidatedThresholds {
    ValidatedThresholds {
        swipe_time_max: 0.9,
        swipe_distance_min_pct: 0.15,
        angle_tolerance_deg: 30.0,
        tap_time_max: 0.2,
        long_press_time_min: 0.8,
        double_tap_interval: 0.3,
        tap_distance_max: 50.0,
        double_tap_distance_max: 50.0,
        pinch_threshold_pct: 0.1,
    }
}

fn make_recognizer(thresholds: Option<ValidatedThresholds>) -> GestureRecognizer {
    let th = thresholds.unwrap_or_else(default_thresholds);
    GestureRecognizer::new(th, X_RANGE, Y_RANGE)
}

fn simulate_touch(
    rec: &mut GestureRecognizer,
    x_start: f64,
    y_start: f64,
    x_end: f64,
    y_end: f64,
    duration: f64,
    tracking_id: i32,
) {
    let now = Instant::now();
    let start = TouchPoint {
        x: x_start,
        y: y_start,
        time: now,
        tracking_id,
    };
    let end = TouchPoint {
        x: x_end,
        y: y_end,
        time: now + Duration::from_secs_f64(duration),
        tracking_id,
    };
    rec.touch_start = Some(start);
    rec.touch_current = Some(end);
    rec.touch_points = vec![start, end];
    rec.active_touches = HashMap::from([(tracking_id, end)]);
}

// -- Swipe tests ------------------------------------------

#[test]
fn test_swipe_left() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 800.0, 500.0, 100.0, 500.0, 0.3, 0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::SwipeLeft));
}

#[test]
fn test_swipe_right() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 100.0, 500.0, 800.0, 500.0, 0.3, 0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::SwipeRight));
}

#[test]
fn test_swipe_up() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 800.0, 500.0, 100.0, 0.3, 0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::SwipeUp));
}

#[test]
fn test_swipe_down() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 100.0, 500.0, 800.0, 0.3, 0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::SwipeDown));
}

#[test]
fn test_swipe_too_slow() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 800.0, 500.0, 100.0, 500.0, 2.0, 0);
    let result = rec.recognize_gesture();
    assert_ne!(result, Some(GestureType::SwipeLeft));
}

#[test]
fn test_swipe_too_short() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 510.0, 500.0, 0.3, 0);
    let result = rec.recognize_gesture();
    assert!(
        result != Some(GestureType::SwipeLeft)
            && result != Some(GestureType::SwipeRight)
            && result != Some(GestureType::SwipeUp)
            && result != Some(GestureType::SwipeDown)
    );
}

#[test]
fn test_diagonal_rejected() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 100.0, 100.0, 900.0, 900.0, 0.3, 0);
    let result = rec.recognize_gesture();
    assert!(
        result != Some(GestureType::SwipeLeft)
            && result != Some(GestureType::SwipeRight)
            && result != Some(GestureType::SwipeUp)
            && result != Some(GestureType::SwipeDown)
    );
}

// -- Tap tests --------------------------------------------

#[test]
fn test_single_tap_sets_pending() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 0.05, 0);
    let result = rec.recognize_gesture();
    // First tap returns None (waiting for possible double tap)
    assert_eq!(result, None);
    assert!(rec.has_pending_tap());
}

#[test]
fn test_get_pending_tap_consumes() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 0.05, 0);
    rec.recognize_gesture();
    assert!(rec.get_pending_tap());
    assert!(!rec.get_pending_tap());
}

#[test]
fn test_double_tap() {
    let mut rec = make_recognizer(None);

    // First tap
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 0.05, 0);
    let result1 = rec.recognize_gesture();
    assert_eq!(result1, None);

    // Second tap shortly after
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 0.05, 0);
    let result2 = rec.recognize_gesture();
    assert_eq!(result2, Some(GestureType::DoubleTap));
}

#[test]
fn test_tap_too_long_is_not_tap() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 0.5, 0);
    let result = rec.recognize_gesture();
    assert_ne!(result, Some(GestureType::Tap));
    assert!(!rec.has_pending_tap());
}

#[test]
fn test_tap_with_movement_rejected() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 600.0, 600.0, 0.05, 0);
    rec.recognize_gesture();
    assert!(!rec.has_pending_tap());
}

// -- Long press tests ------------------------------------

#[test]
fn test_long_press() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 1.5, 0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::LongPress));
}

#[test]
fn test_long_press_with_slight_movement() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 505.0, 505.0, 1.5, 0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::LongPress));
}

#[test]
fn test_long_press_with_too_much_movement() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 700.0, 700.0, 1.5, 0);
    assert_ne!(rec.recognize_gesture(), Some(GestureType::LongPress));
}

// -- Pinch tests ------------------------------------------

fn simulate_pinch(rec: &mut GestureRecognizer, start_dist: f64, end_dist: f64) {
    let now = Instant::now();
    let center = 500.0;

    let p1_start = TouchPoint {
        x: center - start_dist / 2.0,
        y: center,
        time: now,
        tracking_id: 0,
    };
    let p2_start = TouchPoint {
        x: center + start_dist / 2.0,
        y: center,
        time: now,
        tracking_id: 1,
    };
    let p1_end = TouchPoint {
        x: center - end_dist / 2.0,
        y: center,
        time: now + Duration::from_secs_f64(0.3),
        tracking_id: 0,
    };
    let p2_end = TouchPoint {
        x: center + end_dist / 2.0,
        y: center,
        time: now + Duration::from_secs_f64(0.3),
        tracking_id: 1,
    };

    rec.touch_start = Some(p1_start);
    rec.touch_current = Some(p1_end);
    rec.touch_points = vec![p1_start, p2_start, p1_end, p2_end];
    rec.active_touches = HashMap::from([(0, p1_end), (1, p2_end)]);
}

#[test]
fn test_pinch_in() {
    let mut rec = make_recognizer(None);
    simulate_pinch(&mut rec, 400.0, 100.0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::PinchIn));
}

#[test]
fn test_pinch_out() {
    let mut rec = make_recognizer(None);
    simulate_pinch(&mut rec, 100.0, 400.0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::PinchOut));
}

#[test]
fn test_pinch_no_movement() {
    let mut rec = make_recognizer(None);
    simulate_pinch(&mut rec, 200.0, 200.0);
    let result = rec.recognize_gesture();
    assert!(result != Some(GestureType::PinchIn) && result != Some(GestureType::PinchOut));
}

#[test]
fn test_pinch_needs_enough_points() {
    let mut rec = make_recognizer(None);
    let now = Instant::now();
    let later = now + Duration::from_secs_f64(0.3);
    let start = TouchPoint {
        x: 400.0,
        y: 500.0,
        time: now,
        tracking_id: 0,
    };
    let current = TouchPoint {
        x: 600.0,
        y: 500.0,
        time: later,
        tracking_id: 0,
    };
    rec.touch_start = Some(start);
    rec.touch_current = Some(current);
    rec.touch_points = vec![start, current];
    rec.active_touches = HashMap::from([
        (0, current),
        (
            1,
            TouchPoint {
                x: 700.0,
                y: 500.0,
                time: later,
                tracking_id: 1,
            },
        ),
    ]);
    // Only 2 points - pinch should not trigger
    let result = rec.recognize_gesture();
    assert!(result != Some(GestureType::PinchIn) && result != Some(GestureType::PinchOut));
}

// -- Reset tests -----------------------------------------

#[test]
fn test_reset_clears_state() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 100.0, 100.0, 900.0, 100.0, 0.3, 0);
    assert_eq!(rec.recognize_gesture(), Some(GestureType::SwipeRight));

    rec.reset();
    assert!(rec.touch_start.is_none());
    assert!(rec.touch_current.is_none());
    assert!(rec.touch_points.is_empty());
    assert!(rec.active_touches.is_empty());
    assert_eq!(rec.recognize_gesture(), None);
}

// -- Flush pending tests ---------------------------------

#[test]
fn test_flush_creates_touch_point() {
    let mut rec = make_recognizer(None);
    rec.set_pending_x(100.0);
    rec.set_pending_y(200.0);
    rec.set_tracking_id(5);
    rec.flush_pending();

    assert!(rec.touch_start.is_some());
    let start = rec.touch_start.unwrap();
    assert_eq!(start.x, 100.0);
    assert_eq!(start.y, 200.0);
    assert_eq!(start.tracking_id, 5);
    assert_eq!(rec.touch_current, rec.touch_start);
}

#[test]
fn test_flush_nothing_pending() {
    let mut rec = make_recognizer(None);
    rec.flush_pending();
    assert!(rec.touch_start.is_none());
}

#[test]
fn test_flush_partial_x_only() {
    let mut rec = make_recognizer(None);
    rec.set_pending_x(300.0);
    rec.flush_pending();
    assert_eq!(rec.touch_current.unwrap().x, 300.0);
    assert_eq!(rec.touch_current.unwrap().y, 0.0);
}

#[test]
fn test_flush_preserves_previous_y() {
    let mut rec = make_recognizer(None);
    rec.set_pending_x(100.0);
    rec.set_pending_y(200.0);
    rec.flush_pending();
    rec.set_pending_x(150.0);
    rec.flush_pending();
    assert_eq!(rec.touch_current.unwrap().x, 150.0);
    assert_eq!(rec.touch_current.unwrap().y, 200.0);
}

#[test]
fn test_multiple_flushes_append_points() {
    let mut rec = make_recognizer(None);
    rec.set_pending_x(10.0);
    rec.set_pending_y(20.0);
    rec.flush_pending();
    rec.set_pending_x(30.0);
    rec.set_pending_y(40.0);
    rec.flush_pending();
    assert_eq!(rec.touch_points.len(), 2);
}

// -- Custom thresholds tests -----------------------------

#[test]
fn test_stricter_swipe_distance() {
    let th = ValidatedThresholds {
        swipe_distance_min_pct: 0.9, // Very strict
        ..default_thresholds()
    };
    let mut rec = make_recognizer(Some(th));
    // Move 300px on a 1000px screen = 30% - below 90%
    simulate_touch(&mut rec, 500.0, 500.0, 200.0, 500.0, 0.3, 0);
    let result = rec.recognize_gesture();
    assert_ne!(result, Some(GestureType::SwipeLeft));
}

#[test]
fn test_longer_tap_time_allows_slower_taps() {
    let th = ValidatedThresholds {
        tap_time_max: 0.5, // Allow longer taps
        ..default_thresholds()
    };
    let mut rec = make_recognizer(Some(th));
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 0.3, 0);
    rec.recognize_gesture();
    assert!(rec.has_pending_tap());
}

// -- GestureType tests -----------------------------------

#[test]
fn test_all_gesture_values() {
    let expected = [
        (GestureType::SwipeLeft, "swipe_left"),
        (GestureType::SwipeRight, "swipe_right"),
        (GestureType::SwipeUp, "swipe_up"),
        (GestureType::SwipeDown, "swipe_down"),
        (GestureType::Tap, "tap"),
        (GestureType::DoubleTap, "double_tap"),
        (GestureType::LongPress, "long_press"),
        (GestureType::PinchIn, "pinch_in"),
        (GestureType::PinchOut, "pinch_out"),
    ];
    for (gesture, value) in &expected {
        assert_eq!(gesture.to_string(), *value);
    }
}

#[test]
fn test_gesture_count() {
    let all = [
        GestureType::SwipeLeft,
        GestureType::SwipeRight,
        GestureType::SwipeUp,
        GestureType::SwipeDown,
        GestureType::Tap,
        GestureType::DoubleTap,
        GestureType::LongPress,
        GestureType::PinchIn,
        GestureType::PinchOut,
    ];
    assert_eq!(all.len(), 9);
}

#[test]
fn test_gesture_from_str() {
    assert_eq!(
        "swipe_left".parse::<GestureType>(),
        Ok(GestureType::SwipeLeft)
    );
    assert_eq!("tap".parse::<GestureType>(), Ok(GestureType::Tap));
    assert!("unknown".parse::<GestureType>().is_err());
}

#[test]
fn test_gesture_display() {
    assert_eq!(format!("{}", GestureType::Tap), "tap");
    assert_eq!(format!("{}", GestureType::DoubleTap), "double_tap");
}

// -- TouchPoint tests ------------------------------------

#[test]
fn test_basic_creation() {
    let now = Instant::now();
    let p = TouchPoint {
        x: 100.0,
        y: 200.0,
        time: now,
        tracking_id: -1,
    };
    assert_eq!(p.x, 100.0);
    assert_eq!(p.y, 200.0);
    assert_eq!(p.time, now);
    assert_eq!(p.tracking_id, -1);
}

#[test]
fn test_custom_tracking_id() {
    let p = TouchPoint {
        x: 0.0,
        y: 0.0,
        time: Instant::now(),
        tracking_id: 42,
    };
    assert_eq!(p.tracking_id, 42);
}

// -- check_pending_tap_expired tests ----------------------

#[test]
fn test_pending_tap_expires_after_double_tap_interval() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 0.05, 0);
    rec.recognize_gesture();
    assert!(rec.has_pending_tap());

    // Force last_tap_time far enough into the past
    rec.last_tap_time = Some(Instant::now() - Duration::from_secs_f64(1.0));

    let result = rec.check_pending_tap_expired();
    assert_eq!(result, Some(GestureType::Tap));
    assert!(!rec.has_pending_tap());
}

#[test]
fn test_pending_tap_does_not_expire_within_interval() {
    let mut rec = make_recognizer(None);
    simulate_touch(&mut rec, 500.0, 500.0, 500.0, 500.0, 0.05, 0);
    rec.recognize_gesture();
    assert!(rec.has_pending_tap());

    // last_tap_time is just set - well within the double_tap_interval
    let result = rec.check_pending_tap_expired();
    assert_eq!(result, None);
    assert!(rec.has_pending_tap());
}

#[test]
fn test_check_expired_returns_none_when_no_pending_tap() {
    let mut rec = make_recognizer(None);
    assert_eq!(rec.check_pending_tap_expired(), None);
}

// -- GestureType IntoStaticStr test -----------------------

#[test]
fn test_gesture_into_static_str() {
    let name: &str = GestureType::SwipeLeft.into();
    assert_eq!(name, "swipe_left");

    let name: &str = GestureType::DoubleTap.into();
    assert_eq!(name, "double_tap");

    let name: &str = GestureType::PinchOut.into();
    assert_eq!(name, "pinch_out");
}
