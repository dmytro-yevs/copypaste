"""Read the declared Linux release scope without removing experimental targets."""

import json
from itertools import product
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = json.loads((ROOT / "packaging/linux/release-contract.json").read_text(encoding="utf-8"))
ARCHITECTURES = tuple(CONTRACT["architectures"])
DESKTOPS = tuple(CONTRACT["desktops"])
SESSIONS = tuple(CONTRACT["sessions"])
FORMATS = tuple(CONTRACT["formats"])
PREVIEW_FORMATS = tuple(CONTRACT.get("preview_formats", ()))
PUBLISHED_FORMATS = FORMATS + PREVIEW_FORMATS
TARGETS = tuple(CONTRACT.get("targets", ()))
APPIMAGETOOL = CONTRACT["appimagetool"]
CAPABILITIES = {
    "clipboardCapture", "clipboardReadWrite", "sourceAttribution",
    "privacyEnforcement", "globalShortcuts", "quickPaste",
    "secureStorage", "desktopIntegration", "updates",
}
RPM_PREVIEW_ASSERTIONS = (
    "dnf_install", "rpm_verify", "elf_dependencies", "keybinder", "version",
    "remove", "reinstall",
)
BASELINES = json.loads(
    (ROOT / "packaging/linux/compositor-runtime/release-contract.json").read_text(encoding="utf-8")
)["baselines"]

if CONTRACT.get("schema") != 1 or not all((ARCHITECTURES, DESKTOPS, SESSIONS, FORMATS, BASELINES)):
    raise ValueError("Linux release contract must declare a nonempty supported scope")
for values, allowed in (
    (ARCHITECTURES, {"x86_64", "aarch64"}),
    (DESKTOPS, {"GNOME", "KDE"}),
    (SESSIONS, {"x11", "wayland"}),
    (FORMATS, {"AppImage", "deb", "rpm"}),
    (PREVIEW_FORMATS, {"rpm"}),
):
    if len(set(values)) != len(values) or not set(values) <= allowed:
        raise ValueError("Linux release contract contains duplicate or unsupported targets")
if set(FORMATS) & set(PREVIEW_FORMATS) or (PREVIEW_FORMATS and ARCHITECTURES != ("x86_64",)):
    raise ValueError("Linux previews require distinct formats and the x86_64 architecture")
if {(baseline["desktop"], baseline["architecture"]) for baseline in BASELINES} != {
    (desktop, architecture) for desktop in DESKTOPS for architecture in ARCHITECTURES
}:
    raise ValueError("Linux compositor baselines differ from the declared release scope")

if not TARGETS or len({target["id"] for target in TARGETS}) != len(TARGETS):
    raise ValueError("Linux platform targets must have unique IDs")
for target in TARGETS:
    if (target["tier"] not in {"stable", "experimental"}
            or target["session"] not in {"x11", "wayland"}
            or target["architecture"] not in {"x86_64", "aarch64"}
            or target["desktop"] not in {"GNOME", "KDE"}
            or not target["packaging"]
            or not set(target["packaging"]) <= set(PUBLISHED_FORMATS)
            or not target["required_capabilities"]
            or not set(target["required_capabilities"]) <= CAPABILITIES
            or type(target["native_qualification"]) is not bool
            or not target["test_environment"]["runner"]
            or not target["test_environment"]["image"]):
        raise ValueError(f"Invalid Linux platform target: {target['id']}")
    if target["tier"] == "stable" and not target["native_qualification"]:
        raise ValueError("Stable Linux targets require native qualification")
    if target["native_qualification"] and not target.get("compositor_artifact"):
        raise ValueError("Native qualification requires an exact compositor artifact")
    tool = APPIMAGETOOL[target["architecture"]]
    if (not tool["url"].startswith("https://github.com/AppImage/appimagetool/")
            or len(tool["sha256"]) != 64
            or any(character not in "0123456789abcdef" for character in tool["sha256"])):
        raise ValueError("Linux AppImageTool source is not pinned")
stable = tuple(target for target in TARGETS if target["tier"] == "stable")
expected_stable = set(product(DESKTOPS, SESSIONS, ARCHITECTURES))
declared_stable = {
    (target["desktop"], target["session"], target["architecture"])
    for target in stable
}
if (len(stable) != len(expected_stable) or declared_stable != expected_stable
        or any(
            (target["distribution"], target["version"], target["desktop_version"])
            != (CONTRACT["qualification_environment"]["distribution"],
                CONTRACT["qualification_environment"]["version"],
                CONTRACT["qualification_environment"]["desktop_version"])
            or tuple(target["packaging"]) != FORMATS
            for target in stable
        )):
    raise ValueError("Stable targets differ from the declared release and qualification scope")
