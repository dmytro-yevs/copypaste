import AppKit

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
}
