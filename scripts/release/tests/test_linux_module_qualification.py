import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "linux_module_qualification", ROOT / "scripts/release/linux-module-qualification.py",
)
qualification = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(qualification)


class LinuxModuleQualificationTest(unittest.TestCase):
    def test_native_receipt_binds_the_exact_product_version_and_package_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory) / "module.cpmodule"
            package.write_bytes(b"signed-package-bytes")
            package.with_name(package.name + ".receipt.json").write_text(json.dumps({
                "module_id": "copypaste.ocr",
                "target": {"platform": "linux", "architecture": "x86_64"},
                "app_version": "1.0.23",
                "package_sha256": qualification.sha256(package),
                "package_size_bytes": package.stat().st_size,
                "signature_verified": True,
                "cases_passed": 3,
            }), encoding="utf-8")
            qualification.receipt_for(package, "copypaste.ocr", "x86_64", "1.0.23")
            with self.assertRaisesRegex(ValueError, "does not bind"):
                qualification.receipt_for(package, "copypaste.ocr", "x86_64", "1.0.24")

    def test_privileged_operations_use_the_gui_and_reads_use_its_daemon(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts = root / "artifacts"
            fixtures = root / "fixtures"
            evidence = root / "evidence"
            application_data = root / "application-data"
            artifacts.mkdir()
            fixtures.mkdir()
            application_data.mkdir()
            packages = {}
            for module_id in qualification.MODULES:
                package = artifacts / (module_id + ".cpmodule")
                receipt = package.with_name(package.name + ".receipt.json")
                package.write_bytes(module_id.encode())
                receipt.write_text("{}", encoding="utf-8")
                packages[module_id] = {
                    "package": package, "receipt_path": receipt,
                    "receipt": {}, "manifest": {"id": module_id, "title": module_id.upper()},
                }
            titles = {package["manifest"]["title"]: module_id for module_id, package in packages.items()}
            state = {}
            socket_operations = []
            ui_calls = []
            restarts = []
            records = []

            def response(value):
                return {
                    "id": 992, "ok": True,
                    "data": {"modules": {"json": json.dumps(value)}},
                }

            def fake_rpc(socket_path, operation):
                socket_operations.append(operation["operation"])
                trace = {
                    "endpoint_category": "gui_owned_unix_socket", "method": "modules",
                    "operation": operation["operation"], "module_id": operation.get("id"),
                    "request_sha256": "a" * 64, "response_sha256": "b" * 64,
                }
                if operation["operation"] == "list":
                    return response(list(state.values())), trace
                if operation["operation"] == "invoke":
                    if operation["id"] == "copypaste.ocr":
                        return response({"kind": "text", "text": "CopyPaste"}), trace
                    if operation["id"] == "copypaste.semantic-search":
                        return response({"kind": "embeddings", "vectors": [[0.0] * 384]}), trace
                    return response({"kind": "data", "data": {"status": {"configured": False, "signed_in": False}}}), trace
                raise AssertionError(operation)

            def action(operation):
                return {"operation": operation, "request_sha256": "c" * 64, "response_sha256": "d" * 64}

            class Screen:
                def install(self, package):
                    module_id = next(module for module, item in packages.items() if item["package"] == package)
                    ui_calls.append(("install", module_id))
                    state[module_id] = {"id": module_id, "enabled": module_id == "copypaste.ocr"}
                    return action("install")

                def set_languages(self, title, languages):
                    ui_calls.append(("set_preferences", titles[title], tuple(languages)))
                    return action("set_preferences")

                def set_enabled(self, title, enabled):
                    ui_calls.append(("set_enabled", titles[title]))
                    state[titles[title]]["enabled"] = not state[titles[title]]["enabled"]
                    return action("set_enabled")

                def remove(self, title):
                    ui_calls.append(("remove", titles[title]))
                    del state[titles[title]]
                    return action("remove")

            def restart():
                restarts.append(True)
                return f"socket-{len(restarts)}"

            with mock.patch.object(qualification, "staged_packages", return_value=packages), \
                    mock.patch.object(qualification, "ocr_fixture", return_value=(root / "english.png", "CopyPaste", {"ocr/english.png": "a" * 64})), \
                    mock.patch.object(qualification, "semantic_fixture", return_value=({"id": "english", "files": []}, {"semantic-search/model.onnx": "b" * 64})), \
                    mock.patch.object(qualification, "seed_semantic_model"), \
                    mock.patch.object(qualification, "rpc", side_effect=fake_rpc):
                final_socket = qualification.qualify_modules(
                    "socket-0", artifacts, fixtures, application_data, "x86_64", "1.0.23", evidence, restart,
                    records.append, ui=Screen(),
                )
            self.assertEqual(final_socket, "socket-3")
            self.assertEqual(len(restarts), 3)
            self.assertEqual(set(socket_operations), {"list", "invoke"})
            self.assertIn(("set_preferences", "copypaste.semantic-search", ("English",)), ui_calls)
            for module_id in qualification.MODULES:
                operations = [call[0] for call in ui_calls if call[1] == module_id]
                toggles = 3 if module_id == "copypaste.ocr" else 2
                self.assertEqual([item for item in operations if item != "set_preferences"],
                                 ["install", *["set_enabled"] * toggles, "remove"])
            for record in records:
                expected = "gui_owned_unix_socket" if record["operation"] in {"list", "invoke"} else "gui_atspi"
                self.assertEqual(record["endpoint_category"], expected)
                self.assertTrue(record["method"] == "modules" and record["success"] is True)
            enabled = [record["enabled"] for record in records if record["operation"] == "set_enabled"]
            self.assertEqual(enabled, [False, True, False] + [True, False] * (len(qualification.MODULES) - 1))
            output = json.loads((evidence / "linux-module-qualification.json").read_text())
            self.assertEqual([module["id"] for module in output["modules"]], list(qualification.MODULES))

    def test_semantic_fixtures_must_match_the_signed_package_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixtures = root / "fixtures"
            semantic = fixtures / "semantic-search"
            semantic.mkdir(parents=True)
            (semantic / "search-models.json").write_text('{"models":[]}', encoding="utf-8")
            package = root / "semantic.cpmodule"
            import zipfile
            with zipfile.ZipFile(package, "w") as archive:
                archive.writestr("assets/search-models.json", '{"models":[{"id":"en","files":[]}]}')
            with self.assertRaisesRegex(ValueError, "differs from the signed package"):
                qualification.semantic_fixture(fixtures, package)


if __name__ == "__main__":
    unittest.main()
