#!/usr/bin/env bash
# Install an exact x86_64 RPM preview through DNF in a disposable Fedora host.
set -euo pipefail

ARTIFACT="${1:-}"
VERSION="${2:-}"
RECEIPT="${3:-}"
[[ ( "$#" == 2 || "$#" == 3 ) && "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "ERROR: usage: $0 <x86_64-rpm> <stable-version> [installation-receipt]" >&2; exit 1;
}
[[ -f "$ARTIFACT" && ! -L "$ARTIFACT" && -s "$ARTIFACT" &&
   "$(basename "$ARTIFACT")" == "CopyPaste-v$VERSION-linux-x86_64.rpm" ]] || {
  echo "ERROR: an exact, nonempty x86_64 RPM package is required" >&2; exit 1;
}
PACKAGE_DIRECTORY="$(cd "$(dirname "$ARTIFACT")" && pwd)"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EXPECTED_IMAGE="$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["rpm_preview_environment"]["image"])' "$ROOT/packaging/linux/release-contract.json")"
IMAGE="${COPYPASTE_RPM_VALIDATION_IMAGE:-$EXPECTED_IMAGE}"
if [[ -n "$RECEIPT" ]]; then
  [[ "${GITHUB_SHA:-}" =~ ^[0-9a-f]{40}$ && "${GITHUB_RUN_ID:-}" =~ ^[1-9][0-9]*$ && "$IMAGE" == "$EXPECTED_IMAGE" ]] || {
    echo "ERROR: RPM installation receipts require exact CI provenance and the pinned Fedora image" >&2; exit 1;
  }
fi
PACKAGE_SHA="$(python3 -c 'import hashlib, sys; print(hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest())' "$ARTIFACT")"

docker run --rm --platform linux/amd64 \
  --volume "$PACKAGE_DIRECTORY:/artifacts:ro" \
  --env "PACKAGE_NAME=$(basename "$ARTIFACT")" --env "VERSION=$VERSION" \
  "$IMAGE" bash -ceu '
    set -o pipefail
    [[ "$(rpm --eval "%fedora")" == 44 ]] || {
      echo "ERROR: RPM preview qualification requires Fedora 44" >&2; exit 1;
    }
    package="/artifacts/$PACKAGE_NAME"
    [[ "$(rpm -qp --queryformat "%{NAME} %{VERSION} %{ARCH}" "$package")" == "copypaste $VERSION x86_64" ]]
    # DNF must resolve every declared dependency. Never bypass its solver.
    dnf --assumeyes install "$package" binutils findutils file python3
    [[ "$(rpm -q --queryformat "%{VERSION} %{ARCH}" copypaste)" == "$VERSION x86_64" ]]
    rpm --verify copypaste
    python3 -c "import json; metadata=json.load(open(\"/usr/lib/copypaste/package-metadata.json\")); assert metadata == {\"schema\": 1, \"kind\": \"rpm\", \"architecture\": \"x86_64\"}"
    for executable in copypaste copypaste-daemon copypaste-cli; do
      test -x "/usr/lib/copypaste/$executable"
    done
    test "$(readlink -f /usr/bin/copypaste)" = /usr/lib/copypaste/copypaste
    test "$(readlink -f /usr/bin/copypaste-cli)" = /usr/lib/copypaste/copypaste-cli
    test -f /usr/share/applications/com.copypaste.CopyPaste.desktop
    test -f /usr/share/copypaste/autostart/com.copypaste.CopyPaste.desktop
    test ! -e /etc/xdg/autostart/com.copypaste.CopyPaste.desktop
    while IFS= read -r -d "" candidate; do
      if file --brief "$candidate" | grep -q "^ELF " && readelf --dynamic "$candidate" | grep -q NEEDED; then
        dependencies="$(ldd "$candidate")"
        printf "%s\n" "$dependencies"
        if [[ "$dependencies" == *"not found"* ]]; then
          echo "ERROR: unresolved runtime library in $candidate" >&2
          exit 1
        fi
      fi
    done < <(find /usr/lib/copypaste -type f -print0)
    # Keybinder is loaded dynamically by the X11 hotkey plugin.
    python3 -c "import ctypes; ctypes.CDLL(\"libkeybinder-3.0.so.0\")"
    [[ "$(/usr/bin/copypaste --version)" == "$VERSION" ]]
    # Test package-manager removal and installation of the same immutable bytes.
    dnf --assumeyes remove copypaste
    ! rpm -q copypaste
    test ! -e /usr/lib/copypaste/copypaste
    dnf --assumeyes install "$package"
    rpm --verify copypaste
    [[ "$(/usr/bin/copypaste --version)" == "$VERSION" ]]
  '
if [[ -n "$RECEIPT" ]]; then
  python3 - "$ROOT" "$ARTIFACT" "$VERSION" "$RECEIPT" "$PACKAGE_SHA" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import runpy
import sys

root, artifact, version, receipt, tested_sha = sys.argv[1:]
contract = runpy.run_path(str(Path(root) / "scripts/release/linux_release_contract.py"))
package = Path(artifact)
digest = hashlib.sha256(package.read_bytes()).hexdigest()
if digest != tested_sha:
    raise ValueError("RPM bytes changed during installation qualification")
payload = {
    "schema": 1, "qualification": "package_installation_only",
    "version": version, "architecture": "x86_64",
    "commit": os.environ["GITHUB_SHA"], "run_id": os.environ["GITHUB_RUN_ID"],
    "artifact": {"name": package.name, "bytes": package.stat().st_size, "sha256": digest},
    "environment": contract["CONTRACT"]["rpm_preview_environment"],
    "assertions": {name: True for name in contract["RPM_PREVIEW_ASSERTIONS"]},
}
Path(receipt).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY
fi
