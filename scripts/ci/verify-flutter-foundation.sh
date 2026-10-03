#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP="$ROOT/apps/copypaste_flutter"
VERSION_FILE="$ROOT/.flutter-version"
[[ -f "$VERSION_FILE" ]] || { echo "missing .flutter-version" >&2; exit 1; }
[[ -f "$APP/pubspec.yaml" ]] || { echo "missing apps/copypaste_flutter/pubspec.yaml" >&2; exit 1; }
read -r expected < "$VERSION_FILE"
[[ "$expected" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo ".flutter-version must be a stable Flutter version" >&2; exit 1; }
workspace_version=$(awk '/^\[workspace\.package\]/{inside=1; next} inside && /^version[[:space:]]*=/{print $3; exit}' "$ROOT/Cargo.toml" | tr -d '"[:space:]')
pubspec_version=$(awk '/^version:[[:space:]]*/ { print $2; exit }' "$APP/pubspec.yaml")
[[ -n "$pubspec_version" && "$pubspec_version" == "$workspace_version" ]] || { echo "pubspec.yaml version must match Cargo.toml workspace.package.version" >&2; exit 1; }
grep -Eq '^  shadcn_ui: 0\.57\.1$' "$APP/pubspec.yaml" || { echo "pubspec.yaml must pin shadcn_ui: 0.57.1" >&2; exit 1; }
actual="$(flutter --version | sed -n '1s/Flutter //p' | awk '{print $1}')"
[[ "$actual" == "$expected" ]] || { echo "Flutter $expected is required; found ${actual:-unknown}" >&2; exit 1; }
cd "$APP"
flutter pub get --enforce-lockfile
dart format --output=none --set-exit-if-changed lib
flutter analyze
flutter build "${COPYPASTE_FLUTTER_BUILD_TARGET:?missing Flutter build target}" --debug
