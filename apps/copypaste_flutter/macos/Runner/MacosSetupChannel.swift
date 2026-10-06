import FlutterMacOS
import os.log
import ServiceManagement

private let macosSetupChannelName = "com.copypaste.app/macos_setup"
private let macosSetupLogger = Logger(
  subsystem: Bundle.main.bundleIdentifier ?? "com.copypaste.app",
  category: "macos-setup"
)

final class MacosSetupChannel {
  private let channel: FlutterMethodChannel

  init(binaryMessenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(
      name: macosSetupChannelName,
      binaryMessenger: binaryMessenger
    )
    channel.setMethodCallHandler(handle)
  }

  deinit {
    channel.setMethodCallHandler(nil)
  }

  private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "accessibilityGranted":
      result(MacosAccessibility.isTrusted(prompt: false))
    case "requestAccessibility":
      result(MacosAccessibility.isTrusted(prompt: true))
    case "loginItemStatus":
      result(loginItemStatus())
    case "setLaunchAtLogin":
      guard
        let arguments = call.arguments as? [String: Any],
        let enabled = arguments["enabled"] as? Bool
      else {
        result(FlutterError(code: "invalid_arguments", message: nil, details: nil))
        return
      }
      setLaunchAtLogin(enabled, result: result)
    case "openLoginItemsSettings":
      guard #available(macOS 13.0, *) else {
        result(
          FlutterError(
            code: "login_items_unavailable",
            message: "Login Items settings are unavailable on this macOS version.",
            details: nil
          )
        )
        return
      }
      SMAppService.openSystemSettingsLoginItems()
      result(nil)
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  private func loginItemStatus() -> String {
    if isDevelopmentBuild {
      return "development_unavailable"
    }
    guard #available(macOS 13.0, *) else {
      return "unavailable"
    }
    return statusName(SMAppService.mainApp.status)
  }

  private func setLaunchAtLogin(_ enabled: Bool, result: @escaping FlutterResult) {
    if isDevelopmentBuild {
      result("development_unavailable")
      return
    }
    guard #available(macOS 13.0, *) else {
      result("unavailable")
      return
    }
    let service = SMAppService.mainApp
    do {
      if enabled {
        if service.status == .notRegistered || service.status == .notFound {
          try service.register()
        }
      } else if service.status == .enabled || service.status == .requiresApproval {
        try service.unregister()
      }
      result(statusName(service.status))
    } catch {
      let error = error as NSError
      macosSetupLogger.error(
        "Start at login update failed: \(error.domain, privacy: .public) / \(error.code)"
      )
      if service.status == .requiresApproval {
        result("requires_approval")
        return
      }
      result(
        FlutterError(
          code: "login_item_update_failed",
          message: "CopyPaste could not update Start at login.",
          details: nil
        )
      )
    }
  }

  private var isDevelopmentBuild: Bool {
    // `flutter run` produces an ad-hoc `.dev` bundle in the build directory.
    // Only the installed, locally re-signed app owns a stable Login Item.
    Bundle.main.bundleIdentifier?.hasSuffix(".dev") == true
  }

  @available(macOS 13.0, *)
  private func statusName(_ status: SMAppService.Status) -> String {
    switch status {
    case .notRegistered:
      return "not_registered"
    case .enabled:
      return "enabled"
    case .requiresApproval:
      return "requires_approval"
    case .notFound:
      return "not_found"
    @unknown default:
      return "unavailable"
    }
  }
}
