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
  # Quarantine
  # ---------------------------------------------------------------------------
  # Homebrew applies com.apple.quarantine to everything it downloads, and the
  # escape hatch is gone: `--no-quarantine` was deprecated in Homebrew 5.1 with
  # no replacement. A Homebrew maintainer's guidance on the deprecation thread
  # (Homebrew discussion #6537) is verbatim:
  #
  #   "Yes, post-processing is required, as it would be if you download and
  #    extract the files using other methods."
  #
  # This block is that post-processing.
  #
  # It names our bundle. It does NOT operate on `#{appdir}` — the widely-copied
  # `xattr -rd com.apple.quarantine /Applications/*` form de-quarantines every
  # app the user has ever downloaded. A Homebrew maintainer objected to exactly
  # that suggestion on the same thread, for the same reason. Doing it here
  # rather than in a README also matters: telling users to run `xattr -rd`
  # teaches a habit that is genuinely dangerous applied anywhere else.
  #
  # Verified against Homebrew's current source (2026-07-30), not assumed:
  # `postflight` is still registered from Cask::DSL via
  # ARTIFACT_BLOCK_CLASSES, and `system_command` is still a public method on
  # Cask::DSL::Base alongside the `appdir` delegator, so both names below
  # resolve. Flight blocks run inside `install_artifacts`, after the `app`
  # stanza has moved the bundle into place — which is the ordering this needs.
  postflight do
    app_path = "#{appdir}/CopyPaste.app"

    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", app_path],
                   print_stderr: false

    # Second step, and a deviation from ADR-0001's snippet — deliberate, and
    # the reason is recorded rather than inferred.
    #
    # v1 shipped this same cask with the hardened runtime enabled on an ad-hoc
    # signature and found that removing the quarantine attribute was not
    # sufficient: the app still would not launch, failing with
    # RBSRequestErrorDomain Code=5 / POSIX 163, which macOS shows the user as
    # "CopyPaste.app can't be opened." Re-sealing the bundle fixed it.
    #
    # v2's build drops the hardened runtime (it is a precondition for
    # notarisation and buys nothing without it), which SHOULD make this
    # unnecessary. "Should" is doing real work in that sentence — nobody on
    # this project has run either build on a Mac. Since the cost of being wrong
    # is an app that cannot be opened at all, the re-seal stays until a real
    # install proves it redundant.
    #
    # It is safe to keep: the app needs no TCC permission, so nothing depends
    # on the code hash staying put (ADR-0001, consequence 1). Not `--deep`,
    # which Apple deprecated and which would re-sign the injected daemon and
    # CLI with no identifier of their own; the outer seal is what changed.
    #
    # /usr/bin/codesign is an Xcode Command Line Tools shim, but Homebrew
    # itself requires the CLT, so it is present wherever this can run.
    system_command "/usr/bin/codesign",
                   args: ["--force", "--sign", "-", "--timestamp=none", app_path],
                   print_stderr: false
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

  zap trash: [
    "~/Library/Application Support/CopyPaste",
    "~/Library/Caches/CopyPaste",
    "~/Library/Logs/CopyPaste",
  ]

  caveats <<~EOS
    CopyPaste is ad-hoc signed and not notarised by Apple — the project has no
    Apple Developer ID. This cask removes the Gatekeeper quarantine attribute
    from CopyPaste.app on install so the app opens normally.

    The app requires no Accessibility or Input Monitoring permission. Choosing
    an item puts it on the clipboard; you press Cmd+V yourself.

    The command-line tool is a separate formula:
      brew install dmytro-yevs/copypaste/copypaste-cli
  EOS
end
