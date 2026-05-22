.PHONY: build test bench check fmt clippy install-daemon clean android-so

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
	cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build -p copypaste-android

# Run daemon with debug logging
dev-daemon:
	RUST_LOG=debug cargo run -p copypaste-daemon

# CLI shortcuts
ls:
	cargo run -p copypaste-cli -- list

search:
	cargo run -p copypaste-cli -- search "$(Q)"
