#!/usr/bin/env python3
"""Bundle the GStreamer camera runtime required by the Linux AppImage.

The Linux `camera_desktop` plugin discovers its elements at runtime.  Copying
the Flutter executable's ELF dependencies is therefore insufficient: this
script resolves the required element plugins from the installed package set,
copies their transitive non-glibc dependencies into the AppDir, and verifies
the staged plugin registry with all ambient plugin paths disabled.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from collections import deque
from pathlib import Path
from typing import Iterable


MAX_GLIBC = "2.39"
PLUGIN_DIR_NAME = "gstreamer-1.0"
RUNTIME_DIR_NAME = "gstreamer-runtime"
SCANNER_DIR_NAME = "gstreamer-1.0"
REQUIRED_ELEMENTS = {
    "videoconvert": "gstreamer1.0-plugins-base",
    "videoscale": "gstreamer1.0-plugins-base",
    "videorate": "gstreamer1.0-plugins-base",
    "appsink": "gstreamer1.0-plugins-base",
    "v4l2src": "gstreamer1.0-plugins-good",
    "jpegdec": "gstreamer1.0-plugins-good",
}
LICENSE_PACKAGES = (
    "libgstreamer1.0-0",
    "gstreamer1.0-plugins-base",
    "gstreamer1.0-plugins-good",
)
GLIBC_SONAMES = {
    "libc.so.6", "libdl.so.2", "libm.so.6", "libpthread.so.0",
    "librt.so.1", "libutil.so.1", "libresolv.so.2", "ld-linux-x86-64.so.2",
    "ld-linux-aarch64.so.1",
}
FILENAME = re.compile(r"^\s*Filename\s+(?P<path>/\S+)\s*$", re.MULTILINE)
NEEDED = re.compile(r"\(NEEDED\).*\[(?P<soname>[^]]+)\]")


class BundleError(RuntimeError):
    pass


def run(argv: list[str], *, environment: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(argv, check=True, text=True, capture_output=True, env=environment)
    except (OSError, subprocess.CalledProcessError) as error:
        raise BundleError(f"required command failed: {Path(argv[0]).name}") from error


def regular(path: Path) -> Path:
    try:
        details = path.lstat()
    except OSError as error:
        raise BundleError("required runtime file is unavailable") from error
    if stat.S_ISLNK(details.st_mode) or not stat.S_ISREG(details.st_mode) or details.st_size <= 0:
        raise BundleError("required runtime file must be a non-empty regular file")
    return path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def plugin_filename(output: str) -> Path:
    match = FILENAME.search(output)
    if match is None:
        raise BundleError("gst-inspect did not identify the element plugin file")
    return Path(match.group("path"))


def package_owner(path: Path) -> str:
    output = run(["dpkg-query", "--search", str(path)]).stdout
    line = next((value for value in output.splitlines() if ": " in value), "")
    if not line:
        raise BundleError("installed package ownership for a GStreamer plugin is unavailable")
    return line.split(": ", 1)[0].split(":", 1)[0]


def required_plugins(inspect: str) -> dict[str, Path]:
    resolved: dict[str, Path] = {}
    for element, expected_package in REQUIRED_ELEMENTS.items():
        source = regular(plugin_filename(run([inspect, element]).stdout))
        if package_owner(source) != expected_package:
            raise BundleError(f"{element} did not originate from {expected_package}")
        resolved[element] = source
    return resolved


def needed(path: Path) -> set[str]:
    return set(NEEDED.findall(run(["readelf", "-d", str(path)]).stdout))


def library_cache() -> dict[str, Path]:
    paths: dict[str, Path] = {}
    for line in run(["ldconfig", "-p"]).stdout.splitlines():
        if " => " not in line:
            continue
        name, candidate = line.strip().split(" => ", 1)
        soname = name.split(" ", 1)[0]
        path = Path(candidate)
        if soname and path.is_absolute() and path.exists() and soname not in paths:
            paths[soname] = path
    return paths


def allowed_runtime_file(path: Path) -> Path:
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise BundleError("dynamic runtime dependency is unavailable") from error
    if not resolved.is_file() or not any(
        str(resolved).startswith(prefix) for prefix in ("/lib/", "/usr/lib/", "/usr/libexec/")
    ):
        raise BundleError("dynamic runtime dependency escapes the trusted system library roots")
    return resolved


def copy_with_soname(source: Path, destination: Path, soname: str | None = None) -> Path:
    target = allowed_runtime_file(source)
    destination.mkdir(parents=True, exist_ok=True)
    copied = destination / target.name
    if copied.exists():
        if not copied.is_file() or copied.is_symlink() or sha256(copied) != sha256(target):
            raise BundleError("two runtime dependencies collide in the AppImage")
    else:
        shutil.copy2(target, copied, follow_symlinks=False)
        copied.chmod(0o755)
    names = {source.name, soname} - {"", target.name, None}
    for name in names:
        link = destination / name
        if link.exists() or link.is_symlink():
            if not link.is_symlink() or os.readlink(link) != target.name:
                raise BundleError("runtime dependency symlink collides in the AppImage")
        else:
            link.symlink_to(target.name)
    return copied


def copy_runtime_closure(initial: Iterable[Path], runtime: Path) -> list[Path]:
    cache = library_cache()
    queue: deque[Path] = deque()
    for source in initial:
        for soname in sorted(needed(source)):
            if soname in GLIBC_SONAMES:
                continue
            dependency = cache.get(soname)
            if dependency is None:
                raise BundleError(f"could not resolve dynamic dependency {soname}")
            queue.append(dependency)
    copied: dict[Path, Path] = {}
    while queue:
        candidate = queue.popleft()
        resolved = allowed_runtime_file(candidate)
        if resolved in copied:
            continue
        copied[resolved] = copy_with_soname(candidate, runtime)
        for soname in sorted(needed(resolved)):
            if soname in GLIBC_SONAMES:
                continue
            dependency = cache.get(soname)
            if dependency is None:
                raise BundleError(f"could not resolve dynamic dependency {soname}")
            queue.append(dependency)
    return sorted(copied.values())


def scanner_path() -> Path:
    directory = Path(run(["pkg-config", "--variable=pluginscannerdir", "gstreamer-1.0"]).stdout.strip())
    return regular(directory / "gst-plugin-scanner")


def camera_plugin_path(appdir: Path) -> Path:
    root = appdir / "usr/lib/copypaste"
    candidates = sorted(path for path in root.rglob("libcamera_desktop_plugin.so") if path.is_file())
    if len(candidates) != 1:
        raise BundleError("AppImage must contain exactly one Linux camera plugin")
    plugin = regular(candidates[0])
    if plugin.relative_to(appdir) != Path("usr/lib/copypaste/lib/libcamera_desktop_plugin.so"):
        raise BundleError("Linux camera plugin must use the Flutter bundle library path")
    return plugin


def patch_rpath(path: Path, rpath: str) -> None:
    run(["patchelf", "--set-rpath", rpath, str(path)])


def package_license(package: str) -> Path:
    candidates = [Path(line) for line in run(["dpkg-query", "--listfiles", package]).stdout.splitlines()]
    copyright = next((path for path in candidates if path.name == "copyright" and "/usr/share/doc/" in str(path)), None)
    if copyright is None:
        raise BundleError(f"license bytes for {package} are unavailable")
    return regular(copyright)


def copy_licenses(destination: Path) -> list[dict[str, str]]:
    destination.mkdir(parents=True, exist_ok=True)
    records = []
    for package in LICENSE_PACKAGES:
        source = package_license(package)
        target = destination / f"{package}.copyright"
        if target.exists():
            raise BundleError("GStreamer license destination collides")
        shutil.copyfile(source, target, follow_symlinks=False)
        records.append({"package": package, "path": str(target.name), "sha256": sha256(target)})
    return records


def isolated_environment(*, appdir: Path, registry: Path) -> dict[str, str]:
    plugins = appdir / "usr/lib" / PLUGIN_DIR_NAME
    scanner = appdir / "usr/libexec" / SCANNER_DIR_NAME / "gst-plugin-scanner"
    runtime = appdir / "usr/lib" / RUNTIME_DIR_NAME
    return {
        "HOME": str(registry.parent),
        "XDG_CACHE_HOME": str(registry.parent),
        "GST_REGISTRY_1_0": str(registry),
        "GST_PLUGIN_PATH": str(plugins),
        "GST_PLUGIN_PATH_1_0": str(plugins),
        "GST_PLUGIN_SYSTEM_PATH": str(plugins),
        "GST_PLUGIN_SYSTEM_PATH_1_0": str(plugins),
        "GST_PLUGIN_SCANNER": str(scanner),
        "LD_LIBRARY_PATH": str(runtime),
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
    }


def verify_elements(inspect: str, *, appdir: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="copypaste-gstreamer-registry-") as directory:
        registry = Path(directory) / "registry.bin"
        environment = isolated_environment(appdir=appdir, registry=registry)
        plugin_root = (appdir / "usr/lib" / PLUGIN_DIR_NAME).resolve()
        for element in REQUIRED_ELEMENTS:
            output = run([inspect, element], environment=environment).stdout
            source = plugin_filename(output).resolve()
            try:
                source.relative_to(plugin_root)
            except ValueError as error:
                raise BundleError(f"{element} resolved outside the staged AppImage plugin directory") from error


def validate_glibc(runtime: Path, *, architecture: str, maximum: str, output: Path) -> None:
    root = Path(__file__).resolve().parents[2]
    run([
        sys.executable, str(root / "scripts/release/verify-linux-runtime-baseline.py"),
        "--bundle", str(runtime), "--architecture", architecture,
        "--maximum", maximum, "--output", str(output),
    ])


def bundle(*, appdir: Path, architecture: str, maximum_glibc: str, manifest: Path, inspect: str) -> None:
    appdir = appdir.resolve(strict=True)
    manifest = manifest.resolve(strict=False)
    try:
        manifest.relative_to(appdir)
    except ValueError as error:
        raise BundleError("GStreamer manifest must stay inside the AppImage") from error
    if architecture not in {"x86_64", "aarch64"}:
        raise BundleError("unsupported Linux camera runtime architecture")
    if maximum_glibc != MAX_GLIBC:
        raise BundleError("camera runtime must use the Linux glibc compatibility ceiling")
    plugins = appdir / "usr/lib" / PLUGIN_DIR_NAME
    runtime = appdir / "usr/lib" / RUNTIME_DIR_NAME
    scanner_destination = appdir / "usr/libexec" / SCANNER_DIR_NAME
    licenses = appdir / "usr/share/licenses/copypaste-gstreamer"
    if any(path.exists() or path.is_symlink() for path in (plugins, runtime, scanner_destination, licenses, manifest)):
        raise BundleError("GStreamer AppImage destination must be empty")
    plugin_sources = required_plugins(inspect)
    plugin_records = []
    copied_plugins = []
    for element, source in plugin_sources.items():
        copied = copy_with_soname(source, plugins)
        copied_plugins.append(copied)
        plugin_records.append({
            "element": element,
            "source_package": REQUIRED_ELEMENTS[element],
            "path": str(copied.relative_to(appdir)),
            "sha256": sha256(copied),
        })
    scanner_source = scanner_path()
    scanner = copy_with_soname(scanner_source, scanner_destination)
    copied_runtime = copy_runtime_closure([*plugin_sources.values(), scanner_source], runtime)
    camera_plugin = camera_plugin_path(appdir)
    patch_rpath(camera_plugin, "$ORIGIN:$ORIGIN/../../gstreamer-runtime")
    for plugin in copied_plugins:
        patch_rpath(plugin, "$ORIGIN/../gstreamer-runtime")
    for library in copied_runtime:
        patch_rpath(library, "$ORIGIN")
    patch_rpath(scanner, "$ORIGIN/../../lib/gstreamer-runtime")
    license_records = copy_licenses(licenses)
    baseline = appdir / "usr/share/copypaste/gstreamer-camera-runtime-glibc.json"
    baseline.parent.mkdir(parents=True, exist_ok=True)
    # Cover every ELF byte that will be shipped under /usr, including plugin
    # modules and the plugin scanner rather than only the copied libraries.
    validate_glibc(appdir / "usr", architecture=architecture, maximum=maximum_glibc, output=baseline)
    verify_elements(inspect, appdir=appdir)
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(json.dumps({
        "schema": 1,
        "architecture": architecture,
        "elements": plugin_records,
        "scanner": {"path": str(scanner.relative_to(appdir)), "sha256": sha256(scanner)},
        "camera_plugin": {"path": str(camera_plugin.relative_to(appdir)), "sha256": sha256(camera_plugin)},
        "runtime_libraries": [
            {"path": str(path.relative_to(appdir)), "sha256": sha256(path)}
            for path in copied_runtime
        ],
        "licenses": license_records,
        "glibc_baseline": str(baseline.relative_to(appdir)),
    }, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--appdir", required=True, type=Path)
    parser.add_argument("--architecture", required=True, choices=("x86_64", "aarch64"))
    parser.add_argument("--maximum-glibc", required=True)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--gst-inspect", default="gst-inspect-1.0")
    args = parser.parse_args(argv)
    try:
        bundle(
            appdir=args.appdir,
            architecture=args.architecture,
            maximum_glibc=args.maximum_glibc,
            manifest=args.manifest,
            inspect=args.gst_inspect,
        )
    except BundleError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
