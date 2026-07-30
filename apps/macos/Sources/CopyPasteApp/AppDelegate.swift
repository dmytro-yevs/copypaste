import AppKit
import CopyPasteKit
import SwiftUI

/// Owns everything that outlives a view: the IPC client, the stores, the status
/// item, the global hotkey and the two windows.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let client = DaemonClient()
    private let preferences = Preferences()

    private lazy var service = ServiceController(client: client)
    private lazy var history = HistoryStore(client: client)
    private lazy var devices = DevicesStore(client: client)

    private var statusItem: StatusItemController?
    private lazy var devicesWindow = UtilityWindow(
        title: "Devices",
        size: NSSize(width: 560, height: 520),
        preferences: preferences
    ) { [weak self] in
        guard let self else { return AnyView(EmptyView()) }
        return AnyView(
            DevicesScreen(store: self.devices, service: self.service)
                .onAppear { self.devices.start() }
                .onDisappear { self.devices.stop() }
        )
    }

    private lazy var settingsWindow = UtilityWindow(
        title: "CopyPaste Settings",
        size: NSSize(width: 480, height: 430),
        preferences: preferences
    ) { [weak self] in
        guard let self else { return AnyView(EmptyView()) }
        return AnyView(
            SettingsScreen(
                preferences: self.preferences,
                service: self.service,
                openDevices: { self.devicesWindow.show() }
            )
        )
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let controller = StatusItemController(
            history: history,
            service: service,
            preferences: preferences,
            openDevices: { [weak self] in self?.devicesWindow.show() },
            openSettings: { [weak self] in self?.settingsWindow.show() }
        )
        statusItem = controller

        // A registration failure must not take the app down with it
        // (manifest 06 INV-24): the menu-bar item still works.
        GlobalHotkey.onToggle { [weak self] in
            self?.statusItem?.toggle()
        }

        Task { [weak self] in
            guard let self else { return }
            let running = await self.service.refresh()
            if !running, self.preferences.startServiceOnLaunch {
                await self.service.start()
            }
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        // Anything the user deleted inside the undo window is committed rather
        // than quietly forgotten (manifest 06 §3.1.8).
        history.stop()
        devices.stop()
    }

    /// The app has no windows of its own to re-open; the menu-bar item is the
    /// entry point.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        statusItem?.showPopover()
        return false
    }
}
