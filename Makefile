PREFIX      ?= /usr
BINDIR      ?= $(PREFIX)/bin
SYSCONFDIR  ?= /etc
UNITDIR     ?= /usr/lib/systemd/system
BINARY      := target/release/bodgestr

.PHONY: build install uninstall clean

build:
	cargo build --release

install: build
	install -Dm755 $(BINARY)                       $(DESTDIR)$(BINDIR)/bodgestr
	install -Dm644 dist/systemd/bodgestr.service $(DESTDIR)$(UNITDIR)/bodgestr.service
	install -Dm644 config/gestures.example.toml    $(DESTDIR)$(SYSCONFDIR)/bodgestr/gestures.example.toml
	install -Dm644 dist/logrotate/bodgestr       $(DESTDIR)$(SYSCONFDIR)/logrotate.d/bodgestr
	@if [ ! -f $(DESTDIR)$(SYSCONFDIR)/bodgestr/gestures.toml ]; then \
		install -Dm644 config/gestures.example.toml $(DESTDIR)$(SYSCONFDIR)/bodgestr/gestures.toml; \
		echo "Installed default config to $(DESTDIR)$(SYSCONFDIR)/bodgestr/gestures.toml"; \
	else \
		echo "Config already exists at $(DESTDIR)$(SYSCONFDIR)/bodgestr/gestures.toml – not overwriting."; \
	fi

uninstall:
	rm -f  $(DESTDIR)$(BINDIR)/bodgestr
	rm -f  $(DESTDIR)$(UNITDIR)/bodgestr.service
	rm -rf $(DESTDIR)$(SYSCONFDIR)/bodgestr

clean:
	cargo clean

