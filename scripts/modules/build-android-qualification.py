#!/usr/bin/env python3
"""Build a test-only Android host for the signed package's real native lifecycle."""
import argparse
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]


def run(arguments, **kwargs):
    subprocess.run(list(map(str, arguments)), check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk", type=Path, required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    tools = args.sdk / "build-tools/36.0.0"
    android = args.sdk / "platforms/android-36/android.jar"
    if not android.is_file() or not tools.is_dir():
        raise ValueError("Android qualification requires API 36 and build-tools 36.0.0")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="android-module-qualification-") as directory:
        directory = Path(directory)
        classes = directory / "classes"
        classes.mkdir()
        run(["javac", "-source", "8", "-target", "8", "-classpath", android,
             "-d", classes, ROOT / "scripts/modules/android-qualification/MainActivity.java"])
        dex = directory / "dex"
        dex.mkdir()
        run([tools / "d8", "--lib", android, "--output", dex, *classes.rglob("*.class")])
        staged = directory / "host.apk"
        run([tools / "aapt2", "link", "-I", android, "--manifest",
             ROOT / "scripts/modules/android-qualification/AndroidManifest.xml", "-o", staged])
        with zipfile.ZipFile(staged, "a", compression=zipfile.ZIP_STORED) as archive:
            archive.write(dex / "classes.dex", "classes.dex")
            archive.write(args.library, "lib/x86_64/libcopypaste_module_qualification.so")
            archive.write(args.package, "assets/ocr.cpmodule")
            for fixture in sorted((ROOT / "scripts/modules/fixtures").iterdir()):
                archive.write(fixture, "assets/fixtures/" + fixture.name)
        aligned = directory / "aligned.apk"
        run([tools / "zipalign", "-P", "16", "-f", "4", staged, aligned])
        key = directory / "qualification.jks"
        environment = {**os.environ, "COPYPASTE_QUALIFICATION_KEY_PASSWORD": secrets.token_hex(16)}
        run(["keytool", "-genkeypair", "-keystore", key, "-alias", "qualification",
             "-storepass:env", "COPYPASTE_QUALIFICATION_KEY_PASSWORD",
             "-keypass:env", "COPYPASTE_QUALIFICATION_KEY_PASSWORD",
             "-keyalg", "RSA", "-keysize", "2048", "-validity", "2",
             "-dname", "CN=CopyPaste temporary module qualification"], env=environment)
        run([tools / "apksigner", "sign", "--ks", key, "--ks-key-alias", "qualification",
             "--ks-pass", "env:COPYPASTE_QUALIFICATION_KEY_PASSWORD",
             "--key-pass", "env:COPYPASTE_QUALIFICATION_KEY_PASSWORD",
             "--out", args.output, aligned], env=environment)
    print(f"Built test-only Android qualification APK: {args.output}")


if __name__ == "__main__":
    main()
