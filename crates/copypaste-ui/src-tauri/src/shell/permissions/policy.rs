//! Maps OS facts onto the onboarding row. Kept free of `cfg` so Linux CI owns it.

use super::model::PermissionStatus;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AndroidNotificationFacts {
    pub api_level: u32,
    pub granted: bool,
    pub ever_asked: bool,
    pub show_rationale: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AndroidTileFacts {
    pub api_level: u32,
    pub last_add_result: Option<i32>,
    pub result_constants: TileAddResultConstants,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authorization {
    NotDetermined,
    Denied,
    Granted,
    #[allow(dead_code)]
    NotRequired,
}

#[cfg_attr(not(any(target_os = "macos", target_os = "android")), allow(dead_code))]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TileAddResultConstants {
    pub not_added: i32,
    pub already_added: i32,
    pub added: i32,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn android_tile_status(
    api_level: u32,
    last_add_result: Option<i32>,
    constants: TileAddResultConstants,
) -> PermissionStatus {
    if api_level < 33 {
        return PermissionStatus::Unavailable;
    }
    match last_add_result {
        None => PermissionStatus::Prompt,
        Some(result) if result == constants.added || result == constants.already_added => {
            PermissionStatus::Granted
        }
        Some(result) if result == constants.not_added => PermissionStatus::Denied,
        Some(_) => PermissionStatus::Denied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE_RESULTS: TileAddResultConstants = TileAddResultConstants {
        not_added: 0,
        already_added: 1,
        added: 2,
    };

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BridgeFixture {
        notification_facts: AndroidNotificationFacts,
        tile_facts: AndroidTileFacts,
    }

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
    fn android_api_policy_matrix() {
        for api_level in [24, 29] {
            assert_eq!(
                android_notification_authorization(api_level, false, false, false),
                Authorization::NotRequired
            );
            assert_eq!(
                android_tile_status(api_level, None, TILE_RESULTS),
                PermissionStatus::Unavailable
            );
        }

        for api_level in [33, 34, 36] {
            assert_eq!(
                android_notification_authorization(api_level, false, false, false),
                Authorization::NotDetermined
            );
            assert_eq!(
                android_notification_authorization(api_level, false, true, false),
                Authorization::Denied
            );
            assert_eq!(
                android_notification_authorization(api_level, false, true, true),
                Authorization::NotDetermined
            );
            assert_eq!(
                android_notification_authorization(api_level, true, true, false),
                Authorization::Granted
            );
            assert_eq!(
                android_tile_status(api_level, None, TILE_RESULTS),
                PermissionStatus::Prompt
            );
            assert_eq!(
                android_tile_status(api_level, Some(TILE_RESULTS.not_added), TILE_RESULTS),
                PermissionStatus::Denied
            );
            assert_eq!(
                android_tile_status(api_level, Some(i32::MAX), TILE_RESULTS),
                PermissionStatus::Denied
            );
            for result in [TILE_RESULTS.already_added, TILE_RESULTS.added] {
                assert_eq!(
                    android_tile_status(api_level, Some(result), TILE_RESULTS),
                    PermissionStatus::Granted
                );
            }
        }
    }

    #[test]
    fn rust_consumes_androids_permission_fact_fixture() {
        let fixture: BridgeFixture = serde_json::from_str(include_str!(
            "../../../gen/android/app/src/test/resources/capture-bridge-contract.json"
        ))
        .unwrap();
        assert_eq!(fixture.notification_facts.api_level, 36);
        assert!(fixture.notification_facts.granted);
        assert!(fixture.notification_facts.ever_asked);
        assert!(!fixture.notification_facts.show_rationale);
        assert_eq!(fixture.tile_facts.api_level, 36);
        assert_eq!(fixture.tile_facts.last_add_result, Some(2));
        assert_eq!(fixture.tile_facts.result_constants, TILE_RESULTS);
        assert_eq!(
            android_tile_status(
                fixture.tile_facts.api_level,
                fixture.tile_facts.last_add_result,
                fixture.tile_facts.result_constants,
            ),
            PermissionStatus::Granted
        );
    }
}
