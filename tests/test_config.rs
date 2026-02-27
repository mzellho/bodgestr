//! Tests for `bodgestr::config` - TOML parsing, threshold merging,
//! gesture inheritance, device filtering, and error handling.

use std::io::Write;
use tempfile::NamedTempFile;

use bodgestr::config::{AppConfig, parse_config_file};

// ── Helpers ──────────────────────────────────────────────────

/// All required thresholds as a TOML snippet for embedding in test configs.
const ALL_THRESHOLDS: &str = r#"
[global.thresholds]
swipe_time_max = 0.9
swipe_distance_min_pct = 0.15
angle_tolerance_deg = 30.0
tap_time_max = 0.2
tap_distance_max = 50.0
long_press_time_min = 0.8
double_tap_interval = 0.3
double_tap_distance_max = 50.0
pinch_threshold_pct = 0.1
"#;

/// Write TOML to a temp file and parse it. Optionally prepends global thresholds.
fn load(toml_content: &str, with_thresholds: bool) -> AppConfig {
    let full = if with_thresholds {
        format!("{ALL_THRESHOLDS}\n{toml_content}")
    } else {
        toml_content.to_string()
    };
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(full.as_bytes()).unwrap();
    f.flush().unwrap();
    parse_config_file(f.path()).unwrap()
}

/// Parse raw TOML that is expected to fail.
fn load_err(toml_content: &str) -> String {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml_content.as_bytes()).unwrap();
    f.flush().unwrap();
    parse_config_file(f.path()).unwrap_err().to_string()
}

// ── Error handling ───────────────────────────────────────────

#[test]
fn test_file_not_found() {
    let msg = parse_config_file(std::path::Path::new("/no/such/file.toml"))
        .unwrap_err()
        .to_string();
    assert!(msg.contains("Failed to read config file"));
    assert!(msg.contains("/no/such/file.toml"));
}

#[test]
fn test_invalid_toml() {
    let msg = load_err("this is not valid toml [[[");
    assert!(msg.contains("Failed to parse config file"));
}

#[test]
fn test_missing_thresholds_lists_field_names() {
    let msg = load_err(
        r#"
[global.thresholds]
swipe_time_max = 0.9

[device.d1]
device_usb_id = "1234:5678"
enabled = true
"#,
    );
    assert!(msg.contains("tap_time_max"));
    assert!(msg.contains("long_press_time_min"));
}

// ── Empty / minimal configs ──────────────────────────────────

#[test]
fn test_empty_config() {
    let config = load("", false);
    assert!(config.devices.is_empty());
    assert_eq!(config.log_level, "info");
}

#[test]
fn test_global_log_level() {
    let config = load(
        r#"
[global]
log_level = "WARNING"
"#,
        true,
    );
    assert_eq!(config.log_level, "WARNING");
}

#[test]
fn test_unknown_keys_ignored() {
    let config = load(
        r#"
[foobar]
setting = "value"

[device.d1]
device_usb_id = "1111:2222"
enabled = true
"#,
        true,
    );
    assert!(config.devices.contains_key("d1"));
}

// ── Device filtering ─────────────────────────────────────────

#[test]
fn test_device_disabled_by_default() {
    let config = load(
        r#"
[device.d1]
device_usb_id = "1234:5678"
"#,
        true,
    );
    assert!(!config.devices.contains_key("d1"));
}

#[test]
fn test_device_explicitly_disabled() {
    let config = load(
        r#"
[device.d1]
device_usb_id = "1234:5678"
enabled = false
"#,
        true,
    );
    assert!(!config.devices.contains_key("d1"));
}

#[test]
fn test_device_without_usb_id_skipped() {
    let config = load(
        r#"
[device.d1]
enabled = true

[device.d1.gestures.tap]
action = "echo tap"
enabled = true
"#,
        true,
    );
    assert!(!config.devices.contains_key("d1"));
}

#[test]
fn test_device_with_empty_usb_id_skipped() {
    let config = load(
        r#"
[device.d1]
device_usb_id = ""
enabled = true
"#,
        true,
    );
    assert!(!config.devices.contains_key("d1"));
}

#[test]
fn test_enabled_device_loaded() {
    let config = load(
        r#"
[device.d1]
device_usb_id = "1111:2222"
enabled = true
"#,
        true,
    );
    assert_eq!(config.devices["d1"].device_usb_id, "1111:2222");
}

#[test]
fn test_multiple_devices() {
    let config = load(
        r#"
[device.a]
device_usb_id = "1111:1111"
enabled = true

[device.b]
device_usb_id = "2222:2222"
enabled = true
"#,
        true,
    );
    assert!(config.devices.contains_key("a"));
    assert!(config.devices.contains_key("b"));
}

// ── Threshold merging ────────────────────────────────────────

#[test]
fn test_complete_thresholds_pass() {
    let config = load(
        r#"
[device.d1]
device_usb_id = "1234:5678"
enabled = true
"#,
        true,
    );
    assert!(config.devices.contains_key("d1"));
}

#[test]
fn test_device_inherits_global_thresholds() {
    let config = load(
        r#"
[global.thresholds]
swipe_time_max = 2.0
swipe_distance_min_pct = 0.15
angle_tolerance_deg = 30.0
tap_time_max = 0.2
long_press_time_min = 0.8
double_tap_interval = 0.3
tap_distance_max = 80.0
double_tap_distance_max = 50.0
pinch_threshold_pct = 0.1

[device.d1]
device_usb_id = "1234:5678"
enabled = true
"#,
        false,
    );
    let th = &config.devices["d1"].thresholds;
    assert_eq!(th.swipe_time_max, 2.0);
    assert_eq!(th.tap_distance_max, 80.0);
}

#[test]
fn test_device_overrides_global_thresholds() {
    let config = load(
        r#"
[device.d1]
device_usb_id = "1234:5678"
enabled = true

[device.d1.thresholds]
swipe_time_max = 3.0
"#,
        true,
    );
    let th = &config.devices["d1"].thresholds;
    assert_eq!(th.swipe_time_max, 3.0);
    assert_eq!(th.tap_time_max, 0.2); // inherited
}

#[test]
fn test_all_threshold_fields() {
    let config = load(
        r#"
[device.d1]
device_usb_id = "1111:2222"
enabled = true

[device.d1.thresholds]
swipe_time_max = 1.1
swipe_distance_min_pct = 0.2
angle_tolerance_deg = 25.0
tap_time_max = 0.3
long_press_time_min = 1.0
double_tap_interval = 0.4
tap_distance_max = 40.0
double_tap_distance_max = 55.0
pinch_threshold_pct = 0.15
"#,
        true,
    );
    let th = &config.devices["d1"].thresholds;
    assert_eq!(th.swipe_time_max, 1.1);
    assert_eq!(th.swipe_distance_min_pct, 0.2);
    assert_eq!(th.angle_tolerance_deg, 25.0);
    assert_eq!(th.tap_time_max, 0.3);
    assert_eq!(th.long_press_time_min, 1.0);
    assert_eq!(th.double_tap_interval, 0.4);
    assert_eq!(th.tap_distance_max, 40.0);
    assert_eq!(th.double_tap_distance_max, 55.0);
    assert_eq!(th.pinch_threshold_pct, 0.15);
}

// ── Gesture configuration ────────────────────────────────────

#[test]
fn test_device_gesture() {
    let config = load(
        r#"
[device.d1]
device_usb_id = "1234:5678"
enabled = true

[device.d1.gestures.tap]
action = "echo tap"
enabled = true
"#,
        true,
    );
    let g = &config.devices["d1"].gestures["tap"];
    assert_eq!(g.action, Some("echo tap".to_string()));
    assert!(g.enabled);
}

#[test]
fn test_all_gesture_types_configurable() {
    let names = [
        "swipe_left",
        "swipe_right",
        "swipe_up",
        "swipe_down",
        "tap",
        "double_tap",
        "long_press",
        "pinch_in",
        "pinch_out",
    ];
    let gesture_toml: String = names
        .iter()
        .map(|g| format!("[device.d1.gestures.{g}]\naction = \"echo {g}\"\nenabled = true\n\n"))
        .collect();
    let config = load(
        &format!(
            r#"
[device.d1]
device_usb_id = "1111:2222"
enabled = true

{gesture_toml}
"#
        ),
        true,
    );
    for g in &names {
        assert!(
            config.devices["d1"].gestures[*g].enabled,
            "gesture {g} not enabled"
        );
    }
}

// ── Global gesture inheritance ───────────────────────────────

#[test]
fn test_global_gestures_inherited() {
    let config = load(
        r#"
[global.gestures.tap]
action = "xdotool click 1"
enabled = true

[global.gestures.swipe_left]
action = "xdotool key ctrl+shift+Tab"
enabled = true

[device.d1]
device_usb_id = "1111:1111"
enabled = true
"#,
        true,
    );
    let d1 = &config.devices["d1"];
    assert_eq!(d1.gestures["tap"].action, Some("xdotool click 1".into()));
    assert_eq!(
        d1.gestures["swipe_left"].action,
        Some("xdotool key ctrl+shift+Tab".into())
    );
}

#[test]
fn test_device_overrides_global_gesture() {
    let config = load(
        r#"
[global.gestures.tap]
action = "xdotool click 1"
enabled = true

[device.d1]
device_usb_id = "1111:1111"
enabled = true

[device.d1.gestures.tap]
action = "xdotool click 3"
"#,
        true,
    );
    assert_eq!(
        config.devices["d1"].gestures["tap"].action,
        Some("xdotool click 3".into())
    );
    assert!(config.devices["d1"].gestures["tap"].enabled);
}

#[test]
fn test_device_disables_global_gesture() {
    let config = load(
        r#"
[global.gestures.tap]
action = "xdotool click 1"
enabled = true

[device.d1]
device_usb_id = "1111:1111"
enabled = true

[device.d1.gestures.tap]
enabled = false
"#,
        true,
    );
    assert!(!config.devices["d1"].gestures["tap"].enabled);
    assert_eq!(
        config.devices["d1"].gestures["tap"].action,
        Some("xdotool click 1".into())
    );
}

#[test]
fn test_device_adds_gesture_beyond_global() {
    let config = load(
        r#"
[global.gestures.tap]
action = "xdotool click 1"
enabled = true

[device.d1]
device_usb_id = "1111:1111"
enabled = true

[device.d1.gestures.long_press]
action = "xdotool key ctrl+r"
enabled = true
"#,
        true,
    );
    let d1 = &config.devices["d1"];
    assert!(d1.gestures.contains_key("tap"));
    assert!(d1.gestures.contains_key("long_press"));
}

#[test]
fn test_override_does_not_mutate_other_devices() {
    let config = load(
        r#"
[global.gestures.tap]
action = "global tap"
enabled = true

[device.d1]
device_usb_id = "1111:1111"
enabled = true

[device.d1.gestures.tap]
action = "device1 tap"

[device.d2]
device_usb_id = "2222:2222"
enabled = true
"#,
        true,
    );
    assert_eq!(
        config.devices["d1"].gestures["tap"].action,
        Some("device1 tap".into())
    );
    assert_eq!(
        config.devices["d2"].gestures["tap"].action,
        Some("global tap".into())
    );
}

#[test]
fn test_no_global_gestures_fine() {
    let config = load(
        r#"
[device.d1]
device_usb_id = "1111:1111"
enabled = true

[device.d1.gestures.tap]
action = "echo tap"
enabled = true
"#,
        true,
    );
    assert_eq!(
        config.devices["d1"].gestures["tap"].action,
        Some("echo tap".into())
    );
}

// ── Global-only configs (no auto-device creation) ────────────

#[test]
fn test_global_only_gestures_no_device() {
    let config = load(
        r#"
[global.gestures.tap]
action = "xdotool click 1"
enabled = true
"#,
        true,
    );
    assert!(config.devices.is_empty());
}

#[test]
fn test_global_only_thresholds_no_device() {
    let config = load(
        r#"
[global.thresholds]
swipe_time_max = 1.5
"#,
        false,
    );
    assert!(config.devices.is_empty());
}

// ── Full roundtrip ───────────────────────────────────────────

#[test]
fn test_full_config_roundtrip() {
    let config = load(
        r#"
[global]
log_level = "DEBUG"

[global.thresholds]
swipe_time_max = 1.5
swipe_distance_min_pct = 0.15
angle_tolerance_deg = 30.0
tap_time_max = 0.2
tap_distance_max = 60.0
long_press_time_min = 0.8
double_tap_interval = 0.3
double_tap_distance_max = 50.0
pinch_threshold_pct = 0.1

[global.gestures.tap]
action = "xdotool click 1"
enabled = true

[global.gestures.swipe_left]
action = "xdotool key Left"
enabled = true

[device.d1]
device_usb_id = "1234:5678"
enabled = true

[device.d1.gestures.long_press]
action = "echo long"
enabled = true

[device.d2]
device_usb_id = "5678:9abc"
enabled = true

[device.d2.gestures.tap]
action = "xdotool click 3"

[device.d2.thresholds]
swipe_time_max = 2.0
"#,
        false,
    );

    assert_eq!(config.log_level, "DEBUG");

    let d1 = &config.devices["d1"];
    assert_eq!(d1.thresholds.swipe_time_max, 1.5);
    assert_eq!(d1.gestures["tap"].action, Some("xdotool click 1".into()));
    assert_eq!(
        d1.gestures["swipe_left"].action,
        Some("xdotool key Left".into())
    );
    assert_eq!(d1.gestures["long_press"].action, Some("echo long".into()));

    let d2 = &config.devices["d2"];
    assert_eq!(d2.gestures["tap"].action, Some("xdotool click 3".into()));
    assert!(d2.gestures["tap"].enabled);
    assert_eq!(d2.thresholds.swipe_time_max, 2.0);
    assert_eq!(d2.thresholds.tap_distance_max, 60.0);
}
