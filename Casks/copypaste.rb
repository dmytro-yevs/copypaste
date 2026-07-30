# frozen_string_literal: true

# CopyPaste — Homebrew Cask.
#
# This file is BOTH the checked-in source of truth and the template that
# scripts/release/gen-cask.sh rewrites: the release pipeline replaces the
# `version` and `sha256` lines in place and copies the result into the tap.
# There is deliberately no second copy to drift out of sync.
#
# The seeded version below is 0.0.0 with an all-zero sha256, so an unreleased
# copy of this file cannot install anything: the URL 404s, and if it somehow
# resolved, checksum verification fails closed rather than being skipped.
#
# Distribution rationale is ADR-0001. The short version: no Apple Developer ID,
# so the app is ad-hoc signed; homebrew/cask is closed to us because its audit
# now requires notarisation and casks failing it are removed on 2026-09-01; so
# this lives in our own tap, where no such audit runs.

cask "copypaste" do
  version "0.0.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/dmytro-yevs/copypaste/releases/download/v#{version}/CopyPaste-v#{version}-macos-arm64.dmg",
      verified: "github.com/dmytro-yevs/copypaste/"
  name "CopyPaste"
  desc "Encrypted clipboard manager with local history and peer sync"
  homepage "https://github.com/dmytro-yevs/copypaste"

  livecheck do
    url :url
    strategy :github_latest
  end

  # arm64 only, and stated rather than implied. The release builds a single
  # aarch64-apple-darwin slice — not a universal binary — so without this an
  # Intel Mac would install a bundle it cannot execute and fail at launch
  # instead of at install, which is the more confusing of the two.
  depends_on arch: :arm64
  depends_on macos: ">= :sonoma"

  # `brew upgrade` is the update mechanism. ADR-0001 leaves auto-update
  # undecided precisely because Sparkle expects a signed feed.
  auto_updates false

  app "CopyPaste.app"

  # ---------------------------------------------------------------------------
  # Quarantine, and the signature
  # ---------------------------------------------------------------------------
  # Two jobs, both delegated to one script that ships inside the bundle:
  # Contents/Resources/selfsign.sh, checked in at packaging/macos/selfsign.sh.
  #
  # 1. Homebrew applies com.apple.quarantine to everything it downloads, and the
  #    escape hatch is gone: `--no-quarantine` was deprecated in Homebrew 5.1
  #    with no replacement. A Homebrew maintainer's guidance on the deprecation
  #    thread (Homebrew discussion #6537) is verbatim:
  #
  #      "Yes, post-processing is required, as it would be if you download and
  #       extract the files using other methods."
  #
  #    The script names our bundle and only ours. The widely-copied
  #    `xattr -rd com.apple.quarantine /Applications/*` form de-quarantines
  #    every app the user has ever downloaded, and a Homebrew maintainer
  #    objected to exactly that on the same thread. Doing it here rather than in
  #    a README also matters: telling users to run `xattr -rd` teaches a habit
  #    that is genuinely dangerous applied anywhere else.
  #
  # 2. It re-signs the bundle. v1 did this too, with `--sign -`, to work around
  #    a quarantined hardened-runtime ad-hoc bundle refusing to launch. v2 signs
  #    with a self-signed certificate generated once on this machine instead,
  #    because that is what makes a TCC grant survive an update — see ADR-0001,
  #    which also carries the test procedure for the one thing about it that
  #    documentation does not settle. The script falls back to `--sign -` if the
  #    certificate path fails for any reason, so the outcome is never worse than
  #    what v1 shipped.
  #
  # Why a script in the bundle rather than commands here. The logic has real
  # failure handling in it — roughly two hundred lines — and inlining that into
  # a cask would put it beyond review, duplicate it into the tap, and make it
  # untestable. It is copied in before the release build signs the bundle, so
  # the seal covers it.
  #
  # Why `/bin/bash <script>` rather than executing it directly: the bundle is
  # still quarantined at this point, and invoking the interpreter explicitly
  # takes Gatekeeper out of the question entirely.
  #
  # Verified against Homebrew's current source (2026-07-30), not assumed:
  # `postflight` is still registered from Cask::DSL via ARTIFACT_BLOCK_CLASSES;
  # `system_command` is still a public method on Cask::DSL::Base alongside the
  # `appdir` delegator; flight blocks run inside `install_artifacts`, after the
  # `app` stanza has moved the bundle into place, which is the ordering this
  # needs. One correction to what was assumed before: `system_command` is
  # `SystemCommand.run!`, which is must-succeed — a non-zero exit raises and
  # rolls the install back. That is why the script owns its own fallbacks and
  # exits non-zero only when the app would genuinely not open, and why the
  # unguarded `xattr -dr` this block used to run was a latent abort (it exits
  # non-zero for any file in the tree that has no such attribute).
  postflight do
    app_path = "#{appdir}/CopyPaste.app"
    selfsign = "#{app_path}/Contents/Resources/selfsign.sh"

    if File.exist?(selfsign)
      system_command "/bin/bash", args: [selfsign, app_path]
    else
      # A DMG built before the script existed. Do what v1 did, and keep the
      # `|| true` on the xattr call for the must-succeed reason above.
      system_command "/bin/bash",
                     args: ["-c",
                            "/usr/bin/xattr -dr com.apple.quarantine \"$1\" 2>/dev/null || true; " \
                            "exec /usr/bin/codesign --force --sign - --timestamp=none \"$1\"",
                            "--", app_path]
    end
  end

  # Carried from v1, where it fixed an observed production failure.
  #
  # On `brew upgrade`/`reinstall`, Homebrew uninstalls the old version by
  # MOVING /Applications/CopyPaste.app back to staging. If that path is already
  # gone — which is what an earlier failed upgrade leaves behind — the move
  # raises "It seems the App source '/Applications/CopyPaste.app' is not
  # there." and aborts the upgrade, leaving the user stuck in the same broken
  # state that caused it.
  #
  # uninstall_preflight runs before the App artifact's uninstall phase, so
  # putting a minimal placeholder there gives the move something to find. It
  # then backs it up, deletes it, and the new version installs over the top.
  #
  # This looks gratuitous. It is not: it is the difference between a bad
  # upgrade being self-healing and needing `brew reinstall --force` typed by
  # hand (CLAUDE.md rule 2).
  uninstall_preflight do
    app_path = "#{appdir}/CopyPaste.app"
    unless File.exist?(app_path)
      system_command "/bin/mkdir", args: ["-p", "#{app_path}/Contents/MacOS"]
    end
  end

  # The signing keychain and its password live under
  # ~/Library/Application Support/CopyPaste/signing, so this already removes
  # them. That is the right semantics for `zap` — remove every trace — and it is
  # worth knowing that it is not free: a later reinstall generates a new
  # certificate, macOS sees a different app, and any permission has to be
  # granted again. `brew uninstall` alone leaves the certificate in place, which
  # is what makes uninstall-then-reinstall keep a grant.
  zap trash: [
    "~/Library/Application Support/CopyPaste",
    "~/Library/Caches/CopyPaste",
    "~/Library/Logs/CopyPaste",
  ]

  caveats <<~EOS
    CopyPaste is not notarised by Apple — the project has no Apple Developer ID.
    On install this cask removes the Gatekeeper quarantine attribute from
    CopyPaste.app and re-signs it with a certificate generated on this machine,
    which stays in your keychain and is never sent anywhere. Updates are signed
    by the same certificate, so macOS keeps treating it as the same app.

    The app requires no Accessibility or Input Monitoring permission. Choosing
    an item puts it on the clipboard; you press Cmd+V yourself.

    The command-line tool is a separate formula:
      brew install dmytro-yevs/copypaste/copypaste-cli
  EOS
end
