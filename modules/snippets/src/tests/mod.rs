mod fake;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use copypaste_module_sdk::{
    ClipboardText, LegacyCollection, LegacySnippet, LegacySnippetPage, Module, ModuleInvocation,
    ModuleOutput, ModuleView, ServiceReply, ViewAction, ViewItem,
};
use fake::Fake;
use serde_json::{json, Value};

use crate::Snippets;

fn module() -> (Snippets, Arc<Fake>, PathBuf) {
    let fake = Arc::new(Fake::default());
    fake.state().locale = "en".into();
    let dir = std::env::temp_dir().join(format!("copypaste-snippets-{}", uuid::Uuid::new_v4()));
    (Snippets::with_host(fake.clone(), dir.clone()), fake, dir)
}

fn invoke_with(
    module: &mut Snippets,
    command: &str,
    arguments: Value,
    preferences: Value,
) -> Result<ModuleOutput, String> {
    let output = module.invoke(ModuleInvocation {
        command: command.into(),
        arguments: serde_json::from_value(arguments).unwrap(),
        preferences: serde_json::from_value(preferences).unwrap(),
    })?;
    if let ModuleOutput::View { view } = &output {
        check_against_manifest(view);
    }
    Ok(output)
}

fn invoke(module: &mut Snippets, command: &str, arguments: Value) -> ModuleOutput {
    invoke_with(module, command, arguments, json!({})).unwrap()
}

fn view(output: ModuleOutput) -> ModuleView {
    match output {
        ModuleOutput::View { view } => view,
        other => panic!("a view expected, got {other:?}"),
    }
}

/// Every action a view carries names a declared command, and every
/// argument it or the host passes is declared by that command.
fn check_against_manifest(view: &ModuleView) {
    let manifest: Value = serde_json::from_str(include_str!("../../module.json")).unwrap();
    let declared = |command: &str| -> BTreeSet<String> {
        manifest["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| candidate["id"] == command)
            .unwrap_or_else(|| panic!("command {command} is not declared"))["arguments"]
            .as_array()
            .map(|fields| {
                fields
                    .iter()
                    .map(|field| field["id"].as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let check = |action: &ViewAction, implied: &[&str]| {
        let arguments = declared(&action.command);
        for key in action
            .arguments
            .keys()
            .map(String::as_str)
            .chain(implied.iter().copied())
        {
            assert!(arguments.contains(key), "{} takes no {key}", action.command);
        }
    };
    for section in &view.sections {
        for item in &section.items {
            match item {
                ViewItem::Row { actions, open, .. } => {
                    actions
                        .iter()
                        .chain(open)
                        .for_each(|action| check(action, &[]));
                }
                ViewItem::Search { action, .. } => check(action, &["query"]),
                ViewItem::Filter { action, .. } => check(action, &["value"]),
                ViewItem::Toolbar { actions } => {
                    actions.iter().for_each(|action| check(action, &[]))
                }
                ViewItem::Form(form) => {
                    std::iter::once(&form.submit)
                        .chain(&form.preview)
                        .for_each(|action| check(action, &["values"]));
                }
                _ => {}
            }
        }
        section
            .bulk_actions
            .iter()
            .for_each(|action| check(action, &["ids"]));
        section
            .reorder
            .iter()
            .for_each(|action| check(action, &["id", "before"]));
        section.more.iter().for_each(|action| check(action, &[]));
    }
}

fn save(module: &mut Snippets, fake: &Fake, title: &str, abbreviation: &str, body: &str) -> String {
    let output = invoke(
        module,
        "save",
        json!({"values": {"title": title, "abbreviation": abbreviation, "body": body,
            "template": body.contains("{{"), "concealed": title.starts_with("Secret")}}),
    );
    assert!(matches!(output, ModuleOutput::Message { .. }), "{output:?}");
    snippet_id(fake, title)
}

fn snippet_id(fake: &Fake, title: &str) -> String {
    fake.state()
        .records
        .iter()
        .find(|(key, value)| {
            key.starts_with("meta/")
                && serde_json::from_str::<Value>(value).unwrap()["title"] == title
        })
        .map(|(key, _)| key["meta/".len()..].to_owned())
        .unwrap_or_else(|| panic!("no snippet {title}"))
}

fn results(output: ModuleOutput) -> Vec<(String, bool)> {
    match output {
        ModuleOutput::Results { items } => items
            .into_iter()
            .map(|item| (item.id, item.preselect))
            .collect(),
        other => panic!("results expected, got {other:?}"),
    }
}

#[test]
fn library_rules_hold_and_every_view_matches_the_manifest() {
    let (mut module, fake, _) = module();
    let first = save(&mut module, &fake, "Signature", "sig", "Best regards");
    let taken = view(invoke(
        &mut module,
        "save",
        json!({"values": {"title": "Other", "abbreviation": "SIG", "body": "x"}}),
    ));
    let ViewItem::Form(form) = &taken.sections[0].items[0] else {
        panic!("the editor expected");
    };
    assert_eq!(form.errors["abbreviation"], "Already used by “Signature”");

    view(invoke(&mut module, "library", json!({})));
    view(invoke(&mut module, "edit", json!({"id": first})));
    view(invoke(&mut module, "collection-edit", json!({})));
    view(invoke(&mut module, "import", json!({})));
    view(invoke(&mut module, "export", json!({"scope": "all"})));
    view(invoke(&mut module, "move-to", json!({"ids": [first]})));

    invoke(&mut module, "delete", json!({"ids": [first]}));
    let deleted = view(invoke(&mut module, "browse", json!({"scope": "deleted"})));
    assert!(deleted.sections[0].items.iter().any(|item| matches!(
        item,
        ViewItem::Row { id: Some(id), .. } if *id == first
    )));
    invoke(&mut module, "purge", json!({"ids": [first]}));
    let stub = fake.record(&format!("meta/{first}")).unwrap();
    assert_eq!(
        (stub["purged"].clone(), stub["title"].clone()),
        (json!(true), json!(""))
    );
    assert!(fake.record(&format!("body/{first}")).is_none());
}

#[test]
fn quick_paste_ranks_tiers_and_delivers_filled_templates() {
    let (mut module, fake, _) = module();
    let body = save(&mut module, &fake, "Gamma", "", "ends with sig");
    let title = save(&mut module, &fake, "sig notes", "", "first");
    let prefix = save(&mut module, &fake, "Beta", "sig2", "second");
    let exact = save(&mut module, &fake, "Alpha", "Sig", "third");
    assert_eq!(
        results(invoke(&mut module, "search", json!({"query": "sig"}))),
        [
            (exact, true),
            (prefix.clone(), false),
            (title, false),
            (body, false)
        ]
    );

    save(&mut module, &fake, "Name", "nm", "world");
    let template = save(
        &mut module,
        &fake,
        "Greeting",
        "",
        r#"{{field name="who" required=true}} {{snippet abbr=nm | upper}}: [{{clipboard}}]{{cursor}}!"#,
    );
    let form = view(invoke(&mut module, "use", json!({"id": template})));
    assert!(matches!(&form.sections[0].items[0], ViewItem::Form(form) if form.fields.len() == 1));

    fake.state().clipboard = Some(ClipboardText {
        text: Some("copied".into()),
        secret: false,
    });
    let delivered = invoke_with(
        &mut module,
        "fill",
        json!({"id": template, "values": {"f0": "Hello"}}),
        json!({"record-pasted": true}),
    )
    .unwrap();
    let ModuleOutput::Deliver { delivery } = delivered else {
        panic!("a delivery expected");
    };
    assert_eq!(delivery.text, "Hello WORLD: [copied]!");
    assert_eq!(delivery.cursor_graphemes_from_end, Some(1));
    assert!(delivery.record_history && !delivery.secret);

    fake.state().clipboard = Some(ClipboardText {
        text: Some("token".into()),
        secret: true,
    });
    let ModuleOutput::Deliver { delivery } = invoke_with(
        &mut module,
        "fill",
        json!({"id": template, "values": {"f0": "Hi"}}),
        json!({"record-pasted": true, "require-auth": true}),
    )
    .unwrap() else {
        panic!("a delivery expected");
    };
    assert!(delivery.secret && delivery.require_auth && !delivery.record_history);
    let usage = fake
        .state()
        .local
        .values()
        .any(|value| value.contains(&template));
    assert!(usage, "usage is counted locally");
}

#[test]
fn concurrent_body_edits_leave_one_deterministic_copy() {
    let (mut module, fake, _) = module();
    let id = save(&mut module, &fake, "Note", "", "local edit");
    let key = format!("body/{id}");
    let local = fake.record(&key).unwrap();
    let local_revision = local["revision"].as_str().unwrap().to_owned();

    let fast_forward = json!({"revision": "f".repeat(32), "ancestors": [local_revision],
        "text": "newer"});
    fake.sync_replace(&key, fast_forward);
    invoke(&mut module, "library", json!({}));
    assert!(fake.state().conflicts.is_empty());
    let copies = |fake: &Fake| {
        fake.state()
            .records
            .keys()
            .filter(|key| key.starts_with("meta/conflict-"))
            .count()
    };
    assert_eq!(copies(&fake), 0);

    let concurrent = json!({"revision": "e".repeat(32), "ancestors": [], "text": "remote edit"});
    fake.sync_replace(&key, concurrent);
    invoke(&mut module, "library", json!({}));
    let copy = crate::conflict::copy_id(&"f".repeat(32));
    let meta = fake.record(&format!("meta/{copy}")).unwrap();
    assert!(meta["title"]
        .as_str()
        .unwrap()
        .starts_with("Note (conflict, "));
    assert_eq!(
        fake.record(&format!("body/{copy}")).unwrap()["text"],
        "newer"
    );
    assert!(fake.state().conflicts.is_empty());
}

fn legacy(id: &str, text: &str, concealed: bool) -> LegacySnippet {
    LegacySnippet {
        id: id.into(),
        collection_id: Some("c1".into()),
        title: format!("Legacy {id}"),
        abbreviation: Some(format!("l{id}")),
        template: false,
        pinned: false,
        pin_order: None,
        order_key: "V".into(),
        concealed,
        deleted_at_ms: None,
        created_at_ms: 1,
        created_device: "phone".into(),
        revision: "a".repeat(32),
        ancestors: Vec::new(),
        text: text.into(),
        rich_html: None,
        use_count: 3,
        last_used_at_ms: Some(5),
    }
}

#[test]
fn the_legacy_library_moves_once_without_overwriting_module_data() {
    let (mut module, fake, dir) = module();
    fake.state().records.insert(
        "meta/s2".into(),
        json!({"title": "Newer", "order_key": "V", "preview": "newer"}).to_string(),
    );
    fake.state().legacy = Some(LegacySnippetPage {
        collections: vec![LegacyCollection {
            id: "c1".into(),
            name: "Work".into(),
            icon: "folder".into(),
            accent: "blue".into(),
            order_key: "V".into(),
            deleted_at_ms: None,
        }],
        snippets: vec![
            legacy("s1", "secret value", true),
            legacy("s2", "old", false),
        ],
        next: None,
    });
    invoke(&mut module, "library", json!({}));
    assert_eq!(fake.record("collection/c1").unwrap()["name"], "Work");
    let body = fake.record("body/s1").unwrap();
    assert_eq!(
        (body.get("text"), body["secret_text"].clone()),
        (None, json!("secret value"))
    );
    assert_eq!(fake.record("meta/s2").unwrap()["title"], "Newer");
    assert!(
        fake.record("body/s2").is_none(),
        "a known snippet is left alone"
    );
    assert!(fake
        .state()
        .local
        .values()
        .any(|usage| usage.contains("\"s1\":[3,5]")));

    let mut again = Snippets::with_host(fake.clone(), dir);
    invoke(&mut again, "library", json!({}));
    assert_eq!(fake.state().legacy_calls, 1);
}

#[test]
fn the_snippet_service_answers_like_the_runtime_did() {
    let (mut module, fake, _) = module();
    save(&mut module, &fake, "Plain", "pl", "visible");
    save(&mut module, &fake, "Secret key", "", "hidden");
    let clip = save(&mut module, &fake, "Clip", "", "[{{clipboard}}]");
    let fields = save(
        &mut module,
        &fake,
        "Fields",
        "",
        r#"{{field name="who" required=true}}"#,
    );
    let serve = |module: &mut Snippets, request: Value| -> ServiceReply {
        let ModuleOutput::Data { data } =
            invoke(module, "serve", json!({"request": request.to_string()}))
        else {
            panic!("data expected");
        };
        serde_json::from_value(data).unwrap()
    };
    let ServiceReply::Ok { reply } = serve(
        &mut module,
        json!({"operation": "snippet_search", "query": "", "limit": 50}),
    ) else {
        panic!("a list expected");
    };
    let titles: BTreeSet<&str> = reply
        .as_array()
        .unwrap()
        .iter()
        .map(|summary| summary["title"].as_str().unwrap())
        .collect();
    assert!(!titles.contains("Secret key") && titles.contains("Plain"));

    let refused = |reply: ServiceReply| match reply {
        ServiceReply::Error { error } => error.message,
        ServiceReply::Ok { reply } => panic!("refusal expected, got {reply}"),
    };
    let render = |id: &str, values: Value| json!({"operation": "snippet_render", "id": id, "values": values});
    assert!(refused(serve(&mut module, render(&clip, json!({})))).contains("clipboard"));
    assert_eq!(
        refused(serve(&mut module, render(&fields, json!({})))),
        "Fill in these values: who."
    );
    assert_eq!(
        refused(serve(
            &mut module,
            json!({"operation": "snippet_save", "title": "New", "body": "x", "abbreviation": "PL"})
        )),
        "That abbreviation is already used by another snippet."
    );
}

#[test]
fn renaming_an_abbreviation_can_rewrite_the_templates_that_insert_it() {
    let (mut module, fake, _) = module();
    let address = save(&mut module, &fake, "Address", "addr", "street");
    let letter = save(
        &mut module,
        &fake,
        "Letter",
        "",
        "{{snippet abbr=addr | upper}}",
    );
    let base = fake.record(&format!("body/{address}")).unwrap()["revision"].clone();
    let values = json!({"title": "Address", "abbreviation": "home", "body": "street"});
    let prompt = view(invoke(
        &mut module,
        "save",
        json!({"id": address, "base": base, "values": values}),
    ));
    assert!(matches!(
        &prompt.sections[0].items[1],
        ViewItem::Toolbar { .. }
    ));
    invoke(
        &mut module,
        "save",
        json!({"id": address, "base": base, "references": "update", "values": values}),
    );
    assert_eq!(
        fake.record(&format!("body/{letter}")).unwrap()["text"],
        r#"{{snippet abbr="home" | upper}}"#
    );
}

#[test]
fn an_encrypted_export_imports_back_under_the_chosen_policies() {
    let (mut module, fake, dir) = module();
    save(&mut module, &fake, "One", "one", "first");
    save(&mut module, &fake, "Secret two", "", "second");
    let ModuleOutput::SaveFile { path, .. } = invoke(
        &mut module,
        "export-run",
        json!({"scope": "all", "values": {"passphrase": "correct horse",
            "passphrase_confirm": "correct horse"}}),
    ) else {
        panic!("a file expected");
    };
    let file = dir.join(path);
    let locked = view(invoke(
        &mut module,
        "import-preview",
        json!({"values": {"file": file.to_str().unwrap()}}),
    ));
    let ViewItem::Form(form) = &locked.sections[0].items[0] else {
        panic!("the import form expected");
    };
    assert!(form.errors.contains_key("passphrase"));

    let preview = view(invoke(
        &mut module,
        "import-preview",
        json!({"values": {"file": file.to_str().unwrap(), "passphrase": "correct horse"}}),
    ));
    let token = preview.sections[0]
        .items
        .iter()
        .find_map(|item| match item {
            ViewItem::Form(form) => form.submit.arguments.get("token").cloned(),
            _ => None,
        })
        .unwrap();
    let ModuleOutput::Message { message } = invoke(
        &mut module,
        "import-commit",
        json!({"token": token, "values": {"same_id": ["keep-both"]}}),
    ) else {
        panic!("a report expected");
    };
    assert!(message.starts_with("Added: 0."), "{message}");
    let _ = std::fs::remove_dir_all(dir);
    let metas = fake
        .state()
        .records
        .keys()
        .filter(|key| key.starts_with("meta/"))
        .count();
    assert_eq!(metas, 2, "identical rows are skipped");
}
