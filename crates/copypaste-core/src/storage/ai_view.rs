//! The history view AI tools may read, grant credential verifiers and the
//! bounded access log.
//!
//! Visibility is one SQL predicate, so every tool hides the same clips: gone,
//! expired, secret, transient, credential, hidden from AI (synced note or
//! local rule), from an excluded device or app, of a disallowed kind, or
//! unpinned and older than the grant's lookback. Verifiers and the log are
//! local only and never restored.

use std::collections::HashSet;

use copypaste_ipc::ai::AiAccessEntry;
use rand::{rngs::OsRng, RngCore};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::connection::write_tx;
use super::local_state::unexpired;
use super::organization::AiGrantRecord;
use super::{Store, StoreError};

/// The access log keeps at most this many rows.
pub const ACCESS_LOG_ROWS: i64 = 1000;
/// And nothing older than this.
pub const ACCESS_LOG_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const CREDENTIAL_PREFIX: &str = "cpmcp_";
/// Most ids one visibility check accepts.
pub const MAX_VISIBILITY_IDS: usize = 512;

/// What one AI session may see, decided per call from the synced policy, the
/// local capture exclusions and the grant's lookback.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiExposure {
    pub include_other_devices: bool,
    pub excluded_devices: Vec<String>,
    pub excluded_apps: Vec<String>,
    /// Plain-text clips (links included; the caller tells them apart).
    pub text: bool,
    pub rich_text: bool,
    pub images: bool,
    pub files: bool,
    /// Unpinned clips captured before this instant are invisible.
    pub not_before_ms: Option<i64>,
}

const VISIBLE: &str = concat!(
    "SELECT ci.id FROM clipboard_items ci WHERE ci.id IN (SELECT value FROM json_each(?1)) \
     AND ci.deleted = 0 AND ",
    unexpired!("ci."),
    " AND (ci.payload_metadata IS NULL OR (json_valid(ci.payload_metadata) \
       AND IFNULL(json_extract(ci.payload_metadata, '$.privacy.secret'), 0) = 0 \
       AND IFNULL(json_extract(ci.payload_metadata, '$.privacy.transient'), 0) = 0 \
       AND IFNULL(json_extract(ci.payload_metadata, '$.credentials'), 0) = 0)) \
     AND NOT EXISTS (SELECT 1 FROM clip_notes n WHERE n.clip_id = ci.id AND n.hidden_from_ai = 1) \
     AND NOT EXISTS (SELECT 1 FROM clip_local_state s WHERE s.clip_id = ci.id \
       AND (s.rule_secret = 1 OR s.rule_hidden_from_ai = 1)) \
     AND (?2 OR ci.origin_device_id IN ('', ?3)) \
     AND (CASE WHEN ci.origin_device_id = '' THEN ?3 ELSE ci.origin_device_id END) \
       NOT IN (SELECT value FROM json_each(?4)) \
     AND (ci.app_bundle_id IS NULL OR ci.app_bundle_id NOT IN (SELECT value FROM json_each(?5))) \
     AND (ci.pinned = 1 OR ?6 IS NULL OR ci.created_at >= ?6) \
     AND ((ci.content_type = 'text' AND ?7) \
       OR (ci.content_type LIKE 'text/%' AND ?8) \
       OR (ci.content_type LIKE 'image/%' AND ?9) \
       OR (ci.content_type = 'file' AND ?10))"
);

/// A new random identifier for a grant, session or approval request.
#[must_use]
pub fn new_ai_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// A new 256-bit grant credential and the verifier stored in its place.
#[must_use]
pub fn new_ai_credential() -> (Zeroizing<String>, [u8; 32]) {
    let mut bytes = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(bytes.as_mut());
    let credential = Zeroizing::new(format!("{CREDENTIAL_PREFIX}{}", hex::encode(*bytes)));
    let verifier = ai_credential_verifier(&credential);
    (credential, verifier)
}

/// The verifier of a presented credential.
#[must_use]
pub fn ai_credential_verifier(credential: &str) -> [u8; 32] {
    Sha256::digest(credential.trim().as_bytes()).into()
}

fn json_list(values: &[String]) -> Result<String, StoreError> {
    serde_json::to_string(values).map_err(|_| StoreError::InvalidHistoryQuery)
}

impl Store {
    /// The ids among `ids` an AI tool may see. Unknown ids are simply absent,
    /// exactly like hidden ones.
    pub fn ai_visible_among(
        &self,
        ids: &[String],
        local_device_id: &str,
        exposure: &AiExposure,
    ) -> Result<HashSet<String>, StoreError> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }
        if ids.len() > MAX_VISIBILITY_IDS {
            return Err(StoreError::InvalidHistoryQuery);
        }
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(VISIBLE)?;
        let rows = stmt.query_map(
            params![
                json_list(ids)?,
                exposure.include_other_devices,
                local_device_id,
                json_list(&exposure.excluded_devices)?,
                json_list(&exposure.excluded_apps)?,
                exposure.not_before_ms,
                exposure.text,
                exposure.rich_text,
                exposure.images,
                exposure.files,
            ],
            |row| row.get::<_, String>(0),
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every grant that is not deleted, each with whether this device holds
    /// its verifier, oldest first.
    pub fn ai_grants(&self) -> Result<Vec<(AiGrantRecord, bool)>, StoreError> {
        let conn = self.conn()?;
        let ids: Vec<(String, bool)> = conn
            .prepare_cached(
                "SELECT g.id, EXISTS(SELECT 1 FROM ai_grant_verifiers v WHERE v.grant_id = g.id) \
                 FROM ai_grants g WHERE g.deleted = 0 ORDER BY g.created_at, g.id",
            )?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut grants = Vec::with_capacity(ids.len());
        for (id, verified) in ids {
            if let Some(super::organization::Record::AiGrant(grant)) =
                super::organization::read(&conn, super::organization::RecordKey::AiGrant(&id))?
            {
                grants.push((grant, verified));
            }
        }
        Ok(grants)
    }

    /// Whether this device holds a verifier for `grant_id`.
    pub fn has_ai_grant_verifier(&self, grant_id: &str) -> Result<bool, StoreError> {
        Ok(self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM ai_grant_verifiers WHERE grant_id = ?1)",
            [grant_id],
            |row| row.get(0),
        )?)
    }

    pub fn set_ai_grant_verifier(
        &self,
        grant_id: &str,
        verifier: &[u8; 32],
    ) -> Result<(), StoreError> {
        self.conn()?.execute(
            "INSERT INTO ai_grant_verifiers(grant_id, verifier) VALUES (?1, ?2) \
             ON CONFLICT(grant_id) DO UPDATE SET verifier = excluded.verifier",
            params![grant_id, verifier.as_slice()],
        )?;
        Ok(())
    }

    /// The grant a presented credential belongs to, if this device issued it.
    pub fn ai_grant_for_credential(&self, credential: &str) -> Result<Option<String>, StoreError> {
        let verifier = ai_credential_verifier(credential);
        Ok(self
            .conn()?
            .query_row(
                "SELECT grant_id FROM ai_grant_verifiers WHERE verifier = ?1",
                [verifier.as_slice()],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Drops what this device keeps locally for a grant: its verifier and its
    /// access log.
    pub fn forget_ai_grant(&self, grant_id: &str) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        tx.execute(
            "DELETE FROM ai_grant_verifiers WHERE grant_id = ?1",
            [grant_id],
        )?;
        tx.execute("DELETE FROM ai_access_log WHERE grant_id = ?1", [grant_id])?;
        tx.commit()?;
        Ok(())
    }

    /// Appends one served call and trims the log to its bounds, in one write.
    pub fn append_ai_access(
        &self,
        grant_id: &str,
        entry: &AiAccessEntry,
    ) -> Result<(), StoreError> {
        let ids = json_list(&entry.item_ids)?;
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        tx.execute(
            "INSERT INTO ai_access_log(at, grant_id, tool, item_ids_json, result_count, error) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entry.at,
                grant_id,
                entry.tool,
                ids,
                entry.result_count,
                entry.error
            ],
        )?;
        tx.execute(
            "DELETE FROM ai_access_log WHERE at < ?1 \
             OR seq <= (SELECT MAX(seq) FROM ai_access_log) - ?2",
            params![entry.at.saturating_sub(ACCESS_LOG_MS), ACCESS_LOG_ROWS],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The logged calls of one grant, newest first.
    pub fn ai_access_log(&self, grant_id: &str) -> Result<Vec<AiAccessEntry>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(
            "SELECT at, tool, item_ids_json, result_count, error FROM ai_access_log \
             WHERE grant_id = ?1 ORDER BY seq DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![grant_id, ACCESS_LOG_ROWS], |row| {
            let ids: String = row.get(2)?;
            Ok(AiAccessEntry {
                at: row.get(0)?,
                tool: row.get(1)?,
                item_ids: serde_json::from_str(&ids).unwrap_or_default(),
                result_count: row.get(3)?,
                error: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{item, store, T0};
    use super::*;

    fn entry(at: i64) -> AiAccessEntry {
        AiAccessEntry {
            at,
            tool: "list_recent".into(),
            item_ids: vec!["a".into()],
            result_count: 1,
            error: None,
        }
    }

    #[test]
    fn the_access_log_keeps_the_newest_thousand_rows_within_thirty_days() {
        let store = store();
        let now = T0 + ACCESS_LOG_MS * 2;
        store
            .append_ai_access("g", &entry(now - ACCESS_LOG_MS - 1))
            .unwrap();
        for at in 0..ACCESS_LOG_ROWS + 5 {
            store.append_ai_access("g", &entry(now + at)).unwrap();
        }
        let log = store.ai_access_log("g").unwrap();
        assert_eq!(log.len() as i64, ACCESS_LOG_ROWS);
        assert_eq!(log[0].at, now + ACCESS_LOG_ROWS + 4);
        store.forget_ai_grant("g").unwrap();
        assert!(store.ai_access_log("g").unwrap().is_empty());
    }

    #[test]
    fn a_credential_resolves_to_its_grant_only_through_its_verifier() {
        let store = store();
        let (credential, verifier) = new_ai_credential();
        store.set_ai_grant_verifier("g", &verifier).unwrap();
        assert_eq!(
            store.ai_grant_for_credential(&credential).unwrap(),
            Some("g".into())
        );
        assert_eq!(store.ai_grant_for_credential("cpmcp_00").unwrap(), None);
    }

    #[test]
    fn visibility_applies_lookback_to_unpinned_clips_and_the_kind_switches() {
        let store = store();
        let old = store.insert(item("old", T0)).unwrap().id;
        let pinned = store.insert(item("pinned", T0 + 1)).unwrap().id;
        store.set_pinned(&pinned, true).unwrap();
        let ids = vec![old.clone(), pinned.clone(), "missing".into()];
        let exposure = AiExposure {
            include_other_devices: true,
            text: true,
            not_before_ms: Some(T0 + 10),
            ..Default::default()
        };
        let visible = store.ai_visible_among(&ids, "me", &exposure).unwrap();
        assert_eq!(visible, HashSet::from([pinned.clone()]));
        let no_text = AiExposure {
            text: false,
            ..exposure
        };
        assert!(store
            .ai_visible_among(&ids, "me", &no_text)
            .unwrap()
            .is_empty());
    }
}
