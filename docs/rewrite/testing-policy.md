# Testing policy

Every requirement has exactly one **authoritative** layer. Lower layers may
support a requirement; they never replace it.

A requirement whose authoritative layer does not run is recorded as
`NOT VERIFIED IN CI` — that exact string, in the matrix below and wherever the
claim is made. Never report it as passed, as skipped successfully, or as
implicitly covered by a layer that cannot see it. If a layer cannot reach an
assertion, move the requirement to the layer that can or mark it; do not weaken
the assertion.

## The layers

| Layer | What runs it | May claim | May never claim |
|---|---|---|---|
| Rust | `ci.yml` → `test + clippy (linux)`, `demo.sh` + `demo-p2p.sh` | portable logic against fakes: crypto, storage, detection, merge, IPC framing, CLI | anything a platform API answers |
| Browser (jsdom) | `ci.yml` → `npm build + test (jsdom)` | component logic, hooks, reducers, catalogue coverage | layout, scrolling, virtualisation, anything crossing the Tauri bridge |
| Browser (WebKitGTK, Linux) | `browser-webkitgtk.yml` | shared React behaviour in a real engine: rendering, composition, navigation, responsive layout, forms, dialogs, menus, loading/empty/error states, keyboard navigation, focus, the accessibility tree, overflow and scrolling, console errors, behaviour against a real daemon | Tauri commands as shipped, WKWebView, Android WebView, NSPasteboard, Keychain, Keystore, intents, tray or menu bar, global shortcuts, launch at login, native notifications, native window focus or dismissal, platform permissions, packaging, signing, installation |
| Android | `android-emulator.yml` on x86_64, including `e2e-android/` over CDP; `release.yml` on a physical arm64 device | Android platform behavior at each API level that ran; the debug WebView document; the signed release APK and arm64 path at release | macOS; OEM background policy; a Play-updated WebView |
| macOS | `ci.yml` → `macOS check + platform (arm64)`; `release.yml` → `smoke-macos-dmg.sh` on a tag | macOS | Android |
| Windows | `windows-native-e2e.yml` over WebView2; `ci.yml` workspace and installed-product evidence; user-requested `native-nightly.yml` contracts | the shipped WebView and Tauri bridge, native clipboard capture, named-pipe IPC, DPAPI, app launch, shell-setting registration | tray interaction, OS hotkey delivery, autostart after sign-in, notifications, signing |

The browser layer does execute Tauri commands through `wry` on Linux. That is
useful and it is not verification: the shipped bridges are WKWebView and the
Android WebView, and each command's behaviour belongs to its platform's layer.

The CLI is a test surface, not a product surface (CLAUDE.md rule 6). A CLI
assertion never satisfies a UI requirement.

`e2e-android/` attaches to the debug APK's WebView over the Chrome DevTools
Protocol. It can assert the document, layout and keyboard input, but not pixels,
TalkBack or native Android surfaces. The release smoke scans the shipped APK's
native libraries and fails if wry's debugger-enabling call is present. A
userdebug emulator may still expose CDP independently of the APK.

## What triggers what

A change to shared React, the Rust core, `copypaste-ipc` or the Tauri bridge
can break either platform, so it must run every platform layer. `ci.yml`
(Linux + macOS) and `browser-webkitgtk.yml` run on every push and pull request
with no path filter. `android-emulator.yml` runs nightly, on demand, and on any
push or pull request touching those shared paths or the Android tree.

`windows-native-e2e.yml` runs on every push and pull request. It drives the
debug Windows app through `tauri-driver` and the hosted runner's matching Edge
WebDriver; the whole shared suite runs against a real named-pipe daemon, and a
Windows-only file exercises native clipboard and shell-setting state.

`native-nightly.yml` runs Android API 34/36 and macOS 14/15 matrices each night.
Its optional Windows evidence job runs only when a workflow dispatch explicitly
requests it; the receipt covers native contracts, not an unbuilt Windows UI.
`release.yml` runs on a tag. Everything it alone proves — installing the DMG,
launching the bundle, the cask — is therefore post-hoc, and is marked below.

Publication also waits for the native-parity gate. It verifies checksummed,
same-commit receipts from the macOS smoke and the physical Android smoke, and
fails when either platform or its measured evidence is absent. The macOS
receipt producer has not been validated from this Windows recovery environment;
until a macOS run succeeds, WKWebView remains **NOT VERIFIED IN CI** as recorded
below.

## What an emulator is not

The emulator layer is a Google APIs system image on `ubuntu-24.04`, x86_64,
with no vendor software on it. Three requirements sit outside that at every
API level.
A green Android job says nothing about any of them, and each has its row in the
matrix.

- **The aarch64 hardware SHA-2 backend.** Every Android device we ship to is
  arm64; the emulator is x86_64 and takes the x86 path, so those instructions
  never execute in any run, here or in CI. Static disassembly of the aarch64
  artifact is the only check available without arm64 hardware, and it is what
  caught the `asm` feature being off once already — count `SHA256H`,
  `SHA256H2`, `SHA256SU0`, `SHA256SU1` in the built object. **Count on this
  tree: 56** in the `aarch64-linux-android` `sha2-asm` archive — 16, 16, 12 and
  12 in that order — against 0 for the x86_64 control. Taken with `cargo build`
  and `llvm-objdump`: `cargo check` emits no object code and counts 0 for both.
  It establishes that the instructions ship, not that they execute; on x86_64
  they cannot.
- **OEM background restrictions.** Stock AOSP applies none of the vendor battery
  management that decides whether the capture service is still alive an hour
  after the user last opened the app. That is where a clipboard manager dies in
  the field, and no runner reproduces it.
- **The Play-updated System WebView.** The emulator's WebView is the one pinned
  into its system image. On a user's phone it updates from Play independently of
  the OS, so any behaviour that turns on the WebView version — including which
  one the shipped app gets — is unobserved.

### API levels

Clipboard access is API-gated — background reads at 10 (29), the notification
runtime permission at 13 (33), foreground service types at 14 (34) — so one API
level is one point on that curve
([android-clipboard-access.md](android-clipboard-access.md)). The scheduled
matrix runs API 24, 29, 33, 34 and 36; dispatches select one level. The rungs
harness runs only on API 36 because its shell clipboard transaction uses that
level's argument vector.

## Matrix

`Verified` — the authoritative layer runs it on every push or pull request.
`Partial` — it runs, but only on a tag or a nightly, or it asserts less than
the requirement. `NOT VERIFIED IN CI` — no run anywhere establishes it.

### Capture

| Requirement | Authoritative layer | State |
|---|---|---|
| `changeCount`, burst loss, self-write suppression, `org.nspasteboard.*`, size cap | macOS | Verified — `--ignored` tests on `macos-14`; an empty run fails the job |
| A real `pbcopy` reaches history through the shipped bundle | macOS | Partial — ENFORCED in `smoke-macos-dmg.sh`, tag-only |
| A real Windows clipboard change reaches history, and private mode blocks it | Windows | Verified — WebView2 changes private mode and PowerShell writes the session clipboard |
| Capture pipeline against the fake source | Rust | Verified |
| `ACTION_SEND` / `ACTION_PROCESS_TEXT` reach SQLCipher | Android | Partial — debug leg only; the release leg has no `run-as` and prints `NOT ASSERTED`. `e2e-android/` follows an `ACTION_SEND` onto the screen |
| Rung 2: the shell-uid clipboard read | Android | Partial — the API 36 leg reads a foreign clip as uid 2000 without focus; Shizuku's binder proxy and listener still need pairing on a phone |
| Kotlin → Rust capture bridge shape | Rust, Android | Verified — the debug APK build runs the Kotlin serializer/fixture guard, and the Rust workspace test strictly consumes that fixture |
| `ClipListener` / `ClipQueue` | Android | **NOT VERIFIED IN CI** — the listener never registers without Shizuku; no Kotlin unit test exists |
| `CaptureService` | Android | Partial — the API 36 leg proves it stays stopped and makes no claim when no listener exists; nothing asserts positive capture through it |
| Quick Settings tile | Android | Partial — the API 36 leg requires one tile click to persist a foreign clip in encrypted history |
| Android 12+ clipboard-toast consent gate | Android | **NOT VERIFIED IN CI** — the Rust refusal and the jsdom dialog support it; the OS toast is unobserved |
| Capture surviving OEM battery policy | Android | **NOT VERIFIED IN CI** — stock AOSP applies no vendor background restrictions; this needs a phone left idle |
| API-level spread | Android | Partial — the scheduled matrix runs API 24, 29, 33, 34 and 36; targeted dispatches run one selected level |

### Crypto and device secret

| Requirement | Authoritative layer | State |
|---|---|---|
| XChaCha20-Poly1305 + HKDF, item id as AAD, fail-closed, zeroized | Rust | Verified |
| Device secret in the macOS Keychain | macOS | Verified — throwaway keychain; a self-skipped test fails the job |
| Device secret in the Android Keystore | Android | Verified — a second launch reopens the same database |
| Device secret protected by DPAPI | Windows | Verified — Windows runs the keystore suite and refuses an unusable persisted blob over the named pipe |
| The Keychain item survives a re-signed binary (manifest 02 §3.8) | macOS | **NOT VERIFIED IN CI** — REPORTED leg, tag-only |
| The aarch64 hardware SHA-2 path executes as built | Android | Partial — the signed universal APK must pass on a physical `arm64-v8a` device before a release is published |

### Storage and detection

| Requirement | Authoritative layer | State |
|---|---|---|
| SQLCipher at rest, no plaintext in the file | Rust | Verified — and confirmed against a pulled database on Android |
| Schema, dedup interval, tombstones, pins, FTS5, keyset pagination | Rust | Verified |
| A sensitive item never reaches the index (write, read, purge) | Rust | Verified |
| Retention: history cap, TTL, sensitive auto-wipe | Rust | Verified |
| Export, import, backup, restore over IPC | Rust | Verified |
| The same four from the Settings screen | Windows, Browser (WebKitGTK) | **NOT VERIFIED IN CI** — the Windows run asserts export and restore safety dialogs, but native file pickers prevent completing these flows through WebDriver |
| Secret-detection ruleset, NFKC, Luhn, confidence bands | Rust | Verified |
| A v0.4 database encountered by v2 is explained, never opened | Rust | **NOT VERIFIED IN CI** — the distinct filename prevents ordinary discovery, and no current test exercises an explicit encounter |

### IPC and daemon

| Requirement | Authoritative layer | State |
|---|---|---|
| `0600` socket, `LinesCodec`, timeouts, connection and watcher caps | Rust | Verified |
| No filesystem path in any user-facing string (INV-12) | Rust, and a sweep at the browser layer | Verified |
| `GetConfig` / `SetConfig` with per-field liveness | Rust | Verified |
| The Settings screen driving them | Windows | Verified — the WebView changes a value and the independent CLI reads it back through the named pipe |
| Service start, restart and shutdown from the app | Windows, macOS | Verified on Windows; macOS remains **NOT VERIFIED IN CI** because launchd and Homebrew are unexercised |
| Push (`Method::Watch`) and degrade-to-polling | Rust, Windows, Browser (WebKitGTK) | Verified through the shipped Windows bridge and on Linux; macOS remains **NOT VERIFIED IN CI** |

### Sync

| Requirement | Authoritative layer | State |
|---|---|---|
| Pairing: code mint, TTL, wrong code, unpair, revoke, Noise `NNpsk0` | Rust, Windows, Browser (WebKitGTK) | Verified — both desktop WebDriver runs pair against a second real daemon |
| Merge: LWW, delete-wins, total tie-break, skew refusal | Rust | Verified |
| The QR payload never enters the DOM (INV-13) | Browser (WebKitGTK) | Verified |
| Camera QR scanning | macOS, Android | **NOT VERIFIED IN CI** — only the no-camera fallback runs |
| mDNS discovery at runtime | macOS, Android | **NOT VERIFIED IN CI** — no multicast on any runner; every test passes `--addr` |
| Cloud sync against a real Supabase project | macOS, Android | **NOT VERIFIED IN CI** — the Rust suite runs against fakes and `demo-cloud.sh` drives a local stub from no workflow |

### UI

| Requirement | Authoritative layer | State |
|---|---|---|
| History render, virtualisation, row-height reservation (INV-5) | Browser (WebKitGTK) | Verified |
| Scroll anchoring and shrink clamp (INV-1/6) | Browser (WebKitGTK) | Verified |
| Search, filter, sort, bulk actions | Browser (WebKitGTK) | Verified |
| A sensitive item is absent from the document, not obscured | Browser (WebKitGTK) | Verified |
| Settings tabs, preferences surviving a reload (INV-22) | Browser (WebKitGTK) | Verified |
| Devices and pairing screens, the service-offline screen | Browser (WebKitGTK) | Verified |
| Keyboard navigation, focus, accessibility tree (the 15 A11Y rules) | Browser (WebKitGTK) | Verified as DOM and ARIA |
| i18n: no catalogue key reaches the screen | Rust, Browser (WebKitGTK) | Verified |
| Design tokens and contrast | `ci.yml` → `design tokens` | Verified |
| The app renders on WKWebView | macOS | **NOT VERIFIED IN CI** — the tag-only smoke now requires native accessibility, screenshot and latency evidence, but no recovered run establishes it yet |
| The app renders and lays out in WebView2 | Windows | Verified — the Windows suite requires the Tauri bridge, a populated React root and non-zero layout boxes |
| The app renders on the Android WebView | Android | Verified — both APK legs fail unless the screen is awake and uiautomator reports named WebView content |
| The frontend mounts and lays out in the Android WebView | Android | Verified — `e2e-android/` requires a populated React root and non-zero history-row boxes |
| Navigation and keyboard input on Android | Android | Verified — the CDP harness taps between screens and types a search that must filter the list |
| A sensitive item is absent from the Android document | Android | Verified — the harness checks `outerHTML` and every live input value |
| No filesystem path in any Android accessible string (INV-12) | Android | Verified — the harness sweeps text and naming attributes |
| Android UI beyond mount, navigation, typing and the two disclosure sweeps | Android | **NOT VERIFIED IN CI** — CDP sees no pixels, TalkBack or native surface |
| The release APK cannot enable WebView debugging | Android | Verified — release smoke rejects wry's debugger call and fails closed if its neighbouring JNI markers are absent |
| Android native accessibility surface | Android | Partial — the API 33 and API 36 release jobs fail when the native tree cannot be observed, exposes fewer than three named nodes, has no actions, or exposes unnamed actions |
| VoiceOver and TalkBack surfaces | macOS, Android | Partial — the macOS release job fails when the native accessibility surface cannot be observed, has no menu bar, or exposes no named elements; gestures and speech output remain **NOT VERIFIED IN CI** |

### Native shell

| Requirement | Authoritative layer | State |
|---|---|---|
| Menu-bar item, menu, recent-items submenu | macOS | **NOT VERIFIED IN CI** — the tag-only smoke infers it from the daemon answering |
| Tray icon, menu and recent-items submenu | Windows | **NOT VERIFIED IN CI** — Edge WebDriver reaches the WebView, not the native notification area |
| Popover placement under the tray icon | macOS | **NOT VERIFIED IN CI** — geometry is unit-tested against synthetic monitors |
| Window dismissal on blur | macOS | **NOT VERIFIED IN CI** |
| Main-window launch and WebView response | Windows | Verified — every E2E file opens a native app session and rejects a non-Tauri document |
| Close-to-tray and Quick Paste window lifecycle | Windows | **NOT VERIFIED IN CI** — WebDriver cannot observe a hidden native window after its session closes |
| Global hotkey, including whether TCC accepts the certificate (B-31) | macOS | **NOT VERIFIED IN CI** — nothing registers a shortcut anywhere |
| Launch at login | macOS | **NOT VERIFIED IN CI** |
| Global shortcut registration | Windows | Partial — Settings replaces and restores the native registration; OS-level delivery is **NOT VERIFIED IN CI** |
| Launch-at-login registration | Windows | Partial — Settings changes and reads back the Windows registration; execution after sign-in is **NOT VERIFIED IN CI** |
| Notification on copy | macOS | **NOT VERIFIED IN CI** — nothing posts or asserts one |
| Sound on copy | macOS | **NOT VERIFIED IN CI** — the `should_play` gate is Rust-verified; the spawn is not |
| Screen-capture protection, INV-35 (`contentProtected`, `FLAG_SECURE`) | macOS, Android | Partial — the Android API 36 leg finds `FLAG_SECURE` in twenty window dumps, with another window read as unprotected by the same reader. macOS `contentProtected` remains unasserted |

### Packaging

| Requirement | Authoritative layer | State |
|---|---|---|
| macOS build, codesign, DMG, no linkage outside the bundle | macOS | Partial — ENFORCED, tag-only |
| Cask postflight, quarantine removal, re-seal | macOS | Partial — ENFORCED, tag-only; Gatekeeper's verdict is a note |
| `brew install --cask` as a user runs it | macOS | **NOT VERIFIED IN CI** — `check.sh` round-trips the generators only |
| Published universal APK: build, R8, release signing, install, launch, no stripped symbol | Android | Partial — the exact signed artifact is checksum-verified and smoke-tested on API 33 and API 36 before publication, tag-only |
| The signed release APK installed on a physical device | Android | Partial — publication depends on a tag-only hardware gate that installs and smoke-tests the exact artifact on one physical `arm64-v8a` device |
| Windows current-user install, sidecars, launch and uninstall | Windows | Verified — `ci.yml` installs the NSIS output into a throwaway directory and requires complete cleanup; signing is unverified |
| Notarisation and Gatekeeper acceptance | macOS | **NOT VERIFIED IN CI** — ADR-0001 decided against notarisation; recorded so it is not mistaken for coverage |
| CLI verbs and `--json` | Rust | Verified |
