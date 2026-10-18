"""The generated matrix cannot silently promote unqualified targets."""

from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts/release"))
import platform_manifest  # noqa: E402


class PlatformManifestTest(unittest.TestCase):
    def test_stable_qualification_matches_the_exact_release_row(self):
        rows = platform_manifest.matrix("stable", "qualification")["include"]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["architecture"], "x86_64")
        self.assertEqual(rows[0]["session"], "x11")
        self.assertEqual(rows[0]["compositor_artifact"],
                         "copypaste-compositor-runtime-gnome-46-ubuntu24.04-x86_64")

    def test_experimental_builds_do_not_become_qualified(self):
        self.assertEqual(platform_manifest.matrix("experimental", "qualification"),
                         {"include": []})
        self.assertEqual({row["architecture"] for row in
                          platform_manifest.matrix("experimental", "build")["include"]},
                         {"x86_64", "aarch64"})

    def test_supplemental_jobs_only_build_architectures_missing_from_stable_ci(self):
        rows = platform_manifest.matrix("all", "supplemental-build")["include"]
        self.assertEqual([row["architecture"] for row in rows], ["aarch64"])
        self.assertEqual(rows[0]["runner"], "ubuntu-24.04-arm")
        self.assertEqual(rows[0]["formats"], "AppImage,deb")

    def test_compatibility_table_tracks_every_declared_target(self):
        table = platform_manifest.table()
        documentation = (ROOT / "docs/platform/linux-support.md").read_text(encoding="utf-8")
        self.assertIn(table, documentation)
        for target in platform_manifest.TARGETS:
            self.assertIn(f"{target['distribution']} {target['version']}", table)
        self.assertIn("not yet qualified", table)


if __name__ == "__main__":
    unittest.main()
