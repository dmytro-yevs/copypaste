import Foundation

/// Where the daemon socket lives.
///
/// This must agree with `copypaste_ipc::socket_path()`, which is
/// `directories::ProjectDirs::from("com", "copypaste", "CopyPaste").data_dir()`
/// joined with `daemon.sock`. On macOS the `directories` crate builds the
/// bundle-identifier-shaped directory name `com.copypaste.CopyPaste` under
/// `~/Library/Application Support`, so the full path is
///
///     ~/Library/Application Support/com.copypaste.CopyPaste/daemon.sock
///
/// Two things are deliberate here:
///
/// * The path is derived from `NSHomeDirectory()` rather than
///   `FileManager.urls(for: .applicationSupportDirectory ...)`. In a sandboxed
///   process the latter answers with the app's container, which is *not* where
///   the daemon listens. This app is not sandboxed (see `App/Info.plist` and
///   the README); resolving through the real home directory means a future
///   sandbox flag produces an obvious "cannot reach the service" rather than a
///   silent connection to nothing.
/// * Nothing in this type is ever rendered. The path spells out the local
///   username, and CLAUDE.md rule 4 keeps that off screens and out of
///   screenshots. `DaemonError` has no case that can carry it.
public enum SocketPath {
    /// The `directories`-crate name for this application on macOS.
    public static let bundleDirectoryName = "com.copypaste.CopyPaste"

    /// `~/Library/Application Support/com.copypaste.CopyPaste`.
    public static var dataDirectory: String {
        (NSHomeDirectory() as NSString)
            .appendingPathComponent("Library/Application Support")
            .appending("/" + bundleDirectoryName)
    }

    /// The daemon's Unix domain socket.
    public static var daemonSocket: String {
        (dataDirectory as NSString).appendingPathComponent("daemon.sock")
    }
}
