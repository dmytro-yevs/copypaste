import 'dart:convert';
import 'dart:io';

import 'package:copypaste_flutter/generated/api.dart' as runtime;

import '../../../platform/notifications/capture_notification_preview.dart';

import '../models/settings_models.dart';
import 'settings_repository.dart';

class RuntimeSettingsRepository implements SettingsRepository {
  RuntimeSettingsRepository();

  @override
  Stream<String?> capturedEvents() async* {
    // Android feedback belongs to its capture service, including while Dart sleeps.
    if (Platform.isAndroid) return;
    final watchId = await runtime.allocateRuntimeWatch();
    try {
      await for (final event in runtime.watchRuntime(watchId: watchId)) {
        if (event.captured) yield event.capturedItemId;
      }
    } finally {
      await runtime.cancelRuntimeWatch(watchId: watchId);
    }
  }

  @override
  Future<CaptureNotificationPreview?> capturePreview(String id) async {
    final clip = await runtime.getClip(id: id);
    switch (clip.contentClass) {
      case runtime.ClipContentClass.image:
        final image = await runtime.clipImagePreview(id: id, maxEdge: 256);
        final details = clip.imageDetails;
        return CaptureNotificationPreview(
          text: details == null
              ? 'Image'
              : 'Image · ${details.width} × ${details.height}',
          imagePng: base64Decode(image.pngBase64),
        );
      case runtime.ClipContentClass.file:
        final details = clip.fileDetails;
        return CaptureNotificationPreview(
          text: CaptureNotificationPreview.textPreview(
            details?.filename ?? details?.sourceReference ?? 'File',
          ),
        );
      case runtime.ClipContentClass.text:
      case runtime.ClipContentClass.other:
        return CaptureNotificationPreview(
          text: CaptureNotificationPreview.textPreview(clip.content),
        );
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
        notificationPreview: change.notificationPreview,
        soundOnCopy: change.soundOnCopy,
      ),
    );
    return _settings(updated);
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
        notificationPreview: settings.notificationPreview,
        soundOnCopy: settings.soundOnCopy,
      );
}
