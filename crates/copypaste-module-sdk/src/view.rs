//! A structured result the host renders with its own components.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const MAX_VIEW_SECTIONS: usize = 32;
pub const MAX_VIEW_ITEMS: usize = 256;
pub const MAX_ROW_ACTIONS: usize = 4;
pub const MAX_VIEW_BLOCK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleView {
    pub sections: Vec<ViewSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ViewSection {
    pub title: Option<String>,
    pub items: Vec<ViewItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewItem {
    Row {
        title: String,
        subtitle: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        actions: Vec<ViewAction>,
    },
    /// Monospace text with a host copy button.
    Block { label: Option<String>, text: String },
}

/// Invokes a command of the same module; its output replaces the view when it
/// is itself a view, otherwise the host shows it and re-invokes the command
/// that produced this view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ViewAction {
    pub title: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub arguments: BTreeMap<String, Value>,
    /// Rendered as destructive and confirmed by the host before running.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub destructive: bool,
}

impl ModuleView {
    /// Shape limits; the host checks action commands against the manifest.
    pub fn validate(&self) -> Result<(), String> {
        let text = |value: &str, limit: usize| value.chars().count() <= limit;
        let valid = self.sections.len() <= MAX_VIEW_SECTIONS
            && self
                .sections
                .iter()
                .map(|section| section.items.len())
                .sum::<usize>()
                <= MAX_VIEW_ITEMS
            && self.sections.iter().all(|section| {
                section
                    .title
                    .as_deref()
                    .is_none_or(|title| text(title, 160))
                    && section.items.iter().all(|item| match item {
                        ViewItem::Row {
                            title,
                            subtitle,
                            actions,
                        } => {
                            text(title, 256)
                                && subtitle.as_deref().is_none_or(|value| text(value, 1024))
                                && actions.len() <= MAX_ROW_ACTIONS
                                && actions.iter().all(|action| {
                                    !action.title.trim().is_empty()
                                        && text(&action.title, 64)
                                        && crate::valid_id(&action.command)
                                })
                        }
                        ViewItem::Block { label, text: body } => {
                            label.as_deref().is_none_or(|value| text(value, 160))
                                && body.len() <= MAX_VIEW_BLOCK_BYTES
                        }
                    })
            });
        if valid {
            Ok(())
        } else {
            Err("The module view exceeds its limits.".into())
        }
    }
}
