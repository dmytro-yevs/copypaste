#!/usr/bin/env python3
"""Self-tests for scripts/target-budget.py.

Each case is a way the sweep could destroy something it should not: an
unmarked-but-new artifact, a hash cargo never named, a build holding a lock
under a profile the probe forgot to look at, a directory that is not cargo's.
"""

import importlib.util
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

HOLD_LOCK = """
import sys, time
path = sys.argv[1]
handle = open(path, "r+b")
if sys.platform == "win32":
    import msvcrt
    msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
else:
    import fcntl
    fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
print("held", flush=True)
time.sleep(120)
"""

SCRIPTS = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

from target_budget import locks, marks, plan, reclaim, survey  # noqa: E402

SPEC = importlib.util.spec_from_file_location("target_budget_cli", SCRIPTS / "target-budget.py")
cli = importlib.util.module_from_spec(SPEC)
sys.modules["target_budget_cli"] = cli
SPEC.loader.exec_module(cli)


def make_target(root: pathlib.Path, units: dict[str, str], profile: str = "debug") -> pathlib.Path:
    """A minimal but structurally real cargo target directory."""
    target = root / "target"
    (target).mkdir(parents=True, exist_ok=True)
    (target / "CACHEDIR.TAG").write_bytes(locks.CACHEDIR_SIGNATURE + b"\n")
    prof = target / profile
    for area in ("deps", "build", ".fingerprint"):
        (prof / area).mkdir(parents=True, exist_ok=True)
    for unit_hash, stem in units.items():
        fp = prof / ".fingerprint" / f"{stem}-{unit_hash}"
        fp.mkdir(exist_ok=True)
        (fp / "lib-something.json").write_text('{"rustc": 1}', encoding="utf-8")
        (prof / "deps" / f"lib{stem.replace('-', '_')}-{unit_hash}.rlib").write_bytes(b"x" * 4096)
    return target


class SurveyTest(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_groups_by_unit_hash_not_package_name(self):
        """Two hashes of one package are two generations, never one.

        `copypaste-ipc` really does carry four live hashes in one build — lib,
        lib test, and two integration tests. Collapsing them by name is what
        makes a sweep delete current output.
        """
        target = make_target(self.tmp, {"a" * 16: "copypaste-ipc", "b" * 16: "copypaste-ipc"})
        profile = survey.survey(target)[0]
        self.assertEqual(len(profile.generations), 2)

    def test_finds_profiles_by_fingerprint_not_by_name(self):
        target = make_target(self.tmp, {"c" * 16: "serde"}, profile="x86_64-linux-android/debug")
        roots = survey.profile_dirs(target)
        self.assertEqual([r.name for r in roots], ["debug"])
        self.assertIn("x86_64-linux-android", str(roots[0]))

    def test_hash_absent_from_fingerprint_is_left_alone(self):
        target = make_target(self.tmp, {"d" * 16: "serde"})
        stray = target / "debug" / "deps" / f"libmystery-{'e' * 16}.rlib"
        stray.write_bytes(b"y" * 128)
        profile = survey.survey(target)[0]
        self.assertEqual(len(profile.generations), 1)
        self.assertEqual(profile.unattributed, 1)


class PlanTest(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.target = make_target(self.tmp, {"a" * 16: "serde", "b" * 16: "serde"})
        self.profiles = survey.survey(self.target)

    def _marks(self, hashes, recorded_at):
        return marks.Marks(frozenset(hashes), recorded_at, ("build --workspace",))

    def test_unmarked_and_old_is_evicted(self):
        result = plan.build(self.profiles, self._marks({"a" * 16}, time.time() + 60))
        self.assertEqual([e.key for e in result.evictions], ["b" * 16])

    def test_unmarked_but_newer_than_the_mark_is_kept(self):
        """Output made after we last asked cargo is work we know nothing about."""
        result = plan.build(self.profiles, self._marks({"a" * 16}, 0.0))
        self.assertEqual(result.evictions, [])
        self.assertEqual(result.newer_than_mark, 1)

    def test_sweeping_without_a_mark_is_refused(self):
        empty = marks.Marks(frozenset(), 0.0, ())
        with self.assertRaises(ValueError):
            plan.build(self.profiles, empty)

    def test_incremental_older_than_the_mark_is_dropped_unless_asked_otherwise(self):
        session = self.target / "debug" / "incremental" / "serde-24ksdgmtsspka"
        session.mkdir(parents=True)
        (session / "s-abc-def").write_bytes(b"z" * 8192)
        profiles = survey.survey(self.target)
        self.assertEqual(len(profiles[0].incremental), 1, "13-char session ids must parse")

        marks_now = self._marks({"a" * 16, "b" * 16}, time.time() + 60)
        dropped = plan.build(profiles, marks_now)
        self.assertEqual([e.reason for e in dropped.evictions], [plan.STALE_SCRATCH])
        self.assertEqual(plan.build(profiles, marks_now, keep_incremental=True).evictions, [])

    def test_budget_never_evicts_a_marked_generation(self):
        both = self._marks({"a" * 16, "b" * 16}, time.time() + 60)
        result = plan.build(self.profiles, both, budget=1)
        self.assertEqual(result.evictions, [])
        self.assertGreater(result.shortfall, 0)


class LockTest(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_lock_is_found_under_any_profile_not_just_debug_and_release(self):
        """A cross-target or custom-profile build must not read as idle.

        Probing a hardcoded `debug`/`release` pair reports "no build running"
        during `--target x86_64-linux-android` or `--profile evidence`, and the
        sweep then deletes that build's inputs while it runs.
        """
        target = make_target(self.tmp, {"a" * 16: "serde"})
        for profile in ("x86_64-linux-android/debug", "evidence"):
            path = target / profile
            path.mkdir(parents=True, exist_ok=True)
            (path / ".cargo-lock").write_bytes(b"")
        found = {p.parent.name for p in locks.build_locks(target)}
        self.assertEqual(found, {"debug", "evidence"})
        self.assertIn("x86_64-linux-android", str(locks.build_locks(target)[-1].parent.parent))

    def test_held_lock_blocks_the_sweep(self):
        """A real build holds the lock from another process, so the test must too.

        Both `fcntl.flock` and `msvcrt.locking` are per-process: taking the lock
        in this process leaves the probe free to take it again, and the test
        passes while the guard does nothing.
        """
        target = make_target(self.tmp, {"a" * 16: "serde"})
        lock = target / "debug" / ".cargo-lock"
        lock.write_bytes(b"\0")
        self.assertEqual(locks.held_locks(target), [])

        holder = subprocess.Popen(
            [sys.executable, "-c", HOLD_LOCK, str(lock)],
            stdout=subprocess.PIPE, text=True,
        )
        self.addCleanup(holder.kill)
        self.assertEqual(holder.stdout.readline().strip(), "held")

        self.assertEqual(locks.held_locks(target), [lock])
        marks_file = target / marks.MARK_FILE
        marks_file.write_text(
            json.dumps({"hashes": [], "recorded_at": time.time(), "configurations": ["build"]}),
            encoding="utf-8",
        )
        self.assertEqual(cli.main(["--target", str(target), "--repo", str(self.tmp)]), 3)


class CliTest(unittest.TestCase):
    def setUp(self):
        self.tmp = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def test_a_directory_without_cargos_cachedir_tag_is_refused(self):
        """Gradle's `build/` has no tag. The tag is the discriminator."""
        gradle = self.tmp / "build"
        (gradle / "debug" / ".fingerprint").mkdir(parents=True)
        self.assertEqual(cli.main(["--target", str(gradle), "--repo", str(self.tmp)]), 2)

    def test_dry_run_is_the_default_and_removes_nothing(self):
        target = make_target(self.tmp, {"a" * 16: "serde", "b" * 16: "serde"})
        (target / marks.MARK_FILE).write_text(
            json.dumps({"hashes": ["a" * 16], "recorded_at": time.time() + 60,
                        "configurations": ["build"]}),
            encoding="utf-8",
        )
        before = survey.total_size(target)
        self.assertEqual(cli.main(["--target", str(target), "--repo", str(self.tmp)]), 0)
        self.assertEqual(survey.total_size(target), before)
        self.assertEqual(cli.main(["--target", str(target), "--repo", str(self.tmp), "--apply"]), 0)
        self.assertLess(survey.total_size(target), before)

    def test_fingerprint_is_removed_before_the_artifacts_it_vouches_for(self):
        target = make_target(self.tmp, {"a" * 16: "serde"})
        gen = next(iter(survey.survey(target)[0].generations.values()))
        self.assertGreater(len(gen.paths), 1, "generation must span deps and .fingerprint")
        ordered = reclaim.removal_order(tuple(gen.paths))
        self.assertEqual(ordered[0].parent.name, ".fingerprint")
        self.assertEqual(sorted(ordered), sorted(gen.paths), "ordering must not drop a path")

    def test_sizes_parse(self):
        self.assertEqual(cli.parse_size("12GiB"), 12 * 1024**3)
        self.assertEqual(cli.parse_size("512MiB"), 512 * 1024**2)
        self.assertEqual(cli.parse_size("1024"), 1024)
        with self.assertRaises(Exception):
            cli.parse_size("dozens")


class MarkTest(unittest.TestCase):
    def test_hashes_come_from_artifacts_and_build_scripts(self):
        stream = "\n".join([
            json.dumps({"reason": "compiler-artifact",
                        "filenames": [f"/t/target/debug/deps/libserde-{'a' * 16}.rlib"],
                        "executable": None}),
            json.dumps({"reason": "build-script-executed",
                        "out_dir": f"/t/target/debug/build/ahash-{'b' * 16}/out"}),
            "not json at all",
        ])
        self.assertEqual(marks._hashes(stream), frozenset({"a" * 16, "b" * 16}))


if __name__ == "__main__":
    unittest.main(verbosity=2)
