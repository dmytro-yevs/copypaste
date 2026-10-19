# Linux support

The declared platform matrix comes from `release-contract.json`. Regenerate the
table with `python3 scripts/release/platform_manifest.py table`.

| Distribution | Desktop | Session | Architecture | Packages | Tier | Native qualification |
| --- | --- | --- | --- | --- | --- | --- |
| ubuntu 24.04 | GNOME 46 | x11 | x86_64 | AppImage, deb | stable | required |
| ubuntu 24.04 | GNOME 46 | wayland | x86_64 | AppImage, deb | experimental | not yet qualified |
| ubuntu 24.04 | GNOME 46 | wayland | aarch64 | AppImage, deb | experimental | not yet qualified |
| fedora 44 | KDE 6 | wayland | x86_64 | rpm | experimental | not yet qualified |

Stable publication requires the exact-artifact native qualification described in
[native qualification](native-qualification.md). Experimental targets remain
unqualified for stable desktop parity. Fedora RPM installation evidence does not
establish complete KDE or Wayland desktop qualification.

Screenshot blocking is unavailable on Linux and must be shown as an explicit
platform limitation. A session's clipboard access alone does not establish
source-application attribution, privacy-marker enforcement, or Quick Paste input.
Use the runtime capability report and the maintained native qualification matrix
for those guarantees.
