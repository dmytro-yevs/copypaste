.PHONY: build test bench check fmt clippy install-daemon install uninstall clean

build:
	cargo build --workspace

test:
	cargo test --workspace

bench:
	cargo bench -p copypaste-core

check:
	cargo check --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace -- -D warnings

install-daemon:
	./launch/install.sh

## Install pre-built binary + LaunchAgent (macOS)
install:
	./scripts/install-daemon.sh

## Unload LaunchAgent and remove plist (binary kept unless REMOVE_BINARY=1)
uninstall:
	./scripts/uninstall-daemon.sh

install-daemon-linux:
	./contrib/systemd/install.sh

deny:
	cargo deny check

audit:
	cargo audit

clean:
	cargo clean

# Run daemon with debug logging
dev-daemon:
	RUST_LOG=debug cargo run -p copypaste-daemon

# CLI shortcuts
ls:
	cargo run -p copypaste-cli -- list

search:
	cargo run -p copypaste-cli -- search "$(Q)"
