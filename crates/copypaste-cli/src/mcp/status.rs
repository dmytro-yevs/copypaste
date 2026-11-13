//! What the `status` tool and `copypaste mcp status` say: whether CopyPaste
//! is reachable, whether AI tools are on, what this connection may do, and
//! the one next step that unblocks access. Never clipboard content.

use copypaste_ipc::ai::{AiLookback, AiScope, AiSessionState, AiStatus};

pub(super) const NOT_RUNNING: &str = "CopyPaste is not running. Open CopyPaste, then retry.";

/// The one sentence a runtime `next_step` code stands for.
fn next_step(code: &str) -> Option<&'static str> {
    Some(match code {
        "enable_ai_tools" => {
            "AI tools are off on this computer. Turn them on in CopyPaste Settings, AI tools."
        }
        "approve_in_app" => "Waiting for approval in the CopyPaste app on this computer.",
        "open_app_to_approve" => {
            "This AI tool is not authorized yet. Open CopyPaste on this computer to approve it."
        }
        "access_denied" => {
            "Access was denied in CopyPaste. Connect this AI tool from CopyPaste Settings, AI tools."
        }
        "connect_again" => {
            "This connection was revoked. Connect the AI tool again in CopyPaste Settings, AI tools."
        }
        "restart_client" => "This session has ended. Restart the AI tool to reconnect.",
        "close_other_sessions" => {
            "Too many AI tool sessions are open. Close another one, then restart this AI tool."
        }
        _ => return None,
    })
}

fn scope_label(scope: AiScope) -> &'static str {
    match scope {
        AiScope::ReadHistory => "read history",
        AiScope::ReadSnippets => "read snippets",
        AiScope::WriteClipboard => "write clipboard",
        AiScope::Organize => "organize",
    }
}

fn lookback_label(lookback: AiLookback) -> &'static str {
    match lookback {
        AiLookback::Day => "24 hours",
        AiLookback::Week => "7 days",
        AiLookback::Month => "30 days",
        AiLookback::All => "all history",
    }
}

/// The status text; `None` means the daemon could not be reached.
pub(super) fn describe(status: Option<&AiStatus>) -> String {
    let Some(status) = status else {
        return NOT_RUNNING.to_owned();
    };
    let mut lines = vec!["CopyPaste is running.".to_owned()];
    match status.session {
        Some(AiSessionState::Authorized) => {
            let scopes: Vec<&str> = status.scopes.iter().map(|s| scope_label(*s)).collect();
            lines.push(format!(
                "This AI tool is connected. Scopes: {}.",
                scopes.join(", ")
            ));
            if let Some(lookback) = status.lookback {
                lines.push(format!(
                    "History it can read: {} (pinned items always).",
                    lookback_label(lookback)
                ));
            }
        }
        Some(_) => {}
        None => lines.push(format!(
            "AI tools are {} on this computer.",
            if status.mcp_enabled { "on" } else { "off" }
        )),
    }
    lines.push(format!(
        "Devices contributing history: {}.",
        status.contributing_devices
    ));
    if let Some(step) = status.next_step.as_deref().and_then(next_step) {
        lines.push(step.to_owned());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(session: Option<AiSessionState>, next: Option<&str>) -> AiStatus {
        AiStatus {
            mcp_enabled: true,
            session,
            scopes: vec![AiScope::ReadHistory, AiScope::ReadSnippets],
            lookback: Some(AiLookback::Week),
            contributing_devices: 2,
            next_step: next.map(str::to_owned),
        }
    }

    #[test]
    fn every_state_names_its_next_step() {
        assert_eq!(describe(None), NOT_RUNNING);

        let connected = describe(Some(&status(Some(AiSessionState::Authorized), None)));
        assert!(connected.contains("Scopes: read history, read snippets."));
        assert!(connected.contains("7 days"));

        let off = AiStatus {
            mcp_enabled: false,
            ..status(None, Some("enable_ai_tools"))
        };
        let off = describe(Some(&off));
        assert!(off.contains("AI tools are off on this computer."));
        assert!(off.contains("Settings, AI tools"));

        for code in [
            "approve_in_app",
            "open_app_to_approve",
            "access_denied",
            "connect_again",
            "restart_client",
            "close_other_sessions",
        ] {
            assert!(next_step(code).is_some(), "{code}");
        }
    }
}
