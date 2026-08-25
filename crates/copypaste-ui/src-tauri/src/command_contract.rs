//! The command vocabulary shared by Tauri, generated clients and dev tools.

macro_rules! command_registry {
    ($consumer:ident) => {
        $consumer! {
            native {
                SetNativeTheme => ("set_native_theme", crate::commands::appearance::set_native_theme, "{ theme: NativeTheme }", "void"),
                List => ("list", crate::commands::history::list, "{ limit: number; cursor: string | null }", "ItemPage"),
                Search => ("search", crate::commands::history::search, "{ query: string; limit: number }", "ItemPage"),
                AddItem => ("add_item", crate::commands::history::add_item, "{ content: string }", "Item"),
                CopyItem => ("copy_item", crate::commands::history::copy_item, "{ id: string }", "Item"),
                CopyItemAsPlainText => ("copy_item_as_plain_text", crate::commands::history::copy_item_as_plain_text, "{ id: string }", "Item"),
                RevealItem => ("reveal_item", crate::commands::history::reveal_item, "{ id: string }", "string"),
                GetItemBody => ("get_item_body", crate::commands::history::get_item_body, "{ id: string }", "string"),
                CopyItems => ("copy_items", crate::commands::history::copy_items, "{ ids: readonly string[] }", "number"),
                GetImagePreview => ("get_image_preview", crate::commands::history::get_image_preview, "{ id: string }", "ImagePreview"),
                GetSourceAppIcon => ("get_source_app_icon", crate::commands::history::get_source_app_icon, "{ bundleId: string }", "SourceAppIcon | null"),
                ListInstalledSourceApps => ("list_installed_source_apps", crate::commands::history::list_installed_source_apps, "undefined", "InstalledSourceApp[]"),
                DeleteItem => ("delete_item", crate::commands::history::delete_item, "{ id: string }", "boolean"),
                DeleteAll => ("delete_all", crate::commands::history::delete_all, "{ through: number | null }", "number"),
                HistoryCeiling => ("history_ceiling", crate::commands::history::history_ceiling, "undefined", "number"),
                SetPinned => ("set_pinned", crate::commands::history::set_pinned, "{ id: string; pinned: boolean }", "Item"),
                ReorderPinned => ("reorder_pinned", crate::commands::history::reorder_pinned, "{ ids: readonly string[] }", "void"),
                Status => ("status", crate::commands::status::status, "undefined", "StatusData"),
                SetDeviceName => ("set_device_name", crate::commands::status::set_device_name, "{ name: string }", "void"),
                CloudSignIn => ("cloud_sign_in", crate::commands::cloud::cloud_sign_in, "{ email: string; password: string; passphrase: string }", "CloudStatusData"),
                CloudSignUp => ("cloud_sign_up", crate::commands::cloud::cloud_sign_up, "{ email: string; password: string; passphrase: string }", "CloudStatusData"),
                CloudSetEndpoint => ("cloud_set_endpoint", crate::commands::cloud::cloud_set_endpoint, "{ url: string; anonKey: string }", "CloudStatusData"),
                CloudSignOut => ("cloud_sign_out", crate::commands::cloud::cloud_sign_out, "undefined", "CloudStatusData"),
                CloudStatus => ("cloud_status", crate::commands::cloud::cloud_status, "undefined", "CloudStatusData"),
                CloudSyncNow => ("cloud_sync_now", crate::commands::cloud::cloud_sync_now, "undefined", "CloudSyncData"),
                Diagnostics => ("diagnostics", crate::commands::diagnostics::diagnostics, "undefined", "Diagnostics"),
                RuntimeLogEvents => ("runtime_log_events", crate::commands::diagnostics::runtime_log_events, "{ query: Partial<RuntimeLogQuery> }", "RuntimeLogPage"),
                ExportDiagnosticsReport => ("export_diagnostics_report", crate::commands::diagnostics::export_diagnostics_report, "undefined", "boolean"),
                ExportSupportBundle => ("export_support_bundle", crate::commands::diagnostics::export_support_bundle, "undefined", "boolean"),
                CaptureState => ("capture_state", crate::commands::capture::capture_state, "undefined", "CaptureSnapshot"),
                CaptureRefresh => ("capture_refresh", crate::commands::capture::capture_refresh, "undefined", "CaptureSnapshot"),
                CaptureArm => ("capture_arm", crate::commands::capture::capture_arm, "undefined", "CaptureSnapshot"),
                CaptureDisarm => ("capture_disarm", crate::commands::capture::capture_disarm, "undefined", "CaptureSnapshot"),
                CaptureSetEnabled => ("capture_set_enabled", crate::commands::capture::capture_set_enabled, "{ enabled: boolean }", "CaptureSnapshot"),
                CaptureNow => ("capture_now", crate::commands::capture::capture_now, "{ source: CaptureSource }", "Item | null"),
                CaptureToastExplanation => ("capture_toast_explanation", crate::commands::capture::capture_toast_explanation, "undefined", "string"),
                CaptureSetToastSuppressed => ("capture_set_toast_suppressed", crate::commands::capture::capture_set_toast_suppressed, "{ suppressed: boolean; acknowledged: boolean }", "CaptureSnapshot"),
                CaptureOpenShizuku => ("capture_open_shizuku", crate::commands::capture::capture_open_shizuku, "undefined", "void"),
                CaptureOpenDeveloperOptions => ("capture_open_developer_options", crate::commands::capture::capture_open_developer_options, "undefined", "void"),
                CaptureRequestBatteryExemption => ("capture_request_battery_exemption", crate::commands::capture::capture_request_battery_exemption, "undefined", "void"),
                ServiceState => ("service_state", crate::commands::service::service_state, "undefined", "ServiceState"),
                StartService => ("start_service", crate::commands::service::start_service, "undefined", "ServiceState"),
                RestartService => ("restart_service", crate::commands::service::restart_service, "undefined", "ServiceState"),
                HideWindow => ("hide_window", crate::commands::service::hide_window, "undefined", "void"),
                ShowMainWindow => ("show_main_window", crate::commands::service::show_main_window, "undefined", "void"),
                SetAllowScreenshots => ("set_allow_screenshots", crate::commands::protection::set_allow_screenshots, "{ allow: boolean }", "void"),
                GetDefaultShortcut => ("get_default_shortcut", crate::commands::shortcut::get_default_shortcut, "undefined", "string"),
                GetShortcut => ("get_shortcut", crate::commands::shortcut::get_shortcut, "undefined", "string"),
                SetShortcut => ("set_shortcut", crate::commands::shortcut::set_shortcut, "{ accelerator: string }", "void"),
                GetOpenAtLogin => ("get_open_at_login", crate::commands::autostart::get_open_at_login, "undefined", "boolean"),
                SetOpenAtLogin => ("set_open_at_login", crate::commands::autostart::set_open_at_login, "{ enabled: boolean }", "boolean"),
                PermissionSnapshot => ("permission_snapshot", crate::commands::permissions::permission_snapshot, "undefined", "OnboardingPermissions"),
                PermissionRequest => ("permission_request", crate::commands::permissions::permission_request, "{ id: OnboardingPermissionId }", "OnboardingPermissions"),
                PermissionOpenSettings => ("permission_open_settings", crate::commands::permissions::permission_open_settings, "{ id: OnboardingPermissionId }", "OnboardingPermissions"),
                GetConfig => ("get_config", crate::commands::config::get_config, "undefined", "ConfigApplied"),
                SetConfig => ("set_config", crate::commands::config::set_config, "{ patch: ConfigPatch }", "ConfigApplied"),
                GetPrivateMode => ("get_private_mode", crate::commands::config::get_private_mode, "undefined", "PrivateModeData"),
                SetPrivateMode => ("set_private_mode", crate::commands::config::set_private_mode, "{ enabled: boolean }", "PrivateModeData"),
                ExportHistory => ("export_history", crate::commands::transfer::export_history, "{ includeSensitive: boolean }", "ExportReport | null"),
                PrepareImportHistory => ("prepare_import_history", crate::commands::transfer::prepare_import_history, "undefined", "ImportPreview | null"),
                ApplyImportHistory => ("apply_import_history", crate::commands::transfer::apply_import_history, "{ token: string }", "ImportData"),
                CancelImportHistory => ("cancel_import_history", crate::commands::transfer::cancel_import_history, "{ token: string }", "void"),
                BackupDatabase => ("backup_database", crate::commands::transfer::backup_database, "undefined", "number | null"),
                RestoreDatabase => ("restore_database", crate::commands::transfer::restore_database, "undefined", "boolean"),
                UpdateStatus => ("update_status", crate::updater::update_status, "undefined", "UpdateStatus"),
                CheckForUpdate => ("check_for_update", crate::updater::check_for_update, "undefined", "UpdateStatus"),
                InstallUpdate => ("install_update", crate::updater::install_update, "{ expectedVersion: string; progress: Channel<UpdateProgress> }", "UpdateStatus"),
                Peers => ("peers", crate::commands::peers::peers, "undefined", "PeerInfo[]"),
                Unpair => ("unpair", crate::commands::peers::unpair, "{ pairingId: string }", "void"),
                Revoke => ("revoke", crate::commands::peers::revoke, "{ pairingId: string }", "void"),
                SyncNow => ("sync_now", crate::commands::peers::sync_now, "{ pairingId: string | null }", "SyncResult[]"),
                Discovered => ("discovered", crate::commands::peers::discovered, "undefined", "DiscoveredDevice[]"),
                Rescan => ("rescan", crate::commands::peers::rescan, "undefined", "DiscoveredDevice[]"),
                PairCreateInvite => ("pair_create_invite", crate::commands::pairing::pair_create_invite, "undefined", "PairingCeremony"),
                PairScanInvite => ("pair_scan_invite", crate::commands::pairing::pair_scan_invite, "undefined", "PairingCeremony"),
                PairProgress => ("pair_progress", crate::commands::pairing::pair_progress, "undefined", "PairingCeremony"),
                PairPresent => ("pair_present", crate::commands::pairing::pair_present, "undefined", "PairingCeremony"),
                PairConfirm => ("pair_confirm", crate::commands::pairing::pair_confirm, "undefined", "PairingCeremony"),
                PairReject => ("pair_reject", crate::commands::pairing::pair_reject, "undefined", "PairingCeremony"),
                PairCancel => ("pair_cancel", crate::commands::pairing::pair_cancel, "undefined", "PairingCeremony"),
                CopyText => ("copy_text", crate::commands::clipboard::copy_text, "{ text: string }", "void"),
            }
            preview_only {
                PairPreviewInvite => ("pair_preview_invite", "undefined", "{ ceremony: PairingCeremony; code: string; listen_addr: string; expires_in_secs: number; qr_svg: string }"),
                PairPreviewJoin => ("pair_preview_join", "{ code: string; addr: string }", "PairingCeremony"),
            }
        }
    };
}

macro_rules! define_contract {
    (
        native { $( $native_variant:ident => ($native_name:literal, $path:path, $native_args:literal, $native_result:literal), )* }
        preview_only { $( $preview_variant:ident => ($preview_name:literal, $preview_args:literal, $preview_result:literal), )* }
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum UiCommandName {
            $( $native_variant, )*
            $( $preview_variant, )*
        }

        impl UiCommandName {
            pub const NATIVE: &'static [Self] = &[$( Self::$native_variant, )*];
            pub const PREVIEW_ONLY: &'static [Self] = &[$( Self::$preview_variant, )*];
            pub const ALL: &'static [Self] = &[
                $( Self::$native_variant, )*
                $( Self::$preview_variant, )*
            ];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$native_variant => $native_name, )*
                    $( Self::$preview_variant => $preview_name, )*
                }
            }

            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $( $native_name => Some(Self::$native_variant), )*
                    $( $preview_name => Some(Self::$preview_variant), )*
                    _ => None,
                }
            }

            pub const fn typescript_args(self) -> &'static str {
                match self {
                    $( Self::$native_variant => $native_args, )*
                    $( Self::$preview_variant => $preview_args, )*
                }
            }

            pub const fn typescript_result(self) -> &'static str {
                match self {
                    $( Self::$native_variant => $native_result, )*
                    $( Self::$preview_variant => $preview_result, )*
                }
            }
        }

        macro_rules! ui_invoke_handler {
            () => {
                tauri::generate_handler![$( $path, )*]
            };
        }
        pub(crate) use ui_invoke_handler;
    };
}

command_registry!(define_contract);

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn names_are_unique_and_parse_back_to_the_same_variant() {
        let mut names = HashSet::new();
        for command in UiCommandName::ALL {
            assert!(names.insert(command.as_str()), "{}", command.as_str());
            assert_eq!(UiCommandName::parse(command.as_str()), Some(*command));
            assert!(!command.typescript_args().trim().is_empty());
            assert!(!command.typescript_result().trim().is_empty());
            assert_ne!(command.typescript_result(), "unknown");
        }
        assert_eq!(
            names.len(),
            UiCommandName::NATIVE.len() + UiCommandName::PREVIEW_ONLY.len()
        );
        assert_eq!(UiCommandName::parse("future_command"), None);
    }
}
