#!/usr/bin/env python3
"""Record non-secret toolchain/configuration identity for production receipts."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess


def metadata(root, environment, rustc, flutter):
    files = ["Cargo.toml", "Cargo.lock", ".flutter-version", "apps/copypaste_flutter/pubspec.lock",
             "apps/copypaste_flutter/android/app/build.gradle.kts", "apps/copypaste_flutter/android/gradle.properties"]
    hashes = {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in files}
    flags = {name: environment.get(name, "") for name in
             ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET", "MACOSX_DEPLOYMENT_TARGET", "OPENSSL_STATIC", "OPENSSL_NO_VENDOR")}
    expected_flutter = (root / ".flutter-version").read_text().strip()
    if flutter["frameworkVersion"] != expected_flutter:
        raise ValueError("installed Flutter SDK differs from the pinned toolchain")
    toolchains = {"rustc": rustc.strip(), "flutter": flutter["frameworkVersion"],
                  "flutter_revision": flutter["frameworkRevision"], "dart": flutter["dartSdkVersion"]}
    targets = (["armv7", "arm64", "x86_64"] if environment["BUILD_PLATFORM"] == "android"
               else ["aarch64" if environment["RUNNER_ARCH"] == "ARM64" else "x86_64"])
    configuration = {"platform": environment["BUILD_PLATFORM"], "profile": environment["BUILD_PROFILE"],
                     "target_architectures": targets, "inputs": hashes, "flags": flags, "toolchains": toolchains}
    return {"schema": 1, **configuration,
            "fingerprint": hashlib.sha256(json.dumps(configuration, sort_keys=True).encode()).hexdigest(),
            "runner": {key: environment.get(key, "unknown") for key in ("RUNNER_OS", "RUNNER_ARCH", "ImageOS", "ImageVersion")}}


def flutter_version(executable):
    # Fresh SDKs bootstrap the Flutter tool and print pub progress on stdout.
    # Complete that initialization before requesting a machine-readable response.
    subprocess.run([executable, "--version"], check=True)
    return json.loads(subprocess.check_output([executable, "--version", "--machine"], text=True))


def main():
    root = Path(__file__).resolve().parents[2]
    rustc = subprocess.check_output([shutil.which("rustc") or "rustc", "--version", "--verbose"], text=True)
    flutter = flutter_version(shutil.which("flutter") or "flutter")
    result = metadata(root, os.environ, rustc, flutter)
    destination = Path(os.environ["RUNNER_TEMP"]) / "copypaste-build-metadata.json"
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    with open(os.environ["GITHUB_ENV"], "a", encoding="utf-8") as output:
        output.write(f"COPYPASTE_BUILD_METADATA={destination}\n")


if __name__ == "__main__":
    main()
