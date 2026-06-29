# frozen_string_literal: true

cask "copypaste" do
  version "0.3.1"
  sha256 "4ce5beff3843f6e9a1b486ff05dda0341d8967fb1265f4c3b546858c13426ec7"

  # DMG filename follows the CI pattern: CopyPaste-v<version>-macos-arm64.dmg
  # where <version> is bare (build-dmg-ci.sh strips any leading 'v'), so the
  # prefix is a single 'v'.
  url "https://github.com/dmytro-yevs/copypaste/releases/download/v#{version}/CopyPaste-v#{version}-macos-arm64.dmg",
      verified: "github.com/dmytro-yevs/copypaste/"
  name "CopyPaste"
  desc "Encrypted clipboard manager with end-to-end sync"
  homepage "https://github.com/dmytro-yevs/copypaste"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates false
  depends_on macos: :sonoma

  app "CopyPaste.app"

  # No postflight quarantine strip needed: the DMG is Developer-ID signed and
  # notarized by Apple (release.yml: xcrun notarytool submit + xcrun stapler staple).
  # Gatekeeper accepts a properly stapled notarization ticket without quarantine
  # removal. xattr -cr on a notarized app would be incorrect and misleading.

  uninstall_preflight do
    # Robustness for the "stuck state" left by an earlier broken upgrade.
    #
    # During `brew upgrade`/`reinstall`, Homebrew uninstalls the old version
    # first by MOVING /Applications/CopyPaste.app back to staging
    # (Cask::Artifact::Moved#move_back). If that target is ABSENT — e.g. a
    # previous failed upgrade already removed it — move_back raises:
    #   "It seems the App source '/Applications/CopyPaste.app' is not there."
    # because a plain upgrade is not run with `force`, and aborts the upgrade.
    #
    # uninstall_preflight runs BEFORE the App artifact's uninstall phase
    # (Homebrew artifact sort order: UninstallPreflightSteps precedes App), so
    # here we recreate a minimal placeholder bundle when the app is missing.
    # move_back then finds a target, backs it up, and deletes it without
    # raising; the new version's `app` stanza installs the real bundle next.
    app_path = "#{appdir}/CopyPaste.app"
    unless File.exist?(app_path)
      # `system_command` is provided by Cask::DSL::Base; bare `opoo`/`ohai`
      # are NOT available in this eval context, so we stay silent here.
      system_command "/bin/mkdir", args: ["-p", "#{app_path}/Contents/MacOS"]
    end
  end

  uninstall launchctl: "com.copypaste.daemon"

  zap trash: [
    "~/Library/Application Support/CopyPaste",
    "~/Library/Caches/CopyPaste",
    "~/Library/LaunchAgents/com.copypaste.daemon.plist",
    "~/Library/Logs/CopyPaste",
  ]

  caveats <<~EOS
    CopyPaste is signed with an Apple Developer ID certificate and notarized by
    Apple. Gatekeeper will accept it without quarantine warnings.

    The daemon is managed by the CopyPaste app (ADR-014): it starts automatically
    when you open the app and stops when you quit. No LaunchAgent is installed.
    Logs at:
      ~/Library/Logs/CopyPaste/

    For headless / CLI-only use (without the desktop app), install the optional
    LaunchAgent via:
      scripts/launchd/install-agent.sh
    or:
      copypaste daemon install
    Note: the LaunchAgent must NOT run at the same time as the desktop app —
    the app will boot it out.

    If a previous upgrade failed and left CopyPaste in a stuck state, recover with:
      brew reinstall --cask --force copypaste
  EOS
end
