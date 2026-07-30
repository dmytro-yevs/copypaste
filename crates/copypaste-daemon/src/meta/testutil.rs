//! A store and a `Meta` on the same file, for the read and write tests.
//! Test-only.

use copypaste_core::{Keyring, NewItem, Store};

use super::Meta;

pub(super) struct Fixture {
    _dir: tempfile::TempDir,
    pub store: Store,
    pub meta: Meta,
}

pub(super) fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("copypaste-v2.db");
    let keyring = Keyring::from_secret(&[7u8; 32]);
    let store = Store::open(&path, &keyring.db_key()).expect("store");
    let meta = Meta::open(&path, &keyring.db_key(), "test-device").expect("meta");
    Fixture {
        _dir: dir,
        store,
        meta,
    }
}

pub(super) fn insert(store: &Store, id: &str, hash: &str, created_at: i64, sensitive: bool) {
    store
        .insert(NewItem {
            id: id.to_string(),
            content_ciphertext: vec![1, 2, 3],
            nonce: vec![4, 5, 6],
            content_type: "text".into(),
            content_hash: hash.to_string(),
            is_sensitive: sensitive,
            search_text: if sensitive {
                None
            } else {
                Some("indexed text".into())
            },
            created_at,
        })
        .expect("insert");
}
