//! Places in the app where the host shows a module (schema 7), and host
//! services a module implements for other modules.
use crate::{ModuleCommand, ModuleFieldValue, ModuleHostRequest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_ITEM_ACTIONS: usize = 4;
pub const MAX_TRAY_ACTIONS: usize = 4;
pub const MAX_MIXED_RESULTS: u32 = 10;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModuleSurfaces {
    /// A top-level Settings section, titled like the module, showing the
    /// command's view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<SettingsSurface>,
    /// Results in Quick Paste, the Android keyboard panel and the widget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_paste: Option<QuickPasteSurface>,
    /// Actions on text history items: History menus and inspectors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_actions: Vec<ItemAction>,
    /// Desktop tray and menu bar entries. Each opens the settings section
    /// with the command's view in a drawer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tray_actions: Vec<TrayAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SettingsSurface {
    /// Takes no arguments; returns a `View`.
    pub command: String,
    pub icon: ModuleIcon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuickPasteSurface {
    /// The tab, keyboard panel tab and widget filter label.
    pub title: String,
    /// Takes `query` and returns `Results`; an empty query lists the tab.
    pub search_command: String,
    /// Takes `id` and returns `Deliver`, or a `View` whose form submit does.
    pub activate_command: String,
    /// Results shown above history for a non-empty query; 0 keeps them in
    /// the tab.
    #[serde(default)]
    pub mixed_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ItemAction {
    pub id: String,
    pub title: String,
    /// Takes `item_id` and `text`; may return a `View` the host opens in a
    /// drawer.
    pub command: String,
    pub icon: ModuleIcon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrayAction {
    pub id: String,
    pub title: String,
    /// Takes no arguments.
    pub command: String,
}

/// Icons the host maps to its own icon set.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModuleIcon {
    NotebookText,
    FilePlus,
    Bookmark,
    Sparkles,
    Puzzle,
    Zap,
}

/// A host service other modules request, answered by this module's command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceProvider {
    pub service: ModuleService,
    /// A hidden command taking `request`, a `ModuleHostRequest` of the
    /// service, and returning `Data` holding a `ServiceReply`.
    pub command: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ModuleService {
    /// `snippet_search`, `snippet_get`, `snippet_render`, `snippet_save`.
    Snippets,
}

impl ModuleService {
    #[must_use]
    pub fn serves(self, request: &ModuleHostRequest) -> bool {
        match self {
            Self::Snippets => matches!(
                request,
                ModuleHostRequest::SnippetSearch { .. }
                    | ModuleHostRequest::SnippetGet { .. }
                    | ModuleHostRequest::SnippetRender { .. }
                    | ModuleHostRequest::SnippetSave { .. }
            ),
        }
    }
}

/// The `Data` a service provider command returns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServiceReply {
    Ok { reply: serde_json::Value },
    Error { error: crate::HostError },
}

impl ModuleSurfaces {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.settings.is_none()
            && self.quick_paste.is_none()
            && self.item_actions.is_empty()
            && self.tray_actions.is_empty()
    }

    pub(crate) fn validate(&self, commands: &[ModuleCommand]) -> Result<(), String> {
        let takes = |id: &str, arguments: &[&str]| {
            commands.iter().any(|command| {
                command.id == id
                    && command.arguments.len() == arguments.len()
                    && command.arguments.iter().all(|field| {
                        arguments.contains(&field.id.as_str())
                            && matches!(field.value, ModuleFieldValue::Text { .. })
                    })
            })
        };
        let title = |title: &str| !title.trim().is_empty() && title.len() <= 160;
        let mut ids = BTreeSet::new();
        let valid = self
            .settings
            .as_ref()
            .is_none_or(|surface| takes(&surface.command, &[]))
            && self.quick_paste.as_ref().is_none_or(|surface| {
                title(&surface.title)
                    && takes(&surface.search_command, &["query"])
                    && takes(&surface.activate_command, &["id"])
                    && surface.mixed_limit <= MAX_MIXED_RESULTS
            })
            && self.item_actions.len() <= MAX_ITEM_ACTIONS
            && self.item_actions.iter().all(|action| {
                crate::valid_id(&action.id)
                    && ids.insert(("item", &action.id))
                    && title(&action.title)
                    && takes(&action.command, &["item_id", "text"])
            })
            && self.tray_actions.len() <= MAX_TRAY_ACTIONS
            && self.tray_actions.iter().all(|action| {
                crate::valid_id(&action.id)
                    && ids.insert(("tray", &action.id))
                    && title(&action.title)
                    && takes(&action.command, &[])
            });
        if valid {
            Ok(())
        } else {
            Err("The module surfaces are invalid.".into())
        }
    }
}

pub(crate) fn validate_providers(
    providers: &[ServiceProvider],
    commands: &[ModuleCommand],
) -> Result<(), String> {
    let mut services = BTreeSet::new();
    let valid = providers.iter().all(|provider| {
        services.insert(provider.service)
            && commands.iter().any(|command| {
                command.id == provider.command
                    && command.hidden
                    && command.arguments.len() == 1
                    && command.arguments[0].id == "request"
                    && matches!(
                        command.arguments[0].value,
                        ModuleFieldValue::Text { required: true, .. }
                    )
            })
    });
    if valid {
        Ok(())
    } else {
        Err("The module service providers are invalid.".into())
    }
}
