import importlib.util
import json
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("gate", HERE / "check-resource-receipts.py")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
COMMIT = "a" * 40


class ResourceReceiptTests(unittest.TestCase):
    def setUp(self):
        self.contract = json.loads((HERE.parents[1] / "config/runtime-resource-budgets.json").read_text())

    def receipt(self, platform="macos", scenario="idle", memory=100_000_000):
        return dict(platform=platform, scenario=scenario, commit=COMMIT, dirty=False,
                    build_mode="release", budget_version=self.contract["budget_version"],
                    artifact_sha256="b" * 64, device="fixture", os_version="fixture",
                    fixture_sha256="c" * 64, signing_identity="fixture", provenance_receipt="fixture",
                    controlled=True, process_coverage_complete=True, physical_device=True,
                    memory_metric=self.contract["platforms"][platform]["memory_metric"],
                    samples=[dict(elapsed_seconds=i, processes=[dict(identity="1:start", memory_bytes=memory)]) for i in range(2)],
                    metrics={name: 0 for name in ("cpu_percent", "wakeups_per_second", "disk_bytes_per_second",
                                                  "network_bytes_per_second", "recovery_seconds", "sustained_growth_bytes")})

    def test_no_receipts_never_pass(self):
        self.assertEqual(gate.check(self.contract, [], COMMIT)["status"], "incomplete")

    def test_memory_sums_processes_and_equality_fails(self):
        receipt = self.receipt(memory=150_000_000)
        for sample in receipt["samples"]:
            sample["processes"].append(dict(identity="2:start", memory_bytes=150_000_000))
        result = gate.check(self.contract, [receipt], COMMIT)
        self.assertEqual(result["status"], "failed")
        self.assertTrue(any("combined memory" in x for x in result["failures"]))

    def test_null_budgets_and_missing_platforms_remain_incomplete(self):
        result = gate.check(self.contract, [self.receipt()], COMMIT)
        self.assertEqual(result["status"], "incomplete")
        self.assertTrue(any("unresolved" in x for x in result["missing"]))

    def test_ai_exception_does_not_apply_to_recovery(self):
        self.contract["platforms"]["macos"]["ocr_ai_peak_limit_bytes"] = 500_000_000
        self.assertEqual(gate.check(self.contract, [self.receipt(scenario="recovery", memory=400_000_000)], COMMIT)["status"], "failed")

    def test_dirty_debug_wrong_commit_and_metric_fail(self):
        for field, value in [("dirty", True), ("build_mode", "debug"), ("commit", "d" * 40), ("memory_metric", "rss")]:
            with self.subTest(field=field):
                receipt = self.receipt()
                receipt[field] = value
                self.assertEqual(gate.check(self.contract, [receipt], COMMIT)["status"], "failed")

    def test_duplicate_identity_nan_negative_and_missing_samples_fail_closed(self):
        for value in [float("nan"), -1, True]:
            self.assertEqual(gate.check(self.contract, [self.receipt(memory=value)], COMMIT)["status"], "failed")
        receipt = self.receipt()
        receipt["samples"][0]["processes"] *= 2
        self.assertEqual(gate.check(self.contract, [receipt], COMMIT)["status"], "failed")
        receipt = self.receipt()
        receipt["samples"] = []
        self.assertEqual(gate.check(self.contract, [receipt], COMMIT)["status"], "incomplete")

    def test_emulator_and_uncontrolled_runs_do_not_qualify(self):
        receipt = self.receipt(platform="android")
        receipt["physical_device"] = False
        receipt["controlled"] = False
        result = gate.check(self.contract, [receipt], COMMIT)
        self.assertTrue(any("physical Android" in x for x in result["missing"]))
        self.assertTrue(any("uncontrolled" in x for x in result["missing"]))

    def test_artifact_changes_between_scenarios_fail(self):
        first, second = self.receipt(), self.receipt(scenario="active")
        second["artifact_sha256"] = "c" * 64
        self.assertEqual(gate.check(self.contract, [first, second], COMMIT)["status"], "failed")

    def test_complete_synthetic_matrix_requires_explicit_budgets(self):
        # Synthetic thresholds test the checker; they never modify product budgets.
        for budget in self.contract["platforms"].values():
            for key in budget:
                if budget[key] is None:
                    budget[key] = 500_000_000 if key == "ocr_ai_peak_limit_bytes" else 1
        receipts = [self.receipt(platform=p, scenario=s) for p in self.contract["platforms"]
                    for s in self.contract["required_scenarios"]]
        self.assertEqual(gate.check(self.contract, receipts, COMMIT)["status"], "passed")
        receipts[0]["metrics"]["cpu_percent"] = 2
        self.assertEqual(gate.check(self.contract, receipts, COMMIT)["status"], "failed")


if __name__ == "__main__":
    unittest.main()
