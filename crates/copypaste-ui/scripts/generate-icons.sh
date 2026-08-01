#!/bin/sh
set -eu

output_dir=$(mktemp -d "${TMPDIR:-/tmp}/copypaste-icons.XXXXXX")
npm exec tauri icon -- src-tauri/icons/icon-manifest.json --output "$output_dir"

for icon in 32x32.png 64x64.png 128x128.png 128x128@2x.png icon.png icon.icns icon.ico; do
  cp "$output_dir/$icon" "src-tauri/icons/$icon"
done

for density in mdpi hdpi xhdpi xxhdpi xxxhdpi; do
  cp "$output_dir/android/mipmap-$density/"*.png "src-tauri/gen/android/app/src/main/res/mipmap-$density/"
done
mkdir -p src-tauri/gen/android/app/src/main/res/mipmap-anydpi-v26
cp "$output_dir/android/mipmap-anydpi-v26/ic_launcher.xml" src-tauri/gen/android/app/src/main/res/mipmap-anydpi-v26/

sips -z 36 36 -s format png src-tauri/icons/copypaste-monochrome.svg --out src-tauri/icons/trayTemplate.png
