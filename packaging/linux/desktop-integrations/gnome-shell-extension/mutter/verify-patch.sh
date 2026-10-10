#!/usr/bin/env bash
set -euo pipefail

# Hosted-Linux verification recipe. Required packages on Ubuntu 24.04 include
# git, build-essential, meson, ninja-build, pkg-config, libglib2.0-dev,
# libwayland-dev, libmutter-14-dev, and Mutter's normal build dependencies.
# The caller provides those dependencies; this script never changes the host
# package database.
version=${1:?usage: $0 46|47 [source-dir] [build-dir] [runtime-output]}
case "$version" in
  46) commit=fe8d2be3f90f89f286c89b164c94a4f86552bc97; patch="$PWD/mutter-46-writer-identity.patch"; target=mutter-14; test_options=(-Dtests=false) ;;
  47) commit=d688d0823fdc044885a4bd5f51dff038c9c6e8fc; patch="$PWD/mutter-47-writer-identity.patch"; target=mutter-15; test_options=(-Dtests=disabled) ;;
  *) echo "unsupported Mutter version: $version" >&2; exit 2 ;;
esac
source_dir=${2:-"$PWD/mutter-source-$version"}
build_dir=${3:-"$source_dir/build-copypaste-writer-identity"}
runtime_output=${4:-}
if [[ ! -d "$source_dir/.git" ]]; then
  mkdir -p "$source_dir"
  git -C "$source_dir" init -q
  git -C "$source_dir" remote add origin https://gitlab.gnome.org/GNOME/mutter.git
fi
git -C "$source_dir" fetch --depth=1 origin "$commit"
test "$(git -C "$source_dir" rev-parse "$commit^{commit}")" = "$commit"
git -C "$source_dir" checkout --detach "$commit"
git -C "$source_dir" apply --check "$patch"
git -C "$source_dir" apply "$patch"
meson setup --wipe "$build_dir" "$source_dir" --prefix /usr \
  "${test_options[@]}" -Dprofiler=false -Dinstalled_tests=false
meson compile -C "$build_dir" "$target"
if [[ -n "$runtime_output" ]]; then
  [[ ! -e "$runtime_output" ]] || { echo "ERROR: runtime output must not exist" >&2; exit 1; }
  mkdir -p "$runtime_output"
  DESTDIR="$runtime_output" meson install -C "$build_dir" --no-rebuild
  "$PWD/build-private-shell.sh" "$runtime_output"
  runtime_id="gnome-${version}-private-shell"
  python3 "$PWD/../../../compositor-runtime/emit_runtime_receipt.py" \
    --runtime-dir "$runtime_output" --output "$runtime_output/runtime-receipt.json" \
    --runtime-id "$runtime_id" --desktop GNOME --source-revision "$commit" \
    --patch "$patch" --glibc-floor 2.39 --dependency gnome-shell --dependency gnome-session \
    --private-entrypoint usr/libexec/copypaste-gnome-shell \
    --qualification-entrypoint usr/libexec/copypaste-gnome-shell-headless \
    --shell-revision "$(cat "$runtime_output/.copypaste-gnome-shell-revision")" --license-file "$source_dir/COPYING"
fi
echo "verified Mutter $version commit $commit and installed immutable runtime=${runtime_output:-none}"
