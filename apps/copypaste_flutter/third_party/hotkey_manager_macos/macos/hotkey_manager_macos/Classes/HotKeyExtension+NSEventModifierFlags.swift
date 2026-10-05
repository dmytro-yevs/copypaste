import AppKit
import Carbon

extension NSEvent.ModifierFlags {
    public init(pluginModifiers: [String]) {
        self.init()
        if pluginModifiers.contains("alt") {
            insert(.option)
        }
        if pluginModifiers.contains("capsLock") {
            insert(.capsLock)
        }
        if pluginModifiers.contains("control") {
            insert(.control)
        }
        if pluginModifiers.contains("fn") {
            insert(.function)
        }
        if pluginModifiers.contains("meta") {
            insert(.command)
        }
        if pluginModifiers.contains("shift") {
            insert(.shift)
        }
    }

    var carbonFlags: UInt32 {
        var flags: UInt32 = 0
        if contains(.command) {
            flags |= UInt32(cmdKey)
        }
        if contains(.option) {
            flags |= UInt32(optionKey)
        }
        if contains(.control) {
            flags |= UInt32(controlKey)
        }
        if contains(.shift) {
            flags |= UInt32(shiftKey)
        }
        return flags
    }
}
