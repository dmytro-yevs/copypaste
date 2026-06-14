//! Cross-language crypto conformance — Rust side (golden-vector generator).
//!
//! This test produces a JSON fixture of golden AEAD vectors using the *exact*
//! UniFFI functions the Android Kotlin bindings call (`encrypt_text`,
//! `decrypt_text`, `derive_cloud_sync_key`, `cloud_encrypt`, `cloud_decrypt`
//! in `crates/copypaste-android/src/lib.rs`). The companion Kotlin instrumented
//! test (`android/app/src/androidTest/.../CryptoConformanceTest.kt`) loads the
//! same fixture on an emulator and asserts the Kotlin UniFFI bindings — calling
//! the real Rust core through the FFI boundary — recover the identical plaintext
//! from the Rust-produced ciphertext (and that Kotlin-produced ciphertext round
//! trips back through Rust). That catches FFI / marshalling / AAD drift.
//!
//! Because XChaCha20-Poly1305 uses a fresh random 24-byte nonce per encryption,
//! the (nonce, ciphertext) pair is NOT reproducible across runs. The fixture
//! therefore records the *actual* nonce + ciphertext this run produced; the
//! cross-language guarantee is on the deterministic DECRYPT side:
//!   decrypt(item_id, recorded_ciphertext, recorded_nonce, key) == plaintext.
//! For the cloud path, key derivation (Argon2id) IS deterministic, so the
//! passphrase → key mapping is also asserted.
//!
//! Regenerate the fixture intentionally:
//!   REGEN_GOLDEN_VECTORS=1 cargo test -p copypaste-android
//!
//! Default (`cargo test`) does NOT regenerate — it only validates Rust's own
//! round-trip against the committed fixture. Set `REGEN_GOLDEN_VECTORS=1`
//! explicitly to overwrite the fixture after an intentional implementation change.

use std::path::PathBuf;

use copypaste_android::{
    cloud_decrypt, cloud_encrypt, decrypt_text, derive_cloud_sync_key, encrypt_text,
};

/// Schema version of the fixture format itself (bump if the JSON shape changes).
const FIXTURE_SCHEMA: u32 = 1;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden_vectors.json")
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn from_hex(s: &str) -> Vec<u8> {
    assert!(
        s.len().is_multiple_of(2),
        "hex string must have even length"
    );
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

/// Deterministic 32-byte raw key for the per-item AEAD vectors. Distinct,
/// non-trivial bytes so a byte-order / endianness marshalling bug in the FFI
/// layer would corrupt decryption and fail the conformance check.
fn raw_key() -> Vec<u8> {
    (0u8..32u8)
        .map(|i| i.wrapping_mul(7).wrapping_add(3))
        .collect()
}

/// The plaintexts exercised by the per-item path. ASCII + Unicode + empty so
/// UTF-8 marshalling across the FFI is covered by the very first vector set.
fn item_plaintexts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("ascii", "hello cross-language conformance"),
        ("unicode", "café ☕ 日本語 🦀 — mixed BMP + astral"),
        ("empty", ""),
    ]
}

#[test]
fn generate_and_selfcheck_golden_vectors() {
    let key = raw_key();

    // ── Per-item AEAD vectors (encrypt_text / decrypt_text) ──────────────────
    // key_version=2 is the current daemon default (ITEM_KEY_VERSION_CURRENT=2,
    // AAD format "{item_id}|4|2"). New golden vectors always use v2.
    let item_key_version: u8 = 2;
    let mut item_vectors = Vec::new();
    for (label, plaintext) in item_plaintexts() {
        let item_id = format!("conformance-item-{label}");
        let blob = encrypt_text(
            item_id.clone(),
            plaintext.as_bytes(),
            &key,
            item_key_version,
        )
        .expect("encrypt_text must succeed");

        // Rust self-check: round-trip its own freshly produced vector.
        let recovered = decrypt_text(
            item_id.clone(),
            &blob.ciphertext,
            &blob.nonce,
            &key,
            item_key_version,
        )
        .expect("decrypt_text must round-trip");
        assert_eq!(
            recovered,
            plaintext.as_bytes(),
            "rust self round-trip failed for {label}"
        );

        item_vectors.push(serde_json::json!({
            "label": label,
            "item_id": item_id,
            "plaintext_utf8": plaintext,
            "key_hex": to_hex(&key),
            "key_version_u8": item_key_version,
            "nonce_hex": to_hex(&blob.nonce),
            "ciphertext_hex": to_hex(&blob.ciphertext),
        }));
    }

    // ── Cloud AEAD vectors (derive_cloud_sync_key + cloud_encrypt/decrypt) ────
    // Key derivation is Argon2id — deterministic — so the passphrase→key map is
    // a golden value Kotlin must reproduce bit-for-bit.
    let passphrase = "conformance-shared-passphrase-✓";
    let sync_key = derive_cloud_sync_key(passphrase.to_string()).expect("derive sync key");
    assert_eq!(sync_key.len(), 32, "sync key must be 32 bytes");

    let mut cloud_vectors = Vec::new();
    for (label, plaintext) in item_plaintexts() {
        let item_id = format!("conformance-cloud-{label}");
        let blob =
            cloud_encrypt(item_id.clone(), plaintext.as_bytes(), &sync_key).expect("cloud_encrypt");

        let recovered =
            cloud_decrypt(item_id.clone(), &blob, &sync_key).expect("cloud_decrypt round-trip");
        assert_eq!(
            recovered,
            plaintext.as_bytes(),
            "rust cloud self round-trip failed for {label}"
        );

        // blob = nonce[24] || ciphertext_with_tag (raw bytes, not base64).
        cloud_vectors.push(serde_json::json!({
            "label": label,
            "item_id": item_id,
            "plaintext_utf8": plaintext,
            "blob_hex": to_hex(&blob),
        }));
    }

    let fixture = serde_json::json!({
        "fixture_schema": FIXTURE_SCHEMA,
        "description":
            "Golden AEAD vectors generated by the Rust core via the Android UniFFI \
             functions. The Kotlin instrumented test must decrypt these and round-trip back.",
        "item_aead": {
            "note": "encrypt_text/decrypt_text path. key_hex is the raw 32-byte device key.",
            "vectors": item_vectors,
        },
        "cloud_aead": {
            "note": "derive_cloud_sync_key + cloud_encrypt/decrypt path. blob_hex = nonce[24]||ct.",
            "passphrase_utf8": passphrase,
            "sync_key_hex": to_hex(&sync_key),
            "vectors": cloud_vectors,
        }
    });

    let path = fixture_path();
    std::fs::create_dir_all(path.parent().unwrap()).expect("create fixtures dir");

    let regen = std::env::var("REGEN_GOLDEN_VECTORS").as_deref() == Ok("1");
    if regen {
        let pretty = serde_json::to_string_pretty(&fixture).expect("serialize fixture");
        std::fs::write(&path, format!("{pretty}\n")).expect("write fixture");
        eprintln!("wrote golden vectors to {}", path.display());
    } else {
        eprintln!(
            "REGEN_GOLDEN_VECTORS not set — not overwriting {}",
            path.display()
        );
    }
}

/// Independent self-test: load the committed fixture from disk and prove Rust
/// recovers every recorded plaintext. This is the Rust mirror of what the
/// Kotlin instrumented test does, so a regression in the fixture itself (or in
/// the decrypt path) is caught even without the emulator.
#[test]
fn rust_recovers_committed_fixture() {
    let path = fixture_path();
    if !path.exists() {
        // The committed fixture must always be present in the repository. If it
        // is missing the test environment is broken (e.g. assets were stripped or
        // the fixture was deleted without regenerating). This is not a vacuous
        // pass — fail loudly so CI catches fixture drift.
        panic!(
            "golden fixture {} is missing — run \
             `REGEN_GOLDEN_VECTORS=1 cargo test -p copypaste-android` \
             to regenerate it and commit the result",
            path.display()
        );
    }
    let raw = std::fs::read_to_string(&path).expect("read fixture");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse fixture");

    // Per-item path.
    // `key_version_u8` was added when the FFI gained the dispatch parameter
    // (CopyPaste-4i2). Old committed fixtures lack the field; default to 1
    // (the legacy AAD format "{item_id}|3") so existing golden vectors still
    // validate correctly before the fixture is regenerated by CopyPaste-ssp.
    let item = &v["item_aead"]["vectors"];
    for vec in item.as_array().expect("item vectors array") {
        let item_id = vec["item_id"].as_str().unwrap().to_string();
        let key = from_hex(vec["key_hex"].as_str().unwrap());
        let nonce = from_hex(vec["nonce_hex"].as_str().unwrap());
        let ct = from_hex(vec["ciphertext_hex"].as_str().unwrap());
        let expected = vec["plaintext_utf8"].as_str().unwrap();
        // Default to key_version=1 for pre-4i2 fixture vectors (no field).
        let key_version: u8 = vec["key_version_u8"].as_u64().unwrap_or(1) as u8;

        let pt = decrypt_text(item_id, &ct, &nonce, &key, key_version)
            .expect("decrypt committed item vector");
        assert_eq!(
            String::from_utf8(pt).unwrap(),
            expected,
            "committed item vector '{}' did not round-trip",
            vec["label"]
        );
    }

    // Cloud path: re-derive the key from the passphrase (deterministic) and
    // assert it matches the recorded key, then decrypt each blob.
    let cloud = &v["cloud_aead"];
    let passphrase = cloud["passphrase_utf8"].as_str().unwrap().to_string();
    let recorded_key_hex = cloud["sync_key_hex"].as_str().unwrap();
    let derived = derive_cloud_sync_key(passphrase).expect("re-derive sync key");
    assert_eq!(
        to_hex(&derived),
        recorded_key_hex,
        "Argon2id key derivation is not reproducible across runs"
    );
    for vec in cloud["vectors"].as_array().expect("cloud vectors array") {
        let item_id = vec["item_id"].as_str().unwrap().to_string();
        let blob = from_hex(vec["blob_hex"].as_str().unwrap());
        let expected = vec["plaintext_utf8"].as_str().unwrap();
        let pt = cloud_decrypt(item_id, &blob, &derived).expect("decrypt committed cloud vector");
        assert_eq!(
            String::from_utf8(pt).unwrap(),
            expected,
            "committed cloud vector '{}' did not round-trip",
            vec["label"]
        );
    }

    // key_version=1 anchor section: static vectors committed alongside kv=2, never regenerated.
    if let Some(v1_section) = v.get("item_aead_v1_anchor") {
        for vec in v1_section["vectors"]
            .as_array()
            .expect("v1 anchor vectors array")
        {
            let item_id = vec["item_id"].as_str().unwrap().to_string();
            let key = from_hex(vec["key_hex"].as_str().unwrap());
            let nonce = from_hex(vec["nonce_hex"].as_str().unwrap());
            let ct = from_hex(vec["ciphertext_hex"].as_str().unwrap());
            let expected = vec["plaintext_utf8"].as_str().unwrap();
            let key_version: u8 = vec["key_version_u8"]
                .as_u64()
                .expect("key_version_u8 field") as u8;
            assert_eq!(
                key_version, 1,
                "item_aead_v1_anchor vector must use key_version=1"
            );

            let pt = decrypt_text(item_id, &ct, &nonce, &key, key_version)
                .expect("decrypt committed v1 anchor vector");
            assert_eq!(
                String::from_utf8(pt).unwrap(),
                expected,
                "committed v1 anchor vector '{}' did not round-trip",
                vec["label"]
            );
        }
    }
}

/// External Known-Answer Test (KAT) for the raw XChaCha20-Poly1305 AEAD primitive.
///
/// Inputs (key, nonce, plaintext, AAD) are taken from the IRTF CFRG XChaCha Internet Draft:
///   draft-irtf-cfrg-xchacha-03, Section 2.7.1
///   <https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha>
///
/// The expected ciphertext was generated by our own XChaCha20-Poly1305 implementation
/// (chacha20poly1305 crate) using those inputs and captured as a static commit — making it a
/// deterministic regression anchor. The CFRG-specified inputs also allow independent verification
/// against libsodium or any other XChaCha20-Poly1305 implementation.
///
/// By encrypting with the CFRG-specified key/nonce/AAD and asserting the committed output, we
/// ensure two things:
///   1. Our XChaCha20-Poly1305 primitive is deterministic under a fixed nonce (no randomness leak).
///   2. Any change to the crypto layer (wrong cipher, AAD bugs, nonce truncation) breaks this test.
///
/// NOTE: production AEAD calls use `encrypt_item_with_aad` / `decrypt_item_with_aad` which bind
/// an `item_id|schema_version` AAD and use a fresh OsRng nonce. This KAT bypasses that layer to
/// test the XChaCha20-Poly1305 primitive directly with a fixed nonce, using a raw AAD from the
/// published spec. The production AAD-binding path is exercised by the golden-vector round-trips
/// above.
///
/// LIMITATION: We could not verify our ciphertext byte-for-byte against the published draft at
/// the time of writing (would require internet access during test authoring). Cross-verification
/// against libsodium or another reference implementation is recommended. The inputs are correct as
/// published; the expected ciphertext is our implementation's output for those inputs.
#[test]
fn external_kat_xchacha20poly1305_cfrg_draft_inputs() {
    use chacha20poly1305::{
        aead::{Aead, KeyInit, Payload},
        XChaCha20Poly1305, XNonce,
    };

    // Key: from draft-irtf-cfrg-xchacha-03 §2.7.1
    let key = from_hex("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
    // Nonce: 24 bytes, from the same draft section
    let nonce_bytes = from_hex("404142434445464748494a4b4c4d4e4f5051525354555657");
    // AAD: 12 bytes, from the same draft section
    let aad = from_hex("50515253c0c1c2c3c4c5c6c7");
    // Plaintext: "Ladies and Gentlemen of the class of '99: If I could offer you only one tip
    //             for the future, sunscreen would be it."
    // Source: draft-irtf-cfrg-xchacha-03 §2.7.1 (with trailing period).
    let plaintext = from_hex(
        "4c616469657320616e642047656e746c656d656e206f662074686520636c617373206f662027393\
         93a204966204920636f756c64206f6666657220796f75206f6e6c79206f6e652074697020666f\
         7220746865206675747572652c2073756e73637265656e20776f756c642062652069742e",
    );
    // Expected ciphertext+tag: produced by our chacha20poly1305 implementation for the inputs
    // above.  Committed as a static anchor — any regression in the AEAD primitive will change
    // this value and fail the test.
    let expected_ct_with_tag = from_hex(
        "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb731c7f1b0b4aa64\
         40bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b4522f8c9ba40db5d945b11b69b982c1\
         bb9e3f3fac2bc369488f76b2383565d3fff921f9664c97637da9768812f615c68b13b52ec08759\
         24c1c7987947deafd8780acf49",
    );

    let key_arr: [u8; 32] = key.try_into().expect("key must be 32 bytes");
    let nonce_arr: [u8; 24] = nonce_bytes.try_into().expect("nonce must be 24 bytes");
    let nonce = XNonce::from(nonce_arr);
    let cipher = XChaCha20Poly1305::new((&key_arr).into());

    // Encrypt with fixed nonce: must produce the committed ciphertext exactly.
    let ct = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: &plaintext,
                aad: &aad,
            },
        )
        .expect("XChaCha20-Poly1305 encryption must succeed");
    assert_eq!(
        ct, expected_ct_with_tag,
        "XChaCha20-Poly1305 output does not match committed CFRG-input anchor — \
         the cipher implementation changed"
    );

    // Decrypt the committed ciphertext: must recover the original plaintext.
    let recovered = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: &expected_ct_with_tag,
                aad: &aad,
            },
        )
        .expect("XChaCha20-Poly1305 decryption of committed KAT vector must succeed");
    assert_eq!(
        recovered, plaintext,
        "decrypted CFRG-input KAT ciphertext does not match expected plaintext"
    );
}
