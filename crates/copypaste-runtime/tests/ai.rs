//! The runtime AI tool service: what a session may see, how results are
//! bounded and redacted, and how grants, scopes and limits gate each call.

use std::sync::{Arc, Mutex};

use copypaste_core::sync::{apply_remote_version, open_version_bytes, RemoteVersion};
use copypaste_core::{ClipboardPayload, ClipboardWriteError, PayloadMetadata};
use copypaste_ipc::ai::{
    AiApprovalKind, AiLookback, AiOp, AiScope, AiSession, AiSessionState, AiToolCall, AiToolResult,
};
use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::payload::ProvenanceKind;
use copypaste_ipc::snippets::{SnippetDraft, SnippetOp};
use copypaste_ipc::{
    content_type, ClipboardPrivacy, ConfigPatch, EventKindFilter, LibraryKinds, Method,
    ResponseData, RuntimeEventData,
};
use copypaste_runtime::{ClipboardWriter, Runtime, WriteOptions};
use serde_json::Value;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Default)]
struct Clipboard(Mutex<Vec<String>>);

impl ClipboardWriter for Clipboard {
    fn write(
        &self,
        payload: &ClipboardPayload,
        _content_type: &str,
        _options: WriteOptions,
    ) -> Result<(), ClipboardWriteError> {
        let text = payload.plain_text().unwrap_or("<binary>").to_owned();
        self.0.lock().unwrap().push(text);
        Ok(())
    }
}

struct Device {
    runtime: Runtime,
    clipboard: Arc<Clipboard>,
    name: &'static str,
    _dir: tempfile::TempDir,
}

impl Device {
    fn new(name: &'static str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let clipboard = Arc::new(Clipboard::default());
        let runtime = Runtime::open_with_clipboard(dir.path(), name, 0, clipboard.clone()).unwrap();
        let device = Self {
            runtime,
            clipboard,
            name,
            _dir: dir,
        };
        device.data(Method::SetConfig {
            patch: ConfigPatch {
                mcp_enabled: Some(true),
                ..Default::default()
            },
        });
        device
    }

    fn data(&self, method: Method) -> ResponseData {
        let response = self.runtime.request_blocking(1, method);
        assert!(response.ok, "{:?}", response.error);
        response.data.unwrap()
    }

    fn ai(&self, op: AiOp) -> ResponseData {
        self.data(Method::Ai { op })
    }

    fn capture_at(&self, text: &str, at: i64, metadata: Option<&PayloadMetadata>) -> String {
        let runtime = &self.runtime;
        let item = copypaste_core::ingest_into_with_capture_source_metadata(
            &runtime.store,
            &runtime.keyring,
            text,
            content_type::TEXT,
            at,
            None,
            None,
            metadata,
            &runtime.settings.config(),
        )
        .unwrap()
        .into_item();
        runtime.note_capture(item.created_at, &item.id);
        item.id
    }

    fn capture(&self, text: &str) -> String {
        self.capture_at(text, copypaste_core::now_ms(), None)
    }

    /// A grant with `scopes` and a session authorized by its credential.
    fn session(&self, scopes: &[AiScope], lookback: AiLookback) -> (String, String) {
        let ResponseData::AiGrantCreated(created) = self.ai(AiOp::CreateGrant {
            name: "Agent".into(),
            client: "claude_code".into(),
            scopes: scopes.to_vec(),
            lookback,
        }) else {
            panic!("no grant");
        };
        let session = self.open(Some(&created.credential));
        assert_eq!(session.authorized_scopes.as_deref(), Some(scopes));
        (session.session_id, created.grant.id)
    }

    fn open(&self, credential: Option<&str>) -> AiSession {
        let ResponseData::AiSession(session) = self.ai(AiOp::SessionOpen {
            credential: credential.map(str::to_owned),
            client_name: "agent".into(),
            client_version: None,
            executable: None,
        }) else {
            panic!("no session");
        };
        session
    }

    fn tool(&self, session_id: &str, call: AiToolCall) -> AiToolResult {
        match self.ai(AiOp::Tool {
            session_id: session_id.into(),
            call,
        }) {
            ResponseData::AiToolResult(result) => result,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn ok(&self, session_id: &str, call: AiToolCall) -> (String, Value) {
        let result = self.tool(session_id, call);
        assert!(!result.is_error, "{}", result.text);
        let structured = serde_json::from_str(result.structured_json.as_deref().unwrap()).unwrap();
        (result.text, structured)
    }

    fn listed(&self, session_id: &str) -> Vec<String> {
        let (_, structured) = self.ok(session_id, list(None));
        ids(&structured["items"])
    }
}

fn list(kind: Option<&str>) -> AiToolCall {
    AiToolCall::ListRecent {
        limit: Some(50),
        kind: kind.map(str::to_owned),
        source_app: None,
        source_device: None,
        pinned_only: false,
        cursor: None,
    }
}

fn get(id: Option<&str>, offset: Option<u64>) -> AiToolCall {
    AiToolCall::GetItem {
        id: id.map(str::to_owned),
        offset,
    }
}

fn ids(items: &Value) -> Vec<String> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Every version `from` holds, applied to `to` as a cloud pull would.
fn sync(from: &Device, to: &Device) {
    let (source, target) = (&from.runtime, &to.runtime);
    for row in source.store.versions_since(0, 1000).unwrap().into_iter() {
        let bytes = if row.deleted {
            Default::default()
        } else {
            open_version_bytes(&source.keyring, &row).unwrap()
        };
        let binary = content_type::is_binary(&row.content_type);
        apply_remote_version(
            &target.store,
            &target.keyring,
            to.name,
            &RemoteVersion {
                item_id: &row.id,
                content: if binary {
                    ""
                } else {
                    std::str::from_utf8(&bytes).unwrap()
                },
                binary_content: (binary && !row.deleted).then_some(bytes.as_slice()),
                payload_metadata: None,
                content_type: &row.content_type,
                created_at: row.created_at,
                deleted: row.deleted,
                content_hash: Some(&row.content_hash),
                origin_device_id: if row.origin_device_id.is_empty() {
                    from.name
                } else {
                    &row.origin_device_id
                },
                app_bundle_id: None,
                app_name: None,
            },
        )
        .unwrap();
    }
}

fn hide(device: &Device, id: &str, hidden: bool) {
    device.data(Method::Clips {
        op: ClipOp::SetHiddenFromAi {
            ids: vec![id.into()],
            hidden,
        },
    });
}

#[test]
fn hidden_items_are_absent_everywhere_and_answer_like_missing_ones() {
    let phone = Device::new("phone");
    let desktop = Device::new("desktop");
    let secret = PayloadMetadata {
        privacy: ClipboardPrivacy {
            secret: true,
            transient: false,
        },
        ..Default::default()
    };
    let concealed =
        desktop.capture_at("vault entry alpha", copypaste_core::now_ms(), Some(&secret));
    let hidden_elsewhere = phone.capture("alpha copied on the phone");
    hide(&phone, &hidden_elsewhere, true);
    sync(&phone, &desktop);
    let visible = desktop.capture("alpha visible note");
    let (session, grant) = desktop.session(&[AiScope::ReadHistory], AiLookback::Week);

    assert_eq!(desktop.listed(&session), vec![visible.clone()]);
    let (_, found) = desktop.ok(
        &session,
        AiToolCall::SearchHistory {
            query: "alpha".into(),
            kind: None,
            source_app: None,
            source_device: None,
            since: None,
            until: None,
            pinned_only: false,
            limit: None,
            cursor: None,
        },
    );
    assert_eq!(ids(&found["items"]), vec![visible.clone()]);
    let missing = desktop.tool(&session, get(Some("no-such-id"), None));
    for hidden in [&concealed, &hidden_elsewhere] {
        let answer = desktop.tool(&session, get(Some(hidden), None));
        assert!(answer.is_error);
        assert_eq!(answer.text, missing.text);
        assert_eq!(answer.structured_json, missing.structured_json);
    }
    let (newest, structured) = desktop.ok(&session, get(None, None));
    assert_eq!(structured["id"], visible.as_str());
    assert!(newest.contains("alpha visible note"));

    let log = match desktop.ai(AiOp::AccessLog { grant_id: grant }) {
        ResponseData::AiAccessLog(log) => log,
        other => panic!("unexpected {other:?}"),
    };
    assert!(log.len() >= 5);
    let logged = serde_json::to_string(&log).unwrap();
    for content in ["alpha", "vault", "no-such-id"] {
        assert!(!logged.contains(content), "{content} leaked into {logged}");
    }
}

#[test]
fn secrets_inside_text_are_redacted_and_mostly_secret_items_are_filtered() {
    let desktop = Device::new("desktop");
    let card = desktop.capture(
        "Order notes for the quarterly supplies, billed to card 4111 1111 1111 1111 as agreed",
    );
    let code = desktop.capture("Your verification code is 482913");
    let (session, _) = desktop.session(&[AiScope::ReadHistory], AiLookback::Week);
    assert_eq!(desktop.listed(&session), vec![card.clone()]);
    let (text, _) = desktop.ok(&session, get(Some(&card), None));
    assert!(
        text.contains("billed to card [redacted: payment card] as agreed"),
        "{text}"
    );
    assert!(!text.contains("4111"));
    assert!(desktop.tool(&session, get(Some(&code), None)).is_error);
}

#[test]
fn lookback_hides_old_unpinned_items_until_they_are_pinned() {
    let desktop = Device::new("desktop");
    let old = desktop.capture_at(
        "eight days old",
        copypaste_core::now_ms() - 8 * DAY_MS,
        None,
    );
    let recent = desktop.capture("from today");
    let (session, _) = desktop.session(&[AiScope::ReadHistory], AiLookback::Week);
    assert_eq!(desktop.listed(&session), vec![recent.clone()]);
    desktop.data(Method::Pin {
        id: old.clone(),
        pinned: true,
    });
    let mut listed = desktop.listed(&session);
    listed.sort();
    let mut expected = vec![old, recent];
    expected.sort();
    assert_eq!(listed, expected);
}

#[test]
fn filters_are_strict_and_long_text_comes_in_bounded_chunks() {
    let desktop = Device::new("desktop");
    let (session, _) = desktop.session(&[AiScope::ReadHistory], AiLookback::All);
    let (empty, structured) = desktop.ok(&session, get(None, None));
    assert!(empty.contains("No clipboard items available"));
    assert!(structured["item"].is_null());

    for (call, value) in [
        (list(Some("spreadsheet")), "spreadsheet"),
        (
            AiToolCall::ListRecent {
                limit: None,
                kind: None,
                source_app: Some("Nowhere".into()),
                source_device: None,
                pinned_only: false,
                cursor: None,
            },
            "Nowhere",
        ),
        (
            AiToolCall::ListRecent {
                limit: None,
                kind: None,
                source_app: None,
                source_device: Some("laptop".into()),
                pinned_only: false,
                cursor: None,
            },
            "laptop",
        ),
    ] {
        let refused = desktop.tool(&session, call);
        assert!(
            refused.is_error && refused.text.contains(value),
            "{}",
            refused.text
        );
    }

    let long: String = "lorem ipsum dolor ".repeat(2_000);
    let id = desktop.capture(&long);
    let mut offset = 0;
    let mut chunks = 0;
    loop {
        let result = desktop.tool(&session, get(Some(&id), Some(offset)));
        assert!(!result.is_error, "{}", result.text);
        assert!(result.text.len() <= 64 * 1024);
        let structured: Value = serde_json::from_str(&result.structured_json.unwrap()).unwrap();
        assert_eq!(structured["total_length"], long.chars().count());
        assert!(structured["returned_length"].as_u64().unwrap() <= 16_000);
        chunks += 1;
        match structured["next_offset"].as_u64() {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(chunks, long.chars().count().div_ceil(16_000));
}

#[test]
fn other_devices_follow_the_policy_toggle() {
    let phone = Device::new("phone");
    let desktop = Device::new("desktop");
    let from_phone = phone.capture("captured on the phone");
    sync(&phone, &desktop);
    let local = desktop.capture("captured here");
    let (session, _) = desktop.session(&[AiScope::ReadHistory], AiLookback::Week);
    let (_, structured) = desktop.ok(&session, get(Some(&from_phone), None));
    assert_eq!(structured["id"], from_phone.as_str());
    desktop.ai(AiOp::SetPolicyMember {
        set: "excluded_devices".into(),
        member: "phone".into(),
        present: true,
    });
    assert_eq!(desktop.listed(&session), vec![local]);
    assert!(
        desktop
            .tool(&session, get(Some(&from_phone), None))
            .is_error
    );
}

#[test]
fn copy_to_clipboard_is_byte_exact_scoped_and_never_wakes_an_ai_only_watcher() {
    let desktop = Device::new("desktop");
    let mut events = desktop.runtime.subscribe_events();
    let (reader, _) = desktop.session(&[AiScope::ReadHistory], AiLookback::Week);
    let text = "Caf\u{e9} %20 \u{1F600} line\nend";
    let refused = desktop.tool(
        &reader,
        AiToolCall::CopyToClipboard {
            id: None,
            text: Some(text.into()),
        },
    );
    assert!(refused.is_error);
    let (writer, _) = desktop.session(&[AiScope::WriteClipboard], AiLookback::Week);
    let (_, structured) = desktop.ok(
        &writer,
        AiToolCall::CopyToClipboard {
            id: None,
            text: Some(text.into()),
        },
    );
    assert_eq!(desktop.clipboard.0.lock().unwrap().last().unwrap(), text);
    let item_id = structured["item_id"].as_str().unwrap().to_owned();
    let ResponseData::Item(item) = desktop.data(Method::Get {
        id: item_id.clone(),
    }) else {
        panic!("no item");
    };
    assert_eq!(item.content, text);
    let provenance = item.annotations.unwrap().provenance.unwrap();
    assert_eq!(provenance.kind, ProvenanceKind::AiTool);
    assert_eq!(provenance.grant_name.as_deref(), Some("Agent"));

    let ai_only = [
        EventKindFilter::AiApproval,
        EventKindFilter::Library {
            kinds: LibraryKinds::GRANTS,
        },
        EventKindFilter::Status,
    ];
    let mut items = 0;
    while let Ok(change) = events.try_recv() {
        let frame = RuntimeEventData {
            file_operation: None,
            change: Some(change.clone()),
            progress: None,
            capture: None,
        };
        if change.event == copypaste_ipc::EventKind::Items {
            items += 1;
            assert!(!EventKindFilter::admits(&ai_only, &frame));
        }
    }
    assert!(items > 0, "the copy was announced to history watchers");
}

#[test]
fn snippets_render_with_missing_variables_named_and_saves_never_overwrite() {
    let desktop = Device::new("desktop");
    let ResponseData::SnippetSave(_) = desktop.data(Method::Snippets {
        op: SnippetOp::Save {
            draft: SnippetDraft {
                id: None,
                collection_id: None,
                title: "Greeting".into(),
                abbreviation: Some("hi".into()),
                body_text: "Hello {{field name=who}}!".into(),
                body_rich: None,
                template: true,
                pinned: false,
                concealed: false,
                base_revision: None,
            },
        },
    }) else {
        panic!("not saved");
    };
    let (session, _) = desktop.session(
        &[AiScope::ReadSnippets, AiScope::Organize],
        AiLookback::Week,
    );
    let (_, found) = desktop.ok(
        &session,
        AiToolCall::SearchSnippets {
            query: "hi".into(),
            limit: None,
        },
    );
    let snippet = found["snippets"][0]["id"].as_str().unwrap().to_owned();
    let render = |values: &[(&str, &str)]| {
        desktop.tool(
            &session,
            AiToolCall::RenderSnippet {
                id: snippet.clone(),
                values: values
                    .iter()
                    .map(|(key, value)| ((*key).into(), (*value).into()))
                    .collect(),
            },
        )
    };
    let missing = render(&[]);
    assert!(
        missing.is_error && missing.text.contains("who"),
        "{}",
        missing.text
    );
    let rendered = render(&[("who", "Ada")]);
    assert!(!rendered.is_error, "{}", rendered.text);
    assert!(rendered.text.contains("Hello Ada!"), "{}", rendered.text);

    let save = |abbreviation: &str| {
        desktop.tool(
            &session,
            AiToolCall::SaveSnippet {
                title: "Another".into(),
                body: "text".into(),
                abbreviation: Some(abbreviation.into()),
            },
        )
    };
    let conflict = save("hi");
    assert!(
        conflict.is_error && conflict.text.contains("Greeting"),
        "{}",
        conflict.text
    );
    assert!(!save("bye").is_error);
}

#[test]
fn rate_and_volume_limits_pause_a_session_without_ending_it() {
    let desktop = Device::new("desktop");
    desktop.capture(&"x".repeat(15_000));
    let (session, _) = desktop.session(&[AiScope::ReadHistory], AiLookback::Week);
    let mut limited = None;
    for _ in 0..31 {
        let result = desktop.tool(&session, list(None));
        if result.retry_after_secs.is_some() {
            limited = Some(result);
            break;
        }
    }
    let limited = limited.expect("the 31st call is limited");
    assert!(limited.is_error && limited.retry_after_secs.unwrap() >= 1);

    let volume = Device::new("volume");
    let big = volume.capture(&"\u{1F600}".repeat(16_000));
    let (session, _) = volume.session(&[AiScope::ReadHistory], AiLookback::Week);
    let mut pending = None;
    for _ in 0..45 {
        let result = volume.tool(&session, get(Some(&big), None));
        if let Some(seconds) = result.retry_after_secs {
            std::thread::sleep(std::time::Duration::from_secs(u64::from(seconds)));
            continue;
        }
        if result.is_error {
            pending = Some(result);
            break;
        }
    }
    let pending = pending.expect("the volume guard asks for confirmation");
    let structured: Value = serde_json::from_str(&pending.structured_json.unwrap()).unwrap();
    let request_id = structured["pending_request_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let ResponseData::AiApprovals(requests) = volume.ai(AiOp::PendingApprovals) else {
        panic!("no approvals");
    };
    assert_eq!(requests[0].kind, AiApprovalKind::Volume);
    volume.ai(AiOp::VolumeDecision {
        request_id,
        allow: false,
    });
    let declined = volume.tool(&session, get(Some(&big), None));
    assert!(
        declined.is_error && declined.text.contains("declined"),
        "{}",
        declined.text
    );
}

#[test]
fn revocation_ends_access_on_the_next_call_and_a_hide_beats_a_concurrent_unhide() {
    let desktop = Device::new("desktop");
    let laptop = Device::new("laptop");
    let id = desktop.capture("shared clip");
    let (session, grant) = desktop.session(&[AiScope::ReadHistory], AiLookback::Week);
    assert!(!desktop.tool(&session, list(None)).is_error);
    desktop.ai(AiOp::RevokeGrant { id: grant });
    let revoked = desktop.tool(&session, list(None));
    assert!(revoked.is_error);
    let ResponseData::AiStatus(status) = desktop.ai(AiOp::Status {
        session_id: Some(session),
    }) else {
        panic!("no status");
    };
    assert_eq!(status.session, Some(AiSessionState::Revoked));

    hide(&desktop, &id, true);
    sync(&desktop, &laptop);
    hide(&desktop, &id, false);
    hide(&desktop, &id, true);
    std::thread::sleep(std::time::Duration::from_millis(5));
    hide(&laptop, &id, false);
    sync(&laptop, &desktop);
    sync(&desktop, &laptop);
    for device in [&desktop, &laptop] {
        let ResponseData::ClipAnnotations(annotations) = device.data(Method::Clips {
            op: ClipOp::Annotations {
                ids: vec![id.clone()],
            },
        }) else {
            panic!("no annotations");
        };
        assert!(annotations[0].annotations.hidden_from_ai, "{}", device.name);
    }
}
