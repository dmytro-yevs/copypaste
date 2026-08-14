#!/usr/bin/env bash
# Sync these gate scripts into the WSL home so ~/env-setup and the repository
# cannot drift. `--check` reports the drift instead of fixing it.
set -uo pipefail
source_dir="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"

check=0
dest="$HOME/env-setup"
for arg in "$@"; do
    case "$arg" in
        --check) check=1 ;;
        *)       dest="$arg" ;;
    esac
done

mkdir -p "$dest" || exit 2
status=0
for name in gate-lib.sh verify.sh verify2.sh verify3.sh verify-android.sh; do
    if [ "$check" -eq 1 ]; then
        if ! diff -u "$dest/$name" "$source_dir/$name"; then
            echo "DRIFT  $dest/$name"
            status=1
        fi
    else
        install -m 0755 "$source_dir/$name" "$dest/$name" && echo "installed $dest/$name"
    fi
done
exit "$status"
