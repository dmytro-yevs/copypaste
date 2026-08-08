//! Reading and changing the daemon's settings.
//!
//! Thin on purpose: validation, persistence and the "last good value survives a
//! rejection" rule all live in [`crate::settings`], which is where a change to
//! them belongs. What is here is the mapping onto the wire, and the side effects
//! a changed value has on a running daemon.

use copypaste_ipc::{
    ConfigApplied, ConfigData, ConfigPatch, ErrorCode, PrivateModeData, Response, ResponseData,
};

use crate::settings::SettingsError;
use crate::AppState;

pub(super) fn get(state: &AppState, id: u64) -> Response {
    Response::ok(
        id,
        ResponseData::Config(ConfigApplied {
            config: state.settings.get().clone(),
            restart_required: Vec::new(),
        }),
    )
}

pub(super) fn set(state: &AppState, id: u64, patch: &ConfigPatch) -> Response {
    let before = state.settings.get().clone();
    match state.settings.apply(&state.meta, patch) {
        Ok(config) => {
            if config.storage_quota_bytes < before.storage_quota_bytes
                || config.history_limit < before.history_limit
                || (config.retention_days > 0
                    && (before.retention_days == 0
                        || config.retention_days < before.retention_days))
            {
                copypaste_core::ingest::enforce_retention(&state.store, &config);
            }
            if config.lan_visibility != before.lan_visibility {
                state.p2p.node().set_lan_visibility(config.lan_visibility);
            }
            Response::ok(
                id,
                ResponseData::Config(ConfigApplied {
                    config,
                    restart_required: ConfigData::restart_required_by(patch)
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                }),
            )
        }
        // `ConfigError`'s text names a field and a bound and can contain no
        // path — the type is built that way, and `copypaste_ipc::config` has a
        // test pinning it — so it is safe to pass through verbatim. That is
        // worth doing: "poll_interval_ms must be between 100 and 60000" is the
        // one message here a user can act on.
        Err(e @ SettingsError::Invalid(_)) => {
            Response::err(id, ErrorCode::InvalidRequest, e.to_string())
        }
        Err(e @ SettingsError::Store) => Response::err(id, ErrorCode::Internal, e.to_string()),
    }
}

pub(super) fn private_mode(state: &AppState, id: u64) -> Response {
    let settings = state.settings.get();
    Response::ok(
        id,
        ResponseData::PrivateMode(PrivateModeData {
            private_mode: settings.private_mode,
            private_mode_epoch: settings.private_mode_epoch(),
        }),
    )
}

pub(super) fn set_private_mode(state: &AppState, id: u64, enabled: bool) -> Response {
    match state.settings.apply_with_epoch(
        &state.meta,
        &ConfigPatch {
            private_mode: Some(enabled),
            ..Default::default()
        },
    ) {
        Ok(config) => Response::ok(
            id,
            ResponseData::PrivateMode(PrivateModeData {
                private_mode: config.config.private_mode,
                private_mode_epoch: config.private_mode_epoch,
            }),
        ),
        Err(e @ SettingsError::Invalid(_)) => {
            Response::err(id, ErrorCode::InvalidRequest, e.to_string())
        }
        Err(SettingsError::Store) => {
            Response::err(id, ErrorCode::Internal, "the settings could not be saved")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_ipc::Method;

    fn call(state: &AppState, method: Method) -> Response {
        crate::server::dispatch::dispatch_store(state, 1, method)
    }

    fn applied(response: Response) -> ConfigApplied {
        match response.data {
            Some(ResponseData::Config(applied)) => applied,
            other => panic!("{other:?}"),
        }
    }

    fn private_mode_data(response: Response) -> PrivateModeData {
        match response.data {
            Some(ResponseData::PrivateMode(mode)) => mode,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn get_reports_the_defaults_on_a_fresh_daemon() {
        let (state, _dir) = test_state("alpha");
        let config = applied(call(&state, Method::GetConfig)).config;
        assert_eq!(config, ConfigData::default());
    }

    #[test]
    fn a_change_takes_effect_and_reports_no_restart() {
        let (state, _dir) = test_state("alpha");
        let result = applied(call(
            &state,
            Method::SetConfig {
                patch: ConfigPatch {
                    poll_interval_ms: Some(250),
                    ..Default::default()
                },
            },
        ));
        assert_eq!(result.config.poll_interval_ms, 250);
        assert!(result.restart_required.is_empty());
        assert_eq!(state.settings.get().poll_interval_ms, 250);
    }

    #[test]
    fn storage_quota_is_live_and_persisted() {
        let (state, _dir) = test_state("alpha");
        let result = applied(call(
            &state,
            Method::SetConfig {
                patch: ConfigPatch {
                    storage_quota_bytes: Some(copypaste_ipc::MIN_STORAGE_QUOTA_BYTES),
                    ..Default::default()
                },
            },
        ));
        assert_eq!(
            result.config.storage_quota_bytes,
            copypaste_ipc::MIN_STORAGE_QUOTA_BYTES
        );
        assert!(result.restart_required.is_empty());
        assert_eq!(
            state.settings.get().storage_quota_bytes,
            copypaste_ipc::MIN_STORAGE_QUOTA_BYTES
        );
    }

    #[test]
    fn lowering_a_live_retention_limit_sweeps_existing_history() {
        let (state, _dir) = test_state("alpha");
        for n in 0..51 {
            crate::testutil::add(&state, &format!("history item {n}"));
        }

        let result = applied(call(
            &state,
            Method::SetConfig {
                patch: ConfigPatch {
                    history_limit: Some(50),
                    ..Default::default()
                },
            },
        ));
        assert_eq!(result.config.history_limit, 50);
        assert_eq!(state.store.count().unwrap(), 50);
    }

    #[test]
    fn turning_lan_visibility_off_takes_effect_without_a_restart() {
        let (state, _dir) = test_state("alpha");
        let result = applied(call(
            &state,
            Method::SetConfig {
                patch: ConfigPatch {
                    lan_visibility: Some(false),
                    ..Default::default()
                },
            },
        ));
        assert!(!result.config.lan_visibility);
        assert!(result.restart_required.is_empty());
        assert!(!state
            .p2p
            .discovery()
            .expect("a discovery handle")
            .is_running());
    }

    #[test]
    fn a_bad_value_is_rejected_with_an_actionable_pathless_message() {
        let (state, _dir) = test_state("alpha");
        let response = call(
            &state,
            Method::SetConfig {
                patch: ConfigPatch {
                    poll_interval_ms: Some(1),
                    ..Default::default()
                },
            },
        );
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        let message = response.error.expect("a reason");
        assert!(message.contains("poll_interval_ms"), "{message}");
        assert!(!message.contains('/'), "{message}");
        assert_eq!(
            state.settings.get().poll_interval_ms,
            ConfigData::default().poll_interval_ms
        );
    }

    #[test]
    fn private_mode_is_persistent_and_has_a_narrow_ipc_surface() {
        let (state, dir) = test_state("private-mode");
        let first = private_mode_data(call(&state, Method::SetPrivateMode { enabled: true }));
        assert!(first.private_mode);
        assert_eq!(first.private_mode_epoch, 1);

        let second = private_mode_data(call(&state, Method::SetPrivateMode { enabled: true }));
        assert!(second.private_mode);
        assert_eq!(second.private_mode_epoch, 2);
        let read = private_mode_data(call(&state, Method::GetPrivateMode));
        assert_eq!(read.private_mode_epoch, second.private_mode_epoch);

        let status = match call(&state, Method::Status).data {
            Some(ResponseData::Status(status)) => status,
            other => panic!("{other:?}"),
        };
        assert_eq!(status.private_mode, read.private_mode);
        assert_eq!(status.private_mode_epoch, read.private_mode_epoch);
        drop(state);

        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "private-mode");
        let restarted_mode = private_mode_data(call(&restarted, Method::GetPrivateMode));
        assert!(restarted_mode.private_mode);
        assert_eq!(restarted_mode.private_mode_epoch, 0);
    }
}
