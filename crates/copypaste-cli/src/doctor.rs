//! Redacted, read-only platform diagnostics from the daemon status reply.

use copypaste_ipc::{PlatformCapability, StatusData};
use serde_json::{json, Value};

const UNPROBED: &str = "No daemon-side probe is registered for this capability.";

fn capability(state: &str, backend: Option<&str>, guarantee: &str, reason: Option<&str>) -> Value {
    json!({
        "state": state,
        "backend": backend,
        "guarantee": guarantee,
        "reason": reason,
        "permissions": [],
    })
}

fn transport(backend: &str) -> Option<&'static str> {
    match backend {
        "linux-x11-system-clipboard" => Some("x11"),
        "linux-gnome-wayland-clipboard" => Some("gnome_bridge"),
        "linux-kwin-wayland-clipboard" => Some("kwin_bridge"),
        "linux-wayland-data-control" => Some("wayland_data_control"),
        _ => None,
    }
}

pub(crate) fn diagnostics(status: &StatusData) -> Value {
    let backend = status
        .clipboard_available
        .then(|| transport(&status.clipboard_backend))
        .flatten();
    let source = matches!(backend, Some("x11" | "gnome_bridge" | "kwin_bridge"));
    let clipboard = if backend.is_some() {
        capability("available", backend, "verified", None)
    } else {
        capability(
            "error",
            None,
            "none",
            Some("Clipboard transport unavailable."),
        )
    };
    let capture = if backend.is_none() {
        clipboard.clone()
    } else if !status.capture_running {
        capability("error", backend, "none", Some("Capture loop is stopped."))
    } else {
        capability(
            "available",
            backend,
            "verified",
            status
                .private_mode
                .then_some("Capture is paused by private mode."),
        )
    };
    let attribution = if backend.is_none() {
        clipboard.clone()
    } else if source {
        capability(
            "available",
            backend,
            "best_effort",
            Some("Identity can be unknown for individual selections."),
        )
    } else {
        capability(
            "unsupported",
            backend,
            "none",
            Some("Data-control does not authenticate the source application."),
        )
    };
    let privacy = if backend.is_none() {
        clipboard.clone()
    } else {
        capability(
            "available",
            backend,
            "verified",
            Some("Unknown sources are rejected before payload reads when exclusions are set."),
        )
    };
    let unprobed = capability("error", None, "none", Some(UNPROBED));
    let mut report = json!({
        "schema": 1,
        "capabilities": {
            "clipboard_capture": capture,
            "clipboard_read_write": clipboard,
            "source_attribution": attribution,
            "privacy_enforcement": privacy,
            "global_shortcuts": unprobed,
            "quick_paste": capability("error", None, "none", Some(UNPROBED)),
            "secure_storage": capability("error", None, "none", Some(UNPROBED)),
            "desktop_integration": capability("error", None, "none", Some(UNPROBED)),
            "updates": capability("error", None, "none", Some(UNPROBED)),
        },
    });
    for (kind, value) in &status.platform_capabilities {
        let name = match kind {
            PlatformCapability::ClipboardCapture => "clipboard_capture",
            PlatformCapability::ClipboardReadWrite => "clipboard_read_write",
            PlatformCapability::SourceAttribution => "source_attribution",
            PlatformCapability::PrivacyEnforcement => "privacy_enforcement",
            PlatformCapability::GlobalShortcuts => "global_shortcuts",
            PlatformCapability::QuickPaste => "quick_paste",
            PlatformCapability::SecureStorage => "secure_storage",
            PlatformCapability::DesktopIntegration => "desktop_integration",
            PlatformCapability::Updates => "updates",
        };
        let mut serialized = serde_json::to_value(value).expect("typed capability is serializable");
        if *kind == PlatformCapability::ClipboardReadWrite {
            serialized
                .as_object_mut()
                .expect("typed capability is a JSON object")
                .remove("mime_types");
        }
        report["capabilities"][name] = serialized;
    }
    if report["capabilities"]["clipboard_read_write"]["state"] == "available" {
        report["capabilities"]["clipboard_read_write"]["mime_formats"] = status
            .platform_capabilities
            .get(&PlatformCapability::ClipboardReadWrite)
            .filter(|capability| !capability.mime_types.is_empty())
            .map(|capability| json!(capability.mime_types))
            .unwrap_or_else(|| {
                json!([
                    "text/plain;charset=utf-8",
                    "text/html",
                    "text/rtf",
                    "image/png",
                    "image/tiff",
                    "text/uri-list",
                ])
            });
        report["capabilities"]["clipboard_read_write"]["limitations"] = json!([
            "One local file per clipboard change.",
            "Capture size limits follow the current settings and transport bounds.",
        ]);
    }
    report
}

pub(crate) fn render(status: &StatusData, as_json: bool) -> String {
    let report = diagnostics(status);
    if as_json {
        return serde_json::to_string_pretty(&report).expect("JSON diagnostic is serializable");
    }
    let capabilities = &report["capabilities"];
    [
        "clipboard_capture",
        "clipboard_read_write",
        "source_attribution",
        "privacy_enforcement",
        "global_shortcuts",
        "quick_paste",
        "secure_storage",
        "desktop_integration",
        "updates",
    ]
    .iter()
    .map(|name| {
        let item = &capabilities[name];
        format!(
            "{name}: {} ({})",
            item["state"].as_str().unwrap_or("error"),
            item["backend"].as_str().unwrap_or("no backend")
        )
    })
    .collect::<Vec<_>>()
    .join("\n")
}

pub(crate) fn unavailable_json() -> String {
    let unavailable = capability("error", None, "none", Some("Daemon status unavailable."));
    let report = json!({
        "schema": 1,
        "capabilities": {
            "clipboard_capture": unavailable,
            "clipboard_read_write": capability("error", None, "none", Some("Daemon status unavailable.")),
            "source_attribution": capability("error", None, "none", Some("Daemon status unavailable.")),
            "privacy_enforcement": capability("error", None, "none", Some("Daemon status unavailable.")),
            "global_shortcuts": capability("error", None, "none", Some("Daemon status unavailable.")),
            "quick_paste": capability("error", None, "none", Some("Daemon status unavailable.")),
            "secure_storage": capability("error", None, "none", Some("Daemon status unavailable.")),
            "desktop_integration": capability("error", None, "none", Some("Daemon status unavailable.")),
            "updates": capability("error", None, "none", Some("Daemon status unavailable.")),
        },
    });
    serde_json::to_string_pretty(&report).expect("JSON diagnostic is serializable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use copypaste_ipc::{CapabilityGuarantee, CapabilityState, CapabilityStatus};

    #[test]
    fn doctor_accepts_the_requested_json_command_line() {
        let cli = crate::cli::Cli::try_parse_from(["copypaste", "doctor", "--json"]).unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, crate::cli::Command::Doctor));
    }

    fn status(backend: &str, available: bool) -> StatusData {
        StatusData {
            platform_capabilities: Default::default(),
            sync_status: Default::default(),
            device_name: "private device name".into(),
            device_id: Some("private device ID".into()),
            version: "1.0.0".into(),
            protocol_version: 1,
            listen_addr: Some("private address".into()),
            device_details: None,
            item_count: 42,
            capture_running: true,
            clipboard_available: available,
            clipboard_backend: backend.into(),
            private_mode: false,
            private_mode_epoch: 0,
            counters: Default::default(),
            settings_health: None,
        }
    }

    #[test]
    fn redacts_status_and_reports_data_control_attribution_limit() {
        let report = render(&status("linux-wayland-data-control", true), true);
        let value: Value = serde_json::from_str(&report).unwrap();
        assert_eq!(
            value["capabilities"]["clipboard_capture"]["state"],
            "available"
        );
        assert_eq!(
            value["capabilities"]["source_attribution"]["state"],
            "unsupported"
        );
        assert!(
            value["capabilities"]["clipboard_read_write"]["mime_formats"]
                .as_array()
                .unwrap()
                .contains(&json!("image/png"))
        );
        for secret in [
            "private device name",
            "private device ID",
            "private address",
            "42",
        ] {
            assert!(!report.contains(secret));
        }
    }

    #[test]
    fn unavailable_transport_never_claims_capture_or_privacy() {
        let report = diagnostics(&status("linux-wayland-data-control-inactive", false));
        assert_eq!(
            report["capabilities"]["clipboard_capture"]["state"],
            "error"
        );
        assert_eq!(
            report["capabilities"]["privacy_enforcement"]["state"],
            "error"
        );
    }

    #[test]
    fn typed_adapter_report_overrides_legacy_backend_inference() {
        let mut status = status("linux-wayland-data-control", true);
        status.platform_capabilities.insert(
            PlatformCapability::SourceAttribution,
            CapabilityStatus {
                state: CapabilityState::Error,
                backend: Some("wayland_data_control".into()),
                reason: Some("Adapter lost its source probe.".into()),
                permissions: Vec::new(),
                guarantee: CapabilityGuarantee::None,
                mime_types: Vec::new(),
                max_bytes: None,
            },
        );
        let report = diagnostics(&status);
        assert_eq!(
            report["capabilities"]["source_attribution"]["state"],
            "error"
        );
        assert_eq!(
            report["capabilities"]["source_attribution"]["reason"],
            "Adapter lost its source probe."
        );
    }

    #[test]
    fn typed_clipboard_formats_keep_the_ipc_size_limit_in_diagnostics() {
        let mut status = status("linux-x11-system-clipboard", true);
        status.platform_capabilities.insert(
            PlatformCapability::ClipboardReadWrite,
            CapabilityStatus {
                state: CapabilityState::Available,
                backend: Some("linux-x11-system-clipboard".into()),
                reason: None,
                permissions: Vec::new(),
                guarantee: CapabilityGuarantee::Verified,
                mime_types: vec!["image/png".into()],
                max_bytes: Some(4 * 1024 * 1024),
            },
        );
        let value = diagnostics(&status);
        let read_write = &value["capabilities"]["clipboard_read_write"];
        assert_eq!(read_write["mime_formats"], json!(["image/png"]));
        assert_eq!(read_write["max_bytes"], 4 * 1024 * 1024);
        assert!(read_write.get("mime_types").is_none());
    }

    #[test]
    fn unavailable_daemon_emits_only_fixed_diagnostic_fields() {
        let value: Value = serde_json::from_str(&unavailable_json()).unwrap();
        assert_eq!(value["capabilities"].as_object().unwrap().len(), 9);
        assert_eq!(value["capabilities"]["secure_storage"]["state"], "error");
        assert!(value.get("device_name").is_none());
    }
}
