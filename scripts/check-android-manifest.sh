#!/usr/bin/env sh
# Guard against reintroduction of broad Android package visibility.
#
# QUERY_ALL_PACKAGES grants visibility to every installed package. Google Play
# requires a declaration review for it, and the exclusion picker only needs
# ACTION_MAIN+CATEGORY_LAUNCHER intent visibility. This check fails the build
# if the permission reappears without an ADR exemption.
set -eu

manifest="crates/copypaste-ui/src-tauri/gen/android/app/src/main/AndroidManifest.xml"

if [ ! -f "$manifest" ]; then
    printf 'SKIP: %s not found\n' "$manifest"
    exit 0
fi

if grep -q 'QUERY_ALL_PACKAGES' "$manifest"; then
    # Allow it only if an ADR explicitly exempts it.
    if ls docs/adr/*query-all-packages* >/dev/null 2>&1 || \
       ls docs/adr/*QUERY_ALL_PACKAGES* >/dev/null 2>&1; then
        printf 'PASS: QUERY_ALL_PACKAGES present with ADR exemption\n'
        exit 0
    fi
    printf 'FAIL: %s declares QUERY_ALL_PACKAGES without an ADR exemption\n' "$manifest" >&2
    printf '  Remove the permission or add an ADR in docs/adr/ whose filename\n' >&2
    printf '  contains "query-all-packages".\n' >&2
    exit 1
fi

# Verify the narrower <queries> block has the launcher intent.
if ! grep -q 'android.intent.action.MAIN' "$manifest"; then
    printf 'FAIL: %s missing ACTION_MAIN intent query for the exclusion picker\n' "$manifest" >&2
    exit 1
fi

if ! grep -q 'android.intent.category.LAUNCHER' "$manifest"; then
    printf 'FAIL: %s missing CATEGORY_LAUNCHER intent query for the exclusion picker\n' "$manifest" >&2
    exit 1
fi

# Verify Shizuku package query is preserved.
if ! grep -q 'moe.shizuku.privileged.api' "$manifest"; then
    printf 'FAIL: %s missing Shizuku package query\n' "$manifest" >&2
    exit 1
fi

printf 'PASS: Android manifest uses narrow package visibility\n'
