import 'dart:async';
import 'dart:io';

import 'package:copypaste_flutter/generated/api.dart' as runtime;

import '../../../platform/notifications/capture_notification_preview.dart';
import '../../../shared/runtime_event_service.dart';

import '../models/settings_models.dart';
import '../models/sync_status.dart';
import 'settings_repository.dart';

class RuntimeSettingsRepository
    implements SettingsRepository, CaptureEventsRepository {
  RuntimeSettingsRepository();

  @override
  Stream<SyncStatus> syncEvents() => RuntimeEventService.instance.events
      .where((event) => event.syncStatus != null)
      .map((event) => _syncStatus(event.syncStatus!));

  @override
  Stream<String?> capturedEvents() {
    // Android feedback belongs to its capture service, including while Dart sleeps.
    if (Platform.isAndroid) return const Stream.empty();
    return RuntimeEventService.instance.events
        .where((event) => event.captured)
        .map((event) => event.capturedItemId);
  }

  @override
  Stream<CaptureSettingsState?> captureEvents() async* {
    final service = RuntimeEventService.instance;
    final pending = StreamController<CaptureSettingsState?>();
    var active = true;
    final subscription = service.events.listen((event) {
      if (event.kind == 'ready') {
        pending.add(null);
      } else if (event.capture case final capture?) {
        pending.add(_captureState(capture));
      }
    }, onError: pending.addError);
    // Another feature may have connected the shared watch before Settings
    // subscribed. Read the authoritative state after joining either way.
    unawaited(
      service.ready.then((_) {
        if (active) pending.add(null);
      }, onError: (Object _, StackTrace _) {}),
    );
    try {
      yield* pending.stream;
    } finally {
      active = false;
      await subscription.cancel();
      await pending.close();
    }
  }

  @override
  Future<CaptureNotificationPreview?> capturePreview(String id) async {
    final clip = await runtime.getClip(id: id);
    if (clip.secret) return null;
    switch (clip.contentClass) {
      case runtime.ClipContentClass.image:
        final image = await runtime.clipImagePreview(id: id, maxEdge: 256);
        final details = clip.imageDetails;
        return CaptureNotificationPreview(
          text: details == null
              ? 'Image'
              : 'Image · ${details.width} × ${details.height}',
          imagePng: image.pngBytes,
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
        maxFileSizeBytes: change.maxFileSizeBytes == null
            ? null
            : BigInt.from(change.maxFileSizeBytes!),
        skipSecret: change.skipSecret,
        skipTransient: change.skipTransient,
        retentionDays: change.retentionDays,
        storageQuotaBytes: change.storageQuotaBytes == null
            ? null
            : BigInt.from(change.storageQuotaBytes!),
        excludedAppIds: change.excludedAppIds,
        lanVisibility: change.lanVisibility,
        syncEnabled: change.syncEnabled,
        instantClipboard: change.instantClipboard,
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
        syncStatus: _syncStatus(state.syncStatus),
        captureWatch: state.captureWatch && state.runtimeWatch,
        captureRevision: state.captureRevision.toInt(),
        running: state.running,
        paused: state.paused,
        epoch: state.privateModeEpoch.toInt(),
        settingsHealth: switch (state.settingsHealth) {
          final health? => RuntimeSettingsHealth(
            recordUnreadable: health.recordUnreadable,
            unreadableFields: List.unmodifiable(health.unreadableFields),
          ),
          null => null,
        },
      );

  SyncStatus _syncStatus(runtime.RuntimeSyncStatus status) => SyncStatus(
    revision: status.revision.toInt(),
    phase: _syncPhase(status.phase),
    peers: status.peers
        .map(
          (peer) => PeerSyncStatus(
            id: peer.pairingId,
            name: peer.name,
            phase: _syncPhase(peer.phase),
            startedAt: _syncTime(peer.startedAtMs?.toInt()),
            lastSuccess: _syncTime(peer.lastSuccessMs?.toInt()),
            sent: peer.sent.toInt(),
            received: peer.received.toInt(),
            skippedTooLarge: peer.skippedTooLarge.toInt(),
            error: peer.error,
          ),
        )
        .toList(growable: false),
  );

  DateTime? _syncTime(int? value) =>
      value == null ? null : DateTime.fromMillisecondsSinceEpoch(value.toInt());

  SyncPhase _syncPhase(runtime.SyncPhase phase) => switch (phase) {
    runtime.SyncPhase.unavailable => SyncPhase.unavailable,
    runtime.SyncPhase.disabled => SyncPhase.disabled,
    runtime.SyncPhase.waiting => SyncPhase.waiting,
    runtime.SyncPhase.syncing => SyncPhase.syncing,
    runtime.SyncPhase.synced => SyncPhase.synced,
    runtime.SyncPhase.failed => SyncPhase.failed,
  };

  RuntimeSettings _settings(runtime.RuntimeSettingsData settings) =>
      RuntimeSettings(
        maxFileSizeBytes: settings.maxFileSizeBytes.toInt(),
        retentionDays: settings.retentionDays,
        storageQuotaBytes: settings.storageQuotaBytes.toInt(),
        skipSecret: settings.skipSecret,
        skipTransient: settings.skipTransient,
        excludedAppIds: List.unmodifiable(settings.excludedAppIds),
        lanVisibility: settings.lanVisibility,
        syncEnabled: settings.syncEnabled,
        instantClipboard: settings.instantClipboard,
        notifyOnCopy: settings.notifyOnCopy,
        notificationPreview: settings.notificationPreview,
        soundOnCopy: settings.soundOnCopy,
      );
}
