Name:           bodgestr
Version:        1.0.0
Release:        1%{?dist}
Summary:        Touch gesture daemon for Linux touchscreens
License:        MIT
URL:            https://github.com/mzellho/bodgestr
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  gcc
BuildRequires:  systemd-rpm-macros

Requires:       systemd
Recommends:     xdotool
Recommends:     logrotate

%description
bodgestr is a lightweight daemon that recognizes multi-touch gestures
(tap, double-tap, long-press, swipe, pinch) on Linux input devices
and executes configurable shell commands. Config-driven via TOML,
supports multiple devices, auto-reconnect, and runs as a systemd service.

%prep
%autosetup

%build
cargo build --release

%install
install -Dm755 target/release/bodgestr         %{buildroot}%{_bindir}/bodgestr
install -Dm644 dist/systemd/bodgestr.service   %{buildroot}%{_unitdir}/bodgestr.service
install -Dm644 config/gestures.example.toml      %{buildroot}%{_sysconfdir}/bodgestr/gestures.example.toml
install -Dm644 dist/logrotate/bodgestr         %{buildroot}%{_sysconfdir}/logrotate.d/bodgestr

# Install default config if not present (handled as %config(noreplace))
install -Dm644 config/gestures.example.toml      %{buildroot}%{_sysconfdir}/bodgestr/gestures.toml

%pre
getent group bodgestr  >/dev/null || groupadd -r bodgestr
getent passwd bodgestr >/dev/null || \
    useradd -r -g bodgestr -d /var/lib/bodgestr -s /sbin/nologin \
    -c "bodgestr service account" bodgestr
install -d -o bodgestr -g bodgestr /var/lib/bodgestr
install -d -o bodgestr -g bodgestr /var/log/bodgestr
exit 0

%post
%systemd_post bodgestr.service

%preun
%systemd_preun bodgestr.service

%postun
%systemd_postun_with_restart bodgestr.service

%files
%license LICENSE
%doc README.md config/gestures.example.toml
%{_bindir}/bodgestr
%{_unitdir}/bodgestr.service
%dir %{_sysconfdir}/bodgestr
%config(noreplace) %{_sysconfdir}/bodgestr/gestures.toml
%{_sysconfdir}/bodgestr/gestures.example.toml
%{_sysconfdir}/logrotate.d/bodgestr

%changelog
* Fri Feb 27 2026 Max Zellhofer <max.zellhofer@gmail.com> - 1.0.0-1
- initial bodgestr project

