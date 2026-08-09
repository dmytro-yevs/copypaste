# ADR-0020: Use NSIS with two independent Windows signatures

Status: accepted

## Decision

Ship one current-user NSIS installer in the shared product version stream.
Upgrades stop the user's daemon before files change, refuse downgrades, and
preserve the launch-at-login choice. Uninstall removes both the Run entry and
Windows' `StartupApproved` override.

Tauri owns installer generation, Authenticode invocation, and updater artifact
signing. Authenticode uses a release certificate selected by thumbprint and a
trusted timestamp; updater metadata uses Tauri's separate minisign-compatible
key. A signed build fails unless every certificate, key, HTTPS endpoint, and
release URL input is present, and verifies both the Authenticode result and
the updater signature artifact before packaging.

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
