import Foundation
import ServiceManagement

/// Open CopyPaste when the user logs in.
///
/// `SMAppService` is the system API for this on macOS 13+; there is no
/// dependency to add and nothing to hand-roll. The two things worth knowing:
///
/// * it only works from a **bundled, code-signed app**. Running the raw
///   SwiftPM executable throws, which is why `isAvailable` exists and why the
///   Settings toggle explains itself instead of failing silently;
/// * the system, not this app, is the source of truth. The toggle reads
///   `status` every time rather than caching a boolean that the user can
///   change behind our back in System Settings › Login Items.
enum LaunchAtLogin {
    static var isEnabled: Bool {
        SMAppService.mainApp.status == .enabled
    }

    /// False when the app is not running from a bundle, so the UI can say why
    /// the toggle is unavailable rather than appearing to do nothing.
    static var isAvailable: Bool {
        Bundle.main.bundleIdentifier != nil && Bundle.main.bundlePath.hasSuffix(".app")
    }

    /// Answers a sentence to show on failure, or `nil` on success.
    @discardableResult
    static func set(_ enabled: Bool) -> String? {
        do {
            if enabled {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            return nil
        } catch {
            // Deliberately not the underlying error: `SMAppService` errors can
            // quote the bundle path, and a path names the local user
            // (CLAUDE.md rule 4).
            return enabled
                ? "macOS wouldn’t add CopyPaste to your login items. You can add it in System Settings › General › Login Items."
                : "macOS wouldn’t remove CopyPaste from your login items."
        }
    }
}
