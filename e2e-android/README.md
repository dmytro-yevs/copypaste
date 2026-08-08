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

## What is driven

| File | What only a running program on this engine can show |
|---|---|
| `attach` | the harness reaches our package's WebView and nothing else's |
| `interaction` | navigation under a real tap, keyboard input, rows with real boxes |
| `leaks` | INV-10 and INV-12 against `outerHTML` and the accessible surface |
| `history-render` | virtualisation, the INV-5 reservation, clipping, INV-8's list semantics |
| `scroll-anchor` | INV-1 when the list grows under the viewport, INV-6 when it shrinks |
| `settings` | the Android tab set, A11Y-15's wrapping row, a preference reaching layout, and one surviving a reload through the Tauri store plugin |
| `bulk-actions` | per-row actions **absent** in selection mode, and a bulk delete reaching the store |
| `history-controls` | every toolbar control laid out inside a 412px screen at its promised touch target, and the kind filter narrowing the list |
| `devices` | the pairing ceremony end to end on the mint side — code, QR, SAS — and the code leaving the document when the dialog closes |

### Seeding

`src/harness/bridge.ts` puts items in through `window.__TAURI_INTERNALS__.invoke`.
The browser layer has a daemon and a CLI to seed with; Android has neither, and
the bridge is the same command the screen calls, so an item seeded through it
arrives exactly as a copied one would. Intents remain the doorway for the
sensitive fixture, because that path is itself under test.

Each file deletes what it seeded. The device is not reset between runs, and
150 rows left behind change the shape of the list the next run measures.

## What is deliberately not ported

- **`daemon-config` and `service-lifecycle` do not apply.** Both drive a daemon
  over a Unix socket, and Android has no daemon — the core is linked in-process
  (ADR-0003). There is no offline screen offering to start a service and no
  second process to configure. `GetConfig`/`SetConfig` still exist behind the
  bridge, and the Android surface for them is the Service and Background
  capture tabs, which `settings` covers.
- **A pairing that completes.** `devices` mints a code, renders the QR and the
  SAS, and asserts the join form; accepting needs a second device on the
  network, which the browser layer supplies as a CLI fixture and a phone has no
  counterpart for.
- **`export-import` and `push`.** Not reached, for budget rather than for a
  reason of principle. `export-import` drives the CLI on the browser layer and
  would have to be re-expressed against the Storage surface, which Android does
  not show; `push` needs a `copypaste://changed` emitter, which on Android is
  the in-process core rather than a daemon.
- **Keyboard navigation of the list.** `history-render` on the browser layer
  asserts Arrow/Home/End/Escape/Ctrl+F. Those are desktop bindings on a screen
  with no keyboard; the Android file asserts the list's ARIA contract instead
  and leaves the bindings to the layer whose users have keys.

## Two things this layer found that the others could not

- **A tab a tap could not reach.** `TabsTrigger` carries `flex-1`, which is
  `flex: 1 1 0%`, and a hypothetical main size of zero always fits — so
  A11Y-15's wrapping row never wrapped, seven `nowrap` labels shared 412px and
  overflowed onto each other. Every box was correct and non-overlapping;
  only `elementFromPoint` knew that the centre of Service belonged to
  Background capture. Fixed by giving the Android trigger `flex-none`.
- **The reservation is a floor here, not a height.** `HistoryList` sets
  `minHeight` on Android where desktop gets `height`, so the virtualiser
  measures the row back and a one-line row settles at 68px against a 67px
  reservation. INV-5's actual claim — same height for every row, decided by the
  setting — is unaffected, and the tests assert the band rather than equality.

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

That is an argument, and a socket cannot settle it. Both workflows run on
`google_apis` emulator images, which are built `userdebug` and so set
`ro.debuggable=1`; the WebView provider then enables remote debugging for
**every** process on the device. Measured on API 36 `sdk_gphone64_x86_64`, both
endpoints answering `/json/version` and listing an attachable page:

| process | `run-as` | serves CDP |
|---|---|---|
| the signed, R8'd release APK | refused — not debuggable | yes, as `com.copypaste.app` |
| `com.android.htmlviewer`, a system package nobody here built | refused | yes |

So run 31229976299 failed on the emulator's behaviour rather than the build's,
and it failed only once the WebView actually painted — which is why the check
had been green until then, not because it had ever proved anything.

What the *artefact* says, counting the JNI method names wry emits side by side
in one function:

| `lib/x86_64/libcopypaste_ui_lib.so` | `setWebContentsDebuggingEnabled` | `setWebViewClient` / `setWebChromeClient` |
|---|---|---|
| debug APK | 1 | 1 / 1 |
| release APK | **0** | 1 / 1 |

The neighbours are what make the zero mean something. They are under no cfg, so
a scan reporting none of the three read the wrong file rather than a clean
build.

The two legs therefore assert:

- `android-smoke.sh` — the debug build publishes the socket. That is this
  harness's precondition and, on these images, nothing more.
- `android-smoke-release.sh` — the shipped APK carries no
  `setWebContentsDebuggingEnabled` call at all, **and** nothing answers CDP for
  our pid unless an app we did not build, which `run-as` also refuses, answers
  on the same device.

The APK assertion is unconditional. The control would otherwise also excuse a
build that had switched its own debugger on, which is the case it exists to
catch.

A device with `ro.debuggable=0` has not been tested here — no such device is
available to CI. The APK assertion is what carries the property to one.

The detectors are `wry_jni_counts`, `apk_wry_jni_counts`, `devtools_sockets`,
`devtools_socket_pids` and `devtools_cdp_package` in `android-smoke-lib.sh`,
with fixtures under `--self-test` — including a pid that is a prefix of another
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

## Known flake

`history-controls`'s fixture wait fails intermittently in a full-suite run —
roughly one run in three or four on a device that has been running the suite
all day, and never when the file runs alone. The clipping is in the store;
`add_item` returned its id, and a `list` right after the failure shows it. The
screen does not. Neither a scroll to the top, nor a remount through Devices and
back, nor two minutes of waiting brings it on, which rules out the virtualised
window and the 3s poll and points at the infinite query's accumulated pages
after a large delete. Unresolved; it is a precondition rather than an
assertion, so what it costs is the file, not a false green.
