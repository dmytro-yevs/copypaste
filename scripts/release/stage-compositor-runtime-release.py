#!/usr/bin/env python3
"""Stage and bind signed public compositor runtime companion assets."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path


SHA256 = re.compile(r"[0-9a-f]{64}")
VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
COMMIT = re.compile(r"[0-9a-f]{40}")
GLIBC = re.compile(r"[0-9]+\.[0-9]+")

COORDINATES = (
    ("gnome", "GNOME", "46", "ubuntu24.04", "ubuntu", "24.04", "deb"),
    ("kde", "KDE", "6.0", "fedora40", "fedora", "40", "rpm"),
)
ARCHITECTURES = ("x86_64", "aarch64")


class ContractError(ValueError):
    pass


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def regular(path: Path, message: str) -> Path:
    if not path.is_file() or path.is_symlink():
        raise ContractError(message)
    return path


def read_json(path: Path, message: str) -> dict:
    try:
        value = json.loads(regular(path, message).read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ContractError(message) from error
    if not isinstance(value, dict):
        raise ContractError(message)
    return value


def metadata(path: Path) -> dict:
    regular(path, "public compositor runtime artifact is missing")
    return {"name": path.name, "sha256": digest(path), "size_bytes": path.stat().st_size}


def require_metadata(value: object, path: Path, message: str) -> None:
    if not isinstance(value, dict) or value != metadata(path):
        raise ContractError(message)


def coordinate_name(desktop: str, family: str, distribution: str, architecture: str) -> str:
    return f"copypaste-compositor-runtime-{desktop}-{family}-{distribution}-{architecture}"


def copy_checked(source: Path, destination: Path) -> Path:
    regular(source, "compositor runtime source artifact is missing")
    if destination.exists() or destination.is_symlink():
        raise ContractError("compositor runtime public asset name is ambiguous")
    shutil.copyfile(source, destination)
    if source.stat().st_size != destination.stat().st_size or digest(source) != digest(destination):
        raise ContractError("compositor runtime copy changed package bytes")
    return destination


def stage(source_root: Path, destination: Path, version: str, commit: str, producer_run_id: str) -> None:
    if not VERSION.fullmatch(version) or not COMMIT.fullmatch(commit):
        raise ContractError("invalid release identity")
    if not re.fullmatch(r"[1-9][0-9]*", producer_run_id):
        raise ContractError("invalid compositor runtime producer run identity")
    if destination.exists() or destination.is_symlink():
        raise ContractError("compositor runtime public output already exists")
    destination.mkdir(parents=True)
    entries = []
    try:
        for short_desktop, desktop, family, short_distribution, distribution_id, distribution_version, package_format in COORDINATES:
            for architecture in ARCHITECTURES:
                name = coordinate_name(short_desktop, family, short_distribution, architecture)
                artifact = source_root / name
                if not artifact.is_dir() or artifact.is_symlink():
                    raise ContractError(f"missing exact compositor runtime producer artifact: {name}")
                producer_path = artifact / "compositor-runtime-producer-receipt.json"
                runtime_path = artifact / "runtime-receipt.json"
                producer = read_json(producer_path, "compositor runtime producer receipt is invalid")
                runtime = read_json(runtime_path, "compositor runtime receipt is invalid")
                expected_identity = {
                    "schema": 1,
                    "commit": commit,
                    "producer_run_id": int(producer_run_id),
                    "version": version,
                    "architecture": architecture,
                    "desktop": desktop,
                    "family": family,
                    "distribution": {"id": distribution_id, "version": distribution_version},
                    "format": package_format,
                }
                for key, expected in expected_identity.items():
                    if producer.get(key) != expected:
                        raise ContractError(f"compositor runtime producer receipt has wrong {key}: {name}")
                runtime_identity = {
                    "schema": 1,
                    "architecture": architecture,
                    "desktop": desktop,
                    "distribution": {"id": distribution_id, "version": distribution_version},
                }
                for key, expected in runtime_identity.items():
                    if runtime.get(key) != expected:
                        raise ContractError(f"compositor runtime receipt has wrong {key}: {name}")
                if runtime.get("runtime_id") != producer.get("runtime_id") or runtime.get("source") != producer.get("source"):
                    raise ContractError(f"compositor runtime receipt identity differs from producer receipt: {name}")
                if (
                    not isinstance(producer.get("glibc_floor"), str)
                    or not GLIBC.fullmatch(producer["glibc_floor"])
                    or runtime.get("glibc_floor") != producer["glibc_floor"]
                ):
                    raise ContractError(f"compositor runtime glibc floor differs from producer receipt: {name}")
                if not isinstance(producer.get("runtime_id"), str) or not producer["runtime_id"]:
                    raise ContractError(f"compositor runtime producer receipt has no runtime ID: {name}")
                if not SHA256.fullmatch(str(runtime.get("source", {}).get("patch_sha256", ""))):
                    raise ContractError(f"compositor runtime source patch identity is invalid: {name}")
                require_metadata(producer.get("runtime_receipt"), runtime_path, "compositor runtime producer receipt does not bind runtime receipt")
                package_meta = producer.get("package")
                if not isinstance(package_meta, dict) or not isinstance(package_meta.get("name"), str):
                    raise ContractError(f"compositor runtime producer receipt has no package metadata: {name}")
                package = artifact / package_meta["name"]
                require_metadata(package_meta, package, "compositor runtime producer receipt does not bind package bytes")
                if package.suffix != f".{package_format}":
                    raise ContractError(f"compositor runtime package format differs from receipt: {name}")
                licenses = producer.get("upstream_licenses")
                if not isinstance(licenses, list) or not licenses:
                    raise ContractError(f"compositor runtime producer receipt has no upstream license records: {name}")
                copied_licenses = []
                for license_meta in licenses:
                    if not isinstance(license_meta, dict) or not isinstance(license_meta.get("name"), str):
                        raise ContractError(f"compositor runtime producer license metadata is invalid: {name}")
                    license_path = artifact / license_meta["name"]
                    require_metadata(license_meta, license_path, "compositor runtime producer receipt does not bind license bytes")
                    copied = copy_checked(license_path, destination / f"{package.name}.license-{license_path.name}")
                    copied_licenses.append(metadata(copied))
                copied_package = copy_checked(package, destination / package.name)
                copied_producer = copy_checked(producer_path, destination / f"{package.name}.producer-receipt.json")
                copied_runtime = copy_checked(runtime_path, destination / f"{package.name}.runtime-receipt.json")
                entries.append({
                    "producer_artifact": name,
                    "package": metadata(copied_package),
                    "producer_receipt": metadata(copied_producer),
                    "runtime_receipt": metadata(copied_runtime),
                    "licenses": copied_licenses,
                    "runtime_id": producer["runtime_id"],
                    "desktop": desktop,
                    "architecture": architecture,
                    "distribution": expected_identity["distribution"],
                    "format": package_format,
                })
        if {entry["producer_artifact"] for entry in entries} != {
            coordinate_name(short, family, distro, architecture)
            for short, _desktop, family, distro, _id, _version, _format in COORDINATES
            for architecture in ARCHITECTURES
        }:
            raise ContractError("compositor runtime producer matrix is incomplete")
        manifest = {
            "schema": 1,
            "version": version,
            "commit": commit,
            "producer_run_id": int(producer_run_id),
            "entries": sorted(entries, key=lambda entry: entry["package"]["name"]),
        }
        (destination / "compositor-runtime-release-sources.json").write_text(
            json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
        )
    except Exception:
        shutil.rmtree(destination)
        raise


def verify_public(root: Path, version: str, commit: str, release_run_id: str) -> dict:
    if not VERSION.fullmatch(version) or not COMMIT.fullmatch(commit):
        raise ContractError("invalid release identity")
    if not re.fullmatch(r"[1-9][0-9]*", release_run_id):
        raise ContractError("invalid release run identity")
    manifest_path = root / "compositor-runtime-release-sources.json"
    manifest = read_json(manifest_path, "compositor runtime source manifest is invalid")
    if (manifest.get("schema"), manifest.get("version"), manifest.get("commit")) != (1, version, commit):
        raise ContractError("compositor runtime source manifest identity differs from the release")
    producer_run_id = manifest.get("producer_run_id")
    if type(producer_run_id) is not int or producer_run_id <= 0:
        raise ContractError("compositor runtime source manifest has no producer run identity")
    entries = manifest.get("entries")
    if not isinstance(entries, list) or len(entries) != 4:
        raise ContractError("compositor runtime source manifest does not contain four companion packages")
    expected = {
        (desktop, architecture, distribution_id, distribution_version, package_format)
        for _short, desktop, _family, _distro, distribution_id, distribution_version, package_format in COORDINATES
        for architecture in ARCHITECTURES
    }
    actual = set()
    public = []
    for entry in entries:
        if not isinstance(entry, dict):
            raise ContractError("compositor runtime source manifest entry is invalid")
        distribution = entry.get("distribution")
        if not isinstance(distribution, dict):
            raise ContractError("compositor runtime source manifest has invalid distribution metadata")
        actual.add((
            entry.get("desktop"), entry.get("architecture"), distribution.get("id"),
            distribution.get("version"), entry.get("format"),
        ))
        package_meta = entry.get("package")
        if not isinstance(package_meta, dict) or not isinstance(package_meta.get("name"), str):
            raise ContractError("compositor runtime source manifest has invalid package metadata")
        package = root / package_meta["name"]
        require_metadata(package_meta, package, "compositor runtime package bytes changed after staging")
        if package.suffix != f".{entry.get('format')}":
            raise ContractError("compositor runtime public package format differs from source manifest")
        signature = regular(package.with_name(package.name + ".sig"), "compositor runtime signature is missing")
        digest_receipt = regular(package.with_name(package.name + ".sha256"), "compositor runtime SHA-256 receipt is missing")
        expected_digest = f"{digest(package)}  {package.name}\n"
        if digest_receipt.read_text(encoding="utf-8") != expected_digest:
            raise ContractError("compositor runtime SHA-256 receipt does not bind package bytes")
        for key in ("producer_receipt", "runtime_receipt"):
            item = entry.get(key)
            if not isinstance(item, dict) or not isinstance(item.get("name"), str):
                raise ContractError("compositor runtime source manifest has invalid receipt metadata")
            require_metadata(item, root / item["name"], "compositor runtime source receipt bytes changed after staging")
        for license_meta in entry.get("licenses", []):
            if not isinstance(license_meta, dict) or not isinstance(license_meta.get("name"), str):
                raise ContractError("compositor runtime source manifest has invalid license metadata")
            require_metadata(license_meta, root / license_meta["name"], "compositor runtime license bytes changed after staging")
        public.extend([
            metadata(package), metadata(signature), metadata(digest_receipt),
            entry["producer_receipt"], entry["runtime_receipt"], *entry.get("licenses", []),
        ])
    if actual != expected:
        raise ContractError("compositor runtime public package matrix is incomplete or ambiguous")
    public.append(metadata(manifest_path))
    receipt = {
        "schema": 1,
        "platform": "compositor-runtime",
        "version": version,
        "commit": commit,
        "run_id": int(release_run_id),
        "producer_run_id": producer_run_id,
        "artifacts": sorted(public, key=lambda item: item["name"]),
        "companions": sorted(entries, key=lambda entry: entry["package"]["name"]),
    }
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    stage_parser = commands.add_parser("stage")
    stage_parser.add_argument("--source-root", required=True, type=Path)
    stage_parser.add_argument("--output", required=True, type=Path)
    stage_parser.add_argument("--version", required=True)
    stage_parser.add_argument("--commit", required=True)
    stage_parser.add_argument("--producer-run-id", required=True)
    finalize = commands.add_parser("finalize")
    finalize.add_argument("--root", required=True, type=Path)
    finalize.add_argument("--version", required=True)
    finalize.add_argument("--commit", required=True)
    finalize.add_argument("--release-run-id", required=True)
    finalize.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        if args.command == "stage":
            stage(args.source_root, args.output, args.version, args.commit, args.producer_run_id)
            print("staged exact compositor runtime companion source assets")
        else:
            receipt = verify_public(args.root, args.version, args.commit, args.release_run_id)
            if args.output.exists() or args.output.is_symlink():
                raise ContractError("compositor runtime production receipt already exists")
            args.output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
            print("verified signed public compositor runtime companion assets")
    except (ContractError, OSError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
