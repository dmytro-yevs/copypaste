//! The live runtime-log level: one filter, swapped in place.

use std::sync::{Mutex, OnceLock};

use copypaste_ipc::LogVerbosity;
use tracing_subscriber::{reload, EnvFilter, Registry};

pub(crate) type FilterLayer = reload::Layer<EnvFilter, Registry>;
type FilterHandle = reload::Handle<EnvFilter, Registry>;

/// The filter for one level. Verbose levels apply to this product's targets
/// only (every crate is `copypaste_*`); dependencies stay at INFO, so a
/// verbose choice cannot flood the bounded files with library internals.
pub(crate) fn filter(level: LogVerbosity) -> EnvFilter {
    EnvFilter::new(match level {
        LogVerbosity::Error => "error",
        LogVerbosity::Warning => "warn",
        LogVerbosity::Info => "info",
        LogVerbosity::Debug => "info,copypaste=debug",
    })
}

fn name(level: LogVerbosity) -> &'static str {
    match level {
        LogVerbosity::Error => "error",
        LogVerbosity::Warning => "warning",
        LogVerbosity::Info => "info",
        LogVerbosity::Debug => "debug",
    }
}

/// A reloadable filter starting at `initial`, and the control that changes it.
pub(crate) fn reloadable(initial: EnvFilter, level: LogVerbosity) -> (FilterLayer, LevelControl) {
    let (layer, handle) = reload::Layer::new(initial);
    (
        layer,
        LevelControl {
            handle,
            current: Mutex::new(level),
        },
    )
}

pub(crate) struct LevelControl {
    handle: FilterHandle,
    current: Mutex<LogVerbosity>,
}

impl LevelControl {
    /// Swaps the filter; `false` when the level was already in effect.
    pub(crate) fn apply(&self, level: LogVerbosity) -> bool {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *current == level || self.handle.reload(filter(level)).is_err() {
            return false;
        }
        *current = level;
        true
    }
}

static CONTROL: OnceLock<LevelControl> = OnceLock::new();

pub(crate) fn install(control: LevelControl) {
    let _ = CONTROL.set(control);
}

/// Applies the user's log level to this process, live.
///
/// A no-op before [`crate::init`], when `RUST_LOG` overrides the setting, or
/// when the level is unchanged, so callers may pass every settings read.
pub fn set_level(level: LogVerbosity) {
    if CONTROL.get().is_some_and(|control| control.apply(level)) {
        tracing::info!("log level set to {}", name(level));
    }
}
