# Linux platform contract

Linux joins the shared Flutter/Rust application as a production target. The
implementation and release qualification must cover both x86_64 and ARM64.
Each stable application release must produce signed AppImage, Debian, and RPM
packages for both architectures. This document defines the acceptance contract;
it does not establish that a particular release has passed qualification.

## Functional contract

The same shared UI, encrypted history, typed clipboard content, privacy settings,
paired-device synchronization, search, file import, desktop notifications, tray,
window lifecycle, Quick Paste, and desktop-compatible optional modules apply on
Linux. Platform adapters implement OS integration; screens do not fork their
own Linux UI or business logic.

The sole accepted platform exception is **Block screenshots**: Linux must
display its unavailability and must not claim to protect GTK windows from
screenshots or screen recording. This does not change encrypted storage or
clipboard privacy controls.

Wayland Quick Paste includes GNOME Shell and KDE KWin companions for restoring
the previous application's focus. The user must install and enable the matching
companion and approve Remote Desktop keyboard access. Packages must not enable
extensions or acquire that permission silently.

Clipboard integration must cover X11 and native Wayland on GNOME and KDE.
Changing a Wayland session to XWayland is not evidence of native Wayland support.
Unavailable protocols, locked keyrings, rejected permissions, and unsuccessful
installations must remain explicit errors rather than successful no-ops.
Source attribution follows the existing best-effort contract: it must never
invent an application identity. Source exclusions must remain safe when the
OS cannot establish the producer.

Production device secrets belong in the desktop Secret Service. A missing or
locked service must not select the development plaintext-file backend or mint
a replacement identity for existing history. Test keyrings must be disposable
and isolated from the developer's production keyring.

## Distribution and updates

Packages contain the Flutter bundle and its Rust daemon. System package updates
use the platform package manager and its authentication boundary. AppImage
updates must verify the publisher signature and digest before replacing a
user-owned executable. Starting an installer is not successful installation.

Release receipts bind the commit, workflow run, architecture, package format,
filename, byte length, and SHA-256 to the exact artifact exercised. Publication
must fail if required artifacts, signatures, or native receipts are absent.
Release recovery must verify the original artifact and qualification contract.

## Native acceptance

Each declared desktop/session combination must exercise an installed release
artifact: background copies from another application, supported clipboard types,
privacy hints and exclusions, clipboard writes, Quick Paste hotkey and insertion,
tray and window lifecycle, restart persistence, pairing and synchronization,
native module loading, and installation/upgrade of the selected package format.
An X11 test under Xvfb does not qualify Wayland, and unit tests do not replace
these installed-artifact scenarios.

Record the actual distribution, desktop and compositor versions, architecture,
permissions and required desktop integrations alongside results. Publish only
the support matrix for which those checks pass; successful compilation alone
does not establish functional parity.
