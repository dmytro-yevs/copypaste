use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use copypaste_ipc::transport::{self, OwnedReadHalf, OwnedWriteHalf, Stream};
use copypaste_ipc::{ErrorCode, Method, Request, Response, ResponseData, PROTOCOL_VERSION};
use futures_util::StreamExt;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio_util::codec::{FramedRead, LinesCodec};

use super::{bind, run};
use crate::testutil::test_state;

enum Expected {
    Data(fn(&ResponseData) -> bool),
    Error(ErrorCode),
}

fn expected(method: &Method) -> Expected {
    match method {
        Method::Status => Expected::Data(|data| matches!(data, ResponseData::Status(_))),
        Method::SetDeviceName { .. } => {
            Expected::Data(|data| matches!(data, ResponseData::Empty { .. }))
        }
        Method::List { .. } | Method::Search { .. } => {
            Expected::Data(|data| matches!(data, ResponseData::Page(_)))
        }
        Method::Add { .. } => Expected::Data(|data| matches!(data, ResponseData::Item(_))),
        Method::DeleteAll { .. } | Method::ReorderPinned { .. } | Method::HistoryCeiling => {
            Expected::Data(|data| matches!(data, ResponseData::Count(_)))
        }
        Method::PairCreateInvite => {
            Expected::Data(|data| matches!(data, ResponseData::PairingInvite(_)))
        }
        Method::PairProgress | Method::PairCancel => {
            Expected::Data(|data| matches!(data, ResponseData::PairingProgress(_)))
        }
        Method::Revoke { .. } => Expected::Data(|data| matches!(data, ResponseData::Empty { .. })),
        Method::Peers => Expected::Data(|data| matches!(data, ResponseData::Peers(_))),
        Method::SyncNow { .. } => Expected::Data(|data| matches!(data, ResponseData::Sync(_))),
        Method::Discovered | Method::Rescan => {
            Expected::Data(|data| matches!(data, ResponseData::Discovered(_)))
        }
        Method::Export { .. } => Expected::Data(|data| matches!(data, ResponseData::Export(_))),
        Method::Backup { .. } => Expected::Data(|data| matches!(data, ResponseData::Backup(_))),
        Method::CloudSignOut | Method::CloudStatus | Method::CloudSetEndpoint { .. } => {
            Expected::Data(|data| matches!(data, ResponseData::CloudStatus(_)))
        }
        Method::GetConfig | Method::SetConfig { .. } => {
            Expected::Data(|data| matches!(data, ResponseData::Config(_)))
        }
        Method::GetPrivateMode | Method::SetPrivateMode { .. } => {
            Expected::Data(|data| matches!(data, ResponseData::PrivateMode(_)))
        }
        Method::Watch | Method::Shutdown => {
            Expected::Data(|data| matches!(data, ResponseData::Empty { .. }))
        }
        Method::Copy { .. }
        | Method::CopyPlainText { .. }
        | Method::Get { .. }
        | Method::ImagePreview { .. }
        | Method::Delete { .. }
        | Method::Pin { .. } => Expected::Error(ErrorCode::NotFound),
        Method::PairConfirm { .. } => Expected::Error(ErrorCode::NotReady),
        Method::PairJoin { .. } => Expected::Error(ErrorCode::PairingCode),
        Method::Unpair { .. } => Expected::Error(ErrorCode::PeerNotFound),
        Method::Import { .. }
        | Method::Restore { .. }
        | Method::CloudSignIn { .. }
        | Method::CloudSignUp { .. }
        | Method::CloudSyncNow => Expected::Error(ErrorCode::InvalidRequest),
    }
}

fn cases(root: &Path) -> Vec<Method> {
    vec![
        Method::Status,
        Method::SetDeviceName {
            name: "contract device".into(),
        },
        Method::List {
            limit: 10,
            cursor: None,
        },
        Method::Search {
            query: "needle".into(),
            limit: 10,
        },
        Method::Copy {
            id: "missing".into(),
        },
        Method::CopyPlainText {
            id: "missing".into(),
        },
        Method::Get {
            id: "missing".into(),
        },
        Method::ImagePreview {
            id: "missing".into(),
        },
        Method::Add {
            content: "contract item".into(),
        },
        Method::Delete {
            id: "missing".into(),
        },
        Method::DeleteAll { through: None },
        Method::HistoryCeiling,
        Method::Pin {
            id: "missing".into(),
            pinned: true,
        },
        Method::ReorderPinned { ids: Vec::new() },
        Method::PairCancel,
        Method::PairCreateInvite,
        Method::PairProgress,
        Method::PairConfirm { accept: true },
        Method::PairJoin {
            code: "malformed".into(),
            addr: "127.0.0.1:1".into(),
        },
        Method::Unpair {
            pairing_id: "missing".into(),
        },
        Method::Revoke {
            pairing_id: copypaste_p2p::transport::PairingToken::generate().pairing_id(),
        },
        Method::Peers,
        Method::SyncNow { pairing_id: None },
        Method::Discovered,
        Method::Rescan,
        Method::Export {
            limit: 0,
            include_sensitive: false,
        },
        Method::Import { items: Vec::new() },
        Method::Backup {
            dest_path: root.join("contract-backup.db").display().to_string(),
        },
        Method::Restore {
            src_path: root.join("missing.db").display().to_string(),
            confirm: false,
        },
        Method::CloudSignIn {
            email: "person@example.com".into(),
            password: "secret".into(),
            passphrase: "correct horse battery staple".into(),
        },
        Method::CloudSignUp {
            email: "person@example.com".into(),
            password: "secret".into(),
            passphrase: "correct horse battery staple".into(),
        },
        Method::CloudSignOut,
        Method::CloudStatus,
        Method::CloudSyncNow,
        Method::CloudSetEndpoint {
            url: "https://example.supabase.co".into(),
            anon_key: "anon".into(),
        },
        Method::GetConfig,
        Method::SetConfig {
            patch: copypaste_ipc::ConfigPatch::default(),
        },
        Method::SetPrivateMode { enabled: true },
        Method::GetPrivateMode,
        Method::Watch,
        Method::Shutdown,
    ]
}

fn request(id: u64, method: Method) -> Request {
    Request {
        id,
        protocol_version: PROTOCOL_VERSION,
        method,
    }
}

struct Client {
    writer: OwnedWriteHalf,
    lines: FramedRead<OwnedReadHalf, LinesCodec>,
}

impl Client {
    fn new(stream: Stream) -> Self {
        let (reader, writer) = stream.into_split();
        Self {
            writer,
            lines: FramedRead::new(reader, LinesCodec::new()),
        }
    }

    async fn call(&mut self, request: Request) -> Response {
        self.call_raw(&serde_json::to_string(&request).unwrap())
            .await
    }

    async fn call_raw(&mut self, line: &str) -> Response {
        self.writer.write_all(line.as_bytes()).await.unwrap();
        self.writer.write_all(b"\n").await.unwrap();
        self.writer.flush().await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), self.next())
            .await
            .expect("the local transport must answer")
            .expect("the local transport closed before answering")
    }

    async fn next(&mut self) -> Option<Response> {
        self.lines
            .next()
            .await
            .map(|line| serde_json::from_str(&line.expect("valid response frame")).unwrap())
    }
}

async fn start(
    name: &str,
) -> (
    Client,
    Arc<crate::AppState>,
    tempfile::TempDir,
    tokio::task::JoinHandle<()>,
) {
    let (state, dir) = test_state(name);
    let endpoint = dir.path().join("daemon.sock");
    let listener = bind(&endpoint).expect("bind the platform IPC endpoint");
    let server = tokio::spawn(run(listener, Arc::clone(&state), state.shutdown_rx()));
    let stream = transport::connect(&endpoint).await.expect("connect");
    (Client::new(stream), state, dir, server)
}

#[tokio::test]
async fn every_method_crosses_the_platform_transport_with_a_typed_outcome() {
    let (mut client, state, dir, server) = start("ipc-contract").await;
    let methods = cases(dir.path());
    assert_eq!(
        methods.len(),
        41,
        "a Method has no contract case, or this count was not bumped with it"
    );

    for (index, method) in methods.into_iter().enumerate() {
        let id = index as u64 + 1;
        let expected = expected(&method);
        let response = if matches!(method, Method::Shutdown) {
            let stream = transport::connect(&dir.path().join("daemon.sock"))
                .await
                .expect("shutdown connection");
            Client::new(stream).call(request(id, method)).await
        } else {
            client.call(request(id, method)).await
        };
        assert_eq!(response.id, id);
        if let Some(message) = &response.error {
            let root = dir.path().display().to_string();
            assert!(!message.contains(&root), "path leaked in {response:?}");
        }
        match expected {
            Expected::Data(predicate) => assert!(
                response.ok && response.data.as_ref().is_some_and(predicate),
                "unexpected success payload: {response:?}"
            ),
            Expected::Error(code) => {
                assert!(!response.ok, "expected {code:?}, got {response:?}");
                assert_eq!(response.error_code, Some(code));
            }
        }
    }

    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("shutdown must stop the server")
        .expect("server task must not panic");
    assert!(*state.shutdown_rx().borrow());
}

#[tokio::test]
async fn retired_pairing_methods_are_rejected_before_dispatch() {
    let (mut client, state, _dir, server) = start("ipc-retired-pairing").await;
    for (id, method, params) in [
        (500, "pair_create", json!({"name":"device"})),
        (
            501,
            "pair_accept",
            json!({"code":"code","addr":"127.0.0.1:1"}),
        ),
    ] {
        let response = client
            .call_raw(
                &json!({
                    "id": id,
                    "protocol_version": PROTOCOL_VERSION,
                    "method": method,
                    "params": params,
                })
                .to_string(),
            )
            .await;
        assert_eq!(response.id, id);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
    }

    state.request_shutdown();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server stops")
        .expect("server task must not panic");
}

#[tokio::test]
async fn every_parameterized_method_is_rejected_before_dispatch_when_malformed() {
    let (mut client, state, dir, server) = start("ipc-malformed").await;
    let mut id = 100;
    for method in cases(dir.path()) {
        let encoded = serde_json::to_value(&method).unwrap();
        let Some(name) = encoded.get("method").and_then(serde_json::Value::as_str) else {
            panic!("typed Method has no wire name: {method:?}");
        };
        if encoded.get("params").is_none() {
            continue;
        }
        let malformed = json!({
            "id": id,
            "protocol_version": PROTOCOL_VERSION,
            "method": name,
            "params": [],
        });
        let response = client.call_raw(&malformed.to_string()).await;
        assert_eq!(response.id, id, "{name} did not echo its id");
        assert_eq!(
            response.error_code,
            Some(ErrorCode::InvalidRequest),
            "{name}"
        );
        id += 1;
    }

    state.request_shutdown();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server stops")
        .expect("server task must not panic");
}

#[tokio::test]
async fn dropping_watchers_cancels_them_and_releases_the_separate_cap() {
    let (client, state, dir, server) = start("ipc-cancel").await;
    drop(client);

    for id in 200..210 {
        let stream = transport::connect(&dir.path().join("daemon.sock"))
            .await
            .expect("watch connection");
        let mut watcher = Client::new(stream);
        let response = watcher.call(request(id, Method::Watch)).await;
        assert!(response.ok, "watcher {id} was not admitted: {response:?}");
        drop(watcher);
        tokio::task::yield_now().await;
    }

    state.request_shutdown();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("server stops")
        .expect("server task must not panic");
}

#[tokio::test(start_paused = true)]
async fn an_idle_authenticated_connection_is_closed_at_the_shared_deadline() {
    let (mut client, state, _dir, server) = start("ipc-timeout").await;
    assert!(client.call(request(300, Method::Status)).await.ok);

    tokio::time::advance(super::listener::READ_TIMEOUT + Duration::from_secs(1)).await;
    assert!(
        tokio::time::timeout(Duration::from_secs(1), client.next())
            .await
            .expect("the read deadline must complete")
            .is_none(),
        "an idle connection remained open"
    );

    state.request_shutdown();
    tokio::time::timeout(Duration::from_secs(1), server)
        .await
        .expect("server stops")
        .expect("server task must not panic");
}
