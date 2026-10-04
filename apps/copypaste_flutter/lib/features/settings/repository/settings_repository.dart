import '../models/settings_models.dart';

abstract interface class SettingsRepository {
  Stream<void> capturedEvents();

  Future<CaptureSettingsState> captureState();

  Future<CaptureSettingsState> setCapturePaused(bool paused);

  Future<RuntimeSettings> settings();

  Future<RuntimeSettings> updateSettings(RuntimeSettingsChange change);

  Future<CloudSettingsState> cloudStatus();

  Future<CloudSettingsState> cloudSignIn({
    required String email,
    required String password,
    required String passphrase,
  });

  Future<CloudSettingsState> cloudSignUp({
    required String email,
    required String password,
    required String passphrase,
  });

  Future<CloudSettingsState> cloudSignOut();

  Future<CloudSyncResult> cloudSyncNow();

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
