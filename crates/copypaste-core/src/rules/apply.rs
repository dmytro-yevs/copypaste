//! Writing rule outcomes in the transaction that stores a clip.
//!
//! At capture every action this device decided is written: hide from AI,
//! rule tags, an owned expiry that syncs, and the sealed pre-transform
//! original. On receipt only the local projection is: the secret mask, hide
//! from AI, rule tags and a local-only expiry that never tombstones. Both
//! read one compiled rule set per store and rules revision.

use std::sync::{Arc, Mutex, PoisonError};

use rusqlite::{params, Connection};
use zeroize::Zeroizing;

use super::compile::{load_inputs, CompiledRuleSet};
use super::evaluate::{
    ContentTypes, EvalFlags, EvaluationContext, EvaluationInput, Expiry, PrivacyMarkers,
    RuleOutcome,
};
use super::model::CaptureOrigin;
use crate::storage::expiry::{self, Clocks, CLEAR_CLIPBOARD_AT_EXPIRY};
use crate::storage::local_state::{self, ClipLocalState};
use crate::storage::{IncomingItem, StoreError};

/// How long a capture keeps its pre-transform original.
pub const ORIGINAL_RETENTION_MS: i64 = 24 * 60 * 60 * 1000;

/// The compiled rules of one store at one rules revision.
#[derive(Debug, Clone)]
pub struct CurrentRules {
    pub set: Arc<CompiledRuleSet>,
    pub revision: i64,
}

struct CachedRules {
    generation: String,
    revision: i64,
    device_id: String,
    set: Arc<CompiledRuleSet>,
}

/// One slot: a process opens one store. The store's generation is part of
/// the key, so another store or a restored database never reads it.
static CACHED: Mutex<Option<CachedRules>> = Mutex::new(None);

/// Content kept so "Restore original" can undo a capture-time transform.
#[derive(Debug, Clone)]
pub struct Original {
    pub content: Zeroizing<Vec<u8>>,
    pub content_type: String,
}

impl Original {
    #[must_use]
    pub fn new(content: &[u8], content_type: &str) -> Self {
        Self {
            content: Zeroizing::new(content.to_vec()),
            content_type: content_type.to_owned(),
        }
    }
}

/// What a capture's rules write next to the stored clip.
#[derive(Debug, Clone, Default)]
pub struct CaptureEffects {
    pub hide_from_ai: bool,
    pub expiry: Option<Expiry>,
    pub tag_ids: Vec<String>,
    pub applied_rules: Vec<String>,
    pub flags: EvalFlags,
    pub rules_revision: Option<i64>,
    pub original: Option<Original>,
}

impl CaptureEffects {
    /// The effects of `outcome`. Secret travels in the payload metadata and
    /// transforms in the content, so neither is here.
    #[must_use]
    pub fn from_outcome(outcome: &RuleOutcome, rules_revision: i64) -> Self {
        Self {
            hide_from_ai: outcome.hide_from_ai,
            expiry: outcome.expiry,
            tag_ids: outcome.tag_ids.clone(),
            applied_rules: outcome.applied_rules.clone(),
            flags: outcome.flags,
            rules_revision: Some(rules_revision),
            original: None,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.hide_from_ai
            && self.expiry.is_none()
            && self.tag_ids.is_empty()
            && self.applied_rules.is_empty()
            && self.flags == EvalFlags::default()
            && self.original.is_none()
    }
}

/// The text content conditions read for a file: its name and its locator.
#[must_use]
pub fn file_subject(file: &crate::FileMetadata) -> String {
    match &file.source_reference {
        Some(reference) => format!("{}\n{reference}", file.filename),
        None => file.filename.clone(),
    }
}

pub(crate) fn current_rules(conn: &Connection) -> Result<CurrentRules, StoreError> {
    let (generation, revision, device_id): (String, i64, Option<String>) = conn
        .prepare_cached(
            "SELECT r.generation, r.rules, \
             (SELECT value FROM sync_device_state WHERE key = 'device_id') \
             FROM resource_revisions r WHERE r.singleton = 0",
        )?
        .query_row([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    let device_id = device_id.unwrap_or_default();
    if let Some(current) = CACHED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .filter(|current| {
            current.generation == generation
                && current.revision == revision
                && current.device_id == device_id
        })
    {
        return Ok(CurrentRules {
            set: Arc::clone(&current.set),
            revision,
        });
    }
    // Compiled outside the lock: the read must not wait behind a writer that
    // waits for the lock.
    let set = Arc::new(CompiledRuleSet::compile(load_inputs(conn)?, &device_id));
    *CACHED.lock().unwrap_or_else(PoisonError::into_inner) = Some(CachedRules {
        generation,
        revision,
        device_id,
        set: Arc::clone(&set),
    });
    Ok(CurrentRules { set, revision })
}

/// [`write_capture`] as the hook a store write runs before it commits. An
/// expiry counts from `captured_at`.
pub(crate) fn capture_writer<'a>(
    key: &'a crate::ItemKey,
    effects: Option<&'a CaptureEffects>,
    captured_at: i64,
) -> Option<impl Fn(&Connection, &str) -> Result<(), StoreError> + 'a> {
    effects.map(move |effects| {
        move |tx: &Connection, clip_id: &str| {
            write_capture(tx, key, clip_id, effects, Clocks::at(captured_at))
        }
    })
}

/// Writes a capture's effects for `clip_id` in the transaction that stored
/// it. The expiry is owned here: this device tombstones the clip at its
/// deadline, and the deadline syncs in the clip's note.
pub(crate) fn write_capture(
    tx: &Connection,
    key: &crate::ItemKey,
    clip_id: &str,
    effects: &CaptureEffects,
    clocks: Clocks,
) -> Result<(), StoreError> {
    if effects.is_empty() {
        return Ok(());
    }
    merge_state(
        tx,
        clip_id,
        false,
        effects.hide_from_ai,
        &effects.applied_rules,
        effects.flags,
        effects.rules_revision,
    )?;
    insert_tags(tx, clip_id, &effects.tag_ids)?;
    if let Some(expiry) = effects.expiry {
        expiry::set_owned_expiry(
            tx,
            key,
            clip_id,
            expiry.duration_ms,
            expiry.clear_clipboard,
            clocks,
        )?;
    }
    if let Some(original) = &effects.original {
        let (nonce, ciphertext) =
            crate::encrypt(&original.content, key, &format!("original:{clip_id}"))
                .map_err(|_| StoreError::InvalidPayload)?;
        tx.execute(
            "INSERT INTO rule_originals(clip_id, content_ciphertext, nonce, content_type, purge_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(clip_id) DO UPDATE SET \
             content_ciphertext = excluded.content_ciphertext, nonce = excluded.nonce, \
             content_type = excluded.content_type, purge_at = excluded.purge_at",
            params![
                clip_id,
                ciphertext,
                nonce,
                original.content_type,
                clocks.wall_ms.saturating_add(ORIGINAL_RETENTION_MS)
            ],
        )?;
    }
    Ok(())
}

/// Evaluates this device's rules on a clip version that was just stored and
/// writes the local projection of what matched. Rules newer than the clip
/// do not run, pinned and kept clips get no expiry, and an expiry derived
/// here only ever deletes the local copy. Answers whether a rule masked the
/// clip as secret.
pub(crate) fn write_receipt(
    tx: &Connection,
    incoming: &IncomingItem<'_>,
    clocks: Clocks,
) -> Result<bool, StoreError> {
    let rules = current_rules(tx)?;
    if rules.set.is_empty() {
        return Ok(false);
    }
    let is_text = copypaste_ipc::content_type::is_text(incoming.content_type);
    let file = incoming
        .payload_metadata
        .filter(|_| !is_text)
        .and_then(|json| crate::PayloadMetadata::from_json(json, incoming.content_type))
        .and_then(|metadata| metadata.file)
        .map(|file| file_subject(&file));
    let (text, byte_len) = if is_text {
        let text = incoming.search_text;
        (text, text.map_or(0, str::len))
    } else {
        let bytes = incoming.content_ciphertext.map_or(0, <[u8]>::len);
        (file.as_deref(), bytes)
    };
    let outcome = rules.set.evaluate(
        &EvaluationInput {
            text,
            content_types: ContentTypes::from_content_type(incoming.content_type),
            byte_len: byte_len as u64,
            source_app: incoming.app_bundle_id,
            markers: PrivacyMarkers::UNKNOWN,
            origin: CaptureOrigin::Received,
            created_at: incoming.created_at,
        },
        EvaluationContext::Receipt,
    );
    if outcome.is_noop() && outcome.flags == EvalFlags::default() {
        return Ok(false);
    }
    let protection = outcome.protection_subset();
    merge_state(
        tx,
        incoming.id,
        protection.secret,
        protection.hide_from_ai,
        &outcome.applied_rules,
        protection.flags,
        Some(rules.revision),
    )?;
    insert_tags(tx, incoming.id, &outcome.tag_ids)?;
    if let Some(expiry) = protection.expiry {
        let deadline =
            expiry::received_deadline(incoming.created_at, expiry.duration_ms, clocks.wall_ms);
        let flags = if expiry.clear_clipboard {
            CLEAR_CLIPBOARD_AT_EXPIRY
        } else {
            0
        };
        expiry::observe_deadline(
            tx,
            incoming.id,
            deadline,
            Some(expiry.duration_ms),
            flags,
            clocks,
        )?;
    }
    Ok(protection.secret)
}

/// Adds rule results to the clip's local state. Masks and flags are sticky;
/// the applied rules are the latest evaluation's.
fn merge_state(
    tx: &Connection,
    clip_id: &str,
    secret: bool,
    hide_from_ai: bool,
    applied_rules: &[String],
    flags: EvalFlags,
    rules_revision: Option<i64>,
) -> Result<(), StoreError> {
    let mut state = local_state::read(tx, clip_id)?.unwrap_or_else(|| ClipLocalState {
        clip_id: clip_id.to_owned(),
        ..Default::default()
    });
    state.rule_secret |= secret;
    state.rule_hidden_from_ai |= hide_from_ai;
    if !applied_rules.is_empty() {
        state.applied_rules_json = serde_json::to_string(applied_rules).ok();
    }
    state.eval_flags |= i64::from(flags.bits());
    state.rules_revision = rules_revision.or(state.rules_revision);
    local_state::write(tx, &state)?;
    Ok(())
}

fn insert_tags(tx: &Connection, clip_id: &str, tag_ids: &[String]) -> Result<(), StoreError> {
    let mut insert = tx.prepare_cached(
        "INSERT OR IGNORE INTO clip_rule_tags(clip_id, tag_id) SELECT ?1, ?2 \
         WHERE EXISTS (SELECT 1 FROM clipboard_items WHERE id = ?1 AND deleted = 0)",
    )?;
    for tag in tag_ids {
        insert.execute(params![clip_id, tag])?;
    }
    Ok(())
}
