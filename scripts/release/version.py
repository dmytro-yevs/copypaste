#!/usr/bin/env python3
"""Own release identity and synchronize the manifests required by Cargo and Flutter."""

import argparse
import json
from pathlib import Path
import re
import subprocess


ROOT = Path(__file__).resolve().parents[2]
SOURCE = "config/release.json"
PUBSPEC = "apps/copypaste_flutter/pubspec.yaml"
SEMVER = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"


def validate(value):
    if not isinstance(value, dict) or set(value) != {"version", "build_number", "android_version_code"}:
        raise ValueError(f"{SOURCE} must contain version, build_number and android_version_code")
    if not isinstance(value["version"], str) or not re.fullmatch(SEMVER, value["version"]):
        raise ValueError("release version must be stable SemVer without a v prefix")
    if type(value["build_number"]) is not int or not 1 <= value["build_number"] <= 65535:
        raise ValueError("build_number must be 1..65535 for Windows version resources")
    if type(value["android_version_code"]) is not int or not 200000038 < value["android_version_code"] <= 2100000000:
        raise ValueError("android_version_code must preserve the established upgrade range")
    return value


def load(root):
    return validate(json.loads((root / SOURCE).read_text(encoding="utf-8")))


def table(text, name):
    matches = list(re.finditer(r"(?m)^\[" + re.escape(name) + r"\][ \t]*\n(?P<body>.*?)(?=^\[|\Z)", text, re.DOTALL))
    if len(matches) != 1:
        raise ValueError(f"expected one [{name}] table in Cargo.toml")
    return matches[0]


def field(text, name, key, value):
    section = table(text, name)
    body, count = re.subn(
        r"(?m)^(" + re.escape(key) + r"[ \t]*=[ \t]*)(?:\"[^\"\n]*\"|[0-9]+)([ \t]*(?:#.*)?)$",
        lambda match: match[1] + json.dumps(value) + match[2], section["body"],
    )
    if count != 1:
        raise ValueError(f"expected one {name}.{key} field in Cargo.toml")
    return text[:section.start("body")] + body + text[section.end("body"):]


def projections(root, release):
    cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
    cargo = field(cargo, "workspace.package", "version", release["version"])
    cargo = field(cargo, "workspace.metadata.copypaste", "android-release-version-code",
                  release["android_version_code"])
    pubspec, count = re.subn(
        r"(?m)^version:[^\n]*$", f"version: {release['version']}+{release['build_number']}",
        (root / PUBSPEC).read_text(encoding="utf-8"),
    )
    if count != 1:
        raise ValueError("expected one Flutter version field")
    expected = {"Cargo.toml": cargo, PUBSPEC: pubspec}
    for path in sorted((root / "modules").glob("*/module.json")):
        text = path.read_text(encoding="utf-8")
        compatibility = json.loads(text).get("app_versions", "")
        bounds = re.fullmatch(r">=(" + SEMVER + r"), <(" + SEMVER + r")", compatibility)
        if not bounds or tuple(map(int, release["version"].split("."))) >= tuple(map(int, bounds[2].split("."))):
            raise ValueError(f"{path.relative_to(root)} needs an explicit module compatibility decision")
        updated = f">={release['version']}, <{bounds[2]}"
        text, count = re.subn(r'("app_versions"[ \t]*:[ \t]*)"[^"\n]*"',
                             lambda match: match[1] + json.dumps(updated), text)
        if count != 1:
            raise ValueError(f"expected one app_versions field in {path.relative_to(root)}")
        expected[path.relative_to(root).as_posix()] = text
    return expected


def lockfiles(root):
    return [root / "Cargo.lock", *sorted((root / "modules").glob("*/Cargo.lock"))]


def check_lock(root, release):
    cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
    members = re.search(r"(?m)^members[ \t]*=[ \t]*\[(.*?)\]", table(cargo, "workspace")["body"], re.DOTALL)
    if not members:
        raise ValueError("workspace members are missing")
    inherited = []
    for member in re.findall(r'"([^"\n]+)"', members[1]):
        manifest = (root / member / "Cargo.toml").read_text(encoding="utf-8")
        package = table(manifest, "package")["body"]
        if not re.search(r"(?m)^version\.workspace[ \t]*=[ \t]*true[ \t]*(?:#.*)?$", package):
            continue
        name = re.search(r'(?m)^name[ \t]*=[ \t]*"([^"\n]+)"', package)[1]
        inherited.append(name)
    for path in lockfiles(root):
        blocks = re.split(r"(?m)^\[\[package\]\][ \t]*\n", path.read_text(encoding="utf-8"))[1:]
        for name in inherited:
            records = [block for block in blocks if re.search(r'(?m)^name = "' + re.escape(name) + r'"$', block)
                       and not re.search(r"(?m)^source[ \t]*=", block)]
            if not records and path != root / "Cargo.lock":
                continue
            if len(records) != 1 or not re.search(r'(?m)^version = "' + re.escape(release["version"]) + r'"$', records[0]):
                raise ValueError(f"{path.relative_to(root)} has stale workspace version for {name}; run version.py sync")


def check(root, release):
    for name, expected in projections(root, release).items():
        if (root / name).read_text(encoding="utf-8") != expected:
            raise ValueError(f"{name} differs from {SOURCE}; run python3 scripts/release/version.py sync")
    check_lock(root, release)


def sync(root, release):
    validate(release)
    expected = projections(root, release)
    locks = lockfiles(root)
    snapshots = {root / name: (root / name).read_bytes() for name in (SOURCE, *expected)}
    snapshots.update({path: path.read_bytes() for path in locks})
    try:
        for name, text in expected.items():
            (root / name).write_text(text, encoding="utf-8")
        # Cargo owns lockfile resolution; retain locked external dependencies.
        for path in locks:
            subprocess.run(["cargo", "update", "--manifest-path", str(path.parent / "Cargo.toml"),
                            "--workspace", "--offline"], cwd=root, check=True)
        check(root, release)
        (root / SOURCE).write_text(json.dumps(release, indent=2) + "\n", encoding="utf-8")
    except (OSError, ValueError, subprocess.CalledProcessError):
        for path, content in snapshots.items():
            path.write_bytes(content)
        raise


def bump(root, version):
    before = load(root)
    check(root, before)
    after = validate({"version": version, "build_number": before["build_number"] + 1,
                      "android_version_code": before["android_version_code"] + 1})
    if tuple(map(int, version.split("."))) <= tuple(map(int, before["version"].split("."))):
        raise ValueError("new release version must increase")
    sync(root, after)


def create_tag(root, release):
    check(root, release)
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True).strip():
        raise ValueError("commit the release changes before creating its tag")
    subprocess.run(["git", "tag", "-a", "v" + release["version"], "-m", "Release " + release["version"]],
                   cwd=root, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    commands = parser.add_subparsers(dest="command", required=True)
    verify = commands.add_parser("check", help="reject stale manifests or a mismatched build/tag")
    verify.add_argument("--version")
    verify.add_argument("--build-number", type=int)
    verify.add_argument("--tag")
    commands.add_parser("values", help="print version, build number, Android code and tag for CI")
    commands.add_parser("sync", help="update Cargo, Flutter and the lockfile from the canonical source")
    commands.add_parser("bump", help="set the next version and increment both build counters").add_argument("version")
    commands.add_parser("tag", help="create the canonical local tag from a clean committed checkout; never push")
    args = parser.parse_args()
    try:
        release = load(args.root)
        if args.command == "sync":
            sync(args.root, release)
        elif args.command == "bump":
            bump(args.root, args.version)
        else:
            check(args.root, release)
            tag = "v" + release["version"]
            if args.command == "values":
                print(release["version"], release["build_number"], release["android_version_code"], tag)
            elif args.command == "tag":
                create_tag(args.root, release)
            elif ((args.version is not None and args.version != release["version"])
                  or (args.build_number is not None and args.build_number != release["build_number"])
                  or (args.tag is not None and args.tag != tag)):
                raise ValueError("requested build or tag differs from the canonical release identity")
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"release identity: {error}\n")


if __name__ == "__main__":
    main()
