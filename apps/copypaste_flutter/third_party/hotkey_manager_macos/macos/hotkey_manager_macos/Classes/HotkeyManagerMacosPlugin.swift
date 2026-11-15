import Cocoa
import FlutterMacOS
import Carbon

public class HotkeyManagerMacosPlugin: NSObject, FlutterPlugin, FlutterStreamHandler {
    private var _eventSink: FlutterEventSink?
    private var hotKeyIdentifiers: [String: UInt32] = [:]

    public static func register(with registrar: FlutterPluginRegistrar) {
        let channel = FlutterMethodChannel(name: "dev.leanflutter.plugins/hotkey_manager", binaryMessenger: registrar.messenger)
        let instance = HotkeyManagerMacosPlugin()
        registrar.addMethodCallDelegate(instance, channel: channel)
        let eventChannel = FlutterEventChannel(name: "dev.leanflutter.plugins/hotkey_manager_event", binaryMessenger: registrar.messenger)
        eventChannel.setStreamHandler(instance)
    }

    public func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
        self._eventSink = events
        return nil
    }

    public func onCancel(withArguments arguments: Any?) -> FlutterError? {
        self._eventSink = nil
        return nil
    }

    public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "register":
            register(call, result: result)
        case "unregister":
            unregister(call, result: result)
        case "unregisterAll":
            unregisterAll(call, result: result)
        default:
            result(FlutterMethodNotImplemented)
        }
    }

    public func register(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        guard let args = call.arguments as? [String: Any],
              let keyCode = args["keyCode"] as? UInt32,
              let modifiers = args["modifiers"] as? [String],
              let identifier = args["identifier"] as? String
        else {
            result(FlutterError(
                code: "invalid_hotkey_arguments",
                message: "The hotkey registration request is missing required arguments.",
                details: nil
            ))
            return
        }

        let releaseStatus = unregister(identifier: identifier)
        guard releaseStatus == noErr else {
            result(releaseError(status: releaseStatus, identifier: identifier))
            return
        }

        let carbonModifiers = NSEvent.ModifierFlags(pluginModifiers: modifiers).carbonFlags
        switch CarbonHotKeyRegistry.register(
            owner: self,
            keyCode: keyCode,
            modifiers: carbonModifiers,
            arguments: args as NSDictionary
        ) {
        case .success(let carbonIdentifier):
            hotKeyIdentifiers[identifier] = carbonIdentifier
            result(true)
        case .failure(let status):
            result(FlutterError(
                code: "hotkey_registration_failed",
                message: "macOS rejected the global shortcut registration (OSStatus \(status)).",
                details: ["osStatus": Int(status), "identifier": identifier]
            ))
        }
    }

    public func unregister(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        guard let args = call.arguments as? [String: Any],
              let identifier = args["identifier"] as? String
        else {
            result(FlutterError(
                code: "invalid_hotkey_arguments",
                message: "The hotkey unregister request is missing an identifier.",
                details: nil
            ))
            return
        }

        let status = unregister(identifier: identifier)
        result(status == noErr ? true : releaseError(status: status, identifier: identifier))
    }

    public func unregisterAll(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        let status = unregisterAll()
        result(status == noErr ? true : releaseError(status: status, identifier: nil))
    }

    deinit {
        unregisterAll()
    }

    private func releaseError(status: OSStatus, identifier: String?) -> FlutterError {
        var details: [String: Any] = ["osStatus": Int(status)]
        if let identifier {
            details["identifier"] = identifier
        }
        return FlutterError(
            code: "hotkey_unregistration_failed",
            message: "macOS could not release the global shortcut (OSStatus \(status)).",
            details: details
        )
    }

    @discardableResult
    private func unregister(identifier: String) -> OSStatus {
        guard let carbonIdentifier = hotKeyIdentifiers[identifier] else {
            return noErr
        }
        let status = CarbonHotKeyRegistry.unregister(carbonIdentifier)
        if status == noErr {
            hotKeyIdentifiers.removeValue(forKey: identifier)
        }
        return status
    }

    @discardableResult
    private func unregisterAll() -> OSStatus {
        var firstFailure: OSStatus = noErr
        for identifier in Array(hotKeyIdentifiers.keys) {
            let status = unregister(identifier: identifier)
            if firstFailure == noErr && status != noErr {
                firstFailure = status
            }
        }
        return firstFailure
    }

    fileprivate func emit(type: String, arguments: NSDictionary) -> Bool {
        guard let eventSink = _eventSink else {
            return false
        }
        eventSink([
            "type": type,
            "data": arguments,
        ])
        return true
    }
}

enum CarbonHotKeyRegistry {
    enum RegistrationResult {
        case success(UInt32)
        case failure(OSStatus)
    }

    private final class Registration {
        weak var owner: HotkeyManagerMacosPlugin?
        let eventHotKey: CarbonHotKeyRegistration
        let arguments: NSDictionary

        init(owner: HotkeyManagerMacosPlugin, eventHotKey: CarbonHotKeyRegistration, arguments: NSDictionary) {
            self.owner = owner
            self.eventHotKey = eventHotKey
            self.arguments = arguments
        }
    }

    private static let signature: OSType = 0x4350484B // "CPHK"
    static let eventSpecs = [
        EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed)),
        EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyReleased)),
    ]
    private static var nextIdentifier: UInt32 = 0
    private static var registrations: [UInt32: Registration] = [:]
    private static var carbon: any CarbonHotKeyRegistrar = SystemCarbonHotKeyRegistrar()

    static func register(
        owner: HotkeyManagerMacosPlugin,
        keyCode: UInt32,
        modifiers: UInt32,
        arguments: NSDictionary
    ) -> RegistrationResult {
        let handlerStatus = carbon.installEventHandler()
        guard handlerStatus == noErr else {
            return .failure(handlerStatus)
        }
        guard carbon.hasEventHandler else {
            return .failure(OSStatus(paramErr))
        }

        let identifier = allocateIdentifier()
        let attempt = carbon.register(
            keyCode: keyCode,
            modifiers: modifiers,
            signature: signature,
            identifier: identifier,
            options: UInt32(kEventHotKeyExclusive)
        )
        guard attempt.status == noErr, let eventHotKey = attempt.registration else {
            return .failure(attempt.status == noErr ? OSStatus(paramErr) : attempt.status)
        }

        registrations[identifier] = Registration(
            owner: owner,
            eventHotKey: eventHotKey,
            arguments: arguments
        )
        return .success(identifier)
    }

    static func unregister(_ identifier: UInt32) -> OSStatus {
        guard let registration = registrations[identifier] else {
            return noErr
        }
        let status = carbon.unregister(registration.eventHotKey)
        if status == noErr {
            registrations.removeValue(forKey: identifier)
        }
        return status
    }

    static func handle(_ event: EventRef?) -> OSStatus {
        guard let event else {
            return OSStatus(eventNotHandledErr)
        }

        var hotKeyID = EventHotKeyID()
        let parameterStatus = GetEventParameter(
            event,
            UInt32(kEventParamDirectObject),
            UInt32(typeEventHotKeyID),
            nil,
            MemoryLayout<EventHotKeyID>.size,
            nil,
            &hotKeyID
        )
        guard parameterStatus == noErr, hotKeyID.signature == signature
        else {
            return OSStatus(eventNotHandledErr)
        }

        return route(hotKeyID: hotKeyID.id, eventKind: GetEventKind(event))
    }

    static func installCarbonRegistrarForTesting(_ registrar: any CarbonHotKeyRegistrar) {
        precondition(registrations.isEmpty)
        carbon = registrar
    }

    static func resetCarbonRegistrarForTesting() {
        precondition(registrations.isEmpty)
        carbon = SystemCarbonHotKeyRegistrar()
    }

    static var activeRegistrationCountForTesting: Int {
        registrations.count
    }

    static var activeHotKeyIdentifierForTesting: UInt32? {
        registrations.keys.first
    }

    static func routeForTesting(hotKeyID: UInt32, eventKind: UInt32) -> OSStatus {
        route(hotKeyID: hotKeyID, eventKind: eventKind)
    }

    private static func route(hotKeyID: UInt32, eventKind: UInt32) -> OSStatus {
        guard let registration = registrations[hotKeyID],
              let owner = registration.owner
        else {
            return OSStatus(eventNotHandledErr)
        }

        switch eventKind {
        case UInt32(kEventHotKeyPressed):
            return owner.emit(type: "onKeyDown", arguments: registration.arguments)
                ? noErr
                : OSStatus(eventNotHandledErr)
        case UInt32(kEventHotKeyReleased):
            return owner.emit(type: "onKeyUp", arguments: registration.arguments)
                ? noErr
                : OSStatus(eventNotHandledErr)
        default:
            return OSStatus(eventNotHandledErr)
        }
    }

    private static func allocateIdentifier() -> UInt32 {
        repeat {
            nextIdentifier &+= 1
        } while nextIdentifier == 0 || registrations[nextIdentifier] != nil
        return nextIdentifier
    }
}

final class CarbonHotKeyRegistration {
    fileprivate let eventHotKey: EventHotKeyRef?

    init(eventHotKey: EventHotKeyRef? = nil) {
        self.eventHotKey = eventHotKey
    }
}

struct CarbonHotKeyRegistrationAttempt {
    let status: OSStatus
    let registration: CarbonHotKeyRegistration?
}

protocol CarbonHotKeyRegistrar: AnyObject {
    var hasEventHandler: Bool { get }

    func installEventHandler() -> OSStatus
    func register(
        keyCode: UInt32,
        modifiers: UInt32,
        signature: OSType,
        identifier: UInt32,
        options: UInt32
    ) -> CarbonHotKeyRegistrationAttempt
    func unregister(_ registration: CarbonHotKeyRegistration) -> OSStatus
}

private final class SystemCarbonHotKeyRegistrar: CarbonHotKeyRegistrar {
    private var eventHandler: EventHandlerRef?

    var hasEventHandler: Bool {
        eventHandler != nil
    }

    func installEventHandler() -> OSStatus {
        guard eventHandler == nil else {
            return noErr
        }
        return InstallEventHandler(
            GetEventDispatcherTarget(),
            carbonHotKeyEventHandler,
            CarbonHotKeyRegistry.eventSpecs.count,
            CarbonHotKeyRegistry.eventSpecs,
            nil,
            &eventHandler
        )
    }

    func register(
        keyCode: UInt32,
        modifiers: UInt32,
        signature: OSType,
        identifier: UInt32,
        options: UInt32
    ) -> CarbonHotKeyRegistrationAttempt {
        var eventHotKey: EventHotKeyRef?
        let status = RegisterEventHotKey(
            keyCode,
            modifiers,
            EventHotKeyID(signature: signature, id: identifier),
            GetEventDispatcherTarget(),
            options,
            &eventHotKey
        )
        return CarbonHotKeyRegistrationAttempt(
            status: status,
            registration: eventHotKey.map(CarbonHotKeyRegistration.init)
        )
    }

    func unregister(_ registration: CarbonHotKeyRegistration) -> OSStatus {
        guard let eventHotKey = registration.eventHotKey else {
            return OSStatus(paramErr)
        }
        return UnregisterEventHotKey(eventHotKey)
    }
}

private func carbonHotKeyEventHandler(
    eventHandlerCall: EventHandlerCallRef?,
    event: EventRef?,
    userData: UnsafeMutableRawPointer?
) -> OSStatus {
    CarbonHotKeyRegistry.handle(event)
}
