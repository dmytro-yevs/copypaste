# ADR-0001 — macOS distribution without an Apple Developer ID

**Status:** accepted · 2026-07-30
**Scope:** how the macOS app is signed, packaged and installed, and what that
forces the app itself to be.

## Decision

CopyPaste for macOS ships **ad-hoc signed**, is **re-signed at install time with
a self-signed certificate generated on the user's own machine**, is distributed
through **our own Homebrew tap** rather than `homebrew/cask`, and **requires no
TCC permission of any kind**.

The install-time re-signing was added on 2026-07-30 and is described in "The
third path", below. It changes nothing a user sees today, because the app asks
for no permission. It exists so that auto-paste — the one feature that would
need Accessibility — stops being blocked on a $99 subscription.

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

**The sentence above about key management turned out to contain the answer.**
The burden it describes is the burden of shipping a key. If the key is generated
on the user's machine and never leaves it, there is no key to manage, and the
stable identity is free. That is "The third path" below, and it is the reason
this ADR now has two decisions in it rather than one.

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

So the cask carries a `postflight`, which now delegates to a script inside the
bundle because it has a second job as well (see "The third path"):

```ruby
postflight do
  app_path = "#{appdir}/CopyPaste.app"
  system_command "/bin/bash", args: ["#{app_path}/Contents/Resources/selfsign.sh", app_path]
end
```

Three things about that are deliberate. The script names our bundle rather than
`#{appdir}`, because the widely-copied recursive form over the whole
Applications folder de-quarantines every app the user has ever downloaded. It
lives in the cask rather than in instructions we ask the user to paste, because
a README that tells people to run `xattr -rd` teaches a habit that is genuinely
dangerous elsewhere. And it is invoked as an argument to `/bin/bash` rather than
executed directly, because the bundle is still quarantined at that moment and
this takes Gatekeeper out of the question.

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
- **`system_command` is must-succeed, and that was missed the first time.** Its
  body is `@command.run!(executable, **options)` — `run!`, not `run`. A non-zero
  exit raises and Homebrew rolls the install back. The cask previously called
  `xattr -dr com.apple.quarantine` unguarded, and `xattr -dr` exits non-zero for
  any file in the tree that does not carry the attribute, which is nearly every
  file in a bundle. That was a latent aborted install. It is now inside a script
  that owns its own exit status.
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
the re-seal, and the question of whether it is still needed is now moot: the
re-seal is how the self-signed certificate gets applied, so it stays regardless
of what the first real install says about `RBSRequestErrorDomain`.

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

Checked against Homebrew's source on 2026-07-30 rather than the Cookbook:
`Cask::DSL` defines `INSTALL_STEP_ARTIFACT_CLASSES` alongside
`ARTIFACT_BLOCK_CLASSES`, and the steps setter calls
`remove_conflicting_flight_blocks` — so the two really are alternatives and
steps win. `Homebrew::InstallSteps::DSL#run` takes `command:`, `args:`, `env:`
and `sudo:`, which is enough to invoke the script the cask now runs. Flight
blocks carry no deprecation on `AbstractFlightBlock` yet. The migration, when it
comes, is a few lines rather than a redesign — because the logic lives in a
script in the bundle and not in the cask.

The fallback beyond that is unchanged: a source-built formula. A locally
compiled binary is never quarantined, so Gatekeeper is never involved and no
attribute needs removing. The cost is that the user needs the Xcode command
line tools and a long build — SQLCipher compiles from source — which is why it
is the fallback and not the default.

## The third path: re-sign on the user's machine

This was recorded as untested on 2026-07-30 and is now built. It falls out of
something v1 already shipped without noticing.

v1's cask re-signed the app **on the user's machine at install time**:

```ruby
postflight do
  system_command "/usr/bin/xattr",    args: ["-dr", "com.apple.quarantine", app_path]
  system_command "/usr/bin/codesign", args: ["--force", "--deep", "--sign", "-", app_path]
end
```

That was aimed at Gatekeeper. But re-signing locally is also the shape that
fixes TCC: sign in `postflight` with a **self-signed certificate generated once
on that machine and kept in a keychain there**, and every later update is signed
by the same certificate. The designated requirement stops moving, so the grant
survives the upgrade — no Apple account, no CI secret, and no private key ever
leaving the machine. TCC is per-machine anyway, so per-machine identities cost
nothing.

Note also that v1 hit the ad-hoc instability and patched around it —
`1bd33bf4`, "eliminate recurring Keychain prompt on ad-hoc installs … stable
codesign identifier" — without connecting it to TCC. The recurring prompt v1
was chasing is the same root cause seen from the other side: a Keychain item's
ACL is keyed on code identity too, and under ad-hoc that identity moves.
A stable certificate fixes both, and v1 fixed neither.

### What is signed, where

- **CI** builds and signs ad-hoc. It holds no certificate and never will; that
  is the premise of this document. Ad-hoc is also the floor rather than a
  choice — macOS on Apple Silicon will not execute an unsigned Mach-O at all.
- **The user's machine** re-signs, from `packaging/macos/selfsign.sh`, which
  ships inside the bundle at `Contents/Resources/selfsign.sh` and is invoked by
  the cask's `postflight`. The script de-quarantines first (removing extended
  attributes can break a seal that covers them), then generates-or-reuses the
  certificate, then signs inner binaries and the bundle.
- **The certificate** is RSA-2048, self-signed, `codeSigning` EKU, ten years,
  in a dedicated keychain under
  `~/Library/Application Support/com.copypaste.CopyPaste/signing` — the app's
  own data directory. Not the unprefixed `CopyPaste` beside it: that is
  v0.4.x's, and CLAUDE.md rule 3 makes it read-only.

### The open question, and how much of it is now answered

The question was: **does TCC require a trusted certificate chain, or only a
valid signature matching the stored `csreq`?** Trusting a self-signed root
system-wide needs admin rights, which would be real install friction.

Most of it is settled, from Apple's own source rather than from search results.
`tccd` itself is closed, but the requirement it stores and the machinery it must
use to evaluate one are both open — `apple-oss-distributions/Security`,
`OSX/libsecurity_codesigning/lib`:

- **What the requirement contains, ad-hoc.** ``SecStaticCode::defaultDesignatedRequirement()``
  in `StaticCode.cpp` branches on `kSecCodeSignatureAdhoc` and returns an `OR`
  chain of `cdhash` terms, one per architecture — and nothing else, not even the
  identifier. This is the primary-source confirmation of the claim at the top of
  this document: under ad-hoc there is nothing stable to key a grant on.
- **What it contains with a certificate.** The same function otherwise hands off
  to `DRMaker::make()` in `drmaker.cpp`, which emits `identifier <ident> and
  <anchor clause>`. For a leaf that is not Apple-anchored it calls
  `nonAppleAnchor()`, which walks up the chain to the last certificate sharing
  the leaf's Organization and emits `Maker::anchor(slot, sha1)`. For a single
  self-signed certificate with an Organization set, the walk ends immediately,
  `slot` becomes `Requirement::anchorCert`, and the requirement is
  **`identifier "…" and certificate root = H"<sha1 of that certificate>"`**.
  A hash pin. Not `anchor trusted`, not `anchor apple generic`.
- **What evaluating it does.** `Requirement::Interpreter::verifyAnchor()` in
  `reqinterp.cpp` is a SHA-1 of the certificate's DER bytes, compared against
  the literal in the requirement. It is reached from `opAnchorHash`, which is a
  different opcode from `opTrustedCert` / `opTrustedCerts` — those are the ones
  that consult the trust settings database, and the generated requirement does
  not contain them. **So the requirement itself neither expresses nor consults
  trust.**
- **Whether validation adds trust on its own.** `CSCommon.h` defines
  `kSecCSCheckTrustedAnchors = 1 << 27`, commented "build certificate chain to
  system trust anchors, not to any self-signed certificate". It is an opt-in
  flag and is not part of the default flags — which is also why `codesign
  --verify` succeeds on a self-signed bundle while `spctl --assess` rejects it.
  Trust is something a caller has to ask for.

That leaves exactly two things that no published source answers, because both
live in closed code:

1. **Does `tccd` ask for it?** Nothing stops a caller passing
   `kSecCSCheckTrustedAnchors`, and `tccd` is not open. The requirement it
   stores cannot demand trust; whether the daemon demands it anyway is not
   visible.
2. **Will `codesign` sign with an untrusted identity at all?** This is the other
   end of the same question and is easy to overlook. `security find-identity -v
   -p codesigning` applies the code-signing policy and may not list an identity
   whose root is in no trust store. The script therefore looks the identity up
   three ways, falling back to the unfiltered listing, and passes the
   certificate's SHA-1 to `codesign -s` rather than a name — but whether
   `codesign` then proceeds is unverified.

**Both are ten minutes on a Mac and neither is a guess worth publishing.** The
procedure is below. It is written so that the answer is a yes/no, recorded here.

### The ten-minute test procedure

Prerequisites: an Apple Silicon Mac, the Xcode command line tools, and two
builds of the app that differ — any code change will do, since the point is that
the cdhash moves. Nothing here needs an Apple account or admin rights.

```sh
# 1. Install build A and let the cask sign it locally.
brew install --cask <tap>/copypaste

# 2. Confirm which signature it actually got. The two outcomes look completely
#    different and this is the step that tells you whether anything below is
#    even being tested.
codesign -d --requirements - -v /Applications/CopyPaste.app
#   self-signed  -> identifier "com.copypaste.app" and certificate root = H"..."
#   ad-hoc       -> cdhash H"..."      (the script fell back; read its output)
security find-identity -v ~/Library/Application\ Support/com.copypaste.CopyPaste/signing/*.keychain-db

# 3. Grant Accessibility by hand:
#    System Settings -> Privacy & Security -> Accessibility -> +CopyPaste.
#    (The shipped app needs no permission, so add it manually. The grant is
#     what is under test, not what uses it.)

# 4. Install build B over the top.
brew upgrade --cask copypaste     # or: brew reinstall --cask copypaste

# 5. Did the grant survive?
sqlite3 ~/Library/Application\ Support/com.apple.TCC/TCC.db \
  "select service, client, auth_value from access where client like '%copypaste%';"
#    auth_value 2 = allowed. Reading TCC.db needs Full Disk Access for Terminal;
#    if you would rather not grant that, just look at the Accessibility list in
#    System Settings — CopyPaste either still has its tick or it does not.
```

Record the answer here:

| Question | Answer | Date / macOS version |
|---|---|---|
| Does `codesign` sign with the untrusted self-signed identity? | **unanswered** | |
| Does the Accessibility grant survive an update? | **unanswered** | |
| If not, does it survive after `security add-trusted-cert` in the **user** domain? | **unanswered** | |

The third row is the one that decides whether this costs the user anything.
Trusting the certificate in the *user* trust domain
(`security add-trusted-cert -p codeSign -k ~/Library/Keychains/login.keychain-db`)
needs the user's own password and **not** admin rights; only `-d`, the admin
domain, does. So even the worst plausible outcome is one password prompt at
first install, not an administrator. That is worth knowing before anyone
concludes the path is closed.

To run the comparison cleanly, the script takes `COPYPASTE_SIGN_MODE=adhoc` to
skip the certificate entirely and `COPYPASTE_SIGN_MODE=selfsigned` to fail
rather than fall back. Repeating steps 1–5 with `adhoc` should lose the grant;
if it does not, the premise of this whole document is wrong and that is a much
more interesting finding.

### What this costs, stated rather than hidden

- **The keychain password is a file.** `set-key-partition-list` is the only way
  to make `codesign` use a key without a GUI prompt, and it needs the
  keychain's password — which for the login keychain is the user's login
  password, which a `postflight` must never ask for. So the script creates its
  own keychain with a password it generates and stores at 0600 next to it. The
  private key is then protected by file permissions rather than by the login
  password. This is a smaller step down than it reads: code already running as
  that user can drive the unlocked login keychain and can rewrite
  `/Applications/CopyPaste.app` outright. The key is imported non-extractable
  and never leaves the machine.
- **`zap` destroys the identity.** `brew zap` removes the signing directory
  along with everything else, which is correct for "remove all traces" and means
  a later reinstall is a different app to macOS. `brew uninstall` does not.
- **Expiry is ten years away and then it is a cliff.** When the certificate
  expires the script regenerates and says so; the grant is lost at that point.
- **Gatekeeper is unaffected.** A self-signed certificate is not a Developer ID.
  The bundle is still not notarised, `spctl` still rejects it, and the
  quarantine attribute still has to be removed. This buys TCC stability and
  nothing else.

### Why this does not reopen auto-paste yet

Consequence 1 stands until the table above has answers in it. The permission-free
design is not a workaround to be discarded the moment a grant looks survivable —
it is also what keeps the app usable for anyone who declines the permission. If
the test passes, auto-paste becomes an *optional* feature that degrades to
today's behaviour when Accessibility is not granted, and it needs its own ADR.

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
