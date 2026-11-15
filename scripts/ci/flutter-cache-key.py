#!/usr/bin/env python3
"""Fingerprint Rust bridge dependencies without creating a cache per release."""

import hashlib
from pathlib import Path
import re


def normalize_manifest(content):
    section = ""
    lines = []
    for line in content.splitlines(keepends=True):
        header = re.match(r"\s*\[([^\[\]]+)\]\s*(?:#.*)?$", line.strip())
        if header:
            section = header.group(1)
        if section == "workspace.metadata" or section.startswith("workspace.metadata."):
            continue
        if section in ("package", "workspace.package") and re.match(r"\s*version\s*=", line):
            continue
        lines.append(line)
    return "".join(lines)


def normalize_lock(content):
    blocks = re.split(r"(?m)^\[\[package\]\]\s*\n", content)
    local_versions = []
    for index in range(1, len(blocks)):
        block = blocks[index]
        if re.search(r"(?m)^\s*source\s*=", block):
            continue
        name = re.search(r'(?m)^name\s*=\s*"([^"]+)"', block)
        version = re.search(r'(?m)^version\s*=\s*"([^"]+)"', block)
        if name and version:
            local_versions.append((name.group(1), version.group(1)))
            blocks[index] = re.sub(r"(?m)^version\s*=.*\n", "", block)
    normalized = "[[package]]\n".join(blocks)
    for name, version in local_versions:
        normalized = normalized.replace(f'"{name} {version}"', f'"{name}"')
    return normalized


def fingerprint(root):
    files = {
        root / "Cargo.toml",
        root / "Cargo.lock",
        root / ".flutter-version",
        root / "apps/copypaste_flutter/pubspec.lock",
        root / "apps/copypaste_flutter/hook/build.dart",
        root / "crates/copypaste-flutter-bridge/rust-toolchain.toml",
        *root.glob("crates/**/Cargo.toml"),
        *root.glob(".cargo/config*"),
        *root.glob("crates/**/.cargo/config*"),
    }
    digest = hashlib.sha256()
    for path in sorted(files):
        content = path.read_text(encoding="utf-8").replace("\r\n", "\n")
        if path.name == "Cargo.toml":
            content = normalize_manifest(content)
        elif path.name == "Cargo.lock":
            content = normalize_lock(content)
        digest.update(path.relative_to(root).as_posix().encode("utf-8") + b"\0")
        digest.update(content.encode("utf-8") + b"\0")
    return digest.hexdigest()


if __name__ == "__main__":
    print(fingerprint(Path(__file__).resolve().parents[2]))
