import CopyPasteKit
import SwiftUI

/// Settings. Deliberately small: four switches, a shortcut, and the truth about
/// the clipboard service.
///
/// The service section is not decoration. v1 surfaced which pasteboard backend
/// was live so that a demo running against the stub could not be mistaken for
/// the real thing, and the same failure is possible here — a service built for
/// a non-macOS host answers every call happily and captures nothing.
struct SettingsScreen: View {
    @Bindable var preferences: Preferences
    let service: ServiceController
    let openDevices: () -> Void

    @State private var launchAtLogin = LaunchAtLogin.isEnabled
    @State private var launchFailure: String?

    var body: some View {
        Form {
            Section("General") {
                Toggle("Open CopyPaste at login", isOn: $launchAtLogin)
                    .disabled(!LaunchAtLogin.isAvailable)
                    .onChange(of: launchAtLogin) { _, enabled in
                        launchFailure = LaunchAtLogin.set(enabled)
                        // The system is the source of truth, not this toggle.
                        launchAtLogin = LaunchAtLogin.isEnabled
                    }
                if !LaunchAtLogin.isAvailable {
                    Text("Available once CopyPaste is running from an app bundle.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                if let launchFailure {
                    Text(launchFailure)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .fixedSize(horizontal: false, vertical: true)
                }

                GlobalHotkey.RecorderField()

                Toggle("Start the clipboard service when CopyPaste opens", isOn: $preferences.startServiceOnLaunch)
            }

            Section("Appearance") {
                Picker("Preview lines", selection: $preferences.previewLines) {
                    ForEach(Preferences.previewLineRange, id: \.self) { count in
                        Text(count == 1 ? "1 line" : "\(count) lines").tag(count)
                    }
                }
            }

            Section("Privacy") {
                Toggle("Allow CopyPaste windows in screenshots", isOn: $preferences.allowScreenshots)
                Text(preferences.allowScreenshots
                    ? "CopyPaste windows will appear in screen recordings and screenshots."
                    : "CopyPaste windows are hidden from screen recordings and screenshots.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                Text("CopyPaste asks for no system permissions. It never records what you type and never presses ⌘V for you — select an item and paste it yourself.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Section("Clipboard service") {
                if let status = service.status {
                    LabeledContent("Version", value: status.version)
                    LabeledContent("Stored items", value: "\(status.itemCount)")
                    LabeledContent("Capturing", value: status.captureRunning ? "Yes" : "No")
                    if service.isUsingFakeClipboard {
                        Text("This service is running against a stub pasteboard (“\(status.clipboardBackend)”), so nothing is really being captured.")
                            .font(.caption)
                            .foregroundStyle(.orange)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    if service.hasProtocolMismatch {
                        Text("CopyPaste and the clipboard service are different versions. Update both, then restart the service.")
                            .font(.caption)
                            .foregroundStyle(.red)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                } else {
                    LabeledContent("Status", value: "Not running")
                    Button("Start the service") {
                        Task { await service.start() }
                    }
                    .disabled(service.isStarting || !service.isInstalled)
                }
                if let failure = service.failure {
                    Text(failure)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Button("Devices…", action: openDevices)
            }

            Section("About") {
                LabeledContent("CopyPaste", value: Self.appVersion)
                LabeledContent("IPC protocol", value: "v\(protocolVersion)")
            }
        }
        .formStyle(.grouped)
        .frame(minWidth: 420, minHeight: 360)
        .task {
            launchAtLogin = LaunchAtLogin.isEnabled
            await service.refresh()
        }
    }

    private static var appVersion: String {
        let info = Bundle.main.infoDictionary
        let short = info?["CFBundleShortVersionString"] as? String
        return short ?? "development build"
    }
}
