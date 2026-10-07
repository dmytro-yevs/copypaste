#!/usr/bin/env python3
"""Prepare the checksum-pinned CPU runtime and its target dependencies."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
ANDROID_ABIS = {"arm": "armeabi-v7a", "aarch64": "arm64-v8a", "x86_64": "x86_64"}
ANDROID_TRIPLES = {"arm": "arm-linux-androideabi", "aarch64": "aarch64-linux-android", "x86_64": "x86_64-linux-android"}


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def prepare(platform, architecture, destination, cache, ndk=None):
    metadata = json.loads((ROOT / "assets/runtime-sources.json").read_text())
    source = metadata["sources"][platform]
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / source["url"].rsplit("/", 1)[-1]
    if not archive.exists() or digest(archive) != source["sha256"]:
        with tempfile.NamedTemporaryFile(dir=cache, delete=False) as output:
            staged = Path(output.name)
            try:
                with urllib.request.urlopen(source["url"], timeout=60) as response:
                    shutil.copyfileobj(response, output, 1024 * 1024)
                output.flush()
                if digest(staged) != source["sha256"]:
                    raise ValueError("ONNX Runtime archive checksum mismatch")
                staged.replace(archive)
            finally:
                staged.unlink(missing_ok=True)
    target = destination / platform / architecture
    target.mkdir(parents=True, exist_ok=True)
    name = {"macos": "libonnxruntime.dylib", "windows": "onnxruntime.dll", "android": "libonnxruntime.so"}[platform]
    output = target / name
    if platform == "macos":
        if architecture != "aarch64":
            raise ValueError("The shipped macOS application uses aarch64")
        with tarfile.open(archive) as package:
            member = package.getmember(source["member"])
            if not member.isfile():
                raise ValueError("Runtime archive member must be a regular file")
            with package.extractfile(member) as library, output.open("wb") as destination_file:
                shutil.copyfileobj(library, destination_file)
    else:
        member = source.get("member", f"jni/{ANDROID_ABIS.get(architecture)}/libonnxruntime.so")
        with zipfile.ZipFile(archive) as package, package.open(member) as library, output.open("wb") as destination_file:
            shutil.copyfileobj(library, destination_file)
    if platform == "android":
        if ndk is None:
            raise ValueError("Android packaging requires the pinned NDK")
        properties = (ndk / "source.properties").read_text()
        if "29.0.13846066" not in properties:
            raise ValueError("Android packaging requires NDK 29.0.13846066")
        dependencies = ndk / "toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib" / ANDROID_TRIPLES[architecture]
        shutil.copyfile(dependencies / "libc++_shared.so", target / "libc++_shared.so")
        subprocess.run(["patchelf", "--page-size", "16384", "--set-rpath", "$ORIGIN", str(output)], check=True)
    print(f"Prepared ONNX Runtime {metadata['version']}: {platform}/{architecture}")
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, choices=["macos", "windows", "android"])
    parser.add_argument("--architecture", required=True, choices=["arm", "aarch64", "x86_64"])
    parser.add_argument("--destination", type=Path, default=ROOT / "native")
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--ndk", type=Path)
    args = parser.parse_args()
    prepare(args.platform, args.architecture, args.destination, args.cache, args.ndk)


if __name__ == "__main__":
    main()
