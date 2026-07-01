//! Config load (first-run vs parse-error distinction).

use crate::paths;
use copypaste_core::AppConfig;

#[tracing::instrument(name = "load_config")]
pub(crate) fn load_config() -> AppConfig {
    let path = paths::config_path();
    AppConfig::load(&path).unwrap_or_else(|e| {
        // P3 (audit): distinguish a missing config file (first run — silent,
        // expected) from a TOML parse error (corrupted/hand-edited config —
        // warn so the operator knows their edits were discarded).
        match &e {
            copypaste_core::config::ConfigError::Io(io_err)
                if io_err.kind() == std::io::ErrorKind::NotFound =>
            {
                // First run: config file does not exist yet; defaults are fine.
                tracing::debug!(
                    "config file not found at {}; using defaults",
                    path.display()
                );
            }
            // CopyPaste-crh3.98: AppConfig::load now wraps the read error with the
            // path (IoWithPath); a missing file is still the silent first-run case.
            copypaste_core::config::ConfigError::IoWithPath { source, .. }
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                tracing::debug!(
                    "config file not found at {}; using defaults",
                    path.display()
                );
            }
            _ => {
                // Parse error or unexpected IO error — operator action may be needed.
                tracing::warn!(
                    error = %e,
                    path = %path.display(),
                    "config file could not be loaded (TOML parse error?); \
                     falling back to defaults — fix or delete the file to silence this"
                );
            }
        }
        let cfg = AppConfig::default();
        if let Err(e) = cfg.save(&path) {
            tracing::warn!("could not save default config: {e}");
        }
        cfg
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // 58ou (PG-31): auto_apply_synced_clip config field contract test
    // -----------------------------------------------------------------------

    /// Verifies that auto_apply_synced_clip defaults to true and can be
    /// persisted/loaded from config.toml.  The actual pasteboard write is
    /// tested in sync_orch; this test confirms the config field contract.
    #[test]
    fn auto_apply_synced_clip_defaults_to_true_in_appconfig() {
        let cfg = AppConfig::default();
        assert!(
            cfg.auto_apply_synced_clip,
            "auto_apply_synced_clip must default to true"
        );
    }

    #[test]
    fn auto_apply_synced_clip_false_persists_and_loads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let cfg = AppConfig {
            auto_apply_synced_clip: false,
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert!(
            !loaded.auto_apply_synced_clip,
            "auto_apply_synced_clip=false must survive save/load"
        );
    }

    // ───────────────────────────────────────────────────────────────────────
    // CopyPaste-9fb6: telemetry wiring smoke tests
    // ───────────────────────────────────────────────────────────────────────

    /// The Disabled reporter (the production default) must never panic and must
    /// accept any ReportableError without performing any I/O.
    #[test]
    fn telemetry_reporter_disabled_is_noop() {
        use copypaste_telemetry::{report_and_log, OsTag, ReportConsent, ReportableError};
        let reporter = copypaste_telemetry::init(ReportConsent::Disabled);
        // Should not panic and should return Ok.
        report_and_log(
            &*reporter,
            ReportableError::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                "test.noop_event",
                OsTag::current(),
            ),
        );
    }

    /// `init_with_dsn` with Disabled consent must return a NoopReporter (no
    /// network I/O) even when a DSN is supplied.
    #[test]
    fn telemetry_init_with_dsn_disabled_returns_noop() {
        use copypaste_telemetry::{report_and_log, OsTag, ReportConsent, ReportableError};
        let reporter = copypaste_telemetry::init_with_dsn(
            ReportConsent::Disabled,
            "https://public@sentry.example/1",
        )
        .expect("disabled init must not fail");
        report_and_log(
            &*reporter,
            ReportableError::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                "test.dsn_disabled_noop",
                OsTag::current(),
            ),
        );
    }

    /// `init_with_dsn` with a garbage DSN and Disabled consent must still
    /// succeed (the DSN is not parsed until the SDK is initialised, and
    /// `Disabled` skips initialisation entirely).
    #[test]
    fn telemetry_init_with_garbage_dsn_disabled_is_ok() {
        use copypaste_telemetry::ReportConsent;
        let reporter =
            copypaste_telemetry::init_with_dsn(ReportConsent::Disabled, "not-a-dsn-at-all")
                .expect("garbage DSN + Disabled must succeed (SDK not initialised)");
        drop(reporter);
    }
}
