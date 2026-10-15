import sys
from pathlib import Path
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from metrics import measure, percentile, summarize
from workflow_yaml import read_workflow


class MetricsTest(unittest.TestCase):
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
            path.write_text("on:\n  push:\npermissions:\n  contents: read\n")
            self.assertIn("push", read_workflow(path)["on"])
            path.write_text("jobs: {}\njobs: {}\n")
            with self.assertRaisesRegex(ValueError, "duplicate YAML"):
                read_workflow(path)


if __name__ == "__main__":
    unittest.main()
