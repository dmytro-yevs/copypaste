import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile

SPEC = importlib.util.spec_from_file_location("catalog", Path(__file__).parents[1] / "catalog.py")
catalog = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(catalog)


class CatalogTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="marketplace-publisher-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.key = self.root / "private.pem"
        subprocess.run(["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(self.key)], check=True, capture_output=True)
        result = subprocess.run(["openssl", "pkey", "-in", str(self.key), "-pubout", "-outform", "DER"], check=True, capture_output=True)
        self.key_id = b"test-key"
        self.public_key = base64.b64encode(b"Ed" + self.key_id + result.stdout[-32:]).decode()

    def sign(self, data, filename="manifest.json"):
        def signature(message):
            path = self.root / "message"
            path.write_bytes(message)
            return subprocess.run(["openssl", "pkeyutl", "-sign", "-rawin", "-inkey", str(self.key), "-in", str(path)], check=True, capture_output=True).stdout
        signed = signature(hashlib.blake2b(data).digest())
        comment = f"timestamp:1\tfile:{filename}"
        global_signature = signature(signed + comment.encode())
        return ("untrusted comment: test\n" + base64.b64encode(b"ED" + self.key_id + signed).decode()
                + "\ntrusted comment: " + comment + "\n" + base64.b64encode(global_signature).decode()).encode()

    def packages(self, version="0.1.0", omit=None, inconsistent=False, tamper=False, platforms=None, schema=1):
        paths = []
        for platform, architecture in sorted(catalog.REQUIRED_TARGETS):
            if platforms is None and platform not in catalog.LEGACY_PLATFORMS:
                continue
            if platforms is not None and platform not in platforms:
                continue
            if (platform, architecture) == omit:
                continue
            suffix = {"macos": ".dylib", "windows": ".dll", "linux": ".so", "android": ".so"}[platform]
            entrypoint = "bin/module" + suffix
            code = b"test-native-library"
            manifest = {
                "schema_version": schema, "api_version": 1, "id": "copypaste.ocr", "title": "OCR",
                "description": "Offline recognition.", "version": version, "app_versions": ">=1.0.0, <2.0.0",
                "target": {"platform": platform, "architecture": architecture}, "entrypoint": entrypoint,
                "files": [{"path": entrypoint, "size_bytes": len(code), "sha256": hashlib.sha256(code).hexdigest()}],
                "commands": [], "preferences": [],
            }
            if inconsistent and platform == "windows":
                manifest["description"] = "Different module contract."
            if platforms is not None:
                manifest["schema_version"] = max(2, schema)
                manifest["supported_platforms"] = platforms
            body = json.dumps(manifest).encode()
            path = self.root / f"ocr-{platform}-{architecture}.cpmodule"
            with zipfile.ZipFile(path, "w") as archive:
                archive.writestr("manifest.json", body)
                archive.writestr("manifest.json.sig", self.sign(body))
                archive.writestr(entrypoint, b"tampered" if tamper else code)
            paths.append(path)
        return paths

    def test_signed_schema_four_sync_packages_are_supported_and_future_schemas_fail_closed(self):
        packages = self.packages(schema=4, platforms=["macos", "windows", "android"])
        value = catalog.build_catalog(packages, "module-copypaste.ocr-v0.1.0", public_key=self.public_key)
        self.assertEqual(len(value["modules"][0]["artifacts"]), 5)
        with self.assertRaises(ValueError):
            catalog.read_package(self.packages(schema=6)[0], self.public_key)

    def test_linux_targets_preserve_published_legacy_catalog_entries(self):
        previous = {
            "schema_version": 1,
            "modules": [{
                "id": "copypaste.published", "version": "0.1.0",
                "artifacts": [{"platform": "macos", "architecture": "aarch64"}],
            }],
        }
        paths = self.packages(platforms=["macos", "windows", "linux", "android"], schema=5)
        value = catalog.build_catalog(paths, "module-copypaste.ocr-v0.1.0", previous, self.public_key)
        current = next(module for module in value["modules"] if module["id"] == "copypaste.ocr")
        self.assertEqual(
            {(artifact["platform"], artifact["architecture"]) for artifact in current["artifacts"]},
            catalog.REQUIRED_TARGETS,
        )
        self.assertEqual(
            {artifact["architecture"] for artifact in current["artifacts"] if artifact["platform"] == "linux"},
            {"x86_64", "aarch64"},
        )
        with self.assertRaises(ValueError):
            catalog.build_catalog(
                self.packages(platforms=["macos", "windows", "linux", "android"], schema=4),
                "module-copypaste.ocr-v0.1.0",
                public_key=self.public_key,
            )

    def test_signed_package_catalog_retains_other_modules_and_exact_targets(self):
        paths = self.packages(platforms=["macos", "windows", "linux", "android"], schema=5)
        previous = {"schema_version": 1, "modules": [
            {"id": "copypaste.other", "version": "1.0.0", "artifacts": []},
            {"id": "copypaste.ocr", "version": "0.0.9", "artifacts": []},
        ]}
        value = catalog.build_catalog(paths, "module-copypaste.ocr-v0.1.0", previous, self.public_key)
        self.assertEqual(len(value["modules"]), 2)
        entry = value["modules"][0]
        self.assertEqual(entry["version"], "0.1.0")
        self.assertEqual({(a["platform"], a["architecture"]) for a in entry["artifacts"]}, catalog.REQUIRED_TARGETS)
        for artifact, path in zip(entry["artifacts"], sorted(paths)):
            self.assertEqual(artifact["size_bytes"], path.stat().st_size)
            self.assertEqual(artifact["sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
            self.assertTrue(artifact["url"].endswith(path.name))

    def test_rejects_missing_platform_and_mismatched_manifests(self):
        for options in [{"omit": ("android", "arm")}, {"inconsistent": True}]:
            paths = self.packages(**options)
            with self.assertRaises(ValueError):
                catalog.build_catalog(paths, "module-copypaste.ocr-v0.1.0", public_key=self.public_key)

    def test_explicit_android_only_module_requires_every_shipped_android_abi(self):
        paths = self.packages(platforms=["android"])
        value = catalog.build_catalog(paths, "module-copypaste.ocr-v0.1.0", public_key=self.public_key)
        self.assertEqual(value["modules"][0]["supported_platforms"], ["android"])
        self.assertEqual(len(value["modules"][0]["artifacts"]), 3)
        with self.assertRaises(ValueError):
            catalog.build_catalog(paths[:-1], "module-copypaste.ocr-v0.1.0", public_key=self.public_key)

    def test_rejects_tampering_foreign_signer_and_wrong_tag(self):
        with self.assertRaises(ValueError):
            catalog.read_package(self.packages(tamper=True)[0], self.public_key)
        paths = self.packages()
        with self.assertRaises(ValueError):
            catalog.read_package(paths[0])
        with self.assertRaises(ValueError):
            catalog.build_catalog(paths, "module-copypaste.ocr-v0.2.0", public_key=self.public_key)

    def test_rejects_equal_versions_and_downgrades(self):
        paths = self.packages()
        for version in ["0.1.0", "0.2.0"]:
            previous = {"schema_version": 1, "modules": [{"id": "copypaste.ocr", "version": version}]}
            with self.assertRaises(ValueError):
                catalog.build_catalog(paths, "module-copypaste.ocr-v0.1.0", previous, self.public_key)

    def test_catalog_signature_authenticates_contents_and_filename(self):
        data = b'{"schema_version":1,"modules":[]}'
        signature = self.sign(data, "modules.json")
        catalog.verify_signature(data, signature, "modules.json", self.public_key)
        for body, filename in [(data + b" ", "modules.json"), (data, "other.json")]:
            with self.assertRaises(ValueError):
                catalog.verify_signature(body, signature, filename, self.public_key)


if __name__ == "__main__":
    unittest.main()
