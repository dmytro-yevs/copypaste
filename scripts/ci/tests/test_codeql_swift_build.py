from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts/ci"))
from workflow_yaml import read_workflow


class CodeQlSwiftBuildTest(unittest.TestCase):
    def test_one_untraced_build_after_init_traces_the_swift_compiler_alone(self):
        workflow = read_workflow(ROOT / ".github/workflows/codeql-scheduled.yml")
        steps = workflow["jobs"]["analyze-swift"]["steps"]
        init = next(index for index, step in enumerate(steps)
                    if step.get("uses", "").startswith("github/codeql-action/init@"))
        builds = [index for index, step in enumerate(steps)
                  if step.get("run", "").startswith("scripts/ci/xcodebuild-macos-release.sh ")]
        self.assertEqual(len(builds), 1)
        build = steps[builds[0]]
        self.assertLess(init, builds[0])
        self.assertEqual(build["name"], "Build Swift sources for CodeQL")
        # An empty value starts the step outside the tracer CodeQL init enabled.
        self.assertEqual(build["env"]["DYLD_INSERT_LIBRARIES"], "")
        script = (ROOT / "scripts/ci/xcodebuild-macos-release.sh").read_text()
        self.assertIn('"SWIFT_EXEC=$traced_swiftc"', script)
        self.assertIn('SWIFT_EXEC_MANIFEST="$traced_swiftc"', script)


if __name__ == "__main__":
    unittest.main()
