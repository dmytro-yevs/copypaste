#!/usr/bin/env python3
"""Build the signed marketplace catalog from qualified first-party packages."""
import argparse
import base64
from functools import lru_cache
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tempfile
from urllib.parse import quote
import zipfile
from typing import NamedTuple, Optional

ROOT = Path(__file__).resolve().parents[2]
PUBLIC_KEY = re.search(
    r'MODULE_RELEASE_PUBLIC_KEY: &str =\s*"([^"]+)"',
    (ROOT / "crates/copypaste-modules/src/lib.rs").read_text(),
).group(1)
REPOSITORY = "https://github.com/dmytro-yevs/copypaste/releases/download"
REQUIRED_TARGETS = {
    ("macos", "aarch64"), ("windows", "x86_64"),
    ("android", "aarch64"), ("android", "arm"), ("android", "x86_64"),
}
MAXIMUM_BYTES = 2 * 1024 * 1024 * 1024


class VerifiedPackage(NamedTuple):
    manifest: dict
    minimum_system_version: Optional[str]


def sha256_stream(source):
    digest = hashlib.sha256()
    for block in iter(lambda: source.read(1024 * 1024), b""):
        digest.update(block)
    return digest.hexdigest()


@lru_cache(maxsize=1)
def require_openssl():
    result = subprocess.run(["openssl", "version"], check=True, capture_output=True, text=True)
    version = re.match(r"OpenSSL ([0-9]+)\.", result.stdout)
    if version is None or int(version.group(1)) < 3:
        raise ValueError("Marketplace publication requires OpenSSL 3 or newer")


def verify_signature(data, signature, filename, public_key=PUBLIC_KEY):
    """Verify both Minisign signatures with the pinned release identity."""
    require_openssl()
    key = base64.b64decode(public_key, validate=True)
    lines = signature.decode("utf-8").strip().splitlines()
    if len(key) != 42 or key[:2] not in (b"Ed", b"ED") or len(lines) != 4:
        raise ValueError("Invalid Minisign envelope")
    if not lines[0].startswith("untrusted comment: ") or not lines[2].startswith("trusted comment: "):
        raise ValueError("Invalid Minisign comments")
    packet = base64.b64decode(lines[1], validate=True)
    global_signature = base64.b64decode(lines[3], validate=True)
    comment = lines[2][len("trusted comment: "):]
    if (len(packet) != 74 or packet[:2] != b"ED" or packet[2:10] != key[2:10]
            or len(global_signature) != 64 or f"file:{filename}" not in comment.split("\t")):
        raise ValueError("Signature does not match the release identity or filename")
    messages = [
        (hashlib.blake2b(data).digest(), packet[10:]),
        (packet[10:] + comment.encode(), global_signature),
    ]
    with tempfile.TemporaryDirectory(prefix="module-signature-") as directory:
        directory = Path(directory)
        # RFC 8410 SubjectPublicKeyInfo for Ed25519.
        key_path = directory / "public.der"
        key_path.write_bytes(bytes.fromhex("302a300506032b6570032100") + key[10:])
        for message, signed in messages:
            (directory / "message").write_bytes(message)
            (directory / "signature").write_bytes(signed)
            result = subprocess.run([
                "openssl", "pkeyutl", "-verify", "-pubin", "-keyform", "DER",
                "-inkey", str(key_path), "-rawin", "-in", str(directory / "message"),
                "-sigfile", str(directory / "signature"),
            ], capture_output=True)
            if result.returncode:
                raise ValueError("Invalid release signature")


def read_package(path, public_key=PUBLIC_KEY):
    path = Path(path)
    if path.suffix != ".cpmodule" or not 0 < path.stat().st_size <= MAXIMUM_BYTES:
        raise ValueError("Invalid module package size or extension")
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        names = [entry.filename for entry in entries]
        if len(names) != len(set(names)) or len(names) > 4098:
            raise ValueError("Duplicate or excessive package entries")
        if (archive.getinfo("manifest.json").file_size > 1024 * 1024
                or archive.getinfo("manifest.json.sig").file_size > 64 * 1024):
            raise ValueError("Module metadata is too large")
        manifest_bytes = archive.read("manifest.json")
        verify_signature(manifest_bytes, archive.read("manifest.json.sig"), "manifest.json", public_key)
        manifest = json.loads(manifest_bytes)
        if manifest["schema_version"] not in (1, 2, 3, 4) or manifest["api_version"] != 1:
            raise ValueError("Unsupported module manifest")
        if not re.fullmatch(r"copypaste\.[a-z0-9][a-z0-9.-]*", manifest["id"]):
            raise ValueError("Invalid first-party module ID")
        version_tuple(manifest["version"])
        files = manifest["files"]
        inventory = {item["path"]: item for item in files}
        if len(inventory) != len(files) or set(names) != set(inventory) | {"manifest.json", "manifest.json.sig"}:
            raise ValueError("The signed file inventory does not match the package")
        total = 0
        for name, item in inventory.items():
            relative = PurePosixPath(name)
            entry = archive.getinfo(name)
            if (relative.is_absolute() or ".." in relative.parts or "\\" in name
                    or (entry.external_attr >> 16) & 0o170000 == 0o120000):
                raise ValueError("Unsafe package path")
            total += entry.file_size
            if entry.file_size != item["size_bytes"] or total > MAXIMUM_BYTES:
                raise ValueError("Invalid expanded module size")
            with archive.open(name) as source:
                digest = sha256_stream(source)
            if digest != item["sha256"]:
                raise ValueError("Module file checksum mismatch")
        platform = manifest["target"]["platform"]
        architecture = manifest["target"]["architecture"]
        suffix = {"macos": ".dylib", "windows": ".dll", "android": ".so"}[platform]
        if architecture not in {"x86", "x86_64", "arm", "aarch64"} or manifest["entrypoint"] != "bin/module" + suffix:
            raise ValueError("Invalid module target or entrypoint")
        if manifest["entrypoint"] not in inventory:
            raise ValueError("Module entrypoint is missing")
        minimum = None
        if "assets/module-distribution.json" in inventory:
            if archive.getinfo("assets/module-distribution.json").file_size > 64 * 1024:
                raise ValueError("Module distribution metadata is too large")
            distribution = json.loads(archive.read("assets/module-distribution.json"))
            minimum = distribution["minimum_system_versions"][platform]
            version_tuple(minimum)
    return VerifiedPackage(manifest, minimum)


def version_tuple(version):
    if not isinstance(version, str) or not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Marketplace modules require stable SemVer versions")
    return tuple(map(int, version.split(".")))


def supported_platforms(manifest):
    platforms = manifest.get("supported_platforms", ["macos", "windows", "android"])
    if (not isinstance(platforms, list) or not platforms
            or len(platforms) != len(set(platforms))
            or not set(platforms) <= {"macos", "windows", "android"}
            or manifest["target"]["platform"] not in platforms
            or (manifest["schema_version"] == 1 and "supported_platforms" in manifest)):
        raise ValueError("Invalid supported module platforms")
    return sorted(platforms)


def build_catalog(packages, release_tag, previous=None, public_key=PUBLIC_KEY):
    previous = previous or {"schema_version": 1, "modules": []}
    if previous["schema_version"] != 1:
        raise ValueError("Unsupported previous catalog")
    records = {}
    targets = set()
    module = None
    for path in sorted(map(Path, packages)):
        verified = read_package(path, public_key)
        manifest = verified.manifest
        metadata = {key: manifest[key] for key in ("id", "title", "description", "version", "app_versions")}
        metadata["supported_platforms"] = supported_platforms(manifest)
        if not metadata["title"].strip() or len(metadata["title"]) > 200 or not metadata["description"].strip() or len(metadata["description"]) > 4000:
            raise ValueError("Invalid module description")
        if module is None:
            module = {**metadata, "artifacts": []}
        elif metadata != {key: module[key] for key in metadata}:
            raise ValueError("All release packages must describe the same module version")
        target = manifest["target"]
        target_key = (target["platform"], target["architecture"])
        if target_key in targets:
            raise ValueError("Duplicate module target")
        targets.add(target_key)
        with path.open("rb") as source:
            digest = sha256_stream(source)
        module["artifacts"].append({
            **target, "url": f"{REPOSITORY}/{quote(release_tag, safe='')}/{quote(path.name, safe='')}",
            "size_bytes": path.stat().st_size, "sha256": digest,
            **({"minimum_system_version": verified.minimum_system_version} if verified.minimum_system_version else {}),
        })
    if module is None or targets != {target for target in REQUIRED_TARGETS if target[0] in module["supported_platforms"]}:
        raise ValueError("Qualified packages are required for every shipped platform and Android ABI")
    if release_tag != f"module-{module['id']}-v{module['version']}":
        raise ValueError("Release tag must identify the exact module ID and version")
    for existing in previous["modules"]:
        if existing["id"] in records:
            raise ValueError("Duplicate module in previous catalog")
        if existing["id"] == module["id"] and version_tuple(existing["version"]) >= version_tuple(module["version"]):
            raise ValueError("Marketplace releases must advance the installed module version")
        records[existing["id"]] = existing
    records[module["id"]] = module
    return {"schema_version": 1, "modules": [records[key] for key in sorted(records)]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packages-dir", required=True, type=Path)
    parser.add_argument("--release-tag", required=True)
    parser.add_argument("--previous-catalog", type=Path)
    parser.add_argument("--previous-signature", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.output.name != "modules.json":
        raise ValueError("The catalog filename must be modules.json")
    previous = None
    if args.previous_catalog:
        if not args.previous_signature:
            raise ValueError("The previous catalog signature is required")
        data = args.previous_catalog.read_bytes()
        signature = base64.b64decode(args.previous_signature.read_text().strip(), validate=True)
        verify_signature(data, signature, "modules.json")
        previous = json.loads(data)
    catalog = build_catalog(args.packages_dir.glob("*.cpmodule"), args.release_tag, previous)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(catalog, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    print(f"Prepared marketplace catalog with {len(catalog['modules'])} modules")


if __name__ == "__main__":
    main()
