import importlib.util
from pathlib import Path
import sys
import unittest


sys.path.insert(0, str(Path(__file__).parents[1]))
SPEC = importlib.util.spec_from_file_location("qualify_desktop", Path(__file__).parents[1] / "qualify-desktop.py")
qualification = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qualification)


class DesktopQualificationTargetTest(unittest.TestCase):
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
