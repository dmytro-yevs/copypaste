.PHONY: build release bundle dmg test smoke-test bench check fmt clippy install-daemon clean

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
