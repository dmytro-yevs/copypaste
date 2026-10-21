import 'sync_status.dart';
import '../../../platform/capture/capture_control_state.dart';

enum SettingsLoadState { loading, ready, error }

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
  });

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
