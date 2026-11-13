//! `copypaste mcp`: an MCP server over stdio that AI tools launch.
//!
//! The process holds no history and no resident state: it translates MCP
//! to the daemon's AI operations, where every privacy, scope and limit check
//! runs, and exits when stdin closes. It never sends a privileged operation
//! (approvals, grants, policy); the daemon refuses those to anything but the
//! CopyPaste app.

mod server;
mod status;
mod tools;

use std::time::Duration;

use copypaste_ipc::ai::AiOp;
use copypaste_ipc::{socket_path, Method, ResponseData};
use rmcp::ServiceExt;

use crate::client;
use crate::error::CliError;
use crate::out;
use server::{Backoff, Config, McpServer};

/// The environment variable that carries the grant credential.
const CREDENTIAL_ENV: &str = "COPYPASTE_MCP_GRANT";

/// Retry while CopyPaste is not running: soon at first, then twice a minute.
const BACKOFF: Backoff = Backoff {
    first: Duration::from_secs(1),
    max: Duration::from_secs(30),
};

fn credential() -> Option<String> {
    std::env::var(CREDENTIAL_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// The executable that launched this process, where the OS reports it.
/// Shown to the user as reported, never trusted.
fn launcher() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let parent = std::os::unix::process::parent_id();
        std::fs::read_link(format!("/proc/{parent}/exe"))
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Serve MCP on stdin and stdout until stdin closes.
pub(crate) async fn serve() -> Result<(), CliError> {
    let server = McpServer::new(Config {
        socket: socket_path(),
        credential: credential(),
        executable: launcher(),
        backoff: BACKOFF,
    });
    let running = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| CliError::local(format!("the MCP client did not initialize: {error}")))?;
    running
        .waiting()
        .await
        .map_err(|error| CliError::local(format!("the MCP server stopped: {error}")))?;
    Ok(())
}

/// `copypaste mcp status`: the `status` tool's answer, without a session.
pub(crate) async fn status(json: bool) -> Result<(), CliError> {
    let response = client::request(Method::Ai {
        op: AiOp::Status { session_id: None },
    })
    .await?;
    if json {
        let text = serde_json::to_string_pretty(&response)
            .map_err(|e| CliError::local(format!("could not render the reply as JSON: {e}")))?;
        out(&text);
    }
    let Some(ResponseData::AiStatus(ai)) = client::into_data(response)? else {
        return Err(CliError::local(
            "the daemon did not answer with AI status; the daemon and this CLI may be \
             different versions",
        ));
    };
    if !json {
        let credential = if credential().is_some() {
            format!("{CREDENTIAL_ENV} is set.")
        } else {
            format!(
                "{CREDENTIAL_ENV} is not set; the first connection asks for approval in CopyPaste."
            )
        };
        out(&format!("{}\n{credential}", status::describe(Some(&ai))));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::client::stub::{socket_path, ScriptedDaemon};
    use copypaste_ipc::ai::{AiScope, AiSession, AiSessionState, AiStatus};
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};
    use tokio::io::DuplexStream;
    use tokio_util::codec::{Framed, LinesCodec};

    const WAIT: Duration = Duration::from_secs(5);

    /// A raw JSON-RPC client on one end of an in-memory pipe.
    struct Client {
        framed: Framed<DuplexStream, LinesCodec>,
        notifications: Vec<String>,
    }

    impl Client {
        fn start(name: &str) -> Self {
            let (server_io, client_io) = tokio::io::duplex(64 * 1024);
            let server = McpServer::new(Config {
                socket: socket_path(name),
                credential: Some("grant-credential".to_owned()),
                executable: None,
                backoff: Backoff {
                    first: Duration::from_millis(20),
                    max: Duration::from_millis(40),
                },
            });
            tokio::spawn(async move {
                if let Ok(running) = server.serve(server_io).await {
                    let _ = running.waiting().await;
                }
            });
            Self {
                framed: Framed::new(client_io, LinesCodec::new()),
                notifications: Vec::new(),
            }
        }

        async fn send(&mut self, message: Value) {
            self.framed.send(message.to_string()).await.unwrap();
        }

        async fn next(&mut self) -> Value {
            let line = tokio::time::timeout(WAIT, self.framed.next())
                .await
                .expect("a message in time")
                .expect("an open pipe")
                .unwrap();
            serde_json::from_str(&line).unwrap()
        }

        /// The response to request `id`, recording notifications on the way.
        async fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
            self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
                .await;
            loop {
                let message = self.next().await;
                if message["id"] == json!(id) {
                    return message;
                }
                if let Some(method) = message["method"].as_str() {
                    self.notifications.push(method.to_owned());
                }
            }
        }

        async fn initialize(&mut self, id: u64) -> Value {
            let params = json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test-client", "version": "1.0" },
            });
            let reply = self.request(id, "initialize", params).await;
            self.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .await;
            reply
        }

        async fn tool_names(&mut self, id: u64) -> Vec<String> {
            let reply = self.request(id, "tools/list", json!({})).await;
            reply["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tool| tool["name"].as_str().unwrap().to_owned())
                .collect()
        }

        async fn wait_for(&mut self, notification: &str) {
            while !self.notifications.iter().any(|seen| seen == notification) {
                let message = self.next().await;
                if let Some(method) = message["method"].as_str() {
                    self.notifications.push(method.to_owned());
                }
            }
            self.notifications.clear();
        }
    }

    fn status(session: AiSessionState, scopes: Vec<AiScope>) -> AiStatus {
        AiStatus {
            mcp_enabled: true,
            session: Some(session),
            scopes,
            lookback: None,
            contributing_devices: 1,
            next_step: None,
        }
    }

    /// Answers session and status requests with whatever `current` holds.
    fn daemon(name: &str, current: Arc<Mutex<AiStatus>>) -> ScriptedDaemon {
        ScriptedDaemon::start(name, move |method| match method {
            Method::Ai {
                op: AiOp::SessionOpen { .. },
            } => ResponseData::AiSession(AiSession {
                session_id: "session-1".to_owned(),
                authorized_scopes: None,
                pending_request_id: None,
            }),
            Method::Ai {
                op: AiOp::Status { .. },
            } => ResponseData::AiStatus(current.lock().unwrap().clone()),
            _ => ResponseData::Empty {},
        })
    }

    /// AC1, AC3, AC5, AC18, AC25: only `status` until CopyPaste runs and the
    /// session is authorized; the list follows scopes and revocation.
    #[tokio::test]
    async fn the_tool_list_follows_reachability_and_authorization() {
        let name = "mcp-list";
        let mut client = Client::start(name);
        assert_eq!(
            client.initialize(1).await["result"]["serverInfo"]["name"],
            "copypaste"
        );
        assert_eq!(client.tool_names(2).await, ["status"]);
        let reply = client
            .request(
                3,
                "tools/call",
                json!({ "name": "status", "arguments": {} }),
            )
            .await;
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("not running"), "{text}");

        let current = Arc::new(Mutex::new(status(
            AiSessionState::Authorized,
            vec![AiScope::ReadHistory],
        )));
        let stub = daemon(name, Arc::clone(&current));
        client.wait_for("notifications/tools/list_changed").await;
        assert_eq!(
            client.tool_names(4).await,
            ["status", "list_recent", "search_history", "get_item"]
        );
        let attached = stub.methods().iter().any(|method| {
            matches!(method, Method::Ai { op: AiOp::SessionAttach { session_id } }
                if session_id == "session-1")
        });
        assert!(attached, "the session is bound to the watch connection");
        let privileged = stub.methods().iter().any(|method| match method {
            Method::Ai { op } => copypaste_ipc::OpGates::privileged(op),
            _ => false,
        });
        assert!(!privileged, "the CLI never sends a privileged operation");

        *current.lock().unwrap() = status(AiSessionState::Revoked, Vec::new());
        stub.push_event();
        client.wait_for("notifications/tools/list_changed").await;
        assert_eq!(client.tool_names(5).await, ["status"]);
    }

    /// AC24: a request before `initialize` is answered with its own id, and
    /// the handshake still succeeds afterwards.
    #[tokio::test]
    async fn a_probe_before_initialize_gets_an_error_with_its_id() {
        let mut client = Client::start("mcp-probe");
        let probe = client.request(7, "tools/list", json!({})).await;
        assert_eq!(probe["id"], 7);
        assert!(probe["error"].is_object(), "{probe}");
        let reply = client.initialize(8).await;
        assert!(reply["result"]["protocolVersion"].is_string(), "{reply}");
        assert_eq!(client.tool_names(9).await, ["status"]);
    }
}
