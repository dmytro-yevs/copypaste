"""Read the declared Linux release scope without removing experimental targets."""

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTRACT = json.loads((ROOT / "packaging/linux/release-contract.json").read_text(encoding="utf-8"))
ARCHITECTURES = tuple(CONTRACT["architectures"])
DESKTOPS = tuple(CONTRACT["desktops"])
SESSIONS = tuple(CONTRACT["sessions"])
FORMATS = tuple(CONTRACT["formats"])
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
):
    if len(set(values)) != len(values) or not set(values) <= allowed:
        raise ValueError("Linux release contract contains duplicate or unsupported targets")
if {(baseline["desktop"], baseline["architecture"]) for baseline in BASELINES} != {
    (desktop, architecture) for desktop in DESKTOPS for architecture in ARCHITECTURES
}:
    raise ValueError("Linux compositor baselines differ from the declared release scope")
