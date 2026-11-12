import 'package:flutter/foundation.dart';

import 'sync_status.dart';
import '../../../platform/capture/capture_control_state.dart';

enum SettingsLoadState { loading, ready, error }

/// One user-started change. Each runs and reports pending and failure on its
/// own; only the history file operations exclude one another.
enum SettingsOperation {
  capture,
  screenshotCapture,
  skipSecret,
  skipTransient,
  retention,
  storageQuota,
  maxFileSize,
  excludedApps,
  lanVisibility,
  instantClipboard,
  syncEnabled,
  notifyOnCopy,
  notificationPreview,
  soundOnCopy,
  blockScreenshots,
  exportText,
  backup,
  restore;

  bool get transfersHistory =>
      this == exportText || this == backup || this == restore;

  /// Writes the runtime settings record.
  bool get writesSettings =>
      !transfersHistory &&
      this != capture &&
      this != screenshotCapture &&
      this != blockScreenshots;

  /// Writes the capture or screenshot capture state.
  bool get writesCapture => this == capture || this == screenshotCapture;
}

class CaptureSettingsState {
  const CaptureSettingsState({
    this.syncStatus = const SyncStatus(),
    this.settingsHealth,
    this.captureWatch = false,
    this.captureRevision = 0,
    required this.running,
    required this.paused,
    required this.epoch,
  });

  final bool captureWatch;
  final int captureRevision;
  final bool running;
  final bool paused;
  final int epoch;
  final SyncStatus syncStatus;
  final RuntimeSettingsHealth? settingsHealth;

  CaptureControlState get controlState => paused
      ? CaptureControlState.paused
      : running
      ? CaptureControlState.running
      : CaptureControlState.stopped;

  String get label => controlState.label;
}

class RuntimeSettingsHealth {
  const RuntimeSettingsHealth({
    required this.recordUnreadable,
    required this.unreadableFields,
  });

  final bool recordUnreadable;
  final List<String> unreadableFields;

  bool get degraded => recordUnreadable || unreadableFields.isNotEmpty;
}

/// History weighed against the storage quota. Pinned clips are outside the
/// quota, so they are outside [usedBytes] and [oldestUnpinnedAt]. One clip
/// larger than the quota is kept, so [usedBytes] can exceed [quotaBytes].
@immutable
class StorageUsage {
  const StorageUsage({
    required this.usedBytes,
    required this.quotaBytes,
    this.oldestUnpinnedAt,
  });

  final int usedBytes;
  final int quotaBytes;

  /// Capture time of the oldest unpinned clip; null when there is none.
  final DateTime? oldestUnpinnedAt;

  /// Without a quota only [usedBytes] is meaningful.
  bool get hasQuota => quotaBytes > 0;
  bool get overQuota => hasQuota && usedBytes > quotaBytes;

  /// The used share of the quota in 0..1; null without a quota.
  double? get fraction =>
      hasQuota ? (usedBytes / quotaBytes).clamp(0.0, 1.0) : null;

  @override
  bool operator ==(Object other) =>
      other is StorageUsage &&
      other.usedBytes == usedBytes &&
      other.quotaBytes == quotaBytes &&
      other.oldestUnpinnedAt == oldestUnpinnedAt;

  @override
  int get hashCode => Object.hash(usedBytes, quotaBytes, oldestUnpinnedAt);
}

class RuntimeSettings {
  const RuntimeSettings({
    this.maxFileSizeBytes = 64 * 1024 * 1024,
    this.skipSecret = true,
    this.skipTransient = true,
    required this.retentionDays,
    required this.storageQuotaBytes,
    required this.excludedAppIds,
    required this.lanVisibility,
    required this.syncEnabled,
    this.instantClipboard = true,
    required this.notifyOnCopy,
    this.notificationPreview = true,
    required this.soundOnCopy,
    this.storageUsage,
  });

  /// Read with these settings; null when the runtime did not report it.
  final StorageUsage? storageUsage;
  final bool skipSecret;
  final bool skipTransient;
  final int retentionDays;
  final int storageQuotaBytes;
  final int maxFileSizeBytes;
  final List<String> excludedAppIds;
  final bool lanVisibility;
  final bool syncEnabled;
  final bool instantClipboard;
  final bool notifyOnCopy;
  final bool notificationPreview;
  final bool soundOnCopy;
}

class RuntimeSettingsChange {
  const RuntimeSettingsChange({
    this.maxFileSizeBytes,
    this.skipSecret,
    this.skipTransient,
    this.retentionDays,
    this.storageQuotaBytes,
    this.excludedAppIds,
    this.lanVisibility,
    this.syncEnabled,
    this.instantClipboard,
    this.notifyOnCopy,
    this.notificationPreview,
    this.soundOnCopy,
  });

  final bool? skipSecret;
  final bool? skipTransient;
  final int? retentionDays;
  final int? storageQuotaBytes;
  final int? maxFileSizeBytes;
  final List<String>? excludedAppIds;
  final bool? lanVisibility;
  final bool? syncEnabled;
  final bool? instantClipboard;
  final bool? notifyOnCopy;
  final bool? notificationPreview;
  final bool? soundOnCopy;
}

class TextExportResult {
  const TextExportResult({
    required this.exported,
    required this.skippedNonText,
    required this.skippedUndecryptable,
  });

  final int exported;
  final int skippedNonText;
  final int skippedUndecryptable;
}

class BackupResult {
  const BackupResult({required this.sizeBytes});

  final int sizeBytes;
}

/// A Settings data operation that reports its progress.
enum DataTransferKind { textExport, backup }

/// Progress of the running text export or backup.
///
/// [processed] and [total] are null until the runtime reports a total. Export
/// counts history rows and backup counts database pages.
class DataTransferProgress {
  const DataTransferProgress(this.kind, {this.processed, this.total});

  final DataTransferKind kind;
  final int? processed;
  final int? total;

  /// The completed share in 0..1, or null while the total is unknown.
  double? get fraction {
    final processed = this.processed;
    final total = this.total;
    if (processed == null || total == null) return null;
    if (total <= 0) return 1;
    return (processed / total).clamp(0.0, 1.0);
  }
}
