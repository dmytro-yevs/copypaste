import hashlib
import importlib.util
import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[3]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"scripts/release/{name.replace('_', '-')}.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


producer = load("produce_linux_native_qualification")
verifier = load("verify_linux_native_qualification")


def media_records(runtime_format="AppImage"):
    digest = lambda value: hashlib.sha256(value.encode("utf-8")).hexdigest()
    dependency = (
        {"origin": "appimage_manifest", "gstreamer_library": {"sha256": "d" * 64}, "v4l2_plugin": {"sha256": "e" * 64}}
        if runtime_format == "AppImage" else
        {"origin": "distro_package", "gstreamer_library": {"package": "gstreamer1", "version": "1.0", "sha256": "d" * 64}, "v4l2_plugin": {"package": "gstreamer1-plugins-good", "version": "1.0", "sha256": "e" * 64}}
    )
    return [
        {"schema": 1, "kind": "atspi_click", "runtime_format": runtime_format, "gui_pid": 42, "target_name_sha256": digest("Scan QR"), "expected_name_sha256": digest("Scan QR code"), "action": "click"},
        {"schema": 1, "kind": "v4l2_qr_feed", "runtime_format": runtime_format, "device": "video42", "frame_format": "yuyv422", "source": "camera_desktop", "decoder": "CameraPluginPairingSource/ZxingQrFrameDecoder", "payload": {"sha256": "a" * 64, "size_bytes": 45}},
        {"schema": 1, "kind": "atspi_click", "runtime_format": runtime_format, "gui_pid": 42, "target_name_sha256": digest("Start scanner"), "expected_name_sha256": digest("Confirm on both devices"), "action": "click"},
        {"schema": 1, "kind": "camera_qr_scan_observed", "runtime_format": runtime_format, "gui_pid": 42, "pairing_state_name_sha256": digest("Confirm on both devices"), "gstreamer_mapping_sha256": "b" * 64, "camera_plugin_sha256": "c" * 64, "gstreamer_library_sha256": "d" * 64, "v4l2_plugin_sha256": "e" * 64, "dependency": dependency},
        {"schema": 1, "kind": "v4l2_qr_feed_stopped", "runtime_format": runtime_format, "device": "video42"},
    ]


class LinuxCameraQualificationEvidenceTest(unittest.TestCase):
    def test_producer_requires_the_complete_real_camera_media_sequence(self):
        records = media_records() + media_records("deb")
        output = "\n".join(
            producer.MEDIA_PREFIX + json.dumps(record, separators=(",", ":"))
            for record in records
        )
        self.assertEqual(producer.media_rows(output, ["AppImage", "deb"]), records)

    def test_verifier_rejects_camera_cleanup_for_a_different_device(self):
        records = media_records() + media_records("deb")
        records[-1]["device"] = "video43"
        with self.assertRaisesRegex(ValueError, "different device"):
            verifier.verify_camera_media(records, [{"format": "AppImage"}, {"format": "deb"}], "row.trace.json")

    def test_producer_rejects_a_missing_native_runtime_camera_sequence(self):
        output = "\n".join(
            producer.MEDIA_PREFIX + json.dumps(record, separators=(",", ":"))
            for record in media_records()
        )
        with self.assertRaisesRegex(ValueError, "incomplete"):
            producer.media_rows(output, ["AppImage", "deb"])

    def test_producer_rejects_dependency_origin_and_mapped_hash_mismatches(self):
        records = media_records() + media_records("deb")
        records[3]["dependency"]["origin"] = "distro_package"
        output = "\n".join(producer.MEDIA_PREFIX + json.dumps(record, separators=(",", ":")) for record in records)
        with self.assertRaisesRegex(ValueError, "origin"):
            producer.media_rows(output, ["AppImage", "deb"])
        records = media_records() + media_records("deb")
        records[3]["dependency"]["gstreamer_library"]["sha256"] = "f" * 64
        with self.assertRaisesRegex(ValueError, "hashes"):
            verifier.verify_camera_media(records, [{"format": "AppImage"}, {"format": "deb"}], "row.trace.json")

    def test_multiarch_native_dependency_passes_producer_and_verifier(self):
        records = media_records() + media_records("deb")
        for item in records[8]["dependency"].values():
            if isinstance(item, dict) and "package" in item:
                item["package"] = "libgstreamer1.0-0:amd64"
        output = "\n".join(producer.MEDIA_PREFIX + json.dumps(record, separators=(",", ":")) for record in records)
        self.assertEqual(producer.media_rows(output, ["AppImage", "deb"]), records)
        verifier.verify_camera_media(records, [{"format": "AppImage"}, {"format": "deb"}], "row.trace.json")

    def test_camera_and_external_drag_are_mandatory_assertions(self):
        self.assertIn("camera_qr_scan", producer.ASSERTIONS)
        self.assertIn("external_file_drag", producer.ASSERTIONS)
        self.assertIn("camera_qr_scan", verifier.ASSERTIONS)
        self.assertIn("external_file_drag", verifier.ASSERTIONS)


if __name__ == "__main__":
    unittest.main()
