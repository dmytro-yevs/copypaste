#!/usr/bin/env python3
"""Run the native production-package scenarios with a real process restart."""
import argparse
import json
from pathlib import Path
import platform
import os
import re
import shutil
import subprocess
import tempfile
import uuid
import zipfile

from catalog import read_package

ROOT = Path(__file__).resolve().parents[2]


def native_run(arguments):
    result = subprocess.run(arguments, capture_output=True, text=True)
    if result.returncode:
        print(result.stdout)
        print(result.stderr)
        raise RuntimeError("The production-signed package failed native qualification")
    return result


def host_target(platform_name, machine):
    architectures = {
        "macos": {"arm64": "aarch64", "aarch64": "aarch64"},
        "windows": {"AMD64": "x86_64", "x86_64": "x86_64"},
        "linux": {"x86_64": "x86_64", "aarch64": "aarch64"},
    }
    try:
        return {"platform": platform_name, "architecture": architectures[platform_name][machine]}
    except KeyError as error:
        raise ValueError("Native package qualification requires the exact shipped desktop target") from error


def linux_glibc_floor(package):
    versions = []
    with zipfile.ZipFile(package) as archive, tempfile.TemporaryDirectory(prefix="module-glibc-") as directory:
        directory = Path(directory)
        libraries = [name for name in archive.namelist() if name.endswith(".so")]
        if not libraries:
            raise ValueError("Linux package has no shared libraries to inspect")
        for index, name in enumerate(libraries):
            library = directory / f"library-{index}.so"
            library.write_bytes(archive.read(name))
            readelf = shutil.which("readelf")
            command = [readelf, "--version-info", str(library)] if readelf else ["strings", str(library)]
            result = subprocess.run(command, check=True, capture_output=True, text=True)
            for value in re.findall(r"GLIBC_([0-9]+(?:\.[0-9]+){1,2})", result.stdout):
                parts = tuple(map(int, value.split(".")))
                versions.append(parts + (0,) * (3 - len(parts)))
    if not versions:
        raise ValueError("Linux package shared libraries do not declare a glibc ABI floor")
    return ".".join(map(str, max(versions)))


def linux_network_prefix(uid, gid, parent_network_namespace):
    # `unshare` creates the network namespace while privileged, then drops to
    # the runner identity before the module host receives any fixture or data
    # path. The shell also proves both conditions for every host restart.
    return [
        "sudo", "unshare", "--net", "--setgid", str(gid), "--setuid", str(uid), "--",
        "sh", "-ceu",
        'test "$(id -u)" = "$1"; test "$(readlink /proc/self/ns/net)" != "$2"; shift 2; exec "$@"',
        "module-qualification", str(uid), parent_network_namespace,
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, choices=["macos", "windows", "linux"])
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--program", required=True, type=Path)
    parser.add_argument("--app-version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--fixtures", type=Path, default=ROOT / "scripts/modules/fixtures")
    args = parser.parse_args()
    expected_system = {"macos": "Darwin", "windows": "Windows", "linux": "Linux"}[args.platform]
    if platform.system() != expected_system:
        raise ValueError("Native package qualification requires the exact shipped desktop target")
    expected_target = host_target(args.platform, platform.machine())
    package_target = read_package(args.package).manifest["target"]
    if package_target != expected_target:
        raise ValueError("Native package target does not match the qualification host")
    glibc_floor = linux_glibc_floor(args.package) if args.platform == "linux" else None
    prefix = []
    runner_uid = os.getuid() if args.platform == "linux" else None
    runner_gid = os.getgid() if args.platform == "linux" else None
    if args.platform == "macos":
        prefix = ["/usr/bin/sandbox-exec", "-p", "(version 1)(allow default)(deny network*)"]
    elif args.platform == "linux":
        prefix = linux_network_prefix(runner_uid, runner_gid, os.readlink("/proc/self/ns/net"))
    rule = "CopyPaste-module-qualification-" + uuid.uuid4().hex
    environment = {
        **os.environ, "COPYPASTE_QUALIFICATION_RULE": rule,
        "COPYPASTE_QUALIFICATION_PROGRAM": str(args.program.resolve()),
    }
    if args.platform == "windows":
        subprocess.run(["powershell", "-NoProfile", "-Command",
            "New-NetFirewallRule -Name $env:COPYPASTE_QUALIFICATION_RULE -DisplayName $env:COPYPASTE_QUALIFICATION_RULE -Direction Outbound -Program $env:COPYPASTE_QUALIFICATION_PROGRAM -Action Block -Profile Any | Out-Null; "
            "$rule = Get-NetFirewallRule -Name $env:COPYPASTE_QUALIFICATION_RULE; "
            "if ($rule.Enabled -ne 'True' -or $rule.Action -ne 'Block') { throw 'Offline qualification firewall policy is inactive' }"
        ], check=True, env=environment)
    try:
        with tempfile.TemporaryDirectory(prefix="module-native-qualification-") as data:
            result = native_run([
                *prefix, str(args.program.resolve()), str(args.package.resolve()),
                str(args.fixtures.resolve()), data, args.app_version,
                args.commit, args.run_id,
            ])
            receipt = json.loads(result.stdout)
            if (receipt["target"] != expected_target or receipt["cases_passed"] != 3
                    or not receipt["signature_verified"]):
                raise ValueError("Desktop package qualification is incomplete")
            result = native_run([
                *prefix, str(args.program.resolve()), "--finish-removal", data, args.app_version,
            ])
            if not json.loads(result.stdout)["removal_completed_after_restart"]:
                raise ValueError("Module removal did not complete after process restart")
            receipt.update({
                "removal_completed_after_restart": True,
                "environment": "github-native-" + args.platform,
                "system_version": platform.platform(),
                **({
                    "glibc_floor": glibc_floor,
                    "effective_uid": runner_uid,
                    "network_namespace_isolated": True,
                } if glibc_floor else {}),
            })
            args.receipt.write_text(json.dumps(receipt), encoding="utf-8")
    finally:
        if args.platform == "windows":
            subprocess.run(["powershell", "-NoProfile", "-Command",
                "Remove-NetFirewallRule -Name $env:COPYPASTE_QUALIFICATION_RULE"], check=True, env=environment)
    print("Verified production-signed module scenarios and lifecycle on " + args.platform)


if __name__ == "__main__":
    main()
