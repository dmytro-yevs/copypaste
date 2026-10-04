import 'dart:async';

import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/repository/settings_repository.dart';
import 'package:copypaste_flutter/platform/notifications/capture_notification_port.dart';

class FakeSettingsRepository implements SettingsRepository {
  final StreamController<void> captures = StreamController<void>.broadcast();
  CaptureSettingsState capture = const CaptureSettingsState(
    running: true,
    paused: false,
    epoch: 0,
  );
  RuntimeSettings currentSettings = const RuntimeSettings(
    retentionDays: 0,
    storageQuotaBytes: 10 * 1024 * 1024 * 1024,
    excludedAppIds: [],
    lanVisibility: true,
    syncEnabled: true,
    notifyOnCopy: false,
    soundOnCopy: false,
  );
  CloudSettingsState currentCloud = const CloudSettingsState(
    configured: true,
    signedIn: false,
    keyReady: false,
    unreadableUploads: 0,
  );
  int exportCalls = 0;
  int backupCalls = 0;
  int restoreCalls = 0;

  @override
  Stream<void> capturedEvents() => captures.stream;

  @override
  Future<BackupResult> backupHistory(String path) async {
    backupCalls += 1;
    return const BackupResult(sizeBytes: 2048);
  }

  @override
  Future<CaptureSettingsState> captureState() async => capture;

  @override
  Future<CloudSettingsState> cloudSignIn({
    required String email,
    required String password,
    required String passphrase,
  }) async {
    currentCloud = CloudSettingsState(
      configured: true,
      signedIn: true,
      keyReady: true,
      email: email,
      unreadableUploads: 0,
    );
    return currentCloud;
  }

  @override
  Future<CloudSettingsState> cloudSignOut() async {
    currentCloud = const CloudSettingsState(
      configured: true,
      signedIn: false,
      keyReady: false,
      unreadableUploads: 0,
    );
    return currentCloud;
  }

  @override
  Future<CloudSettingsState> cloudSignUp({
    required String email,
    required String password,
    required String passphrase,
  }) => cloudSignIn(email: email, password: password, passphrase: passphrase);

  @override
  Future<CloudSettingsState> cloudStatus() async => currentCloud;

  @override
  Future<CloudSyncResult> cloudSyncNow() async => const CloudSyncResult(
    uploaded: 2,
    tombstoned: 0,
    downloaded: 1,
    applied: 1,
    skippedUndecryptable: 0,
    skippedForged: 0,
    skippedFuture: 0,
    skippedTooLarge: 0,
  );

  @override
  Future<TextExportResult> exportTextHistory(String path) async {
    exportCalls += 1;
    return const TextExportResult(
      exported: 4,
      skippedNonText: 1,
      skippedUndecryptable: 0,
    );
  }

  @override
  Future<void> restoreHistory(String path) async {
    restoreCalls += 1;
  }

  @override
  Future<void> dispose() => captures.close();

  @override
  Future<CaptureSettingsState> setCapturePaused(bool paused) async {
    capture = CaptureSettingsState(
      running: !paused,
      paused: paused,
      epoch: capture.epoch + 1,
    );
    return capture;
  }

  @override
  Future<RuntimeSettings> settings() async => currentSettings;

  @override
  Future<RuntimeSettings> updateSettings(RuntimeSettingsChange change) async {
    currentSettings = RuntimeSettings(
      retentionDays: change.retentionDays ?? currentSettings.retentionDays,
      storageQuotaBytes:
          change.storageQuotaBytes ?? currentSettings.storageQuotaBytes,
      excludedAppIds: change.excludedAppIds ?? currentSettings.excludedAppIds,
      lanVisibility: change.lanVisibility ?? currentSettings.lanVisibility,
      syncEnabled: change.syncEnabled ?? currentSettings.syncEnabled,
      notifyOnCopy: change.notifyOnCopy ?? currentSettings.notifyOnCopy,
      soundOnCopy: change.soundOnCopy ?? currentSettings.soundOnCopy,
    );
    return currentSettings;
  }
}

class FakeSettingsFilePicker implements SettingsFilePicker {
  String? exportPath = '/tmp/history.json';
  String? backupPath = '/tmp/history.copypaste-backup';
  String? restorePath = '/tmp/restore.copypaste-backup';
  final List<String> presentedPaths = [];

  @override
  Future<String?> chooseBackupPath() async => backupPath;

  @override
  Future<String?> chooseRestorePath() async => restorePath;

  @override
  Future<String?> chooseTextExportPath() async => exportPath;

  @override
  Future<void> presentCreatedFile(
    String path, {
    required String mimeType,
  }) async {
    presentedPaths.add(path);
  }
}

class FakeCaptureNotificationPort implements CaptureNotificationPort {
  bool permissionGranted = true;
  int notifications = 0;

  @override
  Future<void> initialize() async {}

  @override
  Future<bool> requestPermission() async => permissionGranted;

  @override
  Future<void> showCaptured() async {
    notifications += 1;
  }
}
