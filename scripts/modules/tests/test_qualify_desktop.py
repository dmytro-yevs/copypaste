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


if __name__ == "__main__":
    unittest.main()
