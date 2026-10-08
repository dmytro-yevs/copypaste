import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("android_builder", Path(__file__).parents[1] / "build-android-module.py")
builder = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(builder)


class AndroidToolchainTest(unittest.TestCase):
    def test_ndk_archiver_and_indexer_are_bound_for_each_android_abi(self):
        for architecture, (target, _) in builder.TARGETS.items():
            with self.subTest(architecture=architecture), tempfile.TemporaryDirectory() as directory:
                ndk = Path(directory)
                (ndk / "source.properties").write_text("Pkg.Revision = 29.0.13846066")
                argv = ["build", "--architecture", architecture, "--ndk", str(ndk)]
                result = subprocess.CompletedProcess([], 0, " LOAD 0 0 0 0 0 0 0x4000\n")
                with patch.object(sys, "argv", argv), patch.object(builder.platform, "system", return_value="Linux"), patch.object(builder.subprocess, "run", return_value=result) as run:
                    builder.main()
                environment = run.call_args_list[0].kwargs["env"]
                tools = ndk / "toolchains/llvm/prebuilt/linux-x86_64/bin"
                suffix = target.replace("-", "_")
                self.assertEqual(environment["AR_" + suffix], str(tools / "llvm-ar"))
                self.assertEqual(environment["RANLIB_" + suffix], str(tools / "llvm-ranlib"))
                self.assertIn("max-page-size=16384", environment["RUSTFLAGS"])
