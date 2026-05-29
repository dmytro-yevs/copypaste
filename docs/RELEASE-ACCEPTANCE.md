# Release Acceptance Checklist

**No release of CopyPaste ships until every box below passes on REAL hardware.**
This is the gate. Automated `cargo test`/emulator runs are necessary but NOT sufficient —
they validate source, not the signed artifact on a real device. This list catches the bug
classes that slipped through 0.5.0 / 0.5.1 (build-skew, signing/keychain, notch, OEM, fresh
install, silent failures).

How to use: build the release artifacts, install them the way a real user would, and walk
this list top to bottom. Record date, version, device, and PASS/FAIL per item. A single FAIL
blocks the release.

---

## 0. Pre-flight (orchestrator, before handing to verifier)
- [ ] `git` local `main` == `origin/main` (no stale checkout / build-skew).
- [ ] Release is built from a tagged commit; the tag matches the version in `Cargo.toml`,
      `tauri.conf.json`, and `android/app/build.gradle.kts`.
- [ ] `cargo test --workspace` green (run once, single runner).
- [ ] `scripts/acceptance.sh` green (real-binary daemon + two-process P2P smoke).
- [ ] Asset-name consistency check passes (`scripts/release/check-asset-names.sh`).

## 1. macOS — install & first run
- [ ] Fresh `brew install --cask copypaste` on a machine with NO prior CopyPaste:
      the daemon **auto-starts** (LaunchAgent installed + bootstrapped); menu-bar icon appears.
- [ ] `brew upgrade` from the previous version succeeds even if `/Applications/CopyPaste.app`
      was missing/broken (stuck-state tolerance).
- [ ] App appears in **Cmd+Tab** app switcher.
- [ ] Copy text in any app → it appears in CopyPaste history within ~1s.
- [ ] Open **Settings** → the UI does NOT blank; all panels render.
- [ ] **Keychain prompt appears at most once (ideally zero on ad-hoc file-store).** After
      granting (or with file-store), quit & relaunch the daemon → **no second prompt**.
- [ ] Reinstall/upgrade the app, relaunch → still **no keychain prompt**, history intact.
- [ ] If the keychain/key is unavailable, the daemon stays up and the UI shows a clear
      "needs attention" state — it never silently dies or blanks.

## 2. macOS ↔ macOS (or two daemons) — P2P
- [ ] Pair two devices via QR / password.
- [ ] Copy on A → appears on B within a few seconds. Then copy on B → appears on A
      (BOTH directions).
- [ ] Devices view shows the paired peer.

## 3. Supabase cloud sync
- [ ] One-command cloud setup completes; Settings shows **real** "Signed in ✓"
      (and shows signed-out / error truthfully if auth fails — no fake green).
- [ ] Copy on macOS → appears on a second cloud-linked device (and on Android).
- [ ] Copy on the second device → appears on macOS (both directions; bytea wire format).
- [ ] Existing history older than the latest 20 rows downloads to a freshly linked device
      (watermark pagination, no 20-row cap).

## 4. Android — install & visuals
- [ ] App installs and launches.
- [ ] Header is fully visible on a **notched / punch-hole** phone (portrait AND landscape) —
      not clipped under the status bar / cutout.
- [ ] Overall design looks polished and consistent with the macOS app (dark Darcula style).

## 5. Android — capture & permissions
- [ ] Grant permissions via onboarding; the **OEM autostart / battery** settings screen
      actually opens (or lands on a sane fallback with guidance) — does not dead-end/crash.
- [ ] Copy text (incl. from another app, app backgrounded) → the clip appears in the
      in-app history **without** a manual refresh.

## 6. Android — pairing & sync
- [ ] Opening the **QR** screen does NOT crash.
- [ ] The scanner camera preview is **upright in portrait** (not rotated/landscape).
- [ ] Pair macOS ↔ Android over the same LAN; copy on macOS → appears on Android, and
      copy on Android → appears on macOS (both directions). Decryption succeeds (no 0-items,
      no silent drops).

## 7. Cross-cutting
- [ ] No screen shows a permanent placeholder instead of real content
      (e.g. history must show actual previews, not "(N chars)").
- [ ] No silent failure anywhere: every error surfaces as a visible message/log,
      never a blank screen or a no-op.

---

### Record
| Date | Version | macOS device | Android device | Result | Notes |
|------|---------|--------------|----------------|--------|-------|
|      |         |              |                |        |       |
