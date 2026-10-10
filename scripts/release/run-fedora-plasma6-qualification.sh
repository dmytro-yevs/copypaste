#!/usr/bin/env bash
# Build the pinned Fedora Plasma 6 runtime and execute one native scenario.
set -euo pipefail

SESSION=""
ARCHITECTURE=""
ARTIFACTS=""
EVIDENCE=""
PREVIOUS_ARTIFACTS=""
COMPANION_SOURCE=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --session) SESSION="$2"; shift 2 ;;
    --architecture) ARCHITECTURE="$2"; shift 2 ;;
    --artifacts) ARTIFACTS="$2"; shift 2 ;;
    --evidence) EVIDENCE="$2"; shift 2 ;;
    --previous-artifacts) PREVIOUS_ARTIFACTS="$2"; shift 2 ;;
    --companion-source) COMPANION_SOURCE="$2"; shift 2 ;;
    --) shift; break ;;
    *) echo "ERROR: unknown Fedora qualification argument: $1" >&2; exit 2 ;;
  esac
done
[[ "$SESSION" == x11 || "$SESSION" == wayland ]]
[[ "$ARCHITECTURE" == x86_64 || "$ARCHITECTURE" == aarch64 ]]
[[ -d "$ARTIFACTS" && -d "$EVIDENCE" && "$#" -gt 0 ]]
[[ -z "$PREVIOUS_ARTIFACTS" || -d "$PREVIOUS_ARTIFACTS" ]]
[[ -d "$COMPANION_SOURCE" ]]

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
case "$ARCHITECTURE" in
  x86_64) platform=linux/amd64 ;;
  aarch64) platform=linux/arm64 ;;
esac
image="copypaste-linux-native-qualification:fedora40-$ARCHITECTURE"
docker build --platform "$platform" --file "$ROOT/packaging/linux/Dockerfile.fedora-native-qualification" --tag "$image" "$ROOT"
mounts=(--volume "$ROOT:/work:ro" --volume "$(cd "$ARTIFACTS" && pwd):/artifacts:ro" --volume "$(cd "$EVIDENCE" && pwd):/evidence")
mounts+=(--volume "$(cd "$COMPANION_SOURCE" && pwd):/companion:ro")
if [[ -n "$PREVIOUS_ARTIFACTS" ]]; then
  mounts+=(--volume "$(cd "$PREVIOUS_ARTIFACTS" && pwd):/previous:ro")
fi
docker run --rm --platform "$platform" \
  --cap-add SYS_NICE "${mounts[@]}" --workdir /work "$image" \
  /work/scripts/release/run-fedora-plasma6-session.sh --session "$SESSION" --companion-source /companion -- "$@"
