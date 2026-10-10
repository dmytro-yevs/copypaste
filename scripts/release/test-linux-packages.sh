#!/usr/bin/env bash
# Build and inspect the Linux packages from a minimal native ELF bundle.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE="${COPYPASTE_LINUX_VALIDATION_IMAGE:-ubuntu:24.04}"
VERSION="${COPYPASTE_LINUX_PACKAGE_TEST_VERSION:-1.2.3}"

run_architecture() {
  local architecture="$1"
  local platform appimage_url appimage_sha
  case "$architecture" in
    x86_64)
      platform=linux/amd64
      appimage_url=https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
      appimage_sha=95cbe7cce9717fce90c484e34052ee7c7f1d7635b33c12525b4776826a7d29b6
      ;;
    aarch64)
      platform=linux/arm64
      appimage_url=https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-aarch64.AppImage
      appimage_sha=a595ea34cd6136c7f595e9dcbb16f3e9725d7610efb9e43b38c3c6e86cafc270
      ;;
    *) echo "ERROR: unsupported test architecture: $architecture" >&2; exit 1 ;;
  esac

  docker run --rm --platform "$platform" -e DEBIAN_FRONTEND=noninteractive \
    -e VERSION="$VERSION" -e ARCHITECTURE="$architecture" \
    -e APPIMAGETOOL_URL="$appimage_url" -e APPIMAGETOOL_SHA256="$appimage_sha" \
    -v "$ROOT:/source:ro" "$IMAGE" bash -ceu '
      apt-get update
      apt-get install --yes --no-install-recommends \
        binutils build-essential cpio curl dpkg-dev file rpm xz-utils
      cp -a /source /work
      cd /work
      mkdir -p dist/linux-"$ARCHITECTURE"/bundle
      printf "int main(void) { return 0; }\n" > /tmp/copypaste-fake.c
      cc -O2 -o /tmp/copypaste-fake /tmp/copypaste-fake.c
      install -m 755 /tmp/copypaste-fake dist/linux-"$ARCHITECTURE"/bundle/copypaste
      install -m 755 /tmp/copypaste-fake dist/linux-"$ARCHITECTURE"/bundle/copypaste-daemon
      install -m 755 /tmp/copypaste-fake dist/linux-"$ARCHITECTURE"/bundle/copypaste-cli
      mkdir -p packaging/linux/desktop-integrations/gnome-shell-extension
      mkdir -p packaging/linux/desktop-integrations/gnome-shell-extension/native
      mkdir -p packaging/linux/desktop-integrations/kde-kwin-script/contents/code
      printf "{\"uuid\":\"copypaste-quick-paste@copypaste.app\"}\n" > packaging/linux/desktop-integrations/gnome-shell-extension/metadata.json
      printf "int shim(void) { return 0; }\n" > packaging/linux/desktop-integrations/gnome-shell-extension/native/shim.c
      printf "all:\n\tmkdir -p build\n\t\$(CC) -shared -fPIC -o build/libcopypaste_clipboard_source.so shim.c\n\tprintf fixture > build/CopyPasteClipboard-1.0.typelib\n" > packaging/linux/desktop-integrations/gnome-shell-extension/native/Makefile
      printf "[Desktop Entry]\nX-KDE-PluginInfo-Name=copypaste-quick-paste\n" > packaging/linux/desktop-integrations/kde-kwin-script/metadata.desktop
      printf "// fake test companion\n" > packaging/linux/desktop-integrations/kde-kwin-script/contents/code/main.js
      printf "{}\n" > packaging/linux/desktop-integrations/manifest.json
      curl --fail --location --output /tmp/appimagetool "$APPIMAGETOOL_URL"
      printf "%s  %s\n" "$APPIMAGETOOL_SHA256" /tmp/appimagetool | sha256sum --check --status
      chmod 755 /tmp/appimagetool
      APPIMAGE_EXTRACT_AND_RUN=1 APPIMAGETOOL=/tmp/appimagetool scripts/release/build-linux-packages.sh "$VERSION" "$ARCHITECTURE"
      for extension in AppImage deb rpm; do test -s "dist/CopyPaste-v$VERSION-linux-$ARCHITECTURE.$extension"; done
      dpkg-deb --field "dist/CopyPaste-v$VERSION-linux-$ARCHITECTURE.deb" Version | grep -Fx "$VERSION"
      dpkg-deb --field "dist/CopyPaste-v$VERSION-linux-$ARCHITECTURE.deb" Architecture | grep -Fx "$(test "$ARCHITECTURE" = x86_64 && echo amd64 || echo arm64)"
      dpkg-deb -x "dist/CopyPaste-v$VERSION-linux-$ARCHITECTURE.deb" /tmp/deb
      rpm -qpi "dist/CopyPaste-v$VERSION-linux-$ARCHITECTURE.rpm" | grep -E "^Version[[:space:]]*:[[:space:]]*$VERSION$"
      mkdir /tmp/rpm
      (cd /tmp/rpm && rpm2cpio "/work/dist/CopyPaste-v$VERSION-linux-$ARCHITECTURE.rpm" | cpio -idm --quiet)
      mkdir /tmp/appimage
      (cd /tmp/appimage && APPIMAGE_EXTRACT_AND_RUN=1 "/work/dist/CopyPaste-v$VERSION-linux-$ARCHITECTURE.AppImage" --appimage-extract >/dev/null)
      for root in /tmp/deb /tmp/rpm /tmp/appimage/squashfs-root; do
        test -x "$root/usr/lib/copypaste/copypaste"
        test -x "$root/usr/lib/copypaste/copypaste-daemon"
        test -x "$root/usr/lib/copypaste/copypaste-cli"
        test -L "$root/usr/bin/copypaste"
        test -L "$root/usr/bin/copypaste-cli"
        test -f "$root/usr/share/copypaste/autostart/com.copypaste.CopyPaste.desktop"
        test ! -e "$root/etc/xdg/autostart/com.copypaste.CopyPaste.desktop"
        test -f "$root/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app/metadata.json"
        test -f "$root/usr/share/kwin/scripts/copypaste-quick-paste/metadata.desktop"
      done
      grep -F "\"kind\": \"deb\"" /tmp/deb/usr/lib/copypaste/package-metadata.json
      grep -F "\"kind\": \"rpm\"" /tmp/rpm/usr/lib/copypaste/package-metadata.json
      grep -F "\"kind\": \"appimage\"" /tmp/appimage/squashfs-root/usr/lib/copypaste/package-metadata.json
      test -s /tmp/appimage/squashfs-root/usr/share/copypaste/desktop-integrations/gnome-shell-extension/native/lib/libcopypaste_clipboard_source.so
      test -s /tmp/appimage/squashfs-root/usr/share/copypaste/desktop-integrations/gnome-shell-extension/native/typelib/CopyPasteClipboard-1.0.typelib
      file --brief /tmp/appimage/squashfs-root/usr/share/copypaste/desktop-integrations/gnome-shell-extension/native/lib/libcopypaste_clipboard_source.so | grep -q "^ELF "
      dpkg-deb --ctrl-tarfile "dist/CopyPaste-v$VERSION-linux-$ARCHITECTURE.deb" | tar -tv | grep -E " root/root .*control$"
    '
}

for architecture in ${COPYPASTE_LINUX_PACKAGE_TEST_ARCHITECTURES:-x86_64 aarch64}; do
  run_architecture "$architecture"
done
