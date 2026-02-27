//! Integration tests for the event-processing logic in `event`.
//!
//! Tests use `TouchEvent` directly (no hardware) and also verify
//! `classify_event` with synthetic `evdev::InputEvent`s.
use std::collections::HashMap;

use bodgestr::config::{GestureConfig, ValidatedThresholds};
use bodgestr::event::{
    TouchEvent, classify_event, parse_usb_id, process_touch_events, resolve_action,
};
use bodgestr::recognizer::{GestureRecognizer, GestureType};
use evdev::{AbsoluteAxisType, EventType, InputEvent, Synchronization};

// -- Helpers --------------------------------------------------

fn default_thresholds() -> ValidatedThresholds {
    ValidatedThresholds {
        swipe_time_max: 0.9,
        swipe_distance_min_pct: 0.15,
        angle_tolerance_deg: 30.0,
        tap_time_max: 0.5,
        long_press_time_min: 0.8,
        double_tap_interval: 0.3,
        tap_distance_max: 50.0,
        double_tap_distance_max: 50.0,
        pinch_threshold_pct: 0.1,
    }
}

fn make_recognizer() -> GestureRecognizer {
    GestureRecognizer::new(default_thresholds(), (0.0, 1000.0), (0.0, 1000.0))
}

fn make_gestures(entries: &[(&str, &str, bool)]) -> HashMap<String, GestureConfig> {
    entries
        .iter()
        .map(|(name, action, enabled)| {
            (
                name.to_string(),
                GestureConfig {
                    action: if action.is_empty() {
                        None
                    } else {
                        Some(action.to_string())
                    },
                    enabled: *enabled,
                },
            )
        })
        .collect()
}

/// Shorthand: feed TouchEvents, return recognized gestures.
fn feed(events: &[TouchEvent]) -> Vec<GestureType> {
    let mut rec = make_recognizer();
    process_touch_events(&mut rec, events)
}

/// Build a swipe-left event sequence.
fn swipe_left() -> Vec<TouchEvent> {
    vec![
        TouchEvent::TrackingId(0),
        TouchEvent::PositionX(800.0),
        TouchEvent::PositionY(500.0),
        TouchEvent::SynReport,
        TouchEvent::PositionX(100.0),
        TouchEvent::PositionY(500.0),
        TouchEvent::SynReport,
        TouchEvent::FingerUp,
    ]
}

fn swipe_right() -> Vec<TouchEvent> {
    vec![
        TouchEvent::TrackingId(0),
        TouchEvent::PositionX(100.0),
        TouchEvent::PositionY(500.0),
        TouchEvent::SynReport,
        TouchEvent::PositionX(800.0),
        TouchEvent::PositionY(500.0),
        TouchEvent::SynReport,
        TouchEvent::FingerUp,
    ]
}

fn swipe_up() -> Vec<TouchEvent> {
    vec![
        TouchEvent::TrackingId(0),
        TouchEvent::PositionX(500.0),
        TouchEvent::PositionY(800.0),
        TouchEvent::SynReport,
        TouchEvent::PositionX(500.0),
        TouchEvent::PositionY(100.0),
        TouchEvent::SynReport,
        TouchEvent::FingerUp,
    ]
}

fn swipe_down() -> Vec<TouchEvent> {
    vec![
        TouchEvent::TrackingId(0),
        TouchEvent::PositionX(500.0),
        TouchEvent::PositionY(100.0),
        TouchEvent::SynReport,
        TouchEvent::PositionX(500.0),
        TouchEvent::PositionY(800.0),
        TouchEvent::SynReport,
        TouchEvent::FingerUp,
    ]
}

// -- process_touch_events: swipe recognition ------------------

#[test]
fn test_swipe_left() {
    let gestures = feed(&swipe_left());
    assert_eq!(gestures, vec![GestureType::SwipeLeft]);
}

#[test]
fn test_swipe_right() {
    let gestures = feed(&swipe_right());
    assert_eq!(gestures, vec![GestureType::SwipeRight]);
}

#[test]
fn test_swipe_up() {
    let gestures = feed(&swipe_up());
    assert_eq!(gestures, vec![GestureType::SwipeUp]);
}

#[test]
fn test_swipe_down() {
    let gestures = feed(&swipe_down());
    assert_eq!(gestures, vec![GestureType::SwipeDown]);
}

// -- process_touch_events: edge cases -------------------------

#[test]
fn test_small_movement_no_gesture() {
    let gestures = feed(&[
        TouchEvent::TrackingId(0),
        TouchEvent::PositionX(500.0),
        TouchEvent::PositionY(500.0),
        TouchEvent::SynReport,
        TouchEvent::PositionX(510.0),
        TouchEvent::PositionY(505.0),
        TouchEvent::SynReport,
        TouchEvent::FingerUp,
    ]);
    // Too small for a swipe, and the "tap" path depends on timing -
    // but it should definitely NOT be a swipe.
    assert!(!gestures.contains(&GestureType::SwipeLeft));
    assert!(!gestures.contains(&GestureType::SwipeRight));
    assert!(!gestures.contains(&GestureType::SwipeUp));
    assert!(!gestures.contains(&GestureType::SwipeDown));
}

#[test]
fn test_diagonal_no_swipe() {
    let gestures = feed(&[
        TouchEvent::TrackingId(0),
        TouchEvent::PositionX(100.0),
        TouchEvent::PositionY(100.0),
        TouchEvent::SynReport,
        TouchEvent::PositionX(900.0),
        TouchEvent::PositionY(900.0),
        TouchEvent::SynReport,
        TouchEvent::FingerUp,
    ]);
    let swipes: Vec<_> = gestures
        .iter()
        .filter(|g| {
            matches!(
                g,
                GestureType::SwipeLeft
                    | GestureType::SwipeRight
                    | GestureType::SwipeUp
                    | GestureType::SwipeDown
            )
        })
        .collect();
    assert!(swipes.is_empty());
}

#[test]
fn test_two_swipes_in_sequence() {
    let mut rec = make_recognizer();
    let g1 = process_touch_events(&mut rec, &swipe_left());
    let g2 = process_touch_events(&mut rec, &swipe_right());
    assert_eq!(g1, vec![GestureType::SwipeLeft]);
    assert_eq!(g2, vec![GestureType::SwipeRight]);
}

#[test]
fn test_empty_events_no_gesture() {
    let gestures = feed(&[]);
    assert!(gestures.is_empty());
}

#[test]
fn test_syn_report_only_no_gesture() {
    let gestures = feed(&[TouchEvent::SynReport, TouchEvent::SynReport]);
    assert!(gestures.is_empty());
}

#[test]
fn test_finger_up_without_touch_no_gesture() {
    let gestures = feed(&[TouchEvent::FingerUp]);
    assert!(gestures.is_empty());
}

#[test]
fn test_recognizer_reset_after_finger_up() {
    let mut rec = make_recognizer();
    // After a swipe + finger_up, the recognizer should be reset.
    // A second identical swipe sequence must also produce a gesture,
    // proving the state was cleared.
    let g1 = process_touch_events(&mut rec, &swipe_left());
    let g2 = process_touch_events(&mut rec, &swipe_left());
    assert_eq!(g1, vec![GestureType::SwipeLeft]);
    assert_eq!(g2, vec![GestureType::SwipeLeft]);
}

// -- classify_event: evdev → TouchEvent -----------------------

#[test]
fn test_classify_mt_position_x() {
    let ev = InputEvent::new(
        EventType::ABSOLUTE,
        AbsoluteAxisType::ABS_MT_POSITION_X.0,
        42,
    );
    assert_eq!(classify_event(&ev), Some(TouchEvent::PositionX(42.0)));
}

#[test]
fn test_classify_mt_position_y() {
    let ev = InputEvent::new(
        EventType::ABSOLUTE,
        AbsoluteAxisType::ABS_MT_POSITION_Y.0,
        99,
    );
    assert_eq!(classify_event(&ev), Some(TouchEvent::PositionY(99.0)));
}

#[test]
fn test_classify_tracking_id_new_finger() {
    let ev = InputEvent::new(
        EventType::ABSOLUTE,
        AbsoluteAxisType::ABS_MT_TRACKING_ID.0,
        5,
    );
    assert_eq!(classify_event(&ev), Some(TouchEvent::TrackingId(5)));
}

#[test]
fn test_classify_tracking_id_finger_up() {
    let ev = InputEvent::new(
        EventType::ABSOLUTE,
        AbsoluteAxisType::ABS_MT_TRACKING_ID.0,
        -1,
    );
    assert_eq!(classify_event(&ev), Some(TouchEvent::FingerUp));
}

#[test]
fn test_classify_syn_report() {
    let ev = InputEvent::new(EventType::SYNCHRONIZATION, Synchronization::SYN_REPORT.0, 0);
    assert_eq!(classify_event(&ev), Some(TouchEvent::SynReport));
}

#[test]
fn test_classify_irrelevant_abs_axis() {
    // ABS_X (not multi-touch) should be ignored
    let ev = InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, 100);
    assert_eq!(classify_event(&ev), None);
}

#[test]
fn test_classify_key_event_ignored() {
    let ev = InputEvent::new(EventType::KEY, 0x110, 1); // BTN_LEFT
    assert_eq!(classify_event(&ev), None);
}

// -- resolve_action -------------------------------------------

#[test]
fn test_resolve_action_enabled() {
    let g = make_gestures(&[("swipe_left", "echo left", true)]);
    assert_eq!(
        resolve_action(GestureType::SwipeLeft, &g),
        Some("echo left")
    );
}

#[test]
fn test_resolve_action_disabled() {
    let g = make_gestures(&[("swipe_left", "echo left", false)]);
    assert_eq!(resolve_action(GestureType::SwipeLeft, &g), None);
}

#[test]
fn test_resolve_action_no_action_string() {
    let g = make_gestures(&[("tap", "", true)]);
    assert_eq!(resolve_action(GestureType::Tap, &g), None);
}

#[test]
fn test_resolve_action_not_configured() {
    let g = make_gestures(&[("tap", "echo tap", true)]);
    assert_eq!(resolve_action(GestureType::SwipeLeft, &g), None);
}

#[test]
fn test_resolve_action_empty_map() {
    let g = HashMap::new();
    assert_eq!(resolve_action(GestureType::Tap, &g), None);
}

#[test]
fn test_resolve_action_all_gesture_types() {
    let all = [
        ("swipe_left", GestureType::SwipeLeft),
        ("swipe_right", GestureType::SwipeRight),
        ("swipe_up", GestureType::SwipeUp),
        ("swipe_down", GestureType::SwipeDown),
        ("tap", GestureType::Tap),
        ("double_tap", GestureType::DoubleTap),
        ("long_press", GestureType::LongPress),
        ("pinch_in", GestureType::PinchIn),
        ("pinch_out", GestureType::PinchOut),
    ];
    for (name, gesture_type) in &all {
        let action = format!("echo {name}");
        let g = make_gestures(&[(name, &action, true)]);
        assert_eq!(
            resolve_action(*gesture_type, &g),
            Some(action.as_str()),
            "Failed for gesture {name}"
        );
    }
}

// -- parse_usb_id ---------------------------------------------

#[test]
fn test_parse_usb_id_valid() {
    assert_eq!(parse_usb_id("1234:5678"), Some((0x1234, 0x5678)));
}

#[test]
fn test_parse_usb_id_uppercase() {
    assert_eq!(parse_usb_id("ABCD:EF01"), Some((0xABCD, 0xEF01)));
}

#[test]
fn test_parse_usb_id_with_usb_prefix() {
    assert_eq!(parse_usb_id("USB:1234:5678"), Some((0x1234, 0x5678)));
}

#[test]
fn test_parse_usb_id_invalid_no_colon() {
    assert_eq!(parse_usb_id("12345678"), None);
}

#[test]
fn test_parse_usb_id_invalid_hex() {
    assert_eq!(parse_usb_id("ZZZZ:0000"), None);
}

#[test]
fn test_parse_usb_id_empty() {
    assert_eq!(parse_usb_id(""), None);
}

// -- End-to-end: events → action lookup -----------------------

#[test]
fn test_end_to_end_swipe_fires_correct_action() {
    let gestures = feed(&swipe_left());
    let config_gestures = make_gestures(&[
        ("swipe_left", "xdotool key ctrl+shift+Tab", true),
        ("swipe_right", "xdotool key ctrl+Tab", true),
    ]);
    let actions: Vec<_> = gestures
        .iter()
        .filter_map(|g| resolve_action(*g, &config_gestures))
        .collect();
    assert_eq!(actions, vec!["xdotool key ctrl+shift+Tab"]);
}

#[test]
fn test_end_to_end_disabled_gesture_no_action() {
    let gestures = feed(&swipe_left());
    let config_gestures = make_gestures(&[("swipe_left", "echo left", false)]);
    let actions: Vec<_> = gestures
        .iter()
        .filter_map(|g| resolve_action(*g, &config_gestures))
        .collect();
    assert!(actions.is_empty());
}

#[test]
fn test_end_to_end_unconfigured_gesture_no_action() {
    let gestures = feed(&swipe_left());
    let config_gestures = make_gestures(&[("tap", "echo tap", true)]);
    let actions: Vec<_> = gestures
        .iter()
        .filter_map(|g| resolve_action(*g, &config_gestures))
        .collect();
    assert!(actions.is_empty());
}

#[test]
fn test_end_to_end_two_swipes_two_actions() {
    let mut rec = make_recognizer();
    let mut all_gestures = process_touch_events(&mut rec, &swipe_left());
    all_gestures.extend(process_touch_events(&mut rec, &swipe_right()));

    let config_gestures = make_gestures(&[
        ("swipe_left", "echo left", true),
        ("swipe_right", "echo right", true),
    ]);
    let actions: Vec<_> = all_gestures
        .iter()
        .filter_map(|g| resolve_action(*g, &config_gestures))
        .collect();
    assert_eq!(actions, vec!["echo left", "echo right"]);
}
