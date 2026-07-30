import AppKit
import SwiftUI

/// A real `NSSearchField`, wrapped.
///
/// Not a `TextField` with a magnifier drawn next to it: the system control
/// brings the clear button, the right focus ring, the standard context menu,
/// correct behaviour under increased contrast and reduced transparency, and the
/// accessibility role AT users expect from a search box — all things a
/// hand-drawn imitation gets subtly wrong.
///
/// It also forwards the keys that belong to the list. In a popover the text
/// field holds focus the whole time (that is what makes type-to-filter feel
/// instant), so ↑/↓/Return/Escape have to be handed on rather than swallowed.
/// This is the same arrangement Spotlight and every launcher uses, and it is
/// why the manifest's rule about attaching the key handler to the popup root
/// rather than the input (§3.5.3) is satisfied here by forwarding instead.
struct SearchField: NSViewRepresentable {
    @Binding var text: String
    var placeholder: String = "Search"
    /// −1 for up, +1 for down.
    var onMove: (Int) -> Void = { _ in }
    var onSubmit: () -> Void = {}
    var onCancel: () -> Void = {}
    /// Take keyboard focus shortly after appearing.
    var focusOnAppear = true

    func makeNSView(context: Context) -> NSSearchField {
        let field = NSSearchField()
        field.delegate = context.coordinator
        field.placeholderString = placeholder
        field.sendsSearchStringImmediately = true
        field.sendsWholeSearchString = false
        field.focusRingType = .default
        field.setAccessibilityLabel("Search clipboard history")
        return field
    }

    func updateNSView(_ field: NSSearchField, context: Context) {
        context.coordinator.parent = self
        if field.stringValue != text {
            field.stringValue = text
        }
        guard focusOnAppear, !context.coordinator.hasFocused, field.window != nil else { return }
        context.coordinator.hasFocused = true
        // A short hop: native activation and the first SwiftUI layout pass are
        // not synchronous, and focusing too early silently no-ops
        // (manifest 06 §5.3, popup focus delay).
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak field] in
            guard let field else { return }
            field.window?.makeFirstResponder(field)
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    @MainActor
    final class Coordinator: NSObject, NSSearchFieldDelegate {
        var parent: SearchField
        var hasFocused = false

        init(_ parent: SearchField) {
            self.parent = parent
        }

        func controlTextDidChange(_ notification: Notification) {
            guard let field = notification.object as? NSSearchField else { return }
            parent.text = field.stringValue
        }

        func control(
            _ control: NSControl,
            textView: NSTextView,
            doCommandBy selector: Selector
        ) -> Bool {
            switch selector {
            case #selector(NSResponder.moveUp(_:)):
                parent.onMove(-1)
                return true
            case #selector(NSResponder.moveDown(_:)):
                parent.onMove(1)
                return true
            case #selector(NSResponder.insertNewline(_:)):
                parent.onSubmit()
                return true
            case #selector(NSResponder.cancelOperation(_:)):
                // Escape clears a non-empty query first, and only closes the
                // popover when there is nothing left to clear.
                if parent.text.isEmpty {
                    parent.onCancel()
                } else {
                    parent.text = ""
                    control.stringValue = ""
                }
                return true
            default:
                return false
            }
        }
    }
}
