//! Shared source-application assets. Clips store references, never PNG copies.

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use super::{Store, StoreError, StoredItem};
use crate::{PayloadMetadata, SourceAppIconMetadata};

pub(super) fn normalise(
    conn: &Connection,
    app_id: Option<&str>,
    content_type: &str,
    captured_at: i64,
    metadata: Option<&str>,
) -> Result<(Option<String>, Option<String>), StoreError> {
    let Some(mut parsed) = metadata.and_then(|json| PayloadMetadata::from_json(json, content_type))
    else {
        return Ok((metadata.map(str::to_owned), known_asset(conn, app_id)?));
    };
    let Some(icon) = parsed.source_app_icon.take() else {
        return Ok((metadata.map(str::to_owned), known_asset(conn, app_id)?));
    };
    let id = match app_id.filter(|id| !id.is_empty()) {
        Some(id) => format!("app:{id}"),
        None => format!(
            "png:{}",
            hex::encode(Sha256::digest(icon.png().ok_or(StoreError::InvalidSchema)?))
        ),
    };
    conn.execute(
        "INSERT INTO source_app_icons (id, png_base64, width, height, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(id) DO UPDATE SET png_base64 = excluded.png_base64, width = excluded.width, height = excluded.height, updated_at = excluded.updated_at \
         WHERE excluded.updated_at > source_app_icons.updated_at OR \
            (excluded.updated_at = source_app_icons.updated_at AND excluded.png_base64 > source_app_icons.png_base64)",
        params![id, icon.png_base64, icon.width, icon.height, captured_at],
    )?;
    let remaining = if parsed.file.is_some() {
        parsed.to_json(content_type)
    } else {
        None
    };
    Ok((remaining, Some(id)))
}

fn known_asset(conn: &Connection, app_id: Option<&str>) -> Result<Option<String>, StoreError> {
    let Some(app_id) = app_id.filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    Ok(conn
        .query_row(
            "SELECT id FROM source_app_icons WHERE id = ?1",
            [format!("app:{app_id}")],
            |row| row.get(0),
        )
        .optional()?)
}

pub(super) fn release_unused(conn: &Connection, id: Option<&str>) -> rusqlite::Result<()> {
    if let Some(id) = id {
        conn.execute(
            "DELETE FROM source_app_icons WHERE id = ?1 AND NOT EXISTS \
            (SELECT 1 FROM clipboard_items WHERE source_icon_id = ?1)",
            [id],
        )?;
    }
    Ok(())
}

impl Store {
    /// Hydrate source metadata only at the sync boundary; list rows keep references.
    pub fn payload_metadata_for_sync(
        &self,
        item: &StoredItem,
    ) -> Result<Option<String>, StoreError> {
        let Some(icon_id) = item.source_icon_id.as_deref() else {
            return Ok(item.payload_metadata.clone());
        };
        let Some(icon) = self.source_app_icon_by_id(icon_id)? else {
            return Ok(item.payload_metadata.clone());
        };
        let file = item
            .payload_metadata
            .as_deref()
            .and_then(|json| PayloadMetadata::from_json(json, &item.content_type))
            .and_then(|metadata| metadata.file);
        Ok(PayloadMetadata {
            file,
            source_app_icon: Some(icon),
        }
        .to_json(&item.content_type))
    }

    /// Resolve the asset independently of any individual clipboard row.
    pub fn source_app_icon_by_id(
        &self,
        icon_id: &str,
    ) -> Result<Option<SourceAppIconMetadata>, StoreError> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                "SELECT png_base64, width, height FROM source_app_icons WHERE id = ?1",
                [icon_id],
                |row| {
                    Ok(SourceAppIconMetadata {
                        png_base64: row.get(0)?,
                        width: row.get(1)?,
                        height: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::test_support::{item, store, T0};
    use super::*;

    pub(crate) fn icon() -> SourceAppIconMetadata {
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(16, 16)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        SourceAppIconMetadata::new(&png.into_inner(), 16, 16).unwrap()
    }

    fn capture(store: &Store, text: &str, app: Option<&str>, timestamp: i64) -> StoredItem {
        let mut row = item(text, timestamp);
        row.app_bundle_id = app.map(str::to_owned);
        row.payload_metadata = PayloadMetadata {
            file: None,
            source_app_icon: Some(icon()),
        }
        .to_json("text");
        store.insert(row).unwrap()
    }

    #[test]
    fn application_clips_share_one_asset_and_release_only_the_last_reference() {
        let store = store();
        let first = capture(&store, "first", Some("com.example.editor"), T0);
        let second = capture(&store, "second", Some("com.example.editor"), T0 + 1);
        assert_eq!(first.source_icon_id, second.source_icon_id);
        assert!(first.payload_metadata.is_none());
        assert!(second.payload_metadata.is_none());
        assert_eq!(
            store
                .conn()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM source_app_icons", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .source_app_icon_by_id(first.source_icon_id.as_ref().unwrap())
                .unwrap(),
            Some(icon())
        );
        store.delete(&first.id).unwrap();
        assert_eq!(
            store
                .source_app_icon_by_id(second.source_icon_id.as_ref().unwrap())
                .unwrap(),
            Some(icon())
        );
        store.delete(&second.id).unwrap();
        assert_eq!(
            store
                .conn()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM source_app_icons", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn unidentified_sources_share_by_png_content_instead_of_clip_id() {
        let store = store();
        let first = capture(&store, "first", None, T0);
        let second = capture(&store, "second", None, T0 + 1);
        assert_eq!(first.source_icon_id, second.source_icon_id);
        assert!(first.source_icon_id.unwrap().starts_with("png:"));
    }

    #[test]
    fn later_icon_discovery_links_existing_and_future_application_clips() {
        let store = store();
        let mut old = item("old", T0);
        old.app_bundle_id = Some("com.example.editor".into());
        let old = store.insert(old).unwrap();
        assert!(old.source_icon_id.is_none());
        let discovered = capture(&store, "discovered", Some("com.example.editor"), T0 + 1);
        let mut next = item("next", T0 + 2);
        next.app_bundle_id = Some("com.example.editor".into());
        let next = store.insert(next).unwrap();
        assert_eq!(
            store.get(&old.id).unwrap().unwrap().source_icon_id,
            discovered.source_icon_id
        );
        assert_eq!(next.source_icon_id, discovered.source_icon_id);
        store.delete(&discovered.id).unwrap();
        assert_eq!(
            store
                .source_app_icon_by_id(next.source_icon_id.as_ref().unwrap())
                .unwrap(),
            Some(icon())
        );
    }

    #[test]
    fn synced_clips_normalise_assets_without_losing_wire_metadata() {
        let store = store();
        let metadata = PayloadMetadata {
            file: None,
            source_app_icon: Some(icon()),
        }
        .to_json("text")
        .unwrap();
        for (id, timestamp) in [("peer-first", T0), ("peer-second", T0 + 1)] {
            let incoming = super::super::IncomingItem {
                id,
                content_ciphertext: Some(b"ciphertext"),
                nonce: Some(&[0; 24]),
                content_type: "text",
                content_hash: id,
                created_at: timestamp,
                deleted: false,
                origin_device_id: "peer",
                app_bundle_id: Some("com.example.editor"),
                app_name: Some("Editor"),
                pinned: false,
                pin_order: None,
                pin_updated_at: 0,
                search_text: Some(id),
                payload_metadata: Some(&metadata),
            };
            assert!(store.upsert(&incoming).unwrap());
            let stored = store.get(id).unwrap().unwrap();
            assert!(stored.payload_metadata.is_none());
            assert_eq!(
                store.payload_metadata_for_sync(&stored).unwrap(),
                Some(metadata.clone())
            );
        }
        assert_eq!(
            store
                .conn()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM source_app_icons", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn backup_and_restore_preserve_shared_assets_and_references() {
        use super::super::test_support::KEY;
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("history.db"), &KEY).unwrap();
        let first = capture(&store, "first", Some("com.example.editor"), T0);
        let second = capture(&store, "second", Some("com.example.editor"), T0 + 1);
        let backup = dir.path().join("backup.db");
        store.backup_to(&backup).unwrap();
        store.delete_all().unwrap();
        store.restore_from(&backup, &KEY).unwrap();
        assert_eq!(
            store.get(&first.id).unwrap().unwrap().source_icon_id,
            first.source_icon_id
        );
        assert_eq!(
            store
                .source_app_icon_by_id(second.source_icon_id.as_ref().unwrap())
                .unwrap(),
            Some(icon())
        );
        assert_eq!(
            store
                .conn()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM source_app_icons", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            1
        );
    }
}
