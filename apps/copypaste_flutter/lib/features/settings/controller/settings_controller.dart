import 'dart:async';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:flutter/foundation.dart';

import '../../../platform/capture/capture_service_control.dart';
import '../../../platform/notifications/capture_notification_port.dart';
import '../models/settings_models.dart';
import '../repository/settings_repository.dart';

class SettingsController extends ChangeNotifier {
  SettingsController({
    required SettingsRepository repository,
    required SettingsFilePicker filePicker,
    CaptureServiceControl captureControl =
        const PlatformCaptureServiceControl(),
    CaptureNotificationPort? notifications,
    this.captureRefreshInterval = const Duration(seconds: 3),
  }) : _repository = repository,
       _filePicker = filePicker,
       _captureControl = captureControl,
       _notifications = notifications ?? PlatformCaptureNotificationPort();

  final SettingsRepository _repository;
  final SettingsFilePicker _filePicker;
  final CaptureServiceControl _captureControl;
  final CaptureNotificationPort _notifications;
  final Duration captureRefreshInterval;

  SettingsLoadState _loadState = SettingsLoadState.loading;
  RuntimeSettings? _settings;
  CaptureSettingsState? _capture;
  CloudSettingsState? _cloud;
  CloudSyncResult? _lastCloudSync;
  String? _errorMessage;
  String? _cloudErrorMessage;
  String? _noticeMessage;
  bool _busy = false;
  bool _disposed = false;
  Timer? _captureTimer;
  StreamSubscription<void>? _captureSubscription;

  SettingsLoadState get loadState => _loadState;
  RuntimeSettings? get settings => _settings;
  CaptureSettingsState? get capture => _capture;
  CloudSettingsState? get cloud => _cloud;
  CloudSyncResult? get lastCloudSync => _lastCloudSync;
  String? get errorMessage => _errorMessage;
  String? get cloudErrorMessage => _cloudErrorMessage;
  String? get noticeMessage => _noticeMessage;
  bool get busy => _busy;

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
      ]);
      _settings = values[0] as RuntimeSettings;
      _capture = values[1] as CaptureSettingsState;
      _loadState = SettingsLoadState.ready;
      _errorMessage = null;
      _startCaptureRefresh();
      _captureSubscription ??= _repository.capturedEvents().listen(
        (_) {
          if (_settings?.notifyOnCopy ?? false) {
            unawaited(_notifications.showCaptured());
          }
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
    if (_loadState == SettingsLoadState.ready) {
      await refreshCloud();
    }
  }

  Future<void> retry() => initialize();

  Future<void> refreshCloud() async {
    try {
      _cloud = await _repository.cloudStatus();
      _cloudErrorMessage = null;
    } catch (error) {
      _cloudErrorMessage = _message(error, 'Cloud sync is unavailable.');
    }
    _notify();
  }

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
      _noticeMessage = _capture!.paused
          ? 'Clipboard capture paused.'
          : 'Clipboard capture resumed.';
    });
  }

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
      _noticeMessage = 'Settings saved.';
    });
  }

  Future<bool> setSoundOnCopy(bool value) =>
      _update(RuntimeSettingsChange(soundOnCopy: value));

  Future<bool> cloudSignIn({
    required String email,
    required String password,
    required String passphrase,
  }) => _cloudAccountAction(
    () => _repository.cloudSignIn(
      email: email.trim(),
      password: password,
      passphrase: passphrase,
    ),
    'Signed in to cloud sync.',
  );

  Future<bool> cloudSignUp({
    required String email,
    required String password,
    required String passphrase,
  }) => _cloudAccountAction(
    () => _repository.cloudSignUp(
      email: email.trim(),
      password: password,
      passphrase: passphrase,
    ),
    'Cloud sync account created.',
  );

  Future<bool> cloudSignOut() => _cloudAccountAction(
    _repository.cloudSignOut,
    'Signed out of cloud sync.',
  );

  Future<bool> cloudSyncNow() {
    return _run(() async {
      _lastCloudSync = await _repository.cloudSyncNow();
      _cloud = await _repository.cloudStatus();
      _cloudErrorMessage = null;
      final result = _lastCloudSync!;
      _noticeMessage =
          'Cloud sync complete: ${result.uploaded} uploaded, ${result.applied} applied.';
    }, cloud: true);
  }

  Future<bool> exportTextHistory() async {
    final path = await _filePicker.chooseTextExportPath();
    if (path == null) return false;
    return _run(() async {
      final result = await _repository.exportTextHistory(path);
      await _filePicker.presentCreatedFile(path, mimeType: 'application/json');
      _noticeMessage =
          result.skippedNonText == 0 && result.skippedUndecryptable == 0
          ? 'Exported ${result.exported} text clips.'
          : 'Exported ${result.exported}; skipped ${result.skippedNonText} non-text and ${result.skippedUndecryptable} unreadable clips.';
    });
  }

  Future<bool> createBackup() async {
    final path = await _filePicker.chooseBackupPath();
    if (path == null) return false;
    return _run(() async {
      final result = await _repository.backupHistory(path);
      await _filePicker.presentCreatedFile(
        path,
        mimeType: 'application/octet-stream',
      );
      _noticeMessage =
          'Encrypted backup created (${_formatBytes(result.sizeBytes)}).';
    });
  }

  Future<bool> restoreBackup() async {
    final path = await _filePicker.chooseRestorePath();
    if (path == null) return false;
    return _run(() async {
      await _repository.restoreHistory(path);
      _noticeMessage = 'History restored from the encrypted backup.';
    });
  }

  void clearMessages() {
    if (_errorMessage == null && _noticeMessage == null) return;
    _errorMessage = null;
    _noticeMessage = null;
    _notify();
  }

  Future<bool> _update(RuntimeSettingsChange change) {
    return _run(() async {
      _settings = await _repository.updateSettings(change);
      _noticeMessage = 'Settings saved.';
    });
  }

  Future<bool> _cloudAccountAction(
    Future<CloudSettingsState> Function() action,
    String successMessage,
  ) {
    return _run(() async {
      _cloud = await action();
      _cloudErrorMessage = null;
      _noticeMessage = successMessage;
    }, cloud: true);
  }

  Future<bool> _run(
    Future<void> Function() action, {
    bool cloud = false,
  }) async {
    if (_busy || _disposed) return false;
    _busy = true;
    _errorMessage = null;
    _noticeMessage = null;
    _notify();
    try {
      await action();
      return true;
    } catch (error) {
      final message = _message(
        error,
        'The requested change could not be completed.',
      );
      if (cloud) {
        _cloudErrorMessage = message;
      } else {
        _errorMessage = message;
      }
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
      if (_disposed || next.epoch < (_capture?.epoch ?? 0)) return;
      _capture = next;
      _notify();
    } catch (_) {
      // Keep the last authoritative state while a background refresh is unavailable.
    }
  }

  String _message(Object error, String fallback) =>
      error is runtime.RuntimeError
      ? error.message
      : error is CaptureServiceUnavailable
      ? 'Clipboard capture permissions must be completed before capture can resume.'
      : error is NotificationPermissionDenied
      ? 'Notification permission is required to enable capture notifications.'
      : fallback;

  String _formatBytes(int bytes) {
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    var value = bytes.toDouble();
    var unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit++;
    }
    final digits = value >= 10 || unit == 0 ? 0 : 1;
    return '${value.toStringAsFixed(digits)} ${units[unit]}';
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _captureTimer?.cancel();
    unawaited(_captureSubscription?.cancel());
    unawaited(_repository.dispose());
    super.dispose();
  }
}
