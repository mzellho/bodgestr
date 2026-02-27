# 👆 bodgestr

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-≥1.85-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/mzellho/bodgestr/actions/workflows/ci.yml/badge.svg)](https://github.com/mzellho/bodgestr/actions/workflows/ci.yml)

A lightweight, config-driven gesture daemon for Linux touchscreens. It translates raw multi-touch
input into arbitrary shell commands via [`evdev`](https://docs.rs/evdev/) - no desktop environment
required, perfect for headless kiosk setups on Raspberry Pi and similar devices.

> 💡 The name **bodgestr** is a portmanteau of **bodge** + **gesture** – a nod to the pragmatic,
> "good enough" engineering spirit behind the project.

> 🖥️ Looking for a full kiosk setup? See the
> [kub62-ansible kiosk role](https://github.com/mzellho/kub62-ansible/blob/main/roles/kiosk/tasks/kiosk.yaml) for an
> Ansible-based deployment that includes bodgestr.

## ✨ Features

- 👋 Swipe, tap, double-tap, long-press & pinch gesture recognition
- 🖥️ Multi-device support - configure multiple touchscreens in one file
- 🎚️ Two-tier config hierarchy: global → per-device (thresholds & gestures)
- 🔄 Automatic reconnection on USB disconnect
- 📦 `.deb` and `.rpm` packages - single install, ready to go
- 🪵 [systemd](https://systemd.io/) + journald logging with optional file logging
  and [logrotate](https://github.com/logrotate/logrotate)
- ⚡ Single binary, minimal footprint - no runtime dependencies beyond glibc

## 📋 Prerequisites

- 🐧 Linux with [evdev](https://www.kernel.org/doc/html/latest/input/input.html) support
- 👆 A multi-touch capable touchscreen
- 🔑 Read access to `/dev/input/event*` (typically requires `root` or `input` group)
- 🦀 [Rust](https://www.rust-lang.org/tools/install) ≥ 1.85 (only for building from source)

## ⚙️ Installation

### Debian / Ubuntu / Raspberry Pi OS

Download the latest `.deb` for your architecture from
[GitHub Releases](https://github.com/mzellho/bodgestr/releases):

```bash
sudo apt install ./bodgestr_*.deb
```

### Fedora / RHEL

Download the latest `.rpm` for your architecture from
[GitHub Releases](https://github.com/mzellho/bodgestr/releases):

```bash
sudo dnf install ./bodgestr-*.rpm
```

### From source

```bash
git clone https://github.com/mzellho/bodgestr.git && cd bodgestr
cargo test              # run tests
make build              # release build
sudo make install       # install to /usr/bin, /etc, systemd
```

## 🚀 Usage

### 1. Find your touchscreen

```bash
bodgestr --list-devices
```

### 2. Configure

Edit `/etc/bodgestr/gestures.toml` - register your device and enable the gestures you need:

```toml
[global]
log_level = "info"
log_file = "/var/log/bodgestr/bodgestr.log"

[global.thresholds]
swipe_time_max = 0.9
swipe_distance_min_pct = 0.15
angle_tolerance_deg = 30.0
tap_time_max = 0.2
long_press_time_min = 0.8
double_tap_interval = 0.3
tap_distance_max = 50.0
double_tap_distance_max = 50.0
pinch_threshold_pct = 0.1

[global.gestures.tap]
action = "xdotool click 1"
enabled = true

[global.gestures.long_press]
action = "notify-send 'bodgestr' 'Long press detected'"
enabled = true

[device.kiosk]
device_usb_id = "1234:5678"
enabled = true

[device.kiosk.gestures.swipe_left]
action = "xdotool key Left"
enabled = true

[device.kiosk.gestures.swipe_right]
action = "xdotool key Right"
enabled = true

[device.kiosk.gestures.swipe_up]
action = "brightnessctl set +10%"
enabled = true
```

All thresholds and gesture actions follow a two-tier priority: **per-device → global**. Devices
inherit everything from the global section - you only need to override what differs.

> 📄 See [`config/gestures.example.toml`](config/gestures.example.toml) for the full reference with
> all available options.

### 3. Run

```bash
bodgestr                                              # ▶️  default config (/etc/bodgestr/gestures.toml)
bodgestr /path/to/gestures.toml                       # ▶️  custom config path
bodgestr -v                                           # 🐛 verbose / DEBUG

sudo systemctl enable --now bodgestr                  # 🔁 as systemd service
sudo systemctl status bodgestr                        # ✅ check status
sudo journalctl -u bodgestr -f                        # 📋 follow logs
```

## 👋 Supported Gestures

| Gesture                                               | Description                  |
|-------------------------------------------------------|------------------------------|
| `swipe_left`, `swipe_right`, `swipe_up`, `swipe_down` | Directional swipe            |
| `tap`                                                 | Short single touch           |
| `double_tap`                                          | Two taps in quick succession |
| `long_press`                                          | Touch and hold               |
| `pinch_in`, `pinch_out`                               | Two-finger pinch to zoom     |

Each gesture can trigger any shell command - actions are executed via `sh -c`, so anything your
system can run works:

```bash
xdotool click 1                                          # simulate mouse click
xdotool key --clearmodifiers ctrl+Tab                    # keyboard shortcut
notify-send "Gesture" "Swipe detected!"                  # desktop notification
/usr/local/bin/my-script.sh                              # custom script
brightnessctl set +10%                                   # hardware control
playerctl next                                           # media control
brightnessctl set +10% && notify-send "Brightness" "Up"  # chained commands
```

## 🎚️ Configuration

The configuration file uses [TOML](https://toml.io/) format. Both **thresholds** and **gestures**
follow the same two-tier priority: **per-device → global**.

### Device Registration

Every device must be registered with its USB ID and explicitly enabled:

```toml
[device.kiosk]
device_usb_id = "1234:5678"
enabled = true
```

### Threshold Overrides

Devices inherit all global thresholds. Override per device:

```toml
[device.kiosk.thresholds]
swipe_time_max = 1.5          # allow slower swipes on this device
tap_distance_max = 80.0       # more forgiving tap radius
```

### Gesture Overrides

Devices inherit all global gestures. Override action or enabled state per device:

```toml
# Global: all devices get this tap action
[global.gestures.tap]
action = "xdotool click 1"
enabled = false

# Device: enable tap, add swipe
[device.kiosk.gestures.tap]
enabled = true

[device.kiosk.gestures.swipe_left]
action = "xdotool key Left"
enabled = true
```

## 📦 Packaging

### Debian / Ubuntu

The project includes full `debian/` packaging. Build a `.deb` with:

```bash
sudo apt install debhelper
dpkg-buildpackage -b -us -uc
```

### Fedora / RHEL

The project includes an RPM spec in `dist/rpm/`. Build an `.rpm` with:

```bash
sudo dnf install rpm-build rpmdevtools
rpmdev-setuptree
# create source tarball and build (see ci.yml for the full steps)
rpmbuild -bb dist/rpm/bodgestr.spec
```

### What the packages install

| Path                                       | Description                                                 |
|--------------------------------------------|-------------------------------------------------------------|
| `/usr/bin/bodgestr`                        | Daemon binary                                               |
| `/usr/lib/systemd/system/bodgestr.service` | Systemd unit                                                |
| `/etc/bodgestr/gestures.example.toml`      | Example configuration (reference)                           |
| `/etc/bodgestr/gestures.toml`              | Active config (created on first install, never overwritten) |
| `/etc/logrotate.d/bodgestr`                | Log rotation (daily, 7 days retention)                      |

### Logging

Logs always go to **stderr** (picked up by journald when running as a systemd service).
Additionally, set `log_file` in `[global]` to write to a file:

```toml
[global]
log_file = "/var/log/bodgestr/bodgestr.log"
```

Omit `log_file` to disable file logging. Both `.deb` and `.rpm` packages ship a logrotate config
for `/var/log/bodgestr/bodgestr.log` by default.

### Uninstall

```bash
# Debian / Ubuntu
sudo apt remove bodgestr

# Fedora / RHEL
sudo dnf remove bodgestr

# From source
sudo systemctl disable --now bodgestr
sudo make uninstall
```

## 🏗️ Project Structure

```
src/
  config.rs        TOML parsing, threshold merging, gesture inheritance
  event.rs         Touch event classification & processing (pure logic)
  recognizer.rs    Gesture recognition (swipe, tap, pinch, long-press)
  manager.rs       Device I/O, threading, reconnect (evdev layer)
  main.rs          CLI entry point, logger setup

tests/
  test_config.rs       Config parsing, merging, error handling
  test_event.rs        Event pipeline, classify_event, resolve_action
  test_recognizer.rs   Gesture detection, thresholds, edge cases

config/                Example configuration
debian/                Debian packaging
dist/
  rpm/                 RPM spec
  systemd/             Systemd unit file
  logrotate/           Logrotate configuration
```

## 🔄 CI / CD

The [GitHub Actions workflow](.github/workflows/ci.yml) runs on every push and PR:

| Stage       | Description                                               |
|-------------|-----------------------------------------------------------|
| **Lint**    | `cargo fmt --check` + `cargo clippy`                      |
| **Test**    | Tests via `cargo-nextest` (JUnit reporting)               |
| **Build**   | Cross-compiled release binaries for `amd64` and `arm64`   |
| **DEB**     | Debian packages for Bookworm & Trixie × `amd64` / `arm64` |
| **RPM**     | RPM packages for Fedora 42 & 43 × `x86_64` / `aarch64`    |
| **Release** | Automatic GitHub Release on version tags (`v*`)           |

## 🤖 Transparency

This project was created as part of a personal experiment and developed with significant assistance
from AI (GitHub Copilot / Claude). All code, tests, packaging, and documentation were reviewed and
validated by a human.
