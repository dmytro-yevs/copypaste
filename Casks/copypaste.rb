cask "copypaste" do
  version "0.5.1"
  sha256 "dcff72705469dcac0a24a5e9512c6e39f0e074af78cae29d85806403b575621e"

  # DMG filename follows the CI pattern: CopyPaste-v<tag>-macos-arm64.dmg
  # where <tag> already includes the leading 'v', so the prefix becomes 'vv'.
  url "https://github.com/dmytro-yevs/copypaste/releases/download/v#{version}/CopyPaste-vv#{version}-macos-arm64.dmg",
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
                     args: ["enable", "gui/#{uid}/com.copypaste.daemon"],
                     must_succeed: false
      system_command "/bin/launchctl",
                     args: ["bootstrap", "gui/#{uid}", plist.to_s],
                     must_succeed: false
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
