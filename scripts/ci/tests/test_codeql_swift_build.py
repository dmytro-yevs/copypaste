from pathlib import Path
import shlex
import sys
import unittest


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts/ci"))
from workflow_yaml import read_workflow


class CodeQlSwiftBuildTest(unittest.TestCase):
    def test_swift_targets_recompile_under_tracing_after_untraced_dart_and_rust_build(self):
        workflow = read_workflow(ROOT / ".github/workflows/codeql-scheduled.yml")
        steps = workflow["jobs"]["analyze-swift"]["steps"]
        init = next(index for index, step in enumerate(steps)
                    if step.get("uses", "").startswith("github/codeql-action/init@"))
        builds = [(index, shlex.split(step["run"])) for index, step in enumerate(steps)
                  if step.get("run", "").startswith("scripts/ci/xcodebuild-macos-release.sh ")]
        self.assertEqual(len(builds), 2)
        (prebuild, prebuild_args), (traced, traced_args) = builds
        self.assertLess(prebuild, init)
        self.assertLess(init, traced)
        self.assertEqual(steps[traced]["name"], "Build Swift sources for CodeQL")
        # New derived data recompiles every Xcode target; shared products keep
        # Flutter's assemble cache, which is keyed by the products directory.
        self.assertNotEqual(prebuild_args[1], traced_args[1])
        self.assertEqual(prebuild_args[2], traced_args[2])


if __name__ == "__main__":
    unittest.main()
