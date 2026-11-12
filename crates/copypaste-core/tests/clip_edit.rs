use copypaste_core::storage::clip_edit::{ClipEditError, ClipWriter, SOURCE_APP_NAME};
use copypaste_core::{ClipboardPayload, Keyring, Store, StoreSource, StoredItem};
use copypaste_ipc::payload::ProvenanceKind;
use copypaste_ipc::ConfigData;

struct Device {
    store: Store,
    keys: Keyring,
    source: StoreSource,
    settings: ConfigData,
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
        settings: ConfigData::default(),
    }
}

impl Device {
    fn writer(&self) -> ClipWriter<'_, impl Fn() -> ConfigData + '_> {
        ClipWriter {
            store: &self.store,
            keyring: &self.keys,
            settings: &self.settings,
            current_settings: || self.settings.clone(),
        }
    }

    fn capture(&self, text: &str, content_type: &str) -> StoredItem {
        copypaste_core::ingest::ingest_into(
            &self.store,
            &self.keys,
            text,
            content_type,
            copypaste_core::now_ms(),
            &self.settings,
        )
        .unwrap()
        .into_item()
    }

    fn text(&self, id: &str) -> String {
        let row = self.store.get(id).unwrap().expect("live clip");
        match ClipboardPayload::open(&row, &self.keys.item_key()).unwrap() {
            ClipboardPayload::Text(text) => text.as_str().to_owned(),
            _ => panic!("not text"),
        }
    }

    fn provenance(&self, id: &str) -> Option<(ProvenanceKind, Option<String>)> {
        let mut found = self.store.item_annotations(&[id.to_owned()]).unwrap();
        found
            .remove(id)
            .and_then(|annotations| annotations.provenance)
            .map(|provenance| (provenance.kind, provenance.from_clip_id))
    }
}

/// One P2P session in each direction.
fn exchange(a: &Device, b: &Device) {
    use copypaste_p2p::sync::SyncSource;
    for (from, to) in [(a, b), (b, a)] {
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
}

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

#[test]
fn an_edit_adds_a_top_clip_and_leaves_the_original_byte_identical() {
    let device = device(31, "laptop");
    let original = device.capture("first line\r\nsecond", "text");
    pause();
    device.capture("something newer", "text");
    pause();

    let edit = device
        .writer()
        .save_edit(&original.id, "first line\r\nsecond, edited", false)
        .unwrap();

    assert!(!edit.replaced);
    let kept = device.store.get(&original.id).unwrap().unwrap();
    assert_eq!(kept.content_ciphertext, original.content_ciphertext);
    assert_eq!(kept.content_hash, original.content_hash);
    assert_eq!(device.text(&original.id), "first line\r\nsecond");
    assert_eq!(device.text(&edit.item.id), "first line\nsecond, edited");
    assert_eq!(device.store.list(1, 0).unwrap()[0].id, edit.item.id);
    assert_eq!(edit.item.app_name.as_deref(), Some(SOURCE_APP_NAME));
    assert_eq!(
        device.provenance(&edit.item.id),
        Some((ProvenanceKind::Edited, Some(original.id.clone())))
    );
}

#[test]
fn replacing_the_original_removes_it_on_every_synced_device() {
    let (a, b) = (device(32, "laptop"), device(33, "phone"));
    let original = a.capture("draft text", "text");
    exchange(&a, &b);
    assert!(b.store.get(&original.id).unwrap().is_some());
    pause();

    let edit = a
        .writer()
        .save_edit(&original.id, "final text", true)
        .unwrap();
    assert!(edit.replaced);
    exchange(&a, &b);

    for device in [&a, &b] {
        assert!(device.store.get(&original.id).unwrap().is_none());
        assert_eq!(device.text(&edit.item.id), "final text");
        assert_eq!(
            device.provenance(&edit.item.id).map(|(kind, _)| kind),
            Some(ProvenanceKind::Edited)
        );
    }
}

#[test]
fn unchanged_rich_text_keeps_the_original_and_changed_text_is_stored_plain() {
    let device = device(34, "laptop");
    let rich = device.capture("<p><b>Hello</b> world</p>", "text/html");

    let unchanged = device
        .writer()
        .save_edit(&rich.id, "Hello world", true)
        .unwrap();
    assert_eq!(unchanged.item.id, rich.id);
    assert!(!unchanged.replaced);
    assert_eq!(device.store.list(10, 0).unwrap().len(), 1);
    assert_eq!(device.text(&rich.id), "<p><b>Hello</b> world</p>");

    let changed = device
        .writer()
        .save_edit(&rich.id, "Hello there", false)
        .unwrap();
    assert_ne!(changed.item.id, rich.id);
    assert_eq!(changed.item.content_type, "text");
    assert_eq!(device.text(&changed.item.id), "Hello there");
}

#[test]
fn manual_clips_are_attributed_to_copypaste_and_large_clips_are_read_only() {
    let device = device(35, "laptop");
    let manual = device.writer().create_manual("typed by hand").unwrap();
    assert_eq!(manual.app_name.as_deref(), Some(SOURCE_APP_NAME));
    assert_eq!(
        device.provenance(&manual.id),
        Some((ProvenanceKind::Manual, None))
    );
    assert!(matches!(
        device.writer().create_manual(" \n "),
        Err(ClipEditError::Empty)
    ));

    let large = device.capture(&"x".repeat(256 * 1024 + 1), "text");
    assert!(matches!(
        device.writer().save_edit(&large.id, "short", false),
        Err(ClipEditError::TooLarge)
    ));
}

#[test]
fn save_as_snippet_copies_the_text_with_template_off_and_keeps_the_clip() {
    let device = device(36, "laptop");
    let clip = device.capture("Hi {{name}},\r\nthanks", "text");

    let detail = device
        .store
        .save_clip_as_snippet(&device.keys.item_key(), &clip.id, None)
        .unwrap();

    assert_eq!(detail.body_text, "Hi {{name}},\nthanks");
    assert!(!detail.snippet.template);
    assert!(!detail.snippet.concealed);
    let kept = device.store.get(&clip.id).unwrap().unwrap();
    assert_eq!(kept.content_hash, clip.content_hash);
    assert_eq!(device.text(&clip.id), "Hi {{name}},\r\nthanks");
}
