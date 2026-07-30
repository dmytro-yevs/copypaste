import AppKit

// A menu-bar app, so: no Dock icon, no menu bar of its own, no main window.
// `.accessory` is set before `run()` so the transition never flickers.
//
// This is deliberately AppKit rather than a SwiftUI `App` with `MenuBarExtra`.
// The brief calls for an `NSStatusItem` with a popover, and the behaviours the
// port manifest requires around it — handing focus back to the previous app on
// dismiss, excluding windows from screen capture, a popover that lives longer
// than any one view — are all AppKit-shaped. SwiftUI owns everything inside the
// popover and the windows.
let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
application.setActivationPolicy(.accessory)
application.run()
