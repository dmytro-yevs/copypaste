//! The sentences the user reads.
//!
//! Separated from the state machine because they change for different reasons
//! and by different hands: `model` decides which state the device is in, this
//! decides what that state is called. Keeping them apart is also what keeps
//! either file inside the rule 5 budget.
//!
//! Every string here is shown verbatim — `CaptureSnapshot::headline` is not a
//! code the view translates — so each is a complete sentence, names no
//! filesystem path (AGENTS.md rule 4), and points at nothing that does not
//! work. v1's did the last of those wrong: it told users to "enable
//! AccessibilityService instead", which is not a clipboard exemption and which
//! v1 had not implemented.

use super::model::{CaptureHealth, NotGrantedReason, NotWorkingReason, Rung};

/// The exact text that must be on screen before suppression can be turned on.
///
/// Held here, not in the view, so the test below can assert that it names what
/// is being turned off. A consent screen that does not say what it is asking
/// for is the thing the android doc refuses.
pub const TOAST_EXPLANATION: &str = "Android shows \"Shell pasted from your clipboard\" the first \
     time something reads a clip. That notice is the system telling you CopyPaste read your \
     clipboard, and turning it off turns it off for every app, not just this one. You can turn \
     it back on here at any time.";

/// Refusing a suppression request that nobody explained.
pub const MSG_TOAST_UNEXPLAINED: &str =
    "CopyPaste won't turn off Android's clipboard notice until you've read what that does.";

/// The notification raised the instant background capture stops. Notification
/// copy only — in the app the user is already looking at the button, so "tap
/// to restart" has nothing to tap on, and the screen is often reached
/// deliberately rather than because something just stopped.
pub const LOST_TITLE: &str = "Background capture stopped.";
pub const LOST_BODY: &str =
    "CopyPaste is only saving what you copy inside the app. Tap to restart.";
pub const ONGOING_TEXT: &str = "Capturing from every app.";

/// What rung 0 saves, in the words the onboarding uses. Repeated in several
/// details below because it is the sentence that keeps the app from implying
/// it is capturing when it is not.
const RUNG0: &str = "CopyPaste is saving what you copy inside the app, share to it, or capture \
     with the Quick Settings tile — plus everything your Mac copies.";

pub(super) fn headline(platform: Rung, health: CaptureHealth) -> &'static str {
    if platform == Rung::Desktop {
        return "Capturing everything you copy.";
    }
    match health {
        CaptureHealth::Working => ONGOING_TEXT,
        CaptureHealth::Disabled => "Background capture is off.",
        CaptureHealth::NotGranted { .. } => "Background capture isn't set up.",
        CaptureHealth::GrantedNotWorking { reason } => match reason {
            NotWorkingReason::AwaitingFirstCopy => "Background capture is armed.",
            NotWorkingReason::ReadRefused => "Background capture isn't working.",
            NotWorkingReason::NotArmed => "Background capture needs turning back on.",
        },
    }
}

pub(super) fn detail(platform: Rung, health: CaptureHealth) -> Option<&'static str> {
    if platform == Rung::Desktop {
        return None;
    }
    Some(match health {
        CaptureHealth::Working => return None,
        CaptureHealth::Disabled => RUNG0,
        CaptureHealth::NotGranted { reason } => match reason {
            NotGrantedReason::Unsupported => {
                "Capturing from other apps needs Android 11 or later. Everything else works here."
            }
            NotGrantedReason::NotInstalled => {
                "Install Shizuku to capture from every app. Until then, CopyPaste is saving what \
                 you copy inside the app, share to it, or capture with the tile."
            }
            NotGrantedReason::NotRunning => {
                "Shizuku isn't running yet. Start it once so CopyPaste can apply the background-\
                 capture grants, then return here."
            }
            NotGrantedReason::NoPermission => {
                "Shizuku is running but hasn't given CopyPaste permission yet."
            }
        },
        CaptureHealth::GrantedNotWorking { reason } => match reason {
            NotWorkingReason::AwaitingFirstCopy => {
                "Waiting for your first copy in another app to confirm it works."
            }
            NotWorkingReason::ReadRefused => {
                "CopyPaste couldn't confirm background clipboard reads from other apps. Only what \
                 you copy inside the app is being saved."
            }
            NotWorkingReason::NotArmed => {
                "Background capture is set up but not running. Turning it back on takes one tap."
            }
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut texts = vec![
            TOAST_EXPLANATION,
            MSG_TOAST_UNEXPLAINED,
            LOST_TITLE,
            LOST_BODY,
            ONGOING_TEXT,
        ];
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
}
