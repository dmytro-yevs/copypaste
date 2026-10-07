#!/usr/bin/env python3
"""Build an authenticated, target-specific CopyPaste module package."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile


def package(module_dir, library, output, platform, architecture):
    module_dir = Path(module_dir).resolve()
    library = Path(library).resolve(strict=True)
    output = Path(output).absolute()
    if output.suffix != ".cpmodule":
        raise ValueError("Module packages must use the .cpmodule extension.")
    manifest = json.loads((module_dir / "module.json").read_text())
    if platform not in manifest.get("supported_platforms", ["macos", "windows", "android"]):
        raise ValueError("This module does not support the requested platform.")
    suffix = {"macos": ".dylib", "windows": ".dll", "android": ".so"}[platform]
    manifest["target"] = {"platform": platform, "architecture": architecture}
    manifest["entrypoint"] = "bin/module" + suffix
    inventory = [(manifest["entrypoint"], library)]
    native = module_dir / "native" / platform / architecture
    if native.is_symlink():
        raise ValueError("Native dependencies cannot contain symbolic links.")
    if native.exists():
        for path in sorted(native.rglob("*")):
            if path.is_symlink():
                raise ValueError("Native dependencies cannot contain symbolic links.")
            if path.is_file():
                inventory.append(("bin/" + path.relative_to(native).as_posix(), path))
    assets = module_dir / "assets"
    if assets.is_symlink():
        raise ValueError("Module assets cannot contain symbolic links.")
    if assets.exists():
        for path in sorted(assets.rglob("*")):
            if path.is_symlink():
                raise ValueError("Module assets cannot contain symbolic links.")
            if path.is_file():
                inventory.append((path.relative_to(module_dir).as_posix(), path))
    if len({name.lower() for name, _ in inventory}) != len(inventory):
        raise ValueError("Module file paths collide.")
    manifest["files"] = []
    total_bytes = 0
    for name, path in inventory:
        total_bytes += path.stat().st_size
        if total_bytes > 2 * 1024 * 1024 * 1024:
            raise ValueError("Module package exceeds its size limit.")
        hasher = hashlib.sha256()
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                hasher.update(block)
        digest = hasher.hexdigest()
        manifest["files"].append({"path": name, "size_bytes": path.stat().st_size, "sha256": digest})
    root = Path(__file__).resolve().parents[2]
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="copypaste-module-") as directory:
        directory = Path(directory)
        manifest_path = directory / "manifest.json"
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
        bash = "bash"
        if os.name == "nt":
            git_bash = Path(os.environ.get("PROGRAMFILES", r"C:\Program Files")) / "Git/bin/bash.exe"
            if not git_bash.is_file():
                raise ValueError("Windows module signing requires Git Bash.")
            bash = str(git_bash)
        subprocess.run([bash, (root / "scripts/release/sign-update-artifact.sh").as_posix(), manifest_path.as_posix()], check=True)
        # The updater signer stores a base64-encoded Minisign envelope. Module
        # packages carry the standard plaintext envelope for the Rust verifier.
        signature = base64.b64decode((directory / "manifest.json.sig").read_text().strip(), validate=True)
        staged = directory / "module.cpmodule"
        with zipfile.ZipFile(staged, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            archive.write(manifest_path, "manifest.json")
            archive.writestr("manifest.json.sig", signature)
            for name, path in inventory:
                archive.write(path, name)
        # Stage beside the destination so final publication is one rename.
        with tempfile.NamedTemporaryFile(dir=output.parent, delete=False) as destination:
            temporary = Path(destination.name)
            try:
                with staged.open("rb") as source:
                    while chunk := source.read(1024 * 1024):
                        destination.write(chunk)
                destination.flush()
                os.fsync(destination.fileno())
            except BaseException:
                temporary.unlink(missing_ok=True)
                raise
        os.replace(temporary, output)
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module-dir", required=True, type=Path)
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--platform", required=True, choices=["macos", "windows", "android"])
    parser.add_argument("--architecture", required=True, choices=["x86", "x86_64", "arm", "aarch64"])
    args = parser.parse_args()
    manifest = package(args.module_dir, args.library, args.output, args.platform, args.architecture)
    print(f"Packaged {manifest['id']} {manifest['version']}: {args.output}")


if __name__ == "__main__":
    main()
