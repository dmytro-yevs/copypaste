#!/usr/bin/env bash
# Package an already-built native Flutter Linux bundle as AppImage, deb, and rpm.
set -euo pipefail

VERSION="${1:-}"
ARCHITECTURE="${2:-}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "ERROR: usage: $0 <stable-version> <x86_64|aarch64>" >&2
  exit 1
}
case "$ARCHITECTURE" in x86_64|aarch64) ;; *) echo "ERROR: invalid architecture" >&2; exit 1 ;; esac

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONTRACT="$ROOT/packaging/linux/release-contract.json"
BUNDLE="$ROOT/dist/linux-$ARCHITECTURE/bundle"
DESKTOP_TEMPLATE="$ROOT/packaging/linux/com.copypaste.CopyPaste.desktop"
ICON="$ROOT/apps/copypaste_flutter/assets/brand/copypaste.png"
INTEGRATIONS="$ROOT/packaging/linux/desktop-integrations"
[[ "$(uname -s)" == Linux ]] || { echo "ERROR: Linux packaging requires Linux" >&2; exit 1; }
[[ -f "$CONTRACT" && -f "$DESKTOP_TEMPLATE" && -f "$ICON" && -f "$INTEGRATIONS/manifest.json" ]] || {
  echo "ERROR: Linux package metadata is missing" >&2
  exit 1
}
[[ -x "$BUNDLE/copypaste" && -x "$BUNDLE/copypaste-daemon" && -x "$BUNDLE/copypaste-cli" ]] || {
  echo "ERROR: build-linux-flutter.sh must stage app, daemon, and CLI first" >&2
  exit 1
}
NATIVE_SHIM="$INTEGRATIONS/gnome-shell-extension/native"
make -C "$NATIVE_SHIM"
[[ -s "$NATIVE_SHIM/build/libcopypaste_clipboard_source.so" && -s "$NATIVE_SHIM/build/CopyPasteClipboard-1.0.typelib" ]] || {
  echo "ERROR: GNOME native clipboard shim build is incomplete" >&2
  exit 1
}

case "$ARCHITECTURE" in
  x86_64) DEB_ARCH=amd64; RPM_ARCH=x86_64 ;;
  aarch64) DEB_ARCH=arm64; RPM_ARCH=aarch64 ;;
esac
DIST="$ROOT/dist"
mkdir -p "$DIST"
STAGE="$(mktemp -d)"
TOPDIR="$(mktemp -d)"
trap 'rm -rf "$STAGE" "$TOPDIR"' EXIT

write_package_metadata() {
  local kind="$1"
  cat > "$STAGE/usr/lib/copypaste/package-metadata.json" <<EOF
{
  "schema": 1,
  "kind": "$kind",
  "architecture": "$ARCHITECTURE"
}
EOF
}

install -d "$STAGE/usr/lib/copypaste" "$STAGE/usr/bin" \
  "$STAGE/usr/share/applications" "$STAGE/usr/share/icons/hicolor/256x256/apps" \
  "$STAGE/usr/share/gnome-shell/extensions" "$STAGE/usr/share/kwin/scripts"
cp -a "$BUNDLE/." "$STAGE/usr/lib/copypaste/"
ln -s ../lib/copypaste/copypaste "$STAGE/usr/bin/copypaste"
ln -s ../lib/copypaste/copypaste-cli "$STAGE/usr/bin/copypaste-cli"
install -m 644 "$DESKTOP_TEMPLATE" "$STAGE/usr/share/applications/com.copypaste.CopyPaste.desktop"
install -d "$STAGE/usr/share/copypaste/autostart"
install -m 644 "$DESKTOP_TEMPLATE" "$STAGE/usr/share/copypaste/autostart/com.copypaste.CopyPaste.desktop"
install -m 644 "$ICON" "$STAGE/usr/share/icons/hicolor/256x256/apps/com.copypaste.CopyPaste.png"
cp -a "$INTEGRATIONS/gnome-shell-extension" "$STAGE/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app"
cp -a "$INTEGRATIONS/kde-kwin-script" "$STAGE/usr/share/kwin/scripts/copypaste-quick-paste"
rm -rf "$STAGE/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app/native/build"
install -d "$STAGE/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app/native/lib" \
  "$STAGE/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app/native/typelib"
install -m 755 "$NATIVE_SHIM/build/libcopypaste_clipboard_source.so" \
  "$STAGE/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app/native/lib/"
install -m 644 "$NATIVE_SHIM/build/CopyPasteClipboard-1.0.typelib" \
  "$STAGE/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app/native/typelib/"

DEB="$DIST/CopyPaste-v$VERSION-linux-$ARCHITECTURE.deb"
install -d "$STAGE/DEBIAN"
write_package_metadata deb
SHLIB_WORK="$TOPDIR/shlibdeps"
mkdir -p "$SHLIB_WORK/debian"
printf 'Source: copypaste\nSection: utils\nPriority: optional\nMaintainer: CopyPaste <support@copypaste.app>\nStandards-Version: 4.6.2\n\nPackage: copypaste\nArchitecture: any\nDescription: temporary shlibdeps control file\n' > "$SHLIB_WORK/debian/control"
shlib_inputs=()
while IFS= read -r -d '' candidate; do
  if file --brief "$candidate" | grep -q '^ELF '; then
    shlib_inputs+=("-e$candidate")
  fi
done < <(find "$STAGE/usr" -type f -print0)
[[ "${#shlib_inputs[@]}" -gt 0 ]] || { echo "ERROR: package contains no ELF runtime files" >&2; exit 1; }
DEB_DEPENDS="$(cd "$SHLIB_WORK" && dpkg-shlibdeps -O -l"$STAGE/usr/lib/copypaste" "${shlib_inputs[@]}" \
  | sed -n 's/^shlibs:Depends=//p' | paste -sd, -)"
[[ -n "$DEB_DEPENDS" ]] || { echo "ERROR: could not derive Debian runtime dependencies" >&2; exit 1; }
cat > "$STAGE/DEBIAN/control" <<EOF
Package: copypaste
Version: $VERSION
Section: utils
Priority: optional
Architecture: $DEB_ARCH
Maintainer: CopyPaste <support@copypaste.app>
Depends: $DEB_DEPENDS
Description: Encrypted clipboard history
 CopyPaste keeps encrypted clipboard history locally and syncs only with paired devices.
EOF
dpkg-deb --root-owner-group --build "$STAGE" "$DEB"
rm -rf "$STAGE/DEBIAN"

SPEC="$TOPDIR/SPECS/copypaste.spec"
mkdir -p "$TOPDIR/BUILD" "$TOPDIR/BUILDROOT" "$TOPDIR/RPMS" "$TOPDIR/SOURCES" "$TOPDIR/SPECS" "$TOPDIR/SRPMS"
cat > "$SPEC" <<EOF
Name: copypaste
Version: $VERSION
Release: 1%{?dist}
Summary: Encrypted clipboard history
License: MIT OR Apache-2.0
BuildArch: $RPM_ARCH
Requires: glibc >= 2.39

%description
CopyPaste keeps encrypted clipboard history locally and syncs only with paired devices.

%install
cp -a %{_source_stage}/. %{buildroot}/

%files
/usr/bin/copypaste
/usr/bin/copypaste-cli
/usr/lib/copypaste
/usr/share/applications/com.copypaste.CopyPaste.desktop
/usr/share/icons/hicolor/256x256/apps/com.copypaste.CopyPaste.png
/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app
/usr/share/kwin/scripts/copypaste-quick-paste
/usr/share/copypaste/autostart/com.copypaste.CopyPaste.desktop
EOF
write_package_metadata rpm
rpmbuild -bb "$SPEC" --define "_topdir $TOPDIR" --define "_source_stage $STAGE" --define "_build_id_links none"
RPM="$DIST/CopyPaste-v$VERSION-linux-$ARCHITECTURE.rpm"
mapfile -t rpm_outputs < <(find "$TOPDIR/RPMS" -name 'copypaste-*.rpm' -type f -print)
[[ "${#rpm_outputs[@]}" == 1 ]] || { echo "ERROR: RPM build produced an unexpected output set" >&2; exit 1; }
mv "${rpm_outputs[0]}" "$RPM"
rpm_requires="$TOPDIR/rpm-requires.txt"
rpm -qp --requires "$RPM" > "$rpm_requires"
grep -q '^libc\.so\.6' "$rpm_requires" || { echo "ERROR: RPM automatic ELF dependency analysis is missing" >&2; exit 1; }

APPDIR="$(mktemp -d)"
trap 'rm -rf "$STAGE" "$TOPDIR" "$APPDIR"' EXIT
mkdir -p "$APPDIR/usr"
write_package_metadata appimage
cp -a "$STAGE/usr/." "$APPDIR/usr/"
mkdir -p "$APPDIR/usr/share/copypaste/desktop-integrations"
cp -a "$INTEGRATIONS/manifest.json" "$APPDIR/usr/share/copypaste/desktop-integrations/"
if [[ -f "$INTEGRATIONS/README.md" ]]; then
  cp -a "$INTEGRATIONS/README.md" "$APPDIR/usr/share/copypaste/desktop-integrations/"
fi
cp -a "$STAGE/usr/share/gnome-shell/extensions/copypaste-quick-paste@copypaste.app" \
  "$APPDIR/usr/share/copypaste/desktop-integrations/gnome-shell-extension"
cp -a "$STAGE/usr/share/kwin/scripts/copypaste-quick-paste" \
  "$APPDIR/usr/share/copypaste/desktop-integrations/kde-kwin-script"
sed 's#^Exec=.*#Exec=AppRun %U#' "$DESKTOP_TEMPLATE" > "$APPDIR/com.copypaste.CopyPaste.desktop"
cp "$ICON" "$APPDIR/com.copypaste.CopyPaste.png"
cat > "$APPDIR/AppRun" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$HERE/usr/lib/copypaste/copypaste" "$@"
EOF
chmod 755 "$APPDIR/AppRun"
: "${APPIMAGETOOL:?APPIMAGETOOL must name the pinned appimagetool binary}"
[[ -x "$APPIMAGETOOL" ]] || { echo "ERROR: APPIMAGETOOL is not executable" >&2; exit 1; }
APPIMAGE="$DIST/CopyPaste-v$VERSION-linux-$ARCHITECTURE.AppImage"
ARCH="$ARCHITECTURE" "$APPIMAGETOOL" "$APPDIR" "$APPIMAGE"
[[ -s "$APPIMAGE" && -f "$DEB" && -f "$RPM" ]] || {
  echo "ERROR: Linux package build did not produce every required format" >&2
  exit 1
}
