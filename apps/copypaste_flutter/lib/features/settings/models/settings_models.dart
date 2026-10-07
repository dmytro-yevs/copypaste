import 'sync_status.dart';

enum SettingsLoadState { loading, ready, error }

class CaptureSettingsState {
  const CaptureSettingsState({
    this.syncStatus = const SyncStatus(),
    required this.running,
    required this.paused,
    required this.epoch,
  });

  final bool running;
  final bool paused;
  final int epoch;
  final SyncStatus syncStatus;

  String get label {
    if (paused) return 'Paused';
    if (running) return 'Capturing';
    return 'Not running';
  }
}

class RuntimeSettings {
  const RuntimeSettings({
    required this.retentionDays,
    required this.storageQuotaBytes,
    required this.excludedAppIds,
    required this.lanVisibility,
    required this.syncEnabled,
    required this.notifyOnCopy,
    this.notificationPreview = true,
    required this.soundOnCopy,
  });

  final int retentionDays;
  final int storageQuotaBytes;
  final List<String> excludedAppIds;
  final bool lanVisibility;
  final bool syncEnabled;
  final bool notifyOnCopy;
  final bool notificationPreview;
  final bool soundOnCopy;
}

class RuntimeSettingsChange {
  const RuntimeSettingsChange({
    this.retentionDays,
    this.storageQuotaBytes,
    this.excludedAppIds,
    this.lanVisibility,
    this.syncEnabled,
    this.notifyOnCopy,
    this.notificationPreview,
    this.soundOnCopy,
  });

  final int? retentionDays;
  final int? storageQuotaBytes;
  final List<String>? excludedAppIds;
  final bool? lanVisibility;
  final bool? syncEnabled;
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
