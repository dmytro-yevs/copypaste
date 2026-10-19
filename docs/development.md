# Development and validation

CopyPaste uses a shared Flutter interface and Rust runtime. Install the toolchains
declared by `rust-toolchain.toml`, the bridge toolchain file, and `.flutter-version`.
Keep the application version in `Cargo.toml` and the Flutter pubspec aligned.

From the repository root, run:

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -m unittest discover -s scripts/ci/tests -v
python3 -m unittest scripts/test_release_safety.py scripts/test_linux_qualification_bootstrap.py
python3 -m unittest discover -s scripts/release/tests -p 'test_*.py'
python3 scripts/check-docs.py
```

From `apps/copypaste_flutter`, run:

```sh
flutter pub get --enforce-lockfile
flutter test --no-pub
flutter analyze --no-pub
```

Optional native modules are separate Cargo workspaces. Their tests use each
module's manifest and lockfile. Native clipboard, model, installed-package, and
physical-device scenarios require the matching environment and prepared assets;
record missing evidence instead of treating an ignored scenario as qualified.

See [Flutter architecture](flutter-foundation.md) and
[release qualification](release-qualification.md) for the shared contracts.
