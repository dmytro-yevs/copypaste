cask "copypaste" do
  version "0.4.1"
  sha256 "25270e5e5e439059d5285baecb8c8391990c859ecec8c09cc548bc070e115740"

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
    # Strip quarantine (ad-hoc signed builds, no Apple Developer ID)
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/CopyPaste.app"]
    # Load launchd plist if it exists
    plist = Pathname.new("#{Dir.home}/Library/LaunchAgents/com.copypaste.daemon.plist")
    system_command "/bin/launchctl", args: ["load", "-w", plist.to_s] if plist.exist?
  end

  uninstall launchctl: "com.copypaste.daemon"

  zap trash: [
    "~/Library/Application Support/copypaste",
    "~/Library/Caches/com.copypaste.daemon",
    "~/Library/LaunchAgents/com.copypaste.daemon.plist",
    "~/Library/Logs/copypaste",
  ]

  caveats <<~EOS
    CopyPaste uses ad-hoc signing (no Apple Developer ID). Homebrew strips
    the quarantine attribute on install, so you should not see a Gatekeeper
    warning.

    The daemon runs as a LaunchAgent (#{ENV.fetch("USER", "current")} user). Logs at:
      ~/Library/Logs/copypaste/

    First run starts the daemon automatically.

    To stop the daemon WITHOUT disabling it (so it restarts on next login or
    app launch), use `bootout` — do NOT use `launchctl unload`/`-w`, which
    writes a persistent disable override that prevents the daemon from ever
    starting again:
      launchctl bootout gui/$(id -u)/com.copypaste.daemon

    To start it again (or recover from a previously disabled state):
      launchctl enable gui/$(id -u)/com.copypaste.daemon
      launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.copypaste.daemon.plist
  EOS
end
