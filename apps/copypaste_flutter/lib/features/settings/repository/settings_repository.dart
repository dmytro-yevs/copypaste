import '../models/settings_models.dart';
import '../models/sync_status.dart';
import '../../../platform/notifications/capture_notification_preview.dart';

/// Receives the processed units of a running operation out of its total, at
/// most a few times per second.
typedef DataTransferProgressCallback = void Function(int processed, int total);

abstract interface class SettingsRepository {
  Stream<SyncStatus> syncEvents();
  Stream<String?> capturedEvents();

  Future<CaptureNotificationPreview?> capturePreview(String id);

  Future<CaptureSettingsState> captureState();

  Future<CaptureSettingsState> setCapturePaused(bool paused);

  Future<RuntimeSettings> settings();

  Future<RuntimeSettings> updateSettings(RuntimeSettingsChange change);

  Future<TextExportResult> exportTextHistory(
    String path, {
    DataTransferProgressCallback? onProgress,
  });

  Future<BackupResult> backupHistory(
    String path, {
    DataTransferProgressCallback? onProgress,
  });

  Future<void> restoreHistory(String path);

  Future<void> dispose();
}

/// Optional typed capture stream. A null event marks a newly connected watch.
abstract interface class CaptureEventsRepository {
  Stream<CaptureSettingsState?> captureEvents();
}

/// Optional content-free signal that the retained history changed.
abstract interface class HistoryChangesRepository {
  Stream<void> historyChanges();
}

abstract interface class SettingsFilePicker {
  Future<String?> chooseTextExportPath();

  Future<String?> chooseBackupPath();

  Future<String?> chooseRestorePath();

  Future<void> presentCreatedFile(String path, {required String mimeType});
}
