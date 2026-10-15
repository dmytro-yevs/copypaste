> Current stable release scope: x86_64, GNOME 46, X11 on Ubuntu 24.04.
> Every AppImage and Debian artifact must pass the complete installed-product
> scenario. ARM64, KDE, Wayland, and RPM implementation is retained for
> development outside stable desktop qualification. Releases may include a
> clearly labeled experimental RPM after the separate Fedora installation
> gate passes; this does not establish RPM desktop parity.

# Linux native qualification contract

Linux packages are release inputs only after `.github/workflows/linux-native-qualification.yml`
has produced the `linux-native-qualification` artifact. The production release
workflow consumes that artifact by run ID and verifies its package bytes again;
it never rebuilds a Linux package while qualifying it.

## Trusted source and exact artifacts

Dispatch the workflow from the same immutable commit as the current production
release run. Supply the current numeric run ID and, for regular releases, the
prior numeric run ID:

* `artifact_run_id`: the completed current `Production release` staging run
  containing `production-linux-x86_64`, with successful preflight and Linux build
  jobs. Its evidence gate remains closed until native qualification exists.
* `previous_artifact_run_id`: a successful earlier `Production release` run
  containing the released x86_64 packages, required for real upgrade scenarios.

The workflow verifies supplied API run records, complete job inventories, artifact
origins, repository, workflow path, and commit before downloading anything. It
then checks out the current artifact commit. A dispatch from another commit, a
partial artifact inventory, expired artifact, or different repository fails.

Each row records the qualification workflow run ID as `source_run_id` and the
package-producing release run ID as `artifact_run_id`. The receipt binds every
current AppImage and deb filename, byte count, and SHA-256 for its own
architecture. It also binds a prior package inventory used by upgrade tests.
The final verifier rejects duplicate formats, cross-architecture packages,
symlinks, paths outside the evidence root, missing files, byte-size or digest
changes, stale commits, partial matrices, and non-boolean assertion values.

The first Linux release has no legitimate older Linux package to upgrade.
Dispatch it without `previous_artifact_run_id`; it produces the explicit
`first_install_baseline` receipt with `clean_install_baseline`, never an
invented upgrade result. That signed receipt is the valid upgrade baseline for
the following release, which must use `prior_release` and execute upgrades.

When dispatching the final production workflow, supply `linux_artifact_run_id`
and `linux_evidence_run_id`, plus `previous_linux_artifact_run_id` set to the
same prior run used by native qualification. The production gate authenticates
that prior source, downloads its exact packages, and rechecks upgrade evidence
against their bytes. Omitting the prior source cannot qualify `prior_release`
evidence.

Build the standalone compositor runtime before staging an application release.
Pass its completed run ID as `compositor_runtime_run_id` to the staging,
native qualification, and final production workflows. All three must consume
the same producer bytes; separate builds at the same commit are insufficient.

## Required matrix and scenarios

The released coordinate is mandatory:

| Architecture | Desktop | Sessions | Formats |
| --- | --- | --- | --- |
| x86_64 | GNOME 46 (Ubuntu 24.04) | X11 | AppImage, deb |

For each coordinate, the repository-maintained GTK/GIO fixture driver must
execute and emit a command receipt for every scenario below. A JSON receipt
supplied as an input cannot mark a case successful:
`produce-linux-native-qualification.py` captures only structured command
output, stores a raw driver log and trace, and computes the receipt hashes.

* Install and run each package, verify desktop entry, URI handler, icon,
  autostart template, daemon, CLI, and GUI; then upgrade each package from the
  prior stable artifact while preserving encrypted history.
* Copy from another application and restore text, HTML, RTF, PNG, TIFF, and
  file payloads.
* Exercise confidential hints, excluded-app capture, and private mode without
  retaining or replaying protected content.
* Invoke Quick Paste through the native X11 hotkey and prove focus restoration
  and paste in the active GNOME session. KDE and Wayland scenarios remain
  available for development but are outside this release matrix.
* Exercise tray/window state, capture notification, daemon restart with
  encrypted persistence, pairing and a sync transfer, and every declared
  native module.

Confidential-read evidence is bound to the GUI-owned daemon: X11 uses XRes
requestor PIDs, and GNOME Wayland observes authenticated Clipboard v2 `Read`
calls. The daemon must inspect the confidential hint without requesting the
protected payload. Unknown readers fail qualification; independent desktop
clipboard-manager requests remain visible in the audit. History must remain
unchanged throughout the existing privacy observation window.

The raw trace lists the actual command argv, exit code, and assertions covered
by each command. The final verifier reads the trace and requires successful
coverage of every assertion, in addition to checking the raw attachment bytes.
No clipboard payload, pairing secret, database, credential, or screenshot is
uploaded. A screenshot is not a release requirement; command traces and
non-sensitive state evidence are the authoritative record.

## Hosted desktop execution and evidence

The release workflow uses a GitHub-hosted x86_64 runner. It creates a fresh
D-Bus runtime, software-rendered virtual X11 display or native Wayland socket,
and invokes `linux-native-fixture-driver.py`. The driver launches the exact
AppImage daemon, CLI, and GUI plus a separate GTK source window, writes real
clipboard MIME targets, and observes the product through the CLI. It does not
depend on a self-hosted runner label.

The driver executes confidential/private capture, Quick Paste portal and focus
restoration, tray/notifications, restart, pairing/sync, native modules,
onboarding, camera pairing, and external file drag against the installed
application. Each assertion requires the corresponding observed product
behavior; a fixture or input cannot supply a successful result. A failing
scenario leaves the matrix unqualified. The existence of these executable
checks does not establish that the current implementation has passed them.

Ubuntu 24.04 carries Plasma 5 and cannot qualify the Plasma 6 KWin companion.
RPM install/upgrade also needs a Fedora runtime, not extraction on Ubuntu.
`fedora:40@sha256:3c86d25fef9d2001712bc3d9b091fc40cf04be4767e48f1aa3b785bf58d300ed`
is the pinned multi-architecture official base selected for that runtime;
validate its Plasma 6 compositor, `kpackagetool6`, portals, and software
renderer before it is accepted into the matrix. Preserve this workflow's
source-run provenance and exact artifact downloads; do not replace a missing
desktop capability with a manual receipt or generic success switch.
