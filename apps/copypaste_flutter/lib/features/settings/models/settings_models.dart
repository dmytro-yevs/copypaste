enum SettingsLoadState { loading, ready, error }

class CaptureSettingsState {
  const CaptureSettingsState({
    required this.running,
    required this.paused,
    required this.epoch,
  });

  final bool running;
  final bool paused;
  final int epoch;

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
    required this.soundOnCopy,
  });

  final int retentionDays;
  final int storageQuotaBytes;
  final List<String> excludedAppIds;
  final bool lanVisibility;
  final bool syncEnabled;
  final bool notifyOnCopy;
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
    this.soundOnCopy,
  });

  final int? retentionDays;
  final int? storageQuotaBytes;
  final List<String>? excludedAppIds;
  final bool? lanVisibility;
  final bool? syncEnabled;
  final bool? notifyOnCopy;
  final bool? soundOnCopy;
}

class CloudSettingsState {
  const CloudSettingsState({
    required this.configured,
    required this.signedIn,
    required this.keyReady,
    this.email,
    this.lastSync,
    this.lastError,
    required this.unreadableUploads,
  });

  final bool configured;
  final bool signedIn;
  final bool keyReady;
  final String? email;
  final DateTime? lastSync;
  final String? lastError;
  final int unreadableUploads;
}

class CloudSyncResult {
  const CloudSyncResult({
    required this.uploaded,
    required this.tombstoned,
    required this.downloaded,
    required this.applied,
    required this.skippedUndecryptable,
    required this.skippedForged,
    required this.skippedFuture,
    required this.skippedTooLarge,
  });

  final int uploaded;
  final int tombstoned;
  final int downloaded;
  final int applied;
  final int skippedUndecryptable;
  final int skippedForged;
  final int skippedFuture;
  final int skippedTooLarge;
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
