use copypaste_core::{history_actions, history_item::ItemOrigin, ClipboardPayload, Keyring, Store};

#[test]
fn plain_text_copy_converts_the_native_payload_and_preserves_the_original() {
    let keys = Keyring::from_secret(&[73; 32]);
    let store = Store::open_in_memory(&keys.db_key()).unwrap();
    let html = "<p><b>Hello &amp; world</b></p>";
    let row = copypaste_core::ingest::ingest_into(
        &store,
        &keys,
        html,
        "text/html",
        copypaste_core::now_ms(),
        &Default::default(),
    )
    .unwrap()
    .into_item();
    let origin = ItemOrigin {
        device_id: "local".into(),
        device_name: None,
        device_class: Default::default(),
    };
    let prepared = history_actions::prepare_copy(
        &store,
        &keys.item_key(),
        history_actions::open(&store, &keys.item_key(), &row.id).unwrap(),
        history_actions::CopyOptions {
            origin: &origin,
            local_device_id: "local",
            plain_text: true,
            allow_content_uri: false,
            max_file_size_bytes: 1024,
        },
    )
    .unwrap();
    assert_eq!(prepared.content_type, "text");
    assert_eq!(prepared.item.content_type, "text/html");
    assert_eq!(prepared.item.content, html);
    assert_eq!(prepared.payload().plain_text(), Some("Hello & world"));
    history_actions::finish_copy(&store, prepared).unwrap();
    let original = store.get(&row.id).unwrap().unwrap();
    assert_eq!(original.content_type, "text/html");
    let ClipboardPayload::Text(text) = ClipboardPayload::open(&original, &keys.item_key()).unwrap()
    else {
        panic!("text expected");
    };
    assert_eq!(text.as_str(), html);
}
