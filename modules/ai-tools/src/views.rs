//! Host-rendered views: connected tools with their actions, connection
//! instructions, and one tool's activity.
use crate::access_log::Entry;
use crate::grants::{self, Grant, Scope};
use crate::service::Service;
use crate::strings::{fill, text, Text};
use copypaste_module_sdk::{HostError, ModuleView, ViewAction, ViewItem, ViewSection};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(crate) const CONNECT_COMMAND: [&str; 4] = ["module", "connect", "copypaste.ai-tools", "mcp"];
const CLI: &str = "copypaste";
const MAX_ACTIVITY_ROWS: usize = 200;

fn action(
    title: &str,
    command: &str,
    arguments: BTreeMap<String, Value>,
    destructive: bool,
) -> ViewAction {
    ViewAction {
        title: title.to_owned(),
        command: command.to_owned(),
        arguments,
        destructive,
    }
}

fn day(day: i64) -> String {
    jiff::Timestamp::from_second(day.saturating_mul(86_400))
        .map(|at| at.strftime("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn grant_row(locale: &str, grant: &Grant) -> ViewItem {
    let scopes = grant.scopes();
    let mut parts: Vec<String> = scopes
        .iter()
        .map(|scope| {
            text(
                locale,
                match scope {
                    Scope::ReadHistory => Text::ReadHistory,
                    Scope::ReadSnippets => Text::ReadSnippets,
                    Scope::WriteClipboard => Text::WriteClipboard,
                    Scope::Organize => Text::Organize,
                },
            )
            .to_owned()
        })
        .collect();
    parts.push(if grant.revoked() {
        text(locale, Text::Revoked).to_owned()
    } else {
        match grant.last_used_day {
            Some(used) => fill(text(locale, Text::LastUsed), &[("date", &day(used))]),
            None => text(locale, Text::NeverUsed).to_owned(),
        }
    });
    let owner = grant
        .owner_name
        .as_deref()
        .unwrap_or(text(locale, Text::UnknownDevice));
    parts.push(fill(text(locale, Text::Owner), &[("device", owner)]));
    let id = || BTreeMap::from([("grant".to_owned(), json!(grant.id))]);
    let mut actions = vec![action(
        text(locale, Text::ViewActivity),
        "activity",
        id(),
        false,
    )];
    if grant.revoked() {
        actions.push(action(text(locale, Text::Remove), "remove", id(), true));
    } else {
        let write = scopes.contains(&Scope::WriteClipboard);
        let mut arguments = id();
        arguments.insert("write".to_owned(), json!(!write));
        let title = if write {
            Text::ReadOnly
        } else {
            Text::AllowChanges
        };
        actions.push(action(text(locale, title), "set-access", arguments, false));
        actions.push(action(text(locale, Text::Revoke), "revoke", id(), true));
    }
    ViewItem::Row {
        title: grant.client_name.clone(),
        subtitle: Some(parts.join(" · ")),
        actions,
    }
}

pub(crate) fn connections(service: &Service, locale: &str) -> Result<ModuleView, HostError> {
    let grants = grants::list(service.host())?;
    let mut items: Vec<ViewItem> = grants
        .iter()
        .rev()
        .map(|grant| grant_row(locale, grant))
        .collect();
    if items.is_empty() {
        items.push(ViewItem::Row {
            title: text(locale, Text::NoConnections).to_owned(),
            subtitle: Some(text(locale, endpoint_hint()).to_owned()),
            actions: Vec::new(),
        });
    }
    Ok(ModuleView {
        sections: vec![ViewSection {
            title: Some(text(locale, Text::ConnectionsTitle).to_owned()),
            items,
        }],
    })
}

/// Android has no endpoint; its history reaches AI tools through a computer.
fn endpoint_hint() -> Text {
    if cfg!(target_os = "android") {
        Text::ServedMessage
    } else {
        Text::SetupHint
    }
}

fn quoted(value: &str) -> String {
    serde_json::Value::String(value.to_owned()).to_string()
}

/// The server entry for common MCP client configuration formats.
pub(crate) fn client_configs() -> Vec<(&'static str, String)> {
    let args: Vec<String> = CONNECT_COMMAND.iter().map(|arg| quoted(arg)).collect();
    let server = json!({ "command": CLI, "args": CONNECT_COMMAND });
    let mut vscode = server.clone();
    vscode["type"] = json!("stdio");
    let pretty = |value: Value| serde_json::to_string_pretty(&value).unwrap_or_default();
    vec![
        (
            "MCP (JSON)",
            pretty(json!({ "mcpServers": { "copypaste": server } })),
        ),
        (
            "Claude Code",
            format!(
                "claude mcp add --scope user copypaste -- {CLI} {}",
                CONNECT_COMMAND.join(" ")
            ),
        ),
        (
            "Codex (TOML)",
            format!(
                "[mcp_servers.copypaste]\ncommand = {}\nargs = [{}]\n",
                quoted(CLI),
                args.join(", ")
            ),
        ),
        (
            "VS Code (JSON)",
            pretty(json!({ "servers": { "copypaste": vscode } })),
        ),
    ]
}

pub(crate) fn setup(locale: &str) -> ModuleView {
    if cfg!(target_os = "android") {
        return ModuleView {
            sections: vec![ViewSection {
                title: Some(text(locale, Text::ServedTitle).to_owned()),
                items: vec![ViewItem::Row {
                    title: text(locale, Text::ServedMessage).to_owned(),
                    subtitle: None,
                    actions: Vec::new(),
                }],
            }],
        };
    }
    let mut items = vec![ViewItem::Row {
        title: text(locale, Text::SetupHint).to_owned(),
        subtitle: Some(format!(
            "{} {}",
            text(locale, Text::PolicyNote),
            text(locale, Text::RestartNote)
        )),
        actions: Vec::new(),
    }];
    items.extend(
        client_configs()
            .into_iter()
            .map(|(label, config)| ViewItem::Block {
                label: Some(label.to_owned()),
                text: config,
            }),
    );
    ModuleView {
        sections: vec![ViewSection {
            title: Some(text(locale, Text::Connect).to_owned()),
            items,
        }],
    }
}

fn entry_row(locale: &str, entry: &Entry) -> ViewItem {
    let mut title = fill(
        text(locale, Text::ActivityEntry),
        &[
            ("tool", &entry.tool),
            ("count", &entry.result_count.to_string()),
        ],
    );
    if let Some(error) = &entry.error {
        title.push_str(&format!(" · {error}"));
    }
    ViewItem::Row {
        title,
        subtitle: jiff::Timestamp::from_millisecond(entry.at)
            .ok()
            .map(|at| at.strftime("%Y-%m-%d %H:%M:%S UTC").to_string()),
        actions: Vec::new(),
    }
}

pub(crate) fn activity(
    service: &Service,
    locale: &str,
    grant_id: &str,
) -> Result<ModuleView, HostError> {
    let name = grants::get(service.host(), grant_id)?
        .map_or_else(|| grant_id.to_owned(), |grant| grant.client_name);
    let entries = service.activity(grant_id);
    let mut items: Vec<ViewItem> = entries
        .iter()
        .take(MAX_ACTIVITY_ROWS)
        .map(|entry| entry_row(locale, entry))
        .collect();
    if items.is_empty() {
        items.push(ViewItem::Row {
            title: text(locale, Text::ActivityEmpty).to_owned(),
            subtitle: None,
            actions: Vec::new(),
        });
    }
    Ok(ModuleView {
        sections: vec![ViewSection {
            title: Some(fill(text(locale, Text::ActivityTitle), &[("name", &name)])),
            items,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_configs_launch_the_connect_command_and_parse() {
        for (label, config) in client_configs() {
            assert!(
                config.contains("copypaste.ai-tools") && config.contains("connect"),
                "{label}: {config}"
            );
            if label.ends_with("(JSON)") {
                let value: Value = serde_json::from_str(&config).unwrap();
                let root = value.as_object().unwrap().values().next().unwrap();
                assert_eq!(root["copypaste"]["args"], json!(CONNECT_COMMAND));
            }
        }
        let view = setup("uk");
        view.validate().unwrap();
        assert!(matches!(
            &view.sections[0].items[1],
            ViewItem::Block { text, .. } if text.contains("module")
        ));
    }
}
