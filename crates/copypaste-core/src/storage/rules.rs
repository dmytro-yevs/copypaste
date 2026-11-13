//! Content rule queries and writes.
//!
//! Rules and their placements are organization records; this module holds
//! what sits above them: drafts and limits, ordering, delete and restore,
//! built-in seeding and reset, and the conflicting edits that concurrent
//! definition changes leave behind. Every write records the stamp of the
//! version it replaced, so a version written without knowing the local one
//! is recognised as concurrent: its loser is kept for restore and a delete
//! wins over a concurrent edit. It also reads the clips a retroactive
//! preview evaluates, one at a time.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::connection::write_tx;
use super::local_state::{self, unexpired, ClipLocalState};
use super::organization::{self, ContentRuleRecord, Record, RecordKey, RulePlacementRecord};
use super::snippets::order_key_between;
use super::{Store, StoreError};
use crate::rules::builtin::{self, BuiltinRule};
use crate::rules::compile::{self, RuleError, RuleInput};
use crate::rules::evaluate::TEXT_PREFIX_BYTES;
use crate::rules::model::{MAX_RULES, SCHEMA_VERSION};
use crate::{ItemKey, PayloadMetadata};

/// How long a deleted rule stays listed for restore.
pub const RECENTLY_DELETED_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const MAX_NAME_CHARS: usize = 128;
const MAX_SCOPE_DEVICES: usize = 64;
/// Customized fields of a built-in rule.
const FIELD_NAME: &str = "name";
const FIELD_DEFINITION: &str = "definition";
const FIELD_SCOPE: &str = "device_scope";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDraft {
    /// `None` creates a user rule; otherwise a live rule is updated.
    pub id: Option<String>,
    pub name: String,
    pub enabled: bool,
    pub definition_json: String,
    /// Device ids; empty runs the rule on every device.
    pub device_scope: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RuleWriteError {
    #[error("rule not found")]
    NotFound,
    #[error("the rule definition is not valid")]
    Invalid(Vec<RuleError>),
    #[error("the rule name is not valid")]
    InvalidName,
    #[error("the device scope is not valid")]
    InvalidScope,
    #[error("the rule list is full")]
    LimitReached,
    #[error("only a built-in rule can be reset")]
    NotBuiltin,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for RuleWriteError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

/// A rule as listed: the synced record, its placement and its last writer.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredRule {
    pub record: ContentRuleRecord,
    pub enabled: bool,
    pub order_key: String,
    /// Wall-clock ms of the stored version.
    pub stamp: i64,
    /// The device that wrote the stored version; `None` when this one did.
    pub author: Option<String>,
}

/// A definition that lost to a concurrent edit, kept for 30 days.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRuleConflict {
    pub id: String,
    pub rule_id: String,
    pub definition_json: String,
    /// The device whose edit lost.
    pub from_device: String,
    pub created_at: i64,
}

/// One clip a rule preview evaluates, without its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSubject {
    pub id: String,
    pub content_type: String,
    pub byte_len: u64,
    pub created_at: i64,
    pub pinned: bool,
    /// Secret by its payload or masked by a rule.
    pub secret: bool,
    pub app_bundle_id: Option<String>,
    pub app_name: Option<String>,
    /// Empty when captured here.
    pub origin_device_id: String,
    /// What text conditions read for a file: its name and locator.
    pub file_text: Option<String>,
}

/// Decrypted text of a rule subject, wiped when dropped.
pub type SubjectText = Zeroizing<String>;

/// Where a page of [`Store::rule_subjects`] continues: newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectCursor {
    pub created_at: i64,
    pub id: String,
}

fn this_device(conn: &Connection) -> Result<String, StoreError> {
    Ok(conn
        .query_row(
            "SELECT value FROM sync_device_state WHERE key='device_id'",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_default())
}

/// The stamp and writer of a rule's stored version.
fn version_of(conn: &Connection, rule_id: &str) -> Result<Option<(i64, String)>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT o.created_at, o.origin_device_id FROM content_rules r \
             JOIN organization_records o ON o.id = r.record_id WHERE r.id = ?1",
            [rule_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn stored_rule(conn: &Connection, id: &str) -> Result<Option<ContentRuleRecord>, StoreError> {
    match organization::read(conn, RecordKey::ContentRule(id))? {
        Some(Record::ContentRule(rule)) => Ok(Some(rule)),
        _ => Ok(None),
    }
}

fn stored_placement(
    conn: &Connection,
    id: &str,
) -> Result<Option<RulePlacementRecord>, StoreError> {
    match organization::read(conn, RecordKey::ContentRulePlacement(id))? {
        Some(Record::ContentRulePlacement(placement)) => Ok(Some(placement)),
        _ => Ok(None),
    }
}

/// The placement a rule runs with when none has arrived yet.
fn default_placement(rule: &ContentRuleRecord) -> RulePlacementRecord {
    builtin::builtin(&rule.id).map_or_else(
        || RulePlacementRecord {
            id: rule.id.clone(),
            enabled: true,
            order_key: String::new(),
        },
        BuiltinRule::placement,
    )
}

/// Writes `rule` as the successor of the stored version.
fn write_rule(
    tx: &Connection,
    key: &ItemKey,
    mut rule: ContentRuleRecord,
) -> Result<(), StoreError> {
    rule.base_stamp = version_of(tx, &rule.id)?.map(|(stamp, _)| stamp);
    organization::write(tx, key, Record::ContentRule(rule))
}

/// A semantic edit moves `effective_from`, so receipt evaluation applies the
/// edited rule only to clips created after it.
fn next_effective_from(previous: i64, now_ms: i64) -> i64 {
    now_ms.max(previous.saturating_add(1))
}

fn live_user_rules(conn: &Connection) -> Result<usize, StoreError> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM content_rules WHERE origin='user' AND deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?)
}

/// Sibling rules of one tier in list order, without `except`.
fn siblings(
    conn: &Connection,
    origin: &str,
    except: &str,
) -> Result<Vec<(String, String)>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT r.id, COALESCE(p.order_key, '') FROM content_rules r \
         LEFT JOIN content_rule_placement p ON p.id = r.id \
         WHERE r.origin = ?1 AND r.deleted_at IS NULL AND r.id <> ?2 \
         ORDER BY COALESCE(p.order_key, ''), r.id",
    )?;
    let rows = statement
        .query_map(params![origin, except], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<(String, String)>>>()?;
    Ok(rows)
}

fn origin_name(rule: &ContentRuleRecord) -> &'static str {
    match rule.origin {
        organization::RuleOrigin::Builtin => "builtin",
        organization::RuleOrigin::User => "user",
    }
}

fn valid_scope(scope: &[String]) -> bool {
    scope.len() <= MAX_SCOPE_DEVICES
        && scope
            .iter()
            .all(|id| !id.trim().is_empty() && id.len() <= 128 && !id.chars().any(char::is_control))
}

/// Which fields of a built-in differ from what ships.
fn customized(shipped: &ContentRuleRecord, rule: &ContentRuleRecord) -> Vec<String> {
    let mut fields = Vec::new();
    if rule.name != shipped.name {
        fields.push(FIELD_NAME.to_owned());
    }
    if rule.definition_json != shipped.definition_json {
        fields.push(FIELD_DEFINITION.to_owned());
    }
    if rule.device_scope != shipped.device_scope {
        fields.push(FIELD_SCOPE.to_owned());
    }
    fields
}

fn untouched_builtin(rule: &ContentRuleRecord) -> bool {
    rule.origin == organization::RuleOrigin::Builtin && rule.customized_fields.is_empty()
}

fn conflict_id(rule_id: &str, stamp: i64) -> String {
    let digest = Sha256::digest(format!("{rule_id}:{stamp}").as_bytes());
    hex::encode(&digest[..16])
}

fn keep_conflict(
    tx: &Connection,
    rule_id: &str,
    definition_json: &str,
    from_device: &str,
    stamp: i64,
) -> Result<(), StoreError> {
    let from_device = if from_device.is_empty() {
        this_device(tx)?
    } else {
        from_device.to_owned()
    };
    tx.execute(
        "INSERT OR IGNORE INTO content_rule_conflicts(id, rule_id, definition_json, from_device, \
         created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            conflict_id(rule_id, stamp),
            rule_id,
            definition_json,
            from_device,
            crate::now_ms()
        ],
    )?;
    Ok(())
}

/// Called by the projection before `incoming` replaces the stored rule. A
/// version that was written without knowing the stored one is concurrent
/// with it; when their definitions differ the stored one is kept for restore.
pub(crate) fn observe_replaced(
    tx: &Connection,
    incoming: &ContentRuleRecord,
) -> Result<(), StoreError> {
    let Some(local) = stored_rule(tx, &incoming.id)? else {
        return Ok(());
    };
    let Some((stamp, author)) = version_of(tx, &incoming.id)? else {
        return Ok(());
    };
    let concurrent = incoming.base_stamp.is_none_or(|base| base < stamp);
    if !concurrent || local.definition_json == incoming.definition_json || untouched_builtin(&local)
    {
        return Ok(());
    }
    keep_conflict(tx, &local.id, &local.definition_json, &author, stamp)
}

/// Whether the stored deletion of a rule stands against `incoming`: a live
/// version written without knowing the deletion loses, and its definition is
/// kept for restore. A restore made after seeing the deletion wins.
pub(crate) fn keeps_deletion(
    tx: &Connection,
    incoming: &ContentRuleRecord,
    origin_device_id: &str,
    incoming_stamp: i64,
) -> Result<bool, StoreError> {
    if incoming.deleted_at.is_some() {
        return Ok(false);
    }
    let Some(local) = stored_rule(tx, &incoming.id)? else {
        return Ok(false);
    };
    let Some((stamp, _)) = version_of(tx, &incoming.id)? else {
        return Ok(false);
    };
    if local.deleted_at.is_none() || incoming.base_stamp.is_some_and(|base| base >= stamp) {
        return Ok(false);
    }
    if local.definition_json != incoming.definition_json && !untouched_builtin(incoming) {
        keep_conflict(
            tx,
            &incoming.id,
            &incoming.definition_json,
            origin_device_id,
            incoming_stamp,
        )?;
    }
    Ok(true)
}

/// Clip columns a rule subject reads; the caller adds its filter.
macro_rules! subject_query {
    () => {
        concat!(
            "SELECT ci.id, ci.content_type, length(ci.content_ciphertext), ci.created_at, \
             ci.pinned, ci.payload_metadata, COALESCE(s.rule_secret, 0), ci.app_bundle_id, \
             ci.app_name, ci.origin_device_id FROM clipboard_items ci \
             LEFT JOIN clip_local_state s ON s.clip_id = ci.id WHERE ci.deleted = 0 AND ",
            unexpired!("ci.")
        )
    };
}

fn subject_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<RuleSubject> {
    let content_type: String = r.get(1)?;
    let metadata = r
        .get::<_, Option<String>>(5)?
        .and_then(|json| PayloadMetadata::from_json(&json, &content_type));
    let secret = r.get::<_, bool>(6)? || metadata.as_ref().is_some_and(|m| m.privacy.secret);
    Ok(RuleSubject {
        id: r.get(0)?,
        byte_len: r
            .get::<_, Option<i64>>(2)?
            .and_then(|len| u64::try_from(len).ok())
            .unwrap_or(0),
        created_at: r.get(3)?,
        pinned: r.get(4)?,
        secret,
        app_bundle_id: r.get(7)?,
        app_name: r.get(8)?,
        origin_device_id: r.get(9)?,
        file_text: metadata
            .and_then(|metadata| metadata.file)
            .map(|file| crate::rules::apply::file_subject(&file)),
        content_type,
    })
}

fn plain_text(content_type: &str, bytes: Zeroizing<Vec<u8>>) -> Option<Zeroizing<String>> {
    let text = Zeroizing::new(String::from_utf8(bytes.to_vec()).ok()?);
    let mut text = if content_type == copypaste_ipc::content_type::HTML
        || content_type == copypaste_ipc::content_type::RICH_TEXT
    {
        Zeroizing::new(crate::rich_text::plain_text(content_type, &text).ok()?)
    } else {
        text
    };
    if text.len() > TEXT_PREFIX_BYTES {
        let mut cut = TEXT_PREFIX_BYTES;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    Some(text)
}

impl Store {
    /// Every rule with its placement: live ones, and deleted ones while they
    /// can be restored. Built-ins first, then by order key.
    pub fn content_rule_list(&self, now_ms: i64) -> Result<Vec<StoredRule>, StoreError> {
        let conn = self.conn()?;
        let ids: Vec<String> = conn
            .prepare(
                "SELECT r.id FROM content_rules r \
                 LEFT JOIN content_rule_placement p ON p.id = r.id \
                 WHERE r.deleted_at IS NULL OR r.deleted_at > ?1 \
                 ORDER BY r.origin <> 'builtin', COALESCE(p.order_key, ''), r.id",
            )?
            .query_map([now_ms.saturating_sub(RECENTLY_DELETED_MS)], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut rules = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(rule) = listed_rule(&conn, &id)? {
                rules.push(rule);
            }
        }
        Ok(rules)
    }

    /// One rule, live or deleted.
    pub fn content_rule(&self, id: &str) -> Result<Option<StoredRule>, StoreError> {
        listed_rule(&*self.conn()?, id)
    }

    /// Live rules as the evaluator reads them.
    pub fn content_rule_inputs(&self) -> Result<Vec<RuleInput>, StoreError> {
        compile::load_inputs(&*self.conn()?)
    }

    /// Losing definitions of concurrent edits, newest first.
    pub fn content_rule_conflicts(&self) -> Result<Vec<StoredRuleConflict>, StoreError> {
        let conn = self.conn()?;
        let conflicts = conn
            .prepare(
                "SELECT id, rule_id, definition_json, from_device, created_at \
                 FROM content_rule_conflicts ORDER BY created_at DESC, id",
            )?
            .query_map([], |r| {
                Ok(StoredRuleConflict {
                    id: r.get(0)?,
                    rule_id: r.get(1)?,
                    definition_json: r.get(2)?,
                    from_device: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(conflicts)
    }

    /// Per rule: live clips it acted on and the newest one's creation time.
    pub fn content_rule_stats(&self) -> Result<HashMap<String, (u64, i64)>, StoreError> {
        let conn = self.conn()?;
        let stats = conn
            .prepare(
                "SELECT j.value, COUNT(*), MAX(c.created_at) FROM clip_local_state s \
                 JOIN clipboard_items c ON c.id = s.clip_id AND c.deleted = 0, \
                 json_each(s.applied_rules_json) j \
                 WHERE s.applied_rules_json IS NOT NULL GROUP BY j.value",
            )?
            .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(stats)
    }

    /// Creates or updates a rule and its placement in one transaction.
    pub fn save_content_rule(
        &self,
        key: &ItemKey,
        draft: &RuleDraft,
        now_ms: i64,
    ) -> Result<String, RuleWriteError> {
        let name = draft.name.trim();
        if name.is_empty()
            || name.chars().count() > MAX_NAME_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(RuleWriteError::InvalidName);
        }
        if !valid_scope(&draft.device_scope) {
            return Err(RuleWriteError::InvalidScope);
        }
        let errors = compile::validate(&draft.definition_json);
        if !errors.is_empty() {
            return Err(RuleWriteError::Invalid(errors));
        }
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let existing = match &draft.id {
            Some(id) => Some(
                stored_rule(&tx, id)?
                    .filter(|rule| rule.deleted_at.is_none())
                    .ok_or(RuleWriteError::NotFound)?,
            ),
            None => None,
        };
        let rule = match existing {
            Some(existing) => {
                let semantic = existing.definition_json != draft.definition_json
                    || existing.device_scope != draft.device_scope;
                let mut rule = ContentRuleRecord {
                    name: name.to_owned(),
                    definition_json: draft.definition_json.clone(),
                    device_scope: draft.device_scope.clone(),
                    schema_version: SCHEMA_VERSION,
                    effective_from: if semantic {
                        next_effective_from(existing.effective_from, now_ms)
                    } else {
                        existing.effective_from
                    },
                    ..existing
                };
                if let Some(shipped) = builtin::builtin(&rule.id) {
                    rule.customized_fields = customized(&shipped.record(0), &rule);
                }
                rule
            }
            None => {
                if live_user_rules(&tx)? >= MAX_RULES {
                    return Err(RuleWriteError::LimitReached);
                }
                ContentRuleRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    origin: organization::RuleOrigin::User,
                    builtin_version: None,
                    customized_fields: Vec::new(),
                    name: name.to_owned(),
                    definition_json: draft.definition_json.clone(),
                    device_scope: draft.device_scope.clone(),
                    schema_version: SCHEMA_VERSION,
                    effective_from: now_ms,
                    deleted_at: None,
                    base_stamp: None,
                }
            }
        };
        let placement = match stored_placement(&tx, &rule.id)? {
            Some(placement) => RulePlacementRecord {
                enabled: draft.enabled,
                ..placement
            },
            None => {
                let mut placement = default_placement(&rule);
                if placement.order_key.is_empty() {
                    let last = siblings(&tx, origin_name(&rule), &rule.id)?
                        .pop()
                        .map(|(_, key)| key);
                    placement.order_key = order_key_between(last.as_deref(), None);
                }
                RulePlacementRecord {
                    enabled: draft.enabled,
                    ..placement
                }
            }
        };
        let id = rule.id.clone();
        write_rule(&tx, key, rule)?;
        organization::write(&tx, key, Record::ContentRulePlacement(placement))?;
        tx.commit()?;
        Ok(id)
    }

    /// Deletes (`true`) or restores a rule; a deleted rule keeps its
    /// definition and stays restorable.
    pub fn set_content_rule_deleted(
        &self,
        key: &ItemKey,
        id: &str,
        deleted: bool,
        now_ms: i64,
    ) -> Result<bool, RuleWriteError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let rule = stored_rule(&tx, id)?.ok_or(RuleWriteError::NotFound)?;
        if rule.deleted_at.is_some() == deleted {
            return Ok(false);
        }
        if !deleted
            && rule.origin == organization::RuleOrigin::User
            && live_user_rules(&tx)? >= MAX_RULES
        {
            return Err(RuleWriteError::LimitReached);
        }
        write_rule(
            &tx,
            key,
            ContentRuleRecord {
                deleted_at: deleted.then_some(now_ms),
                ..rule
            },
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn set_content_rule_enabled(
        &self,
        key: &ItemKey,
        id: &str,
        enabled: bool,
    ) -> Result<(), RuleWriteError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let rule = stored_rule(&tx, id)?
            .filter(|rule| rule.deleted_at.is_none())
            .ok_or(RuleWriteError::NotFound)?;
        let placement = stored_placement(&tx, id)?.unwrap_or_else(|| default_placement(&rule));
        organization::write(
            &tx,
            key,
            Record::ContentRulePlacement(RulePlacementRecord {
                enabled,
                ..placement
            }),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Moves a rule right after `after_id` among the rules of its tier, or to
    /// the top. Only the moved rule's placement is rewritten, so concurrent
    /// moves of different rules converge.
    pub fn reorder_content_rule(
        &self,
        key: &ItemKey,
        id: &str,
        after_id: Option<&str>,
    ) -> Result<(), RuleWriteError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let rule = stored_rule(&tx, id)?
            .filter(|rule| rule.deleted_at.is_none())
            .ok_or(RuleWriteError::NotFound)?;
        let siblings = siblings(&tx, origin_name(&rule), id)?;
        let at = match after_id {
            None => 0,
            Some(after) => {
                siblings
                    .iter()
                    .position(|(sibling, _)| sibling == after)
                    .ok_or(RuleWriteError::NotFound)?
                    + 1
            }
        };
        let low = at.checked_sub(1).map(|index| siblings[index].1.as_str());
        let high = siblings.get(at).map(|(_, key)| key.as_str());
        let placement = stored_placement(&tx, id)?.unwrap_or_else(|| default_placement(&rule));
        organization::write(
            &tx,
            key,
            Record::ContentRulePlacement(RulePlacementRecord {
                order_key: order_key_between(low, high),
                ..placement
            }),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Puts a built-in rule back to what ships, live again.
    pub fn reset_builtin_content_rule(
        &self,
        key: &ItemKey,
        id: &str,
        now_ms: i64,
    ) -> Result<(), RuleWriteError> {
        let shipped = builtin::builtin(id).ok_or(RuleWriteError::NotBuiltin)?;
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let existing = stored_rule(&tx, id)?;
        let effective_from = existing.as_ref().map_or(now_ms, |rule| {
            next_effective_from(rule.effective_from, now_ms)
        });
        write_rule(&tx, key, shipped.record(effective_from))?;
        if stored_placement(&tx, id)?.is_none() {
            organization::write(&tx, key, Record::ContentRulePlacement(shipped.placement()))?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Makes a kept losing definition the rule's definition again; answers
    /// the rule id.
    pub fn restore_content_rule_conflict(
        &self,
        key: &ItemKey,
        conflict_id: &str,
        now_ms: i64,
    ) -> Result<String, RuleWriteError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let (rule_id, definition_json): (String, String) = tx
            .query_row(
                "SELECT rule_id, definition_json FROM content_rule_conflicts WHERE id = ?1",
                [conflict_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(RuleWriteError::NotFound)?;
        let rule = stored_rule(&tx, &rule_id)?.ok_or(RuleWriteError::NotFound)?;
        let mut restored = ContentRuleRecord {
            effective_from: next_effective_from(rule.effective_from, now_ms),
            definition_json,
            deleted_at: None,
            ..rule
        };
        if let Some(shipped) = builtin::builtin(&rule_id) {
            restored.customized_fields = customized(&shipped.record(0), &restored);
        }
        write_rule(&tx, key, restored)?;
        tx.execute(
            "DELETE FROM content_rule_conflicts WHERE id = ?1",
            [conflict_id],
        )?;
        tx.commit()?;
        Ok(rule_id)
    }

    /// Writes each built-in rule this device has never seen, and brings older
    /// shipped versions up to date in the fields the user has not changed.
    /// A deleted built-in stays deleted. Answers whether anything was written.
    pub fn seed_builtin_content_rules(
        &self,
        key: &ItemKey,
        now_ms: i64,
    ) -> Result<bool, StoreError> {
        let mut conn = self.conn()?;
        let mut current = conn.prepare_cached(
            "SELECT builtin_version, deleted_at IS NULL FROM content_rules WHERE id = ?1",
        )?;
        let mut needed = false;
        for shipped in builtin::builtins() {
            let found: Option<(Option<u32>, bool)> = current
                .query_row([shipped.id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            needed |= found.is_none_or(|(version, live)| {
                live && version.is_some_and(|version| version < shipped.version)
            });
        }
        drop(current);
        if !needed {
            return Ok(false);
        }
        let tx = write_tx(&mut conn)?;
        let mut written = false;
        for shipped in builtin::builtins() {
            match stored_rule(&tx, shipped.id)? {
                None => {
                    write_rule(&tx, key, shipped.record(now_ms))?;
                    if stored_placement(&tx, shipped.id)?.is_none() {
                        organization::write(
                            &tx,
                            key,
                            Record::ContentRulePlacement(shipped.placement()),
                        )?;
                    }
                    written = true;
                }
                Some(existing)
                    if existing.deleted_at.is_none()
                        && existing
                            .builtin_version
                            .is_some_and(|v| v < shipped.version) =>
                {
                    let mut upgraded = shipped.record(existing.effective_from);
                    for field in &existing.customized_fields {
                        match field.as_str() {
                            FIELD_NAME => upgraded.name.clone_from(&existing.name),
                            FIELD_DEFINITION => {
                                upgraded
                                    .definition_json
                                    .clone_from(&existing.definition_json);
                            }
                            FIELD_SCOPE => upgraded.device_scope.clone_from(&existing.device_scope),
                            _ => {}
                        }
                    }
                    if upgraded.definition_json != existing.definition_json {
                        upgraded.effective_from =
                            next_effective_from(existing.effective_from, now_ms);
                    }
                    upgraded.customized_fields = existing.customized_fields;
                    write_rule(&tx, key, upgraded)?;
                    written = true;
                }
                Some(_) => {}
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Live, unexpired clips newest first, after `after`, without content.
    pub fn rule_subjects(
        &self,
        after: Option<&SubjectCursor>,
        limit: u32,
    ) -> Result<Vec<RuleSubject>, StoreError> {
        let conn = self.conn()?;
        let (at, id) = after.map_or((i64::MAX, ""), |cursor| {
            (cursor.created_at, cursor.id.as_str())
        });
        let mut statement = conn.prepare_cached(concat!(
            subject_query!(),
            " AND (ci.created_at < ?1 OR (ci.created_at = ?1 AND ci.id < ?2)) \
             ORDER BY ci.created_at DESC, ci.id DESC LIMIT ?3"
        ))?;
        let subjects = statement
            .query_map(params![at, id, i64::from(limit)], subject_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(subjects)
    }

    /// One live, unexpired clip as a rule subject.
    pub fn rule_subject(&self, id: &str) -> Result<Option<RuleSubject>, StoreError> {
        let conn = self.conn()?;
        let mut statement = conn.prepare_cached(concat!(subject_query!(), " AND ci.id = ?1"))?;
        Ok(statement.query_row([id], subject_row).optional()?)
    }

    /// The plain text text conditions read from a text clip, cut to the
    /// regex prefix; `None` for other content.
    pub fn rule_subject_text(
        &self,
        key: &ItemKey,
        subject: &RuleSubject,
    ) -> Result<Option<Zeroizing<String>>, StoreError> {
        if !copypaste_ipc::content_type::is_text(&subject.content_type) {
            return Ok(None);
        }
        let conn = self.conn()?;
        let sealed: Option<(Vec<u8>, Vec<u8>)> = conn
            .query_row(
                "SELECT content_ciphertext, nonce FROM clipboard_items \
                 WHERE id = ?1 AND deleted = 0",
                [&subject.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        drop(conn);
        let Some((ciphertext, nonce)) = sealed else {
            return Ok(None);
        };
        let bytes = crate::decrypt(&ciphertext, &nonce, key, &subject.id)
            .map_err(|_| StoreError::InvalidPayload)?;
        Ok(plain_text(&subject.content_type, bytes))
    }

    /// Adds rule tags to a live clip; they stay local, like capture's.
    pub fn add_rule_tags(&self, clip_id: &str, tag_ids: &[String]) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        crate::rules::apply::insert_tags(&tx, clip_id, tag_ids)?;
        tx.commit()?;
        Ok(())
    }

    /// Records that `rule_ids` acted on a live clip, for its trace and the
    /// rules' match counts.
    pub fn note_applied_rules(&self, clip_id: &str, rule_ids: &[String]) -> Result<(), StoreError> {
        let mut conn = self.conn()?;
        let tx = write_tx(&mut conn)?;
        let mut state = local_state::read(&tx, clip_id)?.unwrap_or_else(|| ClipLocalState {
            clip_id: clip_id.to_owned(),
            ..Default::default()
        });
        let mut applied: Vec<String> = state
            .applied_rules_json
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        for rule_id in rule_ids {
            if !applied.contains(rule_id) {
                applied.push(rule_id.clone());
            }
        }
        state.applied_rules_json = serde_json::to_string(&applied).ok();
        local_state::write(&tx, &state)?;
        tx.commit()?;
        Ok(())
    }
}

fn listed_rule(conn: &Connection, id: &str) -> Result<Option<StoredRule>, StoreError> {
    let Some(record) = stored_rule(conn, id)? else {
        return Ok(None);
    };
    let placement = stored_placement(conn, id)?.unwrap_or_else(|| default_placement(&record));
    let (stamp, author) = version_of(conn, id)?.unwrap_or_default();
    Ok(Some(StoredRule {
        record,
        enabled: placement.enabled,
        order_key: placement.order_key,
        stamp,
        author: Some(author).filter(|author| !author.is_empty()),
    }))
}
