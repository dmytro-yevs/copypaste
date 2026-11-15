//! The AI tools module end to end: the built package installed in a daemon,
//! its permissions confirmed, an MCP client connected over the socket like
//! the CLI, an access prompt answered by the app, redacted history served
//! from the runtime, and disabling ending every connection.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use copypaste_ipc::transport::{self, OwnedReadHalf, OwnedWriteHalf};
use copypaste_ipc::{
    EndpointConnected, EndpointFrame, Method, ModuleOperation, Request, Response, ResponseData,
    PROTOCOL_VERSION,
};
use copypaste_module_sdk::{ModuleCapability, ModulePrompt};
use copypaste_modules::{test_support::PackageSigner, InstalledModule};
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tokio_util::codec::{FramedRead, LinesCodec};

use super::{bind, run};
use crate::testutil::{test_socket_path, test_state};
use crate::AppState;

const MODULE: &str = "copypaste.ai-tools";
const WAIT: Duration = Duration::from_secs(30);

async fn modules(state: &AppState, operation: ModuleOperation) -> Value {
    let response = state
        .runtime
        .request(1, Method::Modules { operation })
        .await;
    match response.data {
        Some(ResponseData::Modules { json }) => serde_json::from_str(&json).unwrap(),
        _ => panic!("the module operation failed: {:?}", response.error),
    }
}

async fn prompts(state: &AppState, count: usize) -> Vec<ModulePrompt> {
    tokio::time::timeout(WAIT, async {
        loop {
            let pending: Vec<ModulePrompt> =
                serde_json::from_value(modules(state, ModuleOperation::Prompts).await).unwrap();
            if pending.len() == count {
                return pending;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the prompt count settles")
}

async fn set_enabled(state: &AppState, enabled: bool, capabilities: Vec<ModuleCapability>) {
    modules(
        state,
        ModuleOperation::SetEnabled {
            id: MODULE.into(),
            enabled,
            capabilities,
        },
    )
    .await;
}

/// An MCP client on one endpoint connection, as `copypaste module connect`
/// relays it.
struct McpClient {
    writer: OwnedWriteHalf,
    lines: FramedRead<OwnedReadHalf, LinesCodec>,
}

impl McpClient {
    async fn connect(path: &Path) -> Self {
        let (reader, writer) = transport::connect(path).await.unwrap().into_split();
        let mut client = Self {
            writer,
            lines: FramedRead::new(reader, LinesCodec::new()),
        };
        let request = Request {
            id: 1,
            protocol_version: PROTOCOL_VERSION,
            method: Method::Modules {
                operation: ModuleOperation::Connect {
                    id: MODULE.into(),
                    endpoint: "mcp".into(),
                },
            },
        };
        client
            .write(&serde_json::to_string(&request).unwrap())
            .await;
        let line = client.line().await.expect("a connect reply");
        let response: Response = serde_json::from_str(&line).unwrap();
        let Some(ResponseData::Modules { json }) = response.data else {
            panic!("the endpoint was refused: {:?}", response.error);
        };
        serde_json::from_str::<EndpointConnected>(&json).unwrap();
        client
    }

    async fn write(&mut self, line: &str) {
        self.writer.write_all(line.as_bytes()).await.unwrap();
        self.writer.write_all(b"\n").await.unwrap();
    }

    async fn line(&mut self) -> Option<String> {
        tokio::time::timeout(WAIT, self.lines.next())
            .await
            .expect("the daemon answers in time")
            .and_then(Result::ok)
    }

    async fn send(&mut self, message: Value) {
        let frame = EndpointFrame::Data {
            data: message.to_string(),
        };
        self.write(&serde_json::to_string(&frame).unwrap()).await;
    }

    /// The next JSON-RPC message, or `None` once the host closed.
    async fn message(&mut self) -> Option<Value> {
        let line = self.line().await?;
        match serde_json::from_str(&line).unwrap() {
            EndpointFrame::Data { data } => Some(serde_json::from_str(&data).unwrap()),
            EndpointFrame::Close { .. } => None,
        }
    }

    async fn until(&mut self, matches: impl Fn(&Value) -> bool) -> Value {
        loop {
            let message = self.message().await.expect("the connection is open");
            if matches(&message) {
                return message;
            }
        }
    }

    async fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        self.until(|message| message["id"] == json!(id)).await
    }

    async fn initialize(&mut self, client: &str) {
        let reply = self
            .call(
                1,
                "initialize",
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": client, "version": "1.0"}
                }),
            )
            .await;
        assert_eq!(reply["result"]["serverInfo"]["name"], "copypaste");
        self.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
            .await;
    }

    /// Reads until the host sends `close` and hangs up.
    async fn closed(&mut self) {
        while self.message().await.is_some() {}
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ai_tools_serve_redacted_history_over_an_endpoint_until_disabled() {
    let (state, dir) = test_state("ai-tools-endpoint");
    state.set_ready(true);
    let signer = PackageSigner::default();
    state
        .runtime
        .modules
        .trust_package_key(&signer.public_key());
    let path = test_socket_path(&dir, "daemon.sock");
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server = tokio::spawn(run(bind(&path).unwrap(), Arc::clone(&state), shutdown_rx));

    let package = dir.path().join("ai-tools.cpmodule");
    signer.package_module(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../modules/ai-tools"),
        &package,
    );
    let installed: InstalledModule = serde_json::from_value(
        modules(
            &state,
            ModuleOperation::Install {
                package_path: package.to_string_lossy().into(),
            },
        )
        .await,
    )
    .unwrap();
    assert!(!installed.enabled, "permissions wait for confirmation");
    set_enabled(&state, true, installed.capabilities.clone()).await;
    let added = state
        .runtime
        .request(
            2,
            Method::Add {
                content: "settle the invoice with card 4111 1111 1111 1111 today".into(),
            },
        )
        .await;
    assert!(added.ok, "{:?}", added.error);

    let mut client = McpClient::connect(&path).await;
    client.initialize("Claude").await;
    let asked = prompts(&state, 1).await;
    assert_eq!(asked[0].module_id, MODULE);
    assert!(asked[0].title.contains("Claude"));
    modules(
        &state,
        ModuleOperation::AnswerPrompt {
            prompt_id: asked[0].prompt_id.clone(),
            choice: Some("once".into()),
        },
    )
    .await;
    client
        .until(|message| message["method"] == "notifications/tools/list_changed")
        .await;
    let tools = client.call(2, "tools/list", json!({})).await;
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(names.contains(&"search_history"), "{names:?}");
    let found = client
        .call(
            3,
            "tools/call",
            json!({"name": "search_history", "arguments": {"query": "invoice"}}),
        )
        .await;
    let text = found["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("[redacted: payment card]"), "{text}");
    assert!(!text.contains("4111"), "{text}");

    // A second client waits on its own access prompt; disabling dismisses
    // it before the module is torn down, or teardown would wait it out.
    let mut waiting = McpClient::connect(&path).await;
    waiting.initialize("Cursor").await;
    prompts(&state, 1).await;
    tokio::time::timeout(WAIT, set_enabled(&state, false, Vec::new()))
        .await
        .expect("disabling does not wait for the pending prompt");
    assert!(prompts(&state, 0).await.is_empty());
    client.closed().await;
    waiting.closed().await;

    shutdown_tx.send(true).unwrap();
    state.release_drain_listener();
    let _ = server.await;
}
