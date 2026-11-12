use std::cell::Cell;
use std::path::Path;

use copypaste_core::snippets::bundle::TransferError;
use copypaste_core::snippets::{export, import};
use copypaste_core::storage::snippets::{CollectionDraft, SnippetDraft, SnippetScope};
use copypaste_core::{Keyring, Store};
use copypaste_ipc::snippets::{
    ExportScope, ImportFormat, ImportPolicies, ImportReport, ImportRowOutcome,
};

const PASSPHRASE: &str = "correct horse battery";

struct Library {
    store: Store,
    keys: Keyring,
}

fn library(secret: u8) -> Library {
    let keys = Keyring::from_secret(&[secret; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    store.device_identity("device").unwrap();
    Library { store, keys }
}

impl Library {
    fn save(&self, title: &str, abbreviation: Option<&str>, body: &str, concealed: bool) -> String {
        let draft = SnippetDraft {
            title: title.into(),
            abbreviation: abbreviation.map(str::to_owned),
            body_text: body.into(),
            concealed,
            ..SnippetDraft::default()
        };
        self.store
            .save_snippet(&self.keys.item_key(), &draft)
            .unwrap()
            .snippet
            .id
    }

    fn preview(
        &self,
        path: &Path,
        passphrase: Option<&str>,
        format: ImportFormat,
        convert: bool,
    ) -> Result<import::ImportPlan, TransferError> {
        import::read(path, passphrase, format, convert)
            .and_then(|parsed| import::plan(&self.store, parsed))
    }

    fn commit(&self, plan: import::ImportPlan) -> ImportReport {
        import::commit(
            &self.store,
            &self.keys.item_key(),
            plan,
            ImportPolicies::default(),
            &|| false,
        )
        .unwrap()
    }

    /// Live snippets as `(title, abbreviation, body, template)`.
    fn snippets(&self) -> Vec<(String, Option<String>, String, bool)> {
        let page = self
            .store
            .snippet_page(&SnippetScope::All, None, 1_000)
            .unwrap();
        page.snippets
            .iter()
            .map(|summary| {
                let detail = self.store.snippet(&summary.id).unwrap().unwrap();
                (
                    detail.snippet.title,
                    detail.snippet.abbreviation,
                    detail.body_text,
                    detail.snippet.template,
                )
            })
            .collect()
    }
}

#[test]
fn an_encrypted_export_needs_its_passphrase_and_a_wrong_one_writes_nothing() {
    let source = library(1);
    source.save("Greeting", Some("hi"), "Hello there", false);
    source.save("Token", None, "s3cr3t", true);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("library.cpsnippets");
    let written = export::export(
        &source.store,
        &path,
        &ExportScope::All,
        Some(PASSPHRASE),
        false,
    )
    .unwrap();
    assert_eq!(written, 2, "an encrypted export keeps concealed snippets");
    let raw = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&raw).contains("Hello"));

    let target = library(2);
    for (passphrase, expected) in [
        (None, "PassphraseRequired"),
        (Some("wrong passphrase"), "WrongPassphrase"),
    ] {
        let refused = target.preview(&path, passphrase, ImportFormat::Bundle, false);
        assert_eq!(format!("{:?}", refused.err().unwrap()), expected);
    }
    assert_eq!(target.store.snippet_count().unwrap(), 0);

    let plan = target
        .preview(&path, Some(PASSPHRASE), ImportFormat::Bundle, false)
        .unwrap();
    assert_eq!(plan.counts().new, 2);
    assert_eq!(target.commit(plan).inserted, 2);
}

#[test]
fn reimporting_an_own_export_skips_every_snippet_as_identical() {
    let source = library(1);
    let collection = source
        .store
        .save_snippet_collection(
            &source.keys.item_key(),
            &CollectionDraft {
                id: None,
                name: "Work".into(),
                icon: "folder".into(),
                accent: "neutral".into(),
            },
        )
        .unwrap();
    for index in 0..3 {
        let draft = SnippetDraft {
            collection_id: Some(collection.id.clone()),
            title: format!("Snippet {index}"),
            abbreviation: Some(format!("s{index}")),
            body_text: format!("Body {{{{cursor}}}} {index}"),
            template: true,
            ..SnippetDraft::default()
        };
        source
            .store
            .save_snippet(&source.keys.item_key(), &draft)
            .unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.json");
    export::export(&source.store, &path, &ExportScope::All, None, false).unwrap();

    let plan = source
        .preview(&path, None, ImportFormat::Bundle, false)
        .unwrap();
    assert_eq!(plan.counts().identical, 3);
    assert_eq!(plan.counts().collections, 0);
    let before = source.snippets();
    let report = source.commit(plan);
    assert_eq!((report.inserted, report.skipped), (0, 3));
    assert_eq!(report.collections_created, 0);
    assert_eq!(source.snippets(), before);

    let fresh = library(3);
    let plan = fresh
        .preview(&path, None, ImportFormat::Bundle, false)
        .unwrap();
    let report = fresh.commit(plan);
    assert_eq!((report.inserted, report.collections_created), (3, 1));
    assert_eq!(fresh.snippets(), before, "templates and bodies survive");
}

#[test]
fn a_colliding_abbreviation_is_dropped_and_flagged_but_the_content_is_kept() {
    let target = library(1);
    target.save("Mine", Some("sig"), "my signature", false);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("foreign.json");
    std::fs::write(
        &path,
        r#"[{"name":"Theirs","text":"their signature","keyword":"SIG"}]"#,
    )
    .unwrap();

    let plan = target
        .preview(&path, None, ImportFormat::Json, false)
        .unwrap();
    let rows = plan.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].outcome, ImportRowOutcome::AbbreviationConflict);
    assert_eq!(
        rows[0].reason.as_deref(),
        Some(import::REASON_ABBREVIATION_TAKEN)
    );
    let report = target.commit(plan);
    assert_eq!((report.inserted, report.without_abbreviation), (1, 1));
    let theirs = target
        .snippets()
        .into_iter()
        .find(|(title, ..)| title == "Theirs")
        .unwrap();
    assert_eq!((theirs.1, theirs.2.as_str()), (None, "their signature"));
}

#[test]
fn an_invalid_row_is_skipped_and_a_failure_midway_writes_nothing() {
    let target = library(1);
    let mut rows: Vec<String> = (0..100)
        .map(|index| format!(r#"{{"name":"Row {index}","text":"body {index}"}}"#))
        .collect();
    rows[42] = r#"{"name":"Empty","text":"   "}"#.to_owned();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rows.json");
    std::fs::write(&path, format!("[{}]", rows.join(","))).unwrap();

    let plan = target
        .preview(&path, None, ImportFormat::Json, false)
        .unwrap();
    assert_eq!((plan.counts().new, plan.counts().invalid), (99, 1));
    let invalid = plan.rows();
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].index, 42);
    assert_eq!(
        invalid[0].reason.as_deref(),
        Some(import::REASON_EMPTY_BODY)
    );

    let rows_seen = Cell::new(0);
    let stop_midway = || {
        rows_seen.set(rows_seen.get() + 1);
        rows_seen.get() > 50
    };
    let failed = import::commit(
        &target.store,
        &target.keys.item_key(),
        plan,
        ImportPolicies::default(),
        &stop_midway,
    );
    assert!(matches!(failed, Err(TransferError::Cancelled)));
    assert_eq!(target.store.snippet_count().unwrap(), 0);

    let plan = target
        .preview(&path, None, ImportFormat::Json, false)
        .unwrap();
    let report = target.commit(plan);
    assert_eq!((report.inserted, report.skipped), (99, 1));
    assert_eq!(target.store.snippet_count().unwrap(), 99);
}

#[test]
fn foreign_snippets_arrive_literal_unless_known_placeholders_are_converted() {
    let target = library(1);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("foreign.csv");
    std::fs::write(
        &path,
        "title,keyword,text\nDated,d1,\"Today {date}, copied {clipboard} {argument name=x}\"\n",
    )
    .unwrap();

    let literal = target
        .preview(&path, None, ImportFormat::Csv, false)
        .unwrap();
    assert!(literal.rows().is_empty());
    let converted = target
        .preview(&path, None, ImportFormat::Csv, true)
        .unwrap();
    assert_eq!(
        converted.rows()[0].reason.as_deref(),
        Some("unrecognized_placeholders:{argument name=x}")
    );
    let report = target.commit(converted);
    assert_eq!(report.unrecognized_placeholders, 1);
    assert_eq!(
        target.snippets(),
        vec![(
            "Dated".to_owned(),
            Some("d1".to_owned()),
            "Today {{date}}, copied {{clipboard}} {argument name=x}".to_owned(),
            true,
        )]
    );

    let json = dir.path().join("foreign.json");
    std::fs::write(
        &json,
        r#"[{"name":"Raw","text":"Keep {{braces}} and {date}"}]"#,
    )
    .unwrap();
    let plan = target
        .preview(&json, None, ImportFormat::Json, false)
        .unwrap();
    target.commit(plan);
    let raw = target
        .snippets()
        .into_iter()
        .find(|(title, ..)| title == "Raw")
        .unwrap();
    assert_eq!(
        (raw.2.as_str(), raw.3),
        ("Keep {{braces}} and {date}", false)
    );
}

#[test]
fn an_unencrypted_export_leaves_out_concealed_snippets_unless_confirmed() {
    let source = library(1);
    source.save("Open", None, "visible", false);
    source.save("Hidden", None, "concealed body", true);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.json");

    let written = export::export(&source.store, &path, &ExportScope::All, None, false).unwrap();
    assert_eq!(written, 1);
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("concealed body"));
    let written = export::export(&source.store, &path, &ExportScope::All, None, true).unwrap();
    assert_eq!(written, 2);
}
