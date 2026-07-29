//! Generate golden ciphertext fixtures from the CURRENT (v0.4.x) implementation.
//!
//! # Why this exists
//!
//! The port-manifest harvest (`docs/rewrite/port-manifest/02-crypto.md`) found
//! that the project has **no checked-in byte fixtures from any released
//! version**. Every "can we still read old data" test reproduces the old format
//! in-process by calling the very code under test — which stops being possible
//! the moment the rewrite deletes those encrypt paths. At that point there is
//! nothing left to prove that v2 can still read a v0.4.1 database.
//!
//! This example freezes real ciphertext to disk so the rewrite has an
//! implementation-independent oracle. Run it BEFORE removing any legacy crypto
//! path:
//!
//! ```text
//! cargo run -p copypaste-core --example gen_golden_fixtures
//! ```
//!
//! # What it pins
//!
//! The whole point is to capture bytes that were produced by code we are about
//! to delete, so the generator deliberately avoids re-deriving anything the new
//! implementation could get wrong in the same way. In particular it records the
//! v1 item key **as produced by the v1 chain**, because the subtle part of that
//! chain is that the HKDF output is the key — a prior build double-derived it
//! and silently failed every legacy row's auth tag.
//!
//! Nonces come from `OsRng`, so output is NOT byte-reproducible between runs.
//! That is fine and intended: the generated files ARE the fixture. Generate
//! once, commit, never regenerate — regenerating destroys the evidence.

use copypaste_core::{
    build_item_aad, build_item_aad_v2, decrypt_chunks, decrypt_item_with_aad, derive_storage_key_v1,
    derive_v2, encrypt_chunks, encrypt_item_with_aad, AAD_SCHEMA_VERSION, AAD_SCHEMA_VERSION_V4,
};
use copypaste_core::storage::items::ItemId;
use std::fs;
use std::path::PathBuf;

/// Fixed, non-secret device secret. Deterministic so the derived keys are
/// stable across runs and can be checked into the repository alongside the
/// ciphertext. NEVER used by a real install.
const TEST_DEVICE_SECRET: [u8; 32] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];

const FILE_ID: [u8; 16] = [
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
];

/// Plaintexts chosen to cover the encoding hazards, not just the happy path:
/// empty input, multi-byte UTF-8, an embedded NUL, and a payload larger than
/// one chunk.
fn cases() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("ascii", b"hello clipboard".to_vec()),
        ("empty", Vec::new()),
        ("utf8_multibyte", "Привіт, буфере обміну 👋".as_bytes().to_vec()),
        ("embedded_nul", b"before\0after".to_vec()),
        ("binary_all_bytes", (0u8..=255).collect()),
        ("large_multichunk", vec![0x5a; 80 * 1024]),
    ]
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden-v0.4.1");

    if out_dir.exists() {
        eprintln!(
            "refusing to overwrite existing fixtures at {}\n\
             These files are evidence produced by code that is being deleted; \
             regenerating them destroys the only independent check that the \
             rewrite can still read released data. Delete the directory by hand \
             if you genuinely intend to discard it.",
            out_dir.display()
        );
        std::process::exit(1);
    }
    fs::create_dir_all(&out_dir).expect("create fixture dir");

    // v1 chain: the HKDF output IS the item key and the SQLCipher key.
    // Only v2 applies a second derivation on top of it.
    let seed = derive_storage_key_v1(&TEST_DEVICE_SECRET);
    let v1_key: [u8; 32] = *seed;
    let v2_key: [u8; 32] = *derive_v2(&seed);

    let mut entries: Vec<String> = Vec::new();

    for (name, plaintext) in cases() {
        let item_id = ItemId::from(format!("fixture-{name}"));

        // --- key_version = 1: AAD "{item_id}|{schema_version}" -------------
        let aad_v1 = build_item_aad(&item_id, AAD_SCHEMA_VERSION);
        let (nonce_v1, ct_v1) =
            encrypt_item_with_aad(&plaintext, &v1_key, &aad_v1).expect("v1 encrypt");
        let rt = decrypt_item_with_aad(&ct_v1, &nonce_v1, &v1_key, &aad_v1).expect("v1 round-trip");
        assert_eq!(rt, plaintext, "v1 round-trip mismatch for {name}");

        // --- key_version = 2: AAD "{item_id}|{schema}|{key_version}" -------
        let aad_v2 = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce_v2, ct_v2) =
            encrypt_item_with_aad(&plaintext, &v2_key, &aad_v2).expect("v2 encrypt");
        let rt = decrypt_item_with_aad(&ct_v2, &nonce_v2, &v2_key, &aad_v2).expect("v2 round-trip");
        assert_eq!(rt, plaintext, "v2 round-trip mismatch for {name}");

        // Cross-version negative: the v1 key must NOT open a v2 ciphertext.
        // This is the property build_item_aad_v2 exists to guarantee.
        assert!(
            decrypt_item_with_aad(&ct_v2, &nonce_v2, &v1_key, &aad_v1).is_err(),
            "v1 key must not decrypt a v2 ciphertext ({name})"
        );

        fs::write(out_dir.join(format!("{name}.v1.ct.bin")), &ct_v1).unwrap();
        fs::write(out_dir.join(format!("{name}.v2.ct.bin")), &ct_v2).unwrap();
        fs::write(out_dir.join(format!("{name}.plaintext.bin")), &plaintext).unwrap();

        entries.push(format!(
            r#"    {{
      "name": "{}",
      "item_id": "{}",
      "plaintext_file": "{}.plaintext.bin",
      "plaintext_len": {},
      "v1": {{
        "aad": "{}",
        "nonce_hex": "{}",
        "ciphertext_file": "{}.v1.ct.bin",
        "ciphertext_len": {}
      }},
      "v2": {{
        "aad": "{}",
        "nonce_hex": "{}",
        "ciphertext_file": "{}.v2.ct.bin",
        "ciphertext_len": {}
      }}
    }}"#,
            json_escape(name),
            json_escape(item_id.as_ref()),
            name,
            plaintext.len(),
            json_escape(&String::from_utf8_lossy(&aad_v1)),
            hex::encode(nonce_v1),
            name,
            ct_v1.len(),
            json_escape(&String::from_utf8_lossy(&aad_v2)),
            hex::encode(nonce_v2),
            name,
            ct_v2.len(),
        ));
    }

    // --- chunked container ------------------------------------------------
    // Pins the on-disk chunk framing, including the detail that the AAD magic
    // is 16 bytes with a trailing NUL rather than the 15-character string.
    let chunk_plain = vec![0x7eu8; 160 * 1024];
    let chunk_size = 64 * 1024;
    let chunks = encrypt_chunks(&chunk_plain, &v2_key, &FILE_ID, chunk_size).expect("chunk encrypt");
    let wire: Vec<u8> = chunks.iter().flat_map(|c| c.to_wire()).collect();
    let back = decrypt_chunks(&chunks, &v2_key, &FILE_ID).expect("chunk round-trip");
    assert_eq!(back, chunk_plain, "chunk round-trip mismatch");

    fs::write(out_dir.join("chunked.wire.bin"), &wire).unwrap();
    fs::write(out_dir.join("chunked.plaintext.bin"), &chunk_plain).unwrap();

    let manifest = format!(
        r#"{{
  "_comment": "Golden ciphertext captured from copypaste-core v0.4.1 before the v2 rewrite. Generated once by examples/gen_golden_fixtures.rs. DO NOT REGENERATE - these bytes are the only implementation-independent proof that a new implementation can still read released user data.",
  "source_version": "0.4.1",
  "device_secret_hex": "{}",
  "key_chain": {{
    "note": "The v1 HKDF output IS the item key and the SQLCipher key. Only v2 derives again on top of it. Double-deriving the v1 key is a known past defect that silently failed every legacy row's auth tag.",
    "seed_v1_hex": "{}",
    "v1_item_key_hex": "{}",
    "v2_item_key_hex": "{}"
  }},
  "aad_schema_version_v1": {},
  "aad_schema_version_v2": {},
  "chunked": {{
    "file_id_hex": "{}",
    "chunk_size": {},
    "chunk_count": {},
    "wire_file": "chunked.wire.bin",
    "plaintext_file": "chunked.plaintext.bin",
    "plaintext_len": {},
    "key": "v2_item_key"
  }},
  "items": [
{}
  ]
}}
"#,
        hex::encode(TEST_DEVICE_SECRET),
        hex::encode(*seed),
        hex::encode(v1_key),
        hex::encode(v2_key),
        AAD_SCHEMA_VERSION,
        AAD_SCHEMA_VERSION_V4,
        hex::encode(FILE_ID),
        chunk_size,
        chunks.len(),
        chunk_plain.len(),
        entries.join(",\n"),
    );
    fs::write(out_dir.join("manifest.json"), manifest).unwrap();

    println!("wrote fixtures to {}", out_dir.display());
    println!("  items:  {}", cases().len());
    println!("  chunks: {}", chunks.len());
}
