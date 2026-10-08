#!/usr/bin/env python3
"""Build one shipped Android ABI with explicit NDK and 16 KiB ELF alignment."""
import argparse
import os
import platform
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
TARGETS = {
    "arm": ("armv7-linux-androideabi", "armv7a-linux-androideabi24-clang"),
    "aarch64": ("aarch64-linux-android", "aarch64-linux-android24-clang"),
    "x86_64": ("x86_64-linux-android", "x86_64-linux-android24-clang"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--architecture", choices=TARGETS, required=True)
    parser.add_argument("--ndk", type=Path, required=True)
    parser.add_argument("--module-dir", type=Path, default=ROOT / "modules/ocr")
    parser.add_argument("--library-name", default="libcopypaste_module_ocr.so")
    args = parser.parse_args()
    if "29.0.13846066" not in (args.ndk / "source.properties").read_text():
        raise ValueError("Android module builds require NDK 29.0.13846066")
    target, linker = TARGETS[args.architecture]
    host = {"Linux": "linux-x86_64", "Darwin": "darwin-x86_64"}.get(platform.system())
    if host is None:
        raise ValueError("Android module builds require a Linux or macOS NDK host")
    tools = args.ndk / "toolchains/llvm/prebuilt" / host / "bin"
    environment = {
        **os.environ,
        "CARGO_TARGET_" + target.upper().replace("-", "_") + "_LINKER": str(tools / linker),
        "CC_" + target.replace("-", "_"): str(tools / linker),
        "CXX_" + target.replace("-", "_"): str(tools / linker.replace("-clang", "-clang++")),
        "AR_" + target.replace("-", "_"): str(tools / "llvm-ar"),
        "RUSTFLAGS": "-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-rpath,$ORIGIN",
    }
    subprocess.run(["cargo", "+1.96", "build", "--manifest-path", str(args.module_dir / "Cargo.toml"),
                    "--release", "--locked", "--target", target, "--lib"], env=environment, check=True)
    if args.architecture == "x86_64" and args.module_dir.resolve().name in {"ocr", "supabase", "semantic-search"}:
        subprocess.run(["cargo", "+1.96", "build", "--release", "--locked", "--target", target,
                        "-p", "copypaste-module-qualification", "--lib"], env=environment, check=True)
    library = args.module_dir / "target" / target / "release" / args.library_name
    headers = subprocess.run([str(tools / "llvm-readelf"), "--program-headers", "--wide", str(library)],
                             check=True, capture_output=True, text=True).stdout
    loads = [line.split() for line in headers.splitlines() if line.strip().startswith("LOAD ")]
    if not loads or any(int(line[-1], 16) < 16384 for line in loads):
        raise ValueError("Android module ELF load segments must support 16 KiB pages")
    print(f"Verified {target} native module build and 16 KiB alignment")


if __name__ == "__main__":
    main()
