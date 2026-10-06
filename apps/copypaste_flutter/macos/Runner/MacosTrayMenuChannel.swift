import Cocoa
import FlutterMacOS

final class MacosTrayMenuChannel {
  private let channel: FlutterMethodChannel

  init(binaryMessenger: FlutterBinaryMessenger) {
    channel = FlutterMethodChannel(
      name: "com.copypaste.app/tray_menu",
      binaryMessenger: binaryMessenger
    )
    channel.setMethodCallHandler { call, result in
      guard call.method == "showImages" else {
        result(FlutterMethodNotImplemented)
        return
      }
      guard
        let arguments = call.arguments as? [String: Any],
        let address = arguments["nativeMenuAddress"] as? Int,
        address > 0,
        let pointer = UnsafeRawPointer(bitPattern: address)
      else {
        result(FlutterError(code: "invalid_arguments", message: nil, details: nil))
        return
      }
      // The Dart host retains the nativeapi menu until this call completes.
      DispatchQueue.main.async {
        let menu = Unmanaged<NSMenu>.fromOpaque(pointer).takeUnretainedValue()
        Self.showImages(in: menu)
        result(nil)
      }
    }
  }

  deinit {
    channel.setMethodCallHandler(nil)
  }

  static func showImages(in menu: NSMenu) {
    // AppKit 27 hides menu images by default. Use its public property through
    // KVC so the application also compiles with SDKs predating macOS 27.
    let setter = NSSelectorFromString("setPreferredImageVisibility:")
    for item in menu.items where item.image != nil && item.responds(to: setter) {
      // NSMenuItemImageVisibilityVisible is 1 in the AppKit API.
      item.setValue(1, forKey: "preferredImageVisibility")
    }
  }
}
