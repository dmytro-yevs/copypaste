# ADR-0030: System device names with an explicit manual override

Status: accepted.

## Decision

Every product uses the OS display name, then the hardware model, then
`CopyPaste device`. A name is bounded and stripped of controls before it is
stored or advertised. A stable device ID remains independent of its label.
An explicit rename saves a manual override in the same transaction as the
name and origin registry. Automatic names refresh every five seconds while
the backend runs. The UI and authenticated sync read the same stored name.
There is no legacy-name inference or migration: absent provenance means automatic.

## Dependencies and exemption

Desktop names use maintained `whoami` 2.1, which wraps macOS
`SCDynamicStoreCopyComputerName` and Windows `GetComputerNameExW`.
Model fallback uses `sysctl` (`hw.model`) on macOS and `winreg`
(`HARDWARE\DESCRIPTION\System\BIOS`, `SystemProductName`) on Windows.
These wrappers avoid a second implementation of the platform APIs.

No evaluated package supplies the selected Android display name:
`whoami` lists Android as planned, and `sysinfo` exposes hostnames rather
than `Settings.Global.DEVICE_NAME`. The Android adapter therefore uses the
existing JNI/context boundary to read `Settings.Global.getString` and
`Build.MODEL`. This claims the no-fitting-package exemption for that adapter
and for application-specific precedence/override policy. It introduces no
new permission, crypto implementation, or TLS stack. Java exceptions clear
before trying the model fallback, and JNI local references are frame-bounded.

Sources:
- https://docs.rs/crate/whoami/2.1.3
- https://docs.rs/whoami/2.1.3/whoami/fn.devicename.html
- https://docs.rs/sysinfo/latest/sysinfo/struct.System.html
- https://developer.android.com/reference/android/provider/Settings.Global#DEVICE_NAME
- https://developer.android.com/reference/android/os/Build#MODEL

## Validation

Storage tests cover automatic updates, stable IDs, manual override including
an unchanged label, reopen, rejected edits, and transaction rollback. Backend
adapter tests cover the cached and published label. Each platform still
requires native evidence; host tests cannot qualify Android or Windows.
