#!/usr/bin/env python3
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]


def load():
    path = ROOT / "scripts/release/verify-linux-compositor-runtime.py"
    spec = importlib.util.spec_from_file_location("verify_linux_compositor_runtime", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class CompositorRuntimeSourceTest(unittest.TestCase):
    def setUp(self):
        self.module = load()
        self.run = {
            "id": 420, "head_sha": "a" * 40, "head_repository": {"full_name": "owner/repo"},
            "path": ".github/workflows/compositor-runtime.yml", "event": "workflow_dispatch",
            "status": "completed", "conclusion": "success",
        }
        self.artifacts = {"total_count": 4, "artifacts": [
            {"name": self.module.artifact_name(desktop, architecture), "expired": False,
             "workflow_run": {"id": 420, "head_sha": "a" * 40}}
            for desktop, architecture in self.module.RUNTIMES
        ]}

    def test_accepts_every_exact_baseline_runtime_artifact(self):
        self.module.verify_source(self.run, self.artifacts, "owner/repo", "a" * 40)

    def test_rejects_partial_and_duplicate_runtime_inventory(self):
        partial = copy.deepcopy(self.artifacts)
        partial["total_count"] = 5
        with self.assertRaisesRegex(ValueError, "inventory is incomplete"):
            self.module.verify_source(self.run, partial, "owner/repo", "a" * 40)
        duplicate = copy.deepcopy(self.artifacts)
        duplicate["artifacts"].append(copy.deepcopy(duplicate["artifacts"][0]))
        duplicate["total_count"] += 1
        with self.assertRaisesRegex(ValueError, "exactly one artifact"):
            self.module.verify_source(self.run, duplicate, "owner/repo", "a" * 40)

    def test_accepts_only_a_same_repository_ci_producer_run(self):
        run = copy.deepcopy(self.run)
        run.update({"path": ".github/workflows/ci.yml", "event": "pull_request", "pull_requests": [{"number": 7}]})
        self.module.verify_source(run, self.artifacts, "owner/repo", "a" * 40)
        run["head_sha"] = "b" * 40
        with self.assertRaisesRegex(ValueError, "exact-commit"):
            self.module.verify_source(run, self.artifacts, "owner/repo", "a" * 40)
        run["head_sha"] = "a" * 40
        run["head_repository"] = {"full_name": "fork/repo"}
        with self.assertRaisesRegex(ValueError, "exact-commit"):
            self.module.verify_source(run, self.artifacts, "owner/repo", "a" * 40)

    def test_requires_checked_out_baseline_revision_and_patch_bytes(self):
        revision, shell_revision, patch = self.module.BASELINES["GNOME"]
        source = {"revision": revision, "shell_revision": shell_revision, "patch_sha256": self.module.sha256(ROOT / patch)}
        self.module.verify_baseline_source("GNOME", source)
        source["patch_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "baseline"):
            self.module.verify_baseline_source("GNOME", source)


class InstalledRuntimeTest(unittest.TestCase):
    def setUp(self):
        self.module = load()
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binding = {"schema": 1, "producer_run_id": "7", "commit": "a" * 40,
                        "runtime_id": "kwin-6.0-fedora40", "desktop": "KDE", "architecture": "x86_64",
                        "distribution": {"id": "fedora", "version": "40"}, "format": "rpm",
                        "package": {"name": "sidecar.rpm", "sha256": "a" * 64, "size_bytes": 1},
                        "runtime_receipt": {"name": "runtime-receipt.json", "sha256": "", "size_bytes": 2}}
        self.receipt = {"runtime_id": self.binding["runtime_id"], "desktop": "KDE", "architecture": "x86_64", "distribution": self.binding["distribution"]}
        receipt = self.root / "usr/share/copypaste/compositor-runtime/kwin-6.0-fedora40.receipt.json"
        receipt.parent.mkdir(parents=True)
        receipt.write_text("{}", encoding="utf-8")
        self.binding["runtime_receipt"]["sha256"] = self.module.sha256(receipt)
        runtime = self.root / "usr/lib/copypaste/compositor-runtime/kwin-6.0-fedora40"
        runtime.mkdir(parents=True)
        launcher = self.root / "usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-kwin-6.0-fedora40"
        launcher.parent.mkdir(parents=True)
        launcher.write_text("expected launcher\n", encoding="utf-8")
        launcher.chmod(0o755)
        descriptor = self.root / "usr/share/wayland-sessions/copypaste-kwin-6.0-fedora40.desktop"
        descriptor.parent.mkdir(parents=True)
        descriptor.write_text(self.module.session_descriptor_text(self.receipt), encoding="utf-8")
        self.stage = SimpleNamespace(read_receipt=lambda path: self.receipt, validate_receipt=lambda receipt: None,
                                     validate_payload=lambda runtime, receipt: None,
                                     launcher_text=lambda receipt, prefix: "expected launcher\n")

    def verify(self):
        with mock.patch.object(self.module, "stage_runtime_module", return_value=self.stage):
            self.module.verify_installed(self.binding, self.root)

    def test_rejects_stock_kwin_and_substituted_gnome_launchers(self):
        self.verify()
        launcher = self.root / "usr/lib/copypaste/compositor-runtime/bin/copypaste-compositor-session-kwin-6.0-fedora40"
        launcher.write_text("#!/bin/sh\nexec kwin_wayland --virtual\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "deterministic launcher"):
            self.verify()
        launcher.write_text("expected launcher\n", encoding="utf-8")
        descriptor = self.root / "usr/share/wayland-sessions/copypaste-kwin-6.0-fedora40.desktop"
        descriptor.write_text("[Desktop Entry]\nExec=/usr/bin/gnome-shell\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "deterministic descriptor"):
            self.verify()


class CompositorRuntimeWorkflowTest(unittest.TestCase):
    def test_producer_provisions_its_gnome_source_dependencies(self):
        workflow = (ROOT / ".github/workflows/compositor-runtime.yml").read_text(encoding="utf-8")
        self.assertIn("Types: deb-src", workflow)
        self.assertIn("apt-get build-dep --yes --no-install-recommends mutter gnome-shell", workflow)
        self.assertIn("libmutter-14-dev gobject-introspection libgirepository1.0-dev", workflow)
        self.assertIn("TAURI_SIGNING_PRIVATE_KEY", workflow)
        self.assertIn("workflow_call:", workflow)
        self.assertIn("mkdir -p artifact/runtime", workflow)
        self.assertNotIn("mkdir -p build/runtime-output", workflow)

    def test_ci_calls_the_producer_only_for_same_repository_pull_requests(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        condition = "github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository"
        self.assertGreaterEqual(workflow.count(condition), 2)
        self.assertIn("uses: ./.github/workflows/compositor-runtime.yml", workflow)
        self.assertIn("commit: ${{ needs.compositor-runtime-metadata.outputs.commit }}", workflow)
        self.assertIn("ref: ${{ github.event.pull_request.head.sha }}", workflow)
        self.assertIn('"$commit" == "${{ github.event.pull_request.head.sha }}"', workflow)
        self.assertIn("source_run_id: ${{ needs.compositor-runtime-metadata.outputs.source_run_id }}", workflow)
        self.assertIn("TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}", workflow)
        producer = (ROOT / ".github/workflows/compositor-runtime.yml").read_text(encoding="utf-8")
        self.assertIn('"$(git rev-parse HEAD)" == "${{ inputs.commit }}"', producer)
        self.assertNotIn('"$GITHUB_SHA" == "${{ inputs.commit }}"', producer)
        self.assertIn("COMPOSITOR_RUNTIME_COMMIT: ${{ inputs.commit }}", producer)
        self.assertIn('"commit": os.environ["COMPOSITOR_RUNTIME_COMMIT"]', producer)
        self.assertNotIn('"commit": os.environ["GITHUB_SHA"]', producer)

    def test_mutter_runtime_export_builds_all_install_targets_before_installing(self):
        script = (ROOT / "packaging/linux/desktop-integrations/gnome-shell-extension/mutter/verify-patch.sh").read_text(encoding="utf-8")
        runtime_branch = script.index('if [[ -n "$runtime_output" ]]; then')
        full_build = script.index('meson compile -C "$build_dir"\n', runtime_branch)
        install = script.index('DESTDIR="$runtime_output" meson install -C "$build_dir" --no-rebuild', runtime_branch)
        bridge_build = script.index('meson compile -C "$build_dir" "$target"', install)
        self.assertLess(runtime_branch, full_build)
        self.assertLess(full_build, install)
        self.assertLess(install, bridge_build)

    def test_mutter_export_invokes_the_nonexecutable_private_shell_builder_with_bash(self):
        script = (ROOT / "packaging/linux/desktop-integrations/gnome-shell-extension/mutter/verify-patch.sh").read_text(encoding="utf-8")
        self.assertIn('bash "$PWD/build-private-shell.sh" "$runtime_output"', script)

    def test_private_shell_export_builds_all_install_targets_before_no_rebuild_install(self):
        script = (ROOT / "packaging/linux/desktop-integrations/gnome-shell-extension/mutter/build-private-shell.sh").read_text(encoding="utf-8")
        self.assertIn('meson compile -C "$shell_build"\n', script)
        self.assertNotIn('meson compile -C "$shell_build" gnome-shell', script)
        self.assertLess(
            script.index('meson compile -C "$shell_build"\n'),
            script.index('DESTDIR="$mutter_runtime" meson install -C "$shell_build" --no-rebuild'),
        )

    def test_uses_authenticated_runtime_artifacts_and_generated_launchers(self):
        workflow = (ROOT / ".github/workflows/linux-native-qualification.yml").read_text(encoding="utf-8")
        release = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("uses: ./.github/workflows/compositor-runtime.yml", release)
        self.assertIn("TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}", release)
        self.assertIn("TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}", release)
        self.assertIn("compositor_runtime_run_id:", workflow)
        self.assertIn("verify-linux-compositor-runtime.py source", workflow)
        self.assertIn("verify-linux-compositor-runtime.py artifact", workflow)
        self.assertIn("--compositor-runtime-binding compositor-runtime/binding.json", workflow)
        self.assertIn("--compositor-runtime-binding /compositor-runtime/binding.json", workflow)
        self.assertIn("at-spi2-core python3-pyatspi ffmpeg qrencode v4l-utils", workflow)
        self.assertIn("COPYPASTE_QUALIFICATION_V4L2_DEVICE", workflow)
        self.assertIn("--previous-artifacts previous-artifacts/all", workflow)
        self.assertIn("--previous-version \"${{ needs.source.outputs.previous_version }}\"", workflow)
        for name in ("gnome-46-ubuntu24.04", "kwin-6.0-fedora40"):
            self.assertIn(name, workflow)
        for name in ("run-linux-native-desktop-session.sh", "run-fedora-plasma6-session.sh"):
            script = (ROOT / "scripts/release" / name).read_text(encoding="utf-8")
            self.assertIn("verify-linux-compositor-runtime.py", script)
            self.assertIn(" installed --binding", script)
            self.assertIn("copypaste-compositor-session-$runtime_id", script)
            self.assertNotIn("gnome-shell --headless", script)
            self.assertIn("COPYPASTE_COMPOSITOR_EXECUTION=stock-x11", script)
            self.assertIn("COPYPASTE_COMPOSITOR_EXECUTION=private-wayland", script)
            self.assertIn("COMPOSITOR_QUALIFICATION_ENTRYPOINT", script)
            self.assertIn("record-linux-compositor-session.py", script)
        generic = (ROOT / "scripts/release/run-linux-native-desktop-session.sh").read_text(encoding="utf-8")
        self.assertIn("KDE qualification must use the Fedora", generic)
        fedora_wrapper = (ROOT / "scripts/release/run-fedora-plasma6-qualification.sh").read_text(encoding="utf-8")
        self.assertIn("--device \"$V4L2_DEVICE:$V4L2_DEVICE\"", fedora_wrapper)


if __name__ == "__main__":
    unittest.main()
