from copy import deepcopy
from pathlib import Path
import sys
import json
import tempfile
import unittest
import yaml

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from changes import select, verify_results
from policy import (DEFAULT_BRANCH_DISPATCH, RELEASE_ENVIRONMENT, RELEASE_REF_DISPATCH, UPDATER_SIGNER, check,
                    dependencies, queue_errors, release_boundary_errors, signer_errors, validate)
from workflow_yaml import read_workflow


class PolicyTest(unittest.TestCase):
    def setUp(self):
        self.document = {"on": {"pull_request": None}, "permissions": {"contents": "read"},
                         "jobs": {"test": {"runs-on": "ubuntu-24.04", "timeout-minutes": 20,
                                           "steps": [{"uses": "actions/checkout@" + "a" * 40}]}}}

    def check(self, document):
        return validate(document, Path("ci.yml"), Path.cwd())

    def test_accepts_pinned_minimal_workflow(self):
        self.assertEqual(self.check(self.document), [])

    def test_rejects_mutations(self):
        variants = []
        for field, value in [("timeout-minutes", None), ("continue-on-error", "true"), ("needs", "missing")]:
            data = deepcopy(self.document)
            data["jobs"]["test"][field] = value
            variants.append(data)
        data = deepcopy(self.document)
        data["jobs"]["test"]["steps"][0]["uses"] = "actions/checkout@main"
        variants.append(data)
        data = deepcopy(self.document)
        data["permissions"]["contents"] = "write"
        variants.append(data)
        data = deepcopy(self.document)
        data["on"] = {"pull_request_target": None}
        variants.append(data)
        for data in variants:
            with self.subTest(data=data):
                self.assertTrue(self.check(data))

    def test_artifact_upload_requires_bounded_literal_retention(self):
        for value in (None, 0, 91, True, "3", "${{ inputs.days }}"):
            document = deepcopy(self.document)
            document["jobs"]["test"]["steps"] = [{
                "uses": "actions/upload-artifact@" + "a" * 40,
                "with": {"retention-days": value},
            }]
            with self.subTest(value=value):
                self.assertTrue(any("artifact retention" in error for error in self.check(document)))
        document["jobs"]["test"]["steps"][0]["with"]["retention-days"] = 3
        self.assertEqual(self.check(document), [])

    def test_artifact_retention_contract_rejects_longer_diagnostics_and_shorter_baselines(self):
        from unittest.mock import patch
        import policy
        root = Path(__file__).resolve().parents[3]
        read = policy.read_workflow
        for workflow, job, days in (("ci-metrics.yml", "report", 90), ("release.yml", "linux", 3)):
            def modified(path):
                document = read(path)
                if path.name == workflow:
                    for step in document["jobs"][job]["steps"]:
                        if step.get("uses", "").startswith("actions/upload-artifact@"):
                            step["with"]["retention-days"] = days
                return document
            with self.subTest(workflow=workflow, job=job), patch.object(policy, "read_workflow", modified):
                self.assertTrue(any("artifact retention differs" in error for error in check(root)))

    def test_rejects_cycles(self):
        with self.assertRaisesRegex(ValueError, "circular"):
            dependencies({"a": {"needs": "b"}, "b": {"needs": "a"}})

    def test_active_release_producers_cannot_be_cancelled_by_a_new_run(self):
        document = deepcopy(self.document)
        document["concurrency"] = {"group": "production-release-${{ github.sha }}", "cancel-in-progress": False}
        self.assertEqual(validate(document, Path("release.yml"), Path.cwd()), [])
        document["concurrency"]["cancel-in-progress"] = True
        self.assertTrue(any("must not cancel" in error for error in validate(document, Path("release.yml"), Path.cwd())))

    def test_queue_mode_has_full_structural_validation(self):
        for configuration in ({"queue": "max"}, {"queue": "max", "cancel-in-progress": False},
                              {"queue": "single", "cancel-in-progress": True}):
            self.assertEqual(queue_errors(configuration), [])
        for value in ("invalid", 100, True, [], {}, "${{ inputs.queue }}"):
            self.assertTrue(queue_errors({"queue": value}))
        for value in (True, "true", "${{ inputs.cancel }}"):
            self.assertTrue(queue_errors({"queue": "max", "cancel-in-progress": value}))

    def test_reusable_permission_ceiling_is_checked_before_github_startup(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workflows = root / ".github/workflows"
            workflows.mkdir(parents=True)
            (root / "config").mkdir()
            (root / "config/ci-policy.json").write_text(json.dumps({"schema": 1, "required_jobs": {}}))
            caller = {"name": "CI", "on": {"pull_request": None, "merge_group": None}, "permissions": {"contents": "read"},
                      "jobs": {"call": {"uses": "./.github/workflows/reusable.yaml", "permissions": {"actions": "write"}}}}
            child = {"name": "Reusable", "on": {"workflow_call": None}, "permissions": {"contents": "read"},
                     "jobs": {"publish": {"runs-on": "ubuntu-24.04", "timeout-minutes": 5,
                                          "permissions": {"actions": "write"}, "steps": [{"run": "echo fixture"}]}}}
            static = deepcopy(self.document)
            static["name"] = "Policy"
            static["on"]["merge_group"] = None
            for name, data in [("ci.yml", caller), ("ci-configuration.yml", static), ("reusable.yaml", child)]:
                (workflows / name).write_text(yaml.safe_dump(data))
            self.assertEqual(check(root), [])
            del caller["jobs"]["call"]["permissions"]["actions"]
            (workflows / "ci.yml").write_text(yaml.safe_dump(caller))
            self.assertTrue(any("actions: write" in error for error in check(root)))

    def test_change_selection_fails_closed(self):
        for path in ["Cargo.lock", "crates/core/lib.rs", "apps/copypaste_flutter/lib/generated.dart",
                     "scripts/build.sh", "unregistered-platform/file", "../README.md",
                     "config/runtime-resource-budgets.json", "packaging/linux/release-contract.json"]:
            self.assertTrue(select([path])["native"], path)
        self.assertFalse(select(["README.md", "LICENSE-MIT"])["native"])
        self.assertTrue(select(["README.md", "new-file"])["native"])

    def test_compositor_rebuild_tracks_sources_patches_and_toolchain(self):
        for path in (
            "packaging/linux/desktop-integrations/gnome-shell-extension/mutter/patches/bridge.patch",
            "packaging/linux/compositor-runtime/release-contract.json",
            ".github/workflows/compositor-runtime.yml",
            ".flutter-version",
        ):
            self.assertTrue(select([path])["compositor"], path)
        self.assertFalse(select(["crates/copypaste-daemon/src/clipboard/linux.rs"])["compositor"])
        self.assertFalse(select(["apps/copypaste_flutter/lib/platform/permissions/linux_integration.dart"])["compositor"])

    def test_aggregate_accepts_only_declared_compositor_skip(self):
        needs = {"changes": {"result": "success"},
                 "compositor-runtime-metadata": {"result": "skipped"},
                 "compositor-runtime-producer": {"result": "skipped"},
                 "linux-mutter-bridges": {"result": "skipped"},
                 "rust-and-supabase": {"result": "success"}}
        verify_results(needs, True, True, compositor=False)
        needs["linux-mutter-bridges"]["result"] = "success"
        with self.assertRaises(ValueError):
            verify_results(needs, True, True, compositor=False)

    def test_signer_exercise_is_required_exactly_when_the_signer_changes(self):
        for path in ("scripts/release/updater-signer/package-lock.json", "scripts/release/sign-update-artifact.sh"):
            self.assertTrue(select([path])["signer"], path)
        self.assertFalse(select(["scripts/release/publish-release.py"])["signer"])
        needs = {"changes": {"result": "success"}, "updater-signer": {"result": "skipped"}}
        verify_results(needs, True, False, signer=False)
        with self.assertRaises(ValueError):
            verify_results(needs, True, False, signer=True)

    def test_aggregate_rejects_failed_or_skipped_required_jobs(self):
        needs = {"changes": {"result": "success"}, "rust": {"result": "success"}}
        verify_results(needs, True, False)
        for status in ("skipped", "failure", "cancelled"):
            needs["rust"]["result"] = status
            with self.assertRaises(ValueError):
                verify_results(needs, True, False)
        needs["rust"]["result"] = "skipped"
        verify_results(needs, False, False)


class ReleaseBoundaryTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[3]
        self.documents = {path.name: read_workflow(path)
                          for path in sorted((self.root / ".github/workflows").glob("*.yml"))}

    def rejected(self, workflow, job, change):
        documents = deepcopy(self.documents)
        change(documents[workflow]["jobs"][job])
        return [error for error in release_boundary_errors(documents) if error.startswith(f"{workflow}: {job} ")]

    def test_repository_workflows_stay_inside_the_boundary(self):
        self.assertEqual(release_boundary_errors(self.documents), [])
        self.assertEqual(signer_errors(self.root), [])
        self.assertEqual(self.documents["release.yml"]["jobs"]["publish"]["environment"], RELEASE_ENVIRONMENT)
        # Recovery and exact-source staging run from the release tag; the live catalog never does.
        self.assertEqual(self.documents["release.yml"]["jobs"]["preflight"]["if"], RELEASE_REF_DISPATCH)
        self.assertEqual(self.documents["module-marketplace.yml"]["jobs"]["publish"]["if"], DEFAULT_BRANCH_DISPATCH)

    def test_pull_request_runs_receive_no_secret(self):
        key = "${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}"
        for change in (lambda job: job.update(secrets={"TAURI_SIGNING_PRIVATE_KEY": key}),
                       lambda job: job.update(secrets="inherit")):
            self.assertTrue(self.rejected("ci.yml", "compositor-runtime-producer", change))
        self.assertTrue(self.rejected("ci.yml", "changes", lambda job: job["steps"][-1]["env"].update(KEY=key)))
        # The producer that pull requests call holds no secret on any path.
        self.assertTrue(self.rejected("compositor-runtime.yml", "build", lambda job: job.update(env={"KEY": key})))

    def test_pull_request_reachable_calls_cannot_inherit_secrets(self):
        documents = deepcopy(self.documents)
        documents["compositor-runtime.yml"]["jobs"]["nested"] = {
            "uses": "./.github/workflows/secret-scan.yml", "secrets": "inherit"}
        self.assertIn("compositor-runtime.yml: nested would hand release secrets to pull-request code",
                      release_boundary_errors(documents))

    def test_workflow_level_env_cannot_carry_a_secret(self):
        documents = deepcopy(self.documents)
        documents["release.yml"]["env"] = {"KEY": "${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}"}
        self.assertEqual(release_boundary_errors(documents),
                         ["release.yml: workflow-level env would hand a secret to every job"])

    def test_signing_and_publishing_need_the_environment_and_a_trusted_dispatch_ref(self):
        for workflow, job in (("release.yml", "macos"), ("release.yml", "publish"),
                              ("provider-module.yml", "desktop"), ("module-marketplace.yml", "publish")):
            with self.subTest(workflow=workflow, job=job):
                self.assertTrue(self.rejected(workflow, job, lambda job: job.pop("environment")))
        documents = deepcopy(self.documents)
        del documents["release.yml"]["jobs"]["preflight"]["if"]
        exposed = {error.split(" ")[1] for error in release_boundary_errors(documents) if "manual run" in error}
        self.assertLessEqual({"preflight", "macos", "qualification-ocr", "compositor-runtime-public", "publish"},
                             exposed)
        # Neither a status function nor an alternative may let a publisher outlive the refusal.
        self.assertTrue(self.rejected("release.yml", "publish", lambda job: job.update({"if": "${{ always() }}"})))
        self.assertTrue(self.rejected("module-marketplace.yml", "publish", lambda job: job.update(
            {"if": DEFAULT_BRANCH_DISPATCH + " || inputs.module_release_tag != ''"})))
        # Only v* release tags join the default branch, and never for the live catalog.
        for workflow, job, condition in (
                ("release.yml", "preflight", DEFAULT_BRANCH_DISPATCH + " || startsWith(github.ref, 'refs/tags/')"),
                ("release.yml", "preflight", DEFAULT_BRANCH_DISPATCH + " || startsWith(github.ref, 'refs/heads/')"),
                ("module-marketplace.yml", "publish", RELEASE_REF_DISPATCH)):
            with self.subTest(workflow=workflow, condition=condition):
                self.assertTrue(self.rejected(workflow, job, lambda job: job.update({"if": condition})))

    def test_updater_signer_comes_only_from_the_integrity_pinned_lockfile(self):
        script = "scripts/release/sign-update-artifact.sh"
        manifest, lock = UPDATER_SIGNER + "/package.json", UPDATER_SIGNER + "/package-lock.json"

        def locked(change):
            def apply(text):
                data = json.loads(text)
                change(next(entry for name, entry in data["packages"].items() if name))
                return json.dumps(data)
            return apply

        for name, change in ((script, lambda text: text.replace("npm ci --ignore-scripts", "npm ci")),
                             (script, lambda text: text + "npx --yes @tauri-apps/cli signer sign\n"),
                             (manifest, lambda text: text.replace('"2.', '"^2.')),
                             (lock, locked(lambda entry: entry.pop("integrity"))),
                             (lock, locked(lambda entry: entry.update(hasInstallScript=True)))):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                for path in (script, manifest, lock):
                    text = (self.root / path).read_text(encoding="utf-8")
                    (root / path).parent.mkdir(parents=True, exist_ok=True)
                    (root / path).write_text(change(text) if path == name else text, encoding="utf-8")
                self.assertTrue(signer_errors(root))


if __name__ == "__main__":
    unittest.main()
