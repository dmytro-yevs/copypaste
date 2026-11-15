//! The bounded local access log: one entry of ids and counts per served
//! call, never content or queries. It lives in encrypted local state, is
//! never synced, and is written in batches while connections are open.
use crate::host::{self, Host};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const KEY: &str = "access_log";
const MAX_ENTRIES: usize = 500;
const MAX_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const MAX_BYTES: usize = 96 * 1024;
const MAX_IDS: usize = 10;
const FLUSH_EVERY: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) at: i64,
    pub(crate) grant: String,
    pub(crate) tool: String,
    #[serde(default)]
    pub(crate) item_ids: Vec<String>,
    pub(crate) result_count: u32,
    #[serde(default)]
    pub(crate) error: Option<String>,
}

#[derive(Default)]
pub(crate) struct AccessLog {
    /// Loaded on first use and dropped once flushed with no connection open.
    entries: Option<VecDeque<Entry>>,
    unsaved: usize,
}

impl AccessLog {
    fn load(&mut self, host: &dyn Host) -> &mut VecDeque<Entry> {
        self.entries.get_or_insert_with(|| {
            host::read_state(host, KEY)
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default()
        })
    }

    pub(crate) fn append(&mut self, host: &dyn Host, mut entry: Entry) {
        entry.item_ids.truncate(MAX_IDS);
        self.load(host).push_back(entry);
        self.unsaved += 1;
        if self.unsaved >= FLUSH_EVERY {
            self.flush(host);
        }
    }

    /// Writes pending entries, trimmed to the bounds.
    pub(crate) fn flush(&mut self, host: &dyn Host) {
        if self.unsaved == 0 {
            return;
        }
        let Some(entries) = self.entries.as_mut() else {
            return;
        };
        let newest = entries.back().map_or(0, |entry| entry.at);
        entries.retain(|entry| newest.saturating_sub(entry.at) <= MAX_AGE_MS);
        while entries.len() > MAX_ENTRIES {
            entries.pop_front();
        }
        let mut json = serde_json::to_string(&*entries).unwrap_or_default();
        while json.len() > MAX_BYTES && entries.pop_front().is_some() {
            json = serde_json::to_string(&*entries).unwrap_or_default();
        }
        if host::write_state(host, KEY, json).is_ok() {
            self.unsaved = 0;
        }
    }

    /// Flushes and frees the in-memory copy.
    pub(crate) fn release(&mut self, host: &dyn Host) {
        self.flush(host);
        if self.unsaved == 0 {
            self.entries = None;
        }
    }

    /// One grant's entries, newest first.
    pub(crate) fn of(&mut self, host: &dyn Host, grant: &str) -> Vec<Entry> {
        self.load(host)
            .iter()
            .rev()
            .filter(|entry| entry.grant == grant)
            .cloned()
            .collect()
    }

    pub(crate) fn forget(&mut self, host: &dyn Host, grant: &str) {
        let entries = self.load(host);
        let before = entries.len();
        entries.retain(|entry| entry.grant != grant);
        if entries.len() != before {
            self.unsaved += 1;
            self.flush(host);
        }
    }
}
