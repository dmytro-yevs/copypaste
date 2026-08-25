use serde::{Deserialize, Serialize};

use super::model::{Clip, ReadOutcome, ShizukuProbe, LOST_BODY, LOST_TITLE, ONGOING_TEXT};

/// Rust sends this copy before arming so process teardown cannot force Kotlin
/// to invent notification wording while Rust is no longer scheduled.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AndroidArmRequest<'a> {
    ongoing_text: &'a str,
    lost_title: &'a str,
    lost_body: &'a str,
}

impl AndroidArmRequest<'static> {
    pub(crate) fn current() -> Self {
        Self {
            ongoing_text: ONGOING_TEXT,
            lost_title: LOST_TITLE,
            lost_body: LOST_BODY,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AndroidEmptyResult {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AndroidProbeResult {
    pub probe: ShizukuProbe,
    pub enabled: bool,
    pub listening: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AndroidArmResult {
    pub probe: ShizukuProbe,
    pub enabled: bool,
    pub listening: bool,
    pub outcome: ReadOutcome,
    pub focused: bool,
    #[serde(rename = "notificationPermission")]
    pub _notification_permission: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AndroidReadResult {
    pub outcome: ReadOutcome,
    pub text: Option<String>,
    pub at_ms: i64,
    pub focused: bool,
    pub source_app_bundle_id: Option<String>,
    pub source_app_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AndroidDrainResult {
    pub clips: Vec<Clip>,
    pub dropped: u64,
    pub probe: ShizukuProbe,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::model::CaptureSource;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct CaptureBridgeFixture {
        probe: AndroidProbeResult,
        arms: Vec<AndroidArmResult>,
        arm_request: FixtureArmRequest,
        notification_facts: serde_json::Value,
        tile_facts: serde_json::Value,
        reads: Vec<AndroidReadResult>,
        drain: AndroidDrainResult,
        empty: AndroidEmptyResult,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FixtureArmRequest {
        ongoing_text: String,
        lost_title: String,
        lost_body: String,
    }

    #[test]
    fn rust_consumes_the_fixture_emitted_by_kotlins_production_serializers() {
        let fixture: CaptureBridgeFixture = serde_json::from_str(include_str!(
            "../../gen/android/app/src/test/resources/capture-bridge-contract.json"
        ))
        .unwrap();

        assert!(fixture.probe.enabled && fixture.probe.listening);
        assert!(fixture.probe.probe.running && fixture.probe.probe.enabled);
        assert_eq!(
            fixture
                .arms
                .iter()
                .map(|arm| arm.outcome)
                .collect::<Vec<_>>(),
            vec![
                ReadOutcome::Succeeded,
                ReadOutcome::Empty,
                ReadOutcome::Refused,
            ]
        );
        assert!(fixture.arms.iter().all(|arm| arm.focused));
        assert!(fixture
            .arms
            .iter()
            .all(|arm| arm.probe.running && arm.probe.enabled));
        assert_eq!(
            fixture
                .arms
                .iter()
                .map(|arm| arm._notification_permission)
                .collect::<Vec<_>>(),
            vec![true, true, false]
        );
        assert!(fixture.arms[0].enabled && fixture.arms[0].listening);
        assert!(fixture.arms[1..]
            .iter()
            .all(|arm| !arm.enabled && !arm.listening));
        assert_eq!(fixture.arm_request.ongoing_text, "ongoing");
        assert_eq!(fixture.arm_request.lost_title, "lost title");
        assert_eq!(fixture.arm_request.lost_body, "lost body");
        let _ = fixture.notification_facts;
        let _ = fixture.tile_facts;
        assert_eq!(
            fixture
                .reads
                .iter()
                .map(|read| read.outcome)
                .collect::<Vec<_>>(),
            vec![
                ReadOutcome::Succeeded,
                ReadOutcome::Empty,
                ReadOutcome::Refused,
            ]
        );
        assert!(fixture.reads[0].text.is_some());
        assert_eq!(
            fixture.reads[0].source_app_bundle_id.as_deref(),
            Some("com.example.writer")
        );
        assert_eq!(fixture.reads[0].source_app_name.as_deref(), Some("Writer"));
        assert!(fixture.reads[1..].iter().all(|read| read.text.is_none()));
        assert!(fixture.reads.iter().all(|read| read.focused));
        assert_eq!(fixture.reads[0].at_ms, 1_700_000_000_001);
        assert_eq!(
            fixture
                .drain
                .clips
                .iter()
                .map(|clip| clip.source)
                .collect::<Vec<_>>(),
            vec![
                CaptureSource::InApp,
                CaptureSource::Share,
                CaptureSource::ProcessText,
                CaptureSource::Tile,
                CaptureSource::Background,
            ]
        );
        assert_eq!(fixture.drain.dropped, 2);
        let _ = fixture.empty;
        assert!(fixture.drain.probe.running);
        assert_eq!(
            fixture
                .drain
                .clips
                .last()
                .unwrap()
                .source_app_bundle_id
                .as_deref(),
            Some("com.example.writer")
        );
    }

    #[test]
    fn rust_authors_every_persisted_capture_sentence() {
        let request = serde_json::to_value(AndroidArmRequest::current()).unwrap();
        assert_eq!(request["ongoingText"], ONGOING_TEXT);
        assert_eq!(request["lostTitle"], LOST_TITLE);
        assert_eq!(request["lostBody"], LOST_BODY);
    }

    #[test]
    fn capture_bridge_structs_reject_unknown_fields() {
        let json = r#"{"supported":true,"installed":true,"running":true,"permission":true,
            "enabled":true,"toastSuppressed":false,"rearmRequested":false,"surprise":true}"#;
        let error = serde_json::from_str::<ShizukuProbe>(json).unwrap_err();
        assert!(error.to_string().contains("unknown field `surprise`"));
    }

    #[test]
    fn capture_bridge_structs_reject_missing_fields() {
        let partial = r#"{"installed":true}"#;
        assert!(serde_json::from_str::<ShizukuProbe>(partial).is_err());
    }
}
