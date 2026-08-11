#!/usr/bin/env sh
# Run the repository secret scan, and prove it can still fail.
#
# The self-test seeds a credential-shaped string into a scratch directory
# outside the worktree and asserts gitleaks reports it under the expected rule.
# Without it, an allowlist broad enough to disable a rule would turn the scan
# green and read as a clean repository.
#
# The seed is assembled from fragments at run time, so this file never holds a
# matching literal, and the scratch path is outside the worktree, so no
# allowlist in `.gitleaks.toml` can apply to it.
set -eu

mode=
history=0
for arg in "$@"; do
    case "$arg" in
        --self-test | --scan) mode=$arg ;;
        --history) history=1 ;;
        *)
            printf 'usage: %s --self-test | --scan [--history]\n' "$0" >&2
            exit 2
            ;;
    esac
done
[ -n "$mode" ] || {
    printf 'usage: %s --self-test | --scan [--history]\n' "$0" >&2
    exit 2
}

command -v gitleaks >/dev/null 2>&1 || {
    printf 'gitleaks is not on PATH; see SECURITY.md\n' >&2
    exit 127
}

root=$(git rev-parse --show-toplevel)
config="$root/.gitleaks.toml"

if [ "$mode" = --self-test ]; then
    dir=$(mktemp -d)
    trap 'rm -rf "$dir"' EXIT
    printf 'aws_access_key_id = "%s%s%s"\n' 'AKIA' 'ZZ4TESTSEED' 'GUARD' > "$dir/seeded.txt"

    if gitleaks dir "$dir" --config "$config" --no-banner \
        --report-format json --report-path "$dir/report.json" >/dev/null 2>&1; then
        printf 'self-test: gitleaks exited 0 on a seeded secret\n' >&2
        exit 1
    fi
    grep -q 'aws-access-token' "$dir/report.json" || {
        printf 'self-test: seeded secret was not reported as aws-access-token\n' >&2
        exit 1
    }
    printf 'self-test: seeded secret reported; the scan can still fail\n'
    exit 0
fi

if [ "$history" = 1 ]; then
    exec gitleaks git "$root" --config "$config" --no-banner
fi
exec gitleaks dir "$root" --config "$config" --no-banner
