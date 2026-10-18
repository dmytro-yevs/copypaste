# Linux platform targets

The machine-readable source is `packaging/linux/release-contract.json`. Generate
this table with `python3 scripts/release/platform_manifest.py table`.

| Distribution | Desktop | Session | Architecture | Packages | Tier | Native qualification |
| --- | --- | --- | --- | --- | --- | --- |
| ubuntu 24.04 | GNOME 46 | x11 | x86_64 | AppImage, deb | stable | required |
| ubuntu 24.04 | GNOME 46 | wayland | aarch64 | AppImage, deb | experimental | not yet qualified |
| fedora 44 | KDE 6 | wayland | x86_64 | rpm | experimental | not yet qualified |

An experimental package build is not native acceptance. Only the stable row
has exact installed-artifact qualification in the current release workflow.

Run `copypaste doctor --json` to inspect daemon-visible capabilities without
printing clipboard contents, history, device identity or network addresses.
GUI-only capabilities currently report `error` with a reason stating that no
daemon-side probe is registered. A missing daemon returns a redacted JSON
report and a nonzero exit code.
