//! Touch event classification and processing - no I/O, no hardware, fully testable.
//!
//! This module contains the deterministic core of the gesture event handling:
//! event classification, touch-event processing, USB-ID parsing, and
//! action resolution.  Everything here is a plain function with no
//! side-effects.

use std::collections::HashMap;

use crate::config::GestureConfig;
use crate::recognizer::{GestureRecognizer, GestureType};

// -- TouchEvent -----------------------------------------------

/// Intermediate representation of a relevant touch event,
/// decoupled from `evdev` types for testability.
#[derive(Debug, Clone, PartialEq)]
pub enum TouchEvent {
    PositionX(f64),
    PositionY(f64),
    TrackingId(i32),
    FingerUp,
    SynReport,
}

// -- Core processing ------------------------------------------

/// Feed a sequence of [`TouchEvent`]s into a recognizer and collect any
/// gestures that fire.  This is the **core event-processing logic** - pure,
/// deterministic, and fully testable without hardware.
pub fn process_touch_events(
    recognizer: &mut GestureRecognizer,
    events: &[TouchEvent],
) -> Vec<GestureType> {
    let mut gestures = Vec::new();
    for event in events {
        match event {
            TouchEvent::PositionX(x) => recognizer.set_pending_x(*x),
            TouchEvent::PositionY(y) => recognizer.set_pending_y(*y),
            TouchEvent::TrackingId(id) => recognizer.set_tracking_id(*id),
            TouchEvent::FingerUp => {
                if let Some(g) = recognizer.check_pending_tap_expired() {
                    gestures.push(g);
                }
                if let Some(g) = recognizer.recognize_gesture() {
                    gestures.push(g);
                }
                recognizer.reset();
            }
            TouchEvent::SynReport => {
                recognizer.flush_pending();
                if let Some(g) = recognizer.check_pending_tap_expired() {
                    gestures.push(g);
                }
            }
        }
    }
    gestures
}

// -- Helpers --------------------------------------------------

/// Parse a USB vendor:product ID string into `(vendor, product)`.
///
/// Accepts formats like `"1234:5678"` or `"USB:1234:5678"` (case-insensitive).
/// Returns `None` if the format is invalid or the hex values cannot be parsed.
pub fn parse_usb_id(raw: &str) -> Option<(u16, u16)> {
    let cleaned = raw.to_lowercase().replace("usb:", "");
    let (vendor_str, product_str) = cleaned.split_once(':')?;
    let vendor = u16::from_str_radix(vendor_str, 16).ok()?;
    let product = u16::from_str_radix(product_str, 16).ok()?;
    Some((vendor, product))
}

/// Look up the action string for a recognized gesture in the device config.
///
/// Returns `Some(action)` if the gesture is configured, enabled, and has an action.
pub fn resolve_action(
    gesture: GestureType,
    gestures: &HashMap<String, GestureConfig>,
) -> Option<&str> {
    let gesture_name: &str = gesture.into();
    gestures
        .get(gesture_name)
        .filter(|gc| gc.enabled)
        .and_then(|gc| gc.action.as_deref())
}

/// Classify a single `evdev::InputEvent` into one of the touch-relevant
/// categories the handler cares about.  Returns `None` for irrelevant events.
pub fn classify_event(event: &evdev::InputEvent) -> Option<TouchEvent> {
    use evdev::{AbsoluteAxisType, InputEventKind};

    match event.kind() {
        InputEventKind::AbsAxis(axis) => match axis {
            AbsoluteAxisType::ABS_MT_POSITION_X => {
                Some(TouchEvent::PositionX(event.value() as f64))
            }
            AbsoluteAxisType::ABS_MT_POSITION_Y => {
                Some(TouchEvent::PositionY(event.value() as f64))
            }
            AbsoluteAxisType::ABS_MT_TRACKING_ID => {
                if event.value() == -1 {
                    Some(TouchEvent::FingerUp)
                } else {
                    Some(TouchEvent::TrackingId(event.value()))
                }
            }
            _ => None,
        },
        InputEventKind::Synchronization(evdev::Synchronization::SYN_REPORT) => {
            Some(TouchEvent::SynReport)
        }
        _ => None,
    }
}
