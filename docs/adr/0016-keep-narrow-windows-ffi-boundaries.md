# ADR-0016: Keep two narrow Windows FFI boundaries

Status: accepted

## Decision

Keep direct `windows-sys` calls for DPAPI and clipboard source attribution.
These are the only hand-written Windows API boundaries left in the device
secret, clipboard, IPC, daemon lifecycle and desktop shell paths.

Named-pipe connection waiting, listener instance replacement, security
descriptor ownership and account-SID lookup are instead delegated to
[`interprocess` 2.4](https://crates.io/crates/interprocess/2.4.3) and
[`win-security-identifier` 0.2](https://crates.io/crates/win-security-identifier/0.2.0).

## DPAPI — rule 1 exemption 1

No maintained wrapper provides all four required behaviours: current-user
scope with optional entropy, `CRYPTPROTECT_UI_FORBIDDEN`, zeroization of the
DPAPI-owned plaintext buffer before `LocalFree`, and the raw error code needed
to distinguish an unusable blob from a temporarily unavailable key.

- [`windows-dpapi` 0.2](https://github.com/sheridans/windows-dpapi) supports
  user scope and entropy, but passes zero flags for user scope, frees the
  plaintext buffer without wiping it, erases the Windows error into
  `anyhow::Error`, and adds the older `winapi` binding stack.
- [`stellar-agent-windows-identity` 0.1.0-alpha.5](https://crates.io/crates/stellar-agent-windows-identity/0.1.0-alpha.5)
  forbids UI and wipes plaintext, but deliberately exposes no optional
  entropy. Adopting it would change the v2 sealed-blob contract.
- [`windows-native-keyring-store`](https://crates.io/crates/windows-native-keyring-store)
  uses Credential Manager. Its enumerable generic credentials have the trust
  shape ADR-0013 rejected for the device secret.

The cost is about seventy lines of unsafe FFI and direct error classification
that this workspace owns. Real DPAPI tests cover wrong entropy, corruption,
user scope, buffer length and concurrent creation.

## Clipboard attribution — rule 1 exemption 1

No maintained package answers “which process owns this clipboard write” while
reading neither the window title nor the full process table. `clipboard-win`
provides the owner window, but turning that handle into a PID and executable
name remains four direct Windows calls.

[`active-win-pos-rs` 0.11](https://crates.io/crates/active-win-pos-rs/0.11.0)
reports the foreground window and its title, not the clipboard owner. Using it
would both change attribution and collect user content. [`sysinfo`
0.39](https://crates.io/crates/sysinfo/0.39.6) can snapshot executable paths
after a PID is known, but still cannot map the owner window to that PID; it
would replace one direct query with a system-wide, time-shifted process scan.

The cost is about forty lines of unsafe FFI. The path is reduced to its file
name immediately, elevated or exited processes fail to no attribution, and a
non-empty exclusion list already fails closed when attribution is absent.

## Consequences

New Windows FFI in either boundary requires another candidate audit. A wrapper
becomes preferable as soon as it preserves the listed behaviour; binary size
or build time alone is not a reason to keep the direct calls.
