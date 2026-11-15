//! Producer-provided clipboard hints, checked before any payload is read.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipboardPrivacy {
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub transient: bool,
}

impl ClipboardPrivacy {
    pub fn is_empty(&self) -> bool {
        !self.secret && !self.transient
    }
    pub fn allows(self, settings: &crate::ConfigData) -> bool {
        !(self.secret && settings.skip_secret || self.transient && settings.skip_transient)
    }
    pub fn union(self, other: Self) -> Self {
        Self {
            secret: self.secret || other.secret,
            transient: self.transient || other.transient,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn producer_hints_have_independent_safe_defaults() {
        let mut settings = crate::ConfigData::default();
        let secret = ClipboardPrivacy {
            secret: true,
            transient: false,
        };
        let temporary = ClipboardPrivacy {
            secret: false,
            transient: true,
        };
        assert!(!secret.allows(&settings));
        assert!(!temporary.allows(&settings));
        settings.skip_secret = false;
        assert!(secret.allows(&settings));
        assert!(!temporary.allows(&settings));
        settings.skip_transient = false;
        assert!(secret.union(temporary).allows(&settings));
    }
    #[test]
    fn upgrades_keep_both_gates_and_patches_are_live() {
        let mut record = serde_json::to_value(crate::ConfigData::default()).unwrap();
        record.as_object_mut().unwrap().remove("skip_secret");
        record.as_object_mut().unwrap().remove("skip_transient");
        let restored: crate::ConfigData = serde_json::from_value(record).unwrap();
        assert!(restored.skip_secret && restored.skip_transient);
        let next = crate::ConfigPatch {
            skip_secret: Some(false),
            ..Default::default()
        }
        .apply(&restored)
        .unwrap();
        assert!(!next.skip_secret && next.skip_transient);
        assert!(
            crate::ConfigData::field_liveness().contains(&("skip_secret", crate::Liveness::Live))
        );
    }
}
