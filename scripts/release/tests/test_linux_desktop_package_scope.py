"""Exercise the package gate used by the installed desktop scenario."""

from pathlib import Path
import re
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = (ROOT / "scripts/release/run-linux-native-scenarios.sh").read_text()
PACKAGE_GATE = re.search(
    r"^require_desktop_packages\(\) \{.*?^\}\n", SCRIPT, re.MULTILINE | re.DOTALL
).group(0)


class DesktopPackageScopeTest(unittest.TestCase):
    def check(self, formats, native_format="deb", transition="current", symlink=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for format_name in formats:
                path = root / f"CopyPaste-v1.2.3-linux-x86_64.{format_name}"
                path.write_bytes(b"exact package")
                if format_name == symlink:
                    target = root / "unbound-package"
                    path.rename(target)
                    path.symlink_to(target)
            return subprocess.run(
                ["bash", "-c", "set -euo pipefail\nARCHITECTURE=x86_64\nnative_format=$2\n"
                 + PACKAGE_GATE + '\nrequire_desktop_packages "$1" 1.2.3 "$3"',
                 "desktop-package-gate", directory, native_format, transition],
                capture_output=True, text=True,
            )

    def test_gnome_install_does_not_require_an_unreleased_rpm(self):
        self.assertEqual(self.check(["AppImage", "deb"]).returncode, 0)

    def test_fedora_scenario_retains_its_rpm_install_boundary(self):
        self.assertEqual(self.check(["AppImage", "rpm"], "rpm").returncode, 0)

    def test_each_exercised_current_format_is_mandatory(self):
        for formats, missing in ((["AppImage"], "deb"), (["deb"], "AppImage")):
            result = self.check(formats)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(f"exact current {missing} package is required", result.stderr)

    def test_prior_release_requires_the_same_native_format(self):
        result = self.check(["AppImage", "deb"], "rpm", "prior")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exact prior rpm package is required", result.stderr)

    def test_prior_gnome_packages_do_not_require_rpm(self):
        self.assertEqual(self.check(["AppImage", "deb"], transition="prior").returncode, 0)

    def test_symlink_cannot_replace_an_exact_package(self):
        result = self.check(["AppImage", "deb"], symlink="deb")
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
