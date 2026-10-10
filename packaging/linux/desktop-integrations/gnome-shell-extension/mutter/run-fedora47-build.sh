#!/usr/bin/env bash
set -euo pipefail
platform=${1:?usage: $0 linux/amd64|linux/arm64}
case "$platform" in linux/amd64|linux/arm64) ;; *) exit 2;; esac
root="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/home"
engine="${COPYPASTE_CONTAINER_ENGINE:-docker}"
"$engine" build --platform "$platform" --file "$root/Dockerfile.fedora41-build" --tag copypaste-mutter47-builder:fedora41 "$root"
"$engine" run --rm --platform "$platform" --user "$(id -u):$(id -g)" \
  --env HOME=/work/home --volume "$root:/workspace:ro" \
  --volume "$root/../native:/shim-source:ro" --volume "$work:/work" \
  copypaste-mutter47-builder:fedora41 bash -ceu '
    cp -a /shim-source /work/shim
    make -C /work/shim MUTTER_PKG=libmutter-15
    bash /workspace/verify-patch.sh 47 /work/source /work/build
  '
