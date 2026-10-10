#!/usr/bin/env bash
# Build and run the disposable Fedora RPM metadata smoke on a hosted runner.
set -euo pipefail

root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
docker build --platform linux/amd64 --build-arg KWIN_FAMILY=6.0 \
  --file "$root/packaging/linux/desktop-integrations/kde-native-clipboard/Dockerfile.fedora40-build" \
  --tag copypaste-fedora-closure-smoke:6.0 "$root"
docker run --rm --platform linux/amd64 --volume "$root:/workspace:ro" \
  copypaste-fedora-closure-smoke:6.0 \
  bash /workspace/scripts/release/diagnostics/fedora-closure-smoke-container.sh
