import SwiftUI

/// The empty, loading, offline and error states.
///
/// One view for all of them, because the manifest's rule is about what they
/// *say*: "nothing copied yet", "starting up" and "the service isn't running"
/// are three different situations with three different next steps, and showing
/// an empty list for any of them is the bug (manifest 06 §3.1.11, §3.2.5).
///
/// A loading indicator must be visible. v1 shipped classless empty elements
/// that rendered as nothing at all, which is indistinguishable from a broken
/// layout (CopyPaste-8ebg.29).
struct StateView: View {
    let title: String
    let message: String
    var actionTitle: String?
    var isBusy = false
    var action: () -> Void = {}

    var body: some View {
        VStack(spacing: 8) {
            if isBusy {
                ProgressView()
                    .controlSize(.small)
                    .accessibilityHidden(true)
            }
            Text(title)
                .font(.headline)
            Text(message)
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            if let actionTitle {
                Button(actionTitle, action: action)
                    .buttonStyle(.borderedProminent)
                    .disabled(isBusy)
                    .padding(.top, 2)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(isBusy ? "\(title). \(message). Working." : "\(title). \(message)")
    }
}
