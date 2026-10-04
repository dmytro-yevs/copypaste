# frozen_string_literal: true

# CopyPaste — Homebrew Cask.
#
# This file is BOTH the checked-in source of truth and the template that
# scripts/release/gen-cask.sh rewrites: the release pipeline replaces the
# `version` and `sha256` lines in place and copies the result into the tap.
# There is deliberately no second copy to drift out of sync.
#
# The seeded version below is 1.0.0 with an all-zero sha256, so an unreleased
# copy of this file cannot install anything: the URL 404s, and if it somehow
# resolved, checksum verification fails closed rather than being skipped.
#
# Distribution rationale is ADR-0001. The short version: no Apple Developer ID,
# so the app is ad-hoc signed; homebrew/cask is closed to us because its audit
# now requires notarisation and casks failing it are removed on 2026-09-01; so
# this lives in our own tap, where no such audit runs.

cask "copypaste" do
  version "1.0.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/dmytro-yevs/copypaste/releases/download/v#{version}/CopyPaste-v#{version}-macos-arm64.dmg"
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
  depends_on macos: :sonoma

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
  # 2. It re-signs the bundle with a self-signed certificate generated once on
  #    this machine. That makes a TCC grant survive an update; the script falls
  #    back to `--sign -` if the certificate path fails.
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
  # Homebrew 7 uses declarative steps. The signing helper also needs access
  # to the app's own support directory for its persistent local keychain.
  # A failing run aborts installation, preserving the previous contract.
  postflight_steps do
    if_path_exists "CopyPaste.app/Contents/Resources/selfsign.sh", base: :appdir do
      run "/bin/bash",
          args: ["{{appdir}}/CopyPaste.app/Contents/Resources/selfsign.sh", "{{appdir}}/CopyPaste.app"],
          writable_paths: ["~/Library/Application Support/com.copypaste.CopyPaste"]
    end
    unless_path_exists "CopyPaste.app/Contents/Resources/selfsign.sh", base: :appdir do
      run "/bin/bash",
          args: ["-c",
                 "/usr/bin/xattr -dr com.apple.quarantine \"$1\" 2>/dev/null || true; " \
                 "exec /usr/bin/codesign --force --sign - --timestamp=none \"$1\"",
                 "--", "{{appdir}}/CopyPaste.app"]
    end
  end

  # On `brew upgrade`/`reinstall`, Homebrew uninstalls the old version by
  # MOVING /Applications/CopyPaste.app back to staging. If that path is already
  # gone — which is what an earlier failed upgrade leaves behind — the move
  # raises "It seems the App source '/Applications/CopyPaste.app' is not
  # there." and aborts the upgrade, leaving the user stuck in the same broken
  # state that caused it.
  #
  # uninstall_preflight_steps runs before the App artifact's uninstall phase, so
  # putting a minimal placeholder there gives the move something to find. It
  # then backs it up, deletes it, and the new version installs over the top.
  #
  # This looks gratuitous. It is not: it is the difference between a bad
  # upgrade being self-healing and needing `brew reinstall --force` typed by
  # hand (AGENTS.md rule 2).
  uninstall_preflight_steps do
    unless_path_exists "CopyPaste.app", base: :appdir do
      mkdir_p "CopyPaste.app/Contents/MacOS", base: :appdir
    end
  end

  # Product data and the established signing identity live in separate support
  # roots. `zap` removes both, which is what an explicit full reset means. It is
  # not free: a later reinstall generates a new certificate, macOS
  # sees a different app, and any permission has to be granted again. `brew
  # uninstall` leaves the certificate, which is what makes uninstall-then-
  # reinstall keep a grant.
  #
  zap trash: [
    "~/Library/Application Support/com.copypaste.app",
    "~/Library/Application Support/com.copypaste.CopyPaste",
  ]

  caveats <<~EOS
    CopyPaste is not notarised by Apple — the project has no Apple Developer ID.
    On install this cask removes the Gatekeeper quarantine attribute from
    CopyPaste.app and re-signs it with a certificate generated on this machine,
    which stays in your keychain and is never sent anywhere. Updates are signed
    by the same certificate, so macOS keeps treating it as the same app.

    CopyPaste requests Accessibility only when auto-paste is enabled. Clipboard
    capture, history, pairing, and ordinary copy continue to work without it.

    The command-line tool is a separate formula:
      brew install dmytro-yevs/copypaste/copypaste-cli
  EOS
end
