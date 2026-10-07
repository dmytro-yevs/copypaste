"""Run native cleaners only while build processes and Cargo locks are idle."""

import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from cargo_target import is_build_active


def process_is_build(name, command):
    name = name.replace('\\', '/').rsplit('/', 1)[-1].lower().removesuffix('.exe')
    return (
        name in {'cargo', 'cargo-ndk', 'rustc', 'xcodebuild', 'flutter', 'gradle', 'gradlew'}
        or (name in {'dart', 'dartaotruntime'} and 'flutter_tools' in command)
        or (name == 'java' and any(marker in command for marker in (
            'org.gradle.launcher.GradleMain', 'org.gradle.wrapper.GradleWrapperMain',
        )))
    )


def build_processes_active():
    if sys.platform == 'win32':
        result = subprocess.run([
            'powershell.exe', '-NoProfile', '-NonInteractive', '-Command',
            'Get-CimInstance Win32_Process | Select-Object Name,CommandLine | ConvertTo-Json -Compress',
        ], check=True, capture_output=True, text=True)
        processes = json.loads(result.stdout)
        if isinstance(processes, dict):
            processes = [processes]
        return any(process_is_build(p['Name'], p.get('CommandLine') or '') for p in processes)
    result = subprocess.run(['ps', '-axo', 'comm=,args='],
                            check=True, capture_output=True, text=True)
    return any(process_is_build(*parts) for line in result.stdout.splitlines()
               if len(parts := line.strip().split(None, 1)) == 2)


def cargo_jobs():
    manifests = [ROOT / 'Cargo.toml', *sorted((ROOT / 'modules').glob('*/Cargo.toml')),
                 ROOT / 'crates/copypaste-flutter-bridge/Cargo.toml']
    return [(manifest, manifest.parent / 'target') for manifest in manifests
            if manifest.is_file() and (manifest.parent / 'target').is_dir()]


def idle(targets):
    if build_processes_active() or any(is_build_active(target) for target in targets):
        print('build cleanup: active build; skipped', file=sys.stderr)
        return False
    return True


def clean():
    # Cleaning is restricted to this checkout, even when Cargo uses an external target.
    jobs = cargo_jobs()
    flutter = ROOT / 'apps/copypaste_flutter'
    hook_targets = list((flutter / '.dart_tool/hooks_runner').glob('shared/*/build/*/target'))
    targets = [target for _, target in jobs] + hook_targets
    if not idle(targets):
        return
    commands = [(['cargo', 'clean', '--manifest-path', str(manifest), '--target-dir', str(target)], ROOT)
                for manifest, target in jobs]
    if (flutter / 'build').exists() or (flutter / '.dart_tool').exists():
        commands.append((['flutter', 'clean'], flutter))
    for command, cwd in commands:
        if not idle(targets):
            return
        executable = shutil.which(command[0])
        if executable is None:
            print(f'build cleanup: {command[0]} unavailable; skipped', file=sys.stderr)
            continue
        print(f'build cleanup: {command[0]} clean ({cwd.relative_to(ROOT)})', file=sys.stderr)
        result = subprocess.run([executable, *command[1:]], cwd=cwd, check=False,
                                stdin=subprocess.DEVNULL)
        if result.returncode:
            print(f'build cleanup: {command[0]} clean failed ({result.returncode})', file=sys.stderr)


if __name__ == '__main__':
    try:
        clean()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f'build cleanup: unable to verify or clean; skipped ({error})', file=sys.stderr)
