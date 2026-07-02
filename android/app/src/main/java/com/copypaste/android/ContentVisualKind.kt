package com.copypaste.android

/**
 * Presentation-layer content-visual-kind (android-design-system "Content
 * visual-kind resolver" requirement, P0-6). Android has no stored
 * `HistoryEntry.kind` — this enum + its [resolve] are derived at render time
 * from the ACTUAL data model: [ClipboardItem.contentType], the Rust
 * `TextKind.classify` sub-kinds, and the orthogonal [ClipboardItem.isSensitive].
 * Neither the stored content-type nor the Rust classifier contract changes.
 *
 * Resolver precedence (frozen, spec.md "Content visual-kind resolver"):
 *   1. isSensitive → SECRET (approved new behaviour — a sensitive URL/email/…
 *      shows the SECRET lock tile, not its text-kind chip; see CopyPaste-1b55
 *      for the PRE-existing chip-label behaviour this intentionally changes).
 *   2. image/file from the canonical content-type predicates ([ContentType.kt]).
 *   3. text subtype from [TextKind.classify].
 *   4. unknown/stub → TEXT.
 */
enum class ContentVisualKind {
    TEXT, URL, EMAIL, PHONE, CODE, JSON, NUMBER, COLOR, PATH, FILE, IMAGE, SECRET;

    companion object {
        /**
         * Resolves the visual kind for one item's [contentType]/[isSensitive]/[snippet].
         * Pure function — no Compose/Context dependency — so it is directly unit-testable.
         */
        fun resolve(contentType: String, isSensitive: Boolean, snippet: String): ContentVisualKind {
            if (isSensitive) return SECRET
            if (contentTypeIsImage(contentType)) return IMAGE
            if (contentTypeIsFile(contentType)) return FILE
            if (!contentTypeIsText(contentType)) return TEXT
            val label = if (snippet.isNotBlank()) TextKind.classify(snippet) else TextKind.Kind.TEXT.label
            return fromTextKindLabel(label)
        }

        private fun fromTextKindLabel(label: String): ContentVisualKind = when (label) {
            TextKind.Kind.URL.label -> URL
            TextKind.Kind.EMAIL.label -> EMAIL
            TextKind.Kind.PHONE.label -> PHONE
            TextKind.Kind.COLOR.label -> COLOR
            TextKind.Kind.JSON.label -> JSON
            TextKind.Kind.CODE.label -> CODE
            TextKind.Kind.NUMBER.label -> NUMBER
            TextKind.Kind.PATH.label -> PATH
            else -> TEXT
        }
    }
}
