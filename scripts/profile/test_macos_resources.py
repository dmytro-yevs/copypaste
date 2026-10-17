import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location("macos_resources", Path(__file__).with_name("collect-macos-resources.py"))
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)


def process(identity, memory, cpu=0):
    return dict.fromkeys(collector.FIELDS, 0) | {
        "identity": identity, "phys_footprint": memory, "user_time": cpu,
    }


class MacosResourcesTest(unittest.TestCase):
    def test_family_excludes_installed_app_and_includes_helper_descendants(self):
        table = {10: (1, "/test/Dev.app/Contents/MacOS/Dev"),
                 11: (10, "/test/Dev.app/Contents/MacOS/copypaste-daemon"),
                 12: (11, "/usr/bin/helper"),
                 20: (1, "/Applications/CopyPaste.app/Contents/MacOS/CopyPaste"),
                 21: (20, "/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon"),
                 30: (1, "/test/Dev.app.other/worker")}
        self.assertEqual(collector.family(table, Path("/test/Dev.app"), 10), {10, 11, 12})
        with self.assertRaises(ValueError):
            collector.family(table, Path("/test/Dev.app"), 20)

    def test_peak_is_simultaneous_and_boundary_is_strict(self):
        rows = [{"elapsed_seconds": 0, "processes": [process("a", 80), process("b", 10)]},
                {"elapsed_seconds": 1, "processes": [process("a", 10), process("b", 80)]}]
        self.assertEqual(collector.summarize(rows, 100)["peak_bytes"], 90)
        self.assertTrue(collector.summarize(rows, 100)["observed_memory_budget_pass"])
        self.assertFalse(collector.summarize(rows, 90)["observed_memory_budget_pass"])
        self.assertFalse(collector.summarize([], 100)["observed_memory_budget_pass"])

    def test_cpu_tracks_process_start_identity_not_pid_or_lifetime_totals(self):
        rows = [{"elapsed_seconds": 0, "processes": [process("1:start", 10, 9_000_000_000)]},
                {"elapsed_seconds": 1, "processes": [process("1:start", 10, 9_500_000_000)]},
                {"elapsed_seconds": 2, "processes": [process("1:new", 10, 500_000_000)]}]
        self.assertEqual(collector.summarize(rows, 100)["cpu_percent_one_core"], 25)

    def test_cpu_converts_apple_silicon_absolute_ticks(self):
        rows = [{"elapsed_seconds": 0, "processes": [process("a", 10)]},
                {"elapsed_seconds": 1, "processes": [process("a", 10, 12_000_000)]}]
        self.assertAlmostEqual(
            collector.summarize(rows, 100, 125 / 3)["cpu_percent_one_core"], 50)


if __name__ == "__main__":
    unittest.main()
