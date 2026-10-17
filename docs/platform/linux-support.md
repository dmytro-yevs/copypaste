# Linux platform targets

The machine-readable source is `packaging/linux/release-contract.json`. Generate
this table with `python3 scripts/release/platform_manifest.py table`.

| Distribution | Desktop | Session | Architecture | Packages | Tier | Native qualification |
| --- | --- | --- | --- | --- | --- | --- |
| ubuntu 24.04 | GNOME 46 | x11 | x86_64 | AppImage, deb | stable | required |
| ubuntu 24.04 | GNOME 46 | wayland | x86_64 | AppImage, deb | experimental | not yet qualified |
| ubuntu 24.04 | GNOME 46 | wayland | aarch64 | AppImage, deb | experimental | not yet qualified |
| fedora 44 | KDE 6 | wayland | x86_64 | rpm | experimental | not yet qualified |

The GNOME Wayland x86_64 pilot uses the existing x86_64 package build and
Wayland adapter fixtures. It remains experimental until an installed package
session on its declared runner passes native qualification. An experimental
package build is not native acceptance. Only the stable row has exact
installed-artifact qualification in the current release workflow.

Run `copypaste doctor --json` to inspect daemon-visible capabilities without
printing clipboard contents, history, device identity or network addresses.
GUI-only capabilities currently report `error` with a reason stating that no
daemon-side probe is registered. A missing daemon returns a redacted JSON
report and a nonzero exit code.
