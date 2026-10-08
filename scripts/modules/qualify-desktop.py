#!/usr/bin/env python3
"""Run the native production-package scenarios with a real process restart."""
import argparse
import json
from pathlib import Path
import platform
import os
import subprocess
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[2]


def native_run(arguments):
    result = subprocess.run(arguments, capture_output=True, text=True)
    if result.returncode:
        print(result.stdout)
        print(result.stderr)
        raise RuntimeError("The production-signed package failed native qualification")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, choices=["macos", "windows"])
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--program", required=True, type=Path)
    parser.add_argument("--app-version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--fixtures", type=Path, default=ROOT / "scripts/modules/fixtures")
    args = parser.parse_args()
    expected_system = {"macos": "Darwin", "windows": "Windows"}[args.platform]
    expected_architecture = {"macos": {"arm64", "aarch64"}, "windows": {"AMD64", "x86_64"}}
    if platform.system() != expected_system or platform.machine() not in expected_architecture[args.platform]:
        raise ValueError("Native package qualification requires the exact shipped desktop target")
    prefix = ["/usr/bin/sandbox-exec", "-p", "(version 1)(allow default)(deny network*)"] if args.platform == "macos" else []
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
            if receipt["cases_passed"] != 3 or not receipt["signature_verified"]:
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
            })
            args.receipt.write_text(json.dumps(receipt), encoding="utf-8")
    finally:
        if args.platform == "windows":
            subprocess.run(["powershell", "-NoProfile", "-Command",
                "Remove-NetFirewallRule -Name $env:COPYPASTE_QUALIFICATION_RULE"], check=True, env=environment)
    print("Verified production-signed module scenarios and lifecycle on " + args.platform)


if __name__ == "__main__":
    main()
