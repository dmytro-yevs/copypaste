# macOS Developer ID signing and notarization

The release workflow (`.github/workflows/release.yml`, `build-macos` job) supports
**Developer ID Application** code signing and Apple notarization when the required
secrets are set. When secrets are absent (forks, unsigned CI, local builds) the
workflow falls through to the existing **ad-hoc** signing path and never fails —
unsigned builds remain fully functional for development and internal testing.

## Why this matters (1.0 blocker)

Distributing a `.app` or `.dmg` without a Developer ID certificate and notarization
means:
- Gatekeeper blocks first launch on macOS 12+ (Monterey and later) with
  "Apple could not verify … is free of malware".
- Homebrew Cask installs show a security warning and require `xattr -cr` after copy.
- Users without technical knowledge cannot open the app at all.

Developer ID + notarization removes all of these blockers.

---

## Required GitHub repository secrets

Set all five in **Settings → Secrets and variables → Actions**:

| Secret | Value |
|--------|-------|
| `MACOS_CERTIFICATE_BASE64` | base64-encoded `.p12` Developer ID Application certificate (see below) |
| `MACOS_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` from Keychain Access |
| `MACOS_NOTARIZE_APPLE_ID` | Apple ID (email) enrolled in the Apple Developer Program |
| `MACOS_NOTARIZE_TEAM_ID` | 10-character Apple Developer Team ID (visible at developer.apple.com → Membership) |
| `MACOS_NOTARIZE_APP_PASSWORD` | App-specific password for the Apple ID (not the account password) |

If any of the five secrets is absent, the CI job skips signing/notarization and
emits a `::notice::` annotation — the build still succeeds with ad-hoc signing.

---

## Obtaining the certificate

1. Log in to [developer.apple.com](https://developer.apple.com) and go to
   **Certificates, IDs & Profiles → Certificates**.
2. Click **+** and select **Developer ID Application** (for direct distribution
   outside the App Store).
3. Follow the Certificate Signing Request (CSR) flow and download the `.cer` file.
4. Double-click the `.cer` to install it in Keychain Access.
5. In **Keychain Access → My Certificates**, find the newly installed
   "Developer ID Application: <Your Name> (TEAM_ID)" entry.
6. Right-click → **Export** → choose `.p12` format → set a strong export password.
7. Base64-encode for the GitHub secret:

   ```bash
   # macOS
   base64 -i developer_id.p12 | pbcopy

   # Linux
   base64 -w0 developer_id.p12 | xclip -selection clipboard
   ```

8. Paste the base64 output as `MACOS_CERTIFICATE_BASE64`.
   Store the export password as `MACOS_CERTIFICATE_PASSWORD`.

---

## Generating an app-specific password

1. Sign in at [appleid.apple.com](https://appleid.apple.com).
2. Under **Security**, click **Generate Password** in the App-Specific Passwords
   section.
3. Label it "CopyPaste notarytool CI" (or similar).
4. Copy the generated password (`xxxx-xxxx-xxxx-xxxx` format) and store as
   `MACOS_NOTARIZE_APP_PASSWORD`.

**Never** use your real Apple ID password here — app-specific passwords are
revocable and scoped to a single purpose.

---

## CI signing flow (what the workflow does)

When all five secrets are present the `build-macos` job:

1. **Imports the certificate** into a temporary per-run keychain
   (password-protected, discarded after the job).
2. **Re-signs all inner binaries** (`copypaste-daemon`, `copypaste`, `copypaste-relay`)
   with `codesign --options runtime --timestamp` using the Developer ID identity.
3. **Signs the `.app` bundle** deep and strict (`--deep --options runtime
   --entitlements scripts/macos/entitlements.plist --timestamp`).
4. **Rebuilds the DMG** with the signed `.app` and replaces the ad-hoc DMG produced
   by `build-dmg-ci.sh`.
5. **Submits to Apple notary** via `xcrun notarytool submit --wait`. On failure,
   the notarization log is printed and the job fails.
6. **Staples the ticket** to the DMG (`xcrun stapler staple`) so Gatekeeper
   accepts the app offline (Homebrew Cask install, air-gapped machines, etc.).
7. **Deletes the temporary keychain** in the `always()` cleanup step so the
   certificate never persists on the GitHub-hosted runner.

---

## Local testing

To test signing locally (requires Xcode Command Line Tools + your cert in Keychain):

```bash
# Sign with your Developer ID (replace with your actual identity):
IDENTITY="Developer ID Application: Your Name (TEAM_ID)"

codesign --force --deep \
  --sign "$IDENTITY" \
  --options runtime \
  --entitlements scripts/macos/entitlements.plist \
  --timestamp \
  dist/CopyPaste.app

codesign --verify --deep --strict --verbose=2 dist/CopyPaste.app
spctl --assess --type exec --verbose=4 dist/CopyPaste.app
```

To verify a stapled DMG after notarization:

```bash
xcrun stapler validate dist/CopyPaste-v*.dmg
spctl --assess --type open --context context:primary-signature dist/CopyPaste-v*.dmg
```

---

## Entitlements

The entitlements file at `scripts/macos/entitlements.plist` must be present and
correct before signing. Check it includes at minimum:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Hardened Runtime: allow JIT, allow unsigned executable memory if needed -->
    <key>com.apple.security.cs.allow-jit</key>
    <false/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <false/>
    <!-- Required for clipboard access via NSPasteboard -->
    <key>com.apple.security.temporary-exception.mach-lookup.global-name</key>
    <array>
        <string>com.apple.pasteboard</string>
    </array>
</dict>
</plist>
```

Adjust entitlements to match actual daemon capabilities (network, keychain, etc.).
See Apple's [Hardened Runtime documentation](https://developer.apple.com/documentation/security/hardened_runtime)
for the full list.

---

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `No 'Developer ID Application' identity found` | Wrong cert type or import failed | Check secret is a `.p12`, password is correct, cert is "Developer ID Application" (not "Mac Developer") |
| `The operation couldn't be completed` on notarytool | Wrong Apple ID, Team ID, or app-specific password | Verify all three notarize secrets; regenerate the app-specific password |
| `Notarization failed: The software is not signed` | Signing step didn't run or used wrong identity | Check `steps.cert.outputs.signed` is `true`; inspect the codesign step logs |
| `stapler validate` fails | Notarization ticket not yet propagated | Wait a few seconds and retry; Apple CDN has brief propagation delay |
| Gatekeeper still blocks after install | `com.apple.quarantine` xattr present | The "Strip quarantine xattr" step handles this; check it ran |
