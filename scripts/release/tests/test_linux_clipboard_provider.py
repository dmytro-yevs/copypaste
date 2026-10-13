import importlib.util
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_clipboard_provider", ROOT / "scripts/release/linux-clipboard-provider.py",
)
provider = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(provider)


class LinuxClipboardProviderTest(unittest.TestCase):
    def write_manifest(self, directory: Path) -> tuple[Path, Path]:
        payload = directory / "payload.txt"
        payload.write_text("private clipboard text", encoding="utf-8")
        manifest = directory / "manifest.json"
        manifest.write_text(
            json.dumps({
                "application_id": "org.copypaste.QualificationSource",
                "offers": [{"mime": "text/plain", "path": str(payload)}],
            }),
            encoding="utf-8",
        )
        return manifest, payload

    def test_request_audit_is_optional_and_is_forwarded_as_a_new_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, _ = self.write_manifest(root)
            helper = root / "helper"
            helper.write_text("helper", encoding="utf-8")
            helper.chmod(helper.stat().st_mode | stat.S_IXUSR)
            ready = root / "ready"
            activity = root / "activity"
            audit = root / "audit.jsonl"
            with mock.patch.object(provider.os, "execv", side_effect=RuntimeError("exec")) as execute:
                with self.assertRaisesRegex(RuntimeError, "exec"):
                    provider.run(helper, manifest, ready, activity, 20, audit)
            arguments = execute.call_args.args[1]
            self.assertIn("--request-audit-file", arguments)
            self.assertEqual(arguments[arguments.index("--request-audit-file") + 1], str(audit))
            self.assertFalse(audit.exists())

    def test_request_audit_requires_a_new_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, _ = self.write_manifest(root)
            helper = root / "helper"
            helper.write_text("helper", encoding="utf-8")
            helper.chmod(helper.stat().st_mode | stat.S_IXUSR)
            audit = root / "audit.jsonl"
            audit.write_text("existing", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "output already exists"):
                provider.run(helper, manifest, root / "ready", root / "activity", 20, audit)

    def test_omitting_request_audit_preserves_the_existing_provider_arguments(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, _ = self.write_manifest(root)
            helper = root / "helper"
            helper.write_text("helper", encoding="utf-8")
            helper.chmod(helper.stat().st_mode | stat.S_IXUSR)
            with mock.patch.object(provider.os, "execv", side_effect=RuntimeError("exec")) as execute:
                with self.assertRaisesRegex(RuntimeError, "exec"):
                    provider.run(helper, manifest, root / "ready", root / "activity", 20, None)
            self.assertNotIn("--request-audit-file", execute.call_args.args[1])

    def test_x11_audit_uses_xres_and_emits_only_target_and_attribution_metadata(self):
        source = (ROOT / "scripts/release/linux-clipboard-provider.c").read_text(encoding="utf-8")
        self.assertIn("#include <X11/extensions/XRes.h>", source)
        self.assertIn("gdk_window_add_filter(NULL, audit_selection_request, &provider)", source)
        self.assertIn("xevent->type != SelectionRequest", source)
        self.assertIn("XResQueryClientIds", source)
        self.assertIn("status != Success", source)
        self.assertIn("O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW", source)
        self.assertIn('"{\\\"target\\\":"', source)
        self.assertIn('"requestor_pid\\\":null', source)
        self.assertIn('"error\\\":null', source)
        self.assertNotIn("offer->bytes", source[source.index("audit_selection_request"):source.index("static void provide")])

    def test_build_links_the_maintained_xres_package(self):
        source = (ROOT / "scripts/release/linux-clipboard-provider.py").read_text(encoding="utf-8")
        self.assertIn('["pkg-config", "--cflags", "--libs", "gtk+-3.0", "x11", "xres"]', source)


if __name__ == "__main__":
    unittest.main()
