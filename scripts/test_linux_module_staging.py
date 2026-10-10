#!/usr/bin/env python3
import argparse
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]


def load(name):
    path = ROOT / "scripts/release" / name
    spec = importlib.util.spec_from_file_location(path.stem.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class LinuxModuleSourceRunTest(unittest.TestCase):
    def setUp(self):
        self.verify = load("verify-linux-module-source-run.py").verify
        self.run = {
            "id": 123, "head_sha": "a" * 40,
            "head_repository": {"full_name": "owner/repo"},
            "path": ".github/workflows/provider-module.yml", "event": "workflow_dispatch",
            "status": "completed", "conclusion": "success",
        }
        self.jobs = {"total_count": 2, "jobs": [
            {"name": "desktop (linux, x86_64)", "status": "completed", "conclusion": "success"},
            {"name": "desktop (linux, aarch64)", "status": "completed", "conclusion": "success"},
        ]}
        self.artifacts = {"artifacts": [
            {"name": f"provider-supabase-linux-{architecture}", "expired": False,
             "workflow_run": {"id": 123, "head_sha": "a" * 40}}
            for architecture in ("x86_64", "aarch64")
        ]}

    def test_requires_exact_successful_linux_module_source(self):
        self.verify(self.run, self.jobs, self.artifacts, "owner/repo", "a" * 40,
                    ".github/workflows/provider-module.yml", "supabase")
        self.jobs["jobs"][1]["conclusion"] = "failure"
        with self.assertRaisesRegex(ValueError, "Linux architecture job"):
            self.verify(self.run, self.jobs, self.artifacts, "owner/repo", "a" * 40,
                        ".github/workflows/provider-module.yml", "supabase")

    def test_refuses_expired_or_unbound_artifact(self):
        self.artifacts["artifacts"][0]["expired"] = True
        with self.assertRaisesRegex(ValueError, "provenance"):
            self.verify(self.run, self.jobs, self.artifacts, "owner/repo", "a" * 40,
                        ".github/workflows/provider-module.yml", "supabase")


class LinuxModuleStagingTest(unittest.TestCase):
    def setUp(self):
        self.module = load("stage-linux-module-qualification.py")
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.downloads = self.root / "downloads"
        self.models = b"pinned English model"
        self.tokenizer = b"pinned tokenizer"
        self.write_source()
        self.write_downloads()

    def write(self, path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value if isinstance(value, bytes) else value.encode("utf-8"))

    def write_source(self):
        for name, module_id in self.module.MODULES.items():
            self.write(self.source / "modules" / name / "module.json", json.dumps({
                "id": module_id, "version": "0.1.1", "app_versions": ">=1.0.24, <2.0.0",
            }))
        fixtures = self.source / "scripts/modules/fixtures"
        self.write(fixtures / "fixtures.json", json.dumps([
            {"file": "english.png", "expected": "English"},
            {"file": "separate.png", "expected": "Ukrainian"},
            {"file": "mixed.png", "expected": "Mixed"},
        ]))
        for name in ("english.png", "separate.png", "mixed.png"):
            self.write(fixtures / name, name.encode())
        self.write(self.source / "modules/semantic-search/assets/search-models.json", json.dumps({"models": [{
            "id": "minilm-en-v1", "languages": ["en"], "files": [
                {"path": "model.onnx", "url": "https://fixtures.invalid/model", "sha256": hashlib.sha256(self.models).hexdigest(), "size_bytes": len(self.models)},
                {"path": "tokenizer.json", "url": "https://fixtures.invalid/tokenizer", "sha256": hashlib.sha256(self.tokenizer).hexdigest(), "size_bytes": len(self.tokenizer)},
            ],
        }]}))

    def write_downloads(self):
        self.packages = {}
        for name, module_id in self.module.MODULES.items():
            package = self.downloads / name / self.module.package_filename(module_id, "0.1.1", "x86_64")
            self.write(package, (name + " package").encode())
            self.packages[name] = package
            receipt = {
                "commit": "a" * 40, "run_id": str({"supabase": 101, "semantic-search": 102, "ocr": 103}[name]),
                "module_id": module_id, "module_version": "0.1.1", "app_version": "1.0.24",
                "target": {"platform": "linux", "architecture": "x86_64"},
                "package_sha256": hashlib.sha256(package.read_bytes()).hexdigest(),
                "package_size_bytes": package.stat().st_size, "signature_verified": True,
                "cases_passed": 3, "removal_completed_after_restart": True,
                "restart_required": name == "ocr",
            }
            self.write(package.with_name(package.name + ".receipt.json"), json.dumps(receipt))

    def arguments(self):
        return argparse.Namespace(
            source_root=self.source, downloads=self.downloads,
            module_artifacts=self.root / "module-artifacts", module_fixtures=self.root / "module-fixtures",
            architecture="x86_64", app_version="1.0.24", commit="a" * 40,
            supabase_run_id="101", semantic_search_run_id="102", ocr_run_id="103",
        )

    def verified_package(self, path):
        name = next(name for name, package in self.packages.items() if package.resolve() == path.resolve())
        return SimpleNamespace(manifest={
            "id": self.module.MODULES[name], "version": "0.1.1",
            "app_versions": ">=1.0.24, <2.0.0",
            "target": {"platform": "linux", "architecture": "x86_64"},
        })

    def response(self, url, timeout):
        return io.BytesIO(self.models if url.endswith("/model") else self.tokenizer)

    def test_stages_exact_packages_and_minimal_offline_fixtures(self):
        args = self.arguments()
        with mock.patch.object(self.module.catalog, "read_package", side_effect=self.verified_package), \
             mock.patch.object(self.module, "urlopen", side_effect=self.response):
            self.module.stage(args)
        self.assertEqual(sorted(path.name for path in args.module_artifacts.glob("*.cpmodule")), sorted(path.name for path in self.packages.values()))
        self.assertTrue((args.module_fixtures / "ocr/fixtures.json").is_file())
        self.assertEqual((args.module_fixtures / "semantic-search/minilm-en-v1/model.onnx").read_bytes(), self.models)
        self.assertFalse((args.module_fixtures / "semantic-search/e5-multilingual-v1").exists())

    def test_refuses_tampered_receipt_and_cleans_partial_staging(self):
        receipt = self.packages["ocr"].with_name(self.packages["ocr"].name + ".receipt.json")
        value = json.loads(receipt.read_text())
        value["package_sha256"] = "bad"
        receipt.write_text(json.dumps(value), encoding="utf-8")
        args = self.arguments()
        with mock.patch.object(self.module.catalog, "read_package", side_effect=self.verified_package), \
             self.assertRaisesRegex(ValueError, "does not bind"):
            self.module.stage(args)
        self.assertFalse(args.module_artifacts.exists())
        self.assertFalse(args.module_fixtures.exists())


class LinuxModuleWorkflowContractTest(unittest.TestCase):
    def test_workflow_downloads_and_mounts_arch_specific_authenticated_modules(self):
        workflow = (ROOT / ".github/workflows/linux-native-qualification.yml").read_text(encoding="utf-8")
        for name in ("supabase_module_run_id", "semantic_search_module_run_id", "ocr_module_run_id"):
            self.assertIn(name + ":", workflow)
        for name in ("provider-supabase-linux-${{ matrix.architecture }}", "provider-semantic-search-linux-${{ matrix.architecture }}", "ocr-linux-${{ matrix.architecture }}"):
            self.assertIn(name, workflow)
        self.assertIn("stage-linux-module-qualification.py", workflow)
        fedora = (ROOT / "scripts/release/run-fedora-plasma6-qualification.sh").read_text(encoding="utf-8")
        self.assertIn(":/module-artifacts:ro", fedora)
        self.assertIn(":/module-fixtures:ro", fedora)


if __name__ == "__main__":
    unittest.main()
