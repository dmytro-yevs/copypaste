import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "bundle_linux_keybinder",
    ROOT / "scripts/release/bundle-linux-keybinder.py",
)
bundler = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(bundler)


class BundleLinuxKeybinderTest(unittest.TestCase):
    def test_contract_pins_the_portable_library_and_debian_owner(self):
        self.assertEqual(bundler.LIBRARY_SONAME, "libkeybinder-3.0.so.0")
        self.assertEqual(bundler.PACKAGE, "libkeybinder-3.0-0")
        self.assertEqual(bundler.MAX_GLIBC, "2.39")

    def test_soname_and_host_abi_are_read_from_the_real_elf_metadata(self):
        dynamic = """
 0x000000000000000e (SONAME)             Library soname: [libkeybinder-3.0.so.0]
 0x0000000000000001 (NEEDED)             Shared library: [libgtk-3.so.0]
 0x0000000000000001 (NEEDED)             Shared library: [libX11.so.6]
"""
        with mock.patch.object(bundler, "readelf_dynamic", return_value=dynamic):
            self.assertEqual(
                bundler.verify_soname(Path("/usr/lib/libkeybinder-3.0.so.0.0.0")),
                ["libX11.so.6", "libgtk-3.so.0"],
            )

    def test_architecture_must_match_the_appimage_target(self):
        with mock.patch.object(bundler, "run", return_value=mock.Mock(stdout="  Machine:                           AArch64\n")):
            bundler.verify_architecture(Path("/usr/lib/libkeybinder-3.0.so.0.0.0"), "aarch64")
        with mock.patch.object(bundler, "run", return_value=mock.Mock(stdout="  Machine:                           AArch64\n")):
            with self.assertRaisesRegex(bundler.BundleError, "architecture"):
                bundler.verify_architecture(Path("/usr/lib/libkeybinder-3.0.so.0.0.0"), "x86_64")

    def test_only_the_pinned_debian_package_can_supply_the_library(self):
        successful = [
            mock.Mock(stdout="libkeybinder-3.0-0:amd64: /usr/lib/libkeybinder-3.0.so.0.0.0\n"),
            mock.Mock(stdout="0.3.2-1\n"),
        ]
        with mock.patch.object(bundler, "run", side_effect=successful):
            self.assertEqual(bundler.package_owner(Path("/usr/lib/libkeybinder-3.0.so.0.0.0")), "0.3.2-1")
        with mock.patch.object(bundler, "run", return_value=mock.Mock(stdout="other-package: /usr/lib/libkeybinder-3.0.so.0.0.0\n")):
            with self.assertRaisesRegex(bundler.BundleError, "expected Debian package"):
                bundler.package_owner(Path("/usr/lib/libkeybinder-3.0.so.0.0.0"))

    def test_referenced_common_license_is_copied_with_a_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            copyright = root / "copyright"
            copyright.write_text("See /usr/share/common-licenses/LGPL-2.1.\n", encoding="utf-8")
            common = root / "common-licenses"
            common.mkdir()
            source = common / "LGPL-2.1"
            source.write_text("full LGPL terms", encoding="utf-8")
            destination = root / "destination"
            with mock.patch.object(bundler, "COMMON_LICENSE_ROOT", common):
                records = bundler.copy_common_licenses(copyright, destination)
            self.assertEqual(records, [{"path": "LGPL-2.1", "sha256": bundler.sha256(destination / "LGPL-2.1")}])

    def test_missing_referenced_common_license_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            copyright = root / "copyright"
            copyright.write_text("See /usr/share/common-licenses/LGPL-2.1.\n", encoding="utf-8")
            common = root / "common-licenses"
            common.mkdir()
            with mock.patch.object(bundler, "COMMON_LICENSE_ROOT", common):
                with self.assertRaisesRegex(bundler.BundleError, "common license bytes"):
                    bundler.copy_common_licenses(copyright, root / "destination")

    def test_bundle_records_hashed_library_license_and_abi_provenance(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            appdir = root / "AppDir"
            appdir.mkdir()
            source = root / "libkeybinder-3.0.so.0.0.0"
            source.write_bytes(b"keybinder")
            license_source = root / "copyright"
            license_source.write_text("license; see /usr/share/common-licenses/LGPL-2.1\n", encoding="utf-8")
            common = root / "common-licenses"
            common.mkdir()
            common.joinpath("LGPL-2.1").write_text("full LGPL terms", encoding="utf-8")
            manifest = appdir / "usr/share/copypaste/keybinder-runtime.json"
            with mock.patch.object(bundler, "library_from_cache", return_value=source), \
                 mock.patch.object(bundler, "verify_architecture"), \
                 mock.patch.object(bundler, "verify_soname", return_value=["libgtk-3.so.0", "libX11.so.6"]), \
                 mock.patch.object(bundler, "package_owner", return_value="0.3.2-1"), \
                 mock.patch.object(bundler, "package_license", return_value=license_source), \
                 mock.patch.object(bundler, "COMMON_LICENSE_ROOT", common), \
                 mock.patch.object(bundler, "validate_glibc") as validate_glibc:
                bundler.bundle(appdir=appdir, architecture="x86_64", manifest=manifest)
            document = json.loads(manifest.read_text(encoding="utf-8"))
            self.assertEqual(document["library"]["soname"], bundler.LIBRARY_SONAME)
            self.assertEqual(document["library"]["source_package"], bundler.PACKAGE)
            self.assertEqual(document["library"]["source_version"], "0.3.2-1")
            self.assertEqual(document["library"]["host_abi_sonames"], ["libgtk-3.so.0", "libX11.so.6"])
            self.assertEqual(document["library"]["sha256"], bundler.sha256(appdir / document["library"]["path"]))
            self.assertEqual(document["license"]["sha256"], bundler.sha256(appdir / document["license"]["path"]))
            common_record = document["license"]["common_licenses"]
            self.assertEqual(common_record, [{"path": "LGPL-2.1", "sha256": bundler.sha256(appdir / "usr/share/licenses/copypaste-keybinder/common/LGPL-2.1")}])
            validate_glibc.assert_called_once()

    def test_manifest_cannot_escape_the_appimage(self):
        with tempfile.TemporaryDirectory() as directory:
            appdir = Path(directory) / "AppDir"
            appdir.mkdir()
            with self.assertRaisesRegex(bundler.BundleError, "inside the AppImage"):
                bundler.bundle(appdir=appdir, architecture="x86_64", manifest=appdir.parent / "outside.json")


if __name__ == "__main__":
    unittest.main()
