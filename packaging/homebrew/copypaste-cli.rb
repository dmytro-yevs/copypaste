# frozen_string_literal: true

# CopyPaste CLI + daemon — Homebrew Formula.
#
# Like Casks/copypaste.rb, this file is both the checked-in source of truth and
# the template scripts/release/gen-formula.sh rewrites in place. Seeded with
# 0.0.0 and an all-zero sha256 so an unreleased copy fails closed.
#
# A formula rather than a cask because this is a plain pair of binaries, not an
# .app bundle — and because ADR-0001 notes the distinction matters: Homebrew's
# quarantine handling is Cask machinery, so the CLI does not need the postflight
# the cask carries.
#
# The binaries are still ad-hoc signed by the release pipeline. That is not
# about Gatekeeper: macOS on Apple Silicon refuses to execute an entirely
# unsigned Mach-O at all, so ad-hoc signing is the floor, not a nicety.

class CopypasteCli < Formula
  desc "Encrypted clipboard manager — command-line client and daemon"
  homepage "https://github.com/dmytro-yevs/copypaste"
  version "0.0.0"
  license any_of: ["MIT", "Apache-2.0"]

  # arm64 only, matching the cask and the release pipeline. `on_intel` is
  # deliberately absent rather than pointing at a slice we do not build:
  # an Intel install fails here, with this message, instead of at first run.
  on_macos do
    on_arm do
      url "https://github.com/dmytro-yevs/copypaste/releases/download/v#{version}/copypaste-cli-v#{version}-macos-arm64.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    bin.install "copypaste"
    bin.install "copypaste-daemon"
  end

  # The daemon does not fork — README is explicit that backgrounding is the
  # service manager's job. Homebrew's `service` DSL generates and manages the
  # launchd plist, which is why this project ships no plist of its own.
  #
  #   brew services start copypaste-cli
  service do
    run [opt_bin/"copypaste-daemon", "--foreground"]
    keep_alive true
    log_path var/"log/copypaste-daemon.log"
    error_log_path var/"log/copypaste-daemon.log"
  end

  def caveats
    <<~EOS
      The daemon does not fork. Run it under launchd:
        brew services start copypaste-cli

      Or in the foreground:
        copypaste-daemon --foreground

      If you also installed the app (brew install --cask copypaste), run only
      one of them — two daemons would contend for the same socket.
    EOS
  end

  test do
    # Deliberately does not start a daemon: `brew test` runs in a sandbox with
    # no writable data directory, and a test that needs one would fail for
    # reasons that have nothing to do with the formula. `--version` proves the
    # binary was installed, is executable, and passes the ad-hoc signature
    # check macOS applies on first execution — which is the failure this test
    # is actually guarding against.
    assert_match version.to_s, shell_output("#{bin}/copypaste --version")
    system bin/"copypaste-daemon", "--help"
  end
end
