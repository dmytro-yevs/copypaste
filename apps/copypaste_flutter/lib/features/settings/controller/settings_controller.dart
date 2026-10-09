import 'dart:async';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:flutter/foundation.dart';

import '../../../platform/capture/capture_service_control.dart';
import '../../../platform/android/screenshot_capture.dart';
import '../../../platform/notifications/capture_notification_port.dart';
import '../../../platform/notifications/capture_notification_preview.dart';
import '../../../platform/security/screenshot_protection.dart';
import '../models/settings_models.dart';
import 'settings_navigation_state.dart';
import '../models/sync_status.dart';
import '../repository/settings_repository.dart';

class SettingsController extends ChangeNotifier {
  SettingsController({
    required SettingsRepository repository,
    required SettingsFilePicker filePicker,
    CaptureServiceControl captureControl =
        const PlatformCaptureServiceControl(),
    CaptureNotificationPort? notifications,
    ScreenshotProtection screenshotProtection =
        const MethodChannelScreenshotProtection(),
    ScreenshotCapture screenshotCapture =
        const MethodChannelScreenshotCapture(),
    this.captureRefreshInterval = const Duration(seconds: 3),
  }) : _repository = repository,
       _filePicker = filePicker,
       _captureControl = captureControl,
       _notifications = notifications ?? PlatformCaptureNotificationPort(),
       _screenshotProtection = screenshotProtection,
       _screenshotCapture = screenshotCapture;

  final navigation = SettingsNavigationState();

  final SettingsRepository _repository;
  final SettingsFilePicker _filePicker;
  final CaptureServiceControl _captureControl;
  final CaptureNotificationPort _notifications;
  final ScreenshotProtection _screenshotProtection;
  final ScreenshotCapture _screenshotCapture;
  ScreenshotCaptureStatus _screenshotStatus = const ScreenshotCaptureStatus();
  final Duration captureRefreshInterval;

  SettingsLoadState _loadState = SettingsLoadState.loading;
  RuntimeSettings? _settings;
  CaptureSettingsState? _capture;
  SyncStatus _syncStatus = const SyncStatus();
  StreamSubscription<SyncStatus>? _syncSubscription;
  String? _errorMessage;
  bool _busy = false;
  bool _blockScreenshots = false;
  bool _disposed = false;
  Timer? _captureTimer;
  StreamSubscription<String?>? _captureSubscription;
  Future<void> _notificationQueue = Future<void>.value();

  SettingsLoadState get loadState => _loadState;
  RuntimeSettings? get settings => _settings;
  CaptureSettingsState? get capture => _capture;
  SyncStatus get syncStatus => _syncStatus;
  String? get errorMessage => _errorMessage;
  bool get busy => _busy;
  bool get blockScreenshots => _blockScreenshots;
  bool get blockScreenshotsSupported => _screenshotProtection.supported;
  bool get screenshotCaptureSupported => _screenshotCapture.supported;
  ScreenshotCaptureStatus get screenshotCapture => _screenshotStatus;

  Future<bool> setScreenshotCaptureEnabled(bool value) => _run(() async {
    _screenshotStatus = await _screenshotCapture.setEnabled(value);
  });

  Future<bool> requestScreenshotCapturePermission() => _run(() async {
    _screenshotStatus = await _screenshotCapture.requestPermission();
  });

  Future<void> initialize() async {
    if (_disposed) return;
    _loadState = SettingsLoadState.loading;
    _notify();
    try {
      try {
        await _notifications.initialize();
      } catch (_) {
        // Notification setup is optional until the user enables it.
      }
      final values = await Future.wait<Object>([
        _repository.settings(),
        _repository.captureState(),
        _screenshotProtection.supported
            ? _screenshotProtection.blocked()
            : Future<bool>.value(false),
      ]);
      _settings = values[0] as RuntimeSettings;
      _capture = values[1] as CaptureSettingsState;
      _acceptSyncStatus(_capture!.syncStatus);
      _blockScreenshots = values[2] as bool;
      if (_screenshotCapture.supported) {
        _screenshotStatus = await _screenshotCapture.status();
      }
      _loadState = SettingsLoadState.ready;
      _errorMessage = null;
      _startCaptureRefresh();
      _syncSubscription ??= _repository.syncEvents().listen(
        (status) {
          _acceptSyncStatus(status);
          _notify();
        },
        onError: (Object error) {
          _invalidateSyncStatus();
          _notify();
        },
      );
      _captureSubscription ??= _repository.capturedEvents().listen(
        (id) {
          _notificationQueue = _notificationQueue.then(
            (_) => _showCaptureNotification(id),
          );
        },
        onError: (Object _, StackTrace _) {
          // History remains usable when the optional feedback stream ends.
        },
      );
    } catch (error) {
      _loadState = SettingsLoadState.error;
      _errorMessage = _message(error, 'Settings could not be loaded.');
    }
    _notify();
  }

  Future<void> retry() => initialize();

  Future<bool> toggleCapture() async {
    final capture = _capture;
    if (capture == null || _busy) return false;
    return _run(() async {
      final paused = !capture.paused;
      _capture = await _repository.setCapturePaused(paused);
      try {
        await _captureControl.setPaused(paused);
      } on CaptureServiceUnavailable {
        _capture = await _repository.setCapturePaused(true);
        throw const CaptureServiceUnavailable();
      }
      _capture = await _repository.captureState();
    });
  }

  Future<bool> setSkipSecret(bool value) =>
      _update(RuntimeSettingsChange(skipSecret: value));
  Future<bool> setSkipTransient(bool value) =>
      _update(RuntimeSettingsChange(skipTransient: value));

  Future<bool> setRetentionDays(int value) =>
      _update(RuntimeSettingsChange(retentionDays: value));

  Future<bool> setStorageQuotaBytes(int value) =>
      _update(RuntimeSettingsChange(storageQuotaBytes: value));

  Future<bool> addExcludedApp(String rawValue) async {
    final value = rawValue.trim();
    final settings = _settings;
    if (settings == null || value.isEmpty) return false;
    if (settings.excludedAppIds.contains(value)) {
      _errorMessage = 'That application is already excluded.';
      _notify();
      return false;
    }
    return _update(
      RuntimeSettingsChange(
        excludedAppIds: [...settings.excludedAppIds, value],
      ),
    );
  }

  Future<bool> removeExcludedApp(String value) {
    final settings = _settings;
    if (settings == null) return Future.value(false);
    return _update(
      RuntimeSettingsChange(
        excludedAppIds: settings.excludedAppIds
            .where((item) => item != value)
            .toList(growable: false),
      ),
    );
  }

  Future<bool> setLanVisibility(bool value) =>
      _update(RuntimeSettingsChange(lanVisibility: value));

  Future<bool> setInstantClipboard(bool value) =>
      _update(RuntimeSettingsChange(instantClipboard: value));

  Future<bool> setSyncEnabled(bool value) =>
      _update(RuntimeSettingsChange(syncEnabled: value));

  Future<bool> setNotifyOnCopy(bool value) {
    return _run(() async {
      if (value && !await _notifications.requestPermission()) {
        throw const NotificationPermissionDenied();
      }
      _settings = await _repository.updateSettings(
        RuntimeSettingsChange(notifyOnCopy: value),
      );
    });
  }

  Future<bool> setSoundOnCopy(bool value) =>
      _update(RuntimeSettingsChange(soundOnCopy: value));

  Future<bool> setNotificationPreview(bool value) =>
      _update(RuntimeSettingsChange(notificationPreview: value));

  Future<void> _showCaptureNotification(String? id) async {
    if (_disposed || !(_settings?.notifyOnCopy ?? false)) return;
    CaptureNotificationPreview? preview;
    try {
      if ((_settings?.notificationPreview ?? false) && id != null) {
        preview = await _repository.capturePreview(id);
      }
    } catch (_) {
      // Deleted clips and unavailable previews still allow generic feedback.
    }
    if (_disposed || !(_settings?.notifyOnCopy ?? false)) return;
    try {
      await _notifications.showCaptured(
        preview: (_settings?.notificationPreview ?? false) ? preview : null,
      );
    } catch (_) {
      // Optional system feedback must not interrupt clipboard capture.
    }
  }

  Future<bool> setBlockScreenshots(bool value) => _run(() async {
    if (!_screenshotProtection.supported) {
      throw UnsupportedError('Screenshot blocking is unavailable.');
    }
    await _screenshotProtection.setBlocked(value);
    _blockScreenshots = await _screenshotProtection.blocked();
  });

  Future<bool> exportTextHistory() async {
    final path = await _filePicker.chooseTextExportPath();
    if (path == null) return false;
    return _run(() async {
      await _repository.exportTextHistory(path);
      await _filePicker.presentCreatedFile(path, mimeType: 'application/json');
    });
  }

  Future<bool> createBackup() async {
    final path = await _filePicker.chooseBackupPath();
    if (path == null) return false;
    return _run(() async {
      await _repository.backupHistory(path);
      await _filePicker.presentCreatedFile(
        path,
        mimeType: 'application/octet-stream',
      );
    });
  }

  Future<bool> restoreBackup() async {
    final path = await _filePicker.chooseRestorePath();
    if (path == null) return false;
    return _run(() async {
      await _repository.restoreHistory(path);
    });
  }

  void clearMessages() {
    if (_errorMessage == null) return;
    _errorMessage = null;
    _notify();
  }

  Future<bool> _update(RuntimeSettingsChange change) {
    return _run(() async {
      _settings = await _repository.updateSettings(change);
    });
  }

  Future<bool> _run(Future<void> Function() action) async {
    if (_busy || _disposed) return false;
    _busy = true;
    _errorMessage = null;
    _notify();
    try {
      await action();
      return true;
    } catch (error) {
      final message = _message(
        error,
        'The requested change could not be completed.',
      );
      _errorMessage = message;
      return false;
    } finally {
      _busy = false;
      _notify();
    }
  }

  void _startCaptureRefresh() {
    _captureTimer?.cancel();
    if (captureRefreshInterval <= Duration.zero) return;
    _captureTimer = Timer.periodic(captureRefreshInterval, (_) {
      unawaited(_refreshCapture());
    });
  }

  Future<void> _refreshCapture() async {
    if (_disposed || _busy) return;
    try {
      final next = await _repository.captureState();
      if (_screenshotCapture.supported) {
        _screenshotStatus = await _screenshotCapture.status();
      }
      if (_disposed || next.epoch < (_capture?.epoch ?? 0)) return;
      _capture = next;
      _acceptSyncStatus(next.syncStatus);
      _notify();
    } catch (_) {
      _invalidateSyncStatus();
      _notify();
    }
  }

  void _acceptSyncStatus(SyncStatus status) {
    if (_disposed || status.revision < _syncStatus.revision) return;
    _syncStatus = status;
  }

  void _invalidateSyncStatus() {
    _syncStatus = SyncStatus(
      revision: _syncStatus.revision,
      peers: _syncStatus.peers,
    );
  }

  String _message(Object error, String fallback) =>
      error is runtime.RuntimeError
      ? _runtimeMessage(error) ?? error.message
      : error is CaptureServiceUnavailable
      ? 'Clipboard capture permissions must be completed before capture can resume.'
      : error is NotificationPermissionDenied
      ? 'Notification permission is required to enable capture notifications.'
      : fallback;

  String? _runtimeMessage(runtime.RuntimeError error) {
    if (error.code != 'invalid_request') return null;

    return switch (error.message) {
      'excluded_app_bundle_ids contains an entry that is empty or too long' =>
        'Application identifiers must be non-empty and 256 bytes or fewer.',
      'excluded_app_bundle_ids has too many entries; at most 256 are allowed' =>
        'You can exclude up to 256 applications.',
      _ => null,
    };
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _captureTimer?.cancel();
    unawaited(_captureSubscription?.cancel());
    unawaited(_syncSubscription?.cancel());
    unawaited(_repository.dispose());
    super.dispose();
  }
}
