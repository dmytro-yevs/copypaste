//! The runtime transform service: chains, delivery, provenance, pipelines
//! and preferences across synced devices.

use std::sync::{Arc, Mutex};

use copypaste_core::storage::organization::{PipelineRecord, Record, RecordKey};
use copypaste_core::sync::{apply_remote_version, open_version_bytes, RemoteVersion};
use copypaste_core::{ClipboardPayload, ClipboardWriteError};
use copypaste_ipc::clips::{ClipOp, Delivered};
use copypaste_ipc::payload::ProvenanceKind;
use copypaste_ipc::transforms::{
    AppIdentity, Pipeline, PipelineDraft, TransformAction, TransformOp, TransformSource,
    TransformStep, TransformSteps,
};
use copypaste_ipc::{content_type, ErrorCode, Item, Method, Response, ResponseData};
use copypaste_runtime::{ClipboardWriter, Runtime, WriteOptions};

#[derive(Default)]
struct Clipboard(Mutex<Vec<(String, String, WriteOptions)>>);

impl ClipboardWriter for Clipboard {
    fn write(
        &self,
        payload: &ClipboardPayload,
        content_type: &str,
        options: WriteOptions,
    ) -> Result<(), ClipboardWriteError> {
        let text = payload.plain_text().unwrap_or("<binary>").to_owned();
        self.0
            .lock()
            .unwrap()
            .push((text, content_type.to_owned(), options));
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
        Self {
            runtime,
            clipboard,
            name,
            _dir: dir,
        }
    }

    fn request(&self, op: TransformOp) -> Response {
        self.runtime.request_blocking(1, Method::Transforms { op })
    }

    fn data(&self, op: TransformOp) -> ResponseData {
        let response = self.request(op);
        assert!(response.ok, "{:?}", response.error);
        response.data.unwrap()
    }

    fn capture(&self, text: &str, app: Option<(&str, &str)>) -> String {
        let runtime = &self.runtime;
        let item = copypaste_core::ingest_into_with_capture_source(
            &runtime.store,
            &runtime.keyring,
            text,
            content_type::TEXT,
            copypaste_core::now_ms(),
            app.map(|(bundle, _)| bundle),
            app.map(|(_, name)| name),
            &runtime.settings.config(),
        )
        .unwrap()
        .into_item();
        runtime.note_capture(item.created_at, &item.id);
        item.id
    }

    fn writes(&self) -> Vec<(String, String, WriteOptions)> {
        self.clipboard.0.lock().unwrap().clone()
    }

    fn run(
        &self,
        source: TransformSource,
        steps: TransformSteps,
        action: TransformAction,
    ) -> Response {
        self.request(TransformOp::Run {
            source,
            steps_or_pipeline: steps,
            action,
            bypass_default: false,
        })
    }

    fn save(&self, id: Option<&str>, name: &str, steps: Vec<TransformStep>) -> Pipeline {
        match self.data(TransformOp::SavePipeline {
            draft: PipelineDraft {
                id: id.map(str::to_owned),
                name: name.into(),
                description: None,
                steps,
            },
        }) {
            ResponseData::Pipeline(pipeline) => pipeline,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn pipelines(&self) -> Vec<Pipeline> {
        match self.data(TransformOp::Pipelines) {
            ResponseData::Pipelines(pipelines) => pipelines,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn default_for(&self, platform: &str, identifier: &str) -> Option<String> {
        match self.data(TransformOp::DefaultFor {
            app_identity: AppIdentity {
                platform: platform.into(),
                identifier: identifier.into(),
            },
        }) {
            ResponseData::PipelineDefault(pipeline) => pipeline,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn set_preference(&self, key: &str, value_json: &str) {
        self.data(TransformOp::SetPreference {
            key: key.into(),
            value_json: value_json.into(),
        });
    }
}

/// Every version `from` holds, applied to `to` as a cloud pull would.
fn sync(from: &Device, to: &Device) {
    let (source, target) = (&from.runtime, &to.runtime);
    for row in source
        .store
        .versions_since(0, 1000)
        .unwrap()
        .into_iter()
        .rev()
    {
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

fn step(transform_id: &str) -> TransformStep {
    TransformStep {
        transform_id: transform_id.into(),
        version: 1,
        params_json: "{}".into(),
        enabled: true,
    }
}

fn steps(ids: &[&str]) -> TransformSteps {
    TransformSteps::Steps {
        steps: ids.iter().map(|id| step(id)).collect(),
    }
}

fn text(value: &str) -> TransformSource {
    TransformSource::Text { text: value.into() }
}

fn item(response: Response) -> Item {
    match response.data {
        Some(ResponseData::Item(item)) => item,
        _ => panic!("unexpected {response:?}"),
    }
}

fn delivered(response: Response) -> Delivered {
    match response.data {
        Some(ResponseData::Delivered(delivered)) => delivered,
        _ => panic!("unexpected {response:?}"),
    }
}

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

#[test]
fn a_failing_step_stops_the_chain_and_names_the_step() {
    let device = Device::new("a");
    let mut chain = vec![
        step("case.upper"),
        step("encode.base64_decode"),
        step("case.lower"),
    ];
    let preview = |chain: &Vec<TransformStep>| match device.data(TransformOp::Preview {
        request_id: "r".into(),
        source: text("hello world!"),
        steps: chain.clone(),
    }) {
        ResponseData::TransformPreview(preview) => preview,
        other => panic!("unexpected {other:?}"),
    };
    let failed = preview(&chain);
    assert_eq!(failed.steps[0].output_prefix, "HELLO WORLD!");
    assert!(failed.steps[1].error.is_some());
    assert!(failed.steps[2].not_run && failed.steps[2].output_prefix.is_empty());

    chain[1].enabled = false;
    let skipped = preview(&chain);
    assert!(skipped.steps[1].not_run && skipped.steps[1].error.is_none());
    assert_eq!(skipped.steps[2].output_prefix, "hello world!");
    assert!(skipped.no_changes);

    chain[1].enabled = true;
    let run = device.run(
        text("hello world!"),
        TransformSteps::Steps { steps: chain },
        TransformAction::Paste,
    );
    assert_eq!(run.error_code, Some(ErrorCode::InvalidRequest));
    assert!(run.error.unwrap().starts_with("Step 2 failed"));
    assert!(device.writes().is_empty(), "a failed run writes nothing");
}

#[test]
fn paste_adds_no_history_row_and_restores_the_previous_clipboard() {
    let device = Device::new("a");
    device.capture("previous", None);
    let rows = device.runtime.store.count().unwrap();
    let pasted = delivered(device.run(
        text("hello"),
        steps(&["case.upper"]),
        TransformAction::Paste,
    ));
    assert_eq!(device.runtime.store.count().unwrap(), rows);
    let (written, _, options) = device.writes().pop().unwrap();
    assert_eq!(written, "HELLO");
    assert!(options.transient);

    let restored = device.runtime.request_blocking(
        2,
        Method::Clips {
            op: ClipOp::RestoreClipboard {
                token: pasted.token.unwrap(),
                after_ms: 0,
            },
        },
    );
    assert!(restored.ok);
    assert_eq!(device.writes().pop().unwrap().0, "previous");

    device.set_preference("restore_clipboard_after_paste", "false");
    let unrestored =
        delivered(device.run(text("x"), steps(&["case.upper"]), TransformAction::Paste));
    assert!(unrestored.token.is_none());
}

#[test]
fn copy_creates_a_plain_text_clip_with_provenance() {
    let device = Device::new("a");
    let source = device.capture("Hello World", Some(("com.example.editor", "Editor")));
    let copied = item(device.run(
        TransformSource::Clip { id: source.clone() },
        steps(&["case.snake"]),
        TransformAction::Copy,
    ));
    assert_eq!(copied.content, "hello_world");
    assert_eq!(copied.content_type, content_type::TEXT);
    assert_eq!(
        copied.source_app_bundle_id.as_deref(),
        Some("com.example.editor")
    );
    let provenance = copied.annotations.unwrap().provenance.unwrap();
    assert_eq!(provenance.kind, ProvenanceKind::Transformed);
    assert_eq!(provenance.from_clip_id.as_deref(), Some(source.as_str()));
    assert_eq!(provenance.transform_snapshot[0].transform_id, "case.snake");

    let (written, written_type, options) = device.writes().pop().unwrap();
    assert_eq!(
        (written.as_str(), written_type.as_str()),
        ("hello_world", content_type::TEXT)
    );
    assert!(!options.transient);
    assert!(device.runtime.store.get(&source).unwrap().is_some());
}

#[test]
fn save_as_new_clip_keeps_the_original_and_the_clipboard() {
    let device = Device::new("a");
    let source = device.capture("original", None);
    let writes = device.writes().len();
    let rows = device.runtime.store.count().unwrap();
    let saved = item(device.run(
        TransformSource::Clip { id: source.clone() },
        steps(&["case.upper"]),
        TransformAction::SaveNewClip,
    ));
    assert_eq!(saved.content, "ORIGINAL");
    assert_ne!(saved.id, source);
    assert_eq!(device.runtime.store.count().unwrap(), rows + 1);
    assert_eq!(device.writes().len(), writes);
    assert!(device.runtime.store.get(&source).unwrap().is_some());
}

#[test]
fn oversized_input_is_refused() {
    let device = Device::new("a");
    let input = "a".repeat(17 * 1024 * 1024);
    let preview = device.request(TransformOp::Preview {
        request_id: "big".into(),
        source: text(&input),
        steps: vec![step("case.upper")],
    });
    assert_eq!(preview.error_code, Some(ErrorCode::ContentTooLarge));
    let run = device.run(
        text(&input),
        steps(&["case.upper"]),
        TransformAction::Return,
    );
    assert_eq!(run.error_code, Some(ErrorCode::ContentTooLarge));
}

#[test]
fn concurrent_step_edits_converge_to_one_whole_list() {
    let (a, b) = (Device::new("a"), Device::new("b"));
    let id = a.save(None, "Clean", vec![step("case.upper")]).id;
    sync(&a, &b);
    let from_a = vec![step("case.lower"), step("lines.trim")];
    let from_b = vec![step("case.snake")];
    a.save(Some(&id), "Clean", from_a.clone());
    b.save(Some(&id), "Clean", from_b.clone());
    sync(&a, &b);
    sync(&b, &a);
    let (left, right) = (a.pipelines(), b.pipelines());
    assert_eq!(left, right);
    assert!(left[0].steps == from_a || left[0].steps == from_b);
}

#[test]
fn an_edit_after_a_remote_delete_restores_the_pipeline() {
    let (a, b) = (Device::new("a"), Device::new("b"));
    let id = a.save(None, "Tidy", vec![step("lines.trim")]).id;
    sync(&a, &b);
    a.data(TransformOp::DeletePipeline { id: id.clone() });
    assert!(a.pipelines().is_empty());
    pause();
    b.save(Some(&id), "Tidy lines", vec![step("lines.trim_each")]);
    sync(&a, &b);
    sync(&b, &a);
    for device in [&a, &b] {
        let pipelines = device.pipelines();
        assert_eq!(pipelines.len(), 1);
        assert_eq!(pipelines[0].name, "Tidy lines");
    }
}

#[test]
fn a_pipeline_with_an_unknown_step_round_trips_and_cannot_run() {
    let (a, b, c) = (Device::new("a"), Device::new("b"), Device::new("c"));
    let steps_json =
        r#"[{"transform_id":"future.magic","version":3,"params":{"depth":2},"enabled":true}]"#;
    let record = Record::Pipeline(PipelineRecord {
        id: "from-newer".into(),
        name: "Newer".into(),
        description: None,
        steps_json: steps_json.into(),
        deleted_at: None,
    });
    a.runtime
        .store
        .write_organization_record(&a.runtime.keyring.item_key(), record.clone())
        .unwrap();
    sync(&a, &b);
    let listed = &b.pipelines()[0].steps[0];
    assert_eq!(
        (listed.transform_id.as_str(), listed.version),
        ("future.magic", 3)
    );
    assert_eq!(listed.params_json, r#"{"depth":2}"#);

    let run = b.run(
        text("x"),
        TransformSteps::Pipeline {
            id: "from-newer".into(),
        },
        TransformAction::Paste,
    );
    assert_eq!(run.error_code, Some(ErrorCode::Unsupported));
    assert!(b.writes().is_empty());

    sync(&b, &c);
    let stored = c
        .runtime
        .store
        .organization_record(RecordKey::Pipeline("from-newer"))
        .unwrap();
    assert_eq!(stored, Some(record));
}

#[test]
fn concurrent_tracking_additions_both_survive() {
    let (a, b) = (Device::new("a"), Device::new("b"));
    let add = |device: &Device, member: &str| {
        device.data(TransformOp::SetMember {
            set: "tracking_remove".into(),
            member: member.into(),
            present: true,
        });
    };
    add(&a, "campaign_ref");
    add(&b, "si@example.com");
    sync(&a, &b);
    sync(&b, &a);
    for device in [&a, &b] {
        match device.data(TransformOp::Preferences) {
            ResponseData::TransformPreferences(preferences) => assert_eq!(
                preferences.tracking_remove,
                ["campaign_ref", "si@example.com"]
            ),
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn defaults_resolve_per_app_and_a_failing_default_leaves_the_clipboard_for_the_original() {
    let device = Device::new("a");
    let global = device
        .save(None, "Links", vec![step("url.remove_tracking")])
        .id;
    let mac = device.save(None, "Trim", vec![step("lines.trim")]).id;
    device.set_preference("default_paste_pipeline_id", &format!("\"{global}\""));
    device.set_preference(
        "app_default:macos:com.example.mail",
        &format!(r#"{{"pipeline_id":"{mac}"}}"#),
    );
    device.set_preference(
        "app_default:macos:com.example.raw",
        r#"{"pipeline_id":null}"#,
    );
    assert_eq!(device.default_for("macos", "com.example.mail"), Some(mac));
    assert_eq!(
        device.default_for("windows", "com.example.mail"),
        Some(global.clone())
    );
    assert_eq!(device.default_for("macos", "com.example.raw"), None);

    let decode = device
        .save(None, "Decode", vec![step("encode.base64_decode")])
        .id;
    let failed = device.run(
        text("not base64!"),
        TransformSteps::Pipeline { id: decode.clone() },
        TransformAction::Paste,
    );
    assert!(!failed.ok);
    assert!(device.writes().is_empty());
    let original = device.request(TransformOp::Run {
        source: text("not base64!"),
        steps_or_pipeline: TransformSteps::Pipeline { id: decode },
        action: TransformAction::Paste,
        bypass_default: true,
    });
    assert!(original.ok);
    assert_eq!(device.writes().pop().unwrap().0, "not base64!");

    device.data(TransformOp::DeletePipeline { id: global });
    assert_eq!(device.default_for("windows", "com.example.mail"), None);
}
