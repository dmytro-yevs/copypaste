# ADR-0034: Optional first-party modules

## Contract

CopyPaste owns the shared interface, module registry, package installation,
preferences, and command dispatch. Modules provide separately installed native
code and assets. They are not dependencies of the application build. OCR is an
optional module; the base application includes no OCR engine or models.

The first implementation accepts only packages signed by CopyPaste's pinned
release identity. Modules execute locally on macOS, Android, and Windows. A
package is specific to one OS and architecture; module behavior and manifest
contributions normally have parity across all three platforms. Schema 2 can
declare `supported_platforms` for explicitly platform-specific capabilities.
SMS Codes is an Android-only module; it never runs on macOS or Windows.

Raycast's [extension architecture](https://www.raycast.com/blog/how-raycast-api-extensions-work),
[manifest](https://developers.raycast.com/information/manifest), and
[lifecycle](https://developers.raycast.com/information/lifecycle) inform the
separation between commands, host-rendered interface, and runtime ownership.
CopyPaste retains Flutter, shadcn_flutter, and its Rust runtime. It does not
introduce a JavaScript runtime or compatibility with Raycast extensions.

## Ownership

- `copypaste-module-sdk`: versioned manifest, field, invocation, result, and C
  ABI contracts; the authoring trait and export helper.
- `copypaste-modules`: authenticated package extraction, registry, lifecycle,
  and native command execution. `ModuleHost` composes this owner lazily and
  runs blocking work outside reactor/UI threads.
- Desktop `AppState` and Android `Runtime`: compose the same `ModuleHost` below
  their existing transport boundary.
- Flutter `ModulesController` and repository: shared management state and typed
  adapters. Settings renders module descriptions, commands, and forms through
  existing components and theme tokens.

## Packages and compatibility

A `.cpmodule` is a ZIP containing `manifest.json`, `manifest.json.sig`, and
exactly the regular files inventoried by the signed manifest. Each file has a
SHA-256 digest and an expanded size. Symlinks, duplicate/escaping paths,
unlisted entries, excessive size, unsupported API versions, incompatible app
versions, and a different OS/architecture are rejected before activation.

The release signer produces a Minisign signature of the manifest. The
manifest authenticates every code/asset file, including the entrypoint.
Installed metadata is authenticated on reads; code and assets are reverified
before loading after startup or re-enabling. Large models are streamed during
verification and are not hashed again on every command or settings refresh.

Schema version and native ABI version are separate. The exported symbol is
`copypaste_module_v1`; only C-compatible buffers and opaque instance pointers
cross the native boundary. Each allocator releases its own buffers. No Rust
trait object, `String`, or `Vec` crosses libraries. App compatibility uses a
SemVer requirement in `app_versions`.

## Lifecycle and persistence

Fresh CopyPaste installs contain the lightweight host only. Native modules
load on first command and release their instances when disabled, updated,
removed, or evicted. A module must finish its owned workers before destruction.
The manifest's `unload_policy` defaults to `instance`. A `process` policy pins
native code until OS process exit for runtimes with process-global environments
or callbacks, including ONNX Runtime. Sessions and model instances still drop.
Removing a loaded process-scoped module disables it and clears its data and
preferences immediately; package deletion finishes after restart. Shared
Settings exposes `Restart CopyPaste`: desktop restarts the owned daemon and
Android restarts the application process through ProcessPhoenix. A module that
was never loaded can be removed immediately without restart.

One manager serializes lifecycle mutations. Each module has its own execution
lock, so long commands do not block commands in other modules. Disable, update,
and removal stop new admission and wait for existing work before deleting code.
The host retains at most four idle instances, evicting the least recently used.
Active invocation leases are never unloaded by cache eviction. Commands and
preferences are validated against the manifest. Packages live beneath
`<application data>/modules/packages/<id>/<version>`; module-owned data has a
separate `<application data>/modules/data/<id>` directory.

Installing a newer signed package updates the active registry atomically and
retains valid preferences and the enabled state. Failed validation leaves the
previous version active. Equal versions and downgrades are rejected. Removal
first persists a disabled removal state, then releases code and removes all
versions and module-owned data. An interrupted removal remains visible and
can be retried. Corrupt registry data fails closed and is never reset silently.

## Deliberate limits of this stage

Settings > Modules opens the first-party marketplace. Marketplace and Installed
use the shared shadcn components on all supported platforms. Search filters
module titles and descriptions. Install and Update download the exact package
for the running process ABI and installed application version, with progress,
bounded streaming, SHA-256 verification, and private staging cleanup. The
runtime then verifies the signed manifest and file inventory before activation.
Installed management remains available when the marketplace is offline.

The catalog lives in the `modules` GitHub Release as `modules.json` and its
base64-encoded Minisign `modules.json.sig`, using the pinned release key. Catalog
metadata is authenticated before rendering or resolving a package. Each entry
contains ID, title, description, stable version, `app_versions`, and platform /
architecture artifacts with a versioned release URL, compressed size, and
SHA-256 and optional minimum system versions. All published modules appear;
incompatible app versions, system versions, or architectures are explained on
their ordinary cards with installation disabled. Unknown native version facts
also fail closed on the card. An unpublished or empty catalog uses the shared
empty state; only authentication, invalid metadata, and network failures use
the error state. Module execution remains offline;
only marketplace discovery and installation require network access.

Publish qualified packages under `module-<id>-v<version>` in the first-party
repository. The **Publish module marketplace** workflow authenticates every
package and the previous catalog, enforces matching manifests and packages for
macOS aarch64, Windows x86_64, and Android arm/aarch64/x86_64, preserves other
catalog entries, rejects equal versions and downgrades, and signs the new
catalog with the existing release signer secrets. Publication is a separately
dispatched operation after native qualification; preparing this workflow does
not publish packages or establish native acceptance evidence. Local catalog
generation uses `scripts/modules/catalog.py` and Python 3.9+ / OpenSSL 3+.
Minimum system versions come from authenticated
`assets/module-distribution.json` inside each package. SDK crate releases are
independent of application releases, so updating CopyPaste does not invalidate
the separately maintained OCR dependency lock.

The **Build and publish OCR module** workflow builds every shipped target from
checksum-pinned models and ONNX Runtime 1.28.0. It signs the packages with the
production identity, then qualifies the exact bytes on native macOS and Windows
hosts and inside app-private storage on an Android x86_64 emulator with no
Internet permission. Desktop execution blocks outbound network access. The
scenarios cover English, separate Ukrainian/English lines, a mixed line,
disable/enable, and removal completed after process restart. Receipts bind the
commit, run, target, package size and SHA-256. Android arm and aarch64 builds
add ELF/16 KiB alignment checks; their compilation is not physical-device
execution evidence. Publication requires the three platform receipts and all
five authenticated packages, then dispatches the signed catalog update.

There is no third-party trust UI.
Native first-party code runs inside the owning runtime process: this is not a
sandbox, and manifest declarations cannot restrict native OS access. A native
crash can terminate that process. Third-party execution requires an explicit
isolation design before it can be enabled.

Schema 1 packages remain supported. Schema 2 adds signed supported platforms
and event handlers bound to existing commands. `sms_received` is Android-only
and supplies a bounded invocation-only `text` argument. Event commands are not
rendered as manual command buttons. New event modules install disabled; the
shared Installed card exposes native SMS access setup and enable/disable.
Only enabled, authenticated modules receive events. Lifecycle mutations wait
through recognition and code publication, so disabling or removing a module
prevents late output. The SMS host reuses runtime admission, encrypted ingest,
History events, clipboard writing, and configured sync. Message bodies are
never persisted by the module or host. Android uses inbox observation plus SMS
and boot reception; Shizuku grants access only during setup. See
[`modules/sms-codes/README.md`](../../modules/sms-codes/README.md).

The initial host-rendered primitives are text/boolean/file forms and text/message
results. File arguments use native pickers; the host keeps an invocation-owned,
bounded private snapshot until native execution ends. Internal paths are not
shown as form fields. Background schedules, additional capture event handlers, content-processing
contracts, dependency resolution, rich result views, and secure secret
preferences require concrete module use cases and versioned additions; they
are not placeholder implementations in this foundation.

## Authoring and validation

`examples/modules/text-tools` is built separately and exports the SDK entrypoint.
It is never installed or bundled in CopyPaste by default. The native lifecycle
test builds the dynamic library, signs test packages with an ephemeral key,
loads the real entrypoint, executes Unicode text, restarts, updates, disables,
and removes the module. The test key is not trusted by production hosts.

Build a module with `cargo build -p copypaste-module-text-tools`. Package it with
`scripts/modules/package.py --module-dir examples/modules/text-tools --library
<built library> --platform <platform> --architecture <architecture> --output
<package.cpmodule>`. Signing uses the existing release signer environment;
private keys must never be committed. Install or update the signed package
through the marketplace after publishing the qualified target packages.

Native lifecycle tests establish behavior on their executing host. Android
cross-compilation and Flutter tests establish source/build compatibility, not
physical Android or installed Windows acceptance evidence.
