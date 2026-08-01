//! Reading and changing the daemon's settings.
//!
//! Thin on purpose: validation, persistence and the "last good value survives a
//! rejection" rule all live in [`crate::settings`], which is where a change to
//! them belongs. What is here is the mapping onto the wire — including the list
//! of fields that will not take effect until a restart, which a Settings screen
//! needs at the moment of the change rather than afterwards.

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
    match state.settings.apply(&state.meta, patch) {
        Ok(config) => Response::ok(
            id,
            ResponseData::Config(ConfigApplied {
                config,
                restart_required: ConfigData::restart_required_by(patch)
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            }),
        ),
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
    Response::ok(
        id,
        ResponseData::PrivateMode(PrivateModeData {
            private_mode: state.settings.get().private_mode,
        }),
    )
}

pub(super) fn set_private_mode(state: &AppState, id: u64, enabled: bool) -> Response {
    match state.settings.apply(
        &state.meta,
        &ConfigPatch {
            private_mode: Some(enabled),
            ..Default::default()
        },
    ) {
        Ok(config) => Response::ok(
            id,
            ResponseData::PrivateMode(PrivateModeData {
                private_mode: config.private_mode,
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

    /// The one field that cannot be applied live must say so at the moment it
    /// is changed, not leave the user to notice.
    #[test]
    fn changing_lan_visibility_reports_that_a_restart_is_needed() {
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
        assert_eq!(result.restart_required, ["lan_visibility"]);
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
        let response = call(&state, Method::SetPrivateMode { enabled: true });
        let enabled = match response.data {
            Some(ResponseData::PrivateMode(mode)) => mode.private_mode,
            other => panic!("{other:?}"),
        };
        assert!(enabled);
        drop(state);

        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "private-mode");
        let response = call(&restarted, Method::GetPrivateMode);
        match response.data {
            Some(ResponseData::PrivateMode(mode)) => assert!(mode.private_mode),
            other => panic!("{other:?}"),
        }
    }
}
