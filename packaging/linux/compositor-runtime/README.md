# Sidecar compositor runtime packages

The maintained Mutter and KWin bridge sources are not a user-installable
feature by themselves. A release companion is built from an immutable runtime
output and this directory stages that output into an additional, opt-in
Wayland session. It never replaces the vendor compositor, changes a display
manager default, restarts a running session, installs an alternative, or
enables a service.

Each compiler run produces one receipt for one desktop family, distribution
release, architecture, source revision, and bridge patch. The receipt lists
every runtime file (including internal SONAME links), hashes every byte, pins
the glibc floor, and defines one of two launch modes:

* **KWin** has a private, receipt-listed entrypoint. The session launcher can
  execute only that entrypoint from `/usr/lib/copypaste/compositor-runtime`.
* **GNOME** currently has a maintained Mutter library output, not a private
  GNOME Shell. Its receipt therefore pins the matching system `gnome-session`
  and `gnome-shell` binaries by digest. The launcher verifies those exact
  binaries, gives that trusted matching system session only the private Mutter
  library/search paths, and refuses a mismatched distribution release. A
  compiler may switch to a private Shell only by producing a `private` receipt
  with every Shell byte listed.

When the output contains ELF files, staging also checks their machine type and
rejects any GLIBC symbol requirement above the declared floor. This prevents a
receipt from merely claiming an architecture or compatibility baseline that its
compiled bytes do not satisfy.

The compiler receipt uses schema 1:

```json
{
  "schema": 1,
  "runtime_id": "kwin-6.3-fedora40",
  "desktop": "KDE",
  "architecture": "x86_64",
  "distribution": {"id": "fedora", "version": "40"},
  "glibc_floor": "2.39",
  "source": {"revision": "<immutable source revision>", "patch_sha256": "<64 hex>"},
  "payload": [{"path": "bin/start-plasma", "type": "file", "mode": "0755", "sha256": "<64 hex>"}],
  "launch": {"kind": "private", "entrypoint": "bin/start-plasma"},
  "runtime_env": {},
  "package_dependencies": [{"name": "kwin", "version": "6.3.0"}],
  "upstream_licenses": [{"spdx": "GPL-2.0-or-later", "name": "usr/share/doc/copypaste/COPYING", "sha256": "<64 hex>"}]
}
```

Stage a compiler output into a package root with:

```sh
python3 packaging/linux/compositor-runtime/stage_runtime.py \
  --receipt /build/kwin-6.3-fedora40.receipt.json \
  --runtime-dir /build/kwin-6.3-fedora40 \
  --stage-root "$package_root"
python3 packaging/linux/compositor-runtime/verify_runtime_package.py \
  --root "$package_root" --runtime-id kwin-6.3-fedora40
```

Build the matching signed-repository input as one native companion package:

```sh
python3 packaging/linux/compositor-runtime/build_companion_package.py \
  --receipt /build/kwin-6.3-fedora40.receipt.json \
  --runtime-dir /build/kwin-6.3-fedora40 --version 1.2.3 \
  --format rpm --output dist/copypaste-compositor-runtime-kwin-6.3-fedora40.rpm
```

Build Debian and RPM companions in their matching distribution build roots,
with receipts that pin that target's package versions. The builder creates
`copypaste-compositor-runtime-<id>` and writes exact package dependencies from
the receipt, so a vendor compositor/session upgrade cannot silently retain an
incompatible sidecar. Repository signing happens after this build; unsigned
outputs are not an install instruction.

`scripts/release/build-linux-packages.sh` accepts the same inputs through
`COPYPASTE_COMPOSITOR_RUNTIME_RECEIPT` and
`COPYPASTE_COMPOSITOR_RUNTIME_DIRECTORY` for an explicit target build or a
packaging fixture. It is not the stable-release delivery path: one app-package
run cannot safely embed both Ubuntu and Fedora compositor runtimes. Stable
release gating consumes the separate signed companion assets and verifies their
producer receipts for both baseline desktops. The AppImage never contains a
sidecar session.

This creates only a private prefix, a receipt, a strict launcher, and
`/usr/share/wayland-sessions/copypaste-<id>.desktop`. The user selects the
named **GNOME (CopyPaste Clipboard)** or **Plasma (CopyPaste Clipboard)**
session at login. Removing the package removes only that additional session.

The AppImage remains portable. It cannot contain or activate a compositor
runtime; an AppImage user installs the matching signed `.deb` or `.rpm`
companion, then explicitly selects its session at login. A native qualification
run must start this generated launcher from the staged private prefix and bind
its receipt, package digest, distribution release, architecture, and glibc
floor to the release evidence. Source-only bridge bundles do not qualify a
release.

Current production baselines are GNOME 46 on Ubuntu 24.04 and KWin 6.0 on
Fedora 40, each on x86_64 and aarch64. A current GNOME compiler output contains
private Mutter libraries but no private Shell; it must not produce a companion
receipt until a reviewed session route proves the matching Shell loads those
private libraries. GNOME 47 and KWin 6.3 builds are ABI gates only until they
have matching session runtimes and native evidence.
