# ADR-0020: Use NSIS with two independent Windows signatures

Status: accepted

## Decision

Ship one current-user NSIS installer in the shared product version stream.
Upgrades refuse a running current-user GUI, then use the embedded signed CLI
to stop the user's daemon before files change; they refuse downgrades and
preserve the launch-at-login choice. Uninstall uses the same handoff after its
confirmation, removes payloads before associations or state, and removes both
the Run entry and Windows' `StartupApproved` override.

An app-initiated update verifies its download before requesting the canonical
service drain. An owned daemon acknowledges and exits before installation; an
adopted daemon uses authenticated IPC shutdown and confirmed endpoint stop, or
the update refuses. The reservation remains held through installer handoff, so
the app cannot start a second daemon while the update exits it.

## Installer shutdown completion contract

The Windows installer invokes `copypaste shutdown --wait-for-exit` when it
must drain an adopted daemon. The command takes one five-second deadline across
connect, named-pipe server PID lookup, process-handle acquisition, shutdown
write, ACK, and a single process wait. It opens the connected server process
with only `QUERY_LIMITED_INFORMATION | SYNCHRONIZE` before writing the request,
then accepts completion only for a correlated `empty` ACK followed by
`WaitForSingleObject(OBJECT_0)` and `GetExitCodeProcess() == 0`.

An initial pipe `NotFound` result is the sole already-stopped success. Access
failures, a closed post-connect pipe, a malformed or failed ACK, timeout,
abandonment, API failure, or nonzero exit make installation refuse without
trying another connection or guessing a process. The command emits a
human-readable completion state only; `--json` conflicts with this Windows-only
flag so it cannot present a fabricated IPC response.

The generated NSIS template is a narrow, hash-checked transform of Tauri
`tauri-cli-v2.11.4`'s installer template. Its signed CLI helper is a single
Tauri resource, checked by target name and extracted into the installer and
uninstaller plugin directory; it is never installed as product payload. The
release build checks template drift before bundling, then verifies the exact
staged helper bytes with the non-mutating embedded-signature verifier after
Tauri has signed resource inputs.

The daemon handoff and payload path have no force-kill, reboot cleanup, retry,
or legacy uninstaller handoff. They fail closed before later payload, registry,
or shortcut mutation when the GUI is present, the bounded CLI drain fails, or
an individual payload write/delete fails. An absent or invalid installed
version also refuses rather than being guessed as older. File-by-file
replacement is not an atomic executable-set transaction: a concurrent launch
can leave a partial binary update and a nonzero refusal that requires a new
user attempt; the installer never retries or deletes automatically to recover
it.

An app-initiated updater can hand off to NSIS before the old GUI has fully
exited. The GUI-presence refusal deliberately rejects that race; it does not
add a guessed wait or retry, and automatic update success remains unqualified
until an installed Windows flow demonstrates the handoff.

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
Verification invokes SignTool's embedded-signature policy without catalog
lookup, then uses the .NET PE and CMS libraries to inspect the embedded signer,
digest algorithm, and RFC 3161 timestamp without parsing localized tool output.
Embedded PE certificate data is limited to 16 MiB before allocation; installers
remain streamed from disk.

Updater metadata uses Tauri's separate minisign-compatible key. A signed build
fails unless every certificate, key, endpoint, and release URL input is
present, and verifies the Authenticode signer, timestamp, and updater signature
artifact before packaging.

Release artifacts are named
`CopyPaste-v<version>-windows-x86_64-setup.exe`. `SHA256SUMS` contains relative
names only, and signed releases include Tauri's detached signature plus static
`latest.json` metadata. The publish job is the single owner of the official
feed and combines the Windows entry with the signed `android-universal` APK
entry; the Windows packaging self-test may still use a Windows-only feed while
that artifact is exercised in isolation.

## Rule 1 exemption 1

No maintained package knows this repository's two sidecar names, canonical
release filename, or hosting URL. The narrow PowerShell layer stages those
inputs and writes checksums and static metadata; it does not implement an
installer, Authenticode, or updater cryptography.

The same exemption covers the narrow composition of .NET's maintained PE and
CMS APIs: neither exposes one call returning all release-policy fields, while
PowerShell's path cmdlet deliberately prefers a catalog signature over an
embedded signature.

## Consequences

Unsigned builds are explicit evidence artifacts, not releasable substitutes.
The certificate and updater private key remain outside the repository and CI
configuration; release infrastructure supplies them at execution time.

## References

- [Tauri v2 Windows code signing](https://v2.tauri.app/distribute/sign/windows/)
- [Microsoft SignTool options](https://learn.microsoft.com/windows/win32/seccrypto/signtool)
- [Get-AuthenticodeSignature catalog precedence](https://learn.microsoft.com/powershell/module/microsoft.powershell.security/get-authenticodesignature)
- [.NET PEReader](https://learn.microsoft.com/dotnet/api/system.reflection.portableexecutable.pereader)
- [.NET SignedCms](https://learn.microsoft.com/dotnet/api/system.security.cryptography.pkcs.signedcms)
- [.NET AsnReader](https://learn.microsoft.com/dotnet/api/system.formats.asn1.asnreader)
