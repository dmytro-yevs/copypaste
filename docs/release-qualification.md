# Release qualification

The shared release contract covers macOS, Android, Windows, and Linux.
`.github/workflows/release.yml` builds only production artifacts:

- an ad-hoc sealed macOS DMG with the bundled release daemon and Homebrew
  per-machine self-signing helper;
- universal, arm64, and armv7 Android APKs signed by the durable release keystore;
- an Authenticode-signed current-user Windows NSIS installer;
- signed Linux AppImage, Debian, and RPM packages for x86_64 and ARM64.

Linux publication requires the exact-artifact GNOME/KDE X11/native Wayland
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

Every downloadable updater artifact also receives the repository's detached
updater signature. Each platform job records the exact commit, workflow run,
filename, byte size, and SHA-256 digest. The qualification job re-hashes those
same files before publication can run.

Manual workflow runs qualify artifacts without publishing by default. Publishing
requires an existing stable `v<version>` tag at the exact workflow commit and an
explicit publish request, or a push of that tag. The publish job creates the
GitHub Release and updates the Homebrew tap only after all platform jobs pass.

Every release uses `packaging/release-template.md` for platform download tables.
Update `packaging/release-notes.md` with the release's changes. The publish job
renders the versioned links and rejects missing table artifacts before creating
the GitHub Release.

The download table groups platforms by architecture. Android ARM variants are
built with `--split-per-abi --target-platform android-arm,android-arm64`.
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
run. Recovery verifies that the source run, every native platform job, and the
qualification job passed at the exact stable tag commit. It rejects expired
artifacts and re-hashes all downloaded files against their original receipts.
Product binaries are reused without rebuilding or moving the release tag.
Publication verifies existing assets and uploads only missing files; a different
published digest fails closed. An unchanged Homebrew tap needs no new commit.

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
