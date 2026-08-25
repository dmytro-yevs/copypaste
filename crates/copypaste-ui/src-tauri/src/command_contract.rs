//! The command vocabulary shared by Tauri, generated clients and dev tools.

macro_rules! command_registry {
    ($consumer:ident) => {
        $consumer! {
            native {
                SetNativeTheme => ("set_native_theme", crate::commands::appearance::set_native_theme),
                List => ("list", crate::commands::history::list),
                Search => ("search", crate::commands::history::search),
                AddItem => ("add_item", crate::commands::history::add_item),
                CopyItem => ("copy_item", crate::commands::history::copy_item),
                CopyItemAsPlainText => ("copy_item_as_plain_text", crate::commands::history::copy_item_as_plain_text),
                RevealItem => ("reveal_item", crate::commands::history::reveal_item),
                GetItemBody => ("get_item_body", crate::commands::history::get_item_body),
                CopyItems => ("copy_items", crate::commands::history::copy_items),
                GetImagePreview => ("get_image_preview", crate::commands::history::get_image_preview),
                GetSourceAppIcon => ("get_source_app_icon", crate::commands::history::get_source_app_icon),
                ListInstalledSourceApps => ("list_installed_source_apps", crate::commands::history::list_installed_source_apps),
                DeleteItem => ("delete_item", crate::commands::history::delete_item),
                DeleteAll => ("delete_all", crate::commands::history::delete_all),
                HistoryCeiling => ("history_ceiling", crate::commands::history::history_ceiling),
                SetPinned => ("set_pinned", crate::commands::history::set_pinned),
                ReorderPinned => ("reorder_pinned", crate::commands::history::reorder_pinned),
                Status => ("status", crate::commands::status::status),
                SetDeviceName => ("set_device_name", crate::commands::status::set_device_name),
                CloudSignIn => ("cloud_sign_in", crate::commands::cloud::cloud_sign_in),
                CloudSignUp => ("cloud_sign_up", crate::commands::cloud::cloud_sign_up),
                CloudSetEndpoint => ("cloud_set_endpoint", crate::commands::cloud::cloud_set_endpoint),
                CloudSignOut => ("cloud_sign_out", crate::commands::cloud::cloud_sign_out),
                CloudStatus => ("cloud_status", crate::commands::cloud::cloud_status),
                CloudSyncNow => ("cloud_sync_now", crate::commands::cloud::cloud_sync_now),
                Diagnostics => ("diagnostics", crate::commands::diagnostics::diagnostics),
                RuntimeLogEvents => ("runtime_log_events", crate::commands::diagnostics::runtime_log_events),
                ExportDiagnosticsReport => ("export_diagnostics_report", crate::commands::diagnostics::export_diagnostics_report),
                ExportSupportBundle => ("export_support_bundle", crate::commands::diagnostics::export_support_bundle),
                CaptureState => ("capture_state", crate::commands::capture::capture_state),
                CaptureRefresh => ("capture_refresh", crate::commands::capture::capture_refresh),
                CaptureArm => ("capture_arm", crate::commands::capture::capture_arm),
                CaptureDisarm => ("capture_disarm", crate::commands::capture::capture_disarm),
                CaptureSetEnabled => ("capture_set_enabled", crate::commands::capture::capture_set_enabled),
                CaptureNow => ("capture_now", crate::commands::capture::capture_now),
                CaptureToastExplanation => ("capture_toast_explanation", crate::commands::capture::capture_toast_explanation),
                CaptureSetToastSuppressed => ("capture_set_toast_suppressed", crate::commands::capture::capture_set_toast_suppressed),
                CaptureOpenShizuku => ("capture_open_shizuku", crate::commands::capture::capture_open_shizuku),
                CaptureOpenDeveloperOptions => ("capture_open_developer_options", crate::commands::capture::capture_open_developer_options),
                CaptureRequestBatteryExemption => ("capture_request_battery_exemption", crate::commands::capture::capture_request_battery_exemption),
                ServiceState => ("service_state", crate::commands::service::service_state),
                StartService => ("start_service", crate::commands::service::start_service),
                RestartService => ("restart_service", crate::commands::service::restart_service),
                HideWindow => ("hide_window", crate::commands::service::hide_window),
                ShowMainWindow => ("show_main_window", crate::commands::service::show_main_window),
                SetAllowScreenshots => ("set_allow_screenshots", crate::commands::protection::set_allow_screenshots),
                GetDefaultShortcut => ("get_default_shortcut", crate::commands::shortcut::get_default_shortcut),
                GetShortcut => ("get_shortcut", crate::commands::shortcut::get_shortcut),
                SetShortcut => ("set_shortcut", crate::commands::shortcut::set_shortcut),
                GetOpenAtLogin => ("get_open_at_login", crate::commands::autostart::get_open_at_login),
                SetOpenAtLogin => ("set_open_at_login", crate::commands::autostart::set_open_at_login),
                PermissionSnapshot => ("permission_snapshot", crate::commands::permissions::permission_snapshot),
                PermissionRequest => ("permission_request", crate::commands::permissions::permission_request),
                PermissionOpenSettings => ("permission_open_settings", crate::commands::permissions::permission_open_settings),
                GetConfig => ("get_config", crate::commands::config::get_config),
                SetConfig => ("set_config", crate::commands::config::set_config),
                GetPrivateMode => ("get_private_mode", crate::commands::config::get_private_mode),
                SetPrivateMode => ("set_private_mode", crate::commands::config::set_private_mode),
                ExportHistory => ("export_history", crate::commands::transfer::export_history),
                PrepareImportHistory => ("prepare_import_history", crate::commands::transfer::prepare_import_history),
                ApplyImportHistory => ("apply_import_history", crate::commands::transfer::apply_import_history),
                CancelImportHistory => ("cancel_import_history", crate::commands::transfer::cancel_import_history),
                BackupDatabase => ("backup_database", crate::commands::transfer::backup_database),
                RestoreDatabase => ("restore_database", crate::commands::transfer::restore_database),
                UpdateStatus => ("update_status", crate::updater::update_status),
                CheckForUpdate => ("check_for_update", crate::updater::check_for_update),
                InstallUpdate => ("install_update", crate::updater::install_update),
                Peers => ("peers", crate::commands::peers::peers),
                Unpair => ("unpair", crate::commands::peers::unpair),
                Revoke => ("revoke", crate::commands::peers::revoke),
                SyncNow => ("sync_now", crate::commands::peers::sync_now),
                Discovered => ("discovered", crate::commands::peers::discovered),
                Rescan => ("rescan", crate::commands::peers::rescan),
                PairCreateInvite => ("pair_create_invite", crate::commands::pairing::pair_create_invite),
                PairScanInvite => ("pair_scan_invite", crate::commands::pairing::pair_scan_invite),
                PairProgress => ("pair_progress", crate::commands::pairing::pair_progress),
                PairPresent => ("pair_present", crate::commands::pairing::pair_present),
                PairConfirm => ("pair_confirm", crate::commands::pairing::pair_confirm),
                PairReject => ("pair_reject", crate::commands::pairing::pair_reject),
                PairCancel => ("pair_cancel", crate::commands::pairing::pair_cancel),
                CopyText => ("copy_text", crate::commands::clipboard::copy_text),
            }
            preview_only {
                PairPreviewInvite => "pair_preview_invite",
                PairPreviewJoin => "pair_preview_join",
            }
        }
    };
}

macro_rules! define_contract {
    (
        native { $( $native_variant:ident => ($native_name:literal, $path:path), )* }
        preview_only { $( $preview_variant:ident => $preview_name:literal, )* }
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
        }
        assert_eq!(
            names.len(),
            UiCommandName::NATIVE.len() + UiCommandName::PREVIEW_ONLY.len()
        );
        assert_eq!(UiCommandName::parse("future_command"), None);
    }
}
