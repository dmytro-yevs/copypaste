# ADR-0020: Use NSIS with two independent Windows signatures

Status: accepted

## Decision

Ship one current-user NSIS installer in the shared product version stream.
Upgrades stop the user's daemon before files change, refuse downgrades, and
preserve the launch-at-login choice. Uninstall removes both the Run entry and
Windows' `StartupApproved` override.

Tauri owns installer generation and updater artifact signing. Its custom
`signCommand` uses PowerShell 7 and delegates every Authenticode operation to
`scripts/release/windows-sign.ps1`, using the release PFX directly rather than
importing certificates into Windows stores. The same script prepares and
validates the PFX, normalises the RFC 3161 timestamp URL for SignTool, performs
the workflow smoke signature, and signs every file Tauri supplies.

The timestamp transport is HTTP because SignTool rejects HTTPS timestamp URLs;
the RFC 3161 response is itself signed. Authenticode uses SHA-256 for both the
file and timestamp digests. The workflow never imports into `Root` or
`TrustedPublisher`, and deletes the temporary PFX after the build.

The script exposes five operations: `Prepare` decodes and validates the PFX and
persists its path plus the normalised timestamp URL; `Validate` fails before a
long build when that state is unusable; `Sign` is the sole bounded SignTool
entry point; `Cleanup` removes the PFX; and `SelfTest` exercises preparation and
signer validation with an ephemeral certificate without changing trust stores.

Updater metadata uses Tauri's separate minisign-compatible key. A signed build
fails unless every certificate, key, endpoint, and release URL input is
present, and verifies the Authenticode signer, timestamp, and updater signature
artifact before packaging.

Release artifacts are named
`CopyPaste-v<version>-windows-x86_64-setup.exe`. `SHA256SUMS` contains relative
names only, and signed releases include Tauri's detached signature plus static
`latest.json` metadata for `windows-x86_64`.

## Rule 1 exemption 1

No maintained package knows this repository's two sidecar names, canonical
release filename, or hosting URL. The narrow PowerShell layer stages those
inputs and writes checksums and static metadata; it does not implement an
installer, Authenticode, or updater cryptography.

## Consequences

Unsigned builds are explicit evidence artifacts, not releasable substitutes.
The certificate and updater private key remain outside the repository and CI
configuration; release infrastructure supplies them at execution time.

## References

- [Tauri v2 Windows code signing](https://v2.tauri.app/distribute/sign/windows/)
- [Microsoft SignTool options](https://learn.microsoft.com/windows/win32/seccrypto/signtool)
