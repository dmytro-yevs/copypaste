use copypaste_core::snippets::ranking::SearchTier;
use copypaste_core::storage::snippets::{
    CollectionDraft, SnippetDetail, SnippetDraft, SnippetError, SnippetScope,
};
use copypaste_core::sync::open_version_bytes;
use copypaste_core::{Keyring, Store, StoreSource};
use copypaste_ipc::ConfigData;

struct Device {
    store: Store,
    keys: Keyring,
    source: StoreSource,
}

fn device(secret: u8, name: &str) -> Device {
    let keys = Keyring::from_secret(&[secret; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let id = store.device_identity(name).unwrap().device_id;
    let source = StoreSource::new(
        store.clone(),
        std::sync::Arc::new(Keyring::from_secret(&[secret; 32])),
        id,
        name.into(),
        ConfigData::default(),
    );
    Device {
        store,
        keys,
        source,
    }
}

/// One P2P session in each direction.
fn exchange(a: &Device, b: &Device) {
    session(a, b);
    session(b, a);
}

fn session(from: &Device, to: &Device) {
    use copypaste_p2p::sync::SyncSource;
    let ids = from
        .source
        .summaries(0)
        .unwrap()
        .into_iter()
        .map(|summary| summary.item_id)
        .collect::<Vec<_>>();
    to.source
        .apply_batch(from.source.fetch(&ids).unwrap())
        .unwrap();
}

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

fn draft(title: &str, abbreviation: Option<&str>, text: &str) -> SnippetDraft {
    SnippetDraft {
        title: title.into(),
        abbreviation: abbreviation.map(Into::into),
        body_text: text.into(),
        ..Default::default()
    }
}

fn create(device: &Device, draft: SnippetDraft) -> String {
    device
        .store
        .save_snippet(&device.keys.item_key(), &draft)
        .unwrap()
        .snippet
        .id
}

fn get(device: &Device, id: &str) -> Option<SnippetDetail> {
    device.store.snippet(id).unwrap()
}

fn edit(device: &Device, id: &str, text: &str) {
    let current = get(device, id).unwrap();
    let snippet = current.snippet;
    device
        .store
        .save_snippet(
            &device.keys.item_key(),
            &SnippetDraft {
                id: Some(snippet.id),
                collection_id: snippet.collection_id,
                title: snippet.title,
                abbreviation: snippet.abbreviation,
                body_text: text.into(),
                template: snippet.template,
                pinned: snippet.pinned,
                concealed: snippet.concealed,
                base_revision: current.revision,
                ..Default::default()
            },
        )
        .unwrap();
}

fn listed(device: &Device, scope: SnippetScope) -> Vec<(String, String)> {
    device
        .store
        .snippet_page(&scope, None, 100)
        .unwrap()
        .snippets
        .into_iter()
        .map(|snippet| (snippet.id, snippet.title))
        .collect()
}

fn bodies(device: &Device) -> Vec<String> {
    let mut bodies: Vec<String> = listed(device, SnippetScope::All)
        .into_iter()
        .map(|(id, _)| get(device, &id).unwrap().body_text)
        .collect();
    bodies.sort();
    bodies
}

#[test]
fn abbreviations_are_unique_after_nfkc_case_folding_but_prefixes_coexist() {
    let a = device(1, "Alpha");
    let sig = create(&a, draft("Signature", Some("Sig"), "Kind regards"));
    let taken = a
        .store
        .save_snippet(&a.keys.item_key(), &draft("Other", Some("ｓｉｇ"), "x"))
        .unwrap_err();
    assert!(matches!(
        taken,
        SnippetError::AbbreviationTaken { snippet_id, title } if snippet_id == sig && title == "Signature"
    ));

    let sig2 = create(&a, draft("Long signature", Some("sig2"), "Best"));
    assert!(a
        .store
        .duplicate_snippet_abbreviations()
        .unwrap()
        .is_empty());
    let hits = a.store.quick_search_snippets("sig", 10).unwrap();
    let ranked: Vec<_> = hits
        .iter()
        .map(|hit| (hit.snippet.id.clone(), hit.tier, hit.snippet.duplicate))
        .collect();
    assert_eq!(
        ranked[..2],
        [
            (sig, SearchTier::ExactAbbreviation, false),
            (sig2, SearchTier::AbbreviationPrefix, false)
        ]
    );
}

#[test]
fn concurrent_body_edits_converge_to_the_winner_and_one_conflict_copy() {
    let a = device(2, "Alpha");
    let b = device(3, "Beta");
    let work = a
        .store
        .save_snippet_collection(
            &a.keys.item_key(),
            &CollectionDraft {
                name: "Work".into(),
                icon: "briefcase".into(),
                accent: "blue".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    let id = create(
        &a,
        SnippetDraft {
            collection_id: Some(work.clone()),
            ..draft("Address", Some("addr"), "1 Main Street")
        },
    );
    exchange(&a, &b);

    edit(&a, &id, "2 Main Street");
    pause();
    edit(&b, &id, "3 Main Street");
    exchange(&a, &b);
    exchange(&a, &b);

    for device in [&a, &b] {
        assert_eq!(bodies(device), ["2 Main Street", "3 Main Street"]);
        assert_eq!(get(device, &id).unwrap().body_text, "3 Main Street");
        let (copy, title) = listed(device, SnippetScope::All)
            .into_iter()
            .find(|(copy, _)| *copy != id)
            .unwrap();
        assert!(title.starts_with("Address (conflict, Alpha, "), "{title}");
        let copy = get(device, &copy).unwrap().snippet;
        assert_eq!(copy.abbreviation, None);
        assert_eq!(copy.collection_id.as_deref(), Some(work.as_str()));
    }
}

#[test]
fn identical_edits_and_offline_fast_forwards_make_no_conflict_copy() {
    let a = device(4, "Alpha");
    let b = device(5, "Beta");
    let id = create(&a, draft("Note", None, "A1"));
    exchange(&a, &b);

    edit(&a, &id, "same");
    pause();
    edit(&b, &id, "same");
    exchange(&a, &b);

    for text in ["A2", "A3", "A4"] {
        edit(&a, &id, text);
    }
    exchange(&a, &b);
    for device in [&a, &b] {
        assert_eq!(bodies(device), ["A4"]);
    }
}

#[test]
fn an_edit_after_a_remote_delete_restores_and_a_later_delete_wins() {
    let a = device(6, "Alpha");
    let b = device(7, "Beta");
    let id = create(&a, draft("Greeting", None, "Hello"));
    exchange(&a, &b);

    b.store
        .delete_snippets(&b.keys.item_key(), std::slice::from_ref(&id))
        .unwrap();
    pause();
    edit(&a, &id, "Hello again");
    exchange(&a, &b);
    for device in [&a, &b] {
        assert_eq!(listed(device, SnippetScope::All).len(), 1);
        assert_eq!(get(device, &id).unwrap().body_text, "Hello again");
    }

    pause();
    b.store
        .delete_snippets(&b.keys.item_key(), std::slice::from_ref(&id))
        .unwrap();
    exchange(&a, &b);
    for device in [&a, &b] {
        assert!(listed(device, SnippetScope::All).is_empty());
        assert_eq!(listed(device, SnippetScope::RecentlyDeleted).len(), 1);
    }
}

#[test]
fn a_purge_erases_an_edit_that_raced_it_on_every_device() {
    let a = device(8, "Alpha");
    let b = device(9, "Beta");
    let id = create(&a, draft("Card", Some("card"), "4111 1111"));
    exchange(&a, &b);

    a.store
        .purge_snippets(&a.keys.item_key(), std::slice::from_ref(&id))
        .unwrap();
    pause();
    edit(&b, &id, "late edit 4222");
    exchange(&b, &a);
    exchange(&a, &b);

    for device in [&a, &b] {
        assert!(get(device, &id).is_none());
        assert!(device
            .store
            .quick_search_snippets("card", 10)
            .unwrap()
            .is_empty());
        for row in device.store.versions_since(0, 1000).unwrap() {
            let text = open_version_bytes(&device.keys, &row).unwrap();
            let text = String::from_utf8_lossy(&text);
            assert!(!text.contains("4222") && !text.contains("4111"), "{text}");
        }
    }
}

#[test]
fn a_snippet_filed_into_a_collection_deleted_elsewhere_is_unfiled() {
    let a = device(10, "Alpha");
    let b = device(11, "Beta");
    let work = a
        .store
        .save_snippet_collection(
            &a.keys.item_key(),
            &CollectionDraft {
                name: "Work".into(),
                icon: "briefcase".into(),
                accent: "blue".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    let kept = create(
        &a,
        SnippetDraft {
            collection_id: Some(work.clone()),
            ..draft("Kept", None, "kept")
        },
    );
    exchange(&a, &b);

    a.store
        .delete_snippet_collection(&a.keys.item_key(), &work, false)
        .unwrap();
    let added = create(
        &b,
        SnippetDraft {
            collection_id: Some(work.clone()),
            ..draft("Added", None, "added")
        },
    );
    exchange(&a, &b);

    for device in [&a, &b] {
        assert!(device.store.snippet_collections().unwrap().is_empty());
        let mut unfiled: Vec<String> = listed(device, SnippetScope::Unfiled)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        unfiled.sort();
        let mut expected = vec![kept.clone(), added.clone()];
        expected.sort();
        assert_eq!(unfiled, expected);
    }
}

#[test]
fn offline_duplicate_abbreviations_are_kept_and_both_marked() {
    let a = device(12, "Alpha");
    let b = device(13, "Beta");
    let home = create(&a, draft("Home", Some("addr"), "1 Main Street"));
    let office = create(&b, draft("Office", Some("Addr"), "9 Side Road"));
    exchange(&a, &b);

    let mut pair = vec![home, office];
    pair.sort();
    for device in [&a, &b] {
        assert_eq!(
            device.store.duplicate_snippet_abbreviations().unwrap(),
            [pair.clone()]
        );
        let hits = device.store.quick_search_snippets("addr", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits
            .iter()
            .all(|hit| hit.tier == SearchTier::ExactAbbreviation && hit.snippet.duplicate));
    }
}

#[test]
fn renaming_an_abbreviation_rewrites_its_references_in_one_step() {
    let a = device(14, "Alpha");
    let sig = create(&a, draft("Signature", Some("sig"), "Kind regards"));
    let reference = "{{snippet abbr=sig}}";
    let users: Vec<String> = ["Letter", "Memo"]
        .into_iter()
        .map(|title| {
            create(
                &a,
                SnippetDraft {
                    template: true,
                    ..draft(title, None, &format!("Body\n{reference}"))
                },
            )
        })
        .collect();
    create(&a, draft("Plain", None, "sig is not a reference"));

    let refers = |text: &str| text.contains(reference);
    let mut found = a.store.snippet_references(&sig, &refers).unwrap();
    found.sort();
    let mut expected = users.clone();
    expected.sort();
    assert_eq!(found, expected);

    let rewrite =
        |text: &str| refers(text).then(|| text.replace(reference, "{{snippet abbr=sign}}"));
    let rewritten = a
        .store
        .rename_snippet_abbreviation(&a.keys.item_key(), &sig, Some("sign"), Some(&rewrite))
        .unwrap();
    assert_eq!(rewritten, 2);
    for id in users {
        assert_eq!(
            get(&a, &id).unwrap().body_text,
            "Body\n{{snippet abbr=sign}}"
        );
    }
    assert_eq!(
        get(&a, &sig).unwrap().snippet.abbreviation.as_deref(),
        Some("sign")
    );
}

#[test]
fn clearing_history_keeps_snippets() {
    let a = device(15, "Alpha");
    let id = create(&a, draft("Kept", None, "kept"));
    a.store.delete_all().unwrap();
    assert_eq!(get(&a, &id).unwrap().body_text, "kept");
}
