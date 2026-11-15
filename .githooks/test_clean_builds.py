"""Exercise cleanup hooks without removing real build outputs."""

import importlib.util
import shutil
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('cleanup', Path(__file__).with_name('clean-builds.py'))
cleanup = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cleanup)
PROCESS_QUERY = cleanup.build_processes_active


class CleanupTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.addCleanup(patch.stopall)
        patch.object(cleanup, 'ROOT', self.root).start()
        for location in ['', 'modules/ocr', 'modules/sms-codes', 'crates/copypaste-flutter-bridge']:
            folder = self.root / location
            (folder / 'target').mkdir(parents=True)
            (folder / 'Cargo.toml').touch()
        self.flutter = self.root / 'apps/copypaste_flutter'
        (self.flutter / '.dart_tool/hooks_runner/shared/bridge/build/hash/target').mkdir(parents=True)
        (self.flutter / 'build').mkdir()
        self.run = patch.object(cleanup.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)).start()
        patch.object(cleanup.shutil, 'which', side_effect=lambda name: name).start()
        self.processes = patch.object(cleanup, 'build_processes_active', return_value=False).start()
        self.locks = patch.object(cleanup, 'is_build_active', return_value=False).start()

    def test_native_cleaners_cover_workspace_modules_bridge_and_flutter(self):
        cleanup.clean()
        calls = self.run.call_args_list
        self.assertEqual(len(calls), 5)
        cargo = [call.args[0] for call in calls if call.args[0][0] == 'cargo']
        self.assertEqual({Path(argv[-1]) for argv in cargo}, {
            self.root / 'target', self.root / 'modules/ocr/target',
            self.root / 'modules/sms-codes/target',
            self.root / 'crates/copypaste-flutter-bridge/target',
        })
        self.assertEqual(calls[-1].args[0], ['flutter', 'clean'])
        self.assertEqual(calls[-1].kwargs['cwd'], self.flutter)
        self.assertGreaterEqual(self.processes.call_count, 6)
        self.assertTrue((self.root / 'Cargo.toml').exists())

    def test_active_build_skips_every_command(self):
        self.processes.return_value = True
        cleanup.clean()
        self.run.assert_not_called()

    def test_native_hook_lock_skips_every_command(self):
        self.locks.side_effect = lambda target: 'hooks_runner' in str(target)
        cleanup.clean()
        self.run.assert_not_called()

    def test_build_starting_between_commands_stops_cleanup(self):
        self.processes.side_effect = [False, False, True]
        cleanup.clean()
        self.assertEqual(self.run.call_count, 1)

    def test_missing_cargo_still_allows_flutter_clean(self):
        cleanup.shutil.which.side_effect = lambda name: None if name == 'cargo' else name
        cleanup.clean()
        self.run.assert_called_once()
        self.assertEqual(self.run.call_args.args[0], ['flutter', 'clean'])

    def test_native_failure_is_nonfatal(self):
        self.run.return_value.returncode = 1
        cleanup.clean()
        self.assertEqual(self.run.call_count, 5)

    def test_process_inspection_failure_runs_no_cleaners(self):
        self.processes.side_effect = OSError('cannot inspect processes')
        with self.assertRaises(OSError):
            cleanup.clean()
        self.run.assert_not_called()

    def test_idle_gradle_daemon_is_allowed_but_build_tools_are_active(self):
        self.assertFalse(cleanup.process_is_build('java', 'org.gradle.launcher.daemon.bootstrap.GradleDaemon'))
        for name, command in [
            ('cargo', 'cargo check'), ('rustc.exe', 'rustc'),
            ('dart', 'dart flutter_tools.snapshot run'), ('xcodebuild', 'xcodebuild'),
            ('java.exe', 'org.gradle.wrapper.GradleWrapperMain assembleDebug'),
        ]:
            with self.subTest(name=name):
                self.assertTrue(cleanup.process_is_build(name, command))

    def test_unix_process_query_detects_flutter(self):
        with patch.object(cleanup, 'build_processes_active', wraps=PROCESS_QUERY):
            self.run.return_value.stdout = '/opt/flutter/bin/cache/dart-sdk/bin/dart dart flutter_tools.snapshot build apk\n'
            self.assertTrue(cleanup.build_processes_active())

    def test_windows_process_query_detects_rust(self):
        original = PROCESS_QUERY
        self.run.return_value.stdout = '[{"Name":"rustc.exe","CommandLine":"rustc --crate-name example"}]'
        with patch.object(cleanup.sys, 'platform', 'win32'):
            self.assertTrue(original())


class HookIntegrationTest(unittest.TestCase):
    def test_commit_and_push_invoke_cleanup_without_failing_on_cleanup_error(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / 'checkout'
            repository.mkdir()
            hooks = repository / '.githooks'
            hooks.mkdir()
            source = Path(__file__).parent
            for name in ['post-commit', 'pre-push', 'clean-builds']:
                shutil.copy2(source / name, hooks / name)
            (hooks / 'clean-builds.py').write_text(
                "from pathlib import Path\n"
                "p = Path(__file__).with_name('invocations')\n"
                "with p.open('a') as stream: stream.write('cleanup\\n')\n"
                "raise SystemExit(23)\n"
            )
            def git(*arguments):
                return subprocess.run(['git', *arguments], cwd=repository,
                                      check=True, capture_output=True, text=True)
            git('init', '-q')
            git('config', 'user.name', 'Cleanup test')
            git('config', 'user.email', 'cleanup@example.invalid')
            git('config', 'core.hooksPath', '.githooks')
            (repository / 'fixture').write_text('fixture')
            git('add', 'fixture')
            git('commit', '-qm', 'Fixture')
            self.assertEqual((hooks / 'invocations').read_text().splitlines(), ['cleanup'])
            remote = root / 'remote.git'
            git('init', '--bare', '-q', str(remote))
            git('push', str(remote), 'HEAD:refs/heads/test')
            self.assertEqual((hooks / 'invocations').read_text().splitlines(), ['cleanup', 'cleanup'])


if __name__ == '__main__':
    unittest.main()
