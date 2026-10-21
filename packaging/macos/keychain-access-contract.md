# macOS Keychain update contract

The release application retains the Homebrew/local-self-signing distribution
model. It uses one existing login-Keychain generic-password item:
service `com.copypaste.daemon`, account `device-secret-key`. Never rename, replace,
or delete that item as part of an application update.

## Persistent identity

Homebrew install steps run with a temporary `HOME`. Resolve the account's actual
support directory through the bundled native helper's `support-directory` command
(`getpwuid`), and retain the established local signing identity under
`Library/Application Support/com.copypaste.CopyPaste/signing`.

A stable self-signed certificate preserves a designated requirement, but
securityd still assigns this channel a build-specific `cdhash` partition. The
login-Keychain caller must therefore retain its executable identity too.

The installer signs the bundled `copypaste-keychain` with the local certificate
and hardened runtime, then installs it as `signing/copypaste-keychain-v1` only
when absent. Ordinary upgrades verify and preserve the existing v1 executable
byte-for-byte. An invalid, symlinked, or differently signed persistent helper
stops installation without resetting the device secret. A future security patch
to the accessor requires an explicit version/migration and new authorization.

## Authentication and lifetime

The daemon verifies the helper against its own signing requirement with the
helper's signing identifier. The helper validates its live parent daemon's
signature, exact signing identifier, and exact leaf certificate against its own.
It rechecks parent identity before releasing a key. It accepts only `load` or
`create` for the frozen service/account, and sends key material only through a
pipe to that authenticated caller. It exposes no socket, service, or general
credential lookup API.

The existing eight-second startup deadline applies. Every request ends with an
exited/reaped helper; timeout/error kills and reaps it. Key buffers are bounded
and wiped in native code and Rust. There is no retained worker, polling service,
network request, or periodic wakeup. Native resource qualification remains
unestablished until same-artifact app/daemon/helper peaks and repeated-start
growth are measured against the existing budgets (including unresolved fields).

Only an unambiguous missing-item result authorizes creation, and the existing
history-present guard still applies. Creation uses add, never update: a key
appearing after the absence check must not be overwritten.

The first adoption of v1 may require one user authorization for the existing
item. Ordinary app updates must not add another accessor identity or Keychain
item. Standalone/ad-hoc daemon launches retain their documented existing backend
and per-build permission behavior; do not represent that fallback as stable
Keychain access.

## Validation

`scripts/release/tests/test_macos_keychain_helper.py` builds two different daemon
fixtures with one signing identity. With all Keychain UI disabled, both must read
the same item through one unchanged accessor; duplicate creation and an invalid
signer must fail. It uses only disposable Keychains, restores the test environment,
and asserts the user's default/search list is unchanged. It also verifies that
an invalid retained helper stops installation.

`scripts/release/smoke-macos-production.sh` verifies accessor preservation across
two install passes with different `HOME` values, then launches the real released
application against an isolated test Keychain. Run the native fixture plus the
required CI/release safety suites for signing changes.

An isolated fixture does not qualify the installed-user adoption prompt or a
real Homebrew upgrade. Record those separately, retaining exact candidate commit
and artifact identity. Android and Windows keep their existing OS keystores and
the shared device-secret semantics.
