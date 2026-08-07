# The macOS shell spike

**Status:** open · not one of the six surfaces below has ever run · this
document is the procedure, not a result
**Blocking:** the README's "Unverified" row and six
`NOT VERIFIED IN CI` lines in
[testing-policy.md](testing-policy.md) § Native shell and § UI rest on source
reading alone.

This is [android-spike.md](android-spike.md)'s job for the other platform. It
lists what CI settled, what it structurally cannot settle, and — for each of the
tray, the popover, the global hotkey, launch at login, the notification and
WKWebView — the gesture that would falsify what the code currently claims.

## What CI settled

`.github/workflows/ci.yml` → `macOS check + platform (arm64)` on `macos-14`,
every push and pull request:

* `cargo clippy --all-features --all-targets` **parses and type-checks** the
  macOS-only paths — the `objc2` tray seam, the Keychain backend, the
  `#[ignore]`d platform tests. No other job in the workspace can even compile
  them.
* The real `NSPasteboard`: `changeCount`, burst loss, self-write suppression,
  the `org.nspasteboard.*` opt-outs, the size cap. An empty run fails the job.
* The real Keychain, against a throwaway one the job creates and makes default.

`release.yml` → `smoke-macos-dmg.sh`, on a tag only, adds: the DMG mounts, the
signature verifies, the cask postflight runs, the daemon inside the bundle
answers, and **a `com.apple.WebKit.WebContent` process appears** after `open -a`.

## What none of it settled, and why

The runner is headless. There is no window server, so there is no menu bar to
put a status item in, no display to place a popover on, no notification centre
to post to, no login session to add a launch agent to and nothing to render a
frame for. Every one of the six surfaces below needs a console session with a
human in front of it; that is the whole reason this file exists rather than
another workflow.

The WebContent process is the one exception, and it is worth being precise
about what it is: evidence that a WebView was **instantiated and started
loading**, from outside the process. It is not evidence that anything was
painted, and the smoke script's own comment says so. The entire frontend suite
runs on WebKitGTK under Linux, which is a different engine — nothing in this
repository has ever observed WebKit itself.

## Three traps, each of which will cost a run

**A screenshot of either window is expected to be blank.** `contentProtected`
is `true` in `tauri.conf.json` for `main` and
`QUICK_PASTE_CONTENT_PROTECTED` is `true` for the popover, both **before the
first frame** — that is INV-35, and `shell::protection` is only the loosening
path. So `screencapture` returning nothing is the feature working. To photograph
a painted frame, turn *Allow screenshots* on in Settings first, or point a
camera at the display. Do not conclude "it did not render".

**And the window is `transparent: true`.** Even with protection off, a
pixel-difference check against a background is not the assertion; a human
looking at the screen is.

**There is no accessibility tree to read.** android-spike.md's strongest signal
was uiautomator's 33 named nodes. The macOS equivalent, `AXUIElement`, requires
an Accessibility TCC grant — the exact permission ADR-0001 and
`shell::hotkey` exist to keep this app from ever needing, held against a cdhash
that moves on every build. **Do not grant it to get a test to pass.** Doing so
would make the run unrepresentative of what ships and would invalidate item 3
below at the same time.

## Setup

1. Build and install the way a user does, so the code identity under test is
   the shipped one: `scripts/release/build-macos-app.sh <version>`, then the
   DMG and `brew install --cask`, or at minimum `packaging/macos/selfsign.sh`
   against `dist/CopyPaste.app`. An `open -a` of a `cargo run` build tests a
   different signature and a different bundle.
2. Record the macOS version, the Mac model, the build's `CDHash`
   (`codesign -dvvv`), and whether the certificate path or the ad-hoc fallback
   was taken — `selfsign.sh` says which. Items 3 and 6 turn on it.
3. Nothing in the evidence may contain a clipboard payload, a local path or a
   username (CLAUDE.md rule 4, INV-12). Use a marker like
   `spike-hotkey-YYYYMMDD-HHMM` and confirm it is not classified as sensitive.

## The six

Each names what the code claims, the gesture, what counts as a pass, and what
would falsify it.

### 1. The menu-bar item

Claimed: `shell::tray::build` creates a status item with a tooltip, a menu, and
`show_menu_on_left_click(false)`; the menu is re-read on every change event and
on a 5 s backstop.

*Gesture.* Launch the app. Look at the menu bar. Right-click the icon. Copy
something in another app and watch the recent submenu.

*Pass.* An icon appears in the menu bar. A **secondary** click opens the menu
with Show CopyPaste, Settings, Open at login, Private mode, the recent submenu
and Quit. A newly copied item appears in the submenu within about five seconds
without any further click.

*Falsified by.* No icon at all — `TrayIconBuilder::build` failed and `setup`
returned the error, so the app will not have started its service either. A menu
that opens on the **left** click, which means the popover gesture is gone. A
submenu that never updates: the change stream is dead and the 5 s backstop is
not running, which is exactly the failure `BACKSTOP` was written for.

*And the one nobody would look for:* **the recent items have no icons.** macOS
27 hides menu-item images by default and
`tray::menu_image_visibility::force_menu_image_visibility` exists to force them
back with `setPreferredImageVisibility:`. It runs after Tauri attaches the
status menu and is silent on failure. Missing imagery falsifies that seam, and
nothing else will report it.

*Never acceptable.* A clipping the app flagged as sensitive appearing in the
submenu, in any form — including a "1 hidden" row. `recent::Clipping::from_item`
answers `None` for a flagged item precisely so this cannot happen; seeing one is
a P0 against manifest 06, not a cosmetic bug.

### 2. The popover

Claimed: a left click on the tray icon toggles a frameless 403×624 Quick Paste
window, positioned by `window::anchor` under the icon's rect, clamped to the
monitor with an 8 px inset and a 6 px gap; it hides on focus loss and restores
the previously frontmost application.

*Gesture.* Left-click the icon. Click away. Repeat with the app on an external
display, on a display arranged to the *left* of the primary (negative origin),
and on a display with a different scale factor from the built-in one.

*Pass.* The popover appears directly under the icon, fully on screen, on the
display the icon is on. Clicking elsewhere hides it and returns keyboard focus
to the application that had it before.

*Falsified by.* A popover on the wrong display — the two-scale-factor case is
what `to_physical` and `monitor_from_point` were written for, and it is the one
that cannot be tested against synthetic monitors. A popover half off the screen
edge, which falsifies the clamp. A popover that does not appear at all: one of
`tray.rect()`, `outer_size()` or `monitor_from_point` returned `None` and
`position_under_tray` returned early — deliberately, because a mispositioned
popover beats a missing one, so this failure is silent by design. And a
dismissal that leaves focus nowhere, or that reactivates the prior app twice
(V-12).

*Watch the Dock icon while you do it.* `hide_quick_paste_window` flips the
activation policy to `Accessory` when there is no previous application to
restore and back to `Regular` otherwise (AT-39, V-11). A Dock icon that
disappears and never comes back, or one that flickers on every dismissal,
falsifies that.

### 3. The global hotkey — and TCC (B-31)

Claimed, and this is the strongest claim in the tree made without observation:
`tauri-plugin-global-shortcut` → `global-hotkey` 0.8.0 registers
`CmdOrCtrl+Shift+V` through Carbon `RegisterEventHotKey`, which needs **no
Accessibility grant**; only the five media keys fall through to
`CGEventTapCreate`, and `hotkey::is_permission_free` refuses those. The module
header states outright that this is inferred from the API used, not observed.

*Gesture.* With the app running and **another** application frontmost, press
Cmd+Shift+V. Then open System Settings → Privacy & Security → Accessibility and
look for CopyPaste.

*Pass.* The popover appears. **No permission dialog appears at any point**, and
CopyPaste is absent from the Accessibility list.

*Falsified by.* A TCC dialog asking for Accessibility — that refutes the source
reading and, with it, ADR-0002's conclusion; the feature would then be revoked
on every update under ADR-0001 and needs redeciding, not fixing. An entry
appearing in the Accessibility list even without a prompt is the same finding.
Nothing happening at all means `on_shortcut` returned `Err` — the message is
"That shortcut is already in use by another app", and something else on the Mac
holds Cmd+Shift+V.

*The second press is the assertion.* `global-hotkey` sends both `Pressed` and
`Released` for a Carbon hotkey and the handler filters on `Pressed`. A popover
that opens and instantly closes, or that toggles twice per press, falsifies that
filter.

*While you are there,* rebuild the app, re-install it and press the key again.
That is ADR-0001's cdhash argument on the surface that would suffer from it.

### 4. Launch at login

Claimed: `tauri-plugin-autostart` with `MacosLauncher::LaunchAgent` — a plist
the user owns, deliberately not the `AppleScript` strategy, because that would
be an Automation TCC grant revoked on every update.

*Gesture.* Toggle **Open at login** in the tray menu. Check
`~/Library/LaunchAgents` for a new plist and System Settings → General → Login
Items. Log out and back in. Toggle it off and repeat.

*Pass.* A plist appears, the app is listed under Login Items, it starts on the
next login, and toggling off removes the plist and the listing.

*Falsified by.* A prompt to control System Events — that means the
`AppleScript` strategy was taken and the launcher constant did not do what its
comment says. A toggle that appears to work but writes nothing: `set_enabled`
maps every failure to a fixed string and `is_enabled` answers `false` on any
error, so a write failure is visible only as a toggle that will not stay on.
The app being listed but not starting is a plist naming a path that has moved —
the case the plugin is supposed to own.

*Known, not a defect.* The menu's checkmark is read once at
`TrayMenu::build`, so a change made in System Settings while the app is running
does not show until restart. `tray::build` says so.

### 5. Notification and sound on copy

Claimed: the daemon cannot post — `UNUserNotificationCenter` needs a bundle and
the daemon is a bare launchd executable — so the daemon emits
`EventData::captured` and the app posts, but only when `notify_on_copy` is on
and the app is **not** frontmost.

*Gesture.* Turn on notify-on-copy (and sound-on-copy) in Settings. Move another
app to the front. Copy the marker. Then bring CopyPaste to the front and copy
again. Then choose an item from the tray's recent submenu.

*Pass.* First copy: a notification reading "Saved to CopyPaste" / "What you just
copied is in your history". Second copy: **nothing** — `should_post` returns
false while the app is focused. The tray selection posts "Copied from
CopyPaste", with a sound if sound-on-copy is on.

*Falsified by.* No notification with the app in the background: either
`permission_state()` did not answer `Granted` — the code assumes macOS answers
`Granted` and decides at post time, which is exactly the assumption to check —
or `show()` failed, and it fails into a `tracing::debug!` that nobody will see
without `RUST_LOG=debug`. Run with it. A notification appearing while the app
*is* focused falsifies `is_foreground`, which asks `window.is_focused()` and
treats an error as not-focused.

*Never acceptable.* Any clipboard content in the notification body. A
notification is unblurrable and is often on screen during a screen share; the
four strings are constants for that reason.

### 6. A frame on WKWebView

Claimed: only that a `WebContent` process appears (tag-only, REPORTED). Whether
the React app **paints** on WebKit has never been observed anywhere — the
browser layer is WebKitGTK on Linux, a different engine.

*Gesture.* Open the main window. Read it. Scroll the history past a few hundred
items. Open Settings, the Devices screen and the pairing dialog. Resize the
window narrow and wide.

*Pass.* Text renders, the history list scrolls and virtualises without gaps or
overlap, the tabs switch, dialogs open and close, and the console (Develop →
Web Inspector, or `RUST_LOG=debug`) shows **no errors**. Note this is the same
set of behaviours the WebKitGTK suite asserts on Linux; the question is only
whether WebKit answers the same way.

*Falsified by.* A blank or white window with a live WebContent process — the
CSP is stricter than the dev CSP and a violation shows only in the console. A
list that renders but jumps while scrolling: INV-1/5/6's row-height reservation
and scroll anchoring are verified on WebKitGTK and nowhere else. Fonts falling
back, which would be the `font-src 'self' data:` rule biting differently.

*Remember trap 1.* Turn Allow screenshots on before trying to capture any of
this.

## What to answer while you are there

* Does the app survive a **second** launch — `open -a` while it is already
  running? It is a menu-bar app with a `Regular` activation policy at startup;
  the second instance's behaviour is undefined by anything in the tree.
* Does the tray menu's **Private mode** toggle round-trip to the daemon and back
  into the menu's checkmark, and does the history screen agree?
* Does **Quit** from the tray actually exit, and does closing either window
  leave the app running (INV-36)?
* With the daemon **not** running, does the app show the service-offline screen
  and does starting the service from it work? `ci.yml` cannot reach launchd or
  Homebrew, so this is `NOT VERIFIED IN CI` too.
* How long after `open -a` does the first frame appear? android-spike.md found
  33 and 38 seconds on an emulator and nearly shipped a check that sampled at
  25. Time it, and say what the margin is.
