# CopyPaste native KWin clipboard bridge

KWin's public Script and Effect package APIs do not expose the Wayland seat
selection owner. CopyPaste therefore ships this source as a small in-process
KWin patch, rather than claiming that a JavaScript companion can prove producer
identity.

The patch is only for KWin 6.0 through 6.3. A distribution maintainer applies
the matching integration patch while building KWin from its authenticated source
package. CopyPaste packages carry the source and patch recipes, but do not
modify, replace, or restart a user's compositor. This bridge is unavailable
until their distribution ships the patched KWin build.

## D-Bus v2 contract

The bridge registers on KWin's existing session service (`org.kde.KWin`) at
`/app/copypaste/Clipboard`, interface `app.copypaste.Clipboard`.

| Method or signal | Signature | Rule |
| --- | --- | --- |
| `Version` | `() -> u` | Always returns `2`. |
| `Snapshot` | `() -> t, as, (s u u s)` | Returns exactly three top-level out arguments: generation, MIME names, and writer identity. |
| `Read` | `(t s u) -> ay` | Reads only the supplied generation and MIME name. A replacement before completion is `StaleSelection`. |
| `Write` | `(a{say}) -> t` | Creates a bounded KWin-owned selection source. |
| `OwnerChanged` | `(t as (s u u s))` | Emitted after every KWin seat-selection replacement. |

The identity tuple is `(status, pid, uid, app_id)`. `status` is one of
`verified`, `no-client`, `no-app-id`, or `ambiguous`; `app_id` is nonempty only
for `verified`. `verified` requires exactly one nonempty raw XDG `app_id` among
the current Wayland toplevel windows whose `surface()->client()` is the exact
client that owns the clipboard data source. The versioned KWin patch exposes a
typed `rawAppId()` accessor backed by the XDG toplevel protocol value before
KWin rules can rewrite `desktopFileName`. It never uses the focused window,
title, a PID lookup, executable inference, or a timing relationship. The
compositor reads Wayland credentials from that same client before any payload
read. The daemon must fail closed unless `status == "verified"`.

Only the current unique owner of `app.copypaste.Daemon` with the KWin session
UID can invoke the bridge. `Read` is bounded to 4 MiB and two seconds; `Write`
allows at most 64 MIME types, 4 MiB each, and 32 MiB in total. The bridge
returns `AccessDenied`, `StaleSelection`, `UnsupportedMime`, `TooLarge`, or
`Unavailable` as D-Bus errors. Authorization gives each D-Bus name and UID
lookup a 250 ms bound. It allows one pending read and eight pending writes per
KWin-owned selection, cancels all pending transfers when the selection or
daemon name owner changes, and rejects a queued read reply when its captured
daemon-owner epoch no longer matches. It does not log payloads. A KWin-owned
write has `no-client` identity, so the daemon's fail-closed exclusion also
prevents it from being attributed to another application or recaptured as an
external producer.

## Maintainer integration

Run `./apply-to-kwin-source.sh <6.0|6.3> /path/to/kwin-source` from this
directory. It copies the bridge sources into the KWin source tree and applies a
small versioned integration patch. Then build KWin through the distribution's
normal authenticated source-package workflow. The script refuses any other
version and never installs output or touches a live KWin process.

## Hosted Linux qualification recipe

The source bundle is not qualification evidence. Hosted Linux must build and
run the exact patched compositor in a disposable Fedora 40 container for each
immutable upstream release below:

| KWin release | Immutable ref | Bridge patch argument |
| --- | --- | --- |
| 6.0.0 | `1ddcb4e288c4f7dcecdc94efccd655b7e3666d30` | `6.0` |
| 6.3.0 | `3e19ea5a1bd69fa619aa5fd3c1b285e5b9168b5b` | `6.3` |

The container must install `dnf-plugins-core`, enable source repositories, run
`dnf builddep --assumeyes kwin`, and install `cmake`, `ninja-build`, `git`,
`dbus-daemon`, `dbus-tools`, `plasma-workspace-x11`, `kwin-wayland`, and
`wl-clipboard`. It clones the selected ref with `git -c protocol.version=2
clone --filter=blob:none`, verifies `HEAD` exactly, applies this source bundle,
then runs `cmake -B build -G Ninja`, `cmake --build build`, and the resulting
`kwin_wayland --virtual --no-lockscreen` under `dbus-run-session`.

Runtime qualification must call `Version`, verify the v2 method signatures,
create a real Wayland clipboard producer, assert a `verified` identity only for
one exact-client app ID, then prove `StaleSelection` when the producer replaces
the selection during `Read`. It must also prove `no-app-id` and `ambiguous`
states exclude capture. These commands belong in the hosted pinned-KWin job;
they cannot be replaced by a package fixture or an API probe.

The executable build check is `run-fedora-build.sh`. Run
`./run-fedora-build.sh linux/amd64 all` and
`./run-fedora-build.sh linux/arm64 all`; each call builds the pinned Fedora 40
builder, checks out both immutable sources, applies the patch, configures CMake,
and completes the Ninja build without installing a compositor on the host.
`DISTRIBUTION.md` describes the required opt-in, signed side-by-side runtime and
session route for making the bridge usable.

`test-wire.sh` is a Qt 6-only private peer-to-peer D-Bus serialization fixture.
Set `COPYPASTE_QT_PREFIX` to the Qt 6 SDK prefix; it asserts that `Snapshot`
replies with the exact shared `tas(suus)` signature and three top-level values,
rather than one enclosing struct.
