use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use copypaste_cloud::sync::{Applied, CloudSource, LocalItem, SensitiveGuard};
use copypaste_cloud::{
    CloudConfig, CloudCrypto, CloudItem, CloudSync, RealtimeEvent, RealtimeSubscription,
    SupabaseAuth, SupabaseRest,
};

const PASSWORD: &str = "copypaste-dev";
const PASSPHRASE: &str = "correct horse battery staple";

struct Source {
    items: Vec<LocalItem>,
    watermark: Mutex<i64>,
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

    fn watermark(&self) -> Result<i64, copypaste_cloud::SyncError> {
        Ok(*self.watermark.lock().expect("watermark lock"))
    }

    fn set_watermark(&self, ms: i64) -> Result<(), copypaste_cloud::SyncError> {
        *self.watermark.lock().expect("watermark lock") = ms;
        Ok(())
    }
}

fn config() -> CloudConfig {
    CloudConfig {
        url: std::env::var("SUPABASE_URL").expect("SUPABASE_URL"),
        anon_key: std::env::var("SUPABASE_ANON_KEY").expect("SUPABASE_ANON_KEY"),
    }
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
    let auth = SupabaseAuth::new(config.clone());
    let alice = auth
        .sign_in("dev-a@example.test", PASSWORD)
        .await
        .expect("Alice signs in through GoTrue");
    let bob = auth
        .sign_in("dev-b@example.test", PASSWORD)
        .await
        .expect("Bob signs in through GoTrue");
    let rest = SupabaseRest::new(config.clone());
    let crypto = CloudCrypto::derive(PASSPHRASE, &alice.user_id).expect("derive sync key");
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
    let row = match event {
        RealtimeEvent::Insert(row) | RealtimeEvent::Update(row) => row,
        other => panic!("unexpected Realtime event: {other:?}"),
    };
    row.verify(crypto.key())
        .expect("Realtime metadata signature");
    assert_eq!(
        crypto
            .open(&row.ciphertext, &row.nonce, &row.item_id)
            .expect("device B decrypts device A's row"),
        b"encrypted convergence"
    );

    assert!(rest
        .fetch_since(&bob.access_token, created_at, None, 200)
        .await
        .expect("Bob's RLS page")
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
            created_at + 1,
            "device-a",
        );
        row.sign(crypto.key());
        page_rows.push(row);
    }
    rest.upsert(&alice.access_token, &page_rows)
        .await
        .expect("write a multi-page timestamp bucket");
    let first = rest
        .fetch_since(&alice.access_token, created_at + 1, None, 200)
        .await
        .expect("first page");
    assert_eq!(first.len(), 200);
    let last = first.last().expect("first page has a cursor");
    let second = rest
        .fetch_since(
            &alice.access_token,
            last.created_at,
            Some(&last.item_id),
            200,
        )
        .await
        .expect("second page");
    assert_eq!(second.len(), 5);

    let mut tombstone =
        CloudItem::tombstone("real-convergence", "text", created_at + 2, "device-b");
    tombstone.sign(crypto.key());
    rest.upsert(&alice.access_token, &[tombstone])
        .await
        .expect("write tombstone");
    let deleted = rest
        .fetch_since(&alice.access_token, created_at + 2, None, 10)
        .await
        .expect("read tombstone");
    assert!(deleted.iter().any(|row| {
        row.item_id == "real-convergence" && row.deleted && row.ciphertext.is_empty()
    }));

    let sensitive = LocalItem {
        item_id: "real-sensitive".into(),
        content: b"must never leave this device".to_vec(),
        content_type: "text".into(),
        payload_metadata: None,
        created_at: created_at + 3,
        deleted: false,
        origin_device_id: "device-a".into(),
    };
    let source = Source {
        items: vec![sensitive],
        watermark: Mutex::new(0),
    };
    let sync = CloudSync::new(
        rest.clone(),
        auth,
        copypaste_cloud::derive_sync_key(PASSPHRASE, &alice.user_id).expect("sync key"),
        config,
        alice,
        SensitiveGuard::new(|item| item.item_id == "real-sensitive"),
    );
    let stats = sync.push(&source).await.expect("sensitive-only push");
    assert_eq!(stats.skipped_sensitive, 1);
    assert_eq!(stats.uploaded, 0);
    assert!(!rest
        .fetch_since(
            &sync.inspect_session(|session| session.access_token.clone()),
            created_at + 3,
            None,
            10,
        )
        .await
        .expect("sensitive refusal query")
        .iter()
        .any(|row| row.item_id == "real-sensitive"));

    realtime.close().await;
}
