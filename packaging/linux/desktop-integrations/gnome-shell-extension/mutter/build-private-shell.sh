#!/usr/bin/env bash
# Build a matching GNOME Shell into an already-installed private Mutter prefix.
set -euo pipefail

shell_commit=9fca03bb1544c85928041a935f4ce895333722f1 # GNOME Shell 46.9
mutter_runtime=${1:?usage: $0 <private-mutter-destdir> [shell-source] [shell-build]}
shell_source=${2:-"$(dirname "$mutter_runtime")/gnome-shell-source"}
shell_build=${3:-"$shell_source/build-copypaste-private"}
shell_patch="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/gnome-shell-46-relocatable-runtime.patch"
[[ -d "$mutter_runtime/usr" && ! -L "$mutter_runtime" ]] || { echo "ERROR: private Mutter prefix is missing or unsafe" >&2; exit 1; }
[[ -f "$shell_patch" && ! -L "$shell_patch" ]] || { echo "ERROR: private GNOME Shell patch is missing or unsafe" >&2; exit 1; }
if [[ ! -d "$shell_source/.git" ]]; then
  git -C "$(dirname "$shell_source")" init -q "$(basename "$shell_source")"
  git -C "$shell_source" remote add origin https://gitlab.gnome.org/GNOME/gnome-shell.git
fi
git -C "$shell_source" fetch --depth=1 origin "$shell_commit"
git -C "$shell_source" checkout --detach "$shell_commit"
git -C "$shell_source" clean -fdx
git -C "$shell_source" apply --check "$shell_patch"
git -C "$shell_source" apply "$shell_patch"
private_lib="$mutter_runtime/usr/lib/$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
[[ -d "$private_lib" ]] || { echo "ERROR: private Mutter library directory is missing" >&2; exit 1; }
export PKG_CONFIG_PATH="$private_lib/pkgconfig"
export GI_TYPELIB_PATH="$private_lib/mutter-14"
export LD_LIBRARY_PATH="$private_lib"
meson setup --wipe "$shell_build" "$shell_source" --prefix /usr -Dtests=false
meson compile -C "$shell_build"
DESTDIR="$mutter_runtime" meson install -C "$shell_build" --no-rebuild
command -v patchelf >/dev/null
find "$mutter_runtime/usr" -type f -print0 | while IFS= read -r -d '' elf; do
  file --brief "$elf" | grep -q '^ELF ' || continue
  patchelf --remove-rpath "$elf"
done
entrypoint="$mutter_runtime/usr/libexec/copypaste-gnome-shell"
install -d "$(dirname "$entrypoint")"
cat > "$entrypoint" <<'EOF'
#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
private_library_path="$root/lib:$root/lib64:$root/lib/x86_64-linux-gnu:$root/lib/x86_64-linux-gnu/mutter-14:$root/lib/x86_64-linux-gnu/gnome-shell:$root/lib/aarch64-linux-gnu:$root/lib/aarch64-linux-gnu/mutter-14:$root/lib/aarch64-linux-gnu/gnome-shell"
private_typelib_path="$root/lib/x86_64-linux-gnu/mutter-14:$root/lib/x86_64-linux-gnu/gnome-shell:$root/lib/aarch64-linux-gnu/mutter-14:$root/lib/aarch64-linux-gnu/gnome-shell"
system_typelib_path="/usr/lib/x86_64-linux-gnu/girepository-1.0:/usr/lib/aarch64-linux-gnu/girepository-1.0"
export LD_LIBRARY_PATH="$private_library_path"
export GI_TYPELIB_PATH="$private_typelib_path:$system_typelib_path${GI_TYPELIB_PATH:+:$GI_TYPELIB_PATH}"
exec "$root/bin/gnome-shell" --wayland
EOF
chmod 755 "$entrypoint"
qualification_entrypoint="$mutter_runtime/usr/libexec/copypaste-gnome-shell-headless"
cat > "$qualification_entrypoint" <<'EOF'
#!/bin/sh
set -eu
[ "$#" -eq 0 ] || { echo "CopyPaste GNOME qualification session does not accept arguments" >&2; exit 64; }
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
private_library_path="$root/lib:$root/lib64:$root/lib/x86_64-linux-gnu:$root/lib/x86_64-linux-gnu/mutter-14:$root/lib/x86_64-linux-gnu/gnome-shell:$root/lib/aarch64-linux-gnu:$root/lib/aarch64-linux-gnu/mutter-14:$root/lib/aarch64-linux-gnu/gnome-shell"
private_typelib_path="$root/lib/x86_64-linux-gnu/mutter-14:$root/lib/x86_64-linux-gnu/gnome-shell:$root/lib/aarch64-linux-gnu/mutter-14:$root/lib/aarch64-linux-gnu/gnome-shell"
system_typelib_path="/usr/lib/x86_64-linux-gnu/girepository-1.0:/usr/lib/aarch64-linux-gnu/girepository-1.0"
export LD_LIBRARY_PATH="$private_library_path"
export GI_TYPELIB_PATH="$private_typelib_path:$system_typelib_path${GI_TYPELIB_PATH:+:$GI_TYPELIB_PATH}"
exec "$root/bin/gnome-shell" --headless --virtual-monitor 1280x800
EOF
chmod 755 "$qualification_entrypoint"
test -x "$mutter_runtime/usr/bin/gnome-shell"
test -x "$entrypoint"
test -x "$qualification_entrypoint"
printf '%s\n' "$shell_commit" > "$mutter_runtime/.copypaste-gnome-shell-revision"
sha256sum "$shell_patch" | awk '{print $1}' > "$mutter_runtime/.copypaste-gnome-shell-patch-sha256"
