//! Backward-compatibility oracle: decrypt ciphertext produced by v0.4.1.
//!
//! These fixtures were generated once by `examples/gen_golden_fixtures.rs` while
//! the v0.4.x crypto paths still existed, and committed. They are the only
//! implementation-independent evidence that a reimplementation can still read
//! data written by a released version — every other compatibility test in the
//! tree reproduces the old format by calling the code under test, which proves
//! nothing once that code is replaced.
//!
//! **This file must survive the rewrite.** If it is deleted or its fixtures are
//! regenerated, the guarantee "we can still read your clipboard history after
//! upgrading" becomes unverifiable.
//!
//! What each assertion protects:
//!
//! - The v1 item key is the HKDF output itself, not a second derivation of it.
//!   A prior build double-derived it and silently failed every legacy row.
//! - The v1 AAD is `"{item_id}|3"` and the v2 AAD is `"{item_id}|4|2"`. AAD is
//!   authenticated but not encrypted, so a wrong AAD is indistinguishable from
//!   corrupt data — it fails closed and looks like data loss.
//! - The chunk container framing, including that its AAD magic is 16 bytes with
//!   a trailing NUL rather than the 15-character string.

use copypaste_core::{
    build_item_aad, build_item_aad_v2, decrypt_item_with_aad, derive_storage_key_v1, derive_v2,
    NONCE_SIZE,
};
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden-v0.4.1")
}

fn manifest() -> serde_json::Value {
    let path = fixture_dir().join("manifest.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("manifest.json is valid JSON")
}

fn read_bin(name: &str) -> Vec<u8> {
    let path = fixture_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn hex32(s: &str) -> [u8; 32] {
    let v = hex::decode(s).expect("valid hex");
    v.try_into().expect("32 bytes")
}

fn nonce_from_hex(s: &str) -> [u8; NONCE_SIZE] {
    let v = hex::decode(s).expect("valid hex");
    v.try_into().expect("24-byte nonce")
}

/// The recorded key chain must be reproducible from the recorded device secret.
///
/// This is the assertion that catches a re-derivation mistake in a rewrite: if
/// someone "tidies up" the v1 chain by deriving the storage key from the seed,
/// `v1_item_key` stops matching and every legacy row becomes unreadable.
#[test]
fn key_chain_reproduces_from_recorded_secret() {
    let m = manifest();
    let secret = hex32(m["device_secret_hex"].as_str().unwrap());

    let seed = derive_storage_key_v1(&secret);
    let recorded_seed = hex32(m["key_chain"]["seed_v1_hex"].as_str().unwrap());
    assert_eq!(*seed, recorded_seed, "v1 seed derivation changed");

    let recorded_v1 = hex32(m["key_chain"]["v1_item_key_hex"].as_str().unwrap());
    assert_eq!(
        *seed, recorded_v1,
        "the v1 item key IS the seed — it must not be derived a second time"
    );

    let v2 = derive_v2(&seed);
    let recorded_v2 = hex32(m["key_chain"]["v2_item_key_hex"].as_str().unwrap());
    assert_eq!(*v2, recorded_v2, "v2 key derivation changed");
}

#[test]
fn decrypts_every_v0_4_1_item_under_both_key_versions() {
    let m = manifest();
    let v1_key = hex32(m["key_chain"]["v1_item_key_hex"].as_str().unwrap());
    let v2_key = hex32(m["key_chain"]["v2_item_key_hex"].as_str().unwrap());

    let items = m["items"].as_array().expect("items array");
    assert!(!items.is_empty(), "fixture set must not be empty");

    for item in items {
        let name = item["name"].as_str().unwrap();
        let item_id = item["item_id"].as_str().unwrap().to_string().into();
        let expected = read_bin(item["plaintext_file"].as_str().unwrap());

        // --- key_version = 1 ------------------------------------------------
        let aad = build_item_aad(&item_id, 3);
        assert_eq!(
            String::from_utf8_lossy(&aad),
            item["v1"]["aad"].as_str().unwrap(),
            "v1 AAD layout changed for {name}"
        );
        let got = decrypt_item_with_aad(
            &read_bin(item["v1"]["ciphertext_file"].as_str().unwrap()),
            &nonce_from_hex(item["v1"]["nonce_hex"].as_str().unwrap()),
            &v1_key,
            &aad,
        )
        .unwrap_or_else(|e| panic!("v1 decrypt failed for {name}: {e:?}"));
        assert_eq!(got, expected, "v1 plaintext mismatch for {name}");

        // --- key_version = 2 ------------------------------------------------
        let aad = build_item_aad_v2(&item_id, 4, 2);
        assert_eq!(
            String::from_utf8_lossy(&aad),
            item["v2"]["aad"].as_str().unwrap(),
            "v2 AAD layout changed for {name}"
        );
        let got = decrypt_item_with_aad(
            &read_bin(item["v2"]["ciphertext_file"].as_str().unwrap()),
            &nonce_from_hex(item["v2"]["nonce_hex"].as_str().unwrap()),
            &v2_key,
            &aad,
        )
        .unwrap_or_else(|e| panic!("v2 decrypt failed for {name}: {e:?}"));
        assert_eq!(got, expected, "v2 plaintext mismatch for {name}");
    }
}

/// The key-version binding must fail closed: a v2 ciphertext must not open
/// under the v1 key and v1 AAD. This is the whole reason `build_item_aad_v2`
/// includes `key_version`.
#[test]
fn v1_key_cannot_open_v2_ciphertext() {
    let m = manifest();
    let v1_key = hex32(m["key_chain"]["v1_item_key_hex"].as_str().unwrap());

    for item in m["items"].as_array().unwrap() {
        let name = item["name"].as_str().unwrap();
        let item_id = item["item_id"].as_str().unwrap().to_string().into();
        let aad_v1 = build_item_aad(&item_id, 3);

        let result = decrypt_item_with_aad(
            &read_bin(item["v2"]["ciphertext_file"].as_str().unwrap()),
            &nonce_from_hex(item["v2"]["nonce_hex"].as_str().unwrap()),
            &v1_key,
            &aad_v1,
        );
        assert!(
            result.is_err(),
            "v1 key must not decrypt the v2 ciphertext for {name}"
        );
    }
}

/// A tampered AAD must be rejected. AAD is authenticated but not encrypted, so
/// this is the only thing binding a ciphertext to its item identity.
#[test]
fn wrong_item_id_in_aad_is_rejected() {
    let m = manifest();
    let v1_key = hex32(m["key_chain"]["v1_item_key_hex"].as_str().unwrap());
    let item = &m["items"][0];

    let wrong_id = "fixture-not-the-right-item".to_string().into();
    let result = decrypt_item_with_aad(
        &read_bin(item["v1"]["ciphertext_file"].as_str().unwrap()),
        &nonce_from_hex(item["v1"]["nonce_hex"].as_str().unwrap()),
        &v1_key,
        &build_item_aad(&wrong_id, 3),
    );
    assert!(result.is_err(), "AAD must bind the item id");
}

/// The persisted chunk container must still parse and decrypt.
#[test]
fn decrypts_v0_4_1_chunk_container() {
    let m = manifest();
    let key = hex32(m["key_chain"]["v2_item_key_hex"].as_str().unwrap());
    let file_id: [u8; 16] = hex::decode(m["chunked"]["file_id_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let expected = read_bin(m["chunked"]["plaintext_file"].as_str().unwrap());
    let wire = read_bin(m["chunked"]["wire_file"].as_str().unwrap());

    let chunks = parse_wire(&wire);
    assert_eq!(
        chunks.len(),
        m["chunked"]["chunk_count"].as_u64().unwrap() as usize,
        "chunk count changed"
    );

    let got = copypaste_core::decrypt_chunks(&chunks, &key, &file_id).expect("chunk decrypt");
    assert_eq!(got, expected, "chunk plaintext mismatch");
}

/// Parse the committed wire bytes back into chunks.
///
/// Deliberately hand-parsed against the format documented in the manifest —
/// `[version:u8=1][index:u32 BE][is_final:u8][nonce:24][len:u32 BE][ciphertext]`
/// — rather than calling a helper from the crate under test, so the test still
/// detects a framing change in a reimplementation.
fn parse_wire(mut buf: &[u8]) -> Vec<copypaste_core::EncryptedChunk> {
    let mut out = Vec::new();
    while !buf.is_empty() {
        assert!(buf.len() >= 34, "truncated chunk header");
        assert_eq!(buf[0], 1, "unexpected chunk format version");
        let chunk_index = u32::from_be_bytes(buf[1..5].try_into().unwrap());
        let is_final = buf[5] == 1;
        let nonce: [u8; NONCE_SIZE] = buf[6..30].try_into().unwrap();
        let len = u32::from_be_bytes(buf[30..34].try_into().unwrap()) as usize;
        assert!(buf.len() >= 34 + len, "truncated chunk body");
        let ciphertext = buf[34..34 + len].to_vec();
        out.push(copypaste_core::EncryptedChunk {
            chunk_index,
            is_final,
            nonce,
            ciphertext,
        });
        buf = &buf[34 + len..];
    }
    out
}
