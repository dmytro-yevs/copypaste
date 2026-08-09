//! What capture is doing, as one value — and the rules that decide it.
//!
//! Everything in this file is ordinary Rust with no Android in it, which is
//! deliberate: this host has no SDK and no device, so any decision left in
//! Kotlin is a decision nothing can compile or test. Kotlin reports facts
//! (is Shizuku running, did a read return); this file decides what those facts
//! mean and what sentence the user sees.
//!
//! # The one invariant
//!
//! [`CaptureHealth::Working`] is reachable only through
//! [`CaptureModel::record_read`] — there is no setter, no constructor and no
//! deserialiser that can produce it. A permission being present is not
//! evidence that a read succeeds, and v1 shipped the opposite (`CopyPaste-qzhu`
//! removed an optimistic `working = true`). Losing the binder clears the
//! evidence as well as the grant, so a re-arm cannot inherit a claim made
//! before a reboot.

use serde::{Deserialize, Serialize};

use super::messages::{detail, headline, MSG_TOAST_UNEXPLAINED};

pub use super::messages::{LOST_BODY, LOST_TITLE, TOAST_EXPLANATION};

/// Which mechanism is capturing *right now*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rung {
    /// macOS: the background service reads the pasteboard. No ladder applies.
    Desktop,
    /// Rung 0 — in-app copies, the share sheet, the text-selection action, the
    /// Quick Settings tile, and the Mac's history over sync.
    InApp,
    /// Rung 2 — background capture from every app, as the shell uid.
    Shizuku,
}

/// How an item arrived.
///
/// Travels on the capture event so the history can say where a row came from.
/// It is **not persisted yet**: the store has no column for it and
/// `copypaste_ipc` has no field, so the value dies with the event. See
/// ADR-0005 for what has to change for the android doc's §5 rule 5 to be met
/// in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    /// Copied inside CopyPaste.
    InApp,
    /// Arrived through the system share sheet.
    Share,
    /// Arrived through the text-selection `ACTION_PROCESS_TEXT` action.
    ProcessText,
    /// The Quick Settings tile: the user tapped, our activity took focus, and
    /// the read was legal because of it.
    Tile,
    /// Read in the background as the shell uid.
    Background,
}

/// Why background capture is not set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotGrantedReason {
    /// Android 10 and below cannot pair wireless debugging on the device
    /// itself, so rung 2 there would need a computer. We do not offer it.
    Unsupported,
    NotInstalled,
    /// The expected state after every restart: pairing is once, starting is
    /// every boot.
    NotRunning,
    NoPermission,
}

/// Granted, and still not proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotWorkingReason {
    /// Everything is in place but nothing has been read yet. Not a fault.
    AwaitingFirstCopy,
    /// A read was attempted as the shell uid and refused. This is the state
    /// the whole design is unverified against.
    ReadRefused,
    /// The listener is not registered — the user has not armed it, or arming
    /// failed.
    NotArmed,
}

/// v1's four states, carried forward per the android doc §5 rule 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CaptureHealth {
    NotGranted {
        reason: NotGrantedReason,
    },
    /// The user turned background capture off. Distinct from `NotGranted`:
    /// nothing is broken and nothing needs fixing.
    Disabled,
    GrantedNotWorking {
        reason: NotWorkingReason,
    },
    Working,
}

/// What one tap should do next.
///
/// Computed here rather than in the view so that the notification, the setup
/// screen and the status chip cannot disagree about the next step — the android
/// doc's §5 rule 4 is only cheap if there is exactly one answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextStep {
    /// Nothing to do.
    None,
    /// Send the user to Shizuku on Play.
    InstallShizuku,
    /// Send the user to Shizuku to run its wireless-debugging start.
    StartShizuku,
    /// Ask Shizuku for our permission.
    GrantPermission,
    /// Register the listener.
    Arm,
}

/// What Kotlin found when it looked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShizukuProbe {
    /// `Build.VERSION.SDK_INT >= 30`: wireless debugging can be paired from the
    /// phone itself, with no computer.
    pub supported: bool,
    pub installed: bool,
    /// `Shizuku.pingBinder()`. False after every reboot until the user starts
    /// it again.
    pub running: bool,
    pub permission: bool,
    /// `Settings.Secure.CLIPBOARD_SHOW_ACCESS_NOTIFICATIONS == 0`. Read, never
    /// assumed: another app or the user may have set it.
    pub toast_suppressed: bool,
    /// The app was opened from the "background capture stopped" notification.
    ///
    /// Read-and-cleared on the Kotlin side, so it is true on exactly the probe
    /// that follows the tap and false afterwards. That is what makes re-arming
    /// one tap (android doc §5 rule 4) instead of a hunt through settings.
    pub rearm_requested: bool,
}

/// The status fields every Kotlin command returning a probe must include.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AndroidProbeResult {
    pub probe: ShizukuProbe,
    pub enabled: bool,
    pub listening: bool,
}

/// What one read attempt did.
///
/// `Empty` is separate from `Refused` because they are indistinguishable from
/// a single `getPrimaryClip` — both answer `null` — and conflating them is how
/// a working setup would be reported as broken on a phone whose clipboard
/// happened to be empty. Kotlin distinguishes them by asking
/// `hasPrimaryClip()` first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadOutcome {
    Succeeded,
    Empty,
    Refused,
}

/// The result of Kotlin's `arm` command.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AndroidArmResult {
    #[serde(flatten)]
    pub status: AndroidProbeResult,
    pub outcome: ReadOutcome,
    pub focused: bool,
}

/// One clip Kotlin captured and has not handed over yet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub text: String,
    pub source: CaptureSource,
    /// Milliseconds since the Unix epoch, stamped when it was read rather than
    /// when it was drained.
    pub at_ms: i64,
    /// Android package that wrote the current clip, when the privileged
    /// clipboard service exposed it.
    #[serde(default)]
    pub source_app_bundle_id: Option<String>,
    /// Resolved from Android's PackageManager only after the clipboard service
    /// supplied a package id. It is not guessed from the copied content.
    #[serde(default)]
    pub source_app_name: Option<String>,
}

/// Whether the user has been told what suppressing the toast turns off.
///
/// A struct rather than a `bool` so that "on" cannot be represented without
/// the acknowledgement that produced it (android doc §4: never silently).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToastConsent {
    acknowledged_at: Option<i64>,
}

impl ToastConsent {
    pub fn acknowledged(&self) -> bool {
        self.acknowledged_at.is_some()
    }
}

/// Everything known about capture on this device.
#[derive(Debug, Clone)]
pub struct CaptureModel {
    /// `Desktop` for the macOS build, and then nothing else here applies.
    platform_rung: Rung,
    /// The user's switch. Defaults off: rung 2 is an opt-in upgrade, not the
    /// state a new install starts in.
    enabled: bool,
    probe: ShizukuProbe,
    armed: bool,
    read_refused: bool,
    /// The only evidence that background capture works. Cleared by any loss.
    last_read_ok_at: Option<i64>,
    last_capture_at: Option<i64>,
    /// Clips Kotlin had to drop because its hand-off queue was full. Surfaced,
    /// never swallowed: a copy that was not saved is the failure this whole
    /// feature is about.
    dropped_clips: u64,
    toast: ToastConsent,
}

impl CaptureModel {
    /// The macOS model: one state, no ladder, nothing to arm.
    pub fn desktop() -> Self {
        Self {
            platform_rung: Rung::Desktop,
            enabled: true,
            probe: ShizukuProbe::default(),
            armed: true,
            read_refused: false,
            last_read_ok_at: None,
            last_capture_at: None,
            dropped_clips: 0,
            toast: ToastConsent::default(),
        }
    }

    /// A fresh Android install: rung 0, working, nothing asked of the user.
    pub fn android() -> Self {
        Self {
            platform_rung: Rung::InApp,
            enabled: false,
            ..Self::desktop()
        }
    }

    pub fn set_probe(&mut self, probe: ShizukuProbe) {
        // Losing the binder between two probes is a loss like any other, and
        // must clear the evidence rather than leave a stale "working".
        if self.probe.running && !probe.running {
            self.record_loss();
        }
        self.probe = probe;
    }

    pub fn probe(&self) -> ShizukuProbe {
        self.probe
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.record_loss();
        }
    }

    /// The listener registered. Note what this does **not** do: it does not
    /// claim the thing works.
    pub fn record_armed(&mut self, armed: bool) {
        self.armed = armed;
        if !armed {
            self.last_read_ok_at = None;
        }
    }

    /// The one way into [`CaptureHealth::Working`].
    ///
    /// `focused` is whether our own window had focus when the read happened,
    /// and it is the difference between evidence and a false positive. Rung 0's
    /// tile read and the read `arm` performs both run with the app in front,
    /// where *any* app may read the clipboard — so they prove that the
    /// clipboard is readable, not that it is readable in the background, which
    /// is the only claim `Working` makes. Marking those as proof would
    /// reintroduce `CopyPaste-qzhu` in a subtler costume: the setup screen would
    /// go green at the exact moment it cannot know anything.
    pub fn record_read(&mut self, outcome: ReadOutcome, focused: bool, now_ms: i64) {
        match outcome {
            ReadOutcome::Succeeded if focused => {}
            ReadOutcome::Succeeded => {
                self.read_refused = false;
                self.last_read_ok_at = Some(now_ms);
            }
            // An empty clipboard proves nothing either way, so it changes
            // nothing. It must not clear a previous success and must not count
            // as one.
            ReadOutcome::Empty => {}
            // A refusal counts wherever it happened. With focus the read is
            // allowed outright, so being refused there is a worse sign, not a
            // milder one.
            ReadOutcome::Refused => {
                self.read_refused = true;
                self.last_read_ok_at = None;
            }
        }
    }

    /// Binder death, a failed probe, or the user switching it off.
    ///
    /// Idempotent, because the death recipient and the next probe will both
    /// report the same loss and neither should have to know about the other.
    pub fn record_loss(&mut self) {
        self.armed = false;
        self.last_read_ok_at = None;
        self.read_refused = false;
    }

    pub fn record_capture(&mut self, at_ms: i64) {
        self.last_capture_at = Some(at_ms.max(self.last_capture_at.unwrap_or(i64::MIN)));
    }

    pub fn record_dropped(&mut self, count: u64) {
        self.dropped_clips = self.dropped_clips.saturating_add(count);
    }

    /// Decide whether a request to suppress the Android 12+ notice may go
    /// through, recording the acknowledgement when it does.
    ///
    /// The gate lives here rather than in the Android binding for the same
    /// reason everything else does: the binding cannot be compiled on this
    /// host, and "never silently" is a rule that has to be enforced somewhere a
    /// test can reach. Turning suppression **off** is always allowed —
    /// restoring a privacy indicator is not a thing to gate.
    pub fn authorise_toast(
        &mut self,
        suppressed: bool,
        acknowledged: bool,
        now_ms: i64,
    ) -> std::result::Result<(), &'static str> {
        if !suppressed {
            return Ok(());
        }
        if !acknowledged {
            return Err(MSG_TOAST_UNEXPLAINED);
        }
        self.toast.acknowledged_at = Some(now_ms);
        Ok(())
    }

    pub fn toast_consent(&self) -> ToastConsent {
        self.toast
    }

    pub fn health(&self) -> CaptureHealth {
        if self.platform_rung == Rung::Desktop {
            return CaptureHealth::Working;
        }
        if !self.enabled {
            return CaptureHealth::Disabled;
        }
        if let Some(reason) = self.missing_grant() {
            return CaptureHealth::NotGranted { reason };
        }
        if !self.armed {
            return CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::NotArmed,
            };
        }
        if self.read_refused {
            return CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::ReadRefused,
            };
        }
        if self.last_read_ok_at.is_some() {
            CaptureHealth::Working
        } else {
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::AwaitingFirstCopy,
            }
        }
    }

    fn missing_grant(&self) -> Option<NotGrantedReason> {
        if !self.probe.supported {
            Some(NotGrantedReason::Unsupported)
        } else if !self.probe.installed {
            Some(NotGrantedReason::NotInstalled)
        } else if !self.probe.running {
            Some(NotGrantedReason::NotRunning)
        } else if !self.probe.permission {
            Some(NotGrantedReason::NoPermission)
        } else {
            None
        }
    }

    /// The rung actually in force — never the one that was asked for.
    pub fn rung(&self) -> Rung {
        match (self.platform_rung, self.health()) {
            (Rung::Desktop, _) => Rung::Desktop,
            (_, CaptureHealth::Working) => Rung::Shizuku,
            _ => Rung::InApp,
        }
    }

    pub fn next_step(&self) -> NextStep {
        match self.health() {
            CaptureHealth::Working | CaptureHealth::Disabled => NextStep::None,
            CaptureHealth::NotGranted { reason } => match reason {
                NotGrantedReason::Unsupported => NextStep::None,
                NotGrantedReason::NotInstalled => NextStep::InstallShizuku,
                NotGrantedReason::NotRunning => NextStep::StartShizuku,
                NotGrantedReason::NoPermission => NextStep::GrantPermission,
            },
            CaptureHealth::GrantedNotWorking { reason } => match reason {
                // Refused is the failure the device spike exists to find. There
                // is no next tap that fixes it, so offering one would be a
                // button that does nothing.
                NotWorkingReason::ReadRefused => NextStep::None,
                _ => NextStep::Arm,
            },
        }
    }

    pub fn snapshot(&self) -> CaptureSnapshot {
        let health = self.health();
        CaptureSnapshot {
            rung: self.rung(),
            health,
            shizuku: self.probe,
            next_step: self.next_step(),
            headline: headline(self.platform_rung, health),
            detail: detail(self.platform_rung, health),
            last_read_ok_at: self.last_read_ok_at,
            last_capture_at: self.last_capture_at,
            dropped_clips: self.dropped_clips,
            toast_suppressed: self.probe.toast_suppressed,
            toast_acknowledged: self.toast.acknowledged(),
            rearm_requested: self.probe.rearm_requested,
        }
    }
}

/// Capture state as the WebView sees it.
///
/// The android doc's §5 rule 1 puts this next to the history, not in a
/// diagnostics screen, which is why [`CaptureSnapshot::headline`] is a finished
/// sentence rather than a code the view has to translate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSnapshot {
    pub rung: Rung,
    pub health: CaptureHealth,
    pub shizuku: ShizukuProbe,
    pub next_step: NextStep,
    pub headline: &'static str,
    pub detail: Option<&'static str>,
    pub last_read_ok_at: Option<i64>,
    pub last_capture_at: Option<i64>,
    pub dropped_clips: u64,
    pub toast_suppressed: bool,
    pub toast_acknowledged: bool,
    /// Open the rung 2 screen with the start step selected, once.
    pub rearm_requested: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_700_000_000_000;

    fn granted() -> ShizukuProbe {
        ShizukuProbe {
            supported: true,
            installed: true,
            running: true,
            permission: true,
            toast_suppressed: false,
            rearm_requested: false,
        }
    }

    fn armed_model() -> CaptureModel {
        let mut model = CaptureModel::android();
        model.set_enabled(true);
        model.set_probe(granted());
        model.record_armed(true);
        model
    }

    /// `CopyPaste-qzhu`: a grant is not evidence. Everything can be in place
    /// and the state is still not `Working` until a read has actually returned.
    #[test]
    fn a_full_grant_is_not_working_until_a_read_succeeds() {
        let mut model = armed_model();
        assert_eq!(
            model.health(),
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::AwaitingFirstCopy
            }
        );
        assert_eq!(model.rung(), Rung::InApp);

        model.record_read(ReadOutcome::Succeeded, false, T0);
        assert_eq!(model.health(), CaptureHealth::Working);
        assert_eq!(model.rung(), Rung::Shizuku);
    }

    /// The subtle half of `CopyPaste-qzhu`. Any app may read the clipboard
    /// while it is in front, so a read taken with focus — the tile's, and the
    /// one `arm` performs while the user is on the setup screen — says nothing
    /// about whether background capture works. Counting it would turn the setup
    /// screen green at the exact moment it knows least.
    #[test]
    fn a_read_taken_with_focus_is_not_evidence_of_background_capture() {
        let mut model = armed_model();
        model.record_read(ReadOutcome::Succeeded, true, T0);
        assert_eq!(
            model.health(),
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::AwaitingFirstCopy
            }
        );

        model.record_read(ReadOutcome::Succeeded, false, T0 + 1);
        assert_eq!(model.health(), CaptureHealth::Working);
    }

    /// Being refused while we *do* have focus is a worse sign than being
    /// refused without it, so it is never discounted.
    #[test]
    fn a_refusal_counts_wherever_it_happened() {
        let mut model = armed_model();
        model.record_read(ReadOutcome::Succeeded, false, T0);
        model.record_read(ReadOutcome::Refused, true, T0 + 1);
        assert_eq!(
            model.health(),
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::ReadRefused
            }
        );
    }

    /// An empty clipboard is not a refusal and not a success.
    #[test]
    fn an_empty_read_moves_nothing_in_either_direction() {
        let mut model = armed_model();
        model.record_read(ReadOutcome::Empty, false, T0);
        assert!(matches!(
            model.health(),
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::AwaitingFirstCopy
            }
        ));

        model.record_read(ReadOutcome::Succeeded, false, T0);
        model.record_read(ReadOutcome::Empty, false, T0 + 1);
        assert_eq!(
            model.health(),
            CaptureHealth::Working,
            "an empty clipboard must not retract a proven read"
        );
    }

    /// The reboot case, which is the expected one rather than an error path.
    #[test]
    fn binder_death_drops_out_of_working_and_asks_for_a_restart() {
        let mut model = armed_model();
        model.record_read(ReadOutcome::Succeeded, false, T0);
        assert_eq!(model.health(), CaptureHealth::Working);

        model.record_loss();
        assert_eq!(model.rung(), Rung::InApp);
        assert_eq!(
            model.health(),
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::NotArmed
            }
        );
        assert_eq!(model.next_step(), NextStep::Arm);
        assert_ne!(model.snapshot().headline, LOST_TITLE);
    }

    /// A probe that finds Shizuku gone must clear the evidence too — otherwise
    /// the next arm would inherit a success from before the reboot.
    #[test]
    fn a_probe_that_loses_the_binder_clears_the_proof() {
        let mut model = armed_model();
        model.record_read(ReadOutcome::Succeeded, false, T0);
        model.set_probe(ShizukuProbe {
            running: false,
            ..granted()
        });
        assert_eq!(
            model.health(),
            CaptureHealth::NotGranted {
                reason: NotGrantedReason::NotRunning
            }
        );
        assert_eq!(model.next_step(), NextStep::StartShizuku);

        // …and re-arming without a fresh read does not resurrect it.
        model.set_probe(granted());
        model.record_armed(true);
        assert_ne!(model.health(), CaptureHealth::Working);
    }

    #[test]
    fn a_refused_read_is_reported_and_offers_no_button_that_would_do_nothing() {
        let mut model = armed_model();
        model.record_read(ReadOutcome::Refused, false, T0);
        assert_eq!(
            model.health(),
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::ReadRefused
            }
        );
        assert_eq!(model.next_step(), NextStep::None);
    }

    #[test]
    fn the_ladder_is_walked_in_order() {
        let mut model = CaptureModel::android();
        model.set_enabled(true);

        for (probe, reason, step) in [
            (
                ShizukuProbe::default(),
                NotGrantedReason::Unsupported,
                NextStep::None,
            ),
            (
                ShizukuProbe {
                    supported: true,
                    ..Default::default()
                },
                NotGrantedReason::NotInstalled,
                NextStep::InstallShizuku,
            ),
            (
                ShizukuProbe {
                    supported: true,
                    installed: true,
                    ..Default::default()
                },
                NotGrantedReason::NotRunning,
                NextStep::StartShizuku,
            ),
            (
                ShizukuProbe {
                    supported: true,
                    installed: true,
                    running: true,
                    ..Default::default()
                },
                NotGrantedReason::NoPermission,
                NextStep::GrantPermission,
            ),
        ] {
            model.set_probe(probe);
            assert_eq!(model.health(), CaptureHealth::NotGranted { reason });
            assert_eq!(model.next_step(), step);
        }
    }

    /// Off is off: a disabled model must not report a fault, because nothing
    /// is faulty.
    #[test]
    fn disabling_reads_as_a_choice_rather_than_a_failure() {
        let mut model = armed_model();
        model.record_read(ReadOutcome::Succeeded, false, T0);
        model.set_enabled(false);
        assert_eq!(model.health(), CaptureHealth::Disabled);
        assert_eq!(model.next_step(), NextStep::None);
        assert_eq!(model.rung(), Rung::InApp);
    }

    #[test]
    fn the_desktop_model_has_no_ladder() {
        let model = CaptureModel::desktop();
        assert_eq!(model.rung(), Rung::Desktop);
        assert_eq!(model.health(), CaptureHealth::Working);
        assert_eq!(model.next_step(), NextStep::None);
        assert!(model.snapshot().detail.is_none());
    }

    #[test]
    fn dropped_clips_accumulate_rather_than_being_swallowed() {
        let mut model = armed_model();
        model.record_dropped(3);
        model.record_dropped(2);
        assert_eq!(model.snapshot().dropped_clips, 5);
    }

    /// AGENTS.md rule 4: nothing shown to a user names a path. These strings
    /// are shown verbatim, so they are checked the same way `BackendError`'s
    /// are.
    #[test]
    fn every_sentence_is_a_sentence_and_names_no_path() {
        let healths = [
            CaptureHealth::Working,
            CaptureHealth::Disabled,
            CaptureHealth::NotGranted {
                reason: NotGrantedReason::Unsupported,
            },
            CaptureHealth::NotGranted {
                reason: NotGrantedReason::NotInstalled,
            },
            CaptureHealth::NotGranted {
                reason: NotGrantedReason::NotRunning,
            },
            CaptureHealth::NotGranted {
                reason: NotGrantedReason::NoPermission,
            },
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::AwaitingFirstCopy,
            },
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::ReadRefused,
            },
            CaptureHealth::GrantedNotWorking {
                reason: NotWorkingReason::NotArmed,
            },
        ];
        let mut texts = vec![TOAST_EXPLANATION, LOST_TITLE, LOST_BODY];
        for platform in [Rung::Desktop, Rung::InApp] {
            for health in healths {
                texts.push(headline(platform, health));
                texts.extend(detail(platform, health));
            }
        }
        for text in texts {
            assert!(text.ends_with('.'), "not a sentence: {text}");
            assert!(!text.contains('/'), "a path could hide here: {text}");
            assert!(
                !text.contains("Accessibility"),
                "v1 pointed users at a service that is not a clipboard exemption: {text}"
            );
        }
    }

    /// The consent text has to name what it turns off, or it is not consent.
    #[test]
    fn the_toast_explanation_says_what_it_turns_off_and_that_it_is_global() {
        assert!(TOAST_EXPLANATION.contains("every app"));
        assert!(TOAST_EXPLANATION.contains("turn it back on"));
    }

    /// Turning off one of the OS's privacy indicators without the user having
    /// been told is the move the android doc forbids by name.
    #[test]
    fn suppression_without_an_acknowledgement_is_refused() {
        let mut model = armed_model();
        assert_eq!(
            model.authorise_toast(true, false, T0),
            Err(MSG_TOAST_UNEXPLAINED)
        );
        assert!(!model.toast_consent().acknowledged());

        assert_eq!(model.authorise_toast(true, true, T0), Ok(()));
        assert!(model.toast_consent().acknowledged());
    }

    /// Restoring the indicator is never gated: a user who wants the warning
    /// back gets it back.
    #[test]
    fn restoring_the_notice_needs_no_acknowledgement() {
        let mut model = armed_model();
        assert_eq!(model.authorise_toast(false, false, T0), Ok(()));
    }

    #[test]
    fn a_snapshot_names_its_state_in_the_words_the_frontend_reads() {
        let mut model = armed_model();
        model.record_read(ReadOutcome::Succeeded, false, T0);
        let json = serde_json::to_string(&model.snapshot()).unwrap();
        assert!(json.contains(r#""rung":"shizuku""#), "{json}");
        assert!(json.contains(r#""state":"working""#), "{json}");
        assert!(json.contains(r#""nextStep":"none""#), "{json}");
        assert!(json.contains(r#""lastReadOkAt":1700000000000"#), "{json}");
    }

    /// The Kotlin side sends these; a rename on either side must fail here
    /// rather than at runtime on a phone nobody has.
    #[test]
    fn the_kotlin_payloads_deserialise_from_the_json_kotlin_sends() {
        let probe: ShizukuProbe = serde_json::from_str(
            r#"{"supported":true,"installed":true,"running":true,"permission":false,
                "enabled":true,"toastSuppressed":false,"rearmRequested":true}"#,
        )
        .unwrap();
        assert!(probe.running && !probe.permission && probe.rearm_requested);

        let status: AndroidProbeResult = serde_json::from_str(
            r#"{"probe":{"supported":true,"installed":true,"running":true,"permission":true,
                "toastSuppressed":false,"rearmRequested":false},"enabled":true,"listening":true}"#,
        )
        .unwrap();
        assert!(status.enabled && status.listening && status.probe.running);

        let arm: AndroidArmResult = serde_json::from_str(
            r#"{"probe":{"supported":true,"installed":true,"running":true,"permission":true,
                "toastSuppressed":false,"rearmRequested":false},"enabled":true,"listening":true,
                "outcome":"succeeded","focused":true}"#,
        )
        .unwrap();
        assert!(arm.status.enabled && arm.status.listening && arm.focused);
        assert_eq!(arm.outcome, ReadOutcome::Succeeded);

        let clip: Clip =
            serde_json::from_str(r#"{"text":"hi","source":"process_text","atMs":12}"#).unwrap();
        assert_eq!(clip.source, CaptureSource::ProcessText);
        assert_eq!(clip.at_ms, 12);

        let outcome: ReadOutcome = serde_json::from_str(r#""refused""#).unwrap();
        assert_eq!(outcome, ReadOutcome::Refused);

        // A probe from an older Kotlin that predates a field must not fail the
        // whole read — it degrades to "not supported", which is a refusal to
        // capture rather than a false claim to.
        let partial: ShizukuProbe = serde_json::from_str(r#"{"installed":true}"#).unwrap();
        assert!(!partial.supported);
    }
}
