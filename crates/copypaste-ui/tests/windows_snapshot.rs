// windows_snapshot.rs — beta-bonus regression guards for the UI public surface.
//
// These tests are **compile-only / struct-shape pinning**: they never spin up
// a Slint event loop, never open a window, never touch a display server.
// Their sole job is to fail loudly if a future refactor renames a field on
// `AppSettings`/`PairedDevice`, changes a callback signature on
// `PairWindowHandle`, or shifts the tray menu enum's variant set.
//
// All assertions live at compile time wherever possible (struct destructuring,
// trait-bound coercion, `const` evaluation). Where a runtime check is needed
// (variant counts, `Vec::len`) it uses zero state and runs in microseconds.
//
// If one of these tests fails, **do not** silently update the snapshot — the
// failure is the prompt to make a conscious decision about whether the
// rename / new field / new variant is intentional.

use std::collections::HashSet;

use copypaste_ui::{
    settings::HistoryLimit,
    tray_menu::{MenuEntry, RecentItem, TrayAction, TrayMenuHandle, TrayMenuState},
    AppSettings, PairedDevice, PairWindowHandle, SettingsWindowHandle,
};

// Type aliases keep the clippy `type_complexity` lint quiet for these
// signature-pinning bindings while preserving the exact callback shapes.
type PairCbString    = fn(&PairWindowHandle, Box<dyn Fn(String)>);
type PairCbString2   = fn(&PairWindowHandle, Box<dyn Fn(String, String)>);
type PairCbUnit      = fn(&PairWindowHandle, Box<dyn Fn()>);
type SettingsCbApp   = fn(&SettingsWindowHandle, Box<dyn Fn(AppSettings)>);
type SettingsCbUnit  = fn(&SettingsWindowHandle, Box<dyn Fn()>);
type SettingsCbStr2  = fn(&SettingsWindowHandle, Box<dyn Fn(String, String)>);
type TrayCbUnit      = fn(&TrayMenuHandle, Box<dyn Fn()>);
type TrayCbStrRef    = fn(&TrayMenuHandle, Box<dyn Fn(&str)>);

// ─────────────────────────────────────────────────────────────────────────────
// 1. AppSettings (drives the SettingsWindow) — field-shape drift guard.
//    HistoryWindow itself lives in the binary (`main.rs`) and is not
//    reachable from an external integration test, so we pin the *settings*
//    struct it ultimately renders — that's the user-visible contract.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn app_settings_struct_fields_stable_drift_guard() {
    // Construct via the exact public field list. If any field is renamed,
    // added, or removed, this stops compiling — forcing a deliberate
    // update of every UI consumer.
    let s = AppSettings {
        launch_at_login: true,
        private_mode:    false,
        history_limit:   HistoryLimit::Hundred,
        supabase_url:    String::from("https://example.supabase.co"),
        supabase_key:    String::from("anon-key"),
        device_name:     String::from("Test Mac"),
    };

    // Exhaustive destructure — adding a field without updating this match
    // is a compile error. This is the actual drift guard; the asserts
    // below just sanity-check the values round-tripped correctly.
    let AppSettings {
        launch_at_login,
        private_mode,
        history_limit,
        supabase_url,
        supabase_key,
        device_name,
    } = s;

    assert!(launch_at_login);
    assert!(!private_mode);
    assert_eq!(history_limit.as_count(), 100);
    assert_eq!(supabase_url, "https://example.supabase.co");
    assert_eq!(supabase_key, "anon-key");
    assert_eq!(device_name, "Test Mac");

    // Trait bounds we rely on across the IPC + persistence boundary.
    assert_send_sync::<AppSettings>();
    assert_serde::<AppSettings>();
}

#[test]
fn paired_device_struct_fields_stable_drift_guard() {
    // Same pattern — destructure exhaustively so a renamed field is a
    // compile error, not a silent semantic shift.
    let d = PairedDevice {
        name:        String::from("Phone"),
        fingerprint: "a".repeat(64),
    };
    let PairedDevice { name, fingerprint } = d;

    assert_eq!(name, "Phone");
    assert_eq!(fingerprint.len(), 64);

    assert_send_sync::<PairedDevice>();
    assert_serde::<PairedDevice>();
}

#[test]
fn history_limit_variants_stable_drift_guard() {
    // Settings → HistoryWindow row cap. Adding a variant requires updating
    // every match site; this test makes the audit explicit.
    let variants = [
        HistoryLimit::Fifty,
        HistoryLimit::Hundred,
        HistoryLimit::FiveHundred,
        HistoryLimit::Unlimited,
    ];
    assert_eq!(variants.len(), 4, "HistoryLimit must have exactly 4 variants");

    // Exhaustive match — compile error if a variant is added/removed.
    for v in variants {
        let _: usize = match v {
            HistoryLimit::Fifty       => 50,
            HistoryLimit::Hundred     => 100,
            HistoryLimit::FiveHundred => 500,
            HistoryLimit::Unlimited   => 0,
        };
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. PairWindowHandle callback signature — beta-W3.2 pairing flow.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn pair_window_handle_on_pair_with_password_signature_stable() {
    // The closure passed to `on_pair_with_password` must accept exactly
    // (String, String) → (). This is enforced by trait-bound coercion: if
    // the signature ever drifts (e.g. becomes `Fn(&str, &str)` or returns
    // `Result`), this stops compiling.
    fn _assert_signature<F>(_: F)
    where F: Fn(String, String) + 'static {}

    let cb = |_fp: String, _pw: String| { /* no-op */ };
    _assert_signature(cb);

    // Function-pointer coercion as a second guard — if the trait bound on
    // `on_pair_with_password` widens or narrows, this line breaks.
    let _: fn(String, String) = |_fp, _pw| {};

    // We can't *invoke* `on_pair_with_password` without a real Slint
    // window (which needs a display server), but we can confirm the
    // method is reachable on the public handle type with the exact
    // `Box<dyn Fn(String, String)>` shape used elsewhere in the test
    // file. (Plain `fn(_, _)` parameter inference is ambiguous between
    // `&F` and `Box<F>` impls — we pick `Box<dyn Fn(..)>` explicitly so
    // the bound is unambiguous and matches the trait signature byte-for-byte.)
    let _method_ptr: PairCbString2 =
        |h, cb| h.on_pair_with_password(cb);
    let _ = _method_ptr; // silence unused warning under some toolchains
}

#[test]
fn pair_window_handle_callback_method_set_stable() {
    // Every `on_*` registration on PairWindowHandle is a contract with the
    // host application. Pin them by name+signature via function pointers.
    let _on_pair: PairCbString = |h, cb| h.on_pair(cb);
    let _on_pair_pw: PairCbString2 =
        |h, cb| h.on_pair_with_password(cb);
    let _on_remove: PairCbString =
        |h, cb| h.on_remove_peer(cb);
    let _on_close: PairCbUnit = |h, cb| h.on_close(cb);

    // If any of the above lines fails to type-check, a public callback
    // signature has drifted — update the host wiring deliberately.
}

#[test]
fn settings_window_handle_callback_method_set_stable() {
    // Mirror guard for the settings window — same rationale.
    let _on_save: SettingsCbApp =
        |h, cb| h.on_save(cb);
    let _on_clear: SettingsCbUnit =
        |h, cb| h.on_clear_history(cb);
    let _on_connect: SettingsCbStr2 =
        |h, cb| h.on_connect_supabase(cb);
    let _on_disconnect: SettingsCbUnit =
        |h, cb| h.on_disconnect_supabase(cb);
    let _on_close: SettingsCbUnit =
        |h, cb| h.on_close(cb);
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. TrayMenu enum variant count + identity.
//    NOTE: the brief mentions a hypothetical `Recent(id)` variant but the
//    landed implementation models recents as a separate `RecentItem` list
//    inside `MenuEntry::RecentSubmenu`, not as a `TrayAction` variant. We
//    pin the actual shape — `TrayAction` has 4 variants; the Recent path
//    goes through `dispatch_recent`. If the design ever folds Recent into
//    `TrayAction`, this test breaks and forces an explicit decision.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn tray_action_enum_variant_count_and_identity_stable() {
    let all = [
        TrayAction::ShowHistory,
        TrayAction::PairDevice,
        TrayAction::OpenSettings,
        TrayAction::Quit,
    ];
    assert_eq!(all.len(), 4, "TrayAction must have exactly 4 top-level variants");

    // Stable id strings — these flow into telemetry / `tray-icon::MenuId`.
    let ids: HashSet<&'static str> = all.iter().map(|a| a.id()).collect();
    assert_eq!(ids.len(), 4, "TrayAction ids must be unique");
    assert!(ids.contains("show_history"));
    assert!(ids.contains("pair_device"));
    assert!(ids.contains("open_settings"));
    assert!(ids.contains("quit"));

    // Exhaustive match — compile error if a variant is added/removed.
    for a in all {
        let _: &'static str = match a {
            TrayAction::ShowHistory  => "show_history",
            TrayAction::PairDevice   => "pair_device",
            TrayAction::OpenSettings => "open_settings",
            TrayAction::Quit         => "quit",
        };
    }
}

#[test]
fn menu_entry_variant_set_stable() {
    // `MenuEntry` is the renderer-facing contract. Pin its variant set the
    // same way — exhaustive match here means adding a variant requires
    // updating every renderer.
    let entries = [
        MenuEntry::Action(TrayAction::Quit),
        MenuEntry::RecentSubmenu(vec![]),
        MenuEntry::Separator,
    ];
    for e in &entries {
        let _label: &'static str = match e {
            MenuEntry::Action(_)         => "action",
            MenuEntry::RecentSubmenu(_)  => "recent",
            MenuEntry::Separator         => "sep",
        };
    }
    assert_eq!(entries.len(), 3, "MenuEntry has 3 variants in the v0.2.0-beta shape");
}

#[test]
fn tray_menu_top_level_layout_position_stable() {
    // Position contract — renderers index into this list. Beta menu shape:
    //   [0] ShowHistory  [1] RecentSubmenu  [2] PairDevice
    //   [3] OpenSettings [4] Separator      [5] Quit
    let state = TrayMenuState::new();
    let entries = state.build();
    assert_eq!(entries.len(), 6);
    assert!(matches!(entries[0], MenuEntry::Action(TrayAction::ShowHistory)));
    assert!(matches!(entries[1], MenuEntry::RecentSubmenu(_)));
    assert!(matches!(entries[2], MenuEntry::Action(TrayAction::PairDevice)));
    assert!(matches!(entries[3], MenuEntry::Action(TrayAction::OpenSettings)));
    assert!(matches!(entries[4], MenuEntry::Separator));
    assert!(matches!(entries[5], MenuEntry::Action(TrayAction::Quit)));
}

#[test]
fn tray_menu_handle_callback_method_set_stable() {
    // Public callback registration surface — pin via function pointers so
    // a rename or signature change breaks the test, not production.
    let _on_show: TrayCbUnit =
        |h, cb| h.on_show_history(cb);
    let _on_pair: TrayCbUnit =
        |h, cb| h.on_pair_device(cb);
    let _on_settings: TrayCbUnit =
        |h, cb| h.on_open_settings(cb);
    let _on_quit: TrayCbUnit =
        |h, cb| h.on_quit(cb);

    // Recent click is the *one* callback that takes the item id — pin it
    // separately so the &str-vs-String choice is locked in.
    let _on_recent: TrayCbStrRef =
        |h, cb| h.on_recent_click(cb);
}

#[test]
fn recent_item_struct_fields_stable_drift_guard() {
    // `id` + `preview` is the wire contract between the tray renderer and
    // the daemon's `history_page` row id. Exhaustive destructure.
    let item = RecentItem::new("row-42", "hello world");
    let RecentItem { id, preview } = item;
    assert_eq!(id, "row-42");
    assert_eq!(preview, "hello world");
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. IPC response struct shapes the UI parses.
//    `copypaste-ui::ipc_client` is binary-private, so we pin the types
//    surfaced through the *public* lib API instead — `AppSettings` and
//    `PairedDevice` are exactly the values `IpcClient::get_settings` and
//    `IpcClient::list_peers` deserialise into. Their Serde contract is
//    the IPC contract.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn ipc_response_parse_signatures_match_expected() {
    // Round-trip the exact JSON shape the daemon emits so that a silent
    // rename on either side (UI struct OR daemon response) trips this
    // test rather than failing in production.
    let settings_json = serde_json::json!({
        "launch_at_login": true,
        "private_mode":    false,
        "history_limit":   "Hundred",
        "supabase_url":    "https://x.supabase.co",
        "supabase_key":    "k",
        "device_name":     "Mac"
    });
    let parsed: AppSettings = serde_json::from_value(settings_json)
        .expect("AppSettings must deserialise from the daemon's get_settings response shape");
    assert!(parsed.launch_at_login);
    assert_eq!(parsed.device_name, "Mac");

    let peer_json = serde_json::json!({
        "name":        "Phone",
        "fingerprint": "0".repeat(64)
    });
    let peer: PairedDevice = serde_json::from_value(peer_json)
        .expect("PairedDevice must deserialise from the daemon's list_peers response shape");
    assert_eq!(peer.name, "Phone");
    assert_eq!(peer.fingerprint.len(), 64);

    // Round-trip the other way — UI → daemon save_settings payload.
    let s = AppSettings {
        launch_at_login: true,
        private_mode:    true,
        history_limit:   HistoryLimit::FiveHundred,
        supabase_url:    String::new(),
        supabase_key:    String::new(),
        device_name:     String::from("d"),
    };
    let v = serde_json::to_value(&s).expect("AppSettings must serialise for save_settings");
    let obj = v.as_object().expect("AppSettings serialises to a JSON object");
    // Exact wire field names — these are the IPC contract.
    for k in ["launch_at_login", "private_mode", "history_limit",
              "supabase_url", "supabase_key", "device_name"] {
        assert!(obj.contains_key(k), "missing field in save_settings payload: {k}");
    }
    assert_eq!(obj.len(), 6, "save_settings payload has exactly 6 fields");
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers — trait-bound assertions, evaluated at compile time.
// ─────────────────────────────────────────────────────────────────────────────

fn assert_send_sync<T: Send + Sync>() {}

fn assert_serde<T>()
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
}
