import CopyPasteKit
import SwiftUI

/// One clipboard entry.
///
/// Three requirements from manifest 06 are visible in this file:
///
/// * **INV-10 / A11Y-3 — a sensitive item never renders its content.** There is
///   no blur here and no masking: `ClipItem.preview` is `nil` for a sensitive
///   item, so there is no string to render, to announce, or to leak into a
///   screenshot. The redaction is a property of the data, not of a modifier
///   someone could remove.
/// * **INV-5 — the row reserves its full height.** The frame is the whole
///   preview-line cap whatever the content turns out to be, so a two-line clip
///   next to a one-line clip cannot make rows overlap, and the scroll offset
///   cannot shift when text reflows at a different width.
/// * **INV-8 — the row is not a single button.** It carries interactive
///   children (pin, delete); making the row itself a control would flatten
///   them out of the accessibility tree. The row is a container with an
///   explicit "Copy" action instead.
struct HistoryRow: View {
    let item: ClipItem
    let previewLines: Int
    let isSelected: Bool
    let isFlashing: Bool
    let onCopy: () -> Void
    let onTogglePin: () -> Void
    let onDelete: () -> Void

    @State private var isHovered = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .body) private var lineHeight: CGFloat = 17
    @ScaledMetric(relativeTo: .caption) private var metaHeight: CGFloat = 15

    /// The reserved box. Over-reserved on purpose (INV-5): v1's "smarter"
    /// character-count estimate was itself the bug.
    private var reservedHeight: CGFloat {
        lineHeight * CGFloat(previewLines) + metaHeight + 18
    }

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: symbolName)
                .font(.body)
                .foregroundStyle(item.isSensitive ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.secondary))
                .frame(width: 16)
                .padding(.top, 1)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 2) {
                previewText
                metaLine
            }

            Spacer(minLength: 0)

            if isHovered || isSelected {
                actions
            }
        }
        .padding(.horizontal, 10)
        .frame(height: reservedHeight, alignment: .top)
        .padding(.vertical, 0)
        .background(background)
        .contentShape(Rectangle())
        .onHover { isHovered = $0 }
        .onTapGesture(perform: onCopy)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(accessibilityLabel)
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
        .accessibilityAction(named: "Copy", onCopy)
        .accessibilityAction(named: item.pinned ? "Unpin" : "Pin", onTogglePin)
        .accessibilityAction(named: "Delete", onDelete)
    }

    // MARK: - Pieces

    @ViewBuilder
    private var previewText: some View {
        if let preview = item.preview {
            Text(preview)
                .font(.body)
                .lineLimit(previewLines)
                .truncationMode(.tail)
                .textSelection(.disabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            // The only thing a sensitive row is allowed to say.
            Label(ClipItem.redactedLabel, systemImage: "lock.fill")
                .labelStyle(.titleOnly)
                .font(.body.italic())
                .foregroundStyle(.secondary)
                .lineLimit(previewLines)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var metaLine: some View {
        HStack(spacing: 6) {
            Text(item.createdAt, format: .relative(presentation: .numeric))
            if item.pinned {
                Image(systemName: "pin.fill")
                    .accessibilityHidden(true)
            }
            if item.isTruncated {
                Text("truncated preview")
            }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .frame(height: metaHeight, alignment: .leading)
    }

    private var actions: some View {
        HStack(spacing: 2) {
            Button(action: onTogglePin) {
                Image(systemName: item.pinned ? "pin.slash" : "pin")
            }
            .help(item.pinned ? "Unpin" : "Pin")
            .accessibilityLabel(item.pinned ? "Unpin" : "Pin")

            Button(action: onDelete) {
                Image(systemName: "trash")
            }
            .help("Delete")
            .accessibilityLabel("Delete")
        }
        .buttonStyle(.borderless)
        .font(.caption)
        .foregroundStyle(.secondary)
    }

    @ViewBuilder
    private var background: some View {
        let shape = RoundedRectangle(cornerRadius: 6, style: .continuous)
        if isFlashing {
            // A brief confirmation that the copy landed (manifest 06 §5.3).
            // Reduced motion gets the same information without the transition.
            shape.fill(.tint.opacity(0.28))
                .animation(reduceMotion ? nil : .easeOut(duration: 0.2), value: isFlashing)
        } else if isSelected {
            shape.fill(.selection)
        } else if isHovered {
            shape.fill(.quaternary)
        } else {
            Color.clear
        }
    }

    // MARK: - Descriptions

    private var symbolName: String {
        switch item.kind {
        case .sensitive: "lock.fill"
        case .url: "link"
        case .multiline: "text.alignleft"
        case .text: "textformat"
        }
    }

    /// What VoiceOver reads for the row.
    ///
    /// For a sensitive item this is the fixed placeholder and nothing else —
    /// there is no plaintext on this type to accidentally interpolate.
    private var accessibilityLabel: String {
        var parts: [String] = []
        if item.isSensitive {
            parts.append(ClipItem.redactedLabel)
        } else {
            parts.append(String(item.singleLineLabel.prefix(120)))
        }
        if item.pinned { parts.append("Pinned") }
        parts.append(item.createdAt.formatted(.relative(presentation: .named)))
        return parts.joined(separator: ", ")
    }
}
