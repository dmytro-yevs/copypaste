from copy import deepcopy
from pathlib import Path
import runpy
import sys
import hashlib
import json
import tempfile
import os
import subprocess
from types import SimpleNamespace
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts/release"))


def load(name):
    return runpy.run_path(str(ROOT / "scripts/release" / name))


class ReleasePipelineTest(unittest.TestCase):
    def setUp(self):
        self.run = {"id": 123, "path": ".github/workflows/release.yml", "event": "workflow_dispatch",
                    "status": "in_progress", "conclusion": None, "head_sha": "a" * 40,
                    "head_repository": {"full_name": "owner/repo"}}

    def jobs(self, names):
        return {"total_count": len(names), "jobs": [{"name": name, "status": "completed", "conclusion": "success"} for name in names]}

    def artifact(self, name):
        return {"total_count": 1, "artifacts": [{"id": 1, "name": name, "expired": False,
                "workflow_run": {"id": 123, "head_sha": "a" * 40}}]}

    def test_current_artifact_requires_successful_builds_and_same_run(self):
        verify = load("verify-linux-artifact-source-run.py")["verify"]
        jobs = self.jobs(["preflight", "Linux build (x86_64)"])
        artifacts = self.artifact("production-linux-x86_64")
        verify(self.run, jobs, artifacts, "owner/repo", "a" * 40, current_run_id="123")
        verify({**self.run, "status": "queued"}, jobs, artifacts, "owner/repo", "a" * 40, current_run_id="123")
        for current in (None, "124"):
            with self.assertRaises(ValueError):
                verify(self.run, jobs, artifacts, "owner/repo", "a" * 40, current_run_id=current)
        for status in ("failure", "skipped", "cancelled"):
            bad = deepcopy(jobs)
            bad["jobs"][-1]["conclusion"] = status
            with self.assertRaises(ValueError):
                verify(self.run, bad, artifacts, "owner/repo", "a" * 40, current_run_id="123")

    def test_current_module_requires_its_own_producer_and_exact_provenance(self):
        verify = load("verify-linux-module-source-run.py")["verify"]
        jobs = self.jobs(["Build Linux supabase module / preflight", "Build Linux supabase module / desktop (linux, x86_64, ubuntu-24.04)"])
        artifacts = self.artifact("provider-supabase-linux-x86_64")
        args = [self.run, jobs, artifacts, "owner/repo", "a" * 40, ".github/workflows/provider-module.yml", "supabase"]
        verify(*args, current_run_id="123")
        for index, mutation in [(0, {**self.run, "head_sha": "b" * 40}), (1, self.jobs(["unrelated linux x86_64"])),
                                (2, self.artifact("provider-supabase-linux-aarch64"))]:
            bad = deepcopy(args)
            bad[index] = mutation
            with self.assertRaises(ValueError):
                verify(*bad, current_run_id="123")
        artifacts["artifacts"][0]["expired"] = True
        with self.assertRaises(ValueError):
            verify(*args, current_run_id="123")

    def test_current_evidence_requires_complete_native_matrix(self):
        module = load("verify-linux-qualification-evidence-source.py")
        jobs = self.jobs(["Qualify current Linux artifacts / " + name for name in module["BASE"]])
        module["verify"](self.run, jobs, "owner/repo", "a" * 40, current_run_id="123")
        jobs["jobs"][-1]["conclusion"] = "skipped"
        with self.assertRaises(ValueError):
            module["verify"](self.run, jobs, "owner/repo", "a" * 40, current_run_id="123")

    def test_current_compositor_requires_its_finished_producer(self):
        module = load("verify-linux-compositor-runtime.py")
        artifacts = self.artifact("copypaste-compositor-runtime-gnome-46-ubuntu24.04-x86_64")
        jobs = self.jobs(["Build opt-in compositor runtime companions / GNOME 46 Ubuntu 24.04 (x86_64)"])
        module["verify_source"](self.run, artifacts, "owner/repo", "a" * 40, current_run_id="123", jobs=jobs)
        with self.assertRaises(ValueError):
            module["verify_source"](self.run, artifacts, "owner/repo", "a" * 40, current_run_id="123", jobs=self.jobs([]))

    def test_preflight_missing_secrets_fails_without_exposing_values(self):
        require = load("preflight-sources.py")["require_secrets"]
        require({}, build=False, publish=False)
        for build, publish in [(True, False), (False, True)]:
            with self.assertRaisesRegex(ValueError, "Missing release"):
                require({}, build=build, publish=publish)

    def test_external_compositor_source_is_rejected_during_preflight(self):
        main = load("preflight-sources.py")["main"]
        with tempfile.TemporaryDirectory() as directory:
            environment = {"BUILD": "false", "PUBLISH": "false", "GITHUB_REPOSITORY": "owner/repo",
                           "RELEASE_COMMIT": "a" * 40, "COMPOSITOR_RUNTIME_RUN_ID": "123",
                           "RUNNER_TEMP": directory, "GITHUB_OUTPUT": str(Path(directory) / "outputs")}
            with patch.dict(os.environ, environment, clear=True), patch.dict(main.__globals__, {"inventory": lambda *args: {}}):
                with self.assertRaisesRegex(ValueError, "compositor runtime source"):
                    main()

    def test_tag_baseline_fallback_uses_the_existing_prior_source_verifier(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn(
            "PREVIOUS_LINUX_ARTIFACT_RUN_ID: ${{ inputs.previous_linux_artifact_run_id || (github.ref_type == 'tag' && vars.PREVIOUS_LINUX_ARTIFACT_RUN_ID) || '' }}",
            workflow,
        )
        main = load("preflight-sources.py")["main"]
        source_run = {"head_sha": "b" * 40}
        verified = []

        def inventory(_repository, run_id, kind):
            self.assertEqual(run_id, "123")
            return source_run if kind == "run" else {"kind": kind}

        def run_path(path):
            self.assertTrue(str(path).endswith("verify-linux-artifact-source-run.py"))
            return {"verify": lambda *args: verified.append(args)}

        with tempfile.TemporaryDirectory() as directory:
            environment = {
                "BUILD": "false", "PUBLISH": "false", "GITHUB_REPOSITORY": "owner/repo",
                "RELEASE_COMMIT": "a" * 40, "PREVIOUS_LINUX_ARTIFACT_RUN_ID": "123",
                "RUNNER_TEMP": directory, "GITHUB_OUTPUT": str(Path(directory) / "outputs"),
            }
            with patch.dict(os.environ, environment, clear=True), patch.dict(
                main.__globals__, {"inventory": inventory, "runpy": SimpleNamespace(run_path=run_path)}
            ):
                main()
        self.assertEqual(verified, [
            (source_run, {"kind": "jobs"}, {"kind": "artifacts"}, "owner/repo", "b" * 40),
        ])

    def test_tag_baseline_fallback_rejects_an_invalid_prior_source(self):
        main = load("preflight-sources.py")["main"]

        def inventory(_repository, _run_id, kind):
            return {"head_sha": "b" * 40} if kind == "run" else {"kind": kind}

        with tempfile.TemporaryDirectory() as directory:
            environment = {
                "BUILD": "false", "PUBLISH": "false", "GITHUB_REPOSITORY": "owner/repo",
                "RELEASE_COMMIT": "a" * 40, "PREVIOUS_LINUX_ARTIFACT_RUN_ID": "123",
                "RUNNER_TEMP": directory, "GITHUB_OUTPUT": str(Path(directory) / "outputs"),
            }
            rejected = SimpleNamespace(run_path=lambda _path: {"verify": lambda *_args: (_ for _ in ()).throw(ValueError("rejected"))})
            with patch.dict(os.environ, environment, clear=True), patch.dict(
                main.__globals__, {"inventory": inventory, "runpy": rejected}
            ), self.assertRaisesRegex(ValueError, "rejected"):
                main()

    def test_partial_module_matrix_can_never_publish(self):
        matrix = runpy.run_path(str(ROOT / "scripts/ci/module-matrix.py"))["matrix"]
        contract = {"schema": 1, "desktop": {"provider": [{"platform": "linux", "architecture": "x86_64"}, {"platform": "macos", "architecture": "aarch64"}]}}
        self.assertEqual(len(matrix(contract, "provider", True, False)), 1)
        self.assertEqual(len(matrix(contract, "provider", False, True)), 2)
        with self.assertRaises(ValueError):
            matrix(contract, "provider", True, True)

    def test_fresh_release_qualifies_complete_publishable_module_matrices(self):
        from workflow_yaml import read_workflow
        workflow = read_workflow(ROOT / ".github/workflows/release.yml")
        for module in ("supabase", "semantic-search", "ocr"):
            with self.subTest(module=module):
                producer = workflow["jobs"]["qualification-" + module]
                self.assertIs(producer["with"]["linux_only"], False)
                self.assertIs(producer["with"]["publish"], False)
                self.assertIn("qualification-" + module, workflow["jobs"]["linux-native-current"]["needs"])
        sms = workflow["jobs"]["qualification-sms-codes"]
        self.assertEqual(sms["uses"], "./.github/workflows/sms-codes-module.yml")
        self.assertIs(sms["with"]["publish"], False)
        self.assertIn("qualification-sms-codes", workflow["jobs"]["qualify"]["needs"])
        self.assertIn("needs.qualification-sms-codes.result == 'success'", workflow["jobs"]["qualify"]["if"])

    def test_publication_stays_draft_until_every_asset_matches(self):
        publish = load("publish-release.py")["publish"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "package.deb").write_bytes(b"qualified bytes")
            assets = [{"name": "package.deb", "size": 15,
                       "digest": "sha256:" + hashlib.sha256(b"qualified bytes").hexdigest()}]
            draft = {"id": 42, "tag_name": "v1.2.3", "draft": True, "prerelease": False, "assets": assets}
            stable = {**draft, "draft": False}
            args = SimpleNamespace(repository="owner/repo", version="1.2.3", artifacts=root, notes=Path("notes.md"))
            with patch("subprocess.check_output", side_effect=["[[]]", json.dumps([[draft]]), json.dumps(draft), json.dumps(stable)]), patch("subprocess.run") as execute:
                publish(args)
                self.assertIn("--draft", execute.call_args_list[0].args[0])
                self.assertIn("--draft=false", execute.call_args_list[1].args[0])
            incomplete = {**draft, "assets": []}
            with patch("subprocess.check_output", side_effect=["[[]]", json.dumps([[incomplete]]), json.dumps(incomplete)]), patch("subprocess.run") as execute:
                with self.assertRaisesRegex(ValueError, "incomplete"):
                    publish(args)
                self.assertEqual(execute.call_count, 1)
            # A crashed upload resumes the draft; a repeated completed publication is a no-op.
            with patch("subprocess.check_output", side_effect=[json.dumps([[incomplete]]), json.dumps(draft), json.dumps(stable)]), patch("subprocess.run") as execute:
                publish(args)
                self.assertEqual(execute.call_args_list[0].args[0][1:3], ["release", "upload"])
            with patch("subprocess.check_output", side_effect=[json.dumps([[stable]]), json.dumps(stable)]), patch("subprocess.run") as execute:
                publish(args)
                execute.assert_not_called()

    def test_draft_promotion_survives_the_published_tag_endpoints_404(self):
        publish = load("publish-release.py")["publish"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload = b"qualified bytes"
            (root / "package.deb").write_bytes(payload)
            state = {"release": None}
            reads = []

            def read(command, **kwargs):
                endpoint = command[-1]
                reads.append(endpoint)
                if "/releases/tags/" in endpoint:
                    raise subprocess.CalledProcessError(1, command, stderr="HTTP 404")
                if endpoint.endswith("/releases?per_page=100"):
                    return json.dumps([[state["release"]] if state["release"] else []])
                self.assertEqual(endpoint, "repos/owner/repo/releases/42")
                return json.dumps(state["release"])

            def execute(command, **kwargs):
                if command[1:3] == ["release", "create"]:
                    self.assertIn("--draft", command)
                    state["release"] = {"id": 42, "tag_name": "v1.2.3", "draft": True,
                        "prerelease": False, "assets": [{"name": "package.deb", "size": len(payload),
                        "digest": "sha256:" + hashlib.sha256(payload).hexdigest()}]}
                else:
                    self.assertEqual(command[1:3], ["release", "edit"])
                    self.assertIn("--draft=false", command)
                    self.assertIn("repos/owner/repo/releases/42", reads)
                    state["release"]["draft"] = False

            args = SimpleNamespace(repository="owner/repo", version="1.2.3", artifacts=root, notes=Path("notes.md"))
            with patch("subprocess.check_output", side_effect=read), patch("subprocess.run", side_effect=execute):
                publish(args)
            self.assertFalse(state["release"]["draft"])
            self.assertFalse(any("/releases/tags/" in endpoint for endpoint in reads))

    def test_recovery_accepts_only_isolated_publisher_failure(self):
        verify = load("verify-qualified-run.py")["verify"]
        run = {**self.run, "status": "completed", "conclusion": "failure"}
        jobs = self.jobs(["preflight", "macos", "macos-regression", "android", "windows", "qualify", "publish"])
        jobs["jobs"][-1]["conclusion"] = "failure"
        artifacts = self.artifact("production-qualified")
        verify(run, jobs, artifacts, "owner/repo", "a" * 40)
        cancelled = deepcopy(jobs)
        cancelled["jobs"][-1]["conclusion"] = "cancelled"
        verify({**run, "conclusion": "cancelled"}, cancelled, artifacts, "owner/repo", "a" * 40)
        cancelled["jobs"][-1]["conclusion"] = "skipped"
        verify({**run, "conclusion": "cancelled"}, cancelled, artifacts, "owner/repo", "a" * 40)
        jobs["jobs"][2]["conclusion"] = "skipped"
        with self.assertRaises(ValueError):
            verify(run, jobs, artifacts, "owner/repo", "a" * 40)

    def test_discovery_requires_native_qualification_and_unexpired_exact_source(self):
        eligible = load("qualified_source.py")["eligible"]
        run = {**self.run, "status": "completed", "conclusion": "success"}
        jobs = self.jobs(["preflight", "macos", "android", "windows", "qualify",
                          "Linux full parity evidence gate", "Sign and bind opt-in compositor runtime companions",
                          "Build SMS Codes module / android-native"])
        artifacts = self.artifact("production-qualified")
        artifacts["artifacts"].extend({
            "name": f"sms-codes-android-{target}", "expired": False,
            "workflow_run": {"id": 123, "head_sha": "a" * 40},
        } for target in ("arm", "aarch64", "x86_64", "native"))
        artifacts["total_count"] = len(artifacts["artifacts"])
        eligible(run, jobs, artifacts, "owner/repo", "a" * 40, linux=True, compositor=True)
        for name in ("qualify", "Linux full parity evidence gate", "Sign and bind opt-in compositor runtime companions",
                     "Build SMS Codes module / android-native"):
            bad = deepcopy(jobs)
            next(job for job in bad["jobs"] if job["name"] == name)["conclusion"] = "skipped"
            with self.assertRaises(ValueError):
                eligible(run, bad, artifacts, "owner/repo", "a" * 40, linux=True, compositor=True)
        artifacts["artifacts"][0]["expired"] = True
        with self.assertRaises(ValueError):
            eligible(run, jobs, artifacts, "owner/repo", "a" * 40, linux=True, compositor=True)

    def test_compositor_receipt_reads_inputs_as_environment_data(self):
        from workflow_yaml import read_workflow
        workflow = read_workflow(ROOT / ".github/workflows/compositor-runtime.yml")
        step = next(step for step in workflow["jobs"]["build"]["steps"]
                    if step.get("name") == "Sign and bind immutable producer files")
        program = step["run"].split("<<'PY'\n", 1)[1].split("\nPY", 1)[0]
        for name, value in workflow["jobs"]["build"]["strategy"]["matrix"]["include"][0].items():
            program = program.replace("${{ matrix." + name + " }}", str(value))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "artifact").mkdir()
            (root / "package.deb").write_bytes(b"signed package")
            (root / "runtime.json").write_text(json.dumps({"upstream_licenses": [], "runtime_id": "test",
                "glibc_floor": "2.39", "source": {"revision": "test"}}))
            subprocess.run([sys.executable, "-", "package.deb", "runtime.json", "receipt.json"], input=program,
                text=True, cwd=root, check=True, env={**os.environ, "GITHUB_RUN_ID": "123",
                "CI_INPUT_SOURCE_RUN_ID": "456", "CI_INPUT_VERSION": "1.2.3", "COMPOSITOR_RUNTIME_COMMIT": "a" * 40})
            receipt = json.loads((root / "receipt.json").read_text())
            self.assertEqual(receipt["source_run_id"], 456)
            self.assertEqual(receipt["version"], "1.2.3")

    def test_producer_identity_checks_checkout_sha_and_rejects_input_injection(self):
        from workflow_yaml import read_workflow
        workflow = read_workflow(ROOT / ".github/workflows/compositor-runtime.yml")
        step = next(step for step in workflow["jobs"]["build"]["steps"] if step.get("name") == "Check immutable producer identity")
        commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        environment = {**os.environ, "CI_INPUT_VERSION": "1.2.3", "CI_INPUT_SOURCE_RUN_ID": "123", "GITHUB_SHA": "b" * 40}
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "injected"
            for supplied, expected in [(commit, 0), ("b" * 40, 1), (f"$(touch {marker})", 1)]:
                result = subprocess.run(["bash", "-c", step["run"]], cwd=ROOT, text=True, capture_output=True,
                                        env={**environment, "CI_INPUT_COMMIT": supplied})
                self.assertEqual(result.returncode, expected)
            self.assertFalse(marker.exists())

    def test_build_identity_is_toolchain_profile_and_flag_sensitive_without_secrets(self):
        metadata = runpy.run_path(str(ROOT / "scripts/ci/build-metadata.py"))["metadata"]
        environment = {"BUILD_PLATFORM": "android", "BUILD_PROFILE": "release", "RUNNER_ARCH": "X64",
                       "ANDROID_KEYSTORE_PASSWORD": "must-not-appear"}
        flutter = {"frameworkVersion": (ROOT / ".flutter-version").read_text().strip(),
                   "frameworkRevision": "a" * 40, "dartSdkVersion": "pinned"}
        original = metadata(ROOT, environment, "rustc 1.96.0", flutter)
        self.assertEqual(original["target_architectures"], ["armv7", "arm64", "x86_64"])
        self.assertNotIn("must-not-appear", json.dumps(original))
        for change in ({"BUILD_PROFILE": "debug"}, {"RUSTFLAGS": "-C opt-level=1"}):
            self.assertNotEqual(original["fingerprint"], metadata(ROOT, {**environment, **change}, "rustc 1.96.0", flutter)["fingerprint"])
        self.assertNotEqual(original["fingerprint"], metadata(ROOT, environment, "rustc 1.96.1", flutter)["fingerprint"])
        with self.assertRaisesRegex(ValueError, "Flutter SDK"):
            metadata(ROOT, environment, "rustc 1.96.0", {**flutter, "frameworkVersion": "0.0.0"})

    def test_fresh_flutter_bootstrap_progress_does_not_corrupt_machine_metadata(self):
        version = runpy.run_path(str(ROOT / "scripts/ci/build-metadata.py"))["flutter_version"]
        state = {"initialized": False}
        def bootstrap(command, **kwargs):
            if "--version" in command and "--machine" not in command:
                state["initialized"] = True
        def machine(command, **kwargs):
            progress = "" if state["initialized"] else "Resolving dependencies...\n"
            return progress + '{"frameworkVersion":"3.47.6"}'
        with patch("subprocess.run", side_effect=bootstrap), patch("subprocess.check_output", side_effect=machine):
            self.assertEqual(version("flutter")["frameworkVersion"], "3.47.6")

    def test_complete_inventory_rejects_unqualified_files_and_changed_signatures(self):
        verify = load("verify-artifact-receipts.py")["verify"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for platform, names in {
                "macos": ["app.dmg", "cli.tar.gz"], "windows": ["app.exe"],
                "android": ["CopyPaste-v1.2.3-android" + suffix + ".apk" for suffix in ("", "-arm64", "-armv7")],
            }.items():
                target = root / platform
                target.mkdir()
                command = [sys.executable, str(ROOT / "scripts/release/write-artifact-receipt.py"),
                           "--platform", platform, "--version", "1.2.3", "--commit", "a" * 40,
                           "--run-id", "123", "--output", str(target / "production-receipt.json")]
                for name in names:
                    path = target / name
                    path.write_bytes(b"qualified")
                    Path(str(path) + ".sig").write_bytes(b"signature verified by the platform gate")
                    command += ["--artifact", str(path)]
                subprocess.run(command, check=True)
            verify(root, "1.2.3", "a" * 40, "123")
            extra = root / "macos/unqualified.tar.gz"
            extra.write_bytes(b"not qualified")
            with self.assertRaisesRegex(ValueError, "unqualified"):
                verify(root, "1.2.3", "a" * 40, "123")
            extra.unlink()
            (root / "windows/app.exe.sig").write_bytes(b"replaced signature")
            with self.assertRaisesRegex(ValueError, "companion artifact"):
                verify(root, "1.2.3", "a" * 40, "123")


if __name__ == "__main__":
    unittest.main()
