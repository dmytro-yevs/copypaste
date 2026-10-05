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

Cloud Sync is not a CopyPaste 1.0.1 product capability. Local encrypted history
and paired-device synchronization remain fully supported.

CI and emulator smoke are not physical-device evidence. Before publishing
1.0.1, install the exact qualified DMG and APK on the target macOS host and a
physical Android device. Android Full capture passes only when a new background
copy from another application reaches History. Windows requires an installed
same-artifact validation on Windows.

Automated macOS release smoke refuses to start unless the default Keychain and
the complete user search list contain only a disposable test Keychain. Local
unit and Flutter tests never read or create the user's production device key.
