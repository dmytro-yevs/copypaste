package com.copypaste.android

/**
 * Canonical content-type predicates (BUG 4 fix).
 *
 * The raw stored [contentType] string is either the short canonical form
 * ("image", "text", "file") or a full MIME type ("image/png", "text/plain").
 * Use these top-level helpers wherever a type check is needed rather than
 * inlining the two-condition `startsWith` + `==` pattern — that pattern was
 * copy-pasted to 5+ places across the codebase and drifted.
 *
 * [ClipboardItem.isImage] and [ClipboardItem.isFile] already delegate here;
 * [ClipboardItem.isText] is added for symmetry.
 *
 * ## Call sites unified by this fix
 *   - [FgsSyncLoop.poll] / [FgsSyncLoop.storeSyncedItem]
 *   - [SupabasePollWorker.doWork]
 *   - [ClipboardRepository.localItemsForSync]
 *   - [HistoryActivity] icon / copy dispatch
 */

/** Returns true when [contentType] represents an image payload. */
fun contentTypeIsImage(contentType: String): Boolean =
    contentType == "image" || contentType.startsWith("image/")

/** Returns true when [contentType] represents a file payload. */
fun contentTypeIsFile(contentType: String): Boolean =
    contentType == "file"

/**
 * Returns true when [contentType] represents a plain-text payload.
 *
 * Covers the short canonical form ("text") and full MIME types
 * ("text/plain", "text/html", …). "url" is treated as text for
 * display / clipboard-apply purposes.
 */
fun contentTypeIsText(contentType: String): Boolean =
    contentType == "text" ||
        contentType == "url" ||
        contentType.startsWith("text/")
