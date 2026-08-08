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
| Android | `android-emulator.yml` — the built APK installed on an x86_64 emulator, plus `e2e-android/` driving its WebView over CDP | the Android platform APIs, at the API level that ran | macOS; any aarch64 code path; an OEM build; a Play-updated WebView; a physical device |
| macOS | `ci.yml` → `macOS check + platform (arm64)`; `release.yml` → `smoke-macos-dmg.sh` on a tag | macOS | Android |

The browser layer does execute Tauri commands through `wry` on Linux. That is
useful and it is not verification: the shipped bridges are WKWebView and the
Android WebView, and each command's behaviour belongs to its platform's layer.

The Android layer's UI half attaches to the shipping WebView over the Chrome
DevTools Protocol, so it sees the document and nothing outside it: layout, not
paint; the DOM, not TalkBack; and only a debuggable build, because wry compiles
the call that enables it out of a release one. `e2e-android/README.md` states
the edges.

The CLI is a test surface, not a product surface (CLAUDE.md rule 6). A CLI
assertion never satisfies a UI requirement.

## What triggers what

A change to shared React, the Rust core, `copypaste-ipc` or the Tauri bridge
can break either platform, so it must run every platform layer. `ci.yml`
(Linux + macOS) and `browser-webkitgtk.yml` run on every push and pull request
with no path filter. `android-emulator.yml` runs nightly, on demand, and on any
push or pull request touching those shared paths or the Android tree.

`release.yml` runs on a tag. Everything it alone proves — installing the DMG,
launching the bundle, the cask — is therefore post-hoc, and is marked below.

## What an emulator is not

The Android layer is a Google APIs system image on `ubuntu-24.04`, x86_64, with
no vendor software on it. Four requirements sit outside that at every API level.
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
- **A real install of the signed release APK.** R8 runs only on the release
  build type, and the release leg signs with a key generated per run and never
  published (ADR-0006), so the artifact that leg exercises is not the artifact a
  user installs.

### API levels

Clipboard access is API-gated — background reads at 10 (29), the notification
runtime permission at 13 (33), foreground service types at 14 (34) — so one API
level is one point on that curve
([android-clipboard-access.md](android-clipboard-access.md)). The nightly runs
API 36 alone; `api-level` is a `workflow_dispatch` input and nothing else varies
it. A requirement whose gate the level that ran does not cross is not
established by that run.
[android-api-levels.md](android-api-levels.md) is the per-level expectation
table and the local runner that covers the spread.

## Matrix

`Verified` — the authoritative layer runs it on every push or pull request.
`Partial` — it runs, but only on a tag or a nightly, or it asserts less than
the requirement. `NOT VERIFIED IN CI` — no run anywhere establishes it.

### Capture

| Requirement | Authoritative layer | State |
|---|---|---|
| `changeCount`, burst loss, self-write suppression, `org.nspasteboard.*`, size cap | macOS | Verified — `--ignored` tests on `macos-14`; an empty run fails the job |
| A real `pbcopy` reaches history through the shipped bundle | macOS | Partial — ENFORCED in `smoke-macos-dmg.sh`, tag-only |
| Capture pipeline against the fake source | Rust | Verified |
| `ACTION_SEND` / `ACTION_PROCESS_TEXT` reach SQLCipher | Android | Partial — debug leg only; the release leg has no `run-as` and prints `NOT ASSERTED`. `e2e-android/` follows an `ACTION_SEND` further, onto the screen |
| Rung 2: the shell-uid clipboard read | Android | Partial — nightly only; `android-rungs.sh` reads a clip another app copied as uid 2000 naming `com.android.shell`, with no focus, and `AppOpsManager.checkPackage` refuses every other package |
| Rung 2: the Shizuku transport | Android | **NOT VERIFIED IN CI** — the binder wrapper, the reflected `IClipboard` and whether the listener fires need a pairing granted by hand, which a stock emulator cannot give |
| `ClipListener` / `ClipQueue` | Android | **NOT VERIFIED IN CI** — the listener never registers without Shizuku; no Kotlin unit test exists |
| `CaptureService` | Android | Partial — nightly only, and only the negative: with `enabled=true` written straight into `shared_prefs`, no foreground service and no notification appear. Nothing asserts it captures |
| Quick Settings tile | Android | Partial — nightly only; one `click-tile` starts `ClipboardCaptureActivity` and the foreign clip reaches SQLCipher unreadable |
| Android 12+ clipboard-toast consent gate | Android | **NOT VERIFIED IN CI** — the Rust refusal and the jsdom dialog support it; the OS toast is unobserved |
| Capture surviving OEM background restriction and vendor battery management | Android | **NOT VERIFIED IN CI** — stock AOSP applies none of it; spike item 6 needs a phone left idle |
| Any API level other than the one that ran | Android | **NOT VERIFIED IN CI** — the nightly runs 36 alone; the gates at 29, 33 and 34 are uncrossed |

### Crypto and device secret

| Requirement | Authoritative layer | State |
|---|---|---|
| XChaCha20-Poly1305 + HKDF, item id as AAD, fail-closed, zeroized | Rust | Verified |
| Device secret in the macOS Keychain | macOS | Verified — throwaway keychain; a self-skipped test fails the job |
| Device secret in the Android Keystore | Android | Verified — a second launch reopens the same database |
| The Keychain item survives a re-signed binary (manifest 02 §3.8) | macOS | **NOT VERIFIED IN CI** — REPORTED leg, tag-only |
| The aarch64 hardware SHA-2 path executes as built | Android | **NOT VERIFIED IN CI** — the emulator is x86_64 and takes the x86 path; the only evidence is a static count, 56 SHA-2 instructions in the aarch64 object against 0 on x86_64, which shows they ship and not that they run |

### Storage and detection

| Requirement | Authoritative layer | State |
|---|---|---|
| SQLCipher at rest, no plaintext in the file | Rust | Verified — and confirmed against a pulled database on Android |
| Schema, dedup interval, tombstones, pins, FTS5, keyset pagination | Rust | Verified |
| A sensitive item never reaches the index (write, read, purge) | Rust | Verified |
| Retention: history cap, TTL, sensitive auto-wipe | Rust | Verified |
| Export, import, backup, restore over IPC | Rust | Verified |
| The same four from the Settings screen | Browser (WebKitGTK) | **NOT VERIFIED IN CI** — the suite drives the CLI; the commands and the UI exist |
| Secret-detection ruleset, NFKC, Luhn, confidence bands | Rust | Verified |
| A v0.4 database is detected and explained, never opened | Rust | Verified as a probe; **NOT VERIFIED IN CI** on either platform — the Android side answers a hardcoded `false` (B-33) |

### IPC and daemon

| Requirement | Authoritative layer | State |
|---|---|---|
| `0600` socket, `LinesCodec`, timeouts, connection and watcher caps | Rust | Verified |
| No filesystem path in any user-facing string (INV-12) | Rust, and a sweep at the browser layer | Verified |
| `GetConfig` / `SetConfig` with per-field liveness | Rust | Verified |
| The Settings screen driving them | Browser (WebKitGTK) | **NOT VERIFIED IN CI** — jsdom mocks the two calls; the e2e file drives the socket through the CLI |
| Service start, restart and shutdown from the app | macOS | **NOT VERIFIED IN CI** — the browser layer starts a Linux `target/debug` daemon; launchd and Homebrew are unexercised |
| Push (`Method::Watch`) and degrade-to-polling | Rust, Browser (WebKitGTK) | Verified on Linux; the shipped bridges are **NOT VERIFIED IN CI** |

### Sync

| Requirement | Authoritative layer | State |
|---|---|---|
| Pairing: code mint, TTL, wrong code, unpair, revoke, Noise `NNpsk0` | Rust, Browser (WebKitGTK) | Verified — the browser layer pairs against a second real daemon |
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
| The app renders on WKWebView | macOS | **NOT VERIFIED IN CI** — the tag-only smoke observes a `WebContent` process, nothing more |
| The app renders on the Android WebView | Android | Partial — `assert_painted` degrades to `NOT ASSERTED` when the screen is asleep or uiautomator returns no dump |
| The frontend mounts and lays out in the Android WebView | Android | Verified — `e2e-android/` asserts a React root with children and non-zero row boxes |
| Navigation and keyboard input on Android | Android | Verified — a real tap moves between screens, typing filters the list |
| A sensitive item is absent from the Android document, not obscured (INV-10) | Android | Verified — `e2e-android/` against `outerHTML` and every live input value |
| No filesystem path in any Android accessible string (INV-12) | Android | Verified — `e2e-android/` sweeps text and the naming attributes |
| Virtualisation and the INV-5 reservation on Android | Android | Verified — `e2e-android/history-render` measures the window, the spacer and every row box |
| Scroll anchoring on Android (INV-1, INV-6) | Android | Verified — `e2e-android/scroll-anchor`, reading offset and row window in one evaluation |
| Settings on Android: tabs, a preference reaching layout, one surviving a reload | Android | Verified — `e2e-android/settings`, through the Tauri store plugin rather than a daemon |
| Per-row actions absent in selection mode on Android (§3.1.5) | Android | Verified — `e2e-android/bulk-actions`, against the "Item actions" trigger Android renders instead of four buttons |
| Pairing on Android: minting a code, its QR and SAS (INV-13) | Android | Partial — `e2e-android/devices` drives the mint side and the join form; accepting needs a second device and is not driven |
| Android UI beyond the rows above | Android | **NOT VERIFIED IN CI** — CDP sees the document only: no pixels, no TalkBack, nothing native. Export, import and push have no Android coverage |
| The release build exposes no WebView debugger | Android | Verified — `android-smoke-release.sh` fails if the shipped APK publishes `@webview_devtools_remote_<pid>` |
| The WebView a user's phone actually has | Android | **NOT VERIFIED IN CI** — the emulator's is pinned into the system image; the shipped one updates from Play on its own schedule |
| VoiceOver and TalkBack | macOS, Android | **NOT VERIFIED IN CI** — no screen reader is driven anywhere |

### Native shell

| Requirement | Authoritative layer | State |
|---|---|---|
| Menu-bar item, menu, recent-items submenu | macOS | **NOT VERIFIED IN CI** — the tag-only smoke infers it from the daemon answering |
| Popover placement under the tray icon | macOS | **NOT VERIFIED IN CI** — geometry is unit-tested against synthetic monitors |
| Window dismissal on blur | macOS | **NOT VERIFIED IN CI** |
| Global hotkey, including whether TCC accepts the certificate (B-31) | macOS | **NOT VERIFIED IN CI** — nothing registers a shortcut anywhere |
| Launch at login | macOS | **NOT VERIFIED IN CI** |
| Notification on copy | macOS | **NOT VERIFIED IN CI** — nothing posts or asserts one |
| Sound on copy | macOS | **NOT VERIFIED IN CI** — the `should_play` gate is Rust-verified; the spawn is not |
| Screen-capture protection, INV-35 (`contentProtected`, `FLAG_SECURE`) | macOS, Android | Partial — on Android, nightly only: `android-rungs.sh` finds `FLAG_SECURE` on the window in twenty dumps over a minute, with another window on the same device read as unprotected by the same reader. macOS `contentProtected` is unasserted, and the jsdom test covers the preference toggle only |

### Packaging

| Requirement | Authoritative layer | State |
|---|---|---|
| macOS build, codesign, DMG, no linkage outside the bundle | macOS | Partial — ENFORCED, tag-only |
| Cask postflight, quarantine removal, re-seal | macOS | Partial — ENFORCED, tag-only; Gatekeeper's verdict is a note |
| `brew install --cask` as a user runs it | macOS | **NOT VERIFIED IN CI** — `check.sh` round-trips the generators only |
| Published universal APK: build, R8, release signing, install, launch, no stripped symbol | Android | Partial — the exact signed artifact is checksum-verified and smoke-tested before publication, tag-only |
| The signed release APK installed on a physical device | Android | **NOT VERIFIED IN CI** — no device runs anywhere; the emulator leg's release key is generated per run and its APK is never published |
| Notarisation and Gatekeeper acceptance | macOS | **NOT VERIFIED IN CI** — ADR-0001 decided against notarisation; recorded so it is not mistaken for coverage |
| CLI verbs and `--json` | Rust | Verified |
