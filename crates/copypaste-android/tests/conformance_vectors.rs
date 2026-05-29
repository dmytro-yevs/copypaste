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
//!   cargo test -p copypaste-android --test conformance_vectors -- --nocapture
//!
//! Run with `REGEN_GOLDEN_VECTORS=0` to skip overwriting the committed fixture
//! and only validate Rust's own round-trip against the existing file.

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
    let mut item_vectors = Vec::new();
    for (label, plaintext) in item_plaintexts() {
        let item_id = format!("conformance-item-{label}");
        let blob = encrypt_text(item_id.clone(), plaintext.as_bytes(), &key)
            .expect("encrypt_text must succeed");

        // Rust self-check: round-trip its own freshly produced vector.
        let recovered = decrypt_text(item_id.clone(), &blob.ciphertext, &blob.nonce, &key)
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

    let regen = std::env::var("REGEN_GOLDEN_VECTORS").as_deref() != Ok("0");
    if regen {
        let pretty = serde_json::to_string_pretty(&fixture).expect("serialize fixture");
        std::fs::write(&path, format!("{pretty}\n")).expect("write fixture");
        eprintln!("wrote golden vectors to {}", path.display());
    } else {
        eprintln!(
            "REGEN_GOLDEN_VECTORS=0 — not overwriting {}",
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
        // First run: the generator test produces it; nothing to validate yet.
        eprintln!(
            "fixture {} not present yet — run the generator first",
            path.display()
        );
        return;
    }
    let raw = std::fs::read_to_string(&path).expect("read fixture");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse fixture");

    // Per-item path.
    let item = &v["item_aead"]["vectors"];
    for vec in item.as_array().expect("item vectors array") {
        let item_id = vec["item_id"].as_str().unwrap().to_string();
        let key = from_hex(vec["key_hex"].as_str().unwrap());
        let nonce = from_hex(vec["nonce_hex"].as_str().unwrap());
        let ct = from_hex(vec["ciphertext_hex"].as_str().unwrap());
        let expected = vec["plaintext_utf8"].as_str().unwrap();

        let pt = decrypt_text(item_id, &ct, &nonce, &key).expect("decrypt committed item vector");
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
}
