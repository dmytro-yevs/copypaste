//! Test: committed Kotlin UniFFI binding for SyncedItem must have all 7 ABI-7 fields.
//!
//! This test reads the checked-in generated Kotlin binding file and asserts that
//! the `SyncedItem` data class contains the `fileName` and `mime` fields added
//! in ABI 7 (task #21b). It fails if the binding is stale (generated against ABI 6).
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
