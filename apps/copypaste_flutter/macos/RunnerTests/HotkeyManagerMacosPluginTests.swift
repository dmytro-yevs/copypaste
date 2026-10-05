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
    XCTAssertEqual(carbon.registerCalls.first?.options, UInt32(kEventHotKeyExclusive))
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

  func testReleaseFailureRetainsHandleAndPreventsReplacementUntilExplicitRetry() {
    let carbon = FakeCarbonHotKeyRegistrar()
    CarbonHotKeyRegistry.installCarbonRegistrarForTesting(carbon)
    let plugin = HotkeyManagerMacosPlugin()
    XCTAssertEqual(invokeRegister(plugin, identifier: "quick-paste") as? Bool, true)
    carbon.unregisterStatus = OSStatus(paramErr)
    XCTAssertEqual(invokeUnregister(plugin, identifier: "quick-paste")?.code,
                   "hotkey_unregistration_failed")
    XCTAssertEqual((invokeRegister(plugin, identifier: "quick-paste") as? FlutterError)?.code,
                   "hotkey_unregistration_failed")
    XCTAssertEqual(carbon.registerCalls.count, 1)
    XCTAssertEqual(CarbonHotKeyRegistry.activeRegistrationCountForTesting, 1)
    XCTAssertTrue(carbon.unregistered[0] === carbon.unregistered[1])
    carbon.unregisterStatus = noErr
    XCTAssertNil(invokeUnregister(plugin, identifier: "quick-paste"))
    XCTAssertEqual(CarbonHotKeyRegistry.activeRegistrationCountForTesting, 0)
    XCTAssertNil(invokeUnregister(plugin, identifier: "quick-paste"))
    XCTAssertEqual(carbon.unregistered.count, 3)
  }

  func testTwoOwnersDecodeCarbonEventsAndKeepSurvivingOwner() throws {
    let carbon = FakeCarbonHotKeyRegistrar()
    CarbonHotKeyRegistry.installCarbonRegistrarForTesting(carbon)
    var first: HotkeyManagerMacosPlugin? = HotkeyManagerMacosPlugin()
    let second = HotkeyManagerMacosPlugin()
    var firstEvents = 0
    var secondEvents = 0
    _ = first!.onListen(withArguments: nil) { _ in firstEvents += 1 }
    _ = second.onListen(withArguments: nil) { _ in secondEvents += 1 }
    XCTAssertEqual(invokeRegister(first!, identifier: "shared-string") as? Bool, true)
    XCTAssertEqual(invokeRegister(second, identifier: "shared-string") as? Bool, true)
    let firstCall = carbon.registerCalls[0]
    let secondCall = carbon.registerCalls[1]
    XCTAssertNotEqual(firstCall.identifier, secondCall.identifier)
    var event: EventRef?
    XCTAssertEqual(CreateEvent(nil, OSType(kEventClassKeyboard), UInt32(kEventHotKeyPressed),
                              0, 0, &event), noErr)
    let nativeEvent = try XCTUnwrap(event)
    defer { ReleaseEvent(nativeEvent) }
    func send(_ id: UInt32, signature: OSType) -> OSStatus {
      var key = EventHotKeyID(signature: signature, id: id)
      XCTAssertEqual(SetEventParameter(nativeEvent, UInt32(kEventParamDirectObject),
                                      UInt32(typeEventHotKeyID), MemoryLayout<EventHotKeyID>.size,
                                      &key), noErr)
      return CarbonHotKeyRegistry.handle(nativeEvent)
    }
    XCTAssertEqual(send(firstCall.identifier, signature: firstCall.signature), noErr)
    XCTAssertEqual(firstEvents, 1)
    XCTAssertEqual(send(secondCall.identifier, signature: 0), OSStatus(eventNotHandledErr))
    first = nil
    XCTAssertEqual(CarbonHotKeyRegistry.activeRegistrationCountForTesting, 1)
    XCTAssertEqual(send(firstCall.identifier, signature: firstCall.signature), OSStatus(eventNotHandledErr))
    XCTAssertEqual(send(secondCall.identifier, signature: secondCall.signature), noErr)
    XCTAssertEqual(secondEvents, 1)
    _ = second.onCancel(withArguments: nil)
    XCTAssertEqual(send(secondCall.identifier, signature: secondCall.signature), OSStatus(eventNotHandledErr))
    invokeUnregister(second, identifier: "shared-string")
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

  @discardableResult
  private func invokeUnregister(
    _ plugin: HotkeyManagerMacosPlugin,
    identifier: String
  ) -> FlutterError? {
    var error: FlutterError?
    plugin.unregister(
      FlutterMethodCall(
        methodName: "unregister",
        arguments: ["identifier": identifier]
      )
    ) { error = $0 as? FlutterError }
    return error
  }
}

private final class FakeCarbonHotKeyRegistrar: CarbonHotKeyRegistrar {
  struct RegisterCall {
    let keyCode: UInt32
    let modifiers: UInt32
    let signature: OSType
    let identifier: UInt32
    let options: UInt32
  }

  var unregisterStatus: OSStatus = noErr
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
    identifier: UInt32,
    options: UInt32
  ) -> CarbonHotKeyRegistrationAttempt {
    registerCalls.append(RegisterCall(
      keyCode: keyCode,
      modifiers: modifiers,
      signature: signature,
      identifier: identifier,
      options: options
    ))
    return CarbonHotKeyRegistrationAttempt(
      status: registerStatus,
      registration: registerStatus == noErr ? CarbonHotKeyRegistration() : nil
    )
  }

  func unregister(_ registration: CarbonHotKeyRegistration) -> OSStatus {
    unregistered.append(registration)
    return unregisterStatus
  }
}
