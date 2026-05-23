#!/usr/bin/env bash
# regen-uniffi.sh — Regenerate Kotlin UniFFI bindings for copypaste-android.
#
# This is the canonical entrypoint for regenerating bindings after editing
# the UDL file or the Rust UniFFI scaffold. It wraps the lower-level
# `scripts/generate-android-bindings.sh` with safety checks, dry-run mode,
# and output validation.
#
# Run this manually when:
#   - crates/copypaste-android/uniffi/copypaste_android.udl changes
#   - The Rust API surface in crates/copypaste-android/src/lib.rs changes
#   - You see "UniFFI scaffolding mismatch" errors at Android runtime
#
# See docs/uniffi/README.md for full guidance.

set -euo pipefail

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

CRATE_DIR="${REPO_ROOT}/crates/copypaste-android"
UDL_FILE="${CRATE_DIR}/uniffi/copypaste_android.udl"
OUT_DIR="${REPO_ROOT}/android/app/src/main/java/com/copypaste/generated"
BINDGEN="${REPO_ROOT}/target/debug/uniffi-bindgen"

# ---------------------------------------------------------------------------
# Flags
# ---------------------------------------------------------------------------
DRY_RUN=0
VERBOSE=0

usage() {
  cat <<'EOF'
regen-uniffi.sh — Regenerate Kotlin UniFFI bindings for copypaste-android.

USAGE:
  scripts/regen-uniffi.sh [OPTIONS]

OPTIONS:
  -h, --help       Show this help message and exit.
  -n, --dry-run    Print what would be done without building or writing files.
  -v, --verbose    Print every command before executing it.

WHAT IT DOES:
  1. Verifies that the UDL file exists at:
       crates/copypaste-android/uniffi/copypaste_android.udl
  2. Builds the copypaste-android cdylib and the uniffi-bindgen binary.
  3. Runs uniffi-bindgen to emit Kotlin sources into:
       android/app/src/main/java/com/copypaste/generated/
  4. Validates the output:
       - At least one .kt file exists.
       - The main binding file is non-trivial (>100 bytes).
       - If ktlint is installed, runs a syntax check.

WHEN TO RUN:
  After editing copypaste_android.udl, the Rust UniFFI scaffold, or after a
  Rust dependency bump that updates the uniffi crate version. See
  docs/uniffi/README.md for the full list of triggers and troubleshooting.

EXIT CODES:
  0  success (or dry-run completed)
  1  UDL file missing
  2  cargo build failed
  3  uniffi-bindgen invocation failed
  4  validation failed (no output, or output too small / invalid)
EOF
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    -n|--dry-run)
      DRY_RUN=1
      shift
      ;;
    -v|--verbose)
      VERBOSE=1
      shift
      ;;
    *)
      echo "error: unknown flag: $1" >&2
      echo "Run with --help for usage." >&2
      exit 64
      ;;
  esac
done

if [[ "${VERBOSE}" -eq 1 ]]; then
  set -x
fi

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
log() {
  printf '==> %s\n' "$*"
}

run() {
  # Execute a command, or just print it in dry-run mode.
  if [[ "${DRY_RUN}" -eq 1 ]]; then
    printf '    [dry-run] %s\n' "$*"
  else
    "$@"
  fi
}

# ---------------------------------------------------------------------------
# Step 1: Locate UDL
# ---------------------------------------------------------------------------
log "Looking for UDL: ${UDL_FILE}"
if [[ ! -f "${UDL_FILE}" ]]; then
  echo "error: UDL file not found at ${UDL_FILE}" >&2
  echo "       Did the crate move? Update UDL_FILE in this script." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Step 2: Build cdylib + bindgen binary
# ---------------------------------------------------------------------------
log "Building copypaste-android cdylib (debug)"
if ! run cargo build -p copypaste-android; then
  echo "error: cargo build -p copypaste-android failed" >&2
  exit 2
fi

log "Building uniffi-bindgen binary"
if ! run cargo build -p copypaste-android --bin uniffi-bindgen; then
  echo "error: cargo build of uniffi-bindgen failed" >&2
  exit 2
fi

# ---------------------------------------------------------------------------
# Step 3: Run bindgen
# ---------------------------------------------------------------------------
log "Ensuring output directory exists: ${OUT_DIR}"
run mkdir -p "${OUT_DIR}"

log "Running uniffi-bindgen generate (kotlin)"
if [[ "${DRY_RUN}" -eq 1 ]]; then
  printf '    [dry-run] (cd %s && %s generate %s --language kotlin --out-dir %s)\n' \
    "${CRATE_DIR}" "${BINDGEN}" "${UDL_FILE}" "${OUT_DIR}"
else
  if [[ ! -x "${BINDGEN}" ]]; then
    echo "error: uniffi-bindgen binary not found at ${BINDGEN}" >&2
    echo "       The cargo build step should have produced it. Check build output." >&2
    exit 3
  fi
  if ! ( cd "${CRATE_DIR}" && \
         "${BINDGEN}" generate "${UDL_FILE}" \
            --language kotlin \
            --out-dir "${OUT_DIR}" ); then
    echo "error: uniffi-bindgen generate failed" >&2
    exit 3
  fi
fi

# ---------------------------------------------------------------------------
# Step 4: Validate output
# ---------------------------------------------------------------------------
if [[ "${DRY_RUN}" -eq 1 ]]; then
  log "Dry-run complete. No files were written. Skipping validation."
  exit 0
fi

log "Validating generated bindings"

# 4a. At least one .kt file emitted.
KT_FILES=()
while IFS= read -r -d '' f; do
  KT_FILES+=("$f")
done < <(find "${OUT_DIR}" -type f -name '*.kt' -print0)

if [[ "${#KT_FILES[@]}" -eq 0 ]]; then
  echo "error: no .kt files emitted into ${OUT_DIR}" >&2
  exit 4
fi

# 4b. Main binding non-trivial (>100 bytes).
MAIN_BINDING=""
for f in "${KT_FILES[@]}"; do
  base="$(basename "$f")"
  if [[ "${base}" == "copypaste_android.kt" ]]; then
    MAIN_BINDING="$f"
    break
  fi
done
# Fall back to the largest emitted file if the canonical name is missing.
if [[ -z "${MAIN_BINDING}" ]]; then
  MAIN_BINDING="$(ls -1S "${KT_FILES[@]}" | head -n1)"
fi

# Portable file-size check (works on macOS BSD stat and GNU stat).
if SIZE="$(stat -f%z "${MAIN_BINDING}" 2>/dev/null)"; then
  :
elif SIZE="$(stat -c%s "${MAIN_BINDING}" 2>/dev/null)"; then
  :
else
  echo "error: could not stat ${MAIN_BINDING}" >&2
  exit 4
fi

if [[ "${SIZE}" -lt 100 ]]; then
  echo "error: main binding ${MAIN_BINDING} is suspiciously small (${SIZE} bytes)" >&2
  exit 4
fi

# 4c. Optional ktlint syntax check.
if command -v ktlint >/dev/null 2>&1; then
  log "Running ktlint on generated sources"
  if ! ktlint --relative "${OUT_DIR}" >/dev/null 2>&1; then
    echo "warning: ktlint reported style issues in generated bindings" >&2
    echo "         (generated code; safe to ignore unless syntax errors)" >&2
  fi
else
  log "ktlint not installed; skipping syntax check (size check passed)"
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
log "Done. ${#KT_FILES[@]} Kotlin file(s) written to:"
printf '    %s\n' "${OUT_DIR}"
log "Files:"
for f in "${KT_FILES[@]}"; do
  printf '    %s\n' "$f"
done
