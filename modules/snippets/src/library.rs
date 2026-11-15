//! The snippet library over synced records.
//!
//! Holds the rules the app core enforced before snippets became a module:
//! limits and validation, abbreviation uniqueness, ordering, Recently
//! Deleted and purge, collections, abbreviation renames that rewrite nested
//! references, ranked quick search, and conflict copies. Snippet metadata
//! and collections stay in memory; bodies are read one at a time.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use copypaste_module_sdk::{HostError, HostErrorCode, ModuleHostRequest, ReplacedRecord};

use crate::abbreviation::{self, AbbreviationError};
use crate::conflict;
use crate::host::{self, Host};
use crate::model::{self, Body, Collection, Meta, BODY, COLLECTION, META};
use crate::ranking::{self, RankSignals, SearchTier};

pub(crate) const MAX_TITLE_CHARS: usize = 200;
pub(crate) const MAX_BODY_BYTES: usize = 64 * 1024;
/// Plain and rich representations together.
pub(crate) const MAX_BODY_WITH_RICH_BYTES: usize = 256 * 1024;
/// Above this many snippets the library shows a warning.
pub(crate) const SOFT_SNIPPET_LIMIT: usize = 10_000;
pub(crate) const MAX_SNIPPETS: usize = 20_000;
pub(crate) const MAX_COLLECTIONS: usize = 200;
pub(crate) const MAX_COLLECTION_NAME_CHARS: usize = 64;
const MAX_COLLECTION_STYLE_CHARS: usize = 64;
/// How long a deleted snippet stays in Recently Deleted before it is purged.
pub(crate) const RECENTLY_DELETED_MS: i64 = 30 * 24 * 60 * 60 * 1000;
pub(crate) const DEFAULT_COLLECTION_ICON: &str = "folder";
pub(crate) const DEFAULT_COLLECTION_ACCENT: &str = "neutral";
/// Records sync without telling the module, so the index is read again
/// after this long.
const REFRESH: Duration = Duration::from_secs(30);
/// Usage counters are spread over this many local state values.
const USAGE_SHARDS: usize = 16;
const USAGE_SHARD_BYTES: usize = 120 * 1024;
const STATE_DEVICE: &str = "device_id";
/// Shown in conflict copy titles; the module cannot name the device that
/// made the replaced edit.
const UNKNOWN_DEVICE: &str = "unknown device";

#[derive(Debug)]
pub(crate) enum LibraryError {
    AbbreviationTaken { title: String },
    InvalidAbbreviation(AbbreviationError),
    EmptyBody,
    TooLarge,
    InvalidTitle,
    InvalidCollection,
    CollectionNameTaken { collection_id: String },
    LimitReached,
    NotFound,
    Host(HostError),
}

impl From<HostError> for LibraryError {
    fn from(error: HostError) -> Self {
        Self::Host(error)
    }
}

impl LibraryError {
    /// The message the runtime showed for the same refusal.
    pub(crate) fn message(&self) -> String {
        match self {
            Self::AbbreviationTaken { .. } => {
                "This abbreviation is already used by another snippet.".into()
            }
            Self::InvalidAbbreviation(_) => {
                "An abbreviation is 1 to 32 characters with no spaces.".into()
            }
            Self::EmptyBody => "A snippet needs some text.".into(),
            Self::TooLarge => "The snippet is larger than the limit.".into(),
            Self::InvalidTitle => "The title is too long or not valid.".into(),
            Self::InvalidCollection => "That collection is not valid.".into(),
            Self::CollectionNameTaken { .. } => {
                "A collection with this name already exists.".into()
            }
            Self::LimitReached => "The snippet library is full.".into(),
            Self::NotFound => "That snippet or collection no longer exists.".into(),
            Self::Host(error) => error.message.clone(),
        }
    }
}

pub(crate) type Result<T, E = LibraryError> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scope {
    All,
    /// No collection, or one that has been deleted.
    Unfiled,
    Collection(String),
    Pinned,
    RecentlyDeleted,
}

impl Scope {
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "pinned" => Self::Pinned,
            "unfiled" => Self::Unfiled,
            "deleted" => Self::RecentlyDeleted,
            other => match other.strip_prefix("c:") {
                Some(id) if !id.is_empty() => Self::Collection(id.to_owned()),
                _ => Self::All,
            },
        }
    }

    pub(crate) fn id(&self) -> String {
        match self {
            Self::All => "all".into(),
            Self::Unfiled => "unfiled".into(),
            Self::Collection(id) => format!("c:{id}"),
            Self::Pinned => "pinned".into(),
            Self::RecentlyDeleted => "deleted".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Draft {
    pub(crate) id: Option<String>,
    pub(crate) collection_id: Option<String>,
    pub(crate) title: String,
    pub(crate) abbreviation: Option<String>,
    pub(crate) body_text: String,
    pub(crate) body_rich: Option<String>,
    pub(crate) template: bool,
    pub(crate) pinned: bool,
    pub(crate) concealed: bool,
    /// The revision the editor started from; a newer stored body that this
    /// edit did not see is recognized as concurrent and kept as a copy.
    pub(crate) base_revision: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CollectionDraft {
    pub(crate) id: Option<String>,
    pub(crate) name: String,
    pub(crate) icon: String,
    pub(crate) accent: String,
}

/// A live collection with its display name and live snippet count.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CollectionView {
    pub(crate) id: String,
    pub(crate) collection: Collection,
    /// `name (2)` when concurrent creation on two devices left a shared name.
    pub(crate) display_name: String,
    pub(crate) count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Hit {
    pub(crate) id: String,
    pub(crate) tier: SearchTier,
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) meta: Meta,
    /// The normalized abbreviation of a live snippet.
    key: Option<String>,
}

impl Entry {
    fn new(meta: Meta) -> Self {
        let key = meta
            .abbreviation
            .as_deref()
            .filter(|_| meta.live())
            .map(abbreviation::normalize);
        Self { meta, key }
    }
}

/// Local usage counters: never synced.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Usage {
    pub(crate) count: u64,
    pub(crate) last_used_ms: Option<i64>,
}

pub(crate) struct Library {
    host: Arc<dyn Host>,
    entries: HashMap<String, Entry>,
    collections: HashMap<String, Collection>,
    loaded_at: Option<Instant>,
    /// `None` until the local state is read.
    usage: Option<Vec<HashMap<String, Usage>>>,
    device: String,
}

fn shard(id: &str) -> usize {
    id.bytes().fold(0_usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % USAGE_SHARDS
}

fn usage_key(shard: usize) -> String {
    format!("usage_{shard}")
}

pub(crate) fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn single_line(text: &str, max_chars: usize) -> bool {
    text.chars().count() <= max_chars && !text.chars().any(char::is_control)
}

const ORDER_DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn order_digit(byte: u8) -> usize {
    ORDER_DIGITS
        .iter()
        .position(|digit| *digit == byte)
        .unwrap_or(if byte < b'0' {
            0
        } else {
            ORDER_DIGITS.len() - 1
        })
}

/// A key strictly between `low` and `high` (fractional indexing over base-62
/// digits), so a move rewrites one record. `None` is the open end.
pub(crate) fn order_key_between(low: Option<&str>, high: Option<&str>) -> String {
    fn midpoint(low: &[usize], high: Option<&[usize]>, out: &mut Vec<usize>) {
        if let Some(high) = high {
            let shared = high
                .iter()
                .enumerate()
                .take_while(|(at, digit)| low.get(*at).copied().unwrap_or(0) == **digit)
                .count();
            if shared > 0 {
                out.extend_from_slice(&high[..shared]);
                return midpoint(low.get(shared..).unwrap_or(&[]), Some(&high[shared..]), out);
            }
        }
        let below = low.first().copied().unwrap_or(0);
        let above = high
            .and_then(|high| high.first().copied())
            .unwrap_or(ORDER_DIGITS.len());
        if above > below + 1 {
            out.push((below + above) / 2);
        } else if let Some(high) = high.filter(|high| high.len() > 1) {
            out.push(high[0]);
        } else {
            out.push(below);
            midpoint(low.get(1..).unwrap_or(&[]), None, out);
        }
    }
    let digits = |key: &str| key.bytes().map(order_digit).collect::<Vec<_>>();
    let low = low.map(digits).unwrap_or_default();
    let high = high.map(digits);
    let mut out = Vec::new();
    midpoint(&low, high.as_deref(), &mut out);
    out.into_iter()
        .map(|digit| ORDER_DIGITS[digit] as char)
        .collect()
}

impl Library {
    pub(crate) fn new(host: Arc<dyn Host>) -> Self {
        Self {
            host,
            entries: HashMap::new(),
            collections: HashMap::new(),
            loaded_at: None,
            usage: None,
            device: String::new(),
        }
    }

    pub(crate) fn host(&self) -> &dyn Host {
        self.host.as_ref()
    }

    /// The next read loads the index again.
    pub(crate) fn invalidate(&mut self) {
        self.loaded_at = None;
    }

    /// Loads the index when it was never read or may be stale. Returns
    /// whether it was read now.
    pub(crate) fn ensure(&mut self) -> Result<bool> {
        if self
            .loaded_at
            .is_some_and(|loaded| loaded.elapsed() < REFRESH)
        {
            return Ok(false);
        }
        self.load()?;
        Ok(true)
    }

    fn load(&mut self) -> Result<()> {
        let mut entries = HashMap::new();
        host::list(self.host(), META, |record| {
            let Some(id) = record.key.strip_prefix(META) else {
                return;
            };
            if let Ok(meta) = serde_json::from_str::<Meta>(&record.value_json) {
                entries.insert(id.to_owned(), Entry::new(meta));
            }
        })?;
        let mut collections = HashMap::new();
        host::list(self.host(), COLLECTION, |record| {
            let Some(id) = record.key.strip_prefix(COLLECTION) else {
                return;
            };
            if let Ok(collection) = serde_json::from_str::<Collection>(&record.value_json) {
                collections.insert(id.to_owned(), collection);
            }
        })?;
        self.entries = entries;
        self.collections = collections;
        self.loaded_at = Some(Instant::now());
        self.purge_expired(model::now_ms())?;
        Ok(())
    }

    // ---- local state ----

    fn load_state(&mut self) -> Result<()> {
        if self.usage.is_some() {
            return Ok(());
        }
        let mut keys: Vec<String> = (0..USAGE_SHARDS).map(usage_key).collect();
        keys.push(STATE_DEVICE.to_owned());
        let mut values = host::read_state(self.host(), &keys)?;
        let shards = (0..USAGE_SHARDS)
            .map(|at| {
                values
                    .remove(&usage_key(at))
                    .flatten()
                    .and_then(|json| {
                        serde_json::from_str::<HashMap<String, (u64, i64)>>(&json).ok()
                    })
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(id, (count, last))| {
                        (
                            id,
                            Usage {
                                count,
                                last_used_ms: (last > 0).then_some(last),
                            },
                        )
                    })
                    .collect()
            })
            .collect();
        self.device = match values.remove(STATE_DEVICE).flatten() {
            Some(device) => device,
            None => {
                let device = model::new_id();
                host::write_state(
                    self.host(),
                    BTreeMap::from([(STATE_DEVICE.to_owned(), device.clone())]),
                )?;
                device
            }
        };
        self.usage = Some(shards);
        Ok(())
    }

    /// This installation's random id, recorded as `created_device`.
    fn device(&mut self) -> Result<String> {
        self.load_state()?;
        Ok(self.device.clone())
    }

    fn last_used(&self, id: &str) -> Option<i64> {
        self.usage
            .as_ref()
            .and_then(|shards| shards[shard(id)].get(id))
            .and_then(|usage| usage.last_used_ms)
    }

    /// Bumps the local counters of a used snippet.
    pub(crate) fn record_use(&mut self, id: &str, at_ms: i64) -> Result<()> {
        self.load_state()?;
        if let Some(shards) = self.usage.as_mut() {
            let usage = shards[shard(id)].entry(id.to_owned()).or_default();
            usage.count = usage.count.saturating_add(1);
            usage.last_used_ms = usage.last_used_ms.max(Some(at_ms));
        }
        self.write_usage(shard(id))
    }

    /// Keeps the larger count and the later stamp, so importing the same
    /// counters twice changes nothing.
    pub(crate) fn merge_usage(&mut self, id: &str, count: u64, last: Option<i64>) -> Result<()> {
        self.load_state()?;
        if let Some(shards) = self.usage.as_mut() {
            let usage = shards[shard(id)].entry(id.to_owned()).or_default();
            usage.count = usage.count.max(count);
            usage.last_used_ms = usage.last_used_ms.max(last);
        }
        Ok(())
    }

    pub(crate) fn write_usage(&mut self, at: usize) -> Result<()> {
        let Some(shards) = self.usage.as_mut() else {
            return Ok(());
        };
        let encode = |shard: &HashMap<String, Usage>| {
            let plain: HashMap<&String, (u64, i64)> = shard
                .iter()
                .map(|(id, usage)| (id, (usage.count, usage.last_used_ms.unwrap_or(0))))
                .collect();
            serde_json::to_string(&plain).unwrap_or_default()
        };
        let mut json = encode(&shards[at]);
        while json.len() > USAGE_SHARD_BYTES {
            // The least recently used counters make room.
            let oldest = shards[at]
                .iter()
                .min_by_key(|(_, usage)| usage.last_used_ms)
                .map(|(id, _)| id.clone());
            let Some(oldest) = oldest else { break };
            shards[at].remove(&oldest);
            json = encode(&shards[at]);
        }
        host::write_state(self.host(), BTreeMap::from([(usage_key(at), json)]))?;
        Ok(())
    }

    pub(crate) fn write_all_usage(&mut self) -> Result<()> {
        for at in 0..USAGE_SHARDS {
            self.write_usage(at)?;
        }
        Ok(())
    }

    fn forget_usage(&mut self, id: &str) -> Result<()> {
        let removed = self
            .usage
            .as_mut()
            .and_then(|shards| shards[shard(id)].remove(id))
            .is_some();
        if removed {
            self.write_usage(shard(id))?;
        }
        Ok(())
    }

    // ---- reads ----

    pub(crate) fn meta(&self, id: &str) -> Option<&Meta> {
        self.entries
            .get(id)
            .map(|entry| &entry.meta)
            .filter(|meta| !meta.purged)
    }

    pub(crate) fn live(&self, id: &str) -> Result<&Meta> {
        self.meta(id)
            .filter(|meta| meta.deleted_at_ms.is_none())
            .ok_or(LibraryError::NotFound)
    }

    pub(crate) fn known(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    pub(crate) fn body(&self, id: &str) -> Result<Option<Body>> {
        Ok(host::get_json(self.host(), &format!("{BODY}{id}"))?)
    }

    /// The collection a snippet is shown in: `None` when unfiled or when
    /// its collection was deleted.
    pub(crate) fn filed_in<'a>(&self, meta: &'a Meta) -> Option<&'a str> {
        meta.collection_id
            .as_deref()
            .filter(|id| self.live_collection(id).is_ok())
    }

    pub(crate) fn live_collection(&self, id: &str) -> Result<&Collection> {
        self.collections
            .get(id)
            .filter(|collection| collection.deleted_at_ms.is_none())
            .ok_or(LibraryError::NotFound)
    }

    /// Snippets counted against the limits: live and recently deleted.
    pub(crate) fn snippet_total(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| !entry.meta.purged)
            .count()
    }

    /// Live snippets sharing a normalized abbreviation with another.
    pub(crate) fn duplicates(&self) -> HashSet<String> {
        let mut by_key: HashMap<&str, Vec<&String>> = HashMap::new();
        for (id, entry) in &self.entries {
            if let Some(key) = &entry.key {
                by_key.entry(key).or_default().push(id);
            }
        }
        by_key
            .into_values()
            .filter(|ids| ids.len() > 1)
            .flatten()
            .cloned()
            .collect()
    }

    fn abbreviation_owner(&self, wanted: &str, except: &str) -> Option<(String, String)> {
        let key = abbreviation::normalize(wanted);
        let mut owners: Vec<(&String, &Entry)> = self
            .entries
            .iter()
            .filter(|(id, entry)| *id != except && entry.key.as_deref() == Some(key.as_str()))
            .collect();
        owners.sort_by(|a, b| a.0.cmp(b.0));
        owners
            .first()
            .map(|(id, entry)| ((*id).clone(), entry.meta.title.clone()))
    }

    pub(crate) fn abbreviation_taken(&self, wanted: &str) -> bool {
        self.abbreviation_owner(wanted, "").is_some()
    }

    /// The live snippet an abbreviation names; among duplicates the most
    /// recently used.
    pub(crate) fn resolve_abbreviation(&self, wanted: &str) -> Option<String> {
        let key = abbreviation::normalize(wanted);
        self.entries
            .iter()
            .filter(|(_, entry)| entry.key.as_deref() == Some(key.as_str()))
            .max_by(|a, b| {
                self.last_used(a.0)
                    .cmp(&self.last_used(b.0))
                    .then_with(|| b.0.cmp(a.0))
            })
            .map(|(id, _)| id.clone())
    }

    /// Checks a new abbreviation; keeping the stored one is always allowed,
    /// so a duplicate left by a merge can still be edited.
    fn check_abbreviation(
        &self,
        id: &str,
        current: Option<&str>,
        wanted: Option<&str>,
    ) -> Result<()> {
        let Some(wanted) = wanted else {
            return Ok(());
        };
        abbreviation::validate(wanted).map_err(LibraryError::InvalidAbbreviation)?;
        if current.map(abbreviation::normalize) == Some(abbreviation::normalize(wanted)) {
            return Ok(());
        }
        match self.abbreviation_owner(wanted, id) {
            Some((_, title)) => Err(LibraryError::AbbreviationTaken { title }),
            None => Ok(()),
        }
    }

    fn last_order_key(&self, collection_id: Option<&str>) -> String {
        let last = self
            .entries
            .values()
            .filter(|entry| entry.meta.collection_id.as_deref() == collection_id)
            .map(|entry| entry.meta.order_key.as_str())
            .max();
        order_key_between(last, None)
    }

    fn last_pin_order(&self) -> f64 {
        self.entries
            .values()
            .filter(|entry| entry.meta.pinned && !entry.meta.purged)
            .filter_map(|entry| entry.meta.pin_order)
            .fold(None, |max: Option<f64>, order| {
                Some(max.map_or(order, |max| max.max(order)))
            })
            .map_or(1.0, |last| last + 1.0)
    }

    /// Snippets of a list scope in display order.
    pub(crate) fn page(&self, scope: &Scope) -> Vec<(&String, &Meta)> {
        let mut rows: Vec<(&String, &Meta)> = self
            .entries
            .iter()
            .map(|(id, entry)| (id, &entry.meta))
            .filter(|(_, meta)| !meta.purged)
            .filter(|(_, meta)| match scope {
                Scope::All => meta.deleted_at_ms.is_none(),
                Scope::Unfiled => meta.deleted_at_ms.is_none() && self.filed_in(meta).is_none(),
                Scope::Collection(id) => {
                    meta.deleted_at_ms.is_none() && self.filed_in(meta) == Some(id.as_str())
                }
                Scope::Pinned => meta.deleted_at_ms.is_none() && meta.pinned,
                Scope::RecentlyDeleted => meta.deleted_at_ms.is_some(),
            })
            .collect();
        match scope {
            Scope::Pinned => rows.sort_by(|a, b| {
                a.1.pin_order
                    .partial_cmp(&b.1.pin_order)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(b.0))
            }),
            Scope::RecentlyDeleted => rows.sort_by(|a, b| {
                b.1.deleted_at_ms
                    .cmp(&a.1.deleted_at_ms)
                    .then_with(|| b.0.cmp(a.0))
            }),
            _ => rows.sort_by(|a, b| a.1.order_key.cmp(&b.1.order_key).then_with(|| a.0.cmp(b.0))),
        }
        rows
    }

    /// Live collections in display order.
    pub(crate) fn collections(&self) -> Vec<CollectionView> {
        let mut live: Vec<(&String, &Collection)> = self
            .collections
            .iter()
            .filter(|(_, collection)| collection.deleted_at_ms.is_none())
            .collect();
        live.sort_by(|a, b| a.1.order_key.cmp(&b.1.order_key).then_with(|| a.0.cmp(b.0)));
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for entry in self.entries.values().filter(|entry| entry.meta.live()) {
            if let Some(id) = entry.meta.collection_id.as_deref() {
                *counts.entry(id).or_default() += 1;
            }
        }
        let mut seen: HashMap<String, u32> = HashMap::new();
        live.into_iter()
            .map(|(id, collection)| {
                let shared = seen
                    .entry(abbreviation::normalize(&collection.name))
                    .or_default();
                *shared += 1;
                let display_name = if *shared == 1 {
                    collection.name.clone()
                } else {
                    format!("{} ({shared})", collection.name)
                };
                CollectionView {
                    id: id.clone(),
                    collection: collection.clone(),
                    display_name,
                    count: counts.get(id.as_str()).copied().unwrap_or(0),
                }
            })
            .collect()
    }

    /// Ranked matches for a typed query: exact abbreviation, abbreviation
    /// prefix, title, then full text; pins and local recency break ties.
    pub(crate) fn quick_search(&mut self, query: &str, limit: usize) -> Result<Vec<Hit>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let _ = self.load_state();
        let mut tiers: HashMap<String, (SearchTier, usize)> = HashMap::new();
        let mut found = |id: String, tier: SearchTier| {
            let position = tiers.len();
            tiers.entry(id).or_insert((tier, position));
        };
        if !query.chars().any(char::is_whitespace) {
            let wanted = abbreviation::normalize(query);
            let mut keyed: Vec<(&str, &String)> = self
                .entries
                .iter()
                .filter_map(|(id, entry)| entry.key.as_deref().map(|key| (key, id)))
                .filter(|(key, _)| key.starts_with(&wanted))
                .collect();
            keyed.sort();
            for (key, id) in keyed.into_iter().take(limit) {
                if let Some(tier) = ranking::abbreviation_tier(&wanted, key) {
                    found(id.clone(), tier);
                }
            }
        }
        let search_limit = u32::try_from(limit)
            .unwrap_or(u32::MAX)
            .clamp(1, copypaste_module_sdk::MAX_SEARCH_PAGE);
        for (prefix, tier) in [(META, SearchTier::Title), (BODY, SearchTier::FullText)] {
            let page: copypaste_module_sdk::RecordPage = host::request(
                self.host(),
                ModuleHostRequest::RecordSearch {
                    query: query.to_owned(),
                    prefix: Some(prefix.to_owned()),
                    limit: search_limit,
                },
            )?;
            for record in page.records {
                if let Some(id) = record.key.strip_prefix(prefix) {
                    if self.live(id).is_ok() {
                        found(id.to_owned(), tier);
                    }
                }
            }
        }
        let mut hits: Vec<(RankSignals, Hit)> = tiers
            .into_iter()
            .filter_map(|(id, (tier, position))| {
                let meta = self.live(&id).ok()?;
                Some((
                    RankSignals {
                        tier,
                        pinned: meta.pinned,
                        last_used_at: self.last_used(&id),
                        position,
                    },
                    Hit { id, tier },
                ))
            })
            .collect();
        hits.sort_by(|a, b| ranking::compare(&a.0, &b.0));
        Ok(hits.into_iter().take(limit).map(|(_, hit)| hit).collect())
    }

    /// Live snippets for an empty Quick Paste query: pinned first, then the
    /// most recently used, then library order.
    pub(crate) fn recent(&mut self, limit: usize) -> Vec<String> {
        let _ = self.load_state();
        let mut rows: Vec<(&String, &Meta)> = self
            .entries
            .iter()
            .map(|(id, entry)| (id, &entry.meta))
            .filter(|(_, meta)| meta.live())
            .collect();
        rows.sort_by(|a, b| {
            b.1.pinned
                .cmp(&a.1.pinned)
                .then_with(|| {
                    a.1.pin_order
                        .partial_cmp(&b.1.pin_order)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| self.last_used(b.0).cmp(&self.last_used(a.0)))
                .then_with(|| a.1.order_key.cmp(&b.1.order_key))
                .then_with(|| a.0.cmp(b.0))
        });
        rows.into_iter()
            .take(limit)
            .map(|(id, _)| id.clone())
            .collect()
    }

    // ---- writes ----

    fn write_meta(&mut self, id: &str, meta: Meta) -> Result<()> {
        host::put(self.host(), &format!("{META}{id}"), &meta)?;
        self.entries.insert(id.to_owned(), Entry::new(meta));
        Ok(())
    }

    /// Writes a new body revision descending from `base` (or the stored one).
    fn write_body(
        &self,
        id: &str,
        current: Option<&Body>,
        text: String,
        rich_html: Option<String>,
        base: Option<&str>,
        concealed: bool,
    ) -> Result<()> {
        let current_revision =
            current.map(|current| (current.revision.as_str(), current.ancestors.as_slice()));
        let base = base.or(current_revision.map(|(revision, _)| revision));
        let ancestors = base.map_or_else(Vec::new, |base| {
            conflict::ancestors_from(base, current_revision)
        });
        let body = Body::new(
            conflict::new_revision(),
            ancestors,
            text,
            rich_html,
            concealed,
        );
        host::put(self.host(), &format!("{BODY}{id}"), &body)?;
        Ok(())
    }

    /// Saves `draft`; a new snippet takes `new_id` unless some snippet ever
    /// had it.
    pub(crate) fn save(&mut self, draft: &Draft, new_id: Option<&str>) -> Result<String> {
        let title = draft.title.trim();
        if !single_line(title, MAX_TITLE_CHARS) {
            return Err(LibraryError::InvalidTitle);
        }
        let text = normalize_line_endings(&draft.body_text);
        if text.trim().is_empty() {
            return Err(LibraryError::EmptyBody);
        }
        let wanted_abbreviation = draft
            .abbreviation
            .as_deref()
            .map(str::trim)
            .filter(|abbreviation| !abbreviation.is_empty());
        let existing = match &draft.id {
            Some(id) => Some(self.meta(id).cloned().ok_or(LibraryError::NotFound)?),
            None => None,
        };
        if existing.is_none() && self.snippet_total() >= MAX_SNIPPETS {
            return Err(LibraryError::LimitReached);
        }
        if let Some(collection_id) = &draft.collection_id {
            self.live_collection(collection_id)?;
        }
        let id = match (&draft.id, new_id) {
            (Some(id), _) => id.clone(),
            (None, Some(wanted)) if model::valid_id(wanted) && !self.known(wanted) => {
                wanted.to_owned()
            }
            (None, _) => model::new_id(),
        };
        self.check_abbreviation(
            &id,
            existing
                .as_ref()
                .and_then(|existing| existing.abbreviation.as_deref()),
            wanted_abbreviation,
        )?;

        let current = if existing.is_some() {
            self.body(&id)?
        } else {
            None
        };
        let unchanged_text = current
            .as_ref()
            .is_some_and(|current| current.content() == text);
        let rich_html = if draft.template {
            None
        } else if unchanged_text {
            draft.body_rich.clone().or_else(|| {
                current
                    .as_ref()
                    .and_then(|current| current.rich_html.clone())
            })
        } else {
            draft.body_rich.clone()
        };
        if text.len() > MAX_BODY_BYTES
            || text.len() + rich_html.as_ref().map_or(0, String::len) > MAX_BODY_WITH_RICH_BYTES
        {
            return Err(LibraryError::TooLarge);
        }

        let order_key = match &existing {
            Some(existing) if existing.collection_id == draft.collection_id => {
                existing.order_key.clone()
            }
            _ => self.last_order_key(draft.collection_id.as_deref()),
        };
        let pin_order = match &existing {
            _ if !draft.pinned => None,
            Some(existing) if existing.pinned => existing.pin_order,
            _ => Some(self.last_pin_order()),
        };
        let (created_at_ms, created_device) = match &existing {
            Some(existing) => (existing.created_at_ms, existing.created_device.clone()),
            None => (model::now_ms(), self.device()?),
        };
        let stored_unchanged = current.as_ref().is_some_and(|current| {
            current.content() == text
                && current.rich_html == rich_html
                && current.secret_text.is_some() == draft.concealed
        });
        if !stored_unchanged {
            self.write_body(
                &id,
                current.as_ref(),
                text.clone(),
                rich_html,
                draft.base_revision.as_deref(),
                draft.concealed,
            )?;
        }
        self.write_meta(
            &id,
            Meta {
                title: title.to_owned(),
                abbreviation: wanted_abbreviation.map(str::to_owned),
                collection_id: draft.collection_id.clone(),
                template: draft.template,
                pinned: draft.pinned,
                pin_order,
                order_key,
                concealed: draft.concealed,
                deleted_at_ms: None,
                purged: false,
                created_at_ms,
                created_device,
                preview: model::preview(&text, draft.concealed),
            },
        )?;
        Ok(id)
    }

    /// Moves live snippets to Recently Deleted; returns how many moved.
    pub(crate) fn delete(&mut self, ids: &[String]) -> Result<usize> {
        let now = model::now_ms();
        let mut deleted = 0;
        for id in ids {
            if let Ok(meta) = self.live(id) {
                let meta = Meta {
                    deleted_at_ms: Some(now),
                    ..meta.clone()
                };
                self.write_meta(id, meta)?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    /// Brings snippets back from Recently Deleted.
    pub(crate) fn restore(&mut self, ids: &[String]) -> Result<usize> {
        let mut restored = 0;
        for id in ids {
            let Some(meta) = self.meta(id).filter(|meta| meta.deleted_at_ms.is_some()) else {
                continue;
            };
            let meta = Meta {
                deleted_at_ms: None,
                ..meta.clone()
            };
            self.write_meta(id, meta)?;
            restored += 1;
        }
        Ok(restored)
    }

    /// Erases snippets on every device: a content-free metadata stub stays
    /// so a late edit cannot bring one back, and the body is deleted.
    pub(crate) fn purge(&mut self, ids: &[String]) -> Result<usize> {
        let now = model::now_ms();
        let mut purged = 0;
        for id in ids {
            if self.meta(id).is_some() {
                self.purge_one(id, now)?;
                purged += 1;
            }
        }
        Ok(purged)
    }

    fn purge_one(&mut self, id: &str, now: i64) -> Result<()> {
        let Some(meta) = self.meta(id).cloned() else {
            return Ok(());
        };
        self.write_meta(
            id,
            Meta {
                deleted_at_ms: meta.deleted_at_ms.or(Some(now)),
                purged: true,
                order_key: meta.order_key,
                created_at_ms: meta.created_at_ms,
                created_device: meta.created_device,
                ..Meta::default()
            },
        )?;
        host::delete(self.host(), &format!("{BODY}{id}"))?;
        self.forget_usage(id)
    }

    /// Purges snippets that have been in Recently Deleted for the full window.
    pub(crate) fn purge_expired(&mut self, now_ms: i64) -> Result<usize> {
        let cutoff = now_ms.saturating_sub(RECENTLY_DELETED_MS);
        let expired: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                !entry.meta.purged && entry.meta.deleted_at_ms.is_some_and(|at| at <= cutoff)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.purge_one(id, now_ms)?;
        }
        Ok(expired.len())
    }

    fn move_one(&mut self, id: &str, collection_id: Option<&str>) -> Result<()> {
        let Some(meta) = self.meta(id).cloned() else {
            return Ok(());
        };
        let order_key = self.last_order_key(collection_id);
        self.write_meta(
            id,
            Meta {
                collection_id: collection_id.map(str::to_owned),
                order_key,
                ..meta
            },
        )
    }

    /// Files snippets in a collection (`None` = Unfiled), appended in order.
    pub(crate) fn move_to(&mut self, ids: &[String], collection_id: Option<&str>) -> Result<usize> {
        if let Some(collection_id) = collection_id {
            self.live_collection(collection_id)?;
        }
        let mut moved = 0;
        for id in ids {
            let differs = self
                .meta(id)
                .is_some_and(|meta| meta.collection_id.as_deref() != collection_id);
            if differs {
                self.move_one(id, collection_id)?;
                moved += 1;
            }
        }
        Ok(moved)
    }

    /// Places a snippet right before `before` in its collection, or last.
    pub(crate) fn reorder(&mut self, id: &str, before: Option<&str>) -> Result<()> {
        let meta = self.live(id)?.clone();
        let high = match before {
            Some(before) => {
                let next = self.live(before)?;
                if next.collection_id != meta.collection_id {
                    return Err(LibraryError::InvalidCollection);
                }
                Some(next.order_key.clone())
            }
            None => None,
        };
        let low = self
            .entries
            .iter()
            .filter(|(other, entry)| {
                *other != id
                    && entry.meta.collection_id == meta.collection_id
                    && high
                        .as_deref()
                        .is_none_or(|high| entry.meta.order_key.as_str() < high)
            })
            .map(|(_, entry)| entry.meta.order_key.clone())
            .max();
        let order_key = order_key_between(low.as_deref(), high.as_deref());
        self.write_meta(id, Meta { order_key, ..meta })
    }

    /// Places a pinned snippet right before `before` among the pins, or last.
    pub(crate) fn reorder_pin(&mut self, id: &str, before: Option<&str>) -> Result<()> {
        let meta = self.live(id)?.clone();
        let high = match before {
            Some(before) => Some(self.live(before)?.pin_order.ok_or(LibraryError::NotFound)?),
            None => None,
        };
        let low = self
            .entries
            .iter()
            .filter(|(other, entry)| *other != id && entry.meta.pinned && entry.meta.live())
            .filter_map(|(_, entry)| entry.meta.pin_order)
            .filter(|order| high.is_none_or(|high| *order < high))
            .fold(None, |max: Option<f64>, order| {
                Some(max.map_or(order, |max| max.max(order)))
            });
        let pin_order = match (low, high) {
            (Some(low), Some(high)) => low + (high - low) / 2.0,
            (None, Some(high)) => high - 1.0,
            (Some(low), None) => low + 1.0,
            (None, None) => 1.0,
        };
        self.write_meta(
            id,
            Meta {
                pinned: true,
                pin_order: Some(pin_order),
                ..meta
            },
        )
    }

    /// Pins a snippet at the end of the Pinned section, or unpins it.
    pub(crate) fn set_pinned(&mut self, id: &str, pinned: bool) -> Result<()> {
        let meta = self.live(id)?.clone();
        if meta.pinned == pinned {
            return Ok(());
        }
        let pin_order = pinned.then(|| self.last_pin_order());
        self.write_meta(
            id,
            Meta {
                pinned,
                pin_order,
                ..meta
            },
        )
    }

    /// Template snippets other than `except` whose body names `needle`; the
    /// caller decides which really reference it.
    fn reference_candidates(&self, needle: &str, except: &str) -> Result<Vec<(String, Body)>> {
        let mut ids: Vec<&String> = self
            .entries
            .iter()
            .filter(|(id, entry)| *id != except && !entry.meta.purged && entry.meta.template)
            .map(|(id, _)| id)
            .collect();
        ids.sort();
        let mut found = Vec::new();
        for id in ids {
            if let Some(body) = self.body(id)? {
                if body.content().contains(needle) {
                    found.push((id.clone(), body));
                }
            }
        }
        Ok(found)
    }

    /// Ids of template snippets that insert snippet `id` by its abbreviation.
    pub(crate) fn references(&self, id: &str) -> Result<Vec<String>> {
        let Some(previous) = self.meta(id).and_then(|meta| meta.abbreviation.clone()) else {
            return Ok(Vec::new());
        };
        let key = abbreviation::normalize(&previous);
        Ok(self
            .reference_candidates(&previous, id)?
            .into_iter()
            .filter(|(_, body)| {
                copypaste_template::parse(body.content())
                    .rename_snippet_references(
                        &|reference| abbreviation::normalize(reference) == key,
                        "x",
                    )
                    .is_some()
            })
            .map(|(id, _)| id)
            .collect())
    }

    /// Renames (or clears) an abbreviation. With `rewrite`, every template
    /// body naming the old one is rewritten; returns how many were.
    pub(crate) fn rename_abbreviation(
        &mut self,
        id: &str,
        wanted: Option<&str>,
        rewrite: bool,
    ) -> Result<usize> {
        let meta = self.live(id)?.clone();
        let wanted = wanted.map(str::trim).filter(|wanted| !wanted.is_empty());
        self.check_abbreviation(id, meta.abbreviation.as_deref(), wanted)?;
        let previous = meta.abbreviation.clone();
        let mut rewritten = 0;
        if let (true, Some(previous), Some(to)) = (rewrite, &previous, wanted) {
            let key = abbreviation::normalize(previous);
            for (reference, body) in self.reference_candidates(previous, id)? {
                let Some(updated) = copypaste_template::parse(body.content())
                    .rename_snippet_references(
                        &|candidate| abbreviation::normalize(candidate) == key,
                        to,
                    )
                    .filter(|updated| updated != body.content())
                else {
                    continue;
                };
                if updated.len() > MAX_BODY_BYTES {
                    return Err(LibraryError::TooLarge);
                }
                let concealed = body.secret_text.is_some();
                self.write_body(
                    &reference,
                    Some(&body),
                    updated.clone(),
                    None,
                    None,
                    concealed,
                )?;
                if let Some(meta) = self.meta(&reference).cloned() {
                    let preview = model::preview(&updated, meta.concealed);
                    self.write_meta(&reference, Meta { preview, ..meta })?;
                }
                rewritten += 1;
            }
        }
        self.write_meta(
            id,
            Meta {
                abbreviation: wanted.map(str::to_owned),
                ..meta
            },
        )?;
        Ok(rewritten)
    }

    fn write_collection(&mut self, id: &str, collection: Collection) -> Result<()> {
        host::put(self.host(), &format!("{COLLECTION}{id}"), &collection)?;
        self.collections.insert(id.to_owned(), collection);
        Ok(())
    }

    /// Creates or renames a collection; a new one takes `new_id` when unused.
    pub(crate) fn save_collection(
        &mut self,
        draft: &CollectionDraft,
        new_id: Option<&str>,
    ) -> Result<String> {
        let name = draft.name.trim();
        let icon = Some(draft.icon.trim())
            .filter(|icon| !icon.is_empty())
            .unwrap_or(DEFAULT_COLLECTION_ICON);
        let accent = Some(draft.accent.trim())
            .filter(|accent| !accent.is_empty())
            .unwrap_or(DEFAULT_COLLECTION_ACCENT);
        if name.is_empty()
            || !single_line(name, MAX_COLLECTION_NAME_CHARS)
            || !single_line(icon, MAX_COLLECTION_STYLE_CHARS)
            || !single_line(accent, MAX_COLLECTION_STYLE_CHARS)
        {
            return Err(LibraryError::InvalidCollection);
        }
        let existing = match &draft.id {
            Some(id) => Some(self.live_collection(id)?.clone()),
            None => None,
        };
        let folded = abbreviation::normalize(name);
        let live: Vec<(&String, &Collection)> = self
            .collections
            .iter()
            .filter(|(_, collection)| collection.deleted_at_ms.is_none())
            .collect();
        if let Some((collection_id, _)) = live.iter().find(|(id, other)| {
            Some(*id) != draft.id.as_ref() && abbreviation::normalize(&other.name) == folded
        }) {
            return Err(LibraryError::CollectionNameTaken {
                collection_id: (*collection_id).clone(),
            });
        }
        if existing.is_none() && live.len() >= MAX_COLLECTIONS {
            return Err(LibraryError::LimitReached);
        }
        let (id, order_key) = match (existing, &draft.id) {
            (Some(existing), Some(id)) => (id.clone(), existing.order_key),
            _ => {
                let last = self
                    .collections
                    .values()
                    .map(|collection| collection.order_key.as_str())
                    .max();
                let order_key = order_key_between(last, None);
                let id = match new_id {
                    Some(wanted)
                        if model::valid_id(wanted) && !self.collections.contains_key(wanted) =>
                    {
                        wanted.to_owned()
                    }
                    _ => model::new_id(),
                };
                (id, order_key)
            }
        };
        self.write_collection(
            &id,
            Collection {
                name: name.to_owned(),
                icon: icon.to_owned(),
                accent: accent.to_owned(),
                order_key,
                deleted_at_ms: None,
            },
        )?;
        Ok(id)
    }

    /// Every snippet still filed in `collection_id`, live or recently deleted.
    fn filed(&self, collection_id: &str) -> Vec<String> {
        let mut ids: Vec<(&String, &Meta)> = self
            .entries
            .iter()
            .map(|(id, entry)| (id, &entry.meta))
            .filter(|(_, meta)| {
                !meta.purged && meta.collection_id.as_deref() == Some(collection_id)
            })
            .collect();
        ids.sort_by(|a, b| a.1.order_key.cmp(&b.1.order_key).then_with(|| a.0.cmp(b.0)));
        ids.into_iter().map(|(id, _)| id.clone()).collect()
    }

    /// Deletes a collection; its snippets move to Unfiled or, with
    /// `delete_snippets`, to Recently Deleted.
    pub(crate) fn delete_collection(&mut self, id: &str, delete_snippets: bool) -> Result<()> {
        let collection = self.live_collection(id)?.clone();
        let now = model::now_ms();
        for snippet in self.filed(id) {
            if !delete_snippets {
                self.move_one(&snippet, None)?;
            } else {
                self.delete(std::slice::from_ref(&snippet))?;
            }
        }
        self.write_collection(
            id,
            Collection {
                deleted_at_ms: Some(now),
                ..collection
            },
        )
    }

    /// Moves every snippet of `from` into `into` and deletes `from`.
    pub(crate) fn merge_collections(&mut self, from: &str, into: &str) -> Result<usize> {
        if from == into {
            return Err(LibraryError::InvalidCollection);
        }
        let source = self.live_collection(from)?.clone();
        self.live_collection(into)?;
        let filed = self.filed(from);
        for snippet in &filed {
            self.move_one(snippet, Some(into))?;
        }
        self.write_collection(
            from,
            Collection {
                deleted_at_ms: Some(model::now_ms()),
                ..source
            },
        )?;
        Ok(filed.len())
    }

    // ---- sync ----

    /// Handles the values sync replaced: a body replaced by a concurrent
    /// version is kept as a conflict copy, the same on every device. Cheap
    /// when there are none: one request. Returns whether any arrived.
    pub(crate) fn drain_conflicts(&mut self) -> Result<bool> {
        let replaced: Vec<ReplacedRecord> = match host::request(
            self.host(),
            ModuleHostRequest::RecordConflicts {
                limit: copypaste_module_sdk::MAX_RETAINED_CONFLICTS as u32,
            },
        ) {
            Ok(replaced) => replaced,
            Err(error) if error.code == HostErrorCode::Unsupported => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if replaced.is_empty() {
            return Ok(false);
        }
        // Something synced: the index is read again before it is used.
        self.load()?;
        for record in &replaced {
            let Some(id) = record.key.strip_prefix(BODY) else {
                continue;
            };
            let Ok(previous) = serde_json::from_str::<Body>(&record.value_json) else {
                continue;
            };
            let Some(current) = self.body(id)? else {
                continue;
            };
            if conflict::concurrent(&previous.version(), &current.version()) {
                self.keep_copy(id, &previous)?;
            }
        }
        let ids = replaced.iter().map(|record| record.id).collect();
        host::request::<()>(self.host(), ModuleHostRequest::ClearRecordConflicts { ids })?;
        Ok(true)
    }

    /// The losing body as a new snippet whose id derives from its revision.
    fn keep_copy(&mut self, id: &str, losing: &Body) -> Result<()> {
        let copy_id = conflict::copy_id(&losing.revision);
        if self.known(&copy_id) {
            return Ok(());
        }
        let now = model::now_ms();
        // A body seen before its metadata: unknown flags are taken as concealed.
        let original = self.entries.get(id).map(|entry| entry.meta.clone());
        let concealed = original.as_ref().is_none_or(|meta| meta.concealed);
        let text = losing.content().to_owned();
        let body = Body::new(
            losing.revision.clone(),
            Vec::new(),
            text.clone(),
            losing.rich_html.clone(),
            concealed,
        );
        host::put(self.host(), &format!("{BODY}{copy_id}"), &body)?;
        let device = self.device()?;
        self.write_meta(
            &copy_id,
            Meta {
                title: conflict::copy_title(
                    original.as_ref().map_or("", |meta| meta.title.as_str()),
                    UNKNOWN_DEVICE,
                    now,
                    MAX_TITLE_CHARS,
                ),
                abbreviation: None,
                collection_id: original
                    .as_ref()
                    .and_then(|meta| meta.collection_id.clone()),
                template: original.as_ref().is_some_and(|meta| meta.template),
                pinned: false,
                pin_order: None,
                order_key: original
                    .as_ref()
                    .map_or_else(|| "V".to_owned(), |meta| meta.order_key.clone()),
                concealed,
                deleted_at_ms: None,
                purged: false,
                created_at_ms: now,
                created_device: device,
                preview: model::preview(&text, concealed),
            },
        )
    }

    /// Writes a record unless its key is taken (migration never overwrites).
    pub(crate) fn put_absent<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<bool> {
        if host::get(self.host(), key)?.is_some() {
            return Ok(false);
        }
        host::put(self.host(), key, value)?;
        Ok(true)
    }

    pub(crate) fn has_collection(&self, id: &str) -> bool {
        self.collections.contains_key(id)
    }
}
