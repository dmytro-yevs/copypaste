"""Keep the native scenario consumers aligned with canonical runtime bindings."""

import json
import hashlib
from pathlib import Path
import runpy
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
PRODUCER = runpy.run_path(str(ROOT / "scripts/release/produce-linux-native-qualification.py"))
BINDER = runpy.run_path(str(ROOT / "scripts/release/verify-linux-compositor-runtime.py"))
VERIFIER = runpy.run_path(str(ROOT / "scripts/release/verify-linux-native-qualification.py"))


class CompositorBindingContractTest(unittest.TestCase):
    def binding(self, distribution):
        return {"schema": 1, "producer_run_id": "7", "commit": "a" * 40,
                "runtime_id": "gnome-46-ubuntu24.04", "desktop": "GNOME", "architecture": "x86_64",
                "distribution": distribution, "format": "deb",
                "package": {"name": "runtime.deb", "sha256": "a" * 64, "size_bytes": 1},
                "runtime_receipt": {"name": "runtime.json", "sha256": "b" * 64, "size_bytes": 1}}

    def consume(self, binding):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "binding.json"
            path.write_text(json.dumps(binding))
            PRODUCER["compositor_runtime_binding"](path, "a" * 40, "GNOME", "x86_64", "ubuntu", "24.04", "deb")

    def receipt(self):
        return {"commit": "a" * 40, "desktop": "GNOME", "architecture": "x86_64",
                "environment": {"distribution": "ubuntu", "distribution_version": "24.04"}}

    def test_authenticated_raw_binding_uses_the_public_receipt_name_with_identical_bytes(self):
        def metadata(path):
            return {"name": path.name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                    "size_bytes": path.stat().st_size}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "runtime"
            (runtime / "bin").mkdir(parents=True)
            executable = runtime / "bin/session"
            executable.write_text("#!/bin/sh\nexit 0\n")
            executable.chmod(0o755)
            license_file = runtime / "COPYING"
            license_file.write_text("GPL-2.0-or-later")
            revision, shell_revision, patch, shell_patch = BINDER["BASELINES"]["GNOME"]
            source = {"revision": revision, "shell_revision": shell_revision,
                      "patch_sha256": hashlib.sha256((ROOT / patch).read_bytes()).hexdigest(),
                      "shell_patch_sha256": hashlib.sha256((ROOT / shell_patch).read_bytes()).hexdigest()}
            receipt = {"schema": 1, "runtime_id": "gnome-46-private-shell", "desktop": "GNOME",
                       "architecture": "x86_64", "distribution": {"id": "ubuntu", "version": "24.04"},
                       "source": source, "glibc_floor": "2.39", "runtime_env": {}, "host_requirements": [],
                       "launch": {"kind": "private", "entrypoint": "bin/session"},
                       "qualification": {"kind": "headless", "entrypoint": "bin/session"},
                       "upstream_licenses": [{"spdx": "GPL-2.0-or-later", "name": "COPYING",
                                              "sha256": metadata(license_file)["sha256"]}],
                       "payload": [{"path": path.relative_to(runtime).as_posix(), "type": "file",
                                    "mode": f"{path.stat().st_mode & 0o7777:04o}", "sha256": metadata(path)["sha256"]}
                                   for path in (executable, license_file)]}
            raw_receipt = root / "runtime-receipt.json"
            raw_receipt.write_text(json.dumps(receipt))
            package = root / "copypaste-compositor-runtime-gnome-46-private-shell-v1.0.24-linux-x86_64.deb"
            package.write_bytes(b"fixture package")
            package.with_name(package.name + ".sha256").write_text(f"{metadata(package)['sha256']}  {package.name}\n")
            license_copy = root / "license-0-COPYING"
            license_copy.write_bytes(license_file.read_bytes())
            producer = {key: receipt[key] for key in ("schema", "runtime_id", "desktop", "architecture", "distribution", "source", "glibc_floor")}
            producer.update(commit="a" * 40, producer_run_id=7, source_run_id=7, version="1.0.24", family="46",
                            format="deb", runtime_receipt=metadata(raw_receipt), package=metadata(package),
                            upstream_licenses=[metadata(license_copy)])
            (root / "compositor-runtime-producer-receipt.json").write_text(json.dumps(producer))
            binding = BINDER["binding"](root, "GNOME", "x86_64", "a" * 40, "7")
            self.assertEqual(binding["runtime_receipt"], {**metadata(raw_receipt), "name": package.name + ".runtime-receipt.json"})
            self.consume(binding)
            VERIFIER["verify_compositor_runtime"](binding, self.receipt(), "deb")

    def test_both_consumers_accept_the_runtime_producers_canonical_distribution(self):
        binding = self.binding({"id": "ubuntu", "version": "24.04"})
        self.consume(binding)
        VERIFIER["verify_compositor_runtime"](binding, self.receipt(), "deb")

    def test_both_consumers_reject_wrong_or_missing_distribution_identity_and_version(self):
        for distribution in ("ubuntu", {"id": "fedora", "version": "24.04"},
                             {"id": "ubuntu", "version": "24.10"}, {"id": "ubuntu"},
                             {"id": "ubuntu", "version": "24.04", "extra": True}):
            binding = self.binding(distribution)
            with self.subTest(distribution=distribution):
                with self.assertRaisesRegex(ValueError, "binding"):
                    self.consume(binding)
                with self.assertRaisesRegex(ValueError, "binding"):
                    VERIFIER["verify_compositor_runtime"](binding, self.receipt(), "deb")


if __name__ == "__main__":
    unittest.main()
