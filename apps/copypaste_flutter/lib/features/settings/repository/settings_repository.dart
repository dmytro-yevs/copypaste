import '../models/settings_models.dart';

abstract interface class SettingsRepository {
  Stream<void> capturedEvents();

  Future<CaptureSettingsState> captureState();

  Future<CaptureSettingsState> setCapturePaused(bool paused);

  Future<RuntimeSettings> settings();

  Future<RuntimeSettings> updateSettings(RuntimeSettingsChange change);

  Future<TextExportResult> exportTextHistory(String path);

  Future<BackupResult> backupHistory(String path);

  Future<void> restoreHistory(String path);

  Future<void> dispose();
}

abstract interface class SettingsFilePicker {
  Future<String?> chooseTextExportPath();

  Future<String?> chooseBackupPath();

  Future<String?> chooseRestorePath();

  Future<void> presentCreatedFile(String path, {required String mimeType});
}
