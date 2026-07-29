//! Generate golden SQLite/SQLCipher database fixtures at schema v1.
//!
//! # Why this exists
//!
//! Invariant 1 of `docs/rewrite/port-manifest/03-storage.md` is "open and
//! migrate any v1..v15 database". Nothing in the tree proves it against a real
//! file: the migration tests all build their starting database by running the
//! current code, so they verify that today's migrations are self-consistent, not
//! that a database written by a released version still opens.
//!
//! These fixtures are byte-level artifacts of the v1 schema. They stay valid no
//! matter how the implementation is rewritten, and they are what the rewrite has
//! to open on day one.
//!
//! ```text
//! cargo run -p copypaste-core --example gen_golden_db
//! ```
//!
//! # Two files, two code paths
//!
//! - `v1-encrypted.db` — SQLCipher, keyed with the same test secret used by the
//!   ciphertext fixtures. The ordinary upgrade path.
//! - `v1-plaintext.db` — an unencrypted database at `user_version = 1`. This is
//!   the pre-SQLCipher install shape, and opening it must transparently encrypt
//!   in place. It is the case most likely to be dropped in a rewrite, because
//!   nobody has one any more — except the users who never upgraded.
//!
//! Row contents are real ciphertext under the v1 item key with the v1 AAD, so a
//! reader that migrates the schema correctly but derives the key wrongly still
//! fails.

use copypaste_core::{build_item_aad, derive_storage_key_v1, encrypt_item_with_aad};
use rusqlite::Connection;
use std::path::PathBuf;

/// Same secret as the ciphertext fixtures, so the two sets interoperate.
const TEST_DEVICE_SECRET: [u8; 32] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];

const V1_SCHEMA_SQL: &str = include_str!("../src/storage/schema_v1.sql");

struct Row {
    id: &'static str,
    item_id: &'static str,
    content_type: &'static str,
    plaintext: &'static str,
    is_sensitive: i64,
    lamport_ts: i64,
    wall_time: i64,
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            id: "row-0001",
            item_id: "item-0001",
            content_type: "text",
            plaintext: "first clipboard entry",
            is_sensitive: 0,
            lamport_ts: 1,
            wall_time: 1_700_000_000_000,
        },
        Row {
            id: "row-0002",
            item_id: "item-0002",
            content_type: "text",
            plaintext: "друга з багатобайтовим текстом 🎉",
            is_sensitive: 0,
            lamport_ts: 2,
            wall_time: 1_700_000_060_000,
        },
        Row {
            // Sensitive rows must never appear in FTS. Migration v13 exists to
            // purge exactly this leak from databases written before the rule
            // was enforced, so the fixture deliberately contains one.
            id: "row-0003",
            item_id: "item-0003",
            content_type: "text",
            plaintext: "AKIAIOSFODNN7EXAMPLE",
            is_sensitive: 1,
            lamport_ts: 3,
            wall_time: 1_700_000_120_000,
        },
    ]
}

/// SQLCipher raw-key form: the 32-byte key as `x'<64 hex>'`. Supplying the page
/// key directly means there is no PBKDF2 pass and no cipher parameter may be
/// set — get this wrong and the file simply will not open.
fn key_pragma(key: &[u8; 32]) -> String {
    format!("PRAGMA key = \"x'{}'\";", hex::encode(key))
}

fn populate(conn: &Connection, item_key: &[u8; 32]) {
    conn.execute_batch(V1_SCHEMA_SQL).expect("apply v1 schema");

    for r in rows() {
        let item_id = r.item_id.to_string().into();
        let aad = build_item_aad(&item_id, 3);
        let (nonce, ct) =
            encrypt_item_with_aad(r.plaintext.as_bytes(), item_key, &aad).expect("encrypt row");

        conn.execute(
            "INSERT INTO clipboard_items
               (id, item_id, content_type, content, content_nonce, is_sensitive,
                is_synced, lamport_ts, wall_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)",
            rusqlite::params![
                r.id,
                r.item_id,
                r.content_type,
                ct,
                nonce.to_vec(),
                r.is_sensitive,
                r.lamport_ts,
                r.wall_time,
            ],
        )
        .expect("insert row");

        // v1 indexed everything, including sensitive rows — that is the bug
        // migration v13 later had to clean up. Reproduce it faithfully.
        conn.execute(
            "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
            rusqlite::params![r.id, r.plaintext],
        )
        .expect("insert fts");
    }

    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('history_limit', '500')",
        [],
    )
    .expect("insert setting");

    conn.execute(
        "INSERT INTO devices (id, name, platform, public_key, fingerprint, verified, last_seen)
         VALUES ('dev-1', 'Golden Mac', 'macos', 'cHViLWtleQ==', 'AA:BB:CC:DD', 1, 1700000000)",
        [],
    )
    .expect("insert device");

    conn.pragma_update(None, "user_version", 1)
        .expect("stamp user_version = 1");
}

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden-v0.4.1");

    if !out_dir.exists() {
        eprintln!(
            "expected {} to exist — run gen_golden_fixtures first so both fixture \
             sets share one directory and one recorded device secret",
            out_dir.display()
        );
        std::process::exit(1);
    }

    let encrypted = out_dir.join("v1-encrypted.db");
    let plaintext = out_dir.join("v1-plaintext.db");
    for p in [&encrypted, &plaintext] {
        if p.exists() {
            eprintln!(
                "refusing to overwrite {} — these files are evidence; delete by \
                 hand if you genuinely intend to discard them",
                p.display()
            );
            std::process::exit(1);
        }
    }

    // The v1 chain: the HKDF output is used directly, both as the SQLCipher key
    // and as the item AEAD key. No second derivation.
    let seed = derive_storage_key_v1(&TEST_DEVICE_SECRET);
    let item_key: [u8; 32] = *seed;

    // --- encrypted -------------------------------------------------------
    {
        let conn = Connection::open(&encrypted).expect("create encrypted db");
        conn.execute_batch(&key_pragma(&item_key))
            .expect("apply SQLCipher key");
        populate(&conn, &item_key);
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").ok();
    }

    // --- plaintext (pre-SQLCipher install) --------------------------------
    {
        let conn = Connection::open(&plaintext).expect("create plaintext db");
        populate(&conn, &item_key);
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").ok();
    }

    // Sanity: the plaintext file must really be readable without a key, and the
    // encrypted one must really NOT be. Otherwise the fixtures are mislabelled
    // and the test built on them would be meaningless.
    {
        let c = Connection::open(&plaintext).unwrap();
        let v: i64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("plaintext db must open unkeyed");
        assert_eq!(v, 1, "plaintext fixture must be stamped v1");
    }
    {
        let c = Connection::open(&encrypted).unwrap();
        assert!(
            c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .is_err(),
            "encrypted fixture must not be readable without the key"
        );
    }

    println!("wrote {}", encrypted.display());
    println!("wrote {}", plaintext.display());
    println!("  rows: {}", rows().len());
}
