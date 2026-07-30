import Foundation

/// Starting the clipboard service when it is not running.
///
/// The rule this exists for: **an empty list is a lie when the service is
/// down.** If the socket cannot be reached, the interface says so in plain
/// words and offers to start it (manifest 06 §3.1.11, §3.2.5), rather than
/// rendering "Nothing copied yet" over a service that never answered.
///
/// What it will not do is name a path. Neither the search locations nor the one
/// it picked appear in any message: the failure text is the same sentence
/// whether the binary is missing, unreadable or refuses to start
/// (CLAUDE.md rule 4).
public enum ServiceLauncher {
    public enum Failure: Error, Sendable, Equatable {
        /// No `copypaste-daemon` anywhere this app knows to look.
        case notInstalled
        /// It was found and would not start.
        case couldNotStart
        /// It started, and did not answer within the wait.
        case didNotBecomeReady

        public var message: String {
            switch self {
            case .notInstalled:
                "CopyPaste can’t find the clipboard service. Reinstall CopyPaste, or install copypaste-daemon."
            case .couldNotStart:
                "The clipboard service wouldn’t start."
            case .didNotBecomeReady:
                "The clipboard service started but isn’t answering yet."
            }
        }
    }

    /// Where the service might be, most-specific first.
    ///
    /// The bundled copy wins: an app that ships its own service must not be
    /// silently driving a different build that happens to be on `PATH`.
    static func candidates() -> [URL] {
        var found: [URL] = []
        if let bundled = Bundle.main.url(forAuxiliaryExecutable: "copypaste-daemon") {
            found.append(bundled)
        }
        let home = FileManager.default.homeDirectoryForCurrentUser
        found.append(contentsOf: [
            URL(fileURLWithPath: "/opt/homebrew/bin/copypaste-daemon"),
            URL(fileURLWithPath: "/usr/local/bin/copypaste-daemon"),
            home.appendingPathComponent(".cargo/bin/copypaste-daemon"),
        ])
        return found.filter { FileManager.default.isExecutableFile(atPath: $0.path) }
    }

    /// Spawn the service and wait until it answers `status`.
    ///
    /// The wait mirrors v1's tray resync (manifest 06 §3.6): poll at 250 ms and
    /// give up after 30 s, rather than assuming a spawned process is a working
    /// one.
    ///
    /// Note what this is *not*: a service manager. The spawned process is a
    /// child of the app, so quitting CopyPaste takes it down with it. The
    /// service's own documentation is explicit that backgrounding belongs to
    /// launchd; a `LaunchAgent` is the right long-term home and is called out
    /// in the README as unfinished work.
    public static func startAndWait(
        client: DaemonClient,
        pollInterval: Duration = .milliseconds(250),
        giveUpAfter: Duration = .seconds(30)
    ) async throws {
        guard let executable = candidates().first else { throw Failure.notInstalled }

        do {
            let process = Process()
            process.executableURL = executable
            // `--foreground` suppresses the notice the service prints when it
            // expects a service manager. We are the supervisor here, and there
            // is no terminal to detach from.
            process.arguments = ["--foreground"]
            process.standardOutput = FileHandle.nullDevice
            process.standardError = FileHandle.nullDevice
            try process.run()
        } catch {
            throw Failure.couldNotStart
        }

        let deadline = ContinuousClock.now.advanced(by: giveUpAfter)
        while ContinuousClock.now < deadline {
            try? await Task.sleep(for: pollInterval)
            if (try? await client.status()) != nil { return }
        }
        throw Failure.didNotBecomeReady
    }

    /// Whether a service binary is present at all — the difference between
    /// "start it" and "install it" in the offline state's button.
    public static var isInstalled: Bool { !candidates().isEmpty }
}
