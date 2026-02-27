//! Multi-device gesture manager and device discovery (I/O layer).
//!
//! Pure event-processing logic lives in [`crate::event`].
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use evdev::{AbsoluteAxisType, Device};
use log::{debug, error, info, warn};

use crate::config::{AppConfig, BodgestrError, DeviceConfig, parse_config_file};
use crate::recognizer::{GestureRecognizer, GestureType};

// Re-export event symbols so existing `use bodgestr::manager::*` keeps working.
pub use crate::event::{
    TouchEvent, classify_event, parse_usb_id, process_touch_events, resolve_action,
};

// -- GestureManager (top-level orchestrator) ------------------

/// Manages gesture recognition across multiple touch devices.
pub struct GestureManager {
    config: AppConfig,
    running: Arc<AtomicBool>,
}

impl GestureManager {
    pub fn new(config_path: impl AsRef<std::path::Path>) -> Result<Self, BodgestrError> {
        Ok(Self {
            config: parse_config_file(config_path.as_ref())?,
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Start listening to all configured devices.
    pub fn start(&mut self) {
        if self.config.devices.is_empty() {
            error!("No devices configured");
            return;
        }

        self.running.store(true, Ordering::Relaxed);
        info!("Starting gesture manager");

        let mut handles = Vec::new();

        for (device_id, device_config) in &self.config.devices {
            if let Some(device) = find_device(device_id, device_config) {
                let device_id = device_id.clone();
                let config = device_config.clone();
                let running = Arc::clone(&self.running);

                handles.push(
                    thread::Builder::new()
                        .name(format!("gesture-{device_id}"))
                        .spawn(move || {
                            run_device_loop(&device_id, device, &config, &running);
                        })
                        .expect("Failed to spawn device thread"),
                );
            } else {
                warn!("Device not found: {device_id}");
            }
        }

        if handles.is_empty() {
            error!("No devices found, exiting");
            return;
        }

        for handle in handles {
            let _ = handle.join();
        }
    }

    /// Stop listening to devices.
    #[allow(dead_code)]
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        info!("Gesture manager stopped");
    }

    /// Get a reference to the running flag for signal handling.
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Get the log level from the parsed configuration.
    pub fn config_log_level(&self) -> &str {
        &self.config.log_level
    }

    /// Get the optional log file path from the parsed configuration.
    pub fn config_log_file(&self) -> Option<&str> {
        self.config.log_file.as_deref()
    }
}

// -- Device I/O -----------------------------------------------

/// Check if a device has multi-touch capabilities.
fn is_touch_device(device: &Device) -> bool {
    let Some(abs_axes) = device.supported_absolute_axes() else {
        return false;
    };
    abs_axes.contains(AbsoluteAxisType::ABS_MT_POSITION_X)
        && abs_axes.contains(AbsoluteAxisType::ABS_MT_POSITION_Y)
}

/// Find a touchscreen device by USB vendor:product ID.
fn find_device(device_id: &str, config: &DeviceConfig) -> Option<Device> {
    let Some((vendor, product)) = parse_usb_id(&config.device_usb_id) else {
        warn!(
            "Device {device_id}: invalid USB ID format '{}' (expected vendor:product)",
            config.device_usb_id
        );
        return None;
    };

    for (path, device) in evdev::enumerate() {
        if !is_touch_device(&device) {
            continue;
        }
        let id = device.input_id();
        if id.vendor() == vendor && id.product() == product {
            info!(
                "Found device for {} by USB ID {}: {} ({})",
                device_id,
                config.device_usb_id,
                device.name().unwrap_or("unknown"),
                path.display()
            );
            return Some(device);
        }
    }

    warn!(
        "Device {}: no touch device with USB ID {} found",
        device_id, config.device_usb_id
    );
    None
}

/// Initialize recognizer from device axis info and start the event loop.
fn run_device_loop(
    device_id: &str,
    mut device: Device,
    config: &DeviceConfig,
    running: &Arc<AtomicBool>,
) {
    let abs = match device.get_abs_state() {
        Ok(state) => state,
        Err(e) => {
            error!("Device {device_id} failed to get abs state: {e}");
            return;
        }
    };

    let x = &abs[AbsoluteAxisType::ABS_MT_POSITION_X.0 as usize];
    let y = &abs[AbsoluteAxisType::ABS_MT_POSITION_Y.0 as usize];

    info!(
        "Started processing device: {device_id} (USB {})",
        config.device_usb_id
    );
    debug!(
        "  X range: {}..{}, Y range: {}..{}",
        x.minimum, x.maximum, y.minimum, y.maximum
    );

    let mut recognizer = GestureRecognizer::new(
        config.thresholds.clone(),
        (x.minimum as f64, x.maximum as f64),
        (y.minimum as f64, y.maximum as f64),
    );

    event_loop(device_id, &mut device, &mut recognizer, config, running);
}

/// Blocking event loop - reads from the device and dispatches gestures.
fn event_loop(
    device_id: &str,
    device: &mut Device,
    recognizer: &mut GestureRecognizer,
    config: &DeviceConfig,
    running: &Arc<AtomicBool>,
) {
    while running.load(Ordering::Relaxed) {
        match device.fetch_events().map(|iter| iter.collect::<Vec<_>>()) {
            Ok(events) => {
                for event in &events {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(te) = classify_event(event) {
                        let fired = process_touch_events(recognizer, &[te]);
                        for gesture in fired {
                            execute_gesture(device_id, gesture, config);
                        }
                    }
                }
            }
            Err(e) => {
                if running.load(Ordering::Relaxed) {
                    warn!("Device {device_id} disconnected: {e}");
                    attempt_reconnect(device_id, device, recognizer, config, running);
                }
                break;
            }
        }
    }
}

/// Spawn the shell command for a recognized gesture.
fn execute_gesture(device_id: &str, gesture: GestureType, config: &DeviceConfig) {
    let gesture_name: &str = gesture.into();
    if let Some(action) = resolve_action(gesture, &config.gestures) {
        match Command::new("sh").arg("-c").arg(action).spawn() {
            Ok(_) => debug!("Spawned action: {action}"),
            Err(e) => error!("Failed to execute action '{action}': {e}"),
        }
        info!("{device_id}: {gesture_name}");
    }
}

/// Attempt to reconnect to a device after it disconnects.
fn attempt_reconnect(
    device_id: &str,
    device: &mut Device,
    recognizer: &mut GestureRecognizer,
    config: &DeviceConfig,
    running: &Arc<AtomicBool>,
) {
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL: Duration = Duration::from_secs(5);

    for attempt in 1..=MAX_RETRIES {
        if !running.load(Ordering::Relaxed) {
            return;
        }
        info!("Reconnect attempt {attempt}/{MAX_RETRIES} for {device_id}...");
        thread::sleep(RETRY_INTERVAL);

        if let Some(new_device) = find_device(device_id, config) {
            info!("Reconnected to {device_id}");
            *device = new_device;
            event_loop(device_id, device, recognizer, config, running);
            return;
        }
    }
    error!("Failed to reconnect to {device_id} after {MAX_RETRIES} attempts");
}

/// List all multi-touch capable devices.
pub fn list_touch_devices() -> ExitCode {
    println!("\n=== bodgestr: Available Touchscreen Devices ===\n");
    let mut touch_count = 0;

    for (path, device) in evdev::enumerate() {
        if !is_touch_device(&device) {
            continue;
        }

        touch_count += 1;
        println!(
            "Device {touch_count}:\n\
             \x20 Path:      {}\n\
             \x20 Name:      {}\n\
             \x20 USB ID:    {:04x}:{:04x}\n\
             \x20 Phys:      {}\n",
            path.display(),
            device.name().unwrap_or("unknown"),
            device.input_id().vendor(),
            device.input_id().product(),
            device.physical_path().unwrap_or("N/A"),
        );
    }

    if touch_count == 0 {
        println!(
            "No multi-touch devices found.\n\n\
             Troubleshooting:\n\
             \x20 - Check if touchscreen is connected\n\
             \x20 - Run 'libinput list-devices' to see all devices\n\
             \x20 - Run as root if devices are not visible"
        );
        return ExitCode::FAILURE;
    }

    println!(
        "Found {touch_count} touch device(s).\n\n\
         Add the USB ID to your gestures.toml:\n\
         \x20 [device.<name>]\n\
         \x20 device_usb_id = \"<USB ID>\"\n\
         \x20 enabled = true"
    );
    ExitCode::SUCCESS
}
