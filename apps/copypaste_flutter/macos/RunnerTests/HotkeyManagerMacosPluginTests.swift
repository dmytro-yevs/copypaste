import Carbon
import FlutterMacOS
import XCTest
@testable import hotkey_manager_macos

final class HotkeyManagerMacosPluginTests: XCTestCase {
  override func tearDown() {
    CarbonHotKeyRegistry.resetCarbonRegistrarForTesting()
    super.tearDown()
  }

  func testRegistrationFailureReturnsFlutterErrorWithoutSavingRegistration() {
    let carbon = FakeCarbonHotKeyRegistrar(
      registerStatus: OSStatus(eventHotKeyExistsErr)
    )
    CarbonHotKeyRegistry.installCarbonRegistrarForTesting(carbon)
    let plugin = HotkeyManagerMacosPlugin()

    let result = invokeRegister(plugin, identifier: "quick-paste")

    guard let error = result as? FlutterError else {
      return XCTFail("Expected registration failure to reach Dart as FlutterError.")
    }
    XCTAssertEqual(error.code, "hotkey_registration_failed")
    XCTAssertEqual(carbon.registerCalls.count, 1)
    XCTAssertTrue(carbon.unregistered.isEmpty)
    XCTAssertEqual(CarbonHotKeyRegistry.activeRegistrationCountForTesting, 0)
  }

  func testSuccessfulRegistrationRoutesPressedEventToOwningPlugin() throws {
    let carbon = FakeCarbonHotKeyRegistrar()
    CarbonHotKeyRegistry.installCarbonRegistrarForTesting(carbon)
    let plugin = HotkeyManagerMacosPlugin()
    var receivedEvents: [Any] = []
    XCTAssertNil(plugin.onListen(withArguments: nil) { event in
      if let event {
        receivedEvents.append(event)
      }
    })

    XCTAssertEqual(invokeRegister(plugin, identifier: "quick-paste") as? Bool, true)
    let carbonIdentifier = try XCTUnwrap(
      CarbonHotKeyRegistry.activeHotKeyIdentifierForTesting
    )

    XCTAssertEqual(
      CarbonHotKeyRegistry.routeForTesting(
        hotKeyID: carbonIdentifier,
        eventKind: UInt32(kEventHotKeyPressed)
      ),
      noErr
    )

    let event = try XCTUnwrap(receivedEvents.first as? [String: Any])
    XCTAssertEqual(event["type"] as? String, "onKeyDown")
    let data = try XCTUnwrap(event["data"] as? NSDictionary)
    XCTAssertEqual(data["identifier"] as? String, "quick-paste")

    invokeUnregister(plugin, identifier: "quick-paste")
    XCTAssertEqual(carbon.unregistered.count, 1)
    XCTAssertEqual(CarbonHotKeyRegistry.activeRegistrationCountForTesting, 0)
  }

  func testReplacementAndUnregisterReleaseEveryCarbonRegistration() {
    let carbon = FakeCarbonHotKeyRegistrar()
    CarbonHotKeyRegistry.installCarbonRegistrarForTesting(carbon)
    let plugin = HotkeyManagerMacosPlugin()

    XCTAssertEqual(invokeRegister(plugin, identifier: "quick-paste") as? Bool, true)
    XCTAssertEqual(invokeRegister(plugin, identifier: "quick-paste") as? Bool, true)

    XCTAssertEqual(carbon.registerCalls.count, 2)
    XCTAssertEqual(carbon.unregistered.count, 1)
    XCTAssertEqual(CarbonHotKeyRegistry.activeRegistrationCountForTesting, 1)

    invokeUnregister(plugin, identifier: "quick-paste")

    XCTAssertEqual(carbon.unregistered.count, 2)
    XCTAssertEqual(CarbonHotKeyRegistry.activeRegistrationCountForTesting, 0)
  }

  private func invokeRegister(
    _ plugin: HotkeyManagerMacosPlugin,
    identifier: String
  ) -> Any? {
    var receivedResult: Any?
    plugin.register(
      FlutterMethodCall(
        methodName: "register",
        arguments: [
          "keyCode": UInt32(12),
          "modifiers": ["meta"],
          "identifier": identifier,
        ]
      )
    ) { result in
      receivedResult = result
    }
    return receivedResult
  }

  private func invokeUnregister(
    _ plugin: HotkeyManagerMacosPlugin,
    identifier: String
  ) {
    plugin.unregister(
      FlutterMethodCall(
        methodName: "unregister",
        arguments: ["identifier": identifier]
      )
    ) { _ in }
  }
}

private final class FakeCarbonHotKeyRegistrar: CarbonHotKeyRegistrar {
  struct RegisterCall {
    let keyCode: UInt32
    let modifiers: UInt32
    let signature: OSType
    let identifier: UInt32
  }

  let registerStatus: OSStatus
  var hasEventHandler = false
  var registerCalls: [RegisterCall] = []
  var unregistered: [CarbonHotKeyRegistration] = []

  init(registerStatus: OSStatus = noErr) {
    self.registerStatus = registerStatus
  }

  func installEventHandler() -> OSStatus {
    hasEventHandler = true
    return noErr
  }

  func register(
    keyCode: UInt32,
    modifiers: UInt32,
    signature: OSType,
    identifier: UInt32
  ) -> CarbonHotKeyRegistrationAttempt {
    registerCalls.append(RegisterCall(
      keyCode: keyCode,
      modifiers: modifiers,
      signature: signature,
      identifier: identifier
    ))
    return CarbonHotKeyRegistrationAttempt(
      status: registerStatus,
      registration: registerStatus == noErr ? CarbonHotKeyRegistration() : nil
    )
  }

  func unregister(_ registration: CarbonHotKeyRegistration) -> OSStatus {
    unregistered.append(registration)
    return noErr
  }
}
