#!/usr/bin/env python3
"""Qualify the exact signed OCR package inside an Android app process."""
import argparse
import json
from pathlib import Path
import subprocess
import time

PACKAGE = "com.copypaste.qualification"


def adb(*arguments, check=True):
    result = subprocess.run(["adb", *arguments], check=check, capture_output=True, text=True)
    return result.stdout.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apk", required=True, type=Path)
    parser.add_argument("--app-version", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    devices = adb("devices").splitlines()[1:]
    if len([device for device in devices if device.endswith("\tdevice")]) != 1:
        raise ValueError("Qualification requires exactly one online Android emulator")
    if adb("shell", "getprop", "ro.kernel.qemu") != "1" or adb("shell", "getprop", "ro.product.cpu.abi") != "x86_64":
        raise ValueError("Qualification requires the identified Android x86_64 emulator")
    adb("install", "-r", str(args.apk))
    if adb("shell", "pm", "clear", PACKAGE) != "Success":
        raise ValueError("Could not initialize isolated qualification storage")
    for phase in ["execute", "cleanup"]:
        adb("shell", "am", "force-stop", PACKAGE)
        adb("shell", "am", "start", "-n", PACKAGE + "/.MainActivity",
            "--es", "phase", phase, "--es", "appVersion", args.app_version,
            "--es", "commit", args.commit, "--es", "runId", args.run_id)
        # Exercise another Activity instance while the native workload is running.
        adb("shell", "am", "start", "-n", PACKAGE + "/.MainActivity",
            "--es", "phase", phase, "--es", "appVersion", args.app_version,
            "--es", "commit", args.commit, "--es", "runId", args.run_id)
        deadline = time.monotonic() + 180
        while True:
            value = adb("shell", "run-as", PACKAGE, "cat", "files/" + phase + ".json", check=False)
            if value.startswith("{"):
                receipt = json.loads(value)
                break
            if time.monotonic() >= deadline:
                raise TimeoutError("Native Android module qualification did not finish")
            time.sleep(2)
        if "failure" in receipt:
            print(adb("logcat", "-d", "-s", "CopyPasteQualification:E", check=False))
            raise ValueError("The signed OCR package failed inside the Android app process")
        if phase == "execute":
            if receipt["cases_passed"] != 3 or not receipt["signature_verified"]:
                raise ValueError("Android native OCR qualification is incomplete")
            args.receipt.write_text(json.dumps({
                **receipt, "environment": "android-emulator", "api": adb("shell", "getprop", "ro.build.version.sdk"),
                "internet_permission": False,
            }), encoding="utf-8")
        elif receipt.get("removal_completed_after_restart") is not True:
            raise ValueError("Android module removal did not complete after process restart")
    receipt = json.loads(args.receipt.read_text())
    receipt["removal_completed_after_restart"] = True
    args.receipt.write_text(json.dumps(receipt), encoding="utf-8")
    print("Verified production-signed OCR installation, inference, disable/enable, and removal on Android")


if __name__ == "__main__":
    main()
