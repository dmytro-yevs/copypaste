use std::collections::BTreeSet;

use copypaste_ipc::Method;
use serde_json::{json, Value};

fn wire_name(method: &Method) -> &'static str {
    match method {
        Method::Status => "status",
        Method::List { .. } => "list",
        Method::Search { .. } => "search",
        Method::Copy { .. } => "copy",
        Method::CopyPlainText { .. } => "copy_plain_text",
        Method::Get { .. } => "get",
        Method::ImagePreview { .. } => "image_preview",
        Method::Add { .. } => "add",
        Method::Delete { .. } => "delete",
        Method::DeleteAll => "delete_all",
        Method::Pin { .. } => "pin",
        Method::ReorderPinned { .. } => "reorder_pinned",
        Method::PairCreate { .. } => "pair_create",
        Method::PairAccept { .. } => "pair_accept",
        Method::Unpair { .. } => "unpair",
        Method::Revoke { .. } => "revoke",
        Method::Peers => "peers",
        Method::SyncNow { .. } => "sync_now",
        Method::Discovered => "discovered",
        Method::Rescan => "rescan",
        Method::Export { .. } => "export",
        Method::Import { .. } => "import",
        Method::Backup { .. } => "backup",
        Method::Restore { .. } => "restore",
        Method::CloudSignIn { .. } => "cloud_sign_in",
        Method::CloudSignOut => "cloud_sign_out",
        Method::CloudStatus => "cloud_status",
        Method::CloudSyncNow => "cloud_sync_now",
        Method::GetConfig => "get_config",
        Method::SetConfig { .. } => "set_config",
        Method::SetPrivateMode { .. } => "set_private_mode",
        Method::GetPrivateMode => "get_private_mode",
        Method::Watch => "watch",
        Method::Shutdown => "shutdown",
    }
}

fn catalog() -> Vec<Value> {
    vec![
        json!({"method":"status"}),
        json!({"method":"list","params":{"limit":10,"cursor":null}}),
        json!({"method":"search","params":{"query":"needle","limit":10}}),
        json!({"method":"copy","params":{"id":"item"}}),
        json!({"method":"copy_plain_text","params":{"id":"item"}}),
        json!({"method":"get","params":{"id":"item"}}),
        json!({"method":"image_preview","params":{"id":"item"}}),
        json!({"method":"add","params":{"content":"text"}}),
        json!({"method":"delete","params":{"id":"item"}}),
        json!({"method":"delete_all"}),
        json!({"method":"pin","params":{"id":"item","pinned":true}}),
        json!({"method":"reorder_pinned","params":{"ids":["item"]}}),
        json!({"method":"pair_create","params":{"name":"device"}}),
        json!({"method":"pair_accept","params":{"code":"code","addr":"127.0.0.1:1"}}),
        json!({"method":"unpair","params":{"pairing_id":"peer"}}),
        json!({"method":"revoke","params":{"pairing_id":"peer"}}),
        json!({"method":"peers"}),
        json!({"method":"sync_now","params":{"pairing_id":null}}),
        json!({"method":"discovered"}),
        json!({"method":"rescan"}),
        json!({"method":"export","params":{"limit":0,"include_sensitive":false}}),
        json!({"method":"import","params":{"items":[]}}),
        json!({"method":"backup","params":{"dest_path":"backup.db"}}),
        json!({"method":"restore","params":{"src_path":"backup.db","confirm":true}}),
        json!({"method":"cloud_sign_in","params":{"email":"a@example.com","password":"secret","passphrase":"phrase"}}),
        json!({"method":"cloud_sign_out"}),
        json!({"method":"cloud_status"}),
        json!({"method":"cloud_sync_now"}),
        json!({"method":"get_config"}),
        json!({"method":"set_config","params":{"patch":{}}}),
        json!({"method":"set_private_mode","params":{"enabled":true}}),
        json!({"method":"get_private_mode"}),
        json!({"method":"watch"}),
        json!({"method":"shutdown"}),
    ]
}

#[test]
fn every_ipc_method_has_one_executable_wire_contract() {
    let mut names = BTreeSet::new();
    for fixture in catalog() {
        let method: Method = serde_json::from_value(fixture.clone()).unwrap();
        let name = wire_name(&method);
        assert!(names.insert(name), "duplicate contract for {name}");
        assert_eq!(fixture["method"], name);

        let encoded = serde_json::to_value(&method).unwrap();
        assert_eq!(encoded, fixture, "wire contract drifted for {name}");
    }
    assert_eq!(names.len(), 34);
}

#[test]
fn every_parameterized_method_rejects_a_missing_or_malformed_payload() {
    for fixture in catalog() {
        if fixture.get("params").is_none() {
            continue;
        }
        let name = fixture["method"].as_str().unwrap();
        assert!(
            serde_json::from_value::<Method>(json!({"method":name})).is_err(),
            "{name} accepted missing params"
        );
        assert!(
            serde_json::from_value::<Method>(json!({"method":name,"params":[]})).is_err(),
            "{name} accepted a non-object params value"
        );
    }
}

#[test]
fn unknown_methods_are_rejected_by_the_shared_contract() {
    assert!(serde_json::from_value::<Method>(json!({"method":"future_method"})).is_err());
}
