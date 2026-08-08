# e2e-android — the shared frontend in the Android system WebView

This is a **UI harness for the Android layer** of
[`docs/rewrite/testing-policy.md`](../docs/rewrite/testing-policy.md). It
attaches to the WebView of the app already running on a device, over the
Chrome DevTools Protocol, and drives it: query the DOM, tap a control, type
into a field, read the accessibility surface back.

Before it existed, nothing on Android could press a button. `android-smoke.sh`
established that *something painted and had names* — a mapped WebView
implementation in `/proc/<pid>/maps`, a focused window in `dumpsys`, and named
descendants in a uiautomator dump. It could not navigate between screens,
assert a layout, or assert that a secret was absent from `outerHTML`.

```sh
# with the debug APK installed and running (android-smoke.sh leaves it that way)
cd e2e-android && npm ci && npm test
```

## What this establishes, and what it does not

The engine is the Android system WebView — Chromium, the one that ships. That
is the whole reason this layer is worth having, and it is also the reason to
be exact about its edges.

**It establishes**, on the shipping engine:

- The frontend mounts, renders and lays out with real boxes — not jsdom's 0×0.
- Navigation between History, Devices and Settings under a real tap, with
  `Input.dispatchMouseEvent` hit-testing the control's box.
- Keyboard input reaching a text field, and the list re-rendering from it.
- INV-10: a flagged item's plaintext is absent from `outerHTML` and from every
  live input value, not merely covered.
- INV-12 / CLAUDE.md rule 4: no accessible string — text, `title`,
  `aria-label`, `placeholder`, `value` — carries a filesystem path.
- That the share-sheet doorway reaches the store *and the screen*, which is
  further than `android-smoke.sh` follows it.

**It does not establish**:

- **Anything native.** No TalkBack, no `FLAG_SECURE`, no intents beyond the one
  it uses as a fixture, no Quick Settings tile, no notification, no camera. CDP
  sees the document and nothing outside it.
- **What a person sees.** `getBoundingClientRect` is layout, not paint. A
  correct box under a covering surface, a broken font or a compositor failure
  all read as a pass here. `assert_painted` in `android-smoke.sh` is the check
  that looks at pixels, and it is weaker in a different direction.
- **The release build.** It requires a debuggable APK by construction — see
  below. The minified artefact people install is `android-smoke-release.sh`'s
  subject and stays so.
- **Any macOS requirement.** Same rule as every other layer: WKWebView is not
  this engine, and nothing here substitutes for that layer.

## Attaching, and the one thing that goes wrong

The WebView publishes an abstract Unix socket named
`@webview_devtools_remote_<pid>`, and `adb forward` maps a local TCP port onto
it.

The pid is in the name. `adb forward` binds the local port whether or not the
remote name still exists, so a forward established before a restart is still
"there": the connection is accepted and then closed with no bytes. `curl`
reports that as `Empty reply from server` and undici as `other side closed`,
and neither says the only thing it ever means, which is that the app restarted.
`src/harness/devtools.ts` resolves the name from `/proc/net/unix` immediately
before forwarding, proves the endpoint answers `/json/version`, and re-resolves
rather than reporting a broken endpoint.

Two further properties are asserted rather than assumed:

- `/json/version` reports `Android-Package`, and it must be ours. The WebView
  zygote publishes sockets too.
- Android can destroy and recreate the activity at any time — a configuration
  change, memory pressure, a relaunch. CDP then answers every call with
  "detached Frame", so `AndroidApp.withPage` reattaches instead of failing the
  assertion that happened to be in flight.

## Why this cannot reach a release build

wry emits the call that enables it under a compile-time predicate:

```rust
// wry-0.55.1/src/android/main_pipe.rs
#[cfg(any(debug_assertions, feature = "devtools"))]
self.env.call_static_method(&rust_webview_class, "setWebContentsDebuggingEnabled", "(Z)V", …)
```

Nothing in this workspace enables `devtools` — `crates/copypaste-ui/src-tauri`
takes `tauri` with `tray-icon`, `macos-private-api` and `image-png`, and
`[profile.release]` does not turn `debug-assertions` back on. In a release
build the call is not in the binary, and the default it would have passed is
`false` anyway (`wry/src/lib.rs`: `#[cfg(not(debug_assertions))] devtools:
false`).

That is an argument, so both legs assert it instead:

- `android-smoke.sh` requires the debug build to publish the socket. A build
  that does not fails there, naming this harness, rather than timing out here.
- `android-smoke-release.sh` requires the shipped build to publish **no** such
  socket, and fails if one is open.

The detector behind both is `devtools_sockets` in `android-smoke-lib.sh`, with
fixtures under `--self-test` — including a pid that is a prefix of another
process's socket name, which a looser match would report as a leak.

This harness also refuses to start against a non-debuggable package, so it can
never quietly become a reason to ship one.

## Fixtures

Each run mints a nonce and shares two clippings through `ACTION_SEND` into
`IntakeActivity`: a credential matching the `aws_access_key` rule
(`AKIA[0-9A-Z]{16}`, confidence 0.99) and an ordinary clipping. The nonce is
per-run because the store deduplicates, and a second run would otherwise be
asserting against the first one's rows.

Two constraints worth knowing before editing the seeding:

- **One share at a time, each confirmed on screen.** Two `am start`s in a row
  reach `IntakeActivity` while the first is still finishing and the second is
  dropped — silently, because `am` reports that it started the activity either
  way.
- **Clear the search field first.** The activity is `singleTask` and comes back
  exactly where it was left, including a filter from a previous run, which
  reads as "the item was never ingested".
- **Never count rows to decide that one arrived.** The list is virtualised into
  a fixed window, so an item arriving at the top evicts one at the bottom and a
  count of masked rows is unchanged whenever the evicted row was masked too.
  The credential is confirmed by the top row being masked, and the ordinary
  clipping by its rendered text.

The device is not reset between runs; the app keeps its history. Assertions are
written against the run's own fixtures, never against a row count or a
position.
