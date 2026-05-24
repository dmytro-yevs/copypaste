cask "copypaste" do
  version "0.3.1"
  sha256 "5d6dd523de9db2c70e34ae8e165a3d9b8ab4cc6221467e7f380335308aa5456d"

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
    # must_succeed: false — xattr exits non-zero on certain FS configurations;
    # aborting the install over this non-critical op caused the missing-app
    # regression on upgrade (v0.3.1 → v0.3.2): postflight aborted, Homebrew
    # marked the install failed, but the old app was already deleted by the
    # uninstall phase — leaving /Applications/CopyPaste.app absent.
    system_command "/usr/bin/xattr",
                   args: ["-cr", "#{appdir}/CopyPaste.app"],
                   must_succeed: false
    # Load launchd plist if it exists (no-op on upgrade if already loaded)
    plist = Pathname.new("#{Dir.home}/Library/LaunchAgents/com.copypaste.daemon.plist")
    system_command "/bin/launchctl", args: ["load", "-w", plist.to_s],
                   must_succeed: false if plist.exist?
  end

  # IMPORTANT: do NOT add `delete: "#{appdir}/CopyPaste.app"` here.
  # Homebrew already removes artifacts tracked by the `app` stanza on uninstall.
  # The redundant explicit delete in v0.3.1 caused the upgrade regression:
  # uninstall removed the old .app, then postflight aborted (xattr non-zero),
  # leaving no .app in /Applications.
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

    First run starts the daemon automatically. To stop:
      launchctl unload ~/Library/LaunchAgents/com.copypaste.daemon.plist
  EOS
end
