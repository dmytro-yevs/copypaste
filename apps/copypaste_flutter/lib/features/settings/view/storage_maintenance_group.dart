import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/failure_toasts.dart';
import '../../../shared/format_bytes.dart';
import '../../../shared/preference_toggle.dart';
import '../../../shared/system_date_time.dart';
import '../controller/storage_maintenance_controller.dart';
import '../models/storage_maintenance_models.dart';
import 'settings_section.dart';
import 'storage_usage_bar.dart';

/// Search targets of the storage rows.
class StorageRowKeys {
  const StorageRowKeys({
    required this.usage,
    required this.clearCache,
    required this.dataFolder,
    required this.confirmDelete,
    required this.upgradeBackups,
  });

  final GlobalKey usage;
  final GlobalKey clearCache;
  final GlobalKey dataFolder;
  final GlobalKey confirmDelete;
  final GlobalKey upgradeBackups;
}

/// Storage usage, cache clearing, data folders, the single-delete
/// confirmation and pre-upgrade backups, as one settings group.
class StorageMaintenanceGroup extends StatefulWidget {
  const StorageMaintenanceGroup({
    super.key,
    required this.controller,
    required this.keys,
    required this.canRestore,
    this.confirmDelete,
    this.highlighted,
  });

  final StorageMaintenanceController controller;
  final StorageRowKeys keys;
  final PreferenceToggle? confirmDelete;

  /// Whether no other history file operation runs.
  final bool Function() canRestore;

  /// The search result to highlight, as a `result-…` target id.
  final String? highlighted;

  @override
  State<StorageMaintenanceGroup> createState() =>
      _StorageMaintenanceGroupState();
}

class _StorageMaintenanceGroupState extends State<StorageMaintenanceGroup> {
  StorageMaintenanceController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    // Measured each time the section opens; nothing is polled.
    unawaited(_controller.refresh());
  }

  bool _isHighlighted(String id) => widget.highlighted == id;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return FailureToasts(
      listenable: _controller,
      failure: () => _controller.error,
      child: ListenableBuilder(
        listenable: _controller,
        builder: (context, _) => SettingsGroup(
          children: [
            SettingRow(
              key: widget.keys.usage,
              highlighted: _isHighlighted('result-storage-usage'),
              title: l10n.settingsStorageUsageTitle,
              description: l10n.settingsStorageUsageDescription,
              footer: _breakdown(context, _controller.breakdown),
            ),
            SettingRow(
              key: widget.keys.clearCache,
              highlighted: _isHighlighted('result-clear-cache'),
              title: l10n.settingsClearCacheTitle,
              description: l10n.settingsClearCacheDescription,
              trailing: Button.secondary(
                key: const ValueKey<String>('settings-clear-cache'),
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: _controller.busy ? null : _clearCache,
                leading: _icon(
                  StorageMaintenanceOperation.clearCache,
                  LucideIcons.eraser,
                ),
                child: Text(l10n.settingsClearCacheAction),
              ),
            ),
            if (_controller.canOpenFolders) ...[
              SettingRow(
                key: widget.keys.dataFolder,
                highlighted: _isHighlighted('result-data-folder'),
                title: l10n.settingsDataFolderTitle,
                description: l10n.settingsDataFolderDescription,
                trailing: _openButton(
                  const ValueKey<String>('settings-open-data-folder'),
                  _controller.openDataFolder,
                ),
              ),
              SettingRow(
                title: l10n.settingsLogsFolderTitle,
                description: l10n.settingsLogsFolderDescription,
                trailing: _openButton(
                  const ValueKey<String>('settings-open-logs-folder'),
                  _controller.openLogsFolder,
                ),
              ),
            ],
            if (widget.confirmDelete case final toggle?)
              ValueListenableBuilder<bool>(
                valueListenable: toggle,
                builder: (context, enabled, _) => SettingRow(
                  key: widget.keys.confirmDelete,
                  highlighted: _isHighlighted('result-confirm-delete'),
                  title: l10n.settingsConfirmDeleteTitle,
                  description: l10n.settingsConfirmDeleteDescription,
                  trailing: Switch(
                    key: const ValueKey<String>('settings-confirm-delete'),
                    value: enabled,
                    onChanged: (value) => unawaited(toggle.set(value)),
                  ),
                ),
              ),
            SettingRow(
              key: widget.keys.upgradeBackups,
              highlighted: _isHighlighted('result-upgrade-backups'),
              title: l10n.settingsUpgradeBackupsTitle,
              description: l10n.settingsUpgradeBackupsDescription,
              footer: _upgradeBackups(context),
            ),
          ],
        ),
      ),
    );
  }

  Widget? _breakdown(BuildContext context, StorageBreakdown? breakdown) {
    if (breakdown == null) return null;
    final l10n = context.l10n;
    final colors = Theme.of(context).colorScheme;
    final parts = <StorageUsagePart>[
      (
        label: l10n.settingsStorageHistory,
        bytes: breakdown.historyBytes,
        color: colors.chart5,
      ),
      (
        label: l10n.settingsStorageFiles,
        bytes: breakdown.fileBytes,
        color: colors.chart2,
      ),
      (
        label: l10n.settingsStorageCaches,
        bytes: breakdown.cacheBytes,
        color: colors.chart3,
      ),
      (
        label: l10n.settingsStorageUpgradeBackups,
        bytes: breakdown.upgradeBackupBytes,
        color: colors.chart4,
      ),
      (
        label: l10n.settingsStorageOther,
        bytes: breakdown.otherBytes,
        color: colors.mutedForeground,
      ),
    ];
    return Column(
      key: const ValueKey<String>('settings-storage-breakdown'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.baseline,
          textBaseline: TextBaseline.alphabetic,
          children: [
            Text(formatBytes(breakdown.totalBytes)).h4(),
            const Gap(AppSpacing.sm),
            Text(l10n.settingsStorageTotal).textSmall().muted(),
          ],
        ),
        const Gap(AppSpacing.sm),
        StorageUsageBar(parts: parts, total: breakdown.totalBytes),
      ],
    );
  }

  Widget _upgradeBackups(BuildContext context) {
    final backups = _controller.upgradeBackups;
    if (backups.isEmpty) {
      return Text(context.l10n.settingsUpgradeBackupsEmpty).textSmall().muted();
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      spacing: AppSpacing.sm,
      children: [
        for (final backup in backups)
          Row(
            children: [
              Expanded(
                child: Text(
                  context.l10n.settingsUpgradeBackupEntry(
                    formatSystemDateTime(context, backup.createdAt),
                    formatBytes(backup.sizeBytes),
                  ),
                ).textSmall(),
              ),
              const Gap(AppSpacing.md),
              Button.secondary(
                key: ValueKey<String>('upgrade-backup-${backup.path}'),
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: _controller.busy || !widget.canRestore()
                    ? null
                    : () => _confirmRestore(backup),
                leading: const AppIconFrame(
                  child: Icon(LucideIcons.archiveRestore),
                ),
                child: Text(context.l10n.settingsUpgradeBackupRestoreAction),
              ),
            ],
          ),
      ],
    );
  }

  Widget _openButton(Key key, Future<bool> Function() open) => Button.secondary(
    key: key,
    style: AppTheme.actionButtonStyle(const ButtonStyle.secondary()),
    onPressed: _controller.busy ? null : () => unawaited(open()),
    leading: const AppIconFrame(child: Icon(LucideIcons.folderOpen)),
    child: Text(context.l10n.settingsOpenFolderAction),
  );

  Widget _icon(StorageMaintenanceOperation operation, IconData icon) =>
      _controller.isPending(operation)
      ? const SizedBox.square(
          dimension: AppIconSize.sm,
          child: AppIconFrame(
            size: AppIconSize.sm,
            child: CircularProgressIndicator(),
          ),
        )
      : AppIconFrame(child: Icon(icon));

  Future<void> _clearCache() async {
    final cleared = await _controller.clearCache();
    if (!cleared || !mounted) return;
    final l10n = context.l10n;
    AppToast.show(
      context,
      title: l10n.settingsClearCacheDoneTitle,
      message: l10n.settingsClearCacheDoneMessage,
      tone: AppToastTone.success,
    );
  }

  Future<void> _confirmRestore(UpgradeBackup backup) async {
    final l10n = context.l10n;
    final restored = await AppOverlays.showActionDialog(
      context,
      icon: LucideIcons.archiveRestore,
      title: Text(l10n.settingsUpgradeBackupRestoreTitle),
      content: Text(
        l10n.settingsUpgradeBackupRestoreMessage(
          formatSystemDateTime(context, backup.createdAt),
        ),
      ),
      actionLabel: l10n.settingsUpgradeBackupRestoreAction,
      changes: _controller,
      canConfirm: () => !_controller.busy && widget.canRestore(),
      onConfirm: () => _controller.restore(backup),
      failureMessage: () => _controller.error ?? l10n.settingsRestoreFailed,
    );
    if (restored != true || !mounted) return;
    AppToast.show(
      context,
      title: l10n.settingsRestoreDoneTitle,
      message: l10n.settingsRestoreDoneMessage,
      tone: AppToastTone.success,
    );
  }
}
