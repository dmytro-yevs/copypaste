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
| Android | `android-emulator.yml` on x86_64; `release.yml` on a physical arm64 device | Android platform behavior at each API level that ran; the signed release APK and arm64 path at release | macOS; OEM background policy; a Play-updated WebView |
| macOS | `ci.yml` → `macOS check + platform (arm64)`; `release.yml` → `smoke-macos-dmg.sh` on a tag | macOS | Android |

The browser layer does execute Tauri commands through `wry` on Linux. That is
useful and it is not verification: the shipped bridges are WKWebView and the
Android WebView, and each command's behaviour belongs to its platform's layer.

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
| `ACTION_SEND` / `ACTION_PROCESS_TEXT` reach SQLCipher | Android | Partial — debug leg only; the release leg has no `run-as` and prints `NOT ASSERTED` |
| Rung 2: Shizuku shell-uid clipboard read | Android | **NOT VERIFIED IN CI** — pairing cannot be granted on a stock emulator |
| `ClipListener` / `ClipQueue` / `CaptureService` | Android | **NOT VERIFIED IN CI** — only the negative case is asserted; no Kotlin unit test exists |
| Quick Settings tile | Android | **NOT VERIFIED IN CI** — probed, never asserted |
| Android 12+ clipboard-toast consent gate | Android | **NOT VERIFIED IN CI** — the Rust refusal and the jsdom dialog support it; the OS toast is unobserved |
| API-level spread | Android | Partial — the scheduled matrix runs API 24, 29, 33, 34 and 36; targeted dispatches run one selected level |

### Crypto and device secret

| Requirement | Authoritative layer | State |
|---|---|---|
| XChaCha20-Poly1305 + HKDF, item id as AAD, fail-closed, zeroized | Rust | Verified |
| Device secret in the macOS Keychain | macOS | Verified — throwaway keychain; a self-skipped test fails the job |
| Device secret in the Android Keystore | Android | Verified — a second launch reopens the same database |
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
| Anything about the Android UI beyond a painted screen | Android | **NOT VERIFIED IN CI** |
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
| Screen-capture protection, INV-35 (`contentProtected`, `FLAG_SECURE`) | macOS, Android | **NOT VERIFIED IN CI** — the jsdom test covers the preference toggle only |

### Packaging

| Requirement | Authoritative layer | State |
|---|---|---|
| macOS build, codesign, DMG, no linkage outside the bundle | macOS | Partial — ENFORCED, tag-only |
| Cask postflight, quarantine removal, re-seal | macOS | Partial — ENFORCED, tag-only; Gatekeeper's verdict is a note |
| `brew install --cask` as a user runs it | macOS | **NOT VERIFIED IN CI** — `check.sh` round-trips the generators only |
| Published universal APK: build, R8, release signing, install, launch, no stripped symbol | Android | Partial — the exact signed artifact is checksum-verified and smoke-tested before publication, tag-only |
| The signed release APK installed on a physical device | Android | Partial — publication depends on a tag-only hardware gate that installs and smoke-tests the exact artifact on one physical `arm64-v8a` device |
| Notarisation and Gatekeeper acceptance | macOS | **NOT VERIFIED IN CI** — ADR-0001 decided against notarisation; recorded so it is not mistaken for coverage |
| CLI verbs and `--json` | Rust | Verified |
