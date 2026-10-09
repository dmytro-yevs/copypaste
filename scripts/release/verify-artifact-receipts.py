#!/usr/bin/env python3
import argparse
import hashlib
import json
from pathlib import Path
from typing import Optional


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify(root: Path, version: str, commit: str, run_id: str, *, require_linux: bool = False, linux_origin_run_id: Optional[str] = None) -> None:
    platforms = ["macos", "android", "windows"]
    if require_linux:
        platforms.append("linux")
    for platform in platforms:
        receipt_path = root / platform / "production-receipt.json"
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        schema = receipt.get("schema")
        if schema not in (1, 2) or receipt.get("platform") != platform:
            raise ValueError(f"invalid {platform} receipt identity")
        for key, expected in (
            ("version", version),
            ("commit", commit),
            ("run_id", linux_origin_run_id if platform == "linux" and linux_origin_run_id else run_id),
        ):
            if str(receipt.get(key)) != expected:
                raise ValueError(f"{platform} receipt has wrong {key}")
        # Schema 1 remains valid for recovery of previously qualified releases.
        artifacts = [receipt.get("artifact")] if schema == 1 else receipt.get("artifacts")
        if not isinstance(artifacts, list) or not artifacts:
            raise ValueError(f"{platform} receipt has no artifact metadata")
        names = []
        for metadata in artifacts:
            if not isinstance(metadata, dict):
                raise ValueError(f"{platform} receipt has no artifact metadata")
            name = metadata.get("name")
            if not isinstance(name, str) or not name or Path(name).name != name:
                raise ValueError(f"{platform} receipt has an invalid artifact name")
            names.append(name)
            artifact = root / platform / name
            if not artifact.is_file() or artifact.is_symlink():
                raise ValueError(f"{platform} qualified artifact is missing")
            if artifact.stat().st_size != metadata.get("bytes"):
                raise ValueError(f"{platform} artifact size changed")
            if sha256(artifact) != metadata.get("sha256"):
                raise ValueError(f"{platform} artifact digest changed")
        if len(set(names)) != len(names):
            raise ValueError(f"{platform} receipt has duplicate artifacts")
        if platform == "android":
            actual = {path.name for path in (root / platform).glob("*.apk")}
            if actual != set(names):
                raise ValueError("Android receipt does not cover every APK")
        if platform == "android" and schema == 2:
            expected = {
                f"CopyPaste-v{version}-android{suffix}.apk"
                for suffix in ("", "-arm64", "-armv7")
            }
            if set(names) != expected:
                raise ValueError("Android receipt must cover universal, arm64, and armv7 APKs")
        if platform == "linux":
            expected = {
                f"CopyPaste-v{version}-linux-{architecture}.{extension}"
                for architecture in ("x86_64", "aarch64")
                for extension in ("AppImage", "deb", "rpm")
            }
            packages = {name for name in names if name.endswith((".AppImage", ".deb", ".rpm"))}
            if packages != expected:
                raise ValueError("Linux receipt must cover AppImage, deb, and rpm for x86_64 and aarch64")
            for package in packages:
                if f"{package}.sig" not in names or f"{package}.sha256" not in names:
                    raise ValueError("Linux receipt must bind each package signature and SHA-256 receipt")
            baselines = {
                f"CopyPaste-v{version}-linux-{architecture}.runtime-baseline.json"
                for architecture in ("x86_64", "aarch64")
            }
            if not baselines <= set(names):
                raise ValueError("Linux receipt must bind each architecture runtime baseline")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--require-linux", action="store_true")
    parser.add_argument("--linux-origin-run-id")
    args = parser.parse_args()
    verify(args.root, args.version, args.commit, args.run_id, require_linux=args.require_linux, linux_origin_run_id=args.linux_origin_run_id)
    print("verified production artifacts for macOS, Android, Windows, and Linux" if args.require_linux else "verified production artifacts for macOS, Android, and Windows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
