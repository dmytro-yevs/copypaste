//! Reading and changing the daemon's settings.
//!
//! Thin on purpose: validation, persistence and the "last good value survives a
//! rejection" rule all live in [`crate::settings`], which is where a change to
//! them belongs. What is here is the mapping onto the wire, and the side effects
//! a changed value has on a running daemon.

use copypaste_ipc::{
    ConfigApplied, ConfigData, ConfigPatch, ErrorCode, PrivateModeData, Response, ResponseData,
};

use crate::settings::{SettingsError, SettingsTransition};
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
    set_with_effects(state, id, patch, |transition| {
        apply_runtime_effects(state, transition);
    })
}

fn set_with_effects<F>(state: &AppState, id: u64, patch: &ConfigPatch, effects: F) -> Response
where
    F: FnOnce(&SettingsTransition),
{
    let before = {
        let settings = state.settings.get();
        (settings.private_mode_epoch(), settings.health().cloned())
    };
    match state
        .settings
        .apply_with_effects(&state.meta, patch, effects)
    {
        Ok(config) => {
            let changed = {
                let settings = state.settings.get();
                (settings.private_mode_epoch(), settings.health().cloned()) != before
            };
            if changed {
                state.note_capture_state_changed();
            }
            if patch.sync_enabled.is_some() {
                state
                    .modules
                    .sync_enabled_changed(config.config.sync_enabled);
            }
            Response::ok(
                id,
                ResponseData::Config(ConfigApplied {
                    config: config.config,
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

fn apply_runtime_effects(state: &AppState, transition: &SettingsTransition) {
    advance_private_cursor(state, transition);
    let removed = copypaste_core::retention::reconcile_policy(
        &state.store,
        || state.settings.get().clone(),
        transition.should_enforce_retention(),
    );
    if removed > 0 {
        // Retention is local housekeeping, not a sync deletion. This only
        // wakes watchers; `note_local_change` would advance transport cursors
        // for rows that must remain local to this device.
        state.note_remote_change();
    }
    if transition.lan_visibility_changed() {
        state
            .p2p
            .node()
            .set_lan_visibility(transition.config().lan_visibility);
    }
    if let Some(enabled) = transition.sync_enabled_changed() {
        state.p2p.sync_enabled_changed(enabled);
    }
}

fn advance_private_cursor(state: &AppState, transition: &SettingsTransition) {
    if !transition.resumes_capture() {
        return;
    }
    // Settings retains capture authority throughout this effect.
    // Advance the native cursor without reading or replacing private content.
    let mut private_policy = transition.config().clone();
    private_policy.private_mode = true;
    state
        .clipboard()
        .poll_with_policy(crate::clipboard::CapturePolicy::new(&private_policy));
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
    let before = {
        let settings = state.settings.get();
        (settings.private_mode_epoch(), settings.health().cloned())
    };
    match state.settings.apply_with_effects(
        &state.meta,
        &ConfigPatch {
            private_mode: Some(enabled),
            ..Default::default()
        },
        |transition| advance_private_cursor(state, transition),
    ) {
        Ok(config) => {
            let changed = {
                let settings = state.settings.get();
                (settings.private_mode_epoch(), settings.health().cloned()) != before
            };
            if changed {
                state.note_capture_state_changed();
            }
            Response::ok(
                id,
                ResponseData::PrivateMode(PrivateModeData {
                    private_mode: config.config.private_mode,
                    private_mode_epoch: config.private_mode_epoch,
                }),
            )
        }
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
    use std::sync::{mpsc, Arc, Mutex};

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
    fn private_mode_change_publishes_authoritative_capture_state() {
        let (state, _dir) = test_state("private-mode-push");
        let mut events = state.subscribe_runtime();
        assert!(set_private_mode(&state, 1, true).ok);
        let capture = events.try_recv().unwrap().capture.unwrap();
        assert!(capture.private_mode);
        assert_eq!(capture.private_mode_epoch, 1);
        assert_eq!(capture.capture_revision, 1);
        assert!(set_private_mode(&state, 2, true).ok);
        assert!(events.try_recv().is_err());
    }

    #[derive(Default)]
    struct CursorFixture {
        generation: u64,
        observed: u64,
        value: &'static str,
        payload_reads: usize,
    }

    struct CursorSource(Arc<Mutex<CursorFixture>>);

    impl crate::clipboard::ClipboardSource for CursorSource {
        fn poll(&mut self) -> Option<crate::clipboard::Capture> {
            self.poll_with_policy(crate::clipboard::CapturePolicy::new(&ConfigData::default()))
        }

        fn poll_with_policy(
            &mut self,
            policy: crate::clipboard::CapturePolicy<'_>,
        ) -> Option<crate::clipboard::Capture> {
            let mut cursor = self.0.lock().unwrap();
            if cursor.generation == cursor.observed {
                return None;
            }
            cursor.observed = cursor.generation;
            if policy.settings.private_mode {
                return None;
            }
            cursor.payload_reads += 1;
            Some(crate::clipboard::Capture {
                privacy: Default::default(),
                content: cursor.value.into(),
                binary_content: None,
                file_path: None,
                file_metadata: None,
                content_type: copypaste_ipc::content_type::TEXT.into(),
                app_bundle_id: None,
                app_name: None,
                source_policy: crate::clipboard::SourcePolicyEvidence::Legacy,
            })
        }

        fn set_contents(&mut self, _: &str) -> anyhow::Result<()> {
            panic!("private mode must preserve the native clipboard")
        }

        fn backend_name(&self) -> &'static str {
            "private-mode-cursor-fixture"
        }
    }

    #[test]
    fn both_private_resume_paths_advance_without_reading_and_keep_the_next_copy() {
        for via_config in [false, true] {
            let cursor = Arc::new(Mutex::new(CursorFixture::default()));
            let (state, _dir) = crate::testutil::test_state_with_clipboard(
                "private-resume",
                Box::new(CursorSource(Arc::clone(&cursor))),
            );
            assert!(call(&state, Method::SetPrivateMode { enabled: true }).ok);
            {
                let mut pending = cursor.lock().unwrap();
                pending.generation = 1;
                pending.value = "private fixture";
            }
            let resume = || {
                if via_config {
                    Method::SetConfig {
                        patch: ConfigPatch {
                            private_mode: Some(false),
                            ..Default::default()
                        },
                    }
                } else {
                    Method::SetPrivateMode { enabled: false }
                }
            };
            assert!(call(&state, resume()).ok);
            {
                let pending = cursor.lock().unwrap();
                assert_eq!(pending.observed, 1);
                assert_eq!(pending.payload_reads, 0);
                assert_eq!(pending.value, "private fixture");
            }
            let settings = state.settings.get().clone();
            assert!(state
                .clipboard()
                .poll_with_policy(crate::clipboard::CapturePolicy::new(&settings))
                .is_none());
            {
                let mut pending = cursor.lock().unwrap();
                pending.generation = 2;
                pending.value = "first post-resume copy";
            }
            // Repeating an already-off request must not discard a fresh copy.
            assert!(call(&state, resume()).ok);
            let capture = state
                .clipboard()
                .poll_with_policy(crate::clipboard::CapturePolicy::new(&settings))
                .expect("first post-ack copy");
            assert_eq!(capture.content, "first post-resume copy");
            assert_eq!(cursor.lock().unwrap().payload_reads, 1);
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
        let mut events = state.subscribe();

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
        let event = events
            .try_recv()
            .expect("the real sweep must wake watchers");
        assert_eq!(event.event, copypaste_ipc::EventKind::Items);
        assert!(!event.captured);

        let _ = call(
            &state,
            Method::SetConfig {
                patch: ConfigPatch {
                    poll_interval_ms: Some(250),
                    ..Default::default()
                },
            },
        );
        assert!(
            events.try_recv().is_err(),
            "an unrelated settings save emitted an Items event"
        );
    }

    /// F-01. The first request cannot leave a destructive sweep behind after a
    /// later request has acknowledged a relaxed history limit.
    #[test]
    fn a_later_history_limit_acknowledgement_cannot_overtake_retention() {
        let (state, _dir) = test_state("ordered-settings-retention");
        for n in 0..101 {
            crate::testutil::add(&state, &format!("history item {n}"));
        }

        let (effect_ready_tx, effect_ready_rx) = mpsc::channel();
        let (release_effect_tx, release_effect_rx) = mpsc::channel();
        let first = {
            let state = std::sync::Arc::clone(&state);
            std::thread::spawn(move || {
                set_with_effects(
                    &state,
                    1,
                    &ConfigPatch {
                        history_limit: Some(50),
                        ..Default::default()
                    },
                    |transition| {
                        effect_ready_tx.send(()).unwrap();
                        release_effect_rx.recv().unwrap();
                        apply_runtime_effects(&state, transition);
                    },
                )
            })
        };

        effect_ready_rx.recv().unwrap();
        let second = if state.settings.transition_is_in_progress() {
            let (second_started_tx, second_started_rx) = mpsc::channel();
            let state = std::sync::Arc::clone(&state);
            let second = std::thread::spawn(move || {
                second_started_tx.send(()).unwrap();
                let response = set(
                    &state,
                    2,
                    &ConfigPatch {
                        history_limit: Some(100),
                        ..Default::default()
                    },
                );
                for n in 0..20 {
                    crate::testutil::add(&state, &format!("relaxed item {n}"));
                }
                response
            });
            second_started_rx.recv().unwrap();
            release_effect_tx.send(()).unwrap();
            second
        } else {
            let response = set(
                &state,
                2,
                &ConfigPatch {
                    history_limit: Some(100),
                    ..Default::default()
                },
            );
            for n in 0..20 {
                crate::testutil::add(&state, &format!("relaxed item {n}"));
            }
            release_effect_tx.send(()).unwrap();
            std::thread::spawn(move || response)
        };

        assert_eq!(applied(first.join().unwrap()).config.history_limit, 50);
        assert_eq!(applied(second.join().unwrap()).config.history_limit, 100);
        assert_eq!(state.store.count().unwrap(), 70);
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
    fn the_live_sync_switch_replaces_cancelled_transport_cycles() {
        let (state, _dir) = test_state("sync-transition");
        let peers_before = state.p2p.sync_cycle().cancel_token();

        let off = set(
            &state,
            1,
            &ConfigPatch {
                sync_enabled: Some(false),
                ..Default::default()
            },
        );
        assert!(off.ok);
        assert!(!state.settings.get().sync_enabled);
        assert!(peers_before.is_cancelled());

        let on = set(
            &state,
            2,
            &ConfigPatch {
                sync_enabled: Some(true),
                ..Default::default()
            },
        );
        assert!(on.ok);
        assert!(state.settings.get().sync_enabled);
        assert!(!state.p2p.sync_cycle().cancel_token().is_cancelled());
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

        let (restarted, _dir) = crate::testutil::reopen(dir, "private-mode");
        let restarted_mode = private_mode_data(call(&restarted, Method::GetPrivateMode));
        assert!(restarted_mode.private_mode);
        assert_eq!(restarted_mode.private_mode_epoch, 0);
    }
    #[test]
    fn settings_winning_authority_blocks_old_snapshot_payload_admission() {
        let (state, _dir) = test_state("settings-before-capture");
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let settings_state = state.clone();
        let setter = std::thread::spawn(move || {
            set_with_effects(
                &settings_state,
                1,
                &ConfigPatch {
                    private_mode: Some(true),
                    ..Default::default()
                },
                |_| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                },
            )
        });
        entered_rx.recv().unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let capture_state = state.clone();
        let capture = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            capture_state
                .settings
                .with_capture_authority(|settings, _| {
                    // This stands for the provider boundary: the worker receives
                    // the policy published by the transition that won authority.
                    usize::from(!settings.private_mode)
                })
        });
        started_rx.recv().unwrap();
        assert!(state.settings.transition_is_in_progress());
        release_tx.send(()).unwrap();
        assert!(setter.join().unwrap().ok);
        assert_eq!(capture.join().unwrap(), Some(0));
        assert_eq!(state.store.count().unwrap(), 0);
    }
}
