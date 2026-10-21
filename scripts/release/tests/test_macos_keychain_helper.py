"""Native update regression using only task-local disposable Keychains."""
import hashlib
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]


@unittest.skipUnless(sys.platform == "darwin", "requires native macOS code signing and Keychain")
class MacosKeychainUpdateTest(unittest.TestCase):
    def run_command(self, *args, **kwargs):
        try:
            return subprocess.run(args, check=True, capture_output=True, text=True,
                                  timeout=30, env=self.environment, **kwargs)
        except subprocess.CalledProcessError as error:
            self.fail(f"Native fixture command {Path(args[0]).name} failed ({error.returncode}):\n"
                      f"{error.stdout}\n{error.stderr}")

    def test_updated_daemon_reuses_one_key_and_one_helper_grant(self):
        with tempfile.TemporaryDirectory(prefix="keychain-update-") as directory:
            root = Path(directory)
            self.environment = dict(os.environ, TMPDIR=str(root))
            search_before = self.run_command("security", "list-keychains", "-d", "user").stdout
            default_before = self.run_command("security", "default-keychain", "-d", "user").stdout
            def assert_preferences_unchanged():
                for arguments, expected in ((["list-keychains", "-d", "user"], search_before),
                                            (["default-keychain", "-d", "user"], default_before)):
                    self.assertEqual(subprocess.check_output(["security", *arguments],
                                                            env=os.environ, text=True), expected)
            app = root / "CopyPaste.app"
            binaries = app / "Contents/MacOS"
            binaries.mkdir(parents=True)
            (app / "Contents/Info.plist").write_bytes(plistlib.dumps({
                "CFBundleExecutable": "CopyPaste", "CFBundleIdentifier": "com.copypaste.app",
                "CFBundlePackageType": "APPL", "CFBundleVersion": "1",
            }))
            support = root / "persistent support"
            keychain = root / "device-secret.keychain-db"
            self.environment["COPYPASTE_KEYCHAIN_TEST_PATH"] = str(keychain)
            self.run_command("security", "create-keychain", "-p", "disposable-fixture", str(keychain))
            try:
                self.run_command("security", "unlock-keychain", "-p", "disposable-fixture", str(keychain))
                def compile_helper(optimization):
                    self.run_command("xcrun", "clang", optimization, "-Wno-deprecated-declarations",
                                     "-DCOPYPASTE_KEYCHAIN_TESTING", "-framework", "Security",
                                     "-framework", "CoreFoundation", str(ROOT / "packaging/macos/keychain-access.c"),
                                     "-o", str(binaries / "copypaste-keychain"))

                def compile_parent(version):
                    self.run_command("xcrun", "clang", f"-DFIXTURE_VERSION={version}",
                                     str(ROOT / "packaging/macos/tests/keychain-parent.c"),
                                     "-o", str(binaries / "copypaste-daemon"))
                    (binaries / "CopyPaste").write_bytes((binaries / "copypaste-daemon").read_bytes())
                    (binaries / "CopyPaste").chmod(0o755)

                def install(version):
                    temporary_home = root / f"temporary-home-{version}"
                    temporary_home.mkdir()
                    self.environment["HOME"] = str(temporary_home)
                    self.environment["COPYPASTE_SIGN_MODE"] = "selfsigned"
                    self.run_command("/bin/bash", str(ROOT / "packaging/macos/selfsign.sh"), str(app))
                    self.assertFalse((temporary_home / "Library").exists())
                    assert_preferences_unchanged()

                compile_parent(1)
                compile_helper("-Os")
                # The production path resolver must ignore Homebrew's fake HOME.
                resolved = self.run_command(str(binaries / "copypaste-keychain"), "support-directory").stdout.strip()
                import pwd
                self.assertEqual(resolved, str(Path(pwd.getpwuid(os.getuid()).pw_dir) /
                                               "Library/Application Support/com.copypaste.CopyPaste"))
                self.assertEqual(self.run_command(str(binaries / "copypaste-keychain"),
                                                 "home-directory").stdout.strip(),
                                 pwd.getpwuid(os.getuid()).pw_dir)
                self.environment["COPYPASTE_KEYCHAIN_TEST_SUPPORT_DIR"] = str(support)
                self.run_command("xattr", "-w", "com.apple.quarantine", "0081;00000000;CopyPasteFixture;",
                                 str(binaries / "copypaste-keychain"))
                install(1)
                helper = support / "signing/copypaste-keychain-v1"
                digest = hashlib.sha256(helper.read_bytes()).hexdigest()
                first_identity = (support / "signing/certificate-sha1").read_text()
                first_daemon = self.run_command("codesign", "-dvvv", str(binaries / "copypaste-daemon")).stderr
                self.run_command(str(binaries / "copypaste-daemon"), str(helper), "create")
                self.run_command(str(binaries / "copypaste-daemon"), str(helper), "load")
                # A genuinely different helper build must NOT replace the granted one.
                compile_parent(2)
                compile_helper("-O0")
                install(2)
                self.assertEqual(digest, hashlib.sha256(helper.read_bytes()).hexdigest())
                self.assertEqual(first_identity, (support / "signing/certificate-sha1").read_text())
                second_daemon = self.run_command("codesign", "-dvvv", str(binaries / "copypaste-daemon")).stderr
                self.assertNotEqual(first_daemon.split("CDHash=")[1].splitlines()[0],
                                    second_daemon.split("CDHash=")[1].splitlines()[0])
                self.run_command(str(binaries / "copypaste-daemon"), str(helper), "load")
                # Add must fail instead of overwriting an existing device key.
                duplicate = subprocess.run([str(binaries / "copypaste-daemon"), str(helper), "create"],
                                           env=self.environment, capture_output=True, timeout=10)
                self.assertEqual(duplicate.returncode, 2)
                self.run_command(str(binaries / "copypaste-daemon"), str(helper), "load")
                # A failed codesign must also restore the user's search list.
                daemon_binary = binaries / "copypaste-daemon"
                daemon_binary.chmod(0o000)
                failed_signing = subprocess.run(["/bin/bash", str(ROOT / "packaging/macos/selfsign.sh"),
                                                str(app)], env=self.environment,
                                               capture_output=True, timeout=30)
                self.assertEqual(failed_signing.returncode, 1,
                                 (failed_signing.stdout, failed_signing.stderr))
                self.assertIn(b"codesign rejected the certificate", failed_signing.stderr)
                assert_preferences_unchanged()
                daemon_binary.chmod(0o755)
                # Same executable/id, wrong signer: reject before Keychain access.
                self.run_command("codesign", "--force", "--sign", "-", "--identifier",
                                 "com.copypaste.copypaste-daemon", str(binaries / "copypaste-daemon"))
                rejected = subprocess.run([str(binaries / "copypaste-daemon"), str(helper), "load"],
                                          env=self.environment, capture_output=True, timeout=10)
                self.assertEqual(rejected.returncode, 5)
                self.assertEqual(rejected.stdout, b"")
                # An invalid retained helper must stop installation, not rotate it.
                helper.write_bytes(b"invalid retained helper")
                stopped = subprocess.run(["/bin/bash", str(ROOT / "packaging/macos/selfsign.sh"),
                                          str(app)], env=self.environment,
                                         capture_output=True, timeout=30)
                self.assertEqual(stopped.returncode, 1)
                self.assertEqual(helper.read_bytes(), b"invalid retained helper")
            finally:
                if "HOME" in os.environ:
                    self.environment["HOME"] = os.environ["HOME"]
                else:
                    self.environment.pop("HOME", None)
                self.run_command("security", "delete-keychain", str(keychain))
                for signing in (support / "signing").glob("*.keychain*"):
                    self.run_command("security", "delete-keychain", str(signing))
                self.assertEqual(search_before, self.run_command("security", "list-keychains", "-d", "user").stdout)
                self.assertEqual(default_before, self.run_command("security", "default-keychain", "-d", "user").stdout)


if __name__ == "__main__":
    unittest.main()
