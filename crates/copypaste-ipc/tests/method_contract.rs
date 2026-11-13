use std::collections::BTreeSet;

use copypaste_ipc::Method;
use serde_json::{json, Value};

fn wire_name(method: &Method) -> &'static str {
    match method {
        Method::Modules { .. } => "modules",
        Method::Status => "status",
        Method::FileOperations { .. } => "file_operations",
        Method::ImportFileReference { .. } => "import_file_reference",
        Method::DownloadedFile { .. } => "downloaded_file",
        Method::StartFileDownload { .. } => "start_file_download",
        Method::SetDeviceName { .. } => "set_device_name",
        Method::List { .. } => "list",
        Method::Search { .. } => "search",
        Method::HistoryQuery { .. } => "history_query",
        Method::HistoryFacets => "history_facets",
        Method::HistoryCounts { .. } => "history_counts",
        Method::HistoryTags { .. } => "history_tags",
        Method::MutateHistoryTags { .. } => "mutate_history_tags",
        Method::Copy { .. } => "copy",
        Method::CopyPlainText { .. } => "copy_plain_text",
        Method::CopyText { .. } => "copy_text",
        Method::Get { .. } => "get",
        Method::ImagePreview { .. } => "image_preview",
        Method::SourceAppIcon { .. } => "source_app_icon",
        Method::SaveFile { .. } => "save_file",
        Method::CancelFileDownload { .. } => "cancel_file_download",
        Method::Add { .. } => "add",
        Method::ImportFile { .. } => "import_file",
        Method::Delete { .. } => "delete",
        Method::DeleteAll { .. } => "delete_all",
        Method::HistoryCeiling => "history_ceiling",
        Method::Pin { .. } => "pin",
        Method::SetSecret { .. } => "set_secret",
        Method::GetSpoilerState => "get_spoiler_state",
        Method::SetSpoilerRevealed { .. } => "set_spoiler_revealed",
        Method::ReorderPinned { .. } => "reorder_pinned",
        Method::PairCreateInvite => "pair_create_invite",
        Method::PairJoin { .. } => "pair_join",
        Method::PairNearby { .. } => "pair_nearby",
        Method::PairProgress => "pair_progress",
        Method::PairConfirm { .. } => "pair_confirm",
        Method::PairCancel => "pair_cancel",
        Method::Unpair { .. } => "unpair",
        Method::Revoke { .. } => "revoke",
        Method::Peers => "peers",
        Method::SyncNow { .. } => "sync_now",
        Method::Discovered => "discovered",
        Method::Rescan => "rescan",
        Method::Export { .. } => "export",
        Method::ExportStream => "export_stream",
        Method::Import { .. } => "import",
        Method::Backup { .. } => "backup",
        Method::BackupStream { .. } => "backup_stream",
        Method::Restore { .. } => "restore",
        Method::CloudSignIn { .. } => "cloud_sign_in",
        Method::CloudSignUp { .. } => "cloud_sign_up",
        Method::CloudSetEndpoint { .. } => "cloud_set_endpoint",
        Method::CloudSignOut => "cloud_sign_out",
        Method::CloudStatus => "cloud_status",
        Method::CloudSyncNow => "cloud_sync_now",
        Method::GetConfig => "get_config",
        Method::SetConfig { .. } => "set_config",
        Method::SetPrivateMode { .. } => "set_private_mode",
        Method::GetPrivateMode => "get_private_mode",
        Method::Watch => "watch",
        Method::WatchRuntime => "watch_runtime",
        Method::WatchEvents { .. } => "watch_events",
        Method::Snippets { .. } => "snippets",
        Method::Clips { .. } => "clips",
        Method::PasteQueue { .. } => "paste_queue",
        Method::Transforms { .. } => "transforms",
        Method::Rules { .. } => "rules",
        Method::Ai { .. } => "ai",
        Method::Shutdown => "shutdown",
    }
}

#[test]
fn module_update_staging_preserves_its_destination_and_install_budget() {
    let value = json!({
        "method": "modules",
        "params": {"operation": {
            "operation": "stage_update",
            "id": "copypaste.ocr",
            "package_path": "/private/update.cpmodule",
            "app_version": "1.1.0"
        }}
    });
    let method: Method = serde_json::from_value(value.clone()).unwrap();
    assert!(method.is_long_running());
    assert_eq!(serde_json::to_value(method).unwrap(), value);
}

fn catalog() -> Vec<Value> {
    vec![
        json!({"method":"modules","params":{"operation":{"operation":"list"}}}),
        json!({"method":"status"}),
        json!({"method":"file_operations","params":{"clear":false,"before":null,"id":null}}),
        json!({"method":"import_file_reference","params":{"reference":"/source/file.bin","filename":"file.bin","mime_type":"application/octet-stream","size_hint":null}}),
        json!({"method":"downloaded_file","params":{"id":"item"}}),
        json!({"method":"start_file_download","params":{"id":"item","dest_path":"/save/file.bin","token":"operation"}}),
        json!({"method":"set_device_name","params":{"name":"Kitchen Mac"}}),
        json!({"method":"list","params":{"limit":10,"cursor":null}}),
        json!({"method":"search","params":{"query":"needle","limit":10}}),
        json!({"method":"history_query","params":{"query":{"search":"needle","content_classes":["text","image"],"semantic_kinds":["link","color"],"pinned_only":true,"origin_device_id":"device-1","source_app_bundle_id":"com.example.app","sort":"relevance","tag_ids":[],"untagged_only":false},"limit":10,"cursor":null}}),
        json!({"method":"history_facets"}),
        json!({"method":"history_counts","params":{"query":{"content_classes":[],"semantic_kinds":[],"pinned_only":false,"origin_device_id":"device-1","tag_ids":[],"untagged_only":false,"sort":"newest"},"ranges":[{"from_ms":1,"before_ms":2}]}}),
        json!({"method":"history_tags","params":{"clip_ids":[]}}),
        json!({"method":"mutate_history_tags","params":{"mutation":{"action":"save","id":null,"name":"Work"}}}),
        json!({"method":"copy","params":{"id":"item"}}),
        json!({"method":"copy_plain_text","params":{"id":"item"}}),
        json!({"method":"copy_text","params":{"id":"item","text":"selected text"}}),
        json!({"method":"get","params":{"id":"item"}}),
        json!({"method":"image_preview","params":{"id":"item"}}),
        json!({"method":"source_app_icon","params":{"id":"item"}}),
        json!({"method":"save_file","params":{"id":"item","dest_path":"saved.bin"}}),
        json!({"method":"cancel_file_download","params":{"id":"transfer-token"}}),
        json!({"method":"add","params":{"content":"text"}}),
        json!({"method":"import_file","params":{"path":"/selected/file.pdf","filename":"file.pdf","mime_type":"application/pdf","source_reference":null}}),
        json!({"method":"delete","params":{"id":"item"}}),
        json!({"method":"delete_all","params":{"through":null}}),
        json!({"method":"history_ceiling"}),
        json!({"method":"pin","params":{"id":"item","pinned":true}}),
        json!({"method":"set_secret","params":{"id":"item","secret":true}}),
        json!({"method":"get_spoiler_state"}),
        json!({"method":"set_spoiler_revealed","params":{"id":"item","revealed":true}}),
        json!({"method":"reorder_pinned","params":{"ids":["item"]}}),
        json!({"method":"pair_create_invite"}),
        json!({"method":"pair_nearby","params":{"addr":"127.0.0.1:1"}}),
        json!({"method":"pair_join","params":{"code":"code","addr":"127.0.0.1:1"}}),
        json!({"method":"pair_progress"}),
        json!({"method":"pair_confirm","params":{"accept":true}}),
        json!({"method":"pair_cancel"}),
        json!({"method":"unpair","params":{"pairing_id":"peer"}}),
        json!({"method":"revoke","params":{"pairing_id":"peer"}}),
        json!({"method":"peers"}),
        json!({"method":"sync_now","params":{"pairing_id":null}}),
        json!({"method":"discovered"}),
        json!({"method":"rescan"}),
        json!({"method":"export","params":{"limit":0}}),
        json!({"method":"export_stream"}),
        json!({"method":"import","params":{"items":[]}}),
        json!({"method":"backup","params":{"dest_path":"backup.db"}}),
        json!({"method":"backup_stream","params":{"dest_path":"backup.db"}}),
        json!({"method":"restore","params":{"src_path":"backup.db","confirm":true}}),
        json!({"method":"cloud_sign_in","params":{"email":"a@example.com","password":"secret","passphrase":"phrase"}}),
        json!({"method":"cloud_sign_up","params":{"email":"a@example.com","password":"secret","passphrase":"phrase"}}),
        json!({"method":"cloud_set_endpoint","params":{"url":"https://example.supabase.co","anon_key":"anon"}}),
        json!({"method":"cloud_sign_out"}),
        json!({"method":"cloud_status"}),
        json!({"method":"cloud_sync_now"}),
        json!({"method":"get_config"}),
        json!({"method":"set_config","params":{"patch":{}}}),
        json!({"method":"set_private_mode","params":{"enabled":true}}),
        json!({"method":"get_private_mode"}),
        json!({"method":"watch"}),
        json!({"method":"watch_runtime"}),
        json!({"method":"watch_events","params":{"kinds":[{"kind":"ai_approval"},{"kind":"library","kinds":64},{"kind":"status"}]}}),
        json!({"method":"snippets","params":{"op":{"op":"quick_search","query":"sig","include_snippets":true,"limit":10}}}),
        json!({"method":"clips","params":{"op":{"op":"set_hidden_from_ai","ids":["item"],"hidden":true}}}),
        json!({"method":"paste_queue","params":{"op":{"op":"deliver","target":{"kind":"next"},"plain_text":null,"mode":"clipboard"}}}),
        json!({"method":"transforms","params":{"op":{"op":"run","source":{"kind":"clip","id":"item"},"steps_or_pipeline":{"kind":"pipeline","id":"p"},"action":"copy","bypass_default":false}}}),
        json!({"method":"rules","params":{"op":{"op":"apply","preview_token":"t","request_id":"r"}}}),
        json!({"method":"ai","params":{"op":{"op":"session_attach","session_id":"s"}}}),
        json!({"method":"shutdown"}),
    ]
}

#[test]
fn every_ipc_method_has_one_executable_wire_contract() {
    let mut names = BTreeSet::new();
    for fixture in catalog() {
        let method: Method = serde_json::from_value(fixture.clone()).unwrap();
        let name = wire_name(&method);
        assert!(names.insert(name), "duplicate contract for {name}");
        assert_eq!(fixture["method"], name);

        let encoded = serde_json::to_value(&method).unwrap();
        assert_eq!(encoded, fixture, "wire contract drifted for {name}");
    }
    assert_eq!(
        names.len(),
        70,
        "a Method has no wire fixture, or this count was not bumped with it"
    );
}

/// One representative of every operation in each feature group survives a
/// serde round trip, and the gates the daemon delegates to answer per op.
#[test]
fn every_feature_operation_round_trips_and_answers_its_gates() {
    use copypaste_ipc::{ai, clips, paste_queue, rules, snippets, transforms};
    let text = || "text".to_string();
    let id = || "id".to_string();
    let snippet_ops = vec![
        snippets::SnippetOp::List {
            scope: snippets::SnippetScope::Collection { id: id() },
            cursor: None,
            limit: 50,
        },
        snippets::SnippetOp::Save {
            draft: snippets::SnippetDraft {
                id: None,
                collection_id: None,
                title: text(),
                abbreviation: Some("sig".into()),
                body_text: text(),
                body_rich: None,
                template: true,
                pinned: false,
                concealed: false,
                base_revision: None,
            },
        },
        snippets::SnippetOp::Render {
            source: snippets::RenderSource::Body { text: text() },
            values: [("name".to_string(), text())].into(),
            clipboard: snippets::ClipboardInput::None,
            purpose: snippets::RenderPurpose::Preview,
            locale: Some("uk".into()),
        },
        snippets::SnippetOp::ImportCommit {
            token: id(),
            policies: snippets::ImportPolicies::default(),
        },
        snippets::SnippetOp::Release,
        snippets::SnippetOp::SetPreference {
            key: snippets::SnippetPreferenceKey::RequireAuth,
            value: true,
        },
    ];
    let clip_ops = vec![
        clips::ClipOp::SaveEdit {
            id: id(),
            text: text(),
            replace_original: true,
        },
        clips::ClipOp::DeliverText {
            text: text(),
            secret: true,
            delivery: clips::Delivery::Copy,
        },
        clips::ClipOp::SetHiddenFromAi {
            ids: vec![id()],
            hidden: false,
        },
    ];
    let queue_ops = vec![
        paste_queue::PasteQueueOp::Add {
            sources: vec![paste_queue::QueueSource::Text {
                text: text(),
                via: paste_queue::QueueTextVia::Share,
            }],
        },
        paste_queue::PasteQueueOp::SetCollecting {
            on: true,
            on_finished: paste_queue::QueueRestart::Append,
        },
        paste_queue::PasteQueueOp::Confirm {
            token: id(),
            outcome: paste_queue::QueueDeliveryOutcome::Inserted,
        },
    ];
    let transform_ops = vec![
        transforms::TransformOp::Preview {
            request_id: id(),
            source: transforms::TransformSource::Clipboard,
            steps: vec![transforms::TransformStep {
                transform_id: "upper".into(),
                version: 1,
                params_json: "{}".into(),
                enabled: true,
            }],
        },
        transforms::TransformOp::DefaultFor {
            app_identity: transforms::AppIdentity {
                platform: "macos".into(),
                identifier: "com.example".into(),
            },
        },
    ];
    let rule_ops = vec![
        rules::RuleOp::Test {
            definition_json: Some("{}".into()),
            rule_id: None,
            sample: rules::RuleSample::Text { text: text() },
            full_set: false,
        },
        rules::RuleOp::Save {
            draft: rules::RuleDraft {
                id: None,
                name: text(),
                enabled: true,
                definition_json: "{}".into(),
                device_scope: rules::RuleDeviceScope::Devices { ids: vec![id()] },
            },
        },
    ];
    let ai_ops = vec![
        ai::AiOp::Tool {
            session_id: id(),
            call: ai::AiToolCall::RenderSnippet {
                id: id(),
                values: Default::default(),
            },
        },
        ai::AiOp::Approve {
            request_id: id(),
            scopes: vec![ai::AiScope::ReadHistory],
            lookback: ai::AiLookback::Week,
        },
        ai::AiOp::Status { session_id: None },
    ];
    let methods: Vec<Method> = snippet_ops
        .into_iter()
        .map(|op| Method::Snippets { op })
        .chain(clip_ops.into_iter().map(|op| Method::Clips { op }))
        .chain(queue_ops.into_iter().map(|op| Method::PasteQueue { op }))
        .chain(
            transform_ops
                .into_iter()
                .map(|op| Method::Transforms { op }),
        )
        .chain(rule_ops.into_iter().map(|op| Method::Rules { op }))
        .chain(ai_ops.into_iter().map(|op| Method::Ai { op }))
        .collect();
    for method in methods {
        let encoded = serde_json::to_value(&method).unwrap();
        let decoded: Method = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(serde_json::to_value(&decoded).unwrap(), encoded);
        let gates = method.feature_op().expect("a feature-group method");
        assert!(
            !gates.interactive() || gates.blocking_weight() <= 32,
            "{method:?} would never fit the interactive pool"
        );
    }

    let privileged = |op| Method::Ai { op }.privileged();
    assert!(privileged(ai::AiOp::RevokeGrant { id: id() }));
    assert!(!privileged(ai::AiOp::Grants));
    assert!(Method::Clips {
        op: clips::ClipOp::SetHiddenFromAi {
            ids: vec![id()],
            hidden: false
        }
    }
    .privileged());
    assert!(!Method::Clips {
        op: clips::ClipOp::SetHiddenFromAi {
            ids: vec![id()],
            hidden: true
        }
    }
    .privileged());
    assert!(!Method::Status.privileged());
    assert!(Method::Rules {
        op: rules::RuleOp::Apply {
            preview_token: id(),
            request_id: id()
        }
    }
    .is_long_running());
}

#[test]
fn retired_pairing_methods_are_not_part_of_the_wire_contract() {
    for method in [
        json!({"method":"pair_create","params":{"name":"device"}}),
        json!({"method":"pair_accept","params":{"code":"code","addr":"127.0.0.1:1"}}),
    ] {
        assert!(serde_json::from_value::<Method>(method).is_err());
    }
}

#[test]
fn every_parameterized_method_rejects_a_missing_or_malformed_payload() {
    for fixture in catalog() {
        if fixture.get("params").is_none() {
            continue;
        }
        let name = fixture["method"].as_str().unwrap();
        assert!(
            serde_json::from_value::<Method>(json!({"method":name})).is_err(),
            "{name} accepted missing params"
        );
        assert!(
            serde_json::from_value::<Method>(json!({"method":name,"params":[]})).is_err(),
            "{name} accepted a non-object params value"
        );
    }
}

#[test]
fn unknown_methods_are_rejected_by_the_shared_contract() {
    assert!(serde_json::from_value::<Method>(json!({"method":"future_method"})).is_err());
}

#[test]
fn image_preview_resolution_is_an_optional_wire_argument() {
    let value = json!({"method":"image_preview","params":{"id":"item","max_edge":1024}});
    let method: Method = serde_json::from_value(value.clone()).unwrap();
    assert!(matches!(
        method,
        Method::ImagePreview {
            max_edge: Some(1024),
            ..
        }
    ));
    assert_eq!(serde_json::to_value(method).unwrap(), value);
}

#[test]
fn image_preview_bounds_roundtrip_without_changing_legacy_requests() {
    for value in [
        json!({"method":"image_preview","params":{"id":"item"}}),
        json!({"method":"image_preview","params":{"id":"item","bounds":{"width":1400,"height":320}}}),
    ] {
        let method: Method = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(method).unwrap(), value);
    }
}
