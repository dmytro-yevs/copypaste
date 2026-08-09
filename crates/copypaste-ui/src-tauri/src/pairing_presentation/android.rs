use copypaste_ipc::{PairingInviteData, PairingProgressData, PairingState};
use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager as _, Wry};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::invite::{decode_native_invite, encode_native_invite};
use super::{
    NativePairingUi, PairingDecision, PairingPresentationState, PairingPresenter, ScannedPairing,
};

const PLUGIN_PACKAGE: &str = "com.copypaste.app";
const PLUGIN_CLASS: &str = "PairingPresentationPlugin";

pub(crate) fn plugin() -> TauriPlugin<Wry> {
    Builder::new("android-pairing-presentation")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
            app.manage(PairingPresenter::new(AndroidPairingUi(handle)));
            Ok(())
        })
        .build()
}

struct AndroidPairingUi(PluginHandle<Wry>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InviteArgs<'a> {
    payload: &'a str,
    code: &'a str,
    expires_in_secs: u64,
}

#[derive(Serialize)]
struct ProgressArgs {
    state: PairingState,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmArgs<'a> {
    sas: &'a str,
    peer_name: Option<&'a str>,
    role: Option<copypaste_ipc::PairingRole>,
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
}

#[derive(Deserialize)]
struct DecisionResult {
    decision: Option<Decision>,
}

impl AndroidPairingUi {
    fn call<A: Serialize, T: serde::de::DeserializeOwned>(
        &self,
        command: &'static str,
        args: A,
    ) -> Option<T> {
        self.0
            .run_mobile_plugin(command, args)
            .map_err(|_| {
                tracing::warn!(command, "Android pairing presentation failed");
            })
            .ok()
    }
}

impl NativePairingUi for AndroidPairingUi {
    fn present_invite(&self, invite: &PairingInviteData) -> PairingPresentationState {
        let Some(payload) = encode_native_invite(invite) else {
            return PairingPresentationState::Unavailable;
        };
        self.call::<_, PresentationResult>(
            "presentInvite",
            InviteArgs {
                payload: &payload,
                code: &invite.code,
                expires_in_secs: invite.expires_in_secs,
            },
        )
        .filter(|result| result.presented)
        .map_or(PairingPresentationState::Unavailable, |_| {
            PairingPresentationState::Presented
        })
    }

    fn scan_invite(&self) -> Option<ScannedPairing> {
        let mut result: ScanResult = self.call("scanInvite", ())?;
        let payload = Zeroizing::new(result.payload.take()?);
        decode_native_invite(payload)
    }

    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState {
        self.call::<_, PresentationResult>(
            "presentProgress",
            ProgressArgs {
                state: progress.state,
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
        let result: DecisionResult = self.call(
            "confirm",
            ConfirmArgs {
                sas,
                peer_name: progress.peer_name.as_deref(),
                role: progress.role,
            },
        )?;
        result.decision.map(|decision| match decision {
            Decision::Accept => PairingDecision::Accept,
            Decision::Reject => PairingDecision::Reject,
            Decision::Cancel => PairingDecision::Cancel,
        })
    }
}
