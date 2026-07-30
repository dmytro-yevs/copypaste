# ADR-0001 — macOS distribution without an Apple Developer ID

**Status:** accepted · 2026-07-30
**Scope:** how the macOS app is signed, packaged and installed, and what that
forces the app itself to be.

## Decision

CopyPaste for macOS ships **ad-hoc signed**, distributed through **our own
Homebrew tap** rather than `homebrew/cask`, and **requires no TCC permission of
any kind**.

We do not enrol in the Apple Developer Program, so there is no Developer ID
certificate and no notarisation. That is a cost decision, and it propagates
into two technical constraints that are not obvious from the outside. Both are
binding.

## Why this needed a decision at all

The naive reading is that not paying costs a warning dialog on first launch.
That part is true and survivable. The part that is neither is what unsigned
distribution does to *permissions*.

macOS records a TCC grant (Accessibility, Input Monitoring, and friends)
against the identity of the binary that asked for it. For a properly signed
app that identity is the Team ID, which is stable for the life of the
certificate — grant once, and it holds across every future update. An **ad-hoc
signature has no Team ID**, so TCC falls back to the code directory hash.
The cdhash changes on every build.

The consequence, for an unsigned app that needs a permission: **every update
silently revokes it**, and the user must walk back into System Settings and
re-grant it by hand. For a clipboard manager — an app whose entire value is
that it runs unattended in the background — that is not a rough edge, it is a
broken product.

A self-signed certificate produces a *stable* identity and does fix the
revocation problem on the machine that holds the private key. It does not help
distribution: other machines do not trust the leaf, so Gatekeeper treats the
app as it treats any unsigned one, and we would carry the key-management burden
for no user-visible gain.

So the fork was: pay for a Developer ID, or build an app that needs no
permission. We chose the second.

## Consequence 1 — the app must require zero TCC permissions

This is a design constraint on the macOS app, not a packaging detail, and it is
the reason the previous section matters. Three rules follow, and each has a
tempting violation:

| Capability | What we do | What we must not do |
|---|---|---|
| Read the clipboard | Poll `NSPasteboard`, which needs no permission | Install a `CGEvent` tap to notice Cmd+C |
| Global hotkey | Carbon `RegisterEventHotKey` — the one public global-hotkey API that needs no Accessibility, because the app is told about exactly one combination and never sees other input. **Verified**: `tauri-plugin-global-shortcut` → `global-hotkey` 0.8.0 takes this path for every key with a Carbon scancode. See ADR-0002 for the file and line, and for the five media keys that are the exception. | `NSEvent.addGlobalMonitorForEvents` or `CGEvent.tapCreate`, both of which require Accessibility. `global-hotkey` reaches `CGEventTapCreate` for media keys only, which is why the app refuses to bind them |
| Paste back | Put the item on the pasteboard; the user presses Cmd+V | Synthesise Cmd+V with `CGEventPost`, which requires Accessibility |

Two costs we accept in exchange:

- `RegisterEventHotKey` cannot bind modifier-only shortcuts, so the shortcut
  recorder does not offer "double-tap Control" and similar.
- Selecting an item does not paste it. It makes it the clipboard, and the user
  pastes. This is one keystroke worse than the auto-paste behaviour some
  competitors have, and it is the direct price of not needing Accessibility.

The third row is the one to watch. Auto-paste reads like an obvious
improvement, is a handful of lines, and would forfeit every permission-free
property on this page.

**Unrelated, and arriving anyway:** macOS 16 introduces a user-facing alert
when an app reads the general pasteboard programmatically, together with
`NSPasteboard.accessBehavior` to request always-allow. That is a new grant for
every clipboard manager on the platform regardless of how it is signed. It is
not the TCC problem described here and it is not solved by paying Apple.

## Consequence 2 — our own tap, and a quarantine step

`homebrew/cask` is closed to us. Its audit now requires casks to be code-signed
and notarised, and casks failing that check are removed from the core tap
during 2026. A third-party tap has no such audit, so distribution is:

```sh
brew tap dmytro-yevs/copypaste
brew install --cask copypaste     # the .app
brew install copypaste-cli        # the CLI, a normal formula
```

The remaining friction is Gatekeeper's quarantine flag, which Homebrew applies
to anything it downloads. The escape hatch we would have used no longer exists:
**`--no-quarantine` was removed in Homebrew 5.1.** Homebrew's own guidance is
that "post-processing is required, as it would be if you download and extract
the files using other methods".

So the cask carries a `postflight` that strips the attribute from our bundle
and only ours:

```ruby
postflight do
  system_command "/usr/bin/xattr",
                 args: ["-dr", "com.apple.quarantine", "#{appdir}/CopyPaste.app"]
end
```

Two things about that snippet are deliberate. It names our bundle rather than
`#{appdir}`, because the widely-copied recursive form over the whole
Applications folder de-quarantines every app the user has ever downloaded. And
it lives in the cask rather than in instructions we ask the user to paste,
because a README that tells people to run `xattr -rd` teaches a habit that is
genuinely dangerous elsewhere.

### Verified, 2026-07-30

The paragraphs above were originally written from search results. They have
since been checked against Homebrew's own source and issue tracker, because
being wrong about this means users cannot launch the app at all. The findings,
with what was checked:

- **`postflight` still exists and still runs arbitrary Ruby.** `Cask::DSL`
  registers it from `ARTIFACT_BLOCK_CLASSES`
  (`Artifact::PreflightBlock`, `Artifact::PostflightBlock`) on current master.
- **`system_command` is still available inside it**, as a public method on
  `Cask::DSL::Base`, alongside the `appdir` and `staged_path` delegators the
  snippet uses. Both names in the snippet resolve.
- **The ordering is right.** Flight blocks run inside
  `Cask::Installer#install_artifacts`, which happens after `stage` — so the
  bundle is already at `#{appdir}` when `postflight` runs.
- **A third-party tap runs no audit**, and `brew audit` is not invoked on
  install. Nothing checks the cask against the notarisation rule.
- **The `--no-quarantine` guidance is quoted correctly.** The sentence
  attributed to Homebrew above is verbatim from a Homebrew maintainer on
  discussion #6537. On the same thread, a second maintainer objected
  specifically to the `sudo xattr -rd com.apple.quarantine /Applications/*`
  form on the grounds that it trusts every app in the folder — which is the
  same objection this ADR makes, arrived at independently.
- **The homebrew/cask deadline is real and dated.** Casks failing Gatekeeper
  checks are removed from the core tap on **2026-09-01**.

One wording correction: `--no-quarantine` was **deprecated**, not deleted from
the parser. The distinction does not matter — the flag no longer suppresses
quarantine, and there is no replacement — but "removed" invites someone to go
looking for a version where it still worked.

### Two things this ADR did not say, and should

**The snippet may not be sufficient.** v1 shipped this same cask and found that
stripping the attribute was not enough on its own: the app still refused to
launch, with `RBSRequestErrorDomain Code=5` / POSIX 163, shown to the user as
"CopyPaste.app can't be opened." v1's fix was to re-seal the bundle in the same
`postflight` with `codesign --force --sign -`.

The likely cause is that v1 signed with `--options runtime`. The hardened
runtime is a *precondition for notarisation*, so on a bundle we will never
notarise it costs something and buys nothing. v2's build therefore drops it,
which should remove the failure — but the ADR should not have implied the
`xattr` call alone was known to be enough, because it was not. The cask keeps
the re-seal until a real install shows it is redundant. Re-signing is harmless
here for the reason the whole first half of this document exists: the app needs
no TCC grant, so nothing depends on the code hash staying put.

**A hardened runtime and an entitlements file are not wanted.** Stated
explicitly because it is the natural thing to copy from any macOS signing
guide, and because v1 did copy it.

### Why this path may still close

Homebrew has been steadily narrowing unsigned distribution, and there is now a
visible direction of travel: the Cask Cookbook has gained `postflight_steps`
and its siblings — a declarative mini-DSL "stored in the JSON API" that
"avoid[s] loading cask Ruby for common operations" — and states that a phase
"may define either its Ruby flight block or its matching steps block, not
both". Arbitrary Ruby in a cask is now the discouraged form of something with a
declarative replacement, which is usually how a feature ends.

That mini-DSL includes a `run` operation, so the migration target, if flight
blocks are ever restricted, is most likely `postflight_steps` rather than
nothing. **Watch for it**; do not wait to be broken by it.

The fallback beyond that is unchanged: a source-built formula. A locally
compiled binary is never quarantined, so Gatekeeper is never involved and no
attribute needs removing. The cost is that the user needs the Xcode command
line tools and a long build — SQLCipher compiles from source — which is why it
is the fallback and not the default.

## An untested third path: re-sign on the user's machine

Recorded because it would buy back auto-paste for nothing, and because it
falls out of something v1 already shipped without noticing.

v1's cask re-signed the app **on the user's machine at install time**:

```ruby
postflight do
  system_command "/usr/bin/xattr",    args: ["-dr", "com.apple.quarantine", app_path]
  system_command "/usr/bin/codesign", args: ["--force", "--deep", "--sign", "-", app_path]
end
```

That was aimed at Gatekeeper. But re-signing locally is also the shape that
fixes TCC: sign in `postflight` with a **self-signed certificate generated
once on that machine and kept in the user's keychain**, and every later update
is signed by the same certificate. The designated requirement stops moving, so
the grant survives the upgrade — no Apple account, no CI secret, and no
private key ever leaving the machine. TCC is per-machine anyway, so per-machine
identities cost nothing.

**Unresolved, and not answerable from documentation:** whether TCC requires a
trusted certificate chain or only a valid signature matching the stored
`csreq`. Trusting a self-signed root needs admin rights, which is real install
friction if it turns out to be required.

Ten minutes on a Mac settles it: sign with the certificate, grant
Accessibility, rebuild, relaunch, see whether the grant survived. Until then
the app requires no permission and this changes nothing we ship.

Note also that v1 hit the ad-hoc instability and patched around it —
`1bd33bf4`, "eliminate recurring Keychain prompt on ad-hoc installs …
stable codesign identifier" — without connecting it to TCC.

## What would change this

Enrolling in the Apple Developer Program. It would give a stable Team ID
(making TCC grants survive updates, which unblocks auto-paste), pass
`homebrew/cask` audit (removing the tap and the `postflight`), and drop the
first-launch warning. Everything above exists because of one recurring $99.

Note that v0.4.x reached the same conclusion by a different route: its ADR-010
chose ad-hoc signing to keep certificates out of CI. The Gatekeeper problem was
therefore never solved in v1 either — it was only less visible, because v1's
shipping surface was a CLI and CLIs are not quarantined the way `.app` bundles
are.

## Not decided here

Auto-update. Sparkle expects a signed feed, and `brew upgrade` is the update
mechanism for as long as the tap is the distribution channel.
