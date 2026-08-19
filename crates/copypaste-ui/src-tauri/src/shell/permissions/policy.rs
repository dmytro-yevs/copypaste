//! Maps OS facts onto the onboarding row. Kept free of `cfg` so Linux CI owns it.

use super::model::PermissionStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authorization {
    NotDetermined,
    Denied,
    Granted,
    #[allow(dead_code)]
    NotRequired,
}

pub fn notification_status(authorization: Authorization) -> PermissionStatus {
    match authorization {
        Authorization::NotDetermined => PermissionStatus::Prompt,
        Authorization::Denied => PermissionStatus::Denied,
        Authorization::Granted => PermissionStatus::Granted,
        Authorization::NotRequired => PermissionStatus::NotRequired,
    }
}

/// Android < 13 has no `POST_NOTIFICATIONS` prompt. Never asked + not granted
/// is still a prompt: `shouldShowRequestPermissionRationale` is false then too.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn android_notification_authorization(
    api_level: u32,
    granted: bool,
    ever_asked: bool,
    show_rationale: bool,
) -> Authorization {
    if api_level < 33 {
        return Authorization::NotRequired;
    }
    if granted {
        return Authorization::Granted;
    }
    if !ever_asked || show_rationale {
        Authorization::NotDetermined
    } else {
        Authorization::Denied
    }
}

/// `StatusBarManager.requestAddTileService` result codes (API 33).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn tile_status_from_add_result(result: i32) -> PermissionStatus {
    match result {
        1 | 2 => PermissionStatus::Granted,
        _ => PermissionStatus::Denied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_rows_never_become_required() {
        for status in [
            PermissionStatus::Prompt,
            PermissionStatus::Granted,
            PermissionStatus::Denied,
            PermissionStatus::NotRequired,
        ] {
            let item = super::super::model::PermissionItem::of(
                super::super::model::PermissionId::Notifications,
                status,
            );
            assert!(!item.required);
        }
    }

    #[test]
    fn first_android_13_ask_is_a_prompt_even_without_rationale() {
        assert_eq!(
            android_notification_authorization(33, false, false, false),
            Authorization::NotDetermined
        );
    }

    #[test]
    fn a_permanent_android_denial_is_denied() {
        assert_eq!(
            android_notification_authorization(33, false, true, false),
            Authorization::Denied
        );
    }

    #[test]
    fn pre_tiramisu_has_no_notification_prompt() {
        assert_eq!(
            android_notification_authorization(32, false, false, false),
            Authorization::NotRequired
        );
    }

    #[test]
    fn already_added_tiles_count_as_granted() {
        assert_eq!(tile_status_from_add_result(1), PermissionStatus::Granted);
        assert_eq!(tile_status_from_add_result(2), PermissionStatus::Granted);
        assert_eq!(tile_status_from_add_result(0), PermissionStatus::Denied);
    }
}
