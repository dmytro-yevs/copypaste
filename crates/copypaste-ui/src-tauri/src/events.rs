use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub enum TauriEventName {
    #[serde(rename = "copypaste://changed")]
    Changed,
    #[serde(rename = "copypaste://push-state")]
    PushState,
    #[serde(rename = "copypaste://captured")]
    Captured,
    #[serde(rename = "copypaste://capture-state")]
    CaptureState,
    #[serde(rename = "private-mode-changed")]
    PrivateModeChanged,
    #[serde(rename = "autostart-changed")]
    AutostartChanged,
    #[serde(rename = "open-settings")]
    OpenSettings,
}

impl TauriEventName {
    pub const ALL: [Self; 7] = [
        Self::Changed,
        Self::PushState,
        Self::Captured,
        Self::CaptureState,
        Self::PrivateModeChanged,
        Self::AutostartChanged,
        Self::OpenSettings,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "copypaste://changed",
            Self::PushState => "copypaste://push-state",
            Self::Captured => "copypaste://captured",
            Self::CaptureState => "copypaste://capture-state",
            Self::PrivateModeChanged => "private-mode-changed",
            Self::AutostartChanged => "autostart-changed",
            Self::OpenSettings => "open-settings",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn runtime_names_match_the_generated_serialization_contract() {
        let mut unique = HashSet::new();
        for event in TauriEventName::ALL {
            assert_eq!(serde_json::to_value(event).unwrap(), event.as_str());
            assert!(unique.insert(event.as_str()));
        }
    }
}
