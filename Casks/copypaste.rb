cask "copypaste" do
  version "0.3.2"
  sha256 "f10eb5f0235422b541d60ab974df50f2ef9151d8d28f1b60de7d9e69ff1034fb"

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

  uninstall launchctl: "com.copypaste.daemon",
            delete:    "#{appdir}/CopyPaste.app"

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

    First run starts the daemon automatically. To stop:
      launchctl unload ~/Library/LaunchAgents/com.copypaste.daemon.plist
  EOS
end
