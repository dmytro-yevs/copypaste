#!/usr/bin/env bash
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK="$HERE/pre-commit"
TMP="$(mktemp -d)"
BASE_PATH="$(getconf PATH)"
trap 'rm -rf "$TMP"' EXIT

PASS=0
FAIL=0

make_repo() {
    local name="$1" repo="$TMP/$1" fake_bin
    mkdir -p "$repo"
    git -C "$repo" init -q
    git -C "$repo" config user.email test@example.com
    git -C "$repo" config user.name test
    printf '%s\n' 'fn main() {}' > "$repo/staged.rs"
    printf '%s\n' 'fn other() {}' > "$repo/other.rs"
    git -C "$repo" add staged.rs other.rs
    git -C "$repo" commit --no-verify -qm init
    printf '%s\n' 'fn main(){ }' > "$repo/staged.rs"
    git -C "$repo" add staged.rs

    fake_bin="$repo/fake-bin"
    mkdir -p "$fake_bin"
    cat > "$fake_bin/cargo" <<'EOF'
#!/usr/bin/env sh
case "${FAKE_CARGO_MODE:-clean}" in
    clean) exit 0 ;;
    staged-diff)
        printf 'Diff in %s/staged.rs:1:\n' "$PWD"
        exit 1
        ;;
    unrelated-diff)
        printf 'Diff in %s/other.rs:1:\n' "$PWD"
        exit 1
        ;;
    fail)
        printf '%s\n' 'rustfmt unavailable' >&2
        exit 127
        ;;
    fail-with-diff)
        printf 'Diff in %s/other.rs:1:\n' "$PWD"
        printf '%s\n' 'rustfmt crashed' >&2
        exit 1
        ;;
esac
EOF
    chmod +x "$fake_bin/cargo"
}

run_case() {
    local name="$1" mode="$2" want="$3" needle="${4:-}"
    local repo="$TMP/$name" fake_bin output rc before after
    make_repo "$name"
    fake_bin="$repo/fake-bin"
    if [ "$mode" = no-staged-rust ]; then
        git -C "$repo" reset -q
    fi
    before="$(<"$repo/other.rs")"

    if [ "$mode" = absent ] || [ "$mode" = no-staged-rust ]; then
        output="$(cd "$repo" && env PATH="$BASE_PATH" "$HOOK" 2>&1)"
    else
        output="$(cd "$repo" && env PATH="$fake_bin:$BASE_PATH" \
            FAKE_CARGO_MODE="$mode" "$HOOK" 2>&1)"
    fi
    rc=$?
    after="$(<"$repo/other.rs")"

    if [ "$rc" -ne "$want" ]; then
        printf '  FAIL  %s (wanted %s, got %s: %s)\n' "$name" "$want" "$rc" "${output:-(no output)}"
        FAIL=$((FAIL + 1))
    elif [ -n "$needle" ] && [[ "$output" != *"$needle"* ]]; then
        printf '  FAIL  %s (missing %s: %s)\n' "$name" "$needle" "${output:-(no output)}"
        FAIL=$((FAIL + 1))
    elif [ "$before" != "$after" ]; then
        printf '  FAIL  %s (modified unrelated file)\n' "$name"
        FAIL=$((FAIL + 1))
    else
        printf '  ok    %s\n' "$name"
        PASS=$((PASS + 1))
    fi
}

run_case no-staged-rust no-staged-rust 0
run_case cargo-absent absent 1 'cargo is required'
run_case cargo-failure fail 1 'cargo fmt --check failed'
run_case staged-format-diff staged-diff 1 'staged.rs'
run_case unrelated-format-diff unrelated-diff 0
run_case failure-with-unrelated-diff fail-with-diff 1 'cargo fmt --check failed'
run_case clean clean 0

printf '\npassed %d, failed %d\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
