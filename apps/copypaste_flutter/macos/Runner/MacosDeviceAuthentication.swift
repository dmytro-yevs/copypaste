import FlutterMacOS
import LocalAuthentication

/// Verifies the device owner with Touch ID or the account password for one engine.
final class MacosDeviceAuthentication {
  static let channelName = "com.copypaste.app/device_auth"

  private let channel: FlutterMethodChannel
  private var pending: LAContext?

  init(binaryMessenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(name: Self.channelName, binaryMessenger: binaryMessenger)
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self else { return result(FlutterMethodNotImplemented) }
      self.handle(call, result: result)
    }
  }

  deinit {
    pending?.invalidate()
    channel.setMethodCallHandler(nil)
  }

  private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "availability":
      var error: NSError?
      let ready = LAContext().canEvaluatePolicy(.deviceOwnerAuthentication, error: &error)
      result(ready ? "available" : Self.availability(of: error))
    case "authenticate":
      guard let reason = (call.arguments as? [String: Any])?["reason"] as? String,
        !reason.isEmpty
      else {
        result(FlutterError(code: "invalid_arguments", message: nil, details: nil))
        return
      }
      guard pending == nil else {
        result(FlutterError(code: "busy", message: nil, details: nil))
        return
      }
      let context = LAContext()
      pending = context
      context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason) {
        [weak self] success, error in
        DispatchQueue.main.async {
          // A released adapter belongs to a stopped engine that cannot take a reply.
          guard let self, self.pending === context else { return }
          self.pending = nil
          result(success ? "success" : Self.outcome(of: error))
        }
      }
    default:
      result(FlutterMethodNotImplemented)
    }
  }

  static func availability(of error: Error?) -> String {
    guard let code = (error as? LAError)?.code else { return "unsupported" }
    switch code {
    case .passcodeNotSet: return "notConfigured"
    case .biometryLockout: return "available"
    default: return "unsupported"
    }
  }

  static func outcome(of error: Error?) -> String {
    guard let code = (error as? LAError)?.code else { return "cancelled" }
    switch code {
    case .biometryLockout: return "lockedOut"
    case .passcodeNotSet, .biometryNotAvailable, .biometryNotEnrolled, .notInteractive:
      return "unavailable"
    default: return "cancelled"
    }
  }
}
