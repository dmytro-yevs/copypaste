.PHONY: build release bundle dmg test smoke-test bench check fmt clippy install-daemon install uninstall clean android-so android-docker android-docker-clean-cache

build:
	cargo build --workspace

release:
	cargo build --release --workspace

bundle: release
	bash scripts/make_app_bundle.sh

dmg: bundle
	bash scripts/make_dmg.sh

smoke-test: release
	bash scripts/smoke_test.sh

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

# Build Android .so libraries via cargo-ndk
# Requires: cargo install cargo-ndk
#           rustup target add aarch64-linux-android x86_64-linux-android
#           Android NDK installed (set ANDROID_NDK_HOME or let cargo-ndk auto-detect)
# OOM GUARD (CopyPaste-5a9y): run at most ONE android-so / android-docker
# cross-compile at a time. Two concurrent cargo-ndk builds (e.g. two agents)
# exhaust RAM and wedge the machine. Do not parallelize this target.
android-so:
	@command -v cargo-ndk >/dev/null 2>&1 || { \
		echo ""; \
		echo "ERROR: cargo-ndk is not installed."; \
		echo ""; \
		echo "To install cargo-ndk:"; \
		echo "  cargo install cargo-ndk"; \
		echo ""; \
		echo "To add the required Rust targets:"; \
		echo "  rustup target add aarch64-linux-android"; \
		echo "  rustup target add x86_64-linux-android"; \
		echo ""; \
		echo "Ensure the Android NDK is installed and ANDROID_NDK_HOME is set,"; \
		echo "or install via Android Studio: SDK Manager -> SDK Tools -> NDK."; \
		echo ""; \
		exit 1; \
	}
	cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build --profile release-size -p copypaste-android

# Build Android .so via the cached Docker builder image.
# Uses the named volumes wired in docker-compose.yml so the cargo target,
# crates.io registry, sccache and ccache all persist across runs.
# Cold build (first time):  ~5-10 min on amd64-xlarge (image pre-bakes openssl+sqlcipher).
# Warm build (code change):  ~1-2 min (target dir + sccache hits).
# Re-run after image rebuild: cache hits where compiler hash matches.
android-docker:
	docker compose --profile build build android
	docker compose --profile build run --rm android

# Wipe just the cargo target + sccache + ccache volumes (registry kept).
# Use when bisecting cache poisoning or rust toolchain version skew.
android-docker-clean-cache:
	-docker volume rm copypaste_cargo-android-target copypaste_sccache-android copypaste_ccache-android

# Run daemon with debug logging
dev-daemon:
	RUST_LOG=debug cargo run -p copypaste-daemon

# CLI shortcuts
ls:
	cargo run -p copypaste-cli -- list

search:
	cargo run -p copypaste-cli -- search "$(Q)"
