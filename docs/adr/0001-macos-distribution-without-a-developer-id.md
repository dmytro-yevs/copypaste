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
| Global hotkey | `KeyboardShortcuts`, which wraps Carbon `RegisterEventHotKey` — the one public global-hotkey API that needs no Accessibility, because the app is told about exactly one combination and never sees other input | `NSEvent.addGlobalMonitorForEvents` or `CGEvent.tapCreate`, both of which require Accessibility |
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

This path is not guaranteed to last. Homebrew has been steadily narrowing
unsigned distribution, and a future release may close `postflight` as well. The
fallback, if that happens, is a source-built formula: a locally compiled binary
is never quarantined, so Gatekeeper is never involved and no attribute needs
removing. The cost is that the user needs the Xcode command line tools and a
long build — SQLCipher compiles from source — which is why it is the fallback
and not the default.

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
