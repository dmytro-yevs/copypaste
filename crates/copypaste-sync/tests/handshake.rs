//! Handshake state machine tests for the sync protocol.
//!
//! Drives `SyncEngine::run_session` against a **mock peer** implemented on
//! the other end of a `tokio::io::duplex` pair. The mock peer writes raw
//! length-prefixed JSON frames using `Message::encode()` and reads frames
//! with a manual length-prefix parser. This exercises the engine's
//! HELLO/HAVE/WANT/ITEMS/DONE state machine end-to-end without going
//! through TCP/TLS.
//!
//! Coverage (beta-bonus):
//!
//! 1. `happy_path_hello_have_want_items_done` — full sync where the engine
//!    has one item the mock peer wants and receives nothing back. Verifies
//!    every state transition fires in order.
//! 2. `both_sides_have_identical_state_yields_done_immediately` — when
//!    HAVE sets are identical the WANT lists are empty and ITEMS frames are
//!    empty payloads, then DONE flows naturally.
//! 3. `one_side_has_more_items_other_requests_via_want_then_receives_items`
//!    — mock peer has an item the engine lacks; engine must request it via
//!    WANT and accept it from the peer's ITEMS frame.
//! 4. `protocol_violation_unexpected_message_in_state_rejects_with_clear_error`
//!    — peer sends ITEMS where engine expects HAVE; engine MUST return
//!    `SyncError::ProtocolViolation` with a message naming the expected type.
//! 5. `timeout_on_missing_response_after_5s_aborts_session` — peer completes
//!    HELLO then hangs; engine wrapped in `tokio::time::timeout(5s)` must
//!    abort cleanly. Uses `#[tokio::test(start_paused = true)]` so the
//!    5-second wait is virtual (no real sleep).

use copypaste_core::storage::items::ClipboardItem;
use copypaste_sync::{Message, SyncEngine, SyncError, WireItem};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

// ---------------------------------------------------------------------------
// Mock-peer helpers — write/read framed messages on the raw duplex stream.
// ---------------------------------------------------------------------------

/// Send a `Message` as a length-prefixed JSON frame to the engine.
async fn peer_send(stream: &mut DuplexStream, msg: &Message) {
    let frame = msg.encode().expect("encode never fails on valid Message");
    stream
        .write_all(&frame)
        .await
        .expect("mock peer must be able to write to duplex");
}

/// Receive one length-prefixed JSON frame from the engine, decode it.
async fn peer_recv(stream: &mut DuplexStream) -> Message {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .expect("mock peer must read length prefix");
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream
        .read_exact(&mut payload)
        .await
        .expect("mock peer must read full payload");
    Message::decode(&payload).expect("engine must emit valid JSON frames")
}

fn make_item(id: &str, lamport: i64) -> ClipboardItem {
    ClipboardItem {
        id: id.to_string(),
        item_id: format!("{id}-item"),
        content_type: "text".to_string(),
        content: Some(vec![0xAA, 0xBB]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts: lamport,
        wall_time: 1_700_000_000_000 + lamport,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        origin_device_id: format!("dev-{id}"),
        key_version: 1,
        pinned: false,
    }
}

fn wire_from_local(item: &ClipboardItem, origin: &str) -> WireItem {
    WireItem {
        id: item.id.clone(),
        item_id: item.item_id.clone(),
        content_type: item.content_type.clone(),
        content: item.content.clone(),
        content_nonce: item.content_nonce.clone(),
        blob_ref: item.blob_ref.clone(),
        is_sensitive: item.is_sensitive,
        lamport_ts: item.lamport_ts,
        wall_time: item.wall_time,
        expires_at: item.expires_at,
        app_bundle_id: item.app_bundle_id.clone(),
        origin_device_id: origin.to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. Happy path — every state transition fires in order.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_hello_have_want_items_done() {
    let local = make_item("item-engine", 3);
    let mut engine = SyncEngine::new("device-engine");

    let (engine_side, mut peer_side) = tokio::io::duplex(64 * 1024);

    // Mock peer: declares zero items, requests the engine's one item,
    // sends empty ITEMS back, then DONE.
    let peer_task = tokio::spawn(async move {
        // Receive engine HELLO, reply with our HELLO.
        let hello = peer_recv(&mut peer_side).await;
        assert!(
            matches!(hello, Message::Hello { .. }),
            "expected HELLO first"
        );
        peer_send(
            &mut peer_side,
            &Message::Hello {
                device_id: "device-peer".to_string(),
                clock: 5,
                item_count: 0,
            },
        )
        .await;

        // Receive engine HAVE (should list item-engine), reply with empty HAVE.
        let have = peer_recv(&mut peer_side).await;
        match have {
            Message::Have { items } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].0, "item-engine");
            }
            other => panic!("expected HAVE, got {other:?}"),
        }
        peer_send(&mut peer_side, &Message::Have { items: vec![] }).await;

        // Receive engine WANT (we have nothing → engine wants nothing).
        let want = peer_recv(&mut peer_side).await;
        match want {
            Message::Want { item_ids } => assert!(item_ids.is_empty()),
            other => panic!("expected WANT, got {other:?}"),
        }
        // Tell engine we want item-engine.
        peer_send(
            &mut peer_side,
            &Message::Want {
                item_ids: vec!["item-engine".to_string()],
            },
        )
        .await;

        // Engine sends ITEMS containing item-engine.
        let items = peer_recv(&mut peer_side).await;
        match items {
            Message::Items { items } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].id, "item-engine");
            }
            other => panic!("expected ITEMS, got {other:?}"),
        }
        // Reply with empty ITEMS (we had nothing to give).
        peer_send(&mut peer_side, &Message::Items { items: vec![] }).await;

        // Engine sends DONE → we reply DONE.
        let done = peer_recv(&mut peer_side).await;
        assert_eq!(done, Message::Done);
        peer_send(&mut peer_side, &Message::Done).await;
    });

    let items = [local];
    let (result, to_upsert) = {
        let mut s = engine_side;
        engine
            .run_session(&mut s, &items)
            .await
            .expect("happy path must succeed")
    };

    peer_task.await.expect("mock peer must not panic");

    assert_eq!(result.items_sent, 1, "engine should have sent its one item");
    assert_eq!(result.items_received, 0, "engine should receive nothing");
    assert!(to_upsert.is_empty(), "no items to upsert");
    assert!(
        engine.peer_clocks.contains_key("device-peer"),
        "engine must record peer's clock"
    );
}

// ---------------------------------------------------------------------------
// 2. Identical state — empty WANTs and empty ITEMS both directions.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn both_sides_have_identical_state_yields_done_immediately() {
    let shared = make_item("item-shared", 10);
    let mut engine = SyncEngine::new("device-engine");

    let (engine_side, mut peer_side) = tokio::io::duplex(64 * 1024);
    let shared_for_peer = shared.clone();

    let peer_task = tokio::spawn(async move {
        // HELLO exchange.
        let _engine_hello = peer_recv(&mut peer_side).await;
        peer_send(
            &mut peer_side,
            &Message::Hello {
                device_id: "device-peer".to_string(),
                clock: 10,
                item_count: 1,
            },
        )
        .await;

        // HAVE — peer declares the SAME item with the SAME lamport.
        let _engine_have = peer_recv(&mut peer_side).await;
        peer_send(
            &mut peer_side,
            &Message::Have {
                items: vec![(shared_for_peer.id.clone(), shared_for_peer.lamport_ts)],
            },
        )
        .await;

        // Engine WANT should be empty.
        let want = peer_recv(&mut peer_side).await;
        match want {
            Message::Want { item_ids } => {
                assert!(item_ids.is_empty(), "identical HAVE sets ⇒ empty WANT");
            }
            other => panic!("expected WANT, got {other:?}"),
        }
        // Mock peer WANTs nothing either.
        peer_send(&mut peer_side, &Message::Want { item_ids: vec![] }).await;

        // ITEMS — both sides send empty payloads.
        let items = peer_recv(&mut peer_side).await;
        match items {
            Message::Items { items } => assert!(items.is_empty()),
            other => panic!("expected empty ITEMS, got {other:?}"),
        }
        peer_send(&mut peer_side, &Message::Items { items: vec![] }).await;

        // DONE.
        let done = peer_recv(&mut peer_side).await;
        assert_eq!(done, Message::Done);
        peer_send(&mut peer_side, &Message::Done).await;
    });

    let items = [shared];
    let (result, to_upsert) = {
        let mut s = engine_side;
        engine
            .run_session(&mut s, &items)
            .await
            .expect("identical-state path must succeed")
    };

    peer_task.await.unwrap();

    assert_eq!(result.items_sent, 0);
    assert_eq!(result.items_received, 0);
    assert_eq!(result.items_skipped, 0);
    assert!(to_upsert.is_empty());
}

// ---------------------------------------------------------------------------
// 3. Peer has more items — engine WANTs them and accepts ITEMS frame.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn one_side_has_more_items_other_requests_via_want_then_receives_items() {
    // Engine starts empty; peer has one item that engine must request.
    let mut engine = SyncEngine::new("device-engine");
    let peer_item = make_item("item-from-peer", 42);
    let peer_item_for_clone = peer_item.clone();

    let (engine_side, mut peer_side) = tokio::io::duplex(64 * 1024);

    let peer_task = tokio::spawn(async move {
        // HELLO.
        let _engine_hello = peer_recv(&mut peer_side).await;
        peer_send(
            &mut peer_side,
            &Message::Hello {
                device_id: "device-peer".to_string(),
                clock: 42,
                item_count: 1,
            },
        )
        .await;

        // HAVE — engine declares empty, peer declares item-from-peer.
        let engine_have = peer_recv(&mut peer_side).await;
        match engine_have {
            Message::Have { items } => assert!(items.is_empty()),
            other => panic!("expected empty HAVE, got {other:?}"),
        }
        peer_send(
            &mut peer_side,
            &Message::Have {
                items: vec![(
                    peer_item_for_clone.id.clone(),
                    peer_item_for_clone.lamport_ts,
                )],
            },
        )
        .await;

        // WANT — engine MUST request item-from-peer.
        let engine_want = peer_recv(&mut peer_side).await;
        match engine_want {
            Message::Want { item_ids } => {
                assert_eq!(item_ids, vec!["item-from-peer".to_string()]);
            }
            other => panic!("expected WANT['item-from-peer'], got {other:?}"),
        }
        // Peer WANTs nothing.
        peer_send(&mut peer_side, &Message::Want { item_ids: vec![] }).await;

        // ITEMS — engine sends empty (had nothing), peer sends the requested item.
        let engine_items = peer_recv(&mut peer_side).await;
        match engine_items {
            Message::Items { items } => assert!(items.is_empty()),
            other => panic!("expected empty ITEMS, got {other:?}"),
        }
        peer_send(
            &mut peer_side,
            &Message::Items {
                items: vec![wire_from_local(&peer_item_for_clone, "device-peer")],
            },
        )
        .await;

        // DONE.
        let done = peer_recv(&mut peer_side).await;
        assert_eq!(done, Message::Done);
        peer_send(&mut peer_side, &Message::Done).await;
    });

    let no_items: [ClipboardItem; 0] = [];
    let (result, to_upsert) = {
        let mut s = engine_side;
        engine
            .run_session(&mut s, &no_items)
            .await
            .expect("WANT/ITEMS path must succeed")
    };

    peer_task.await.unwrap();

    assert_eq!(result.items_received, 1);
    assert_eq!(result.items_sent, 0);
    assert_eq!(to_upsert.len(), 1);
    assert_eq!(to_upsert[0].id, "item-from-peer");
    assert_eq!(to_upsert[0].lamport_ts, peer_item.lamport_ts);
    assert!(
        to_upsert[0].is_synced,
        "received items must be flagged synced"
    );
}

// ---------------------------------------------------------------------------
// 4. Protocol violation — out-of-order message rejected with clear error.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn protocol_violation_unexpected_message_in_state_rejects_with_clear_error() {
    // After the engine sends HELLO, the peer sends ITEMS (totally wrong)
    // instead of HELLO. Engine must reject with ProtocolViolation that names
    // the expected message type.
    let mut engine = SyncEngine::new("device-engine");

    let (engine_side, mut peer_side) = tokio::io::duplex(64 * 1024);

    let peer_task = tokio::spawn(async move {
        // Read engine HELLO, then send a wildly wrong message.
        let _engine_hello = peer_recv(&mut peer_side).await;
        peer_send(&mut peer_side, &Message::Items { items: vec![] }).await;
        // Keep the stream alive so engine sees the message, not EOF.
        // The engine should return ProtocolViolation immediately on decode.
        // Drain anything the engine writes before erroring (it shouldn't,
        // but we tolerate it).
        let _ = peer_side.read(&mut [0u8; 1024]).await;
    });

    let no_items: [ClipboardItem; 0] = [];
    let err = {
        let mut s = engine_side;
        engine
            .run_session(&mut s, &no_items)
            .await
            .expect_err("engine must reject out-of-order ITEMS")
    };

    peer_task.await.unwrap();

    match err {
        SyncError::ProtocolViolation(msg) => {
            assert!(
                msg.contains("HELLO"),
                "error must mention the expected HELLO state, got: {msg}"
            );
        }
        other => panic!("expected ProtocolViolation, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 5. Timeout on missing response — virtual-time test, no real sleep.
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn timeout_on_missing_response_after_5s_aborts_session() {
    // Peer completes HELLO then hangs forever. The engine itself does not
    // wrap its reads in a timeout (that's the transport layer's job in
    // production), so the *caller* enforces the deadline via
    // `tokio::time::timeout`. With `start_paused = true` time only advances
    // when there's nothing else to do, so the 5s wait completes instantly.

    let mut engine = SyncEngine::new("device-engine");

    let (engine_side, mut peer_side) = tokio::io::duplex(64 * 1024);

    // Peer task: do HELLO, then hold the stream open forever (do not drop).
    let peer_task = tokio::spawn(async move {
        let _engine_hello = peer_recv(&mut peer_side).await;
        peer_send(
            &mut peer_side,
            &Message::Hello {
                device_id: "device-peer".to_string(),
                clock: 0,
                item_count: 0,
            },
        )
        .await;
        // Read engine's HAVE so the engine's write doesn't backpressure-deadlock
        // before we get a chance to time it out.
        let _engine_have = peer_recv(&mut peer_side).await;
        // Now hang forever — never reply with HAVE.
        let () = std::future::pending().await;
        // Unreachable.
        drop(peer_side);
    });

    let no_items: [ClipboardItem; 0] = [];
    let mut s = engine_side;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        engine.run_session(&mut s, &no_items),
    )
    .await;

    assert!(
        result.is_err(),
        "engine must hit the 5s deadline when peer hangs"
    );

    peer_task.abort();
}
