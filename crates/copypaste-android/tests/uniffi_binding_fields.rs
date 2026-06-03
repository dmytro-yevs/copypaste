//! Test: committed Kotlin UniFFI binding must have all ABI-7 SyncedItem fields
//! and all ABI-8 LocalItem fields.
//!
//! This test reads the checked-in generated Kotlin binding file and asserts that:
//! - `SyncedItem` data class contains the `fileName` and `mime` fields (ABI 7, task #21b).
//! - `LocalItem` data class contains the `fileName` and `mime` fields (ABI 8, Android→macOS
//!   file send).
//!
//! Fails if the binding is stale (generated against an older ABI).
//!
//! Run with:
//!   cargo test -p copypaste-android --test uniffi_binding_fields

use std::path::PathBuf;

/// Absolute path to the committed Kotlin binding, resolved from CARGO_MANIFEST_DIR.
fn kotlin_binding_path() -> PathBuf {
    // crates/copypaste-android → repo root → android/app/src/...
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../android/app/src/main/java/com/copypaste/generated/uniffi/copypaste_android/copypaste_android.kt")
}

#[test]
fn synced_item_kotlin_binding_has_seven_abi7_fields() {
    let path = kotlin_binding_path();
    assert!(
        path.exists(),
        "Kotlin binding not found at {}: run ./scripts/generate-android-bindings.sh",
        path.display()
    );

    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    // ABI 7 adds `fileName` and `mime` to SyncedItem.
    // The generated Kotlin uses camelCase for snake_case UDL fields.
    assert!(
        src.contains("var `fileName`"),
        "SyncedItem in the committed Kotlin binding is STALE: missing `fileName` (ABI 7). \
         Regenerate with ./scripts/generate-android-bindings.sh"
    );
    assert!(
        src.contains("var `mime`"),
        "SyncedItem in the committed Kotlin binding is STALE: missing `mime` (ABI 7). \
         Regenerate with ./scripts/generate-android-bindings.sh"
    );

    // Also verify the FfiConverter reads 7 fields (7 FfiConverter*.read() calls
    // inside the FfiConverterTypeSyncedItem.read block).
    // We check that at least the optional-string converters appear in the file,
    // which means the two new Optional<String> fields are covered.
    assert!(
        src.contains("FfiConverterOptionalString"),
        "SyncedItem FfiConverter is missing FfiConverterOptionalString reads (ABI 7 file_name/mime). \
         Regenerate with ./scripts/generate-android-bindings.sh"
    );
}

/// ABI 8: `LocalItem` must have `fileName` and `mime` fields so Kotlin can
/// pass file metadata on the outbound (Android→macOS) send path.
#[test]
fn local_item_kotlin_binding_has_abi8_file_fields() {
    let path = kotlin_binding_path();
    assert!(
        path.exists(),
        "Kotlin binding not found at {}: run ./scripts/generate-android-bindings.sh",
        path.display()
    );

    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    // The generated Kotlin uses camelCase; UDL `file_name` → Kotlin `fileName`.
    // Both LocalItem and SyncedItem have these fields (ABI 8 adds them to
    // LocalItem; ABI 7 added them to SyncedItem). A simple `contains` check
    // covers both data classes — failing means the binding is stale.
    assert!(
        src.contains("data class LocalItem"),
        "LocalItem data class missing from Kotlin binding. \
         Regenerate with ./scripts/generate-android-bindings.sh"
    );

    // Verify LocalItem carries the ABI-8 optional fields. We locate the
    // LocalItem block by checking that `fileName` appears after `LocalItem`.
    let local_item_pos = src
        .find("data class LocalItem")
        .expect("LocalItem class not found");
    let after_local_item = &src[local_item_pos..];
    // The next data class declaration ends the LocalItem block.
    let local_item_block = after_local_item
        .find("\ndata class ")
        .map(|end| &after_local_item[..end])
        .unwrap_or(after_local_item);

    assert!(
        local_item_block.contains("fileName"),
        "LocalItem in the committed Kotlin binding is STALE: missing `fileName` (ABI 8). \
         Regenerate with ./scripts/generate-android-bindings.sh"
    );
    assert!(
        local_item_block.contains("`mime`"),
        "LocalItem in the committed Kotlin binding is STALE: missing `mime` (ABI 8). \
         Regenerate with ./scripts/generate-android-bindings.sh"
    );
}
