# Linux platform capability decision

Status: incremental adoption, October 2026.

## Baseline

`docs/engineering/ci-baseline.json` contains the existing measured workflow
samples. The corresponding audit reports seven successful CI samples with a
24m 05s median and 25m 49s p95, and two Linux native qualification samples
with a 4m 25s median and 6m 00s p95. Cache state is unknown for these samples.
The release audit found that a missing native evidence run ID was detected
after native builds began. These measurements describe the old workflows; no
time saving is attributed to the changes here.

## Decision

Clipboard capture is an operation with a pre-read source policy gate. At
startup, the Wayland data-control worker probes the protocol and seats for
clipboard operations when no source exclusions are configured. That probe is
off the daemon startup path; an inactive worker triggers a bounded bridge
reprobe.
With exclusions, source attribution becomes a requirement and an authenticated
GNOME or KWin bridge is selected when available. A bridge can serve capture
only after its D-Bus owner belongs to the current user and its protocol
version matches. Adapter probing calls only `Version`; it does not ask for a
selection snapshot or payload. A successful Wayland socket connection is the
primary session check; environment hints only prevent an XWayland fallback
while that socket is unavailable. X11 remains selected for X11; a Wayland
session never falls through to XWayland. If no bridge passes the probe, the
existing data-control adapter handles Wayland. Its source identity is not
authoritative, so configured source exclusions reject unknown sources before
payload reads. Status marks capture as needing a companion in that state.

Native capability reports carry a state, backend, reason, required permissions
and guarantee level. A missing probe reports `error` with no guarantee. A
clipboard read/write report also lists the adapter's MIME formats and the 4 MiB
product payload ceiling; a configured capture limit can be lower. A
legacy boolean remains for the current UI during migration. An unavailable
daemon overrides clipboard capability reports even when the GUI transport
probe succeeds. Flutter reads a narrow redacted daemon status view through its
existing Rust bridge, then uses the daemon's four clipboard capability entries
instead of inferring its chosen backend from the GUI's compositor probe.

`copypaste doctor --json` reads the existing daemon status through local IPC
and emits only fixed capability fields. New daemons supply typed clipboard
capabilities through the `ClipboardSource` port; older status replies use a
conservative backend fallback. Status uses the live clipboard backend name,
not the startup label. Daemon-only diagnostics report shortcuts, Quick
Paste, desktop integration and updates as unprobed because those probes live
in the GUI. An unreachable daemon produces a redacted error report and a
nonzero exit code.

The stable platform target and experimental build targets live in
`packaging/linux/release-contract.json`. Only rows with native qualification
can enter the exact-package qualification matrix. A package build never
establishes native acceptance.

Experimental architecture jobs derive their build and fixture matrices from
the same targets. CI change selection runs the patched compositor producer
only for compositor sources, integration ABI, toolchain and workflow changes;
Rust, Flutter and native fixture gates still run for platform code. This is a
configuration change, not a measured time improvement. The native scenario
driver records the installed CLI's redacted capability report beside its
signed-package evidence.

## Follow-up gates

The adaptive wrapper reprobes only while a bridge is disconnected or a
data-control adapter encounters source exclusions. The capture loop checks
that condition from current settings without a native clipboard read, so a
healthy idle data-control session has no extra worker wakeups. Probing is
bounded to one attempt per five seconds and runs on the blocking capture
worker. A bridge is
given one reprobe interval to reconnect before data-control takes over. Both
Wayland adapters initialize their capture cursors from the current selection
without marking that selection dirty; a switch can therefore discard the
selection in flight but cannot replay it as a new capture. The outer clipboard
lock serializes a switch with reads and writes. Test this handoff with real
compositor restarts and permission changes before claiming native acceptance.
An X11-to-Wayland session replacement still requires a daemon restart because
the process environment does not update when the user's login session changes.

Secure storage, desktop integration and update status need separate native
probes. Their current reports intentionally say that no probe is registered.
The experimental Wayland rows need installed-artifact native scenarios before
they can be marked qualified. Compiler cache changes and prepared CI images
require before/after measurements with equivalent workflow inputs. The native
scenario installer still provisions the GNOME/X11 fixture directly; a new
desktop row cannot be promoted to stable until its runner setup, assertion
set, and signed artifact receipt validation are driven by the platform target
as well.
