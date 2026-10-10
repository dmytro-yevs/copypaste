# Shipping the native KWin bridge

The source bundle is an interim maintainer handoff. It does not make the
bridge available to ordinary CopyPaste users. The release path is a signed,
opt-in companion repository that ships a side-by-side KWin runtime and a
separate Plasma Wayland session.

## Package set

For each supported distribution and architecture, the authenticated CopyPaste
repository publishes a `copypaste-kwin-bridge` package built from that
distribution's KWin source package plus the matching bridge patch. The release
matrix has separate Debian-family `.deb` and Fedora-family `.rpm` packages for
`x86_64` and `aarch64`; each row requires its own exact source, patch, build,
and clean-VM runtime receipt. The package contains only its private runtime
prefix, its bridge source receipt, and a session descriptor named **Plasma
(CopyPaste Clipboard)**. It is signed by the repository's normal APT Release or
RPM metadata key and is built only after the exact source revision, bridge
patch, and native qualification receipt match.

The Linux AppImage remains portable and does not install, replace, or enable a
compositor component. An AppImage user explicitly installs the matching signed
distribution companion, then chooses the separate session at login. The same
Version 2 handshake gates clipboard capture for AppImage, `.deb`, and `.rpm`
installs on both architectures.

The KWin 6.0 runtime keeps its KDecoration2 dependency closure inside its
private prefix. Its production receipt must list the actual KDecoration2
SONAME closure found by the producer, and the package validation must show that
its RPM has no `Requires`, `Obsoletes`, or `Conflicts` on the vendor
`kdecoration` packages. Installing this private runtime must never downgrade or
replace a user's KDecoration3 desktop.

The package must never overwrite `/usr/bin/kwin_wayland`, register an
`alternatives` target, change a display manager default, enable a service, or
restart a compositor. Its installation only makes the session visible. The user
explicitly selects **Plasma (CopyPaste Clipboard)** in the display manager and
may return to the vendor Plasma session at the next login. Removing the package
removes that additional session without changing the vendor session.

## Session launcher acceptance contract

The signed package owns a small session launcher. Before publishing it for a
distribution release, qualification must prove that the launcher starts only
the private patched KWin runtime, exposes `org.kde.KWin` plus Clipboard v2, and
does not execute or replace the vendor KWin binary. The launcher is specific to
the target distribution's Plasma session startup contract; it is not published
until this is demonstrated in a clean VM for that exact distribution version.

The companion repository maintains separate packages for each supported KWin
minor family and CPU architecture. The package manager resolves only a package
whose vendor KWin/Plasma dependencies match the installed session. An upgrade
that no longer matches keeps the vendor session available and disables the
bridge session from eligibility until a newly qualified package is published.

## User enablement and app contract

CopyPaste treats the bridge as available only after the active session returns
`Version() == 2` from `org.kde.KWin` at `/app/copypaste/Clipboard`. It does not
infer success from package presence. The user enables it by selecting the
separate session at login; there is no automatic compositor replacement. If the
bridge session is absent or fails the version handshake, Wayland clipboard
capture stays unavailable rather than accepting unknown writer identity.
