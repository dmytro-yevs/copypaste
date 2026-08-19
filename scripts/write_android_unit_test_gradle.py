#!/usr/bin/env python3
"""Point skipRust Kotlin tests at the tauri crate's Android library."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path


def tauri_version(lockfile: Path) -> str:
    text = lockfile.read_text()
    matches = re.findall(r'(?m)^name = "tauri"\nversion = "([^"]+)"', text)
    unique = sorted(set(matches))
    if len(unique) != 1:
        raise SystemExit(f"expected one tauri version in Cargo.lock, found {unique!r}")
    return unique[0]


def tauri_android_dir(version: str) -> Path:
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    found = sorted(
        cargo_home.glob(f"registry/src/*/tauri-{version}/mobile/android/build.gradle.kts")
    )
    if len(found) != 1:
        raise SystemExit(
            f"expected one tauri-{version} Android library under {cargo_home}, found {found!r}"
        )
    return found[0].parent


def gradle_files(android: Path) -> tuple[str, str]:
    quoted = json.dumps(str(android))
    settings = (
        "include ':tauri-android'\n"
        f"project(':tauri-android').projectDir = new File({quoted})\n"
    )
    app_build = """\
val implementation by configurations
dependencies {
  implementation("androidx.lifecycle:lifecycle-process:2.10.0")
  implementation(project(":tauri-android"))
}
"""
    return settings, app_build


def write_files(android_project: Path, android: Path) -> None:
    settings, app_build = gradle_files(android)
    (android_project / "tauri.settings.gradle").write_text(settings)
    (android_project / "app" / "tauri.build.gradle.kts").write_text(app_build)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--android-project", type=Path, required=True)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve()
    version = tauri_version(repo / "Cargo.lock")
    android = tauri_android_dir(version)
    settings, app_build = gradle_files(android)
    if f"tauri-{version}" not in settings or "mobile/android" not in settings:
        raise SystemExit("generated settings.gradle is missing the tauri Android project")
    if 'implementation(project(":tauri-android"))' not in app_build:
        raise SystemExit("generated app Gradle is missing :tauri-android")
    if args.self_test:
        return 0
    write_files(args.android_project.resolve(), android)
    return 0


if __name__ == "__main__":
    sys.exit(main())
