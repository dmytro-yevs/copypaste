#!/usr/bin/env bash
# generate-lucide-icons.sh — Vendors a curated Lucide icon subset as Kotlin
# ImageVector source (design.md "Lucide icons" resolved decision, S0.3 spike,
# tasks.md 2.1). NO Maven dependency: every published Lucide-Compose artifact
# needs kotlin-stdlib >= 2.0, incompatible with this workspace's Kotlin 1.9.23.
#
# Pinned upstream: github.com/lucide-icons/lucide
#   tag: v0.265.0
#   sha: 9fb4b0b161fc256d2333f91812a927f2ed6f84c0
#   license: ISC (see android/NOTICE)
#
# Tool deviation from the S0.3 primary choice (DevSrSouza/svg-to-compose):
# that tool has no published Maven Central or resolvable JitPack artifact
# (verified 2026-07-02 — JitPack returns 404, no build triggered), so running
# it would require standing up a separate Kotlin/Gradle toolchain project
# with no version-compatibility guarantee, and duplicates the "vendor as
# ImageVector.Builder source" fallback that design.md pre-authorizes.
# generate-lucide-icons.mjs performs the equivalent SVG -> Kotlin conversion
# using only the stable public Compose PathBuilder DSL (moveTo/lineTo/
# horizontalLineTo/verticalLineTo/curveTo/reflectiveCurveTo/arcTo/close +
# *Relative variants), which mirrors the SVG path command grammar 1:1 — see
# that script's header for detail. Recorded in bd notes (CopyPaste-myh8.2).
#
# Usage: ./scripts/generate-lucide-icons.sh
# Requires: node, curl (network access to raw.githubusercontent.com)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

LUCIDE_SHA="9fb4b0b161fc256d2333f91812a927f2ed6f84c0"
OUT_DIR="${REPO_ROOT}/android/app/src/main/java/com/copypaste/android/ui/theme/icons"

node "${SCRIPT_DIR}/generate-lucide-icons.mjs" "${LUCIDE_SHA}" "${OUT_DIR}"

echo "Generated Lucide ImageVector sources under ${OUT_DIR}"
echo "Review the diff, then run: (cd android && ./gradlew :app:compileDebugKotlin -x buildCargoNdk)"
