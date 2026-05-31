# frozen_string_literal: true

cask "copypaste" do
  version "0.5.3"
  sha256 "77c575e54360a593cc6f42b2646f508391e6859e1111ee080cb21690ef5bd426"

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
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/CopyPaste.app"]

    # Install + bootstrap the LaunchAgent so the daemon starts on a fresh
    # install. The app bundle ships a plist template at
    #   CopyPaste.app/Contents/Resources/com.copypaste.daemon.plist
    # with a `/Users/USERNAME` placeholder for the log paths. We copy it to
    # the user's LaunchAgents dir (if absent), substitute the placeholder
    # with the real home directory, then enable + bootstrap it.
    #
    # Use `launchctl bootstrap`/`enable` (macOS 13+) rather than the removed
    # `launchctl load -w`. Everything is `must_succeed: false` so a failure
    # never aborts the postflight and rolls back the installation.
    home  = File.expand_path("~")
    plist = Pathname.new("#{home}/Library/LaunchAgents/com.copypaste.daemon.plist")

    unless plist.exist?
      template = Pathname.new("#{appdir}/CopyPaste.app/Contents/Resources/com.copypaste.daemon.plist")
      if template.exist?
        contents = template.read
        contents = contents.gsub("/Users/USERNAME", home).gsub("$HOME", home)
        plist.dirname.mkpath
        plist.write(contents)
      end
    end

    if plist.exist?
      uid = `id -u`.chomp
      system_command "/bin/launchctl",
                     args:         ["enable", "gui/#{uid}/com.copypaste.daemon"],
                     must_succeed: false
      system_command "/bin/launchctl",
                     args:         ["bootstrap", "gui/#{uid}", plist.to_s],
                     must_succeed: false
    end
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

    The daemon runs as a LaunchAgent (#{ENV.fetch("USER", "current")} user). Logs at:
      ~/Library/Logs/CopyPaste/

    First run starts the daemon automatically.

    To stop the daemon WITHOUT disabling it (so it restarts on next login or
    app launch), use `bootout` — do NOT use `launchctl unload`/`-w`, which
    writes a persistent disable override that prevents the daemon from ever
    starting again:
      launchctl bootout gui/$(id -u)/com.copypaste.daemon

    To start it again (or recover from a previously disabled state):
      launchctl enable gui/$(id -u)/com.copypaste.daemon
      launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.copypaste.daemon.plist

    If a previous upgrade failed and left CopyPaste in a stuck state, recover with:
      brew reinstall --cask --force copypaste
  EOS
end
