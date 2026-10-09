import 'dart:async';

import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/models/sync_status.dart';
import 'package:copypaste_flutter/features/settings/repository/settings_repository.dart';
import 'package:copypaste_flutter/platform/notifications/capture_notification_port.dart';
import 'package:copypaste_flutter/platform/notifications/capture_notification_preview.dart';
import 'package:copypaste_flutter/platform/security/screenshot_protection.dart';

class FakeScreenshotProtection implements ScreenshotProtection {
  @override
  bool get supported => true;
  bool value = false;
  @override
  Future<bool> blocked() async => value;
  @override
  Future<void> setBlocked(bool blocked) async {
    value = blocked;
  }
}

class FakeSettingsRepository implements SettingsRepository {
  final syncChanges = StreamController<SyncStatus>.broadcast();

  @override
  Stream<SyncStatus> syncEvents() => syncChanges.stream;
  final StreamController<String?> captures =
      StreamController<String?>.broadcast();
  CaptureNotificationPreview? preview;
  int previewReads = 0;
  Future<CaptureNotificationPreview?> Function(String)? readPreview;
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
  int exportCalls = 0;
  int backupCalls = 0;
  int restoreCalls = 0;

  @override
  Stream<String?> capturedEvents() => captures.stream;

  @override
  Future<CaptureNotificationPreview?> capturePreview(String id) async {
    previewReads += 1;
    return readPreview == null ? preview : await readPreview!(id);
  }

  @override
  Future<BackupResult> backupHistory(String path) async {
    backupCalls += 1;
    return const BackupResult(sizeBytes: 2048);
  }

  @override
  Future<CaptureSettingsState> captureState() async => capture;

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
  Future<void> dispose() async {
    await captures.close();
    await syncChanges.close();
  }

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
      skipSecret: change.skipSecret ?? currentSettings.skipSecret,
      skipTransient: change.skipTransient ?? currentSettings.skipTransient,
      retentionDays: change.retentionDays ?? currentSettings.retentionDays,
      storageQuotaBytes:
          change.storageQuotaBytes ?? currentSettings.storageQuotaBytes,
      excludedAppIds: change.excludedAppIds ?? currentSettings.excludedAppIds,
      lanVisibility: change.lanVisibility ?? currentSettings.lanVisibility,
      syncEnabled: change.syncEnabled ?? currentSettings.syncEnabled,
      instantClipboard:
          change.instantClipboard ?? currentSettings.instantClipboard,
      notifyOnCopy: change.notifyOnCopy ?? currentSettings.notifyOnCopy,
      notificationPreview:
          change.notificationPreview ?? currentSettings.notificationPreview,
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
  final List<CaptureNotificationPreview?> previews = [];

  @override
  Future<void> initialize() async {}

  @override
  Future<bool> requestPermission() async => permissionGranted;

  @override
  Future<void> showCaptured({CaptureNotificationPreview? preview}) async {
    notifications += 1;
    previews.add(preview);
  }
}
