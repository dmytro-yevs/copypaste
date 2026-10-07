# Release qualification

CopyPaste releases one stable version across macOS, Android, and Windows.
`.github/workflows/release.yml` builds only production artifacts:

- an ad-hoc sealed macOS DMG with the bundled release daemon and Homebrew
  per-machine self-signing helper;
- a universal Android APK signed by the durable release keystore;
- an Authenticode-signed current-user Windows NSIS installer.

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
