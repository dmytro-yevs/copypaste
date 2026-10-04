# Release qualification

Application releases remain unavailable while Flutter release packaging and
same-artifact qualification are incomplete.

A future release requires a Flutter product, Rust bridge, platform hosts,
signed exact artifacts, installation and upgrade evidence, and native and
accessibility evidence for approved product flows. Until then the release
workflow fails closed and cannot publish an application. The in-app update
client does not change this boundary: publication must first produce the signed
Homebrew cask, Windows installer, Android APK, detached updater signatures, and
GitHub asset digests that the client requires.
