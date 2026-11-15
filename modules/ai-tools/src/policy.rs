//! The exposure policy: synced preferences turned into the `HistoryScope`
//! every history request carries. The host hides sensitive items itself.
use copypaste_module_sdk::{HistoryContentKind, HistoryScope};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) const OTHER_DEVICES: &str = "other-devices";
pub(crate) const EXCLUDED_DEVICES: &str = "excluded-devices";
pub(crate) const CONTENT_KINDS: &str = "content-kinds";
pub(crate) const EXCLUDED_APPS: &str = "excluded-apps";
pub(crate) const LOOKBACK: &str = "lookback";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Lookback {
    Day,
    Week,
    Month,
    All,
}

impl Lookback {
    fn parse(id: &str) -> Option<Self> {
        Some(match id {
            "day" => Self::Day,
            "week" => Self::Week,
            "month" => Self::Month,
            "all" => Self::All,
            _ => return None,
        })
    }

    pub(crate) fn days(self) -> Option<u32> {
        match self {
            Self::Day => Some(1),
            Self::Week => Some(7),
            Self::Month => Some(30),
            Self::All => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Day => "24 hours",
            Self::Week => "7 days",
            Self::Month => "30 days",
            Self::All => "all history",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Policy {
    pub(crate) scope: HistoryScope,
    pub(crate) lookback: Lookback,
}

fn kind(id: &str) -> Option<HistoryContentKind> {
    Some(match id {
        "text" => HistoryContentKind::Text,
        "link" => HistoryContentKind::Link,
        "rich-text" => HistoryContentKind::RichText,
        "image" => HistoryContentKind::Image,
        "file" => HistoryContentKind::File,
        _ => return None,
    })
}

fn strings(value: Option<&Value>) -> Option<Vec<&str>> {
    value?.as_array()?.iter().map(Value::as_str).collect()
}

impl Policy {
    /// The policy for stored preference values; anything missing or
    /// malformed takes the manifest default.
    pub(crate) fn from_preferences(values: &BTreeMap<String, Value>) -> Self {
        let ids = |key: &str| strings(values.get(key));
        let members = |key: &str| -> Vec<String> {
            ids(key)
                .unwrap_or_default()
                .into_iter()
                .map(str::to_owned)
                .collect()
        };
        let mut content_kinds: Vec<HistoryContentKind> = ids(CONTENT_KINDS)
            .unwrap_or_else(|| vec!["text", "link", "rich-text", "file"])
            .into_iter()
            .filter_map(kind)
            .collect();
        content_kinds.sort();
        content_kinds.dedup();
        // Several selected windows read as the shortest one.
        let lookback = ids(LOOKBACK)
            .unwrap_or_default()
            .into_iter()
            .filter_map(Lookback::parse)
            .min()
            .unwrap_or(Lookback::Week);
        Self {
            scope: HistoryScope {
                include_other_devices: values
                    .get(OTHER_DEVICES)
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                excluded_devices: members(EXCLUDED_DEVICES),
                excluded_source_apps: members(EXCLUDED_APPS),
                content_kinds,
                lookback_days: lookback.days(),
            },
            lookback,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_share_recent_text_from_every_device_and_sets_become_exclusions() {
        let defaults = Policy::from_preferences(&BTreeMap::new());
        assert!(defaults.scope.include_other_devices);
        assert_eq!(defaults.scope.lookback_days, Some(7));
        assert!(!defaults
            .scope
            .content_kinds
            .contains(&HistoryContentKind::Image));

        let policy = Policy::from_preferences(&BTreeMap::from([
            (OTHER_DEVICES.into(), json!(false)),
            (EXCLUDED_DEVICES.into(), json!(["phone"])),
            (EXCLUDED_APPS.into(), json!(["com.bank"])),
            (CONTENT_KINDS.into(), json!([])),
            (LOOKBACK.into(), json!(["all", "day"])),
        ]));
        assert!(!policy.scope.include_other_devices);
        assert_eq!(policy.scope.excluded_devices, ["phone"]);
        assert_eq!(policy.scope.excluded_source_apps, ["com.bank"]);
        assert!(policy.scope.content_kinds.is_empty(), "nothing is shared");
        assert_eq!(policy.lookback, Lookback::Day);
    }
}
