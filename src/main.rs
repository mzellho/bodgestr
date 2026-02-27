//! bodgestr – Gesture recognition for Linux touchscreens.
//!
//! CLI entry point.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use clap::Parser;
use log::{Level, LevelFilter, Log, Metadata, Record};

use bodgestr::manager::{GestureManager, list_touch_devices};

#[derive(Parser)]
#[command(name = "bodgestr", about = "Gesture recognition for touchscreens")]
struct Cli {
    /// Path to configuration file
    #[arg(default_value = "/etc/bodgestr/gestures.toml")]
    config: PathBuf,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// List available touchscreen devices and exit
    #[arg(short, long)]
    list_devices: bool,
}

/// Simple logger that writes to stderr and optionally to a log file.
struct BodgestrLogger {
    level: LevelFilter,
    file: Option<Mutex<std::fs::File>>,
}

impl Log for BodgestrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level && metadata.target().starts_with("bodgestr")
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let level = match record.level() {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        };
        let line = format!("[{secs} {level} bodgestr] {}\n", record.args());

        // Write to stderr (→ journald when running as systemd service)
        eprint!("{line}");

        // Write to log file if configured
        if let Some(ref file_mutex) = self.file {
            if let Ok(mut f) = file_mutex.lock() {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }

    fn flush(&self) {
        if let Some(ref file_mutex) = self.file {
            if let Ok(mut f) = file_mutex.lock() {
                let _ = f.flush();
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.list_devices {
        return list_touch_devices();
    }

    // Parse config first (before logger init) so we can read the configured log level.
    let mut manager = match GestureManager::new(&cli.config) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "Error: {e}\n\n\
                 To find available touchscreen devices, run:\n\
                 \x20 bodgestr --list-devices"
            );
            return ExitCode::FAILURE;
        }
    };

    // Initialize logging: CLI --verbose overrides the config file setting.
    let log_level: LevelFilter = if cli.verbose {
        LevelFilter::Debug
    } else {
        manager
            .config_log_level()
            .parse()
            .unwrap_or(LevelFilter::Info)
    };

    let log_file = manager.config_log_file().and_then(|path| {
        match OpenOptions::new().create(true).append(true).open(path) {
            Ok(file) => Some(Mutex::new(file)),
            Err(e) => {
                eprintln!("Warning: cannot open log file '{path}': {e}");
                None
            }
        }
    });

    let logger = BodgestrLogger {
        level: log_level,
        file: log_file,
    };
    log::set_boxed_logger(Box::new(logger)).expect("Failed to set logger");
    log::set_max_level(log_level);

    // Set up signal handling for graceful shutdown
    let running = manager.running_flag();
    ctrlc::set_handler(move || {
        running.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    log::info!("Loading configuration from: {}", cli.config.display());
    manager.start();

    ExitCode::SUCCESS
}
