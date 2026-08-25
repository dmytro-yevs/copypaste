use std::sync::Mutex;

use copypaste_ipc::{PairingInviteData, PairingProgressData, PairingState};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{AppHandle, Manager as _, Wry};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::invite::{decode_native_invite, encode_native_invite};
use super::{
    NativeAbort, NativePairingUi, NativePresentationOutcome, NativeRefresh, PairingDecision,
    PairingPresentationState, PairingPresenter, ScannedPairing,
};
use crate::backend::{PairingBackend as _, SelectedBackend};

const PLUGIN_PACKAGE: &str = "com.copypaste.app";
const PLUGIN_CLASS: &str = "PairingPresentationPlugin";

pub(crate) fn plugin() -> TauriPlugin<Wry> {
    Builder::new("android-pairing-presentation")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
            let abort = pairing_abort(app.clone());
            let refresh = super::native_refresh(app.clone());
            app.manage(PairingPresenter::new(AndroidPairingUi::new(
                handle, abort, refresh,
            )));
            Ok(())
        })
        .build()
}

fn pairing_abort(app: AppHandle) -> NativeAbort {
    std::sync::Arc::new(move || {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let backend = app.state::<SelectedBackend>();
            let _ = backend.pair_cancel().await;
        });
    })
}

struct AndroidPairingUi {
    plugin: PluginHandle<Wry>,
    abort: NativeAbort,
    refresh: NativeRefresh,
    on_abort: Mutex<Option<Channel<()>>>,
    on_refresh: Mutex<Option<Channel<()>>>,
}

impl AndroidPairingUi {
    fn new(plugin: PluginHandle<Wry>, abort: NativeAbort, refresh: NativeRefresh) -> Self {
        Self {
            plugin,
            abort,
            refresh,
            on_abort: Mutex::new(None),
            on_refresh: Mutex::new(None),
        }
    }

    fn call<A: Serialize, T: serde::de::DeserializeOwned>(
        &self,
        command: &'static str,
        args: A,
    ) -> Option<T> {
        self.plugin
            .run_mobile_plugin(command, args)
            .map_err(|_| {
                tracing::warn!(command, "Android pairing presentation failed");
            })
            .ok()
    }

    fn retain_abort_channel(&self) -> Channel<()> {
        let abort = self.abort.clone();
        let channel = Channel::new(move |_| {
            abort();
            Ok(())
        });
        if let Ok(mut slot) = self.on_abort.lock() {
            *slot = Some(channel.clone());
        }
        channel
    }

    fn retain_refresh_channel(&self) -> Channel<()> {
        let refresh = self.refresh.clone();
        let channel = Channel::new(move |_| {
            refresh();
            Ok(())
        });
        if let Ok(mut slot) = self.on_refresh.lock() {
            *slot = Some(channel.clone());
        }
        channel
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InviteArgs<'a> {
    payload: &'a str,
    code: &'a str,
    expires_in_secs: u64,
    on_abort: Channel<()>,
    on_refresh: Channel<()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressArgs {
    state: PairingState,
    on_abort: Channel<()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmArgs<'a> {
    sas: &'a str,
    peer_name: Option<&'a str>,
    role: Option<copypaste_ipc::PairingRole>,
    expires_in_ms: u64,
}

#[derive(Deserialize)]
struct PresentationResult {
    presented: bool,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
struct ScanResult {
    payload: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Decision {
    Accept,
    Reject,
    Cancel,
    Refresh,
}

#[derive(Deserialize)]
struct DecisionResult {
    decision: Option<Decision>,
}

impl NativePairingUi for AndroidPairingUi {
    fn present_invite(&self, invite: &PairingInviteData) -> NativePresentationOutcome {
        let Some(payload) = encode_native_invite(invite) else {
            return NativePresentationOutcome::Unavailable;
        };
        let on_abort = self.retain_abort_channel();
        let on_refresh = self.retain_refresh_channel();
        self.call::<_, PresentationResult>(
            "presentInvite",
            InviteArgs {
                payload: &payload,
                code: &invite.code,
                expires_in_secs: invite.expires_in_secs,
                on_abort,
                on_refresh,
            },
        )
        .filter(|result| result.presented)
        .map_or(NativePresentationOutcome::Unavailable, |_| {
            NativePresentationOutcome::Presented
        })
    }

    fn scan_invite(&self) -> Option<ScannedPairing> {
        let mut result: ScanResult = self.call("scanInvite", ())?;
        let payload = Zeroizing::new(result.payload.take()?);
        decode_native_invite(payload)
    }

    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState {
        // Awaiting confirmation uses the SAS sheet from confirm(). A modal
        // progress dialog would block the WebView Confirm control and abort the
        // ceremony on dismiss (INV-16), so inbound pairing could never finish.
        if progress.state == PairingState::AwaitingConfirmation {
            return PairingPresentationState::Presented;
        }
        let on_abort = self.retain_abort_channel();
        self.call::<_, PresentationResult>(
            "presentProgress",
            ProgressArgs {
                state: progress.state,
                on_abort,
            },
        )
        .filter(|result| result.presented)
        .map_or(PairingPresentationState::Unavailable, |_| {
            PairingPresentationState::Presented
        })
    }

    fn confirm(&self, progress: &PairingProgressData) -> Option<PairingDecision> {
        let sas = progress.sas.as_deref()?;
        if sas.len() != 6 || !sas.bytes().all(|digit| digit.is_ascii_digit()) {
            return None;
        }
        if let Ok(mut slot) = self.on_abort.lock() {
            *slot = None;
        }
        let expires_in_ms = progress.expires_in_ms?;
        let result: DecisionResult = self.call(
            "confirm",
            ConfirmArgs {
                sas,
                peer_name: progress.peer_name.as_deref(),
                role: progress.role,
                expires_in_ms,
            },
        )?;
        result.decision.map(|decision| match decision {
            Decision::Accept => PairingDecision::Accept,
            Decision::Reject => PairingDecision::Reject,
            Decision::Cancel => PairingDecision::Cancel,
            Decision::Refresh => PairingDecision::Refresh,
        })
    }
}
