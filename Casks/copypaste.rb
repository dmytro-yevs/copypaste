# frozen_string_literal: true

cask "copypaste" do
  version "0.3.0"
  sha256 "2cc615619ea71f93b753f05a2ceefb6a51df1c297e6114889d46c7aa2b0d0356"

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

  postflight do
    # Strip quarantine (ad-hoc signed builds, no Apple Developer ID).
    # Must run before any attempt to launch the app or its helpers.
    # The daemon is NOT bootstrapped here — per ADR-014 the app owns the daemon
    # lifecycle: it starts the daemon as a child on launch and stops it on quit.
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/CopyPaste.app"]
  end

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
    CopyPaste uses ad-hoc signing (no Apple Developer ID). Homebrew strips
    the quarantine attribute on install, so you should not see a Gatekeeper
    warning.

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
