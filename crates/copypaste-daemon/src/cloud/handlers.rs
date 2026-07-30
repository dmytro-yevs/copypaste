//! The four cloud IPC operations.
//!
//! Like the peer handlers these are `async` — they do network I/O, so they run
//! on the reactor rather than on the blocking pool — and like them, every
//! client-visible string is a fixed sentence with no interpolation, because an
//! error must never carry a filesystem path (`CLAUDE.md` rule 4).
//!
//! **Three secrets arrive here and none leaves.** The password and the
//! passphrase are used and dropped: the password goes to GoTrue and is never
//! stored, and the passphrase derives the sync key and is never stored either,
//! so what the daemon keeps is a key for this account rather than the means to
//! re-derive one anywhere else. Nothing in this file logs any of the three at
//! any level, no error payload can carry one (every one is a `const` below, and
//! `SyncError`'s payloads are `&'static str` by construction), and the reply
//! carries only the status a client could have asked for anyway. None of the
//! three ever reaches the store, so none can reach the search index.
//!
//! One residual, stated rather than hidden: the passphrase copy this module
//! derives from is `Zeroizing`, but the request frame it arrived in is an
//! ordinary `String` owned by the JSON codec, and that copy is dropped without
//! being wiped. Closing it means a zeroizing decoder for one field of one
//! method, which is not obviously worth what it costs — but it is the reason
//! this is "the passphrase is not persisted" rather than "the passphrase is not
//! in memory".

use std::sync::Arc;

use copypaste_cloud::auth::{AuthError, SupabaseAuth};
use copypaste_cloud::crypto::derive_sync_key;
use copypaste_cloud::sync::SyncError;
use copypaste_ipc::{ErrorCode, Response, ResponseData};
use tracing::{info, warn};
use zeroize::Zeroizing;

use crate::cloud::{poll, KEY_UPLOAD_FLOOR, KEY_WATERMARK};
use crate::AppState;

const MSG_NOT_CONFIGURED: &str =
    "cloud sync is not configured; the daemon needs a project URL and anon key";
const MSG_SIGNED_OUT: &str = "not signed in to a sync account";
const MSG_NO_EMAIL: &str = "an email address is required";
const MSG_REJECTED: &str = "the email address or password was not accepted";
const MSG_CONFIRM_EMAIL: &str = "confirm the email address before signing in";
const MSG_WEAK_PASSPHRASE: &str =
    "the sync passphrase is too short; it protects data the backend never sees";
const MSG_UNAVAILABLE: &str = "the account service could not be reached";
const MSG_THROTTLED: &str = "the account service is rate limiting this device; try again shortly";
const MSG_STORE: &str = "the sync account could not be stored";
const MSG_DERIVE: &str = "the sync key could not be derived";

/// Every client-visible string this module can produce. Asserted pathless.
#[cfg(test)]
const ALL_MESSAGES: &[&str] = &[
    MSG_NOT_CONFIGURED,
    MSG_SIGNED_OUT,
    MSG_NO_EMAIL,
    MSG_REJECTED,
    MSG_CONFIRM_EMAIL,
    MSG_WEAK_PASSPHRASE,
    MSG_UNAVAILABLE,
    MSG_THROTTLED,
    MSG_STORE,
    MSG_DERIVE,
];

/// Sign in, derive the sync key, and start syncing.
pub async fn sign_in(
    state: &Arc<AppState>,
    id: u64,
    email: &str,
    password: &str,
    passphrase: &str,
) -> Response {
    let Some(config) = state.cloud.config().cloned() else {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_NOT_CONFIGURED);
    };
    let email = email.trim().to_string();
    if email.is_empty() {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_NO_EMAIL);
    }

    let session = match SupabaseAuth::new(config.clone())
        .sign_in(&email, password)
        .await
    {
        Ok(session) => session,
        Err(e) => return sign_in_failure(id, e),
    };
    let user_id = session.user_id.clone();

    // Argon2id at 19 MiB and two passes is hundreds of milliseconds of CPU:
    // running it on the reactor would stall every other request for the
    // duration. The passphrase copy handed over is zeroized when the task ends.
    let passphrase = Zeroizing::new(passphrase.to_string());
    let account = user_id.clone();
    let derived = tokio::task::spawn_blocking(move || derive_sync_key(&passphrase, &account)).await;
    let key = match derived {
        Ok(Ok(key)) => key,
        // The only client-fixable failure the derivation has is a passphrase
        // below the minimum length; anything else is this daemon's problem.
        Ok(Err(e)) => {
            warn!(error = ?e, "could not derive the sync key");
            return Response::err(id, ErrorCode::InvalidRequest, MSG_WEAK_PASSPHRASE);
        }
        Err(e) => {
            warn!(error = %e, "the key derivation task did not complete");
            return Response::err(id, ErrorCode::Internal, MSG_DERIVE);
        }
    };

    // A different account means the stored cursors describe someone else's
    // history: keeping them would skip every row written before now. Signing
    // back into the *same* account keeps the download watermark, so a
    // re-sign-in is not a full re-download.
    let switched = crate::cloud::Cloud::stored_user_id(&state.meta).as_deref() != Some(&user_id);
    if switched {
        if let Err(e) = state.meta.set_state_ms(KEY_WATERMARK, 0) {
            warn!(error = ?e, "could not reset the download cursor");
            return Response::err(id, ErrorCode::Internal, MSG_STORE);
        }
    }
    // Always: signing in is what triggers the backlog sweep, or only future
    // captures ever reach the backend (manifest 05 §4.9, BUG C2).
    if let Err(e) = state.meta.set_state_ms(KEY_UPLOAD_FLOOR, 0) {
        warn!(error = ?e, "could not reset the upload floor");
        return Response::err(id, ErrorCode::Internal, MSG_STORE);
    }

    // The hex is taken before the key is moved into the driver, and is
    // zeroized with the rest of this frame.
    let key_hex = Zeroizing::new(hex::encode(key.to_bytes().as_slice()));
    state
        .cloud
        .install(state, config, email, user_id, key, session);
    if let Err(e) = state.cloud.persist(&state.meta, &key_hex) {
        // The account is live in memory but would not survive a restart, and a
        // half-written record is worse than none.
        warn!(error = ?e, "could not persist the cloud account");
        state.cloud.sign_out(&state.meta);
        return Response::err(id, ErrorCode::Internal, MSG_STORE);
    }

    info!(switched_account = switched, "signed in to a sync account");
    state.cloud.wake();
    Response::ok(id, ResponseData::CloudStatus(state.cloud.status()))
}

/// Map a GoTrue failure onto a fixed sentence and a code.
///
/// `AuthError` distinguishes a bad password from a dead refresh token by the
/// grant kind that was asked for, never by the error body (manifest 05 §4.6.1),
/// so the distinction is safe to act on here.
fn sign_in_failure(id: u64, error: AuthError) -> Response {
    let (code, message) = match error {
        AuthError::InvalidCredentials | AuthError::SessionExpired => {
            (ErrorCode::AuthFailed, MSG_REJECTED)
        }
        AuthError::EmailConfirmationRequired => (ErrorCode::AuthFailed, MSG_CONFIRM_EMAIL),
        AuthError::RateLimited { .. } => (ErrorCode::Internal, MSG_THROTTLED),
        AuthError::Network(_) | AuthError::Server { .. } => (ErrorCode::Internal, MSG_UNAVAILABLE),
        // A malformed or rejected response is not something a user can fix by
        // retyping a password, so it must not read as a credential failure.
        AuthError::Malformed | AuthError::Rejected { .. } => {
            (ErrorCode::Internal, MSG_UNAVAILABLE)
        }
    };
    warn!(error = ?error, "cloud sign-in failed");
    Response::err(id, code, message)
}

/// Forget the account on this device.
///
/// Always succeeds locally. Telling the backend is best-effort: a revoked
/// session is good hygiene, but a network failure must not leave credentials on
/// a device the user has just asked to forget them.
pub async fn sign_out(state: &Arc<AppState>, id: u64) -> Response {
    let driver = state.cloud.sign_out(&state.meta);
    if let (Some(driver), Some(config)) = (driver, state.cloud.config().cloned()) {
        let token = driver.inspect_session(|session| session.access_token.clone());
        if let Err(e) = SupabaseAuth::new(config).sign_out(&token).await {
            warn!(error = ?e, "could not revoke the session with the backend");
        }
    }
    info!("signed out of the sync account");
    Response::ok(id, ResponseData::CloudStatus(state.cloud.status()))
}

pub async fn status(state: &Arc<AppState>, id: u64) -> Response {
    Response::ok(id, ResponseData::CloudStatus(state.cloud.status()))
}

/// Run one round now rather than waiting for the poll.
pub async fn sync_now(state: &Arc<AppState>, id: u64) -> Response {
    if !state.cloud.is_configured() {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_NOT_CONFIGURED);
    }
    match poll::sync_round(state).await {
        Some(Ok(stats)) => Response::ok(id, ResponseData::CloudSync(stats)),
        Some(Err(e)) => Response::err(id, sync_error_code(&e), poll::describe(&e)),
        None => Response::err(id, ErrorCode::AuthFailed, MSG_SIGNED_OUT),
    }
}

/// Only the failures a human can act on are `auth_failed`; the rest are
/// transient and a client should retry rather than prompt.
fn sync_error_code(error: &SyncError) -> ErrorCode {
    match error {
        SyncError::Unauthorized | SyncError::InvalidCredentials | SyncError::SessionExpired => {
            ErrorCode::AuthFailed
        }
        _ => ErrorCode::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::Cloud;
    use crate::testutil::{test_state, test_state_with_cloud};
    use copypaste_cloud::CloudConfig;

    /// A URL that resolves nowhere: these tests assert the paths that never
    /// reach the network, and the one that does must fail rather than hang.
    fn config() -> CloudConfig {
        CloudConfig {
            url: "http://127.0.0.1:1".into(),
            anon_key: "anon".into(),
        }
    }

    #[tokio::test]
    async fn every_operation_reports_unconfigured_rather_than_failing_obscurely() {
        let (state, _dir) = test_state("alpha");

        let response = sign_in(&state, 1, "a@example.com", "pw", "a long passphrase").await;
        assert_eq!(response.error.as_deref(), Some(MSG_NOT_CONFIGURED));
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));

        let response = sync_now(&state, 2).await;
        assert_eq!(response.error.as_deref(), Some(MSG_NOT_CONFIGURED));

        // Status still answers: it is how a client finds out that it is not
        // configured in the first place.
        let response = status(&state, 3).await;
        assert!(response.ok);
        match response.data {
            Some(ResponseData::CloudStatus(status)) => {
                assert!(!status.configured);
                assert!(!status.signed_in);
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn signing_out_when_signed_out_is_not_an_error() {
        let (state, _dir) = test_state("alpha");
        let response = sign_out(&state, 1).await;
        assert!(response.ok);
    }

    #[tokio::test]
    async fn syncing_while_signed_out_is_an_auth_failure_not_a_crash() {
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        let response = sync_now(&state, 1).await;
        assert_eq!(response.error_code, Some(ErrorCode::AuthFailed));
        assert_eq!(response.error.as_deref(), Some(MSG_SIGNED_OUT));
    }

    #[tokio::test]
    async fn a_missing_email_is_refused_before_any_network_call() {
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        let response = sign_in(&state, 1, "   ", "pw", "a long passphrase").await;
        assert_eq!(response.error.as_deref(), Some(MSG_NO_EMAIL));
    }

    #[tokio::test]
    async fn an_unreachable_backend_is_reported_as_unavailable_not_as_bad_credentials() {
        // The distinction matters: "wrong password" makes a user retype a
        // password that was fine (manifest 05 §4.6.1).
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        let response = sign_in(&state, 1, "a@example.com", "pw", "a long passphrase").await;
        assert_eq!(response.error_code, Some(ErrorCode::Internal));
        assert_eq!(response.error.as_deref(), Some(MSG_UNAVAILABLE));
        assert!(!state.cloud.signed_in());
    }

    /// The response to a failed sign-in must not echo any of the three secrets
    /// back, in the message or anywhere else in the frame.
    #[tokio::test]
    async fn a_failed_sign_in_echoes_none_of_the_secrets() {
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        let response = sign_in(
            &state,
            1,
            "person@example.com",
            "pAssw0rd-that-must-not-appear",
            "passphrase-that-must-not-appear",
        )
        .await;

        let frame = serde_json::to_string(&response).expect("serialisable");
        for secret in [
            "pAssw0rd-that-must-not-appear",
            "passphrase-that-must-not-appear",
        ] {
            assert!(!frame.contains(secret), "a secret came back: {frame}");
        }
    }

    #[test]
    fn client_visible_messages_disclose_no_path() {
        for message in ALL_MESSAGES {
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }
}
