//! A structured result the host renders with its own components.
use crate::{ModuleField, ModuleIcon};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const MAX_VIEW_SECTIONS: usize = 32;
pub const MAX_VIEW_ITEMS: usize = 256;
pub const MAX_ROW_ACTIONS: usize = 4;
pub const MAX_VIEW_BLOCK_BYTES: usize = 64 * 1024;
pub const MAX_BADGES: usize = 4;
pub const MAX_TOOLBAR_ACTIONS: usize = 6;
pub const MAX_FILTER_OPTIONS: usize = 256;
pub const MAX_RESULTS: usize = 200;

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
    /// Actions on the selected rows, which the host passes as `ids` (schema 7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bulk_actions: Vec<ViewAction>,
    /// Runs after the user drags a row, with `id` and `before`: the id of the
    /// row it now precedes, or an empty string at the end (schema 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reorder: Option<ViewAction>,
    /// Loads more rows; its view replaces the current one (schema 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub more: Option<ViewAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ViewItem {
    Row {
        title: String,
        subtitle: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        actions: Vec<ViewAction>,
        /// Needed for selection and reordering (schema 7).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Short labels after the title (schema 7).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        badges: Vec<String>,
        /// The host masks the subtitle (schema 7).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        secret: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<ModuleIcon>,
        /// Runs when the row itself is activated (schema 7).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        open: Option<ViewAction>,
    },
    /// Monospace text with a host copy button.
    Block { label: Option<String>, text: String },
    /// A search field; each change runs `action` with `query` (schema 7).
    Search {
        query: String,
        placeholder: String,
        action: ViewAction,
    },
    /// One choice among options, such as a scope or a collection; picking
    /// one runs `action` with `value` (schema 7).
    Filter {
        options: Vec<FilterOption>,
        selected: String,
        action: ViewAction,
    },
    /// A row of buttons (schema 7).
    Toolbar { actions: Vec<ViewAction> },
    /// Inline text in the host's notice style (schema 7).
    Notice { text: String, tone: NoticeTone },
    /// Inputs submitted together as the `values` argument (schema 7).
    Form(ViewForm),
    /// A preference of this module, rendered and saved by the host (schema 7).
    Preference { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FilterOption {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ModuleIcon>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NoticeTone {
    Info,
    Warning,
    Error,
}

/// `submit` receives the field values as `values`; `preview`, when present,
/// runs with the same argument after edits and its `Text` or `Message` is
/// shown below the fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ViewForm {
    pub fields: Vec<ModuleField>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub values: Map<String, Value>,
    /// Messages shown under fields, by field id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub errors: BTreeMap<String, String>,
    pub submit: ViewAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<ViewAction>,
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
    /// Where a resulting view opens (schema 7).
    #[serde(default, skip_serializing_if = "ViewTarget::is_replace")]
    pub target: ViewTarget,
    /// The host authenticates the device owner first (schema 7).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_auth: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ModuleIcon>,
}

/// A drawer closes when an action inside it returns anything but a view, and
/// the view under it runs again.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViewTarget {
    #[default]
    Replace,
    Drawer,
}

impl ViewTarget {
    fn is_replace(&self) -> bool {
        *self == Self::Replace
    }
}

/// One entry of a Quick Paste surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResultItem {
    /// Passed to the surface's activate command as `id`.
    pub id: String,
    pub title: String,
    pub preview: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub badges: Vec<String>,
    /// The preview is masked and activation authenticates when the module
    /// asks for it in its delivery.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
    /// At most one result is preselected, e.g. an exact abbreviation match.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub preselect: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ViewAction>,
}

/// Text the host pastes, inserts, copies or queues, depending on where the
/// user activated it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub text: String,
    /// Written as a secret and never recorded in history.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_auth: bool,
    /// Record the delivered text in history; ignored for secrets.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub record_history: bool,
    /// The provenance label of a recorded item.
    pub label: String,
    /// Where the caret goes after insertion, counted back from the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_graphemes_from_end: Option<u32>,
}

impl ViewSection {
    #[must_use]
    pub fn new(title: Option<String>, items: Vec<ViewItem>) -> Self {
        Self {
            title,
            items,
            bulk_actions: Vec::new(),
            reorder: None,
            more: None,
        }
    }
}

impl ViewItem {
    #[must_use]
    pub fn row(title: String, subtitle: Option<String>, actions: Vec<ViewAction>) -> Self {
        Self::Row {
            title,
            subtitle,
            actions,
            id: None,
            badges: Vec::new(),
            secret: false,
            icon: None,
            open: None,
        }
    }
}

impl ViewAction {
    #[must_use]
    pub fn new(title: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            command: command.into(),
            arguments: BTreeMap::new(),
            destructive: false,
            target: ViewTarget::Replace,
            require_auth: false,
            icon: None,
        }
    }
}

fn text(value: &str, limit: usize) -> bool {
    value.chars().count() <= limit
}

fn short_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256
}

fn badges(values: &[String]) -> bool {
    values.len() <= MAX_BADGES && values.iter().all(|badge| text(badge, 32))
}

impl ViewAction {
    fn valid(&self) -> bool {
        !self.title.trim().is_empty() && text(&self.title, 64) && crate::valid_id(&self.command)
    }
}

impl ViewItem {
    fn valid(&self) -> bool {
        match self {
            Self::Row {
                title,
                subtitle,
                actions,
                id,
                badges: labels,
                open,
                ..
            } => {
                text(title, 256)
                    && subtitle.as_deref().is_none_or(|value| text(value, 1024))
                    && actions.len() <= MAX_ROW_ACTIONS
                    && actions.iter().all(ViewAction::valid)
                    && id.as_deref().is_none_or(short_id)
                    && badges(labels)
                    && open.as_ref().is_none_or(ViewAction::valid)
            }
            Self::Block { label, text: body } => {
                label.as_deref().is_none_or(|value| text(value, 160))
                    && body.len() <= MAX_VIEW_BLOCK_BYTES
            }
            Self::Search {
                query,
                placeholder,
                action,
            } => text(query, 1024) && text(placeholder, 160) && action.valid(),
            Self::Filter {
                options,
                selected,
                action,
            } => {
                !options.is_empty()
                    && options.len() <= MAX_FILTER_OPTIONS
                    && options.iter().enumerate().all(|(index, option)| {
                        short_id(&option.id)
                            && !option.title.trim().is_empty()
                            && text(&option.title, 64)
                            && !options[..index].iter().any(|other| other.id == option.id)
                    })
                    && options.iter().any(|option| &option.id == selected)
                    && action.valid()
            }
            Self::Toolbar { actions } => {
                !actions.is_empty()
                    && actions.len() <= MAX_TOOLBAR_ACTIONS
                    && actions.iter().all(ViewAction::valid)
            }
            Self::Notice { text: body, .. } => !body.trim().is_empty() && text(body, 1024),
            Self::Form(form) => form.valid(),
            Self::Preference { id } => crate::valid_id(id),
        }
    }
}

impl ViewForm {
    fn valid(&self) -> bool {
        let field = |id: &str| self.fields.iter().find(|field| field.id == id);
        !self.fields.is_empty()
            && crate::manifest::validate_fields(&self.fields).is_ok()
            && self.fields.iter().all(|field| {
                !matches!(
                    field.value,
                    crate::ModuleFieldValue::IdList {}
                        | crate::ModuleFieldValue::Values {}
                        | crate::ModuleFieldValue::DeviceSet {}
                        | crate::ModuleFieldValue::SourceAppSet {}
                )
            })
            && self
                .values
                .iter()
                .all(|(id, value)| field(id).is_some_and(|field| field.accepts_stored(value)))
            && self
                .errors
                .iter()
                .all(|(id, message)| field(id).is_some() && text(message, 1024))
            && self.submit.valid()
            && self.preview.as_ref().is_none_or(ViewAction::valid)
    }
}

impl ModuleView {
    /// Shape limits; the host checks action commands against the manifest.
    pub fn validate(&self) -> Result<(), String> {
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
                    && section.items.iter().all(ViewItem::valid)
                    && section.bulk_actions.len() <= MAX_ROW_ACTIONS
                    && section.bulk_actions.iter().all(ViewAction::valid)
                    && section.reorder.as_ref().is_none_or(ViewAction::valid)
                    && section.more.as_ref().is_none_or(ViewAction::valid)
            });
        if valid {
            Ok(())
        } else {
            Err("The module view exceeds its limits.".into())
        }
    }

    /// Every command the view can run, for the host's manifest check.
    pub fn commands(&self) -> impl Iterator<Item = &str> {
        self.sections.iter().flat_map(|section| {
            let items = section.items.iter().flat_map(|item| -> Vec<&ViewAction> {
                match item {
                    ViewItem::Row { actions, open, .. } => actions.iter().chain(open).collect(),
                    ViewItem::Search { action, .. } | ViewItem::Filter { action, .. } => {
                        vec![action]
                    }
                    ViewItem::Toolbar { actions } => actions.iter().collect(),
                    ViewItem::Form(form) => {
                        std::iter::once(&form.submit).chain(&form.preview).collect()
                    }
                    ViewItem::Block { .. }
                    | ViewItem::Notice { .. }
                    | ViewItem::Preference { .. } => Vec::new(),
                }
            });
            items
                .chain(&section.bulk_actions)
                .chain(&section.reorder)
                .chain(&section.more)
                .map(|action| action.command.as_str())
        })
    }
}

/// Shape limits of a Quick Paste result list.
pub fn validate_results(items: &[ResultItem]) -> Result<(), String> {
    let valid = items.len() <= MAX_RESULTS
        && items.iter().filter(|item| item.preselect).count() <= 1
        && items.iter().all(|item| {
            short_id(&item.id)
                && text(&item.title, 256)
                && text(&item.preview, 1024)
                && badges(&item.badges)
                && item.actions.len() <= MAX_ROW_ACTIONS
                && item.actions.iter().all(ViewAction::valid)
        });
    if valid {
        Ok(())
    } else {
        Err("The module results exceed their limits.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn library_views_and_results_validate_and_name_their_commands() {
        let view: ModuleView = serde_json::from_value(json!({"sections": [{
            "title": null,
            "items": [
                {"kind": "search", "query": "", "placeholder": "Search",
                 "action": {"title": "Search", "command": "library"}},
                {"kind": "filter", "selected": "all", "options": [
                    {"id": "all", "title": "All", "count": 2}, {"id": "c1", "title": "Work"}],
                 "action": {"title": "Show", "command": "library"}},
                {"kind": "toolbar", "actions": [
                    {"title": "New", "command": "edit", "target": "drawer"}]},
                {"kind": "row", "id": "s1", "title": "Address", "subtitle": "Main St",
                 "badges": [";addr"], "secret": true,
                 "open": {"title": "Edit", "command": "edit", "target": "drawer",
                          "require_auth": true}},
                {"kind": "form", "fields": [
                    {"id": "body", "title": "Body", "kind": "text", "default": "",
                     "multiline": true, "template": true}],
                 "values": {"body": "Hi {{name}}"}, "errors": {"body": "Unknown field"},
                 "submit": {"title": "Save", "command": "save"},
                 "preview": {"title": "Preview", "command": "preview"}},
                {"kind": "preference", "id": "record-pasted"}],
            "bulk_actions": [{"title": "Delete", "command": "delete", "destructive": true}],
            "reorder": {"title": "Move", "command": "move"},
            "more": {"title": "More", "command": "library", "arguments": {"page": "2"}}}]}))
        .unwrap();
        view.validate().unwrap();
        let commands: std::collections::BTreeSet<_> = view.commands().collect();
        assert_eq!(
            commands.into_iter().collect::<Vec<_>>(),
            ["delete", "edit", "library", "move", "preview", "save"]
        );

        let mut invalid = view.clone();
        let ViewItem::Form(form) = &mut invalid.sections[0].items[4] else {
            panic!("form");
        };
        form.errors.insert("missing".into(), "x".into());
        assert!(invalid.validate().is_err());

        let result = |preselect| ResultItem {
            id: "s1".into(),
            title: "Address".into(),
            preview: "Main St".into(),
            badges: vec![";addr".into()],
            secret: false,
            preselect,
            actions: Vec::new(),
        };
        validate_results(&[result(true), result(false)]).unwrap();
        assert!(validate_results(&[result(true), result(true)]).is_err());
    }
}
