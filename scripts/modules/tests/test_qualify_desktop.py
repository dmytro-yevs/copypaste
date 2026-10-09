import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


sys.path.insert(0, str(Path(__file__).parents[1]))
SPEC = importlib.util.spec_from_file_location("qualify_desktop", Path(__file__).parents[1] / "qualify-desktop.py")
qualification = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qualification)


class DesktopQualificationTargetTest(unittest.TestCase):
    def test_windows_qualification_does_not_require_unix_identity_functions(self):
        with tempfile.TemporaryDirectory() as directory:
            receipt = Path(directory) / "receipt.json"
            target = {"platform": "windows", "architecture": "x86_64"}
            arguments = [
                "qualify-desktop.py", "--platform", "windows",
                "--package", "fixture.cpmodule", "--program", "fixture.exe",
                "--app-version", "1.0.24", "--commit", "a" * 40,
                "--run-id", "123", "--receipt", str(receipt),
            ]
            results = [
                SimpleNamespace(stdout=json.dumps({
                    "target": target, "cases_passed": 3, "signature_verified": True,
                })),
                SimpleNamespace(stdout=json.dumps({
                    "removal_completed_after_restart": True,
                })),
            ]
            with mock.patch.object(sys, "argv", arguments), \
                    mock.patch.object(qualification.platform, "system", return_value="Windows"), \
                    mock.patch.object(qualification.platform, "machine", return_value="AMD64"), \
                    mock.patch.object(qualification, "read_package", return_value=SimpleNamespace(manifest={"target": target})), \
                    mock.patch.object(qualification, "native_run", side_effect=results), \
                    mock.patch.object(qualification.subprocess, "run"), \
                    mock.patch.object(qualification.os, "getuid", side_effect=AssertionError("Windows has no getuid"), create=True), \
                    mock.patch.object(qualification.os, "getgid", side_effect=AssertionError("Windows has no getgid"), create=True):
                qualification.main()
            self.assertTrue(json.loads(receipt.read_text())["removal_completed_after_restart"])

    def test_host_target_normalizes_only_shipped_native_architectures(self):
        self.assertEqual(qualification.host_target("linux", "x86_64"), {"platform": "linux", "architecture": "x86_64"})
        self.assertEqual(qualification.host_target("linux", "aarch64"), {"platform": "linux", "architecture": "aarch64"})
        self.assertEqual(qualification.host_target("windows", "AMD64"), {"platform": "windows", "architecture": "x86_64"})
        with self.assertRaises(ValueError):
            qualification.host_target("linux", "armv7l")

    def test_linux_network_prefix_drops_to_the_runner_identity_without_preserving_environment(self):
        prefix = qualification.linux_network_prefix(1001, 1002, "net:[12345]")
        self.assertEqual(
            prefix[:8],
            ["sudo", "unshare", "--net", "--setgid", "1002", "--setuid", "1001", "--"],
        )
        self.assertNotIn("--preserve-env", prefix)
        self.assertIn('test "$(id -u)" = "$1"', prefix[10])
        self.assertIn('readlink /proc/self/ns/net', prefix[10])


if __name__ == "__main__":
    unittest.main()
