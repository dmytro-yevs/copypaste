import 'package:copypaste_flutter/generated/api.dart' as runtime;

import '../models/settings_models.dart';
import 'settings_repository.dart';

class RuntimeSettingsRepository implements SettingsRepository {
  RuntimeSettingsRepository();

  @override
  Stream<void> capturedEvents() async* {
    final watchId = await runtime.allocateRuntimeWatch();
    try {
      await for (final event in runtime.watchRuntime(watchId: watchId)) {
        if (event.captured) yield null;
      }
    } finally {
      await runtime.cancelRuntimeWatch(watchId: watchId);
    }
  }

  @override
  Future<CaptureSettingsState> captureState() async =>
      _captureState(await runtime.captureState());

  @override
  Future<CaptureSettingsState> setCapturePaused(bool paused) async =>
      _captureState(await runtime.setCapturePaused(paused: paused));

  @override
  Future<RuntimeSettings> settings() async =>
      _settings(await runtime.getRuntimeSettings());

  @override
  Future<RuntimeSettings> updateSettings(RuntimeSettingsChange change) async {
    final updated = await runtime.updateRuntimeSettings(
      patch: runtime.RuntimeSettingsPatch(
        retentionDays: change.retentionDays,
        storageQuotaBytes: change.storageQuotaBytes == null
            ? null
            : BigInt.from(change.storageQuotaBytes!),
        excludedAppIds: change.excludedAppIds,
        lanVisibility: change.lanVisibility,
        syncEnabled: change.syncEnabled,
        notifyOnCopy: change.notifyOnCopy,
        soundOnCopy: change.soundOnCopy,
      ),
    );
    return _settings(updated);
  }

  @override
  Future<CloudSettingsState> cloudStatus() async =>
      _cloud(await runtime.cloudAccountStatus());

  @override
  Future<CloudSettingsState> cloudSignIn({
    required String email,
    required String password,
    required String passphrase,
  }) async => _cloud(
    await runtime.cloudSignIn(
      email: email,
      password: password,
      passphrase: passphrase,
    ),
  );

  @override
  Future<CloudSettingsState> cloudSignUp({
    required String email,
    required String password,
    required String passphrase,
  }) async => _cloud(
    await runtime.cloudSignUp(
      email: email,
      password: password,
      passphrase: passphrase,
    ),
  );

  @override
  Future<CloudSettingsState> cloudSignOut() async =>
      _cloud(await runtime.cloudSignOut());

  @override
  Future<CloudSyncResult> cloudSyncNow() async {
    final result = await runtime.cloudSyncNow();
    return CloudSyncResult(
      uploaded: result.uploaded,
      tombstoned: result.tombstoned,
      downloaded: result.downloaded,
      applied: result.applied,
      skippedUndecryptable: result.skippedUndecryptable,
      skippedForged: result.skippedForged,
      skippedFuture: result.skippedFuture,
      skippedTooLarge: result.skippedTooLarge,
    );
  }

  @override
  Future<TextExportResult> exportTextHistory(String path) async {
    final result = await runtime.exportTextHistory(filePath: path);
    return TextExportResult(
      exported: result.exported,
      skippedNonText: result.skippedNonText,
      skippedUndecryptable: result.skippedUndecryptable,
    );
  }

  @override
  Future<BackupResult> backupHistory(String path) async {
    final result = await runtime.backupHistory(filePath: path);
    return BackupResult(sizeBytes: result.sizeBytes.toInt());
  }

  @override
  Future<void> restoreHistory(String path) =>
      runtime.restoreHistory(filePath: path);

  @override
  Future<void> dispose() async {}

  CaptureSettingsState _captureState(runtime.CaptureState state) =>
      CaptureSettingsState(
        running: state.running,
        paused: state.paused,
        epoch: state.privateModeEpoch.toInt(),
      );

  RuntimeSettings _settings(runtime.RuntimeSettingsData settings) =>
      RuntimeSettings(
        retentionDays: settings.retentionDays,
        storageQuotaBytes: settings.storageQuotaBytes.toInt(),
        excludedAppIds: List.unmodifiable(settings.excludedAppIds),
        lanVisibility: settings.lanVisibility,
        syncEnabled: settings.syncEnabled,
        notifyOnCopy: settings.notifyOnCopy,
        soundOnCopy: settings.soundOnCopy,
      );

  CloudSettingsState _cloud(runtime.CloudAccountStatus status) =>
      CloudSettingsState(
        configured: status.configured,
        signedIn: status.signedIn,
        keyReady: status.keyReady,
        email: status.email,
        lastSync: status.lastSyncMs == null
            ? null
            : DateTime.fromMillisecondsSinceEpoch(
                status.lastSyncMs!.toInt(),
                isUtc: true,
              ),
        lastError: status.lastError,
        unreadableUploads: status.unreadableUploads,
      );
}
