from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts/ci"))
from workflow_yaml import read_workflow


class CodeQlAndroidBuildTest(unittest.TestCase):
    def test_manual_android_build_forces_a_fresh_traced_compilation(self):
        workflow = read_workflow(ROOT / ".github/workflows/codeql-scheduled.yml")
        steps = workflow["jobs"]["analyze-java-kotlin"]["steps"]
        init = next(index for index, step in enumerate(steps)
                    if step.get("uses", "").startswith("github/codeql-action/init@"))
        build = next(index for index, step in enumerate(steps)
                     if step.get("name") == "Build Android Java and Kotlin sources for CodeQL")
        self.assertLess(init, build)
        self.assertEqual(steps[build]["working-directory"], "apps/copypaste_flutter")
        command = steps[build]["run"]
        self.assertIn("flutter pub get --enforce-lockfile", command)
        self.assertIn("flutter build apk --debug --config-only --no-pub", command)
        self.assertIn("cd android", command)
        gradle = next(line.split() for line in command.splitlines()
                      if line.strip().startswith("./gradlew "))
        self.assertEqual(gradle[0], "./gradlew")
        self.assertIn(":app:assembleDebug", gradle)
        self.assertTrue({"--no-daemon", "--no-configuration-cache", "--no-build-cache", "--rerun-tasks"} <= set(gradle))


if __name__ == "__main__":
    unittest.main()
