//! The runtime content-rule service: editing, testing, retroactive preview
//! and apply, and how rule edits from two devices converge.

use copypaste_core::sync::{apply_remote_version, open_version_bytes, RemoteVersion};
use copypaste_core::{ClipboardPayload, ClipboardWriteError};
use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::rules::{RuleDeviceScope, RuleDraft, RuleOp, RuleRow, RuleSample, TraceResult};
use copypaste_ipc::{content_type, ErrorCode, HistoryTagMutation, Method, Response, ResponseData};
use copypaste_runtime::{ClipboardWriter, Runtime, WriteOptions};

struct NoClipboard;

impl ClipboardWriter for NoClipboard {
    fn write(
        &self,
        _payload: &ClipboardPayload,
        _content_type: &str,
        _options: WriteOptions,
    ) -> Result<(), ClipboardWriteError> {
        Ok(())
    }
}

struct Device {
    runtime: Runtime,
    name: &'static str,
    _dir: tempfile::TempDir,
}

impl Device {
    fn new(name: &'static str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let runtime =
            Runtime::open_with_clipboard(dir.path(), name, 0, std::sync::Arc::new(NoClipboard))
                .unwrap();
        Self {
            runtime,
            name,
            _dir: dir,
        }
    }

    fn request(&self, op: RuleOp) -> Response {
        self.runtime.request_blocking(1, Method::Rules { op })
    }

    fn rules(&self, op: RuleOp) -> ResponseData {
        let response = self.request(op);
        assert!(response.ok, "{:?}", response.error);
        response.data.unwrap()
    }

    fn save(&self, id: Option<&str>, definition: &str) -> RuleRow {
        let ResponseData::Rule(row) = self.rules(RuleOp::Save {
            draft: RuleDraft {
                id: id.map(str::to_owned),
                name: "Rule".into(),
                enabled: true,
                definition_json: definition.into(),
                device_scope: RuleDeviceScope::AllDevices,
            },
        }) else {
            panic!("no rule");
        };
        row
    }

    fn list(&self) -> copypaste_ipc::rules::RuleList {
        let ResponseData::RuleList(list) = self.rules(RuleOp::List) else {
            panic!("no list");
        };
        list
    }

    fn rule(&self, id: &str) -> RuleRow {
        self.list().rules.into_iter().find(|r| r.id == id).unwrap()
    }

    fn user_rule_ids(&self) -> Vec<String> {
        self.list()
            .rules
            .into_iter()
            .filter(|rule| rule.origin == copypaste_ipc::rules::RuleOrigin::User)
            .map(|rule| rule.id)
            .collect()
    }

    fn capture(&self, text: &str) -> String {
        let runtime = &self.runtime;
        copypaste_core::ingest_into_with_capture_source_metadata(
            &runtime.store,
            &runtime.keyring,
            text,
            content_type::TEXT,
            copypaste_core::now_ms(),
            None,
            None,
            None,
            &runtime.settings.config(),
        )
        .unwrap()
        .into_item()
        .id
    }

    fn secret(&self, id: &str) -> bool {
        self.runtime
            .store
            .get(id)
            .unwrap()
            .unwrap()
            .clipboard_privacy()
            .secret
    }
}

/// Every version `from` holds, applied to `to` as a cloud pull would.
fn sync(from: &Device, to: &Device) {
    let (source, target) = (&from.runtime, &to.runtime);
    for row in source.store.versions_since(0, 5000).unwrap() {
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

fn contains(value: &str, actions: &str) -> String {
    format!(
        r#"{{"match":"all","conditions":[{{"type":"content_text","op":"contains","value":"{value}"}}],"actions":[{actions}]}}"#
    )
}

const MARK: &str =
    r#"{"type":"mark_secret"},{"type":"hide_from_ai"},{"type":"add_tags","tag_ids":["t1"]}"#;

fn preview(device: &Device, request_id: &str, rule_id: &str) -> Response {
    device.request(RuleOp::Preview {
        request_id: request_id.into(),
        definition_json: None,
        rule_id: Some(rule_id.into()),
        full_set: false,
        limit: 1000,
    })
}

#[test]
fn testing_a_sample_reports_each_condition_and_stores_nothing() {
    let device = Device::new("tester");
    let definition = r#"{"match":"any","conditions":[
        {"type":"content_text","op":"matches_regex","value":"inv-\\d+"},
        {"type":"source_app","any_of":["com.example.mail"],"negate":true}],
        "actions":[{"type":"add_tags","tag_ids":["invoices"]}]}"#;
    let ResponseData::RuleTrace(trace) = device.rules(RuleOp::Test {
        definition_json: Some(definition.into()),
        rule_id: None,
        sample: RuleSample::Text {
            text: "pay inv-42 today".into(),
        },
        full_set: false,
    }) else {
        panic!("no trace");
    };
    let step = &trace.steps[0];
    assert_eq!(step.result, TraceResult::Matched);
    let results: Vec<_> = step.conditions.iter().map(|c| c.result).collect();
    assert_eq!(results, [TraceResult::Matched, TraceResult::Unknown]);
    assert_eq!(step.actions, ["add_tags"]);
    assert_eq!(trace.tag_ids, ["invoices"]);
    assert_eq!(device.runtime.store.count().unwrap(), 0);
}

#[test]
fn editing_a_rule_changes_existing_clips_only_once_applied() {
    let device = Device::new("editor");
    let target = device.capture("alpha report");
    let pinned = device.capture("obsolete pinned");
    let obsolete = device.capture("obsolete note");
    device.capture("unrelated");
    device.runtime.request_blocking(
        1,
        Method::Pin {
            id: pinned.clone(),
            pinned: true,
        },
    );

    let store = &device.runtime.store;
    let save_tag = HistoryTagMutation::Save {
        id: None,
        name: "Reports".into(),
    };
    store
        .mutate_history_tags(&device.runtime.keyring.item_key(), &save_tag)
        .unwrap();
    let tag = store.history_tags(&[]).unwrap().tags.remove(0).id;
    let actions = format!(
        r#"{{"type":"mark_secret"}},{{"type":"hide_from_ai"}},{{"type":"add_tags","tag_ids":["{tag}"]}}"#
    );
    let marking = device.save(None, &contains("alpha", &actions));
    let skipping = device.save(None, &contains("obsolete", r#"{"type":"dont_save"}"#));
    assert!(!device.secret(&target), "a saved rule leaves history alone");

    let ResponseData::RulePreview(plan) = preview(&device, "p1", &marking.id).data.unwrap() else {
        panic!("no preview");
    };
    assert_eq!((plan.counts.matched, plan.counts.secret), (1, 1));
    assert_eq!(plan.items[0].id, target);
    let ResponseData::RuleApplyReport(report) = device.rules(RuleOp::Apply {
        preview_token: plan.token.clone(),
        request_id: "a1".into(),
    }) else {
        panic!("no report");
    };
    assert_eq!(
        (report.changed, report.failed, report.cancelled),
        (1, 0, false)
    );
    assert!(device.secret(&target));
    let annotations = device
        .runtime
        .store
        .item_annotations(std::slice::from_ref(&target))
        .unwrap()
        .remove(&target)
        .unwrap();
    assert!(annotations.hidden_from_ai);
    assert_eq!(annotations.rule_tag_ids, [tag]);
    let ResponseData::OperationProgress(Some(progress)) = device.rules(RuleOp::ApplyProgress {
        request_id: "a1".into(),
    }) else {
        panic!("no progress");
    };
    assert_eq!((progress.done, progress.total), (1, 1));
    let again = device.request(RuleOp::Apply {
        preview_token: plan.token,
        request_id: "a2".into(),
    });
    assert_eq!(again.error_code, Some(ErrorCode::NotFound));

    let ResponseData::RulePreview(plan) = preview(&device, "p2", &skipping.id).data.unwrap() else {
        panic!("no preview");
    };
    assert_eq!((plan.counts.matched, plan.counts.deleted), (1, 1));
    device.rules(RuleOp::Apply {
        preview_token: plan.token,
        request_id: "a3".into(),
    });
    assert!(device.runtime.store.get(&obsolete).unwrap().is_none());
    assert!(device.runtime.store.get(&pinned).unwrap().is_some());

    let ResponseData::RuleTrace(trace) = device
        .runtime
        .request_blocking(
            1,
            Method::Clips {
                op: ClipOp::Trace { id: target },
            },
        )
        .data
        .unwrap()
    else {
        panic!("no trace");
    };
    assert!(trace.secret && trace.hidden_from_ai);
    assert!(trace
        .steps
        .iter()
        .any(|step| step.rule_id == marking.id && step.result == TraceResult::Matched));
}

#[test]
fn a_cancelled_preview_of_a_thousand_clips_changes_nothing() {
    let device = Device::new("previewer");
    for n in 0..1000 {
        device.capture(&format!("item {n}"));
    }
    let rule = device.save(None, &contains("item", MARK));
    device.rules(RuleOp::Cancel {
        request_id: "early".into(),
    });
    let cancelled = preview(&device, "early", &rule.id);
    assert_eq!(cancelled.error_code, Some(ErrorCode::NotFound));

    let ResponseData::RulePreview(plan) = preview(&device, "full", &rule.id).data.unwrap() else {
        panic!("no preview");
    };
    assert_eq!((plan.counts.matched, plan.truncated), (1000, false));
    device.rules(RuleOp::Cancel {
        request_id: "full".into(),
    });
    let gone = device.request(RuleOp::Apply {
        preview_token: plan.token,
        request_id: "apply".into(),
    });
    assert_eq!(gone.error_code, Some(ErrorCode::NotFound));
    let secret = device
        .runtime
        .store
        .list(1000, 0)
        .unwrap()
        .iter()
        .filter(|item| item.clipboard_privacy().secret)
        .count();
    assert_eq!(secret, 0);
}

#[test]
fn concurrent_definition_edits_converge_and_keep_the_loser_for_restore() {
    let (a, b) = (Device::new("mac"), Device::new("pixel"));
    let rule = a.save(None, &contains("one", MARK));
    sync(&a, &b);
    let from_a = contains("two", MARK);
    let from_b = contains("three", MARK);
    a.save(Some(&rule.id), &from_a);
    std::thread::sleep(std::time::Duration::from_millis(5));
    b.save(Some(&rule.id), &from_b);
    sync(&a, &b);
    sync(&b, &a);

    assert_eq!(a.rule(&rule.id).definition_json, from_b);
    assert_eq!(b.rule(&rule.id).definition_json, from_b);
    assert!(b.list().conflicts.is_empty());
    let conflicts = a.list().conflicts;
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].rule_id, rule.id);

    let ResponseData::Rule(restored) = a.rules(RuleOp::RestoreConflict {
        conflict_id: conflicts[0].id.clone(),
    }) else {
        panic!("no rule");
    };
    assert_eq!(restored.definition_json, from_a);
    a.save(Some(&rule.id), &contains("four", MARK));
    sync(&a, &b);
    assert_eq!(b.rule(&rule.id).definition_json, contains("four", MARK));
    assert!(a.list().conflicts.is_empty() && b.list().conflicts.is_empty());
}

#[test]
fn concurrent_reorders_converge() {
    let (a, b) = (Device::new("mac"), Device::new("pixel"));
    let ids: Vec<String> = ["one", "two", "three"]
        .iter()
        .map(|value| a.save(None, &contains(value, MARK)).id)
        .collect();
    sync(&a, &b);
    a.rules(RuleOp::Reorder {
        id: ids[2].clone(),
        after_id: None,
    });
    b.rules(RuleOp::Reorder {
        id: ids[0].clone(),
        after_id: Some(ids[1].clone()),
    });
    sync(&a, &b);
    sync(&b, &a);
    let expected = vec![ids[2].clone(), ids[1].clone(), ids[0].clone()];
    assert_eq!(a.user_rule_ids(), expected);
    assert_eq!(b.user_rule_ids(), expected);
}

#[test]
fn a_delete_wins_over_a_concurrent_edit_and_stays_restorable() {
    let (a, b) = (Device::new("mac"), Device::new("pixel"));
    let rule = a.save(None, &contains("one", MARK));
    sync(&a, &b);
    a.rules(RuleOp::Delete {
        id: rule.id.clone(),
    });
    std::thread::sleep(std::time::Duration::from_millis(5));
    b.save(Some(&rule.id), &contains("edited", MARK));
    sync(&a, &b);
    sync(&b, &a);
    sync(&a, &b);
    assert!(a.rule(&rule.id).deleted_at.is_some());
    assert!(b.rule(&rule.id).deleted_at.is_some());
    assert_eq!(a.list().conflicts.len(), 1, "the losing edit is kept");
    assert_eq!(b.list().conflicts.len(), 1);

    b.rules(RuleOp::Restore {
        id: rule.id.clone(),
    });
    sync(&b, &a);
    assert!(a.rule(&rule.id).deleted_at.is_none());
}

#[test]
fn built_in_rules_seeded_on_two_devices_merge_into_one_each() {
    let (a, b) = (Device::new("mac"), Device::new("pixel"));
    a.runtime.sweep_deadlines();
    b.runtime.sweep_deadlines();
    sync(&a, &b);
    sync(&b, &a);
    a.runtime.sweep_deadlines();
    for device in [&a, &b] {
        let list = device.list();
        let mut builtins: Vec<_> = list
            .rules
            .iter()
            .filter(|rule| rule.origin == copypaste_ipc::rules::RuleOrigin::Builtin)
            .map(|rule| rule.id.clone())
            .collect();
        builtins.dedup();
        assert_eq!(
            builtins.len(),
            copypaste_core::rules::builtin::builtins().len()
        );
        assert!(list.conflicts.is_empty());
    }
}
