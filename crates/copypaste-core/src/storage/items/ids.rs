//! Distinct newtypes for clipboard item identifiers (CopyPaste-crh3.80).
//!
//! ## Why two types?
//!
//! `ClipboardItem` carries two `String` id fields with completely different
//! semantics:
//!
//! - **`id` (`RowId`)** — the SQLite row primary key. Minted fresh as
//!   `Uuid::new_v4()` on every device/insert. *Never* the same across devices
//!   for the same logical item.
//! - **`item_id` (`ItemId`)** — the stable cross-device identity. Bound into
//!   the AEAD AAD as `"{item_id}|{schema_version}"`. Carried unchanged through
//!   sync (HAVE/WANT/LWW keyed on this field).
//!
//! When both were `String`, passing `item.id` (row PK) to `build_item_aad`
//! where `item.item_id` is expected compiled silently — the AEAD tag would
//! reject the ciphertext at decrypt time with no compiler warning. These
//! newtypes make that a compile error.
//!
//! ## Ergonomics
//!
//! - `Deref<Target = str>` — `&RowId` / `&ItemId` coerce to `&str`, so
//!   functions that take `&str` (e.g. `get_key_version`, `insert_tombstone`)
//!   can be called with `&item.id` / `&item.item_id` unchanged.
//! - `Display` — `format!("{}", item.id)` works.
//! - `From<String>` / `From<&str>` — `RowId::from(uuid.to_string())`.
//! - `PartialEq<str>` / `PartialEq<String>` — `assert_eq!(item.item_id, "x")`.
//! - `ToSql` / `FromSql` — `params![item.id, item.item_id, ...]` and
//!   `row.get::<_, RowId>(0)` both work for rusqlite.
//! - `Ord` / `PartialOrd` — available for sorting in tests.
//!
//! ## What is NOT provided
//!
//! There is deliberately **no `AsRef<RowId>` for `ItemId`** (or vice-versa):
//! these types must remain distinct so the compiler rejects
//! `build_item_aad(&item.id, ...)` where `&ItemId` is expected.

use std::fmt;
use std::ops::Deref;

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// RowId
// ─────────────────────────────────────────────────────────────────────────────

/// The SQLite row primary key for a `clipboard_items` row.
///
/// Minted fresh with `Uuid::new_v4()` on every device/insert; this value
/// is **not** stable across devices for the same logical item. Do NOT pass a
/// `RowId` to AEAD functions — use [`ItemId`] for that.
/// Serializes/deserializes as a plain JSON string (transparent).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RowId(pub String);

impl Deref for RowId {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl RowId {
    /// Return a `&str` view of this `RowId`.
    ///
    /// Provided as a stable alternative to calling `.as_str()` via Deref, which
    /// would resolve to `str::as_str()` — an unstable feature as of Rust 1.96.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for RowId {
    fn from(s: String) -> Self {
        RowId(s)
    }
}

impl From<&str> for RowId {
    fn from(s: &str) -> Self {
        RowId(s.to_owned())
    }
}

impl From<RowId> for String {
    fn from(r: RowId) -> String {
        r.0
    }
}

impl PartialEq<str> for RowId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for RowId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for RowId {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<RowId> for String {
    fn eq(&self, other: &RowId) -> bool {
        self == &other.0
    }
}

impl ToSql for RowId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for RowId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value).map(RowId)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ItemId
// ─────────────────────────────────────────────────────────────────────────────

/// The stable cross-device identity of a logical clipboard item.
///
/// Bound into the AEAD AAD as `"{item_id}|{schema_version}"`. Carried through
/// sync unchanged (HAVE/WANT/LWW keyed on this field). Callers MUST pass an
/// `ItemId` to AEAD functions — never a [`RowId`].
/// Serializes/deserializes as a plain JSON string (transparent).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

impl Deref for ItemId {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl ItemId {
    /// Return a `&str` view of this `ItemId`.
    ///
    /// Provided as a stable alternative to calling `.as_str()` via Deref, which
    /// would resolve to `str::as_str()` — an unstable feature as of Rust 1.96.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ItemId {
    fn from(s: String) -> Self {
        ItemId(s)
    }
}

impl From<&str> for ItemId {
    fn from(s: &str) -> Self {
        ItemId(s.to_owned())
    }
}

impl From<ItemId> for String {
    fn from(i: ItemId) -> String {
        i.0
    }
}

impl PartialEq<str> for ItemId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for ItemId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for ItemId {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<ItemId> for String {
    fn eq(&self, other: &ItemId) -> bool {
        self == &other.0
    }
}

impl ToSql for ItemId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for ItemId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        String::column_result(value).map(ItemId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_id_deref_to_str() {
        let r = RowId("abc".to_string());
        assert_eq!(&*r, "abc");
        // PartialEq<str>
        assert_eq!(r, "abc");
    }

    #[test]
    fn item_id_deref_to_str() {
        let i = ItemId("xyz".to_string());
        assert_eq!(&*i, "xyz");
        assert_eq!(i, "xyz");
    }

    #[test]
    fn row_id_and_item_id_not_interchangeable() {
        // Compile-time check: a function taking &ItemId cannot accept &RowId.
        fn needs_item_id(_: &ItemId) {}
        fn needs_row_id(_: &RowId) {}
        let i = ItemId("a".to_string());
        let r = RowId("a".to_string());
        needs_item_id(&i);
        needs_row_id(&r);
        // The following would NOT compile if uncommented:
        // needs_item_id(&r);
        // needs_row_id(&i);
    }

    #[test]
    fn from_string_and_str() {
        let r1 = RowId::from("hello".to_string());
        let r2 = RowId::from("hello");
        assert_eq!(r1, r2);
        let i1 = ItemId::from("world".to_string());
        let i2 = ItemId::from("world");
        assert_eq!(i1, i2);
    }

    #[test]
    fn into_string() {
        let r = RowId("row-1".to_string());
        let s: String = r.into();
        assert_eq!(s, "row-1");
        let i = ItemId("iid-1".to_string());
        let s2: String = i.into();
        assert_eq!(s2, "iid-1");
    }

    #[test]
    fn display() {
        let r = RowId("display-row".to_string());
        assert_eq!(format!("{r}"), "display-row");
        let i = ItemId("display-item".to_string());
        assert_eq!(format!("{i}"), "display-item");
    }
}
