//! Daemon startup: DEGRADED-mode loop, key/plan decision logic, config
//! loading, and persistent state files (device_id, private-mode flag).
//!
//! Split (ADR-017) into cohesive submodules; this file is a thin re-export
//! facade matching, symbol-for-symbol, the set `daemon/mod.rs` imports from
//! `startup::` today.

mod config_load;
mod degraded;
mod keyload;
mod state_files;

pub(crate) use config_load::load_config;
pub(crate) use degraded::run_degraded;
pub(crate) use keyload::{
    decide_db_startup, encrypted_db_exists, load_local_key_bounded, sweep_keys, DbStartupPlan,
    KeyLoad,
};
pub(crate) use state_files::{load_or_create_device_id, load_private_mode, persist_private_mode};
