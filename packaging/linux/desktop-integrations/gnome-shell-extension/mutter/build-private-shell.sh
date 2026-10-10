#!/usr/bin/env bash
# Build a matching GNOME Shell into an already-installed private Mutter prefix.
set -euo pipefail

shell_commit=9fca03bb1544c85928041a935f4ce895333722f1 # GNOME Shell 46.9
mutter_runtime=${1:?usage: $0 <private-mutter-destdir> [shell-source] [shell-build]}
shell_source=${2:-"$(dirname "$mutter_runtime")/gnome-shell-source"}
shell_build=${3:-"$shell_source/build-copypaste-private"}
[[ -d "$mutter_runtime/usr" && ! -L "$mutter_runtime" ]] || { echo "ERROR: private Mutter prefix is missing or unsafe" >&2; exit 1; }
if [[ ! -d "$shell_source/.git" ]]; then
  git -C "$(dirname "$shell_source")" init -q "$(basename "$shell_source")"
  git -C "$shell_source" remote add origin https://gitlab.gnome.org/GNOME/gnome-shell.git
fi
git -C "$shell_source" fetch --depth=1 origin "$shell_commit"
git -C "$shell_source" checkout --detach "$shell_commit"
git -C "$shell_source" clean -fdx
private_lib="$mutter_runtime/usr/lib/$(dpkg-architecture -qDEB_HOST_MULTIARCH)"
[[ -d "$private_lib" ]] || { echo "ERROR: private Mutter library directory is missing" >&2; exit 1; }
export PKG_CONFIG_PATH="$private_lib/pkgconfig"
export GI_TYPELIB_PATH="$private_lib/mutter-14"
export LD_LIBRARY_PATH="$private_lib"
meson setup --wipe "$shell_build" "$shell_source" --prefix /usr -Dtests=false
meson compile -C "$shell_build" gnome-shell
DESTDIR="$mutter_runtime" meson install -C "$shell_build" --no-rebuild
entrypoint="$mutter_runtime/usr/libexec/copypaste-gnome-shell"
install -d "$(dirname "$entrypoint")"
cat > "$entrypoint" <<'EOF'
#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec "$root/bin/gnome-shell" --wayland
EOF
chmod 755 "$entrypoint"
test -x "$mutter_runtime/usr/bin/gnome-shell"
test -x "$entrypoint"
printf '%s\n' "$shell_commit" > "$mutter_runtime/.copypaste-gnome-shell-revision"
