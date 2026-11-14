//! The archive's AI tool tests, ported to the module boundary: everything
//! the module does is observed through an in-memory host.
mod fake;
mod native;

use crate::calls::{ToolCall, ToolResult};
use crate::grants::{self, Scope};
use crate::service::Service;
use crate::strings::{text, Text};
use crate::tools::UNTRUSTED;
use crate::{run, AiTools};
use copypaste_module_sdk::{
    ClipboardSource, Module, ModuleDelivery, ModuleInvocation, ModuleManifest, ModuleTarget,
    Snippet, ViewItem,
};
use fake::{Fake, LOCAL};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const WAIT: Duration = Duration::from_secs(10);

struct Harness {
    fake: Arc<Fake>,
    service: Arc<Service>,
    notified: Arc<AtomicUsize>,
    connections: AtomicUsize,
    _runtime: tokio::runtime::Runtime,
}

impl Harness {
    fn new() -> Self {
        let fake = Arc::new(Fake::default());
        fake.state().locale = "en".into();
        let service = Arc::new(Service::new(fake.clone()));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        service.set_runtime(Some(runtime.handle().clone()));
        Self {
            fake,
            service,
            notified: Arc::default(),
            connections: AtomicUsize::new(0),
            _runtime: runtime,
        }
    }

    /// A connection as the endpoint opens it: identified, then settled.
    fn connect(&self, client: &str) -> String {
        let connection = format!("c{}", self.connections.fetch_add(1, Ordering::SeqCst));
        self.service.open(&connection);
        let notified = Arc::clone(&self.notified);
        self.service.identify(
            &connection,
            client,
            Arc::new(move || {
                notified.fetch_add(1, Ordering::SeqCst);
            }),
        );
        self.service.settle(&connection);
        connection
    }

    /// A connection approved with "Always allow", with changes allowed when
    /// `write` is set.
    fn authorized(&self, client: &str, write: bool) -> String {
        self.fake.answer(Some("always"));
        let connection = self.connect(client);
        assert_eq!(self.session(&connection), "authorized");
        if write {
            self.service.set_access(&self.grant(client), true).unwrap();
        }
        connection
    }

    fn grant(&self, client: &str) -> String {
        grants::list(&*self.fake)
            .unwrap()
            .into_iter()
            .find(|grant| grant.client_name == client)
            .unwrap()
            .id
    }

    fn session(&self, connection: &str) -> String {
        self.service.status(connection).structured["status"]["session"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn call(&self, connection: &str, call: ToolCall) -> ToolResult {
        self.service.call(connection, call)
    }

    fn ok(&self, connection: &str, call: ToolCall) -> (String, Value) {
        let result = self.call(connection, call);
        assert!(!result.is_error, "{}", result.text);
        (result.text, result.structured)
    }

    fn listed(&self, connection: &str) -> Vec<String> {
        ids(&self.ok(connection, list(None)).1["items"])
    }
}

fn list(kind: Option<&str>) -> ToolCall {
    ToolCall::ListRecent {
        limit: Some(50),
        kind: kind.map(str::to_owned),
        source_app: None,
        source_device: None,
        pinned_only: false,
        cursor: None,
    }
}

fn get(id: Option<&str>, offset: Option<u64>) -> ToolCall {
    ToolCall::GetItem {
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

fn wait_until(mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(started.elapsed() < WAIT, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn approvals_remember_a_client_and_repeated_denials_stop_prompting() {
    let h = Harness::new();
    let first = h.authorized("Claude", false);
    let grant = grants::get(&*h.fake, &h.grant("Claude")).unwrap().unwrap();
    assert!(grant.remember && !grant.revoked());
    assert_eq!(grant.scopes(), [Scope::ReadHistory, Scope::ReadSnippets]);
    assert_eq!(grant.owner_device, LOCAL);
    assert_eq!(h.service.scopes(&first), grant.scopes());
    assert!(h.notified.load(Ordering::SeqCst) > 0, "the client was told");

    let again = h.connect("Claude");
    assert_eq!(h.session(&again), "authorized");
    assert_eq!(
        h.fake.state().prompts.len(),
        1,
        "a remembered client is not asked"
    );

    h.fake.answer(Some("once"));
    assert_eq!(h.session(&h.connect("Cursor")), "authorized");
    let unanswered = h.connect("Cursor");
    assert_eq!(h.fake.state().prompts.len(), 3, "allow once asks again");
    assert_eq!(h.session(&unanswered), "unauthorized");

    for _ in 0..3 {
        h.connect("Looping client");
    }
    let asked = h.fake.state().prompts.len();
    let throttled = h.connect("Looping client");
    assert_eq!(h.fake.state().prompts.len(), asked, "no fourth prompt");
    let status = h.service.status(&throttled).structured;
    assert_eq!(status["status"]["next_step"], "access_denied");
    let blocked = h.call(&throttled, list(None));
    assert!(
        blocked.is_error && blocked.text.contains("denied"),
        "{}",
        blocked.text
    );
    assert!(
        h.fake.state().prompts[0].title.contains("Claude"),
        "the prompt names the reported client"
    );
}

#[test]
fn hidden_items_answer_like_missing_ones_and_the_log_holds_no_content() {
    let h = Harness::new();
    let hidden = h.fake.capture("alpha vault entry", LOCAL, 0);
    h.fake.state().items[0].hidden = true;
    let visible = h.fake.capture("alpha visible note", LOCAL, 0);
    let connection = h.authorized("Claude", false);

    assert_eq!(h.listed(&connection), vec![visible.clone()]);
    let (_, found) = h.ok(
        &connection,
        ToolCall::SearchHistory {
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
    let missing = h.call(&connection, get(Some("no-such-id"), None));
    let answer = h.call(&connection, get(Some(&hidden), None));
    assert!(answer.is_error);
    assert_eq!(answer.text, missing.text);
    assert_eq!(answer.structured, missing.structured);
    let (newest, structured) = h.ok(&connection, get(None, None));
    assert_eq!(structured["id"], visible.as_str());
    assert!(newest.starts_with(UNTRUSTED) && newest.contains("alpha visible note"));

    h.service.flush_log();
    let log = h.fake.state().local["access_log"].clone();
    assert!(log.contains("list_recent") && log.contains(&visible));
    for content in ["alpha", "vault", "no-such-id"] {
        assert!(!log.contains(content), "{content} leaked into {log}");
    }
}

#[test]
fn filters_are_strict_and_long_text_comes_in_bounded_chunks() {
    let h = Harness::new();
    h.service
        .remember_preferences(&BTreeMap::from([("lookback".into(), json!(["all"]))]));
    let connection = h.authorized("Claude", false);
    let (empty, structured) = h.ok(&connection, get(None, None));
    assert!(empty.contains("No clipboard items available"));
    assert!(structured["item"].is_null());

    let filtered = |source_app: Option<&str>, source_device: Option<&str>| ToolCall::ListRecent {
        limit: None,
        kind: None,
        source_app: source_app.map(str::to_owned),
        source_device: source_device.map(str::to_owned),
        pinned_only: false,
        cursor: None,
    };
    for (call, value) in [
        (list(Some("spreadsheet")), "spreadsheet"),
        (filtered(Some("Nowhere"), None), "Nowhere"),
        (filtered(None, Some("laptop")), "laptop"),
    ] {
        let refused = h.call(&connection, call);
        assert!(
            refused.is_error && refused.text.contains(value),
            "{}",
            refused.text
        );
    }

    let long: String = "lorem ipsum dolor ".repeat(2_000);
    let id = h.fake.capture(&long, LOCAL, 0);
    let (_, by_app) = h.ok(&connection, filtered(Some("editor"), Some("desktop")));
    assert_eq!(
        ids(&by_app["items"]),
        vec![id.clone()],
        "names match case-insensitively"
    );
    assert!(
        by_app["items"][0]["preview"]
            .as_str()
            .unwrap()
            .chars()
            .count()
            <= 200
    );
    let mut offset = 0;
    let mut chunks = 0;
    loop {
        let result = h.call(&connection, get(Some(&id), Some(offset)));
        assert!(!result.is_error, "{}", result.text);
        assert!(result.text.len() <= 64 * 1024);
        assert_eq!(result.structured["total_length"], long.chars().count());
        assert!(result.structured["returned_length"].as_u64().unwrap() <= 16_000);
        chunks += 1;
        match result.structured["next_offset"].as_u64() {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(chunks, long.chars().count().div_ceil(16_000));
    let beyond = h.call(&connection, get(Some(&id), Some(long.len() as u64 + 1)));
    assert!(
        beyond.is_error && beyond.text.contains("offset"),
        "{}",
        beyond.text
    );
}

#[test]
fn the_synced_policy_reaches_the_host_as_the_history_scope() {
    let h = Harness::new();
    let old = h.fake.capture("eight days old", LOCAL, 8 * DAY_MS);
    let from_phone = h.fake.capture("captured on the phone", "phone", 1_000);
    let recent = h.fake.capture("captured here", LOCAL, 0);
    let connection = h.authorized("Claude", false);
    assert_eq!(
        h.listed(&connection),
        vec![recent.clone(), from_phone.clone()]
    );

    h.fake.state().items[0].entry.pinned = true;
    assert_eq!(
        h.listed(&connection),
        vec![recent.clone(), from_phone.clone(), old.clone()]
    );

    h.service.remember_preferences(&BTreeMap::from([(
        "excluded-devices".into(),
        json!(["phone"]),
    )]));
    assert_eq!(h.listed(&connection), vec![recent.clone(), old]);
    assert!(h.call(&connection, get(Some(&from_phone), None)).is_error);

    h.service
        .remember_preferences(&BTreeMap::from([("content-kinds".into(), json!(["link"]))]));
    assert!(h.listed(&connection).is_empty(), "text is no longer shared");
    let state = h.fake.state();
    assert!(
        state.local["preferences"].contains("link"),
        "kept for later calls"
    );
}

#[test]
fn copy_to_clipboard_is_byte_exact_scoped_and_labeled() {
    let h = Harness::new();
    let item = h.fake.capture("history clip", LOCAL, 0);
    let reader = h.authorized("Reader", false);
    let text = "Caf\u{e9} %20 \u{1F600} line\nend";
    let copy = |id: Option<&str>, text: Option<&str>| ToolCall::CopyToClipboard {
        id: id.map(str::to_owned),
        text: text.map(str::to_owned),
    };
    let refused = h.call(&reader, copy(None, Some(text)));
    assert!(refused.is_error && refused.text.contains("write_clipboard"));

    let writer = h.authorized("Writer", true);
    h.ok(&writer, copy(None, Some(text)));
    h.ok(&writer, copy(Some(&item), None));
    assert!(h.call(&writer, copy(Some(&item), Some(text))).is_error);
    assert!(h.call(&writer, copy(None, Some(""))).is_error);
    let writes = h.fake.state().writes.clone();
    assert_eq!(
        writes[0],
        (ClipboardSource::Text { text: text.into() }, "Writer".into())
    );
    assert!(matches!(&writes[1].0, ClipboardSource::Item { id, .. } if *id == item));
}

#[test]
fn snippets_render_with_variables_checked_and_saves_never_overwrite() {
    let h = Harness::new();
    h.fake.state().snippets.push(Snippet {
        id: "greeting".into(),
        title: "Greeting".into(),
        body: "Hello {{who}}!".into(),
        abbreviation: Some("hi".into()),
        placeholders: vec!["who".into()],
    });
    let connection = h.authorized("Claude", true);
    let (_, found) = h.ok(
        &connection,
        ToolCall::SearchSnippets {
            query: "hi".into(),
            limit: None,
        },
    );
    assert_eq!(found["snippets"][0]["id"], "greeting");
    let render = |values: &[(&str, &str)]| {
        h.call(
            &connection,
            ToolCall::RenderSnippet {
                id: "greeting".into(),
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
    let unknown = render(&[("who", "Ada"), ("bogus", "x")]);
    assert!(
        unknown.is_error && unknown.text.contains("bogus"),
        "{}",
        unknown.text
    );
    let rendered = render(&[("who", "Ada")]);
    assert!(rendered.text.contains("Hello Ada!"), "{}", rendered.text);

    let save = |abbreviation: &str| {
        h.call(
            &connection,
            ToolCall::SaveSnippet {
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
    let absent = h.call(&connection, ToolCall::GetSnippet { id: "nope".into() });
    assert!(absent.text.contains("No snippet"), "{}", absent.text);
}

#[test]
fn rate_and_volume_limits_pause_a_session_without_ending_it() {
    let h = Harness::new();
    h.fake.capture(&"x".repeat(15_000), LOCAL, 0);
    let connection = h.authorized("Claude", false);
    let limited = (0..31)
        .map(|_| h.call(&connection, list(None)))
        .find(|result| result.retry_after_secs.is_some())
        .expect("the 31st call is limited");
    assert!(limited.is_error && limited.retry_after_secs.unwrap() >= 1);

    let h = Harness::new();
    let big = h.fake.capture(&"\u{1F600}".repeat(16_000), LOCAL, 0);
    let connection = h.authorized("Claude", false);
    let mut pending = None;
    for _ in 0..45 {
        let result = h.call(&connection, get(Some(&big), None));
        if let Some(seconds) = result.retry_after_secs {
            std::thread::sleep(Duration::from_secs(u64::from(seconds)));
            continue;
        }
        if result.is_error {
            pending = Some(result);
            break;
        }
    }
    let pending = pending.expect("the volume guard asks for confirmation");
    assert_eq!(pending.structured["pending"], "volume");
    wait_until(|| h.fake.state().prompts.len() == 2);
    assert_eq!(
        h.fake.state().prompts[1].title,
        text("en", Text::VolumeTitle)
    );
    let declined = || h.call(&connection, get(Some(&big), None));
    wait_until(|| declined().text.contains("declined"));
    assert!(!h
        .call(&connection, ToolCall::GetSnippet { id: "x".into() })
        .text
        .contains("ended"));
}

#[test]
fn revocation_from_any_device_ends_access_on_the_next_call() {
    let h = Harness::new();
    h.fake.capture("shared clip", LOCAL, 0);
    let connection = h.authorized("Claude", false);
    assert!(!h.call(&connection, list(None)).is_error);
    let grant = h.grant("Claude");
    let stale = h.fake.state().records[&format!("grant/{grant}")].clone();
    let before = h.notified.load(Ordering::SeqCst);

    h.service.revoke(&grant).unwrap();
    assert!(h.call(&connection, list(None)).is_error);
    assert_eq!(h.session(&connection), "revoked");
    assert!(h.service.scopes(&connection).is_empty());
    assert!(
        h.notified.load(Ordering::SeqCst) > before,
        "the tool list shrank"
    );

    // A copy of the grant written before the revocation arrives late.
    h.fake
        .state()
        .records
        .insert(format!("grant/{grant}"), stale);
    assert!(h.call(&connection, list(None)).is_error);
    assert_eq!(
        h.session(&h.connect("Claude")),
        "unauthorized",
        "asked again"
    );
}

#[test]
fn commands_render_connections_activity_and_changes_as_views() {
    let h = Harness::new();
    let connection = h.authorized("Claude", false);
    h.call(&connection, list(None));
    let grant = h.grant("Claude");
    let invoke = |command: &str, arguments: Value| {
        run(
            Some(&h.service),
            &ModuleInvocation {
                command: command.into(),
                arguments: serde_json::from_value(arguments).unwrap(),
                preferences: BTreeMap::new(),
            },
        )
    };
    let manifest: Value = serde_json::from_str(include_str!("../../module.json")).unwrap();
    let commands: Vec<&str> = manifest["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["id"].as_str().unwrap())
        .collect();
    let view = invoke("connections", json!({})).unwrap();
    let ViewItem::Row { title, actions, .. } = &view.sections[0].items[0] else {
        panic!("a row");
    };
    assert_eq!(title, "Claude");
    assert!(actions
        .iter()
        .all(|action| commands.contains(&action.command.as_str())));
    assert!(actions
        .iter()
        .any(|action| action.command == "revoke" && action.destructive));

    invoke("set-access", json!({ "grant": grant, "write": true })).unwrap();
    assert!(h.service.scopes(&connection).contains(&Scope::Organize));
    let activity = invoke("activity", json!({ "grant": grant })).unwrap();
    assert!(
        matches!(&activity.sections[0].items[0], ViewItem::Row { title, .. } if title.contains("list_recent"))
    );
    let revoked = invoke("revoke", json!({ "grant": grant })).unwrap();
    assert!(
        matches!(&revoked.sections[0].items[0], ViewItem::Row { subtitle: Some(subtitle), .. } if subtitle.contains("Revoked"))
    );
    let removed = invoke("remove", json!({ "grant": grant })).unwrap();
    assert!(
        matches!(&removed.sections[0].items[0], ViewItem::Row { actions, .. } if actions.is_empty())
    );
    assert!(invoke("revoke", json!({ "grant": "../x" })).is_err());

    let setup = ModuleInvocation {
        command: "setup".into(),
        arguments: BTreeMap::new(),
        preferences: BTreeMap::new(),
    };
    assert!(run(None, &setup).is_ok(), "instructions need no host");
    let connections = ModuleInvocation {
        command: "connections".into(),
        ..setup
    };
    assert!(run(None, &connections).is_err());
}

#[test]
fn the_mcp_endpoint_serves_tools_after_approval_and_stops_with_its_last_connection() {
    let fake = Arc::new(Fake::default());
    fake.state().locale = "en".into();
    fake.capture("hello from history", LOCAL, 0);
    fake.answer(Some("always"));
    let mut module = AiTools::with_host(fake.clone());
    let frame = |module: &mut AiTools, message: Value| {
        module
            .deliver(ModuleDelivery::EndpointFrame {
                connection: "c1".into(),
                data: message.to_string(),
            })
            .unwrap();
    };
    let reply = |id: u64| {
        let mut found = None;
        wait_until(|| {
            found = fake.reply("c1", id);
            found.is_some()
        });
        found.unwrap()
    };
    module
        .deliver(ModuleDelivery::EndpointOpen {
            endpoint: "mcp".into(),
            connection: "c1".into(),
            privileged: false,
        })
        .unwrap();
    assert!(module
        .deliver(ModuleDelivery::EndpointOpen {
            endpoint: "other".into(),
            connection: "c2".into(),
            privileged: false,
        })
        .is_err());

    frame(
        &mut module,
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/list", "params": {} }),
    );
    assert!(
        reply(7)["error"].is_object(),
        "a probe before initialize is answered"
    );
    frame(
        &mut module,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0" },
        }}),
    );
    assert_eq!(reply(1)["result"]["serverInfo"]["name"], "copypaste");
    frame(
        &mut module,
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    );
    wait_until(|| fake.notifications("c1", "notifications/tools/list_changed") > 0);

    frame(
        &mut module,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
    );
    let names: Vec<String> = reply(2)["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        names.contains(&"list_recent".to_owned())
            && !names.contains(&"copy_to_clipboard".to_owned())
    );
    frame(
        &mut module,
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
            "name": "list_recent", "arguments": { "limit": 5 },
        }}),
    );
    let listed = reply(3);
    assert_eq!(listed["result"]["isError"], false);
    assert!(listed["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("hello from history"));
    frame(
        &mut module,
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
            "name": "list_recent", "arguments": { "tag": "x" },
        }}),
    );
    assert_eq!(reply(4)["result"]["isError"], true);

    module
        .deliver(ModuleDelivery::EndpointClose {
            connection: "c1".into(),
        })
        .unwrap();
    assert!(module
        .deliver(ModuleDelivery::EndpointFrame {
            connection: "c1".into(),
            data: "{}".into(),
        })
        .is_err());
    drop(module);
    assert!(
        fake.state().local.contains_key("access_log"),
        "the log is flushed on close"
    );
}

#[test]
fn the_manifest_validates_with_every_app_locale() {
    let mut value: Value = serde_json::from_str(include_str!("../../module.json")).unwrap();
    let target = ModuleTarget::current().unwrap();
    let entrypoint = format!("bin/module.{}", target.platform.dynamic_library_extension());
    value["target"] = serde_json::to_value(target).unwrap();
    value["entrypoint"] = json!(entrypoint);
    value["files"] = json!([{ "path": entrypoint, "sha256": "0".repeat(64), "size_bytes": 1 }]);
    let manifest: ModuleManifest = serde_json::from_value(value).unwrap();
    manifest.validate("1.3.0", target).unwrap();
    let locales: Vec<&String> = manifest.localizations.keys().collect();
    assert_eq!(
        locales,
        ["de", "es", "fr", "it", "ja", "ko", "pl", "pt", "tr", "uk", "zh"]
    );
    let keys = |locale: &str| manifest.localizations[locale].keys().collect::<Vec<_>>();
    assert!(locales.iter().all(|locale| keys(locale) == keys("uk")));
    assert_eq!(manifest.localized("uk").title, "Інструменти ШІ");
    assert!(manifest.preferences.iter().all(|field| field.synced));
    assert!(manifest
        .commands
        .iter()
        .filter(|command| ["set-access", "revoke", "remove"].contains(&command.id.as_str()))
        .all(|command| command.app_only && command.hidden));
}
