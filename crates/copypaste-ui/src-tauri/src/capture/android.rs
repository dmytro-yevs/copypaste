//! The binding to the Kotlin side. **Never compiled on this host.**
//!
//! `tauri::plugin::PluginHandle` exists only on a mobile target, so nothing
//! here is type-checked by `cargo check` on Linux and none of it has been run.
//! That is the same hazard ADR-0002 deleted 2,500 lines of Kotlin over, so this
//! file is kept to the smallest thing that can carry a value across the JNI
//! boundary: it holds no policy, decides nothing, and every sentence it can
//! produce comes from [`super::model`], which is compiled and tested.
//!
//! If you are about to add a branch here, add it to `model` instead.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

use crate::backend::{BackendError, Result};

use super::model::{
    AndroidArmResult, AndroidProbeResult, CaptureModel, CaptureSnapshot, CaptureSource, Clip,
    ReadOutcome, ShizukuProbe, LOST_BODY, LOST_TITLE,
};
use super::CaptureControl;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidSourceAppIcon {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidInstalledSourceApp {
    pub package_id: String,
    pub label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidInstalledSourceApps {
    apps: Vec<AndroidInstalledSourceApp>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceAppIconArgs<'a> {
    package_id: &'a str,
}

/// Must match `applicationId` in the generated Gradle project and the package
/// of `gen/android/app/src/main/java/com/copypaste/app/CapturePlugin.kt`.
const PLUGIN_PACKAGE: &str = "com.copypaste.app";
const PLUGIN_CLASS: &str = "CapturePlugin";

/// Every failure crossing the JNI boundary collapses to one of these. The
/// underlying message may be a Java stack trace, which is neither a sentence
/// nor free of paths, so it is logged and never shown.
const MSG_BRIDGE: &str = "CopyPaste couldn't reach the Android side of capture.";
const MSG_ARM: &str = "Background capture couldn't be started. Check that Shizuku is running.";
const MSG_TOAST: &str = "Android's clipboard notice couldn't be changed.";

/// Register the Kotlin plugin and publish the control it wraps.
pub fn init() -> TauriPlugin<Wry> {
    Builder::new("android-capture")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_PACKAGE, PLUGIN_CLASS)?;
            let capture = AndroidCapture::new(handle);
            // Probe once at startup so the first frame shows the real state
            // rather than a default that resolves a moment later.
            capture.refresh();
            app.manage(capture);
            Ok(())
        })
        .build()
}

pub struct AndroidCapture {
    handle: PluginHandle<Wry>,
    model: Mutex<CaptureModel>,
}

// the wire to Kotlin

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArmArgs {
    /// The wording of the loss notification travels *down*, so the death
    /// recipient can post it without Rust being scheduled — a process that is
    /// being torn down may never get another turn. One set of words, decided in
    /// `model`, rendered by whichever side is still alive.
    lost_title: &'static str,
    lost_body: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuppressArgs {
    suppressed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadResult {
    outcome: ReadOutcome,
    text: Option<String>,
    at_ms: i64,
    focused: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DrainResult {
    clips: Vec<Clip>,
    /// Clips Kotlin's own queue had to discard. Added to the model's count so
    /// there is one number for "copies that were not saved".
    dropped: u64,
    /// The probe rides along, so the once-a-second drain is also the liveness
    /// backstop and there is no second poll. The push signal — the binder death
    /// recipient — still fires first; this only bounds how long the in-app
    /// state can disagree with it.
    probe: ShizukuProbe,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivateModeArgs {
    enabled: bool,
}

impl AndroidCapture {
    fn new(handle: PluginHandle<Wry>) -> Self {
        Self {
            handle,
            model: Mutex::new(CaptureModel::android()),
        }
    }

    fn call<A: Serialize, T: serde::de::DeserializeOwned>(
        &self,
        command: &'static str,
        args: A,
        message: &'static str,
    ) -> Result<T> {
        self.handle.run_mobile_plugin(command, args).map_err(|e| {
            tracing::warn!(command, error = %e, "the android capture plugin failed");
            BackendError::Internal(message.to_string())
        })
    }

    fn with<T>(&self, f: impl FnOnce(&mut CaptureModel) -> T) -> T {
        f(&mut self.model.lock().expect("capture model"))
    }

    pub fn source_app_icon(&self, package_id: &str) -> Option<AndroidSourceAppIcon> {
        match self.call(
            "sourceAppIcon",
            SourceAppIconArgs { package_id },
            MSG_BRIDGE,
        ) {
            Ok(icon) => icon,
            Err(error) => {
                tracing::debug!(%error, "Android source app icon is unavailable");
                None
            }
        }
    }

    /// Lists user-visible installed applications for the exclusion picker.
    /// Package identity remains the value written to the service config.
    pub fn installed_source_apps(&self) -> Result<Vec<AndroidInstalledSourceApp>> {
        self.call::<_, AndroidInstalledSourceApps>("installedSourceApps", (), MSG_BRIDGE)
            .map(|response| response.apps)
    }
}

impl CaptureControl for AndroidCapture {
    fn snapshot(&self) -> CaptureSnapshot {
        self.with(|model| model.snapshot())
    }

    fn refresh(&self) -> CaptureSnapshot {
        match self.call::<_, AndroidProbeResult>("probe", (), MSG_BRIDGE) {
            Ok(result) => self.with(|model| {
                model.set_probe(result.probe);
                model.set_enabled(result.enabled);
                model.record_armed(result.listening);
                model.snapshot()
            }),
            // A bridge that will not answer is not evidence that the grant is
            // gone, but it is certainly not evidence that capture works — and
            // the model already treats "no proof" as not working.
            Err(_) => self.snapshot(),
        }
    }

    fn arm(&self) -> Result<CaptureSnapshot> {
        let args = ArmArgs {
            lost_title: LOST_TITLE,
            lost_body: LOST_BODY,
        };
        let result: AndroidArmResult = self.call("arm", args, MSG_ARM)?;
        Ok(self.with(|model| {
            model.set_enabled(result.status.enabled);
            model.set_probe(result.status.probe);
            model.record_armed(result.status.listening);
            model.record_read(result.outcome, result.focused, copypaste_core::now_ms());
            model.snapshot()
        }))
    }

    fn disarm(&self) -> Result<CaptureSnapshot> {
        self.call::<_, ()>("disarm", (), MSG_BRIDGE)?;
        Ok(self.with(|model| {
            model.record_loss();
            model.snapshot()
        }))
    }

    fn read_now(&self, source: CaptureSource) -> Result<Option<Clip>> {
        let result: ReadResult = self.call("readNow", (), MSG_BRIDGE)?;
        // Recorded, but `focused` is true for this path by construction, so it
        // cannot promote the state to `Working`. See `model::record_read`.
        self.with(|model| model.record_read(result.outcome, result.focused, result.at_ms));
        Ok(result.text.map(|text| Clip {
            text,
            source,
            at_ms: result.at_ms,
            source_app_bundle_id: None,
            source_app_name: None,
        }))
    }

    fn drain(&self) -> Result<Vec<Clip>> {
        let result: DrainResult = self.call("drain", (), MSG_BRIDGE)?;
        self.with(|model| {
            model.set_probe(result.probe);
            model.record_dropped(result.dropped);
            // Something arrived while we were not in front, which is the only
            // proof background capture works — and the only place the state can
            // legitimately become `Working`.
            if result
                .clips
                .iter()
                .any(|c| c.source == CaptureSource::Background)
            {
                model.record_read(ReadOutcome::Succeeded, false, copypaste_core::now_ms());
            }
        });
        Ok(result.clips)
    }

    fn set_private_mode(&self, enabled: bool) -> Result<()> {
        self.call("setPrivateMode", PrivateModeArgs { enabled }, MSG_BRIDGE)
    }

    fn set_enabled(&self, enabled: bool) -> Result<CaptureSnapshot> {
        if enabled {
            return self.arm();
        }
        self.call::<_, ()>("disarm", (), MSG_BRIDGE)?;
        Ok(self.with(|model| {
            model.set_enabled(false);
            model.snapshot()
        }))
    }

    fn set_toast_suppressed(
        &self,
        suppressed: bool,
        acknowledged: bool,
    ) -> Result<CaptureSnapshot> {
        self.with(|model| {
            model.authorise_toast(suppressed, acknowledged, copypaste_core::now_ms())
        })
        .map_err(BackendError::Invalid)?;
        let result: AndroidProbeResult =
            self.call("setToastSuppressed", SuppressArgs { suppressed }, MSG_TOAST)?;
        Ok(self.with(|model| {
            // The probe is re-read rather than assumed: the write goes through
            // the shell uid and can fail, and reporting a suppression that did
            // not happen is the same class of lie as reporting capture that is
            // not running.
            model.set_probe(result.probe);
            model.snapshot()
        }))
    }

    fn note_stored(&self, at_ms: i64) {
        self.with(|model| model.record_capture(at_ms));
    }

    fn note_dropped(&self, count: u64) {
        self.with(|model| model.record_dropped(count));
    }
}
