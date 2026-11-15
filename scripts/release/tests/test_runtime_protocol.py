import io
import json
from pathlib import Path
import re
import runpy
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]


class RuntimeProtocolTest(unittest.TestCase):
    def test_native_and_module_clients_use_the_current_daemon_wire_contract(self):
        source = (ROOT / "crates/copypaste-ipc/src/lib.rs").read_text()
        expected = int(re.search(r"pub const PROTOCOL_VERSION: u32 = ([0-9]+);", source)[1])
        driver = runpy.run_path(str(ROOT / "scripts/release/linux-native-fixture-driver.py"))
        modules = runpy.run_path(str(ROOT / "scripts/release/linux-module-qualification.py"))
        requests = []

        class Channel:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                pass

            def settimeout(self, _timeout):
                pass

            def connect(self, _path):
                pass

            def sendall(self, packet):
                self.request = json.loads(packet)
                requests.append(self.request)

            def makefile(self, _mode):
                accepted = self.request["protocol_version"] == expected
                return io.BytesIO(json.dumps({
                    "id": self.request["id"], "ok": accepted,
                    "error_code": None if accepted else "protocol_mismatch", "data": {"empty": {}},
                }).encode() + b"\n")

        with mock.patch.object(driver["socket"], "socket", side_effect=lambda *_args: Channel()):
            self.assertTrue(driver["ipc"]("runtime.sock", "save_file", {"id": "clip", "dest_path": "/saved"})["ok"])
            response, _trace = modules["rpc"]("runtime.sock", {"operation": "list"})
            self.assertTrue(response["ok"])
        self.assertEqual([request["protocol_version"] for request in requests], [expected, expected])

    def test_missing_or_ambiguous_authoritative_versions_fail_closed(self):
        helper = ROOT / "scripts/release/runtime_protocol.py"
        for source in ("", "pub const PROTOCOL_VERSION: u32 = 0;", "pub const PROTOCOL_VERSION: u32 = 6;\npub const PROTOCOL_VERSION: u32 = 7;"):
            with self.subTest(source=source), mock.patch.object(Path, "read_text", return_value=source):
                with self.assertRaisesRegex(ValueError, "one protocol version"):
                    runpy.run_path(str(helper))


if __name__ == "__main__":
    unittest.main()
