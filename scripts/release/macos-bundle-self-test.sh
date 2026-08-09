#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$REPO_ROOT/scripts/release/macos-bundle-lib.sh"

TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEST_ROOT"' EXIT

PLIST_BUDDY="$TEST_ROOT/plist-reader"
cat > "$PLIST_BUDDY" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == "-c" && "$2" == "Print :CFBundleExecutable" ]]
[[ -s "$3" ]]
cat "$3"
EOF
chmod +x "$PLIST_BUDDY"
export PLIST_BUDDY

PASS=0
FAIL=0

ok() {
    PASS=$((PASS + 1))
    printf '  ok      %s\n' "$1"
}

bad() {
    FAIL=$((FAIL + 1))
    printf '  FAIL    %s\n' "$1"
}

fixture() {
    local name="$1"
    local metadata="${2-}"
    local mode="${3-missing}"
    local file_name="${4-BundleRunner}"
    local app="$TEST_ROOT/$name.app"

    mkdir -p "$app/Contents/MacOS"
    printf '%s' "$metadata" > "$app/Contents/Info.plist"
    if [[ "$mode" != "missing" ]]; then
        if [[ "$mode" == "executable" ]]; then
            printf '#!/usr/bin/env bash\nexit 0\n' > "$app/Contents/MacOS/$file_name"
            chmod +x "$app/Contents/MacOS/$file_name"
        else
            printf 'not executable\n' > "$app/Contents/MacOS/$file_name"
        fi
    fi
    printf '%s\n' "$app"
}

expect_path() {
    local label="$1"
    local app="$2"
    local executable="$3"
    local expected="${app}/Contents/MacOS/${executable}"
    local actual

    if actual="$(macos_bundle_executable_path "$app")" && [[ "$actual" == "$expected" ]]; then
        ok "$label"
    else
        bad "$label"
    fi
}

expect_rejection() {
    local label="$1"
    local app="$2"
    local error="$TEST_ROOT/error"

    if macos_bundle_executable_path "$app" > /dev/null 2> "$error"; then
        bad "$label"
    elif grep -Fq "$TEST_ROOT" "$error"; then
        bad "$label (error exposed a path)"
    else
        ok "$label"
    fi
}

expect_path "resolves a renamed executable declared by bundle metadata" \
    "$(fixture valid RenamedProduct executable RenamedProduct)" RenamedProduct
expect_rejection "rejects missing CFBundleExecutable metadata" \
    "$(fixture missing-metadata '' executable)"
expect_rejection "rejects ambiguous multi-line executable metadata" \
    "$(fixture ambiguous $'BundleRunner\nOther' executable)"
expect_rejection "rejects an executable name that escapes Contents/MacOS" \
    "$(fixture traversal ../BundleRunner executable)"
expect_rejection "rejects a missing declared executable" \
    "$(fixture missing-executable BundleRunner missing)"
expect_rejection "rejects a declared file without execute permission" \
    "$(fixture not-executable BundleRunner file)"
VALID_APP="$(fixture missing-reader BundleRunner executable)"
PLIST_BUDDY="$TEST_ROOT/unavailable-reader"
expect_rejection "rejects an unavailable metadata reader" "$VALID_APP"

printf '\npassed %d, failed %d\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]]
