use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use copypaste_cloud::sync::{Applied, CloudSource, LocalItem, PullCursor};
use copypaste_cloud::{
    CloudConfig, CloudCrypto, CloudItem, CloudSync, RealtimeEvent, RealtimeSubscription,
    SupabaseAuth, SupabaseRest,
};

fn secret(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is required"))
}

struct Source {
    items: Vec<LocalItem>,
    cursor: Mutex<PullCursor>,
}

impl CloudSource for Source {
    fn device_id(&self) -> String {
        "integration-device".into()
    }

    fn local_changes_since(
        &self,
        since_ms: i64,
    ) -> Result<Vec<LocalItem>, copypaste_cloud::SyncError> {
        Ok(self
            .items
            .iter()
            .filter(|item| item.created_at >= since_ms)
            .cloned()
            .collect())
    }

    fn apply_remote(&self, item: LocalItem) -> Result<Applied, copypaste_cloud::SyncError> {
        Ok(Applied::Declined(item))
    }

    fn pull_cursor(&self) -> Result<PullCursor, copypaste_cloud::SyncError> {
        Ok(self.cursor.lock().expect("cursor lock").clone())
    }

    fn set_pull_cursor(&self, cursor: &PullCursor) -> Result<(), copypaste_cloud::SyncError> {
        *self.cursor.lock().expect("cursor lock") = cursor.clone();
        Ok(())
    }
}

fn config() -> CloudConfig {
    CloudConfig::new_loopback(
        std::env::var("SUPABASE_URL").expect("SUPABASE_URL"),
        std::env::var("SUPABASE_ANON_KEY").expect("SUPABASE_ANON_KEY"),
    )
    .expect("loopback SUPABASE_URL")
}

fn stamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64
}

#[tokio::test]
#[ignore = "requires the disposable local Supabase stack"]
async fn real_supabase_contract() {
    let config = config();
    let password = secret("SUPABASE_TEST_PASSWORD");
    let passphrase = secret("SUPABASE_TEST_PASSPHRASE");
    let auth = SupabaseAuth::new(config.clone());
    let alice = auth
        .sign_in("dev-a@example.test", &password)
        .await
        .expect("Alice signs in through GoTrue");
    let bob = auth
        .sign_in("dev-b@example.test", &password)
        .await
        .expect("Bob signs in through GoTrue");
    let rest = SupabaseRest::new(config.clone());
    let crypto = CloudCrypto::derive(&passphrase, &alice.user_id).expect("derive sync key");
    let created_at = stamp();

    let mut realtime = RealtimeSubscription::connect(&config, &alice.access_token)
        .await
        .expect("join Realtime with the authenticated user filter");
    let (nonce, ciphertext) = crypto
        .seal(b"encrypted convergence", "real-convergence")
        .expect("seal convergence row");
    let mut convergence = CloudItem::from_sealed_b64(
        "real-convergence",
        ciphertext,
        nonce,
        "text",
        created_at,
        "device-a",
    );
    convergence.sign(crypto.key());
    rest.upsert(&alice.access_token, &[convergence])
        .await
        .expect("write encrypted row");

    let event = tokio::time::timeout(std::time::Duration::from_secs(15), realtime.next_event())
        .await
        .expect("Realtime event timeout")
        .expect("Realtime stream ended")
        .expect("Realtime event");
    match event {
        RealtimeEvent::Insert { item_id } | RealtimeEvent::Update { item_id } => {
            assert_eq!(item_id, "real-convergence");
        }
        other => panic!("unexpected Realtime event: {other:?}"),
    }
    let listed = rest
        .list_since(&alice.access_token, 0, None, 10)
        .await
        .expect("Alice's listing")
        .into_iter()
        .find(|row| row.item_id == "real-convergence")
        .expect("the announced row");
    // The server placed the row; the stamp the client wrote did not.
    let placed = listed.position;
    let row = rest
        .fetch_rows(&alice.access_token, &[listed.item_id])
        .await
        .expect("Alice's row")
        .rows
        .pop()
        .expect("the listed row");
    assert_eq!(row.position, Some(placed), "the two requests disagree");
    row.verify(crypto.key())
        .expect("Realtime metadata signature");
    assert_eq!(
        crypto
            .open(&row.ciphertext, &row.nonce, &row.item_id)
            .expect("device B decrypts device A's row")
            .as_slice(),
        b"encrypted convergence"
    );

    assert!(rest
        .list_since(&bob.access_token, 0, None, 200)
        .await
        .expect("Bob's RLS listing")
        .is_empty());
    assert!(rest
        .fetch_rows(&bob.access_token, &["real-convergence".into()])
        .await
        .expect("Bob's RLS fetch")
        .rows
        .is_empty());

    let mut page_rows = Vec::new();
    for index in 0..205 {
        let item_id = format!("real-page-{index:03}");
        let (nonce, ciphertext) = crypto.seal(b"page", &item_id).expect("seal page row");
        let mut row = CloudItem::from_sealed_b64(
            item_id,
            ciphertext,
            nonce,
            "text",
            // Older than the row already read, as a device that was offline
            // would stamp them: the page order must still put them after it.
            created_at - 60_000,
            "device-a",
        );
        row.sign(crypto.key());
        page_rows.push(row);
    }
    rest.upsert(&alice.access_token, &page_rows)
        .await
        .expect("write batches that each share one position");
    let first = rest
        .list_since(&alice.access_token, placed, Some("real-convergence"), 200)
        .await
        .expect("first page");
    assert_eq!(first.len(), 200);
    let last = first.last().expect("first page has a cursor");
    let second = rest
        .list_since(&alice.access_token, last.position, Some(&last.item_id), 200)
        .await
        .expect("second page");
    assert_eq!(second.len(), 5);

    let mut tombstone =
        CloudItem::tombstone("real-convergence", "text", created_at + 2, "device-b");
    tombstone.sign(crypto.key());
    rest.upsert(&alice.access_token, &[tombstone])
        .await
        .expect("write tombstone");
    let after_pages = second.last().expect("second page has a cursor");
    let deleted = rest
        .list_since(
            &alice.access_token,
            after_pages.position,
            Some(&after_pages.item_id),
            10,
        )
        .await
        .expect("list tombstone");
    assert!(deleted
        .iter()
        .any(|row| row.item_id == "real-convergence" && row.deleted));
    let deleted = rest
        .fetch_rows(&alice.access_token, &["real-convergence".into()])
        .await
        .expect("read tombstone");
    assert!(deleted.rows.iter().any(|row| {
        row.item_id == "real-convergence" && row.deleted && row.ciphertext.is_empty()
    }));

    let credential_shaped = LocalItem {
        item_id: "real-credential-shaped".into(),
        content: zeroize::Zeroizing::new(b"AKIAIOSFODNN7EXAMPLE".to_vec()),
        content_type: "text".into(),
        payload_metadata: None,
        source_app_bundle_id: None,
        source_app_name: None,
        created_at: created_at + 3,
        deleted: false,
        origin_device_id: "device-a".into(),
    };
    let source = Source {
        items: vec![credential_shaped],
        cursor: Mutex::default(),
    };
    let sync = CloudSync::new(
        rest.clone(),
        auth,
        copypaste_cloud::derive_sync_key(&passphrase, &alice.user_id).expect("sync key"),
        config,
        alice,
    );
    let stats = sync.push(&source).await.expect("credential-shaped push");
    assert_eq!(stats.uploaded, 1);
    assert!(rest
        .list_since(
            &sync.inspect_session(|session| session.access_token.clone()),
            after_pages.position,
            Some(&after_pages.item_id),
            10,
        )
        .await
        .expect("credential-shaped query")
        .iter()
        .any(|row| row.item_id == "real-credential-shaped"));

    realtime.close().await;
}
