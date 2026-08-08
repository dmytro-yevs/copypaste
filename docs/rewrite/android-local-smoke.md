# Running the Android smoke harness on a local emulator

`android-emulator.yml` builds an APK and boots an AVD before it reaches
`scripts/release/android-smoke.sh`. Everything in that preamble is a GitHub
runner detail; the harness itself is not. This is how to run it against an
emulator you already have, and what the result does and does not mean.

Observed: on 2026-08-08 this procedure ran the debug leg green on
`copypaste-api36` (API 36, x86_64, google_apis) — **31 assertions passed, 0
failed**, exit 0. The Android job had never reached its
reporting stage in CI, so that is the first end-to-end result this harness has
produced anywhere. It was taken against an APK built from a tree that predates
the performance package, so it says the harness works; it does not certify the
current tree.

## What the scripts actually require

They require far less than the workflow around them. Read this before wiring
anything up, because most of what the YAML installs is for the *build*.

| Assumption | Where it comes from | Locally |
|---|---|---|
| `adb` on `PATH` | asserted in Preflight | any `platform-tools` |
| One attached device, or `ANDROID_SERIAL` naming one | asserted in Preflight | see below |
| `APK` names a readable local file | asserted in Preflight | you supply it |
| The APK carries `lib/<device abi>/libcopypaste_ui_lib.so` | asserted in Preflight | a universal or x86_64 APK |
| The package is debuggable, so `run-as` works | asserted after install | debug leg only |
| `SMOKE_OUT` is writable, default `artifacts/android-smoke` | `mkdir -p` | gitignored |
| GNU `stat`, `od`, `sha256sum`, `unzip`, `python3`, `awk`, `mktemp` | used throughout | see *Hosts* |
| A checkout, for `--self-test`'s backup-rules fixtures | `backup_rules_report` | run from the repo |

`ANDROID_HOME`, `ANDROID_SDK_ROOT`, `NDK_HOME`, `JAVA_HOME`, the pinned
build-tools and `aapt2` are **not read by either smoke script**. They belong to
the build and AVD-creation steps. A machine with nothing but `adb` and a booted
device can run the harness.

`GITHUB_STEP_SUMMARY` is optional — `summary` falls back to `/dev/null`.

## The procedure

Built on Linux (or WSL), driven from the Windows host that owns the emulator.

### 1. An AVD with the same flags CI uses

```sh
avdmanager create avd -n copypaste-api36 -k "system-images;android-36;google_apis;x86_64"
emulator -avd copypaste-api36 -no-window -no-audio -no-boot-anim -gpu swiftshader_indirect
adb wait-for-device
```

Match `-gpu swiftshader_indirect` and `-no-window` even though a windowed
emulator is nicer to watch. They are what CI runs, and they change what
`screencap` returns (below).

`reactivecircus/android-emulator-runner` also disables animations. uiautomator
refuses to dump while the screen animates, which `dump_hierarchy` already
retries through, so this is a speed setting rather than a correctness one:

```sh
adb shell settings put global window_animation_scale 0
adb shell settings put global transition_animation_scale 0
adb shell settings put global animator_duration_scale 0
```

### 2. Get an APK onto the host

The debug leg needs a **debuggable** APK; the release leg needs a **signed,
minified, non-debuggable** one. They cannot be the same file, which is why the
workflow has four jobs.

```sh
cd crates/copypaste-ui && npm ci
npm run tauri -- android build --debug --apk --target x86_64
# → src-tauri/gen/android/app/build/outputs/apk/**/*debug*.apk
```

Copy it to wherever you will run the harness from.

### 3. Run it

```sh
export APK=/path/to/app-debug.apk
export SMOKE_OUT=/tmp/android-smoke          # anywhere outside the checkout
export ANDROID_SERIAL=emulator-5554          # optional; required if >1 device
./scripts/release/android-smoke.sh
```

Roughly four minutes: two 25 s settles, a 40 s capture window, and a paint poll
that resolved at 36–38 s after `am start` in the runs measured here.

The detectors run with no device at all, which is what `scripts/release/check.sh`
exercises:

```sh
./scripts/release/android-smoke.sh --self-test          # 34 passed, 0 failed
./scripts/release/android-smoke-release.sh --self-test
```

## Hosts

### Git Bash on Windows

Works, and is the only option when the emulator is a Windows process — see
*WSL* below. `adb`, `unzip`, `python3`, `sha256sum`, `od` and GNU `stat` are all
present or installable.

MSYS rewrites arguments that look like POSIX paths into Windows ones before
exec, so `adb pull /sdcard/ui.xml` asks the device for
`C:/Program Files/Git/sdcard/ui.xml`. `android-smoke-lib.sh` now excludes the
device-side roots itself; nothing needs setting by hand. It matters more than it
sounds like: unhandled, the UI-paint and `/proc/<pid>/maps` reads fail, both
blocks downgrade to `NOT ASSERTED`, and the run **exits 0 having stopped
asserting three things**.

A Windows checkout with `core.autocrlf=true` has CRLF working files. Git for
Windows' bash runs them anyway, but `shellcheck` reports every line as `SC1017`.
Strip `\r` into a scratch copy before believing shellcheck output there; git
still commits LF.

### WSL

The harness runs cleanly under WSL, but a WSL2 client cannot reach a Windows
`adb` server: WSL2 NAT puts the VM on its own subnet and the server binds
`127.0.0.1` on the host. `ADB_SERVER_SOCKET=tcp:<host>:5037` only works once
that server is restarted with `adb -a`, which binds every interface and exposes
device control to the LAN. Prefer building in WSL and running the harness from
Git Bash, or run the emulator inside WSL and keep both sides there.

### macOS

The scripts call `stat -c %s`, which is GNU-only; BSD `stat` needs `-f%z`. They
have not been run on a macOS host and would need that fixed first.

## What a local run cannot tell you

Nothing in the debug leg is weakened locally — every assertion CI makes, this
makes. What is missing is around it.

* **The release leg.** It runs here and its guard is real: pointed at a debug
  APK it fails on `the installed package is NOT debuggable` and stops, which was
  confirmed. A green release leg needs a minified APK signed with a throwaway
  key, which is Gradle plus `keytool`/`zipalign`/`apksigner` from pinned
  build-tools. Not observed locally.
* **The workflow's own checks** — `aapt2 dump badging`, the R8 `mapping.txt`
  assertion, `dependencyCheckAggregate` — are in `run:` blocks, not in the
  scripts, and no local run exercises them.
* **Rung 2.** Unchanged: Shizuku needs a wireless-debugging pairing by hand and
  prints `NOT ASSERTED` here exactly as in CI.
* **A local pass is not a CI pass.** `docs/rewrite/testing-policy.md` is the
  authority on what counts as verified, and it counts the workflow.

## Two ways a long-lived emulator lies

CI gets a fresh AVD per run. A local one does not, and both failures seen while
bringing this up were that difference rather than the product.

**Anything else driving the device will fail the run.** A concurrent
`am force-stop` on the package, logged as
`am_kill: [...] stop com.copypaste.app due to from pid <n>`, killed the app
mid-settle; a stray `am start` from uid 2000 (`shell`) pulled Settings in front
and took down both the focus and paint assertions. Give the harness the device
exclusively — no parallel `adb`, no clicking in the emulator window. When focus
assertions fail, read `$SMOKE_OUT/launch1.log` for `START u0` and `am_kill`
before suspecting the app.

**Stale tasks outlive an uninstall.** The harness uninstalls the package, so its
own state is clean, but another app left in the recents stack can surface the
moment the activity yields. Send the device home first.

## Evidence worth keeping from the first green run

* Paint resolved at 36 s and 38 s after `am start`, 85 named nodes under the
  WebView, first string `CopyPaste`. `PAINT_TIMEOUT` at 90 s has real margin.
* `screencap` returned a black 15 KB frame while that tree was fully populated,
  on the same run — confirming, on a second machine, the note in
  `assert_painted` that the framebuffer does not track the WebView under
  `-gpu swiftshader_indirect -no-window`. The same emulator captured a Settings
  screen at 133 KB, so `screencap` itself works. Keep reading the accessibility
  tree, not the PNG.
* Native code resolved to an executable mapping of `base.apk`, never a `.so`
  path — the `extractNativeLibs=false` spelling `own_code_maps` exists for.
* The device secret round-tripped: `shared_prefs/copypaste-device-secret.xml`
  unchanged across launches and the SQLCipher page-1 salt unchanged with it.
