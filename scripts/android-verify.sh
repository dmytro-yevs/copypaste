#!/usr/bin/env bash
# =============================================================================
# android-verify.sh — deterministic Android build/test chain for CopyPaste.
# =============================================================================
#
# WHAT THIS IS NOT:
#   This script is NOT a substitute for real-device testing. It runs codegen,
#   a cross-compiled native build, an APK assemble, and JVM-level unit tests.
#   None of that exercises CopyPaste on actual Android hardware:
#     - emulator/unit tests != hardware. JVM unit tests run on your desktop
#       JDK, not on Dalvik/ART, not on a phone's clipboard, not on a real
#       Keystore, not on a real radio/network stack.
#     - the instrumented cross-language crypto conformance suite
#       (CryptoConformanceTest.kt) runs only via
#       `./gradlew connectedDebugAndroidTest` against a device/emulator and is
#       DELIBERATELY NOT part of this chain — it needs a connected device this
#       script cannot assume exists.
#   Because of that, this script NEVER prints "verified" and NEVER prints
#   "release-ready". A GREEN result here means "the Android toolchain built and
#   the JVM unit tests passed", nothing more. Ship decisions still require
#   manual testing on real Android devices.
#
# STEPS (HALT on the first failure, naming the step):
#   1) Regenerate UniFFI Kotlin bindings  (scripts/regen-uniffi.sh)
#   2) Build the Android native .so       (make android-so, via cargo-ndk/NDK)
#   3) Assemble the debug APK             (android/ ./gradlew assembleDebug)
#   4) Kotlin/JVM unit tests              (android/ ./gradlew :app:testDebugUnitTest)
#
# PRECONDITIONS:
#   - Clean git tree. Android codegen and builds must run on a clean tree so
#     that regenerated bindings / produced artifacts are attributable and the
#     run is reproducible. The script REFUSES to start on a dirty tree.
#   - Rust toolchain (cargo) with the aarch64-linux-android target.
#   - cargo-ndk + Android NDK for step 2 (absence is a hard FAIL, not a skip).
#   - A JDK + Android SDK for steps 3 and 4 (gradle resolves these).
#
# USAGE:
#   scripts/android-verify.sh
#
# EXIT CODES:
#   0  ANDROID VERIFY GREEN (all four steps passed)
#   1  ANDROID VERIFY FAILED (a step failed; the failing step is named)
#   2  refused to start (dirty git tree, or run from the wrong directory)
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Rust toolchain PATH — source ~/.cargo/env when cargo is not already on PATH.
# rustup installs cargo to ~/.cargo/bin but login-shell PATH is not always
# inherited by subshells / IDE terminals / Bash invoked via exec.
# ---------------------------------------------------------------------------
if ! command -v cargo >/dev/null 2>&1; then
  if [[ -f "${HOME}/.cargo/env" ]]; then
    # shellcheck source=/dev/null
    source "${HOME}/.cargo/env"
  fi
fi

# ---------------------------------------------------------------------------
# Colours (disabled when stdout is not a TTY, e.g. under CI log capture).
# ---------------------------------------------------------------------------
if [[ -t 1 ]]; then
  RED='\033[0;31m'
  GREEN='\033[0;32m'
  YELLOW='\033[1;33m'
  NC='\033[0m'
else
  RED=''
  GREEN=''
  YELLOW=''
  NC=''
fi

# ---------------------------------------------------------------------------
# Paths — resolve the repo root from this script's location so the script can
# be invoked from anywhere.
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ANDROID_DIR="${REPO_ROOT}/android"

# ---------------------------------------------------------------------------
# Reporting helpers.
# ---------------------------------------------------------------------------
step_pass() {
  printf "${GREEN}✓ %s${NC}\n" "$1"
}

step_fail() {
  printf "${RED}✗ %s${NC}\n" "$1"
}

note() {
  printf "${YELLOW}%s${NC}\n" "$1"
}

# fail <step-name> — print the failure banner and exit non-zero. Used by every
# step so the chain HALTS on the first failure and always names the step.
fail() {
  local step="$1"
  step_fail "${step}"
  echo ""
  printf "${RED}ANDROID VERIFY FAILED: %s${NC}\n" "${step}"
  exit 1
}

# ---------------------------------------------------------------------------
# Preflight: must run from the repo, on a clean git tree.
# ---------------------------------------------------------------------------
cd "${REPO_ROOT}"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  step_fail "preflight: not inside a git work tree"
  echo "  Run this script from within the CopyPaste git repository." >&2
  exit 2
fi

if [[ ! -d "${ANDROID_DIR}" ]]; then
  step_fail "preflight: android/ directory not found at ${ANDROID_DIR}"
  exit 2
fi

if [[ ! -x "${ANDROID_DIR}/gradlew" ]]; then
  step_fail "preflight: ${ANDROID_DIR}/gradlew is missing or not executable"
  exit 2
fi

# Refuse to start on a dirty tree. `git status --porcelain` is empty iff the
# working tree and index are clean (tracked changes + staged changes). We
# intentionally include untracked files too — stray generated bindings or
# build leftovers would make the run non-reproducible.
#
# ANDROID_VERIFY_ALLOW_DIRTY=1 bypasses this check when the caller has
# intentional staged changes (e.g. staged deletions) that do not affect the
# Android build surface. The caller accepts full responsibility for
# reproducibility. Do NOT use in automated CI.
if [[ -n "$(git status --porcelain)" ]]; then
  if [[ "${ANDROID_VERIFY_ALLOW_DIRTY:-0}" == "1" ]]; then
    note "WARNING: dirty tree bypass active (ANDROID_VERIFY_ALLOW_DIRTY=1) — caller accepts reproducibility responsibility."
  else
    step_fail "preflight: git tree is dirty"
    echo "" >&2
    echo "  Refusing to run: Android codegen and cross-compiled builds must run" >&2
    echo "  on a CLEAN tree so regenerated bindings and produced artifacts are" >&2
    echo "  attributable and the run is reproducible." >&2
    echo "" >&2
    echo "  Commit or stash your changes first:" >&2
    echo "    git status" >&2
    echo "    git stash --include-untracked   # or commit" >&2
    echo "" >&2
    echo "  If you have intentional staged changes not touching the Android surface," >&2
    echo "  you may bypass with: ANDROID_VERIFY_ALLOW_DIRTY=1 scripts/android-verify.sh" >&2
    exit 2
  fi
fi

note "Android toolchain build/test chain — NOT a substitute for real-device testing."
echo ""

# ---------------------------------------------------------------------------
# Step 1: Regenerate UniFFI Kotlin bindings.
#
# scripts/regen-uniffi.sh is the canonical bindings entrypoint (it wraps
# scripts/generate-android-bindings.sh with safety checks + output validation).
# ---------------------------------------------------------------------------
STEP1="step 1: regenerate UniFFI bindings (scripts/regen-uniffi.sh)"
echo "==> ${STEP1}"
if [[ ! -x "${REPO_ROOT}/scripts/regen-uniffi.sh" ]]; then
  echo "  ${REPO_ROOT}/scripts/regen-uniffi.sh not found or not executable." >&2
  fail "${STEP1}"
fi
if ! "${REPO_ROOT}/scripts/regen-uniffi.sh"; then
  fail "${STEP1}"
fi
step_pass "${STEP1}"
echo ""

# ---------------------------------------------------------------------------
# Step 2: Build the Android native .so (arm64-v8a + x86_64) via cargo-ndk.
#
# `make android-so` already guards on cargo-ndk and prints install guidance,
# but it does NOT verify the NDK itself. We add an explicit hard FAIL for both
# cargo-ndk and the NDK so a missing toolchain is loud, never a silent skip.
# ---------------------------------------------------------------------------
STEP2="step 2: build Android .so (make android-so / cargo-ndk)"
echo "==> ${STEP2}"

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "  cargo-ndk not found. Install it and the Android Rust targets:" >&2
  echo "    cargo install cargo-ndk" >&2
  echo "    rustup target add aarch64-linux-android x86_64-linux-android" >&2
  fail "${STEP2}: cargo-ndk not found"
fi

# Detect an NDK. cargo-ndk discovers the NDK via ANDROID_NDK_HOME,
# ANDROID_NDK_ROOT, or an ndk/<version> dir under ANDROID_HOME / the usual SDK
# locations. We check the env vars and the common SDK paths so an absent NDK is
# a clear FAIL rather than an opaque cargo-ndk error mid-build.
ndk_present() {
  [[ -n "${ANDROID_NDK_HOME:-}" && -d "${ANDROID_NDK_HOME}" ]] && return 0
  [[ -n "${ANDROID_NDK_ROOT:-}" && -d "${ANDROID_NDK_ROOT}" ]] && return 0
  local sdk
  for sdk in \
    "${ANDROID_HOME:-}" \
    "${ANDROID_SDK_ROOT:-}" \
    "${HOME}/Library/Android/sdk" \
    "/opt/homebrew/share/android-commandlinetools" \
    "/usr/local/share/android-commandlinetools"; do
    [[ -n "${sdk}" && -d "${sdk}/ndk" ]] && \
      [[ -n "$(ls -A "${sdk}/ndk" 2>/dev/null)" ]] && return 0
  done
  return 1
}

if ! ndk_present; then
  echo "  Android NDK not found." >&2
  echo "  Install it and point cargo-ndk at it, e.g.:" >&2
  echo "    sdkmanager 'ndk;27.2.12479018'" >&2
  echo "    export ANDROID_NDK_HOME=\$ANDROID_HOME/ndk/27.2.12479018" >&2
  echo "  (Android Studio: SDK Manager -> SDK Tools -> NDK.)" >&2
  fail "${STEP2}: cargo-ndk/NDK not found — install the Android NDK"
fi

# Auto-export ANDROID_HOME / ANDROID_NDK_HOME when the SDK/NDK exist in
# well-known locations but the env vars are not set.
#   - ANDROID_HOME: Gradle (AGP) + the NDK discovery loop below both need it.
#   - ANDROID_NDK_HOME: cargo-ndk requires this (does NOT auto-discover from
#     ANDROID_HOME/ndk/). Without it the ndk_present() check above passes but
#     cargo-ndk still fails with "Could not find any NDK".
_SDK_SEARCH_PATHS=(
  "${ANDROID_HOME:-}"
  "${ANDROID_SDK_ROOT:-}"
  "${HOME}/Library/Android/sdk"
  "/opt/homebrew/share/android-commandlinetools"
  "/usr/local/share/android-commandlinetools"
)

# Auto-set ANDROID_HOME when Gradle would otherwise report "SDK location not found".
if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then
  for _sdk in "${_SDK_SEARCH_PATHS[@]}"; do
    if [[ -n "${_sdk}" && -d "${_sdk}/platforms" ]]; then
      export ANDROID_HOME="${_sdk}"
      note "Auto-set ANDROID_HOME=${ANDROID_HOME}"
      break
    fi
  done
fi

if [[ -z "${ANDROID_NDK_HOME:-}" && -z "${ANDROID_NDK_ROOT:-}" ]]; then
  for _sdk in "${_SDK_SEARCH_PATHS[@]}"; do
    if [[ -n "${_sdk}" && -d "${_sdk}/ndk" ]]; then
      # Pick the highest version directory.
      _ndk_ver="$(ls "${_sdk}/ndk" 2>/dev/null | sort -V | tail -1)"
      if [[ -n "${_ndk_ver}" ]]; then
        export ANDROID_NDK_HOME="${_sdk}/ndk/${_ndk_ver}"
        note "Auto-set ANDROID_NDK_HOME=${ANDROID_NDK_HOME}"
        break
      fi
    fi
  done
fi

if ! make -C "${REPO_ROOT}" android-so; then
  fail "${STEP2}"
fi
step_pass "${STEP2}"
echo ""

# ---------------------------------------------------------------------------
# JDK guard: Gradle 8.7 supports Java ≤ 21. If JAVA_HOME points at a newer
# JDK (e.g. temurin-26), auto-switch to the highest ≤ 21 JDK available via
# /usr/libexec/java_home (macOS) or the common symlink locations.
# This only applies to steps 3/4 (Gradle); steps 1/2 (cargo) are unaffected.
# ---------------------------------------------------------------------------
_resolve_jdk_le21() {
  local jv
  jv="$( (java -version 2>&1 | head -1 | rg -o '[0-9]+\.[0-9]+' | head -1) 2>/dev/null || true )"
  # Normalise "17.x" -> major 17, "1.8.x" -> major 8, "21.x" -> major 21.
  local major="${jv%%.*}"
  [[ "${jv}" == 1.* ]] && major="${jv#*.}" && major="${major%%.*}"
  if [[ -n "${major}" ]] && (( major > 21 )); then
    if command -v /usr/libexec/java_home >/dev/null 2>&1; then
      local jdk17
      jdk17="$(/usr/libexec/java_home -v 17 2>/dev/null || true)"
      if [[ -n "${jdk17}" && -d "${jdk17}" ]]; then
        export JAVA_HOME="${jdk17}"
        note "Auto-set JAVA_HOME=${JAVA_HOME} (Gradle 8.7 requires Java ≤ 21)"
        export PATH="${JAVA_HOME}/bin:${PATH}"
        return
      fi
      # Try any ≤ 21 JDK
      local v
      for v in 21 17 11; do
        local jhome
        jhome="$(/usr/libexec/java_home -v "${v}" 2>/dev/null || true)"
        if [[ -n "${jhome}" && -d "${jhome}" ]]; then
          export JAVA_HOME="${jhome}"
          note "Auto-set JAVA_HOME=${JAVA_HOME} (Gradle 8.7 requires Java ≤ 21)"
          export PATH="${JAVA_HOME}/bin:${PATH}"
          return
        fi
      done
    fi
    echo "  WARNING: Java ${major} detected (> 21) and no ≤ 21 JDK found." >&2
    echo "  Gradle 8.7 may fail. Install temurin-17 or set JAVA_HOME manually." >&2
  fi
}
_resolve_jdk_le21

# ---------------------------------------------------------------------------
# Step 3: Assemble the debug APK (./gradlew assembleDebug, run inside android/).
#
# assembleDebug also triggers the cargo-ndk Gradle task (buildCargoNdk) wired
# in android/app/build.gradle.kts; the .so produced in step 2 lands under
# jniLibs and is packaged here.
# ---------------------------------------------------------------------------
STEP3="step 3: assemble debug APK (./gradlew assembleDebug)"
echo "==> ${STEP3}"
if ! ( cd "${ANDROID_DIR}" && ./gradlew assembleDebug ); then
  fail "${STEP3}"
fi
step_pass "${STEP3}"
echo ""

# ---------------------------------------------------------------------------
# Step 4: Kotlin/JVM unit tests (./gradlew :app:testDebugUnitTest in android/).
#
# NOTE: as of this writing the project has no src/test JVM unit tests — the
# real Android test surface is the INSTRUMENTED cross-language crypto
# conformance suite (CryptoConformanceTest.kt) under src/androidTest, which
# runs only on a device/emulator via `./gradlew connectedDebugAndroidTest` and
# is intentionally out of scope for this no-device chain. Running the JVM unit
# test task is still a meaningful gate: it compiles the unit-test classpath and
# will fail loudly the moment real unit tests are added and break. If/when JVM
# unit tests exist, this step actually exercises them.
# ---------------------------------------------------------------------------
STEP4="step 4: Kotlin unit tests (./gradlew :app:testDebugUnitTest)"
echo "==> ${STEP4}"
if ! ( cd "${ANDROID_DIR}" && ./gradlew :app:testDebugUnitTest ); then
  fail "${STEP4}"
fi
step_pass "${STEP4}"
echo ""

# ---------------------------------------------------------------------------
# All steps passed.
# ---------------------------------------------------------------------------
printf "${GREEN}ANDROID VERIFY GREEN${NC}\n"
note "Reminder: this is a toolchain build + JVM unit-test gate only."
note "It does NOT verify CopyPaste on real Android hardware. Test on a device."
exit 0
