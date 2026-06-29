//! CopyPaste-crh3.80: distinct newtypes for the two string identifiers a
//! [`super::ClipboardItem`] carries, so they can never be confused at a call
//! site that the compiler should reject.
//!
//! * [`RowId`] — the **local database primary key** (`clipboard_items.id`). A
//!   fresh per-row UUID. Used for row lookups, FTS joins, deletes, and pin
//!   ordering. Has NO cryptographic meaning.
//! * [`ItemId`] — the **cross-device logical identity** (`clipboard_items.item_id`).
//!   Bound into the AEAD AAD (`build_item_aad`/`build_item_aad_v2`) and used by
//!   the sync/merge/dedup layer as the HAVE/WANT/LWW key.
//!
//! Before this split both were a bare `String`, so a call that wanted the AEAD
//! identity but passed the row PK (or vice-versa) compiled cleanly and produced
//! a ciphertext whose AAD bound the wrong identifier — an undetectable bug. The
//! two types are intentionally NOT inter-convertible (no `From<RowId> for
//! ItemId`); the only bridge is an explicit `.as_str()` / `String` round-trip,
//! which is exactly what crossing the wire/DB boundary already does.
//!
//! Both newtypes are `#[serde(transparent)]` and implement rusqlite `ToSql` /
//! `FromSql` by delegating to the inner `String`, so on-wire JSON and on-disk
//! column serialization are byte-for-byte identical to the previous bare-String
//! representation — no schema or protocol change.

use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use std::fmt;

macro_rules! string_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Borrow the inner identifier as a `&str`.
            #[inline]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume the newtype, yielding the inner `String`.
            #[inline]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<String> for $name {
            #[inline]
            fn from(s: String) -> Self {
                $name(s)
            }
        }

        impl From<&str> for $name {
            #[inline]
            fn from(s: &str) -> Self {
                $name(s.to_owned())
            }
        }

        impl From<&String> for $name {
            #[inline]
            fn from(s: &String) -> Self {
                $name(s.clone())
            }
        }

        impl From<$name> for String {
            #[inline]
            fn from(v: $name) -> String {
                v.0
            }
        }

        impl AsRef<str> for $name {
            #[inline]
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        // Deref to `str` so a `&RowId` / `&ItemId` coerces to `&str` at the many
        // storage call sites that take a bare `id: &str` (row lookups, FTS
        // joins, deletes) — they keep working unchanged. This does NOT weaken
        // the AEAD guarantee: `build_item_aad`/`build_item_aad_v2` take a
        // concrete `&ItemId`, and the Deref target is `str` (not `ItemId`/
        // `RowId`), so a `&RowId` still cannot coerce into the `&ItemId` an AAD
        // call demands — that misuse stays a compile error.
        impl std::ops::Deref for $name {
            type Target = str;
            #[inline]
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl PartialEq<str> for $name {
            #[inline]
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            #[inline]
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<String> for $name {
            #[inline]
            fn eq(&self, other: &String) -> bool {
                &self.0 == other
            }
        }

        impl PartialEq<$name> for str {
            #[inline]
            fn eq(&self, other: &$name) -> bool {
                self == other.0
            }
        }

        impl PartialEq<$name> for String {
            #[inline]
            fn eq(&self, other: &$name) -> bool {
                self == &other.0
            }
        }

        impl PartialEq<$name> for &str {
            #[inline]
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        // Delegate DB (de)serialization to the inner String so the on-disk
        // column representation is unchanged — `params![item.id]` and
        // `row.get::<_, $name>(n)` both round-trip through TEXT verbatim.
        impl ToSql for $name {
            #[inline]
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::Borrowed(ValueRef::Text(self.0.as_bytes())))
            }
        }

        impl FromSql for $name {
            #[inline]
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                String::column_result(value).map($name)
            }
        }
    };
}

string_newtype! {
    /// Local DB row primary key (`clipboard_items.id`). NOT a crypto identity.
    /// See module docs.
    RowId
}

string_newtype! {
    /// Cross-device logical item identity (`clipboard_items.item_id`); the value
    /// bound into the AEAD AAD and keyed on by the sync/merge/dedup layer.
    /// See module docs.
    ItemId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newtypes_are_distinct_and_not_interconvertible() {
        // A compile-time guarantee, asserted indirectly: the only way to turn a
        // RowId into an ItemId is via an explicit String/&str round-trip.
        let row = RowId::from("abc");
        let item = ItemId::from(row.as_str());
        assert_eq!(item.as_str(), "abc");
        assert_eq!(row.as_str(), "abc");
    }

    #[test]
    fn display_and_as_ref_yield_inner() {
        let id = ItemId::from("xyz-123");
        assert_eq!(id.to_string(), "xyz-123");
        assert_eq!(id.as_ref() as &str, "xyz-123");
        assert_eq!(format!("{id}|4|2"), "xyz-123|4|2");
    }

    #[test]
    fn partial_eq_against_str_and_string() {
        let id = RowId::from("k");
        assert_eq!(id, "k");
        assert_eq!(id, "k".to_string());
        assert!(id == *"k");
    }

    #[test]
    fn serde_is_transparent() {
        let id = ItemId::from("wire-value");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"wire-value\"");
        let back: ItemId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }
}
