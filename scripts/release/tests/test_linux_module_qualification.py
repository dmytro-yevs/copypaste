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

    def test_each_signed_module_uses_gui_ipc_and_a_real_restart(self):
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
                    "receipt": {}, "manifest": {"id": module_id},
                }
            state = {}
            commands = []
            restarts = []
            records = []

            def response(value):
                return {
                    "id": 992, "ok": True,
                    "data": {"modules": {"json": json.dumps(value)}},
                }

            def fake_rpc(socket_path, operation):
                commands.append((socket_path, operation))
                trace = {
                    "endpoint_category": "gui_owned_unix_socket", "method": "modules",
                    "operation": operation["operation"], "module_id": operation.get("id"),
                    "enabled": operation.get("enabled"), "request_sha256": "a" * 64,
                    "response_sha256": "b" * 64,
                }
                action = operation["operation"]
                if action == "list":
                    return response(list(state.values())), trace
                if action == "install":
                    module_id = next(
                        module for module, package in packages.items()
                        if str(package["package"]) == operation["package_path"]
                    )
                    state[module_id] = {"id": module_id, "enabled": False}
                    return response(state[module_id]), trace
                if action == "set_preferences":
                    return response(None), trace
                if action == "set_enabled":
                    state[operation["id"]]["enabled"] = operation["enabled"]
                    return response(None), trace
                if action == "invoke":
                    if operation["id"] == "copypaste.ocr":
                        return response({"kind": "text", "text": "CopyPaste"}), trace
                    if operation["id"] == "copypaste.semantic-search":
                        return response({"kind": "embeddings", "vectors": [[0.0] * 384]}), trace
                    return response({"kind": "data", "data": {"status": {"configured": False, "signed_in": False}}}), trace
                if action == "remove":
                    del state[operation["id"]]
                    return response(None), trace
                raise AssertionError(operation)

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
                    records.append,
                )
            self.assertEqual(final_socket, "socket-3")
            self.assertEqual(len(restarts), 3)
            self.assertTrue(records)
            self.assertTrue(all(record["endpoint_category"] == "gui_owned_unix_socket" for record in records))
            self.assertTrue(all(record["method"] == "modules" and record["success"] is True for record in records))
            self.assertTrue(all(len(record["request_sha256"]) == 64 and len(record["response_sha256"]) == 64 for record in records))
            lifecycle = [operation["operation"] for _, operation in commands]
            for module_id in qualification.MODULES:
                module_actions = [operation["operation"] for _, operation in commands if operation.get("id") == module_id]
                self.assertIn("set_enabled", module_actions)
                self.assertIn("invoke", module_actions)
                self.assertIn("remove", module_actions)
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
