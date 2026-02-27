//! Configuration data structures and TOML parsing.
//!
//! The config file uses TOML format. Example:
//!
//! ```toml
//! [global]
//! log_level = "info"
//!
//! [global.thresholds]
//! swipe_time_max = 0.9
//! swipe_distance_min_pct = 0.15
//! angle_tolerance_deg = 30.0
//! tap_time_max = 0.2
//! long_press_time_min = 0.8
//! double_tap_interval = 0.3
//! tap_distance_max = 50.0
//! double_tap_distance_max = 50.0
//! pinch_threshold_pct = 0.1
//!
//! [global.gestures.tap]
//! action = "xdotool click 1"
//! enabled = true
//!
//! [device.kiosk]
//! device_usb_id = "1234:5678"
//! enabled = true
//!
//! [device.kiosk.gestures.swipe_left]
//! action = "xdotool key Left"
//! enabled = true
//!
//! [device.kiosk.thresholds]
//! swipe_time_max = 1.5
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use log::{debug, warn};
use serde::Deserialize;
use thiserror::Error;

/// Top-level error type used throughout the crate.
#[derive(Debug, Error)]
pub enum BodgestrError {
    #[error("Failed to read config file {path}: {source}")]
    ConfigReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse config file {path}: {message}")]
    ConfigParseError { path: PathBuf, message: String },

    #[error("Config validation error for device '{device}': missing threshold(s): {missing}")]
    MissingThresholds { device: String, missing: String },
}

/// Root of the TOML config file.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    global: RawGlobal,
    #[serde(default)]
    device: HashMap<String, RawDevice>,
}

/// The `[global]` section.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawGlobal {
    log_level: Option<String>,
    log_file: Option<String>,
    #[serde(default)]
    thresholds: RawThresholds,
    #[serde(default)]
    gestures: HashMap<String, RawGestureConfig>,
}

/// Threshold values - all optional so device sections can partially override.
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
struct RawThresholds {
    swipe_time_max: Option<f64>,
    swipe_distance_min_pct: Option<f64>,
    angle_tolerance_deg: Option<f64>,
    tap_time_max: Option<f64>,
    long_press_time_min: Option<f64>,
    double_tap_interval: Option<f64>,
    tap_distance_max: Option<f64>,
    double_tap_distance_max: Option<f64>,
    pinch_threshold_pct: Option<f64>,
}

/// A gesture entry (action + enabled).
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
struct RawGestureConfig {
    action: Option<String>,
    enabled: Option<bool>,
}

/// A `[device.<id>]` section.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawDevice {
    device_usb_id: Option<String>,
    enabled: Option<bool>,
    #[serde(default)]
    thresholds: RawThresholds,
    #[serde(default)]
    gestures: HashMap<String, RawGestureConfig>,
}

/// Fully validated thresholds - all values guaranteed to be present.
///
/// Created via threshold merging during config parsing.
#[derive(Debug, Clone, Default)]
pub struct ValidatedThresholds {
    pub swipe_time_max: f64,
    pub swipe_distance_min_pct: f64,
    pub angle_tolerance_deg: f64,
    pub tap_time_max: f64,
    pub long_press_time_min: f64,
    pub double_tap_interval: f64,
    pub tap_distance_max: f64,
    pub double_tap_distance_max: f64,
    pub pinch_threshold_pct: f64,
}

/// Gesture configuration (action + enabled).
#[derive(Debug, Clone)]
pub struct GestureConfig {
    pub action: Option<String>,
    pub enabled: bool,
}

/// Configuration for a single touch device.
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    pub device_usb_id: String,
    pub gestures: HashMap<String, GestureConfig>,
    pub thresholds: ValidatedThresholds,
}

/// Top-level parsed configuration.
#[derive(Debug)]
pub struct AppConfig {
    pub log_level: String,
    pub log_file: Option<String>,
    pub devices: HashMap<String, DeviceConfig>,
}

/// Generate merge, validate, and into_validated for threshold fields.
macro_rules! threshold_fields {
    ($($field:ident),+ $(,)?) => {
        impl RawThresholds {
            fn merge_with_fallback(&self, fallback: &RawThresholds) -> RawThresholds {
                RawThresholds {
                    $($field: self.$field.or(fallback.$field),)+
                }
            }

            fn into_validated(self) -> Result<ValidatedThresholds, Vec<&'static str>> {
                let missing: Vec<&str> = [$(
                    if self.$field.is_none() { Some(stringify!($field)) } else { None },
                )+].into_iter().flatten().collect();

                if !missing.is_empty() {
                    return Err(missing);
                }

                Ok(ValidatedThresholds {
                    $($field: self.$field.unwrap(),)+
                })
            }
        }
    };
}

threshold_fields!(
    swipe_time_max,
    swipe_distance_min_pct,
    angle_tolerance_deg,
    tap_time_max,
    long_press_time_min,
    double_tap_interval,
    tap_distance_max,
    double_tap_distance_max,
    pinch_threshold_pct,
);

/// Merge gesture maps: global first, then device-specific overrides.
fn merge_gestures(
    global: &HashMap<String, RawGestureConfig>,
    device: &HashMap<String, RawGestureConfig>,
) -> HashMap<String, GestureConfig> {
    let mut merged = HashMap::new();

    // Insert all global + device gesture names, device values override.
    for (name, gc) in global.iter().chain(device.iter()) {
        let entry = merged.entry(name.clone()).or_insert(GestureConfig {
            action: None,
            enabled: false,
        });
        if gc.action.is_some() {
            entry.action.clone_from(&gc.action);
        }
        if let Some(enabled) = gc.enabled {
            entry.enabled = enabled;
        }
    }

    merged
}

/// Parse a TOML config file and return the fully resolved `AppConfig`.
pub fn parse_config_file(path: &Path) -> Result<AppConfig, BodgestrError> {
    let raw: RawConfig =
        toml::from_str(
            &fs::read_to_string(path).map_err(|e| BodgestrError::ConfigReadError {
                path: path.to_path_buf(),
                source: e,
            })?,
        )
        .map_err(|e| BodgestrError::ConfigParseError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

    let mut devices = HashMap::new();

    for (device_id, raw_dev) in &raw.device {
        if !raw_dev.enabled.unwrap_or(false) {
            debug!("Device '{device_id}' is not enabled – skipping.");
            continue;
        }

        let Some(usb_id) = raw_dev.device_usb_id.as_deref().filter(|s| !s.is_empty()) else {
            warn!(
                "Device '{device_id}' is enabled but has no device_usb_id – skipping. \
                 Run 'bodgestr --list-devices' to find your USB ID.",
            );
            continue;
        };

        devices.insert(
            device_id.clone(),
            DeviceConfig {
                device_usb_id: usb_id.to_string(),
                gestures: merge_gestures(&raw.global.gestures, &raw_dev.gestures),
                thresholds: raw_dev
                    .thresholds
                    .merge_with_fallback(&raw.global.thresholds)
                    .into_validated()
                    .map_err(|missing| BodgestrError::MissingThresholds {
                        device: device_id.to_string(),
                        missing: missing.join(", "),
                    })?,
            },
        );
    }

    Ok(AppConfig {
        log_level: raw.global.log_level.unwrap_or_else(|| "info".to_string()),
        log_file: raw.global.log_file,
        devices,
    })
}
