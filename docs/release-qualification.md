# Release qualification

The shared release contract covers macOS, Android, Windows, and Linux.
`.github/workflows/release.yml` builds only production artifacts:

- an ad-hoc sealed macOS DMG with the bundled release daemon and Homebrew
  per-machine self-signing helper;
- universal, arm64, and armv7 Android APKs signed by the durable release keystore;
- an Authenticode-signed current-user Windows NSIS installer;
- signed Linux AppImage and Debian packages for x86_64.
- a signed experimental x86_64 RPM, with exact-byte Fedora 44 installation
  evidence; this package does not claim qualified desktop parity on RPM hosts.

Linux publication requires the exact-artifact GNOME 46 X11 on Ubuntu 24.04
matrix in [Linux native qualification](../packaging/linux/native-qualification.md).
Package builds and protocol fixtures alone do not satisfy that contract.

Each Linux matrix row also stages the signed OCR, Semantic Search, and Supabase
packages for its exact architecture, their package-native qualification
receipts, and offline fixtures. The installed GUI daemon must receive the typed
IPC install, enable, invoke, disable, and remove operations for every package,
then restart before the post-removal inventory check. The acceptance helper
verifies each package with `MODULE_RELEASE_PUBLIC_KEY`, binds its receipt to
the staged bytes, target, and app version, checks OCR fixture hashes, and
checks semantic model hashes against the model manifest inside the signed
package. Its typed IPC command
trace and `linux-module-qualification.json` are release evidence; an inventory
listing or an independently produced module receipt cannot substitute for this
installed-product lifecycle.

Before changing the application release version, advance the three first-party
module versions and set their source `app_versions` lower bound to that exact
application version. Release safety tests check this binding before native
builds. During an application release, module producers can publish qualified
packages with `publish=true` and `publish_marketplace=false`; update the signed
marketplace through `module-marketplace.yml` after the application is public.
Normal standalone module publication still updates the marketplace by default.

Every downloadable updater artifact also receives the repository's detached
updater signature. Each platform job records the exact commit, workflow run,
filename, byte size, and SHA-256 digest. The qualification job re-hashes those
same files before publication can run.

Manual workflow runs qualify artifacts without publishing by default. Publishing
requires an existing stable `v<version>` tag at the exact workflow commit and an
explicit publish request, or a push of that tag. The publish job creates the
GitHub Release and updates the Homebrew tap only after all platform jobs pass.

Fresh releases automatically call the complete module and compositor producers,
then run installed Linux qualification against the parent run's exact artifacts.
The complete module matrices produce the publishable packages; installed Linux
qualification exercises those same Linux package bytes. Publish their qualified
packages from this parent run instead of rebuilding a separate module release.
Manual evidence run IDs are recovery overrides. Preflight resolves the preceding
published Linux upgrade packages and checks their public digests before builds.
See [delivery architecture](engineering/ci-architecture.md) for the dependency
graph, permission boundaries, artifact inventories and recovery modes.

Every release uses `packaging/release-template.md` for platform download tables.
Update `packaging/release-notes.md` with the release's changes. The publish job
renders the versioned links and rejects missing table artifacts before creating
the GitHub Release.

The download table groups platforms by architecture. Android ARM variants are
built together with the universal APK using one `--split-per-abi` invocation
targeting `android-arm,android-arm64,android-x64`. AGP emits the universal output
and excludes unsupported transitive x86 JNI libraries.
The universal APK also includes x86_64 and keeps the existing `android.apk`
filename for older updaters. All three APKs share the same version code;
`force-version-code-ignoring-abi=true` disables Flutter's per-ABI offsets.
Each APK must contain exactly its expected ABIs and complete Flutter, Dart,
Rust bridge, and pairing libraries. Android qualification receipts cover all
three APKs. The updater selects the running process's ARM variant when its
package and signature metadata are available, otherwise the signed universal
APK. Recovery still accepts single-artifact receipts from earlier releases.

To finish publication after a publisher interruption, dispatch the production
workflow with `publish=true` and `qualified_run_id` set to a successful production
run, including a run whose only failure was publication. Recovery verifies that
every native platform job and the qualification job passed at the exact stable
tag commit. It rejects expired
artifacts and re-hashes all downloaded files against their original receipts.
Product binaries are reused without rebuilding or moving the release tag.
Publication verifies existing assets and uploads only missing files; a different
published digest fails closed. New releases remain drafts until the complete
asset set passes verification. An unchanged Homebrew tap needs no new commit.

Cloud Sync is not a CopyPaste 1.0.2 product capability. Local encrypted history
and paired-device synchronization remain fully supported.

CI and emulator smoke are not physical-device evidence. Before publishing
1.0.2, install the exact qualified DMG and APK on the target macOS host and a
physical Android device. Android Full capture passes only when a new background
copy from another application reaches History. Windows requires an installed
same-artifact validation on Windows.

Automated macOS release smoke refuses to start unless the default Keychain and
the complete user search list contain only a disposable test Keychain. Local
unit and Flutter tests never read or create the user's production device key.

Android in-app updates stage the verified APK in a `PackageInstaller.Session`,
require system user confirmation, and receive the terminal installation status.
The update path does not share APKs through a `FileProvider`. The explicit,
non-exported result receiver retains session state across process recreation;
confirmation is opened only while CopyPaste is in the foreground.

Qualify this flow on a physical device with two APKs signed by the same release
key and increasing version codes. Exercise unknown-source permission, successful
self-update, cancellation, and returning to CopyPaste after backgrounding it
while confirmation is pending. JVM session tests and APK builds do not establish
that physical-device acceptance.

ARM64, KDE, Wayland, and RPM desktop parity are excluded from stable Linux qualification.
Their implementation and opt-in checks remain in `linux-experimental.yml`.
Optional Linux module publication follows the same x86_64 release scope.
