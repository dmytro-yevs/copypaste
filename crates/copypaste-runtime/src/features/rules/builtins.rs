//! Seeding and resetting built-in rules.
//!
//! Built-ins have well-known ids, so the copies two devices seed merge into
//! one rule each. Seeding is idempotent: it writes only a built-in this
//! store has never held, or an older shipped version, and a deleted
//! built-in stays deleted.

use copypaste_core::now_ms;
use copypaste_ipc::rules::RuleOp;
use copypaste_ipc::{LibraryKinds, Response};

use crate::Runtime;

pub(super) fn handle(runtime: &Runtime, id: u64, op: RuleOp) -> Response {
    let RuleOp::ResetBuiltin { id: rule_id } = op else {
        return crate::features::unsupported(id);
    };
    let started = now_ms();
    match runtime
        .store
        .reset_builtin_content_rule(&runtime.keyring.item_key(), &rule_id, started)
    {
        Ok(()) => {
            runtime.library_written(started, LibraryKinds::RULES);
            runtime.rule_response(id, &rule_id)
        }
        Err(error) => super::write_refusal(id, "reset_builtin_rule", &error),
    }
}

impl Runtime {
    /// Writes the built-in rules this store is missing. Every deadline sweep
    /// calls it, starting with the one at open; once they exist it only reads.
    /// The seeded records are offered to sync without a `Library` event, so
    /// a sweep arms no coalescing timer.
    pub(crate) fn seed_builtin_rules(&self) {
        let started = now_ms();
        match self
            .store
            .seed_builtin_content_rules(&self.keyring.item_key(), started)
        {
            Ok(true) => {
                tracing::debug!("built-in content rules seeded");
                self.p2p.node().note_local_version(started);
                self.p2p.wake();
                self.modules.note_version(started);
            }
            Ok(false) => {}
            Err(error) => tracing::warn!(?error, "built-in content rules could not be seeded"),
        }
    }
}
