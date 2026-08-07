# Windows packaging

What `crates/copypaste-ui/src-tauri/tauri.windows.conf.json` decides, what it
deliberately does not, and what signing would cost. ADR-0013 leaves the
installer format, the signing question and the version stream open; nothing here
closes any of them.

**Unverified.** No Windows build has been executed. Every claim below about what
the bundler emits comes from Tauri's documentation and from the shape of the
macOS and Android paths in this repository. The `Windows installers` job in
`ci.yml` is what turns it into fact; read its log before trusting this file.

## What the config decides

**Both MSI and NSIS are built.** ADR-0013 lists the format as undecided, so the
config produces both and the CI job reports each one's size. Deciding by reading
is the failure this avoids; the numbers arrive on the first green run.

MSIX is not a Tauri bundler target at all. Reaching it means external tooling and
a signed package or a Store listing, so it is not a variation on this
configuration — it is a different pipeline.

**NSIS installs per user (`installMode: currentUser`).** No UAC prompt, no
elevation, and the app lands under `%LOCALAPPDATA%`. That matters more than usual
here: an unsigned installer that also asks for administrator rights shows the
yellow *Unknown publisher* consent dialog, which is a much worse first
impression than the blue one. It also matches what CopyPaste is — a per-user
clipboard history with a per-user daemon and per-user secret storage.

Tauri's WiX template has no per-user mode, so the MSI is per-machine and will
elevate. That asymmetry is one of the inputs to the format decision.

**WebView2 is fetched at install time (`downloadBootstrapper`).** Windows 11
ships WebView2; Windows 10 usually has it through Windows Update and Server SKUs
often do not. The alternatives trade the installer's network dependency for
size: `embedBootstrapper` is similar in effect, `offlineInstaller` adds roughly
130 MB to every download, and `fixedRuntime` adds more and pins a runtime version
this project would then have to update by hand. Revisit if a user cannot install
offline.

**`contentProtected`, `dragDropEnabled` and the window geometry are restated in
full.** Tauri merges a platform config over the base one as JSON, and JSON merge
*replaces* arrays rather than merging their elements — so `app.windows` in
`tauri.windows.conf.json` is the whole window definition, and a field left out of
it is lost rather than inherited. `contentProtected` is INV-35 and the security
property this file most easily drops by accident; `lib.rs` has a test that fails
if it goes missing.

**`transparent` is `false`, where macOS has `true`.** There is no vibrancy layer
behind a Windows window for a translucent client area to reveal. This is the one
appearance change from macOS and it is the least verified line in the file.

## What the config does not decide

**The daemon and the CLI are not in the installer.** Tauri bundles only its own
binary; on macOS `build-macos-app.sh` injects `copypaste` and `copypaste-daemon`
into the `.app` afterwards. The Windows equivalents are `bundle.externalBin`
(triple-suffixed filenames, copied next to the app executable) or
`bundle.resources`. Neither is configured, because ADR-0013 leaves the daemon's
Windows lifecycle open — a service, a Run-key process, or spawned by the app —
and how it is packaged follows from that, not the other way round.

**There is no Windows job in `release.yml`.** ADR-0013 has not decided whether
Windows ships in the same version stream. Until it does, the installers exist as
a CI artifact and nothing publishes them.

## Signing

Nothing in this repository holds a signing credential. That is ADR-0001's
premise for macOS, and adding an Authenticode credential would change it for the
project rather than for one platform — so it needs an ADR before it needs a
secret. The `ci.yml` job asserts that neither `certificateThumbprint` nor
`signCommand` has appeared in the config.

### What it would require

Tauri signs through one of two config keys:

- `bundle.windows.certificateThumbprint`, with `digestAlgorithm` and
  `timestampUrl`, for a certificate already in the machine's certificate store.
  `signtool` does the work.
- `bundle.windows.signCommand`, for anything else — the shape every cloud
  signing service needs. Tauri invokes it once per artifact.

Both the application executable and each installer are signed, and the NSIS
uninstaller as well. Timestamping through an RFC 3161 authority is not optional:
without it every signature expires with the certificate.

The obstacle is not the configuration. Since June 2023 the CA/Browser Forum
baseline requirements put the private key of *every* publicly trusted
code-signing certificate on FIPS 140-2 Level 2 hardware — a rule that used to
apply only to EV certificates. There is no longer a `.pfx` that can live in a
GitHub secret. The choice is a physical USB token, which a hosted runner cannot
reach, or a cloud signing service.

### What it would cost

Indicative, and worth re-pricing before any decision is taken:

| Path | Roughly | CI-usable | Notes |
|---|---|---|---|
| Azure Trusted Signing | ~$10/month | yes | Cheapest. Organisations must show three years of verifiable history; individual validation exists. Short-lived certificates issued per signing request. |
| OV certificate + cloud HSM (SSL.com eSigner, DigiCert KeyLocker) | ~$250–600/year, some priced per signature | yes | Two subscriptions, the certificate and the signing service. |
| EV certificate | ~$400–700/year | via the vendor's cloud offering only | The only path that grants SmartScreen reputation immediately. |
| OV or open-source certificate on a USB token | ~$100–300/year | **no** | Cannot be driven from a GitHub-hosted runner. |

An OV certificate does not silence SmartScreen on day one. Reputation accrues
per publisher and per binary over downloads, so the first releases are still
interstitialled; only EV skips that.

### What not signing costs

- SmartScreen shows *Windows protected your PC*. The user clicks **More info**,
  then **Run anyway**. This is the same bargain ADR-0001 already accepted on
  macOS, where the cask's postflight exists to answer Gatekeeper.
- The MSI's elevation prompt reads *Unknown publisher*.
- Managed devices under WDAC, AppLocker, or SmartScreen configured to block
  rather than warn cannot install at all — there is no user-side override. This
  has no macOS equivalent and is the strongest argument for signing.
- Defender occasionally quarantines low-reputation binaries outright.
