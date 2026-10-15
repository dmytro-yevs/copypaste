import sys
from pathlib import Path
import tempfile
import json
import subprocess
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from metrics import cache_observations, compare, critical_path, measure, percentile, summarize
from workflow_yaml import read_workflow


class MetricsTest(unittest.TestCase):
    def test_comparison_never_reports_speedup_for_unknown_or_different_cache_states(self):
        def report(cache, values):
            return {"workflows": {"ci.yml": {"runs": [{"event": "push", "mode": "validation", "cache_cohort": cache,
                "conclusion": "success", "wall_clock_s": value} for value in values]}}}
        self.assertFalse(compare(report("unknown", [100] * 3), report("unknown", [50] * 3))[0]["comparable"])
        self.assertTrue(all(not row["comparable"] for row in compare(report("cold", [100] * 3), report("warm", [50] * 3))))
        self.assertEqual(compare(report("warm", [100] * 3), report("warm", [80] * 3))[0]["improvement_percent"], 20)
    def test_timing_wrapper_preserves_failed_command_status(self):
        command = Path(__file__).resolve().parents[1] / "timed.py"
        result = subprocess.run([sys.executable, str(command), "compilation", sys.executable,
                                 "-c", "print('compiler output'); raise SystemExit(7)"], text=True, capture_output=True)
        self.assertEqual(result.returncode, 7)
        self.assertIn("compiler output", result.stdout)
        record = json.loads(result.stdout.split("COPYPASTE_CI_TIMING ", 1)[1])
        self.assertEqual(record["exit_code"], 7)
        self.assertGreaterEqual(record["duration_s"], 0)
    def test_cache_markers_exclude_echoes_and_do_not_double_count_restore(self):
        result = cache_observations("\n".join([
            "2026-01-01T00:00:00Z echo 'Cache hit for: fake'",
            "2026-01-01T00:00:01.1234567Z Cache hit for: real",
            "2026-01-01T00:00:02Z Cache Size: ~1 MB (1000000 B)",
            "2026-01-01T00:00:04.1234567Z Cache restored successfully",
            "2026-01-01T00:00:05Z Cache restored from key: real",
            "2026-01-01T00:00:06Z Cache not found for input keys: cold, fallback",
        ]))
        self.assertEqual(result["restored"], 1)
        self.assertEqual(result["missed"], 1)
        self.assertEqual(result["restore_s"], 3)
        self.assertEqual(result["observed_hit_rate"], .5)
        self.assertEqual(result["restored_bytes"], 1000000)
        incomplete = cache_observations("2026-01-01T00:00:01Z Cache hit for: corrupt")
        self.assertEqual(incomplete["incomplete_restores"], 1)
        self.assertEqual(incomplete["observed_hit_rate"], 0)
    def test_wall_clock_is_not_sum_of_parallel_jobs_or_updated_at(self):
        run = {"id": 1, "path": ".github/workflows/ci.yml", "html_url": "url", "head_sha": "sha",
               "event": "push", "created_at": "2026-01-01T00:00:00Z", "conclusion": "success",
               "updated_at": "2026-01-02T00:00:00Z"}
        jobs = [{"id": i, "name": str(i), "conclusion": "success", "steps": [],
                 "started_at": "2026-01-01T00:00:10Z", "completed_at": "2026-01-01T00:01:10Z"}
                for i in range(2)]
        result = measure(run, {"jobs": jobs, "total_count": 2})
        self.assertEqual(result["wall_clock_s"], 70)
        self.assertEqual(result["job_execution_s"], 120)
        self.assertEqual(result["initial_queue_s"], 10)
        self.assertEqual(result["cache_cohort"], "unknown")
        with self.assertRaises(ValueError):
            measure(run, {"jobs": jobs, "total_count": 3})

    def test_failed_and_different_cache_cohorts_never_pollute_success_percentiles(self):
        runs = [{"mode": "build", "cache_cohort": cache, "conclusion": conclusion, "wall_clock_s": seconds}
                for cache, conclusion, seconds in [("cold", "success", 100), ("warm", "success", 50),
                                                    ("cold", "cancelled", 1), ("unknown", "success", 60)]]
        self.assertEqual(len(summarize(runs)), 4)
        self.assertEqual(percentile([1, 2, 3, 4, 100], .95), 100)
        self.assertIsNone(percentile([], .95))

    def test_yaml_preserves_triggers_and_rejects_duplicate_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "workflow.yml"
            path.write_text("on:\n  push:\npermissions:\n  contents: read\nconcurrency:\n  cancel-in-progress: false\n")
            self.assertIn("push", read_workflow(path)["on"])
            self.assertIs(read_workflow(path)["concurrency"]["cancel-in-progress"], False)
            path.write_text("jobs: {}\njobs: {}\n")
            with self.assertRaisesRegex(ValueError, "duplicate YAML"):
                read_workflow(path)

    def test_critical_path_expands_reusable_parallel_jobs(self):
        def job(name, needs=(), workflow=None):
            return {"name": name, "needs": list(needs), "workflow": workflow, "matrix": None}
        topology = {"ci.yml": {"jobs": {"prepare": job("prepare"),
                    "build": job("build", ["prepare"], "./.github/workflows/build.yml"),
                    "independent": job("independent"), "gate": job("gate", ["build", "independent"])}},
                    "build.yml": {"jobs": {"a": job("a"), "b": job("b")}}}
        observed = [{"name": name, "duration_s": duration, "conclusion": "success"}
                    for name, duration in [("prepare", 10), ("build / a", 20), ("build / b", 30), ("independent", 35), ("gate", 5)]]
        result = critical_path(topology, "ci.yml", observed)
        self.assertEqual(result["execution_s"], 45)
        self.assertEqual(result["jobs"], ["prepare", "build / b", "gate"])


if __name__ == "__main__":
    unittest.main()
