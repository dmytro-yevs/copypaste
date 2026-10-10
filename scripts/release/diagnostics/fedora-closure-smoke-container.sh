#!/usr/bin/env bash
# Exercise the closure against installed Fedora RPM metadata only.
set -euxo pipefail

runtime=/tmp/kwin-runtime
stage=/tmp/kwin-stage
receipt=/tmp/runtime-receipt.json
package=/tmp/kwin-closure-smoke.rpm
mkdir -p "$runtime" "$stage"

rpm -q kwin kwin-wayland kwin-libs
library="$(rpm -ql kwin-libs | grep -E '/libkwin\.so\.' | head -n 1)"
[[ -f "$library" ]]
python3 /workspace/scripts/release/diagnostics/fedora-closure-provenance.py --library "$library"

while IFS= read -r source; do
  [[ -f "$source" || -L "$source" ]] || continue
  target="$runtime$source"
  mkdir -p "$(dirname "$target")"
  cp -a --no-dereference "$source" "$target"
done < <(rpm -ql kwin kwin-wayland kwin-libs | sort -u)

test -x "$runtime/usr/bin/kwin_wayland"
source_license=/workspace/packaging/linux/desktop-integrations/kde-native-clipboard/src/copypasteclipboardbridge.cpp
test -f "$source_license"
python3 /workspace/scripts/release/diagnostics/fedora-closure-missing.py \
  --runtime "$runtime" --soname libpxbackend-1.0.so
python3 /workspace/scripts/release/diagnostics/fedora-closure-preflight.py --runtime "$runtime"

python3 /workspace/packaging/linux/compositor-runtime/private_elf_closure.py \
  --runtime-dir "$runtime" --entrypoint usr/bin/kwin_wayland
python3 /workspace/packaging/linux/compositor-runtime/emit_runtime_receipt.py \
  --runtime-dir "$runtime" --output "$receipt" \
  --runtime-id kwin-6.0-fedora40-smoke --desktop KDE \
  --source-revision 1ddcb4e288c4f7dcecdc94efccd655b7e3666d30 \
  --patch /workspace/packaging/linux/desktop-integrations/kde-native-clipboard/patches/kwin-6.0.patch \
  --glibc-floor 2.39 --private-entrypoint usr/bin/kwin_wayland \
  --qualification-entrypoint usr/bin/kwin_wayland --license-file "$source_license"
python3 /workspace/packaging/linux/compositor-runtime/stage_runtime.py \
  --receipt "$receipt" --runtime-dir "$runtime" --stage-root "$stage" --expected-architecture x86_64
python3 /workspace/packaging/linux/compositor-runtime/verify_runtime_package.py \
  --root "$stage" --runtime-id kwin-6.0-fedora40-smoke
python3 /workspace/packaging/linux/compositor-runtime/build_companion_package.py \
  --receipt "$receipt" --runtime-dir "$runtime" --version 1.0.0 --format rpm --output "$package"

printf '%s\n' '--- private RPM requires ---'
rpm -qp --requires "$package" | sort
printf '%s\n' '--- private RPM provides ---'
rpm -qp --provides "$package" | sort
printf '%s\n' '--- private RPM obsoletes/conflicts ---'
rpm -qp --obsoletes "$package" | sort
rpm -qp --conflicts "$package" | sort
python3 /workspace/scripts/release/diagnostics/fedora-closure-provenance.py \
  --library "$library" --manifest "$runtime/usr/share/copypaste/compositor-runtime-private-closure.json"
