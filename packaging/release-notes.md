## Download CopyPaste

| Platform | Supported devices | Download |
| --- | --- | --- |
| **macOS** | Apple Silicon · macOS 14+ | **[Download for Mac (.dmg)](https://github.com/{{repository}}/releases/download/v{{version}}/CopyPaste-v{{version}}-macos-arm64.dmg)** |
| **Windows** | Windows 10 / 11 · 64-bit | **[Download for Windows (.exe)](https://github.com/{{repository}}/releases/download/v{{version}}/CopyPaste-v{{version}}-windows-x86_64-setup.exe)** |
| **Android** | Universal APK · phones and tablets | **[Download for Android (.apk)](https://github.com/{{repository}}/releases/download/v{{version}}/CopyPaste-v{{version}}-android.apk)** |

Open the downloaded installer, then follow the setup steps in CopyPaste.

## What's changed

- Guided setup for capture, permissions, privacy and device or cloud sync.
  Android offers Shizuku setup on the phone or copyable adb commands for a computer.
- Shizuku grants Android setup permissions. CopyPaste owns the background reader
  afterward; reopening the app reuses the running reader.
- One pairing link and state flow across macOS, Android and Windows. QR codes
  open immediately, followed by the protected security-code comparison.
- Fixed Android QR cleanup crashes, self-discovery and duplicate network entries.
- Consistent screen headers, compact settings rows and information panels.
  The mobile clip count sits beside the toolbar actions.
- Quick Paste previews sit beside the list. Rows keep clear action buttons,
  keyboard shortcuts and lazy loading of older history.
- Local image capture and sharper previews, with image and file copy-back.
  Source application icons are saved and synced with new captures.
- A draggable native macOS title bar and a background service hidden from the Dock.
- Stable update-check layout and bounded checks. Android saved-copy notifications
  are removed; system-bar and screen-cutout insets apply from startup.
- Fewer duplicate updates and idle background cycles. Test databases no longer
  request access to the macOS device key.

<details>
<summary>CLI, checksums and signatures</summary>

| File | Download |
| --- | --- |
| macOS CLI and daemon · Apple Silicon | [Download archive](https://github.com/{{repository}}/releases/download/v{{version}}/copypaste-cli-v{{version}}-macos-arm64.tar.gz) |
| Checksums for release files | [SHA256SUMS](https://github.com/{{repository}}/releases/download/v{{version}}/SHA256SUMS) |
| Android updater signature | [APK signature](https://github.com/{{repository}}/releases/download/v{{version}}/CopyPaste-v{{version}}-android.apk.sig) |
| Windows updater signature | [Installer signature](https://github.com/{{repository}}/releases/download/v{{version}}/CopyPaste-v{{version}}-windows-x86_64-setup.exe.sig) |

Homebrew:

```sh
brew tap dmytro-yevs/copypaste
brew install --cask copypaste
```

Install the optional CLI with `brew install copypaste-cli`.

</details>

<details>
<summary>Release verification and platform notes</summary>

The macOS DMG, Android APK and Windows installer are the exact files exercised
by the release qualification jobs before publication.

The macOS build is not Apple-notarized; the Homebrew cask removes quarantine
from the installed CopyPaste bundle and signs it locally. The Windows alpha
uses a project-generated signing certificate, so SmartScreen may display a
warning. Android uses the durable release signing key.

Install this release on every syncing device to receive source application icons.
Existing history without stored icons keeps its local icon fallback.

Android may require log-access confirmation again after its reader is stopped
by a reboot, force-stop or the operating system. Source-app exclusions remain
fail-closed when Android cannot identify the clipboard's source.

</details>
