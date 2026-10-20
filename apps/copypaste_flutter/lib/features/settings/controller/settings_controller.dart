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
    this.backgroundCaptureStatusRequired = false,
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

  /// Desktop tray controls still need authoritative capture state when hidden.
  /// Remove this fallback only when runtime capture-state push events exist.
  final bool backgroundCaptureStatusRequired;

  SettingsLoadState _loadState = SettingsLoadState.loading;
  RuntimeSettings? _settings;
  CaptureSettingsState? _capture;
  CaptureSettingsState? _captureSnapshot;
  SyncStatus _syncStatus = const SyncStatus();
  SyncStatus _syncSnapshot = const SyncStatus();
  StreamSubscription<SyncStatus>? _syncSubscription;
  String? _errorMessage;
  String? _settingsHealthNotice;
  bool _busy = false;
  bool _blockScreenshots = false;
  bool _disposed = false;
  Timer? _captureTimer;
  bool _presentationActive = true;
  bool _captureRefreshInFlight = false;
  int _presentationEpoch = 0;
  StreamSubscription<String?>? _captureSubscription;
  StreamSubscription<CaptureSettingsState?>? _captureStateSubscription;
  bool _capturePushActive = false;
  bool _captureWatchReady = false;
  bool _captureStreamConnected = false;
  bool _screenshotEventsReady = false;
  int _screenshotEventGeneration = 0;
  int _screenshotWatchGeneration = 0;
  StreamSubscription<ScreenshotCaptureStatus>? _screenshotSubscription;
  int _captureRevision = -1;
  int _captureWatchGeneration = 0;
  bool _captureEpochReset = false;
  bool _syncRevisionReset = false;
  Future<void> _notificationQueue = Future<void>.value();

  SettingsLoadState get loadState => _loadState;
  RuntimeSettings? get settings => _settings;
  CaptureSettingsState? get capture => _capture;
  SyncStatus get syncStatus => _syncStatus;
  String? get errorMessage => _errorMessage ?? _settingsHealthNotice;
  bool get busy => _busy;
  bool get blockScreenshots => _blockScreenshots;
  bool get blockScreenshotsSupported => _screenshotProtection.supported;
  bool get screenshotCaptureSupported => _screenshotCapture.supported;
  ScreenshotCaptureStatus get screenshotCapture => _screenshotStatus;

  Future<bool> setScreenshotCaptureEnabled(bool value) => _run(() async {
    try {
      _screenshotStatus = await _screenshotCapture.setEnabled(value);
    } catch (_) {
      try {
        _screenshotStatus = await _screenshotCapture.status();
      } catch (_) {
        // Keep the last confirmed status when the native state cannot be read.
      }
      rethrow;
    }
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
      if (_disposed) return;
      _settings = values[0] as RuntimeSettings;
      _setCapture(values[1] as CaptureSettingsState);
      _acceptSyncStatus(_capture!.syncStatus);
      _settingsHealthNotice = _settingsHealthMessage(_capture!.settingsHealth);
      _blockScreenshots = values[2] as bool;
      if (_screenshotCapture.supported) {
        _screenshotStatus = await _screenshotCapture.status();
      }
      if (_disposed) return;
      _loadState = SettingsLoadState.ready;
      _errorMessage = null;
      if (_repository case final CaptureEventsRepository captureEvents) {
        _captureStateSubscription ??= captureEvents.captureEvents().listen(
          _acceptCaptureEvent,
          onError: (Object _, StackTrace _) {
            _captureStreamConnected = false;
            _captureWatchReady = false;
            _updateCapturePush();
            unawaited(_refreshCapture());
          },
        );
      }
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
      _setCapture(await _repository.setCapturePaused(paused));
      try {
        await _captureControl.setPaused(paused);
      } on CaptureServiceUnavailable {
        try {
          _setCapture(await _repository.setCapturePaused(true));
        } catch (_) {
          // The authoritative read below reports whether compensation persisted.
        } finally {
          try {
            _setCapture(await _repository.captureState());
          } catch (_) {
            // Retain the latest known persisted state when the runtime is unavailable.
          }
        }
        throw const CaptureServiceUnavailable();
      } catch (_) {
        try {
          _setCapture(await _repository.captureState());
        } catch (_) {
          // The persisted result above remains the best available state.
        }
        if (paused) throw const CaptureServiceStopFailed();
        rethrow;
      }
      _setCapture(await _repository.captureState());
      _settingsHealthNotice = _settingsHealthMessage(_capture!.settingsHealth);
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

  Future<bool> setMaxFileSizeBytes(int value) =>
      _update(RuntimeSettingsChange(maxFileSizeBytes: value));

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
      _settingsHealthNotice = null;
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

  Future<TextExportResult?> exportTextHistory() async {
    final path = await _filePicker.chooseTextExportPath();
    if (path == null) return null;
    TextExportResult? result;
    final completed = await _run(() async {
      result = await _repository.exportTextHistory(path);
      await _filePicker.presentCreatedFile(path, mimeType: 'application/json');
    });
    return completed ? result : null;
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

  /// Returns null when the native picker is cancelled without restoring.
  Future<bool?> restoreBackup() async {
    var cancelled = false;
    final restored = await _run(() async {
      final path = await _filePicker.chooseRestorePath();
      if (path == null) {
        cancelled = true;
        return;
      }
      await _repository.restoreHistory(path);
    });
    return cancelled ? null : restored;
  }

  void clearMessages() {
    if (_errorMessage == null) return;
    _errorMessage = null;
    _notify();
  }

  Future<bool> _update(RuntimeSettingsChange change) {
    return _run(() async {
      _settings = await _repository.updateSettings(change);
      _settingsHealthNotice = null;
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

  /// Presentation polling is optional; capture and notification streams remain
  /// active while hidden. A desktop tray may retain capture-state polling.
  void setPresentationActive(bool active) {
    if (_disposed || active == _presentationActive) return;
    _presentationActive = active;
    _presentationEpoch++;
    _updateCapturePush();
    _startCaptureRefresh();
    if ((active || backgroundCaptureStatusRequired) &&
        _loadState == SettingsLoadState.ready) {
      unawaited(_refreshCapture());
    }
  }

  void _startCaptureRefresh() {
    _captureTimer?.cancel();
    _captureTimer = null;
    if (_disposed ||
        (!_presentationActive && !backgroundCaptureStatusRequired) ||
        _loadState != SettingsLoadState.ready ||
        captureRefreshInterval <= Duration.zero ||
        _capturePushActive) {
      return;
    }
    _captureTimer = Timer.periodic(captureRefreshInterval, (_) {
      unawaited(_refreshCapture());
    });
  }

  void _acceptCaptureEvent(CaptureSettingsState? next) {
    if (_disposed) return;
    if (next == null) {
      // A new daemon can restart its revision at zero.
      _captureRevision = -1;
      _captureWatchGeneration++;
      _captureEpochReset = true;
      _syncRevisionReset = true;
      _captureStreamConnected = true;
      _captureWatchReady = false;
      _updateCapturePush();
      unawaited(_refreshCapture());
      return;
    }
    if (next.captureRevision < _captureRevision ||
        (!_captureEpochReset &&
            next.captureRevision == _captureRevision &&
            next.epoch < (_capture?.epoch ?? 0))) {
      return;
    }
    _captureRevision = next.captureRevision;
    final previousCapture = _captureSnapshot;
    final previousSync = _syncSnapshot;
    final previousNotice = _settingsHealthNotice;
    _setCapture(next);
    _captureEpochReset = false;
    _acceptSyncStatus(next.syncStatus);
    _settingsHealthNotice = _settingsHealthMessage(next.settingsHealth);
    _captureWatchReady = _captureStreamConnected && next.captureWatch;
    _updateCapturePush();
    if (!_sameCapture(previousCapture, _captureSnapshot) ||
        !_sameSync(previousSync, _syncSnapshot) ||
        previousNotice != _settingsHealthNotice) {
      _notify();
    }
  }

  void _acceptScreenshotEvent(ScreenshotCaptureStatus next) {
    if (_disposed) return;
    final previous = _screenshotStatus;
    _screenshotEventGeneration++;
    _screenshotStatus = next;
    _screenshotEventsReady = true;
    _updateCapturePush();
    if (!_sameScreenshot(previous, next)) _notify();
  }

  void _updateCapturePush() {
    final screenshotRequired =
        _screenshotCapture.supported && _presentationActive;
    if (_captureWatchReady &&
        screenshotRequired &&
        _screenshotSubscription == null) {
      final screenshotEvents = _screenshotCapture;
      if (screenshotEvents is ScreenshotCaptureEvents) {
        final generation = ++_screenshotWatchGeneration;
        _screenshotSubscription = (screenshotEvents as ScreenshotCaptureEvents)
            .changes
            .listen(
              _acceptScreenshotEvent,
              onError: (Object _, StackTrace _) {
                if (generation != _screenshotWatchGeneration) return;
                _screenshotEventsReady = false;
                _updateCapturePush();
              },
              onDone: () {
                if (generation != _screenshotWatchGeneration) return;
                _screenshotEventsReady = false;
                _updateCapturePush();
              },
            );
      }
    } else if ((!_captureWatchReady || !screenshotRequired) &&
        _screenshotSubscription != null) {
      _screenshotWatchGeneration++;
      final subscription = _screenshotSubscription;
      _screenshotSubscription = null;
      _screenshotEventsReady = false;
      unawaited(subscription?.cancel());
    }
    final active =
        _captureWatchReady && (!screenshotRequired || _screenshotEventsReady);
    if (active == _capturePushActive) return;
    _capturePushActive = active;
    _startCaptureRefresh();
  }

  Future<void> _refreshCapture() async {
    if (_disposed ||
        _busy ||
        (!_presentationActive && !backgroundCaptureStatusRequired) ||
        _captureRefreshInFlight) {
      return;
    }
    _captureRefreshInFlight = true;
    final epoch = _presentationEpoch;
    final watchGeneration = _captureWatchGeneration;
    final captureRevision = _captureRevision;
    final screenshotGeneration = _screenshotEventGeneration;
    try {
      final next = await _repository.captureState();
      if (_disposed || epoch != _presentationEpoch) {
        return;
      }
      var screenshotStatus = _screenshotStatus;
      if (_presentationActive && _screenshotCapture.supported) {
        screenshotStatus = await _screenshotCapture.status();
      }
      if (_disposed ||
          _busy ||
          epoch != _presentationEpoch ||
          watchGeneration != _captureWatchGeneration ||
          (!_captureEpochReset && next.epoch < (_capture?.epoch ?? 0)) ||
          next.captureRevision < _captureRevision) {
        return;
      }
      final previousCapture = _captureSnapshot;
      final previousSync = _syncSnapshot;
      final previousScreenshot = _screenshotStatus;
      final previousNotice = _settingsHealthNotice;
      if (screenshotGeneration == _screenshotEventGeneration) {
        _screenshotStatus = screenshotStatus;
      }
      _captureRevision = next.captureRevision;
      _setCapture(next);
      _captureEpochReset = false;
      _captureWatchReady = _captureStreamConnected && next.captureWatch;
      _updateCapturePush();
      _acceptSyncStatus(next.syncStatus);
      _settingsHealthNotice = _settingsHealthMessage(next.settingsHealth);
      if (!_sameCapture(previousCapture, _captureSnapshot) ||
          !_sameSync(previousSync, _syncSnapshot) ||
          !_sameScreenshot(previousScreenshot, _screenshotStatus) ||
          previousNotice != _settingsHealthNotice) {
        _notify();
      }
    } catch (_) {
      if (!_disposed &&
          epoch == _presentationEpoch &&
          watchGeneration == _captureWatchGeneration &&
          captureRevision == _captureRevision) {
        _invalidateSyncStatus();
        _notify();
      }
    } finally {
      _captureRefreshInFlight = false;
      // A resumed window may have inherited an outstanding hidden request.
      if (!_disposed &&
          (_presentationActive || backgroundCaptureStatusRequired) &&
          (epoch != _presentationEpoch ||
              watchGeneration != _captureWatchGeneration)) {
        unawaited(_refreshCapture());
      }
    }
  }

  void _acceptSyncStatus(SyncStatus status) {
    if (_disposed ||
        (!_syncRevisionReset && status.revision < _syncStatus.revision)) {
      return;
    }
    _syncRevisionReset = false;
    _syncStatus = status;
    _syncSnapshot = _snapshotSync(status);
  }

  void _setCapture(CaptureSettingsState state) {
    if (state.captureRevision < _captureRevision ||
        (!_captureEpochReset &&
            state.captureRevision == _captureRevision &&
            state.epoch < (_capture?.epoch ?? 0))) {
      return;
    }
    if (state.captureRevision > _captureRevision) {
      _captureRevision = state.captureRevision;
    }
    _capture = state;
    _captureSnapshot = _snapshotCapture(state);
  }

  CaptureSettingsState _snapshotCapture(CaptureSettingsState state) {
    final health = state.settingsHealth;
    return CaptureSettingsState(
      captureWatch: state.captureWatch,
      captureRevision: state.captureRevision,
      running: state.running,
      paused: state.paused,
      epoch: state.epoch,
      syncStatus: _snapshotSync(state.syncStatus),
      settingsHealth: health == null
          ? null
          : RuntimeSettingsHealth(
              recordUnreadable: health.recordUnreadable,
              unreadableFields: List.unmodifiable(health.unreadableFields),
            ),
    );
  }

  SyncStatus _snapshotSync(SyncStatus status) => SyncStatus(
    revision: status.revision,
    phase: status.phase,
    peers: List.unmodifiable(status.peers),
  );

  bool _sameCapture(CaptureSettingsState? a, CaptureSettingsState? b) {
    if (a == null || b == null) return a == b;
    final aHealth = a.settingsHealth;
    final bHealth = b.settingsHealth;
    return a.running == b.running &&
        a.paused == b.paused &&
        a.epoch == b.epoch &&
        _sameSync(a.syncStatus, b.syncStatus) &&
        (aHealth == null || bHealth == null
            ? aHealth == bHealth
            : aHealth.recordUnreadable == bHealth.recordUnreadable &&
                  listEquals(
                    aHealth.unreadableFields,
                    bHealth.unreadableFields,
                  ));
  }

  bool _sameSync(SyncStatus a, SyncStatus b) {
    if (a.revision != b.revision ||
        a.phase != b.phase ||
        a.peers.length != b.peers.length) {
      return false;
    }
    for (var i = 0; i < a.peers.length; i++) {
      final x = a.peers[i];
      final y = b.peers[i];
      if (x.id != y.id ||
          x.name != y.name ||
          x.phase != y.phase ||
          x.startedAt != y.startedAt ||
          x.lastSuccess != y.lastSuccess ||
          x.sent != y.sent ||
          x.received != y.received ||
          x.skippedTooLarge != y.skippedTooLarge ||
          x.error != y.error) {
        return false;
      }
    }
    return true;
  }

  bool _sameScreenshot(ScreenshotCaptureStatus a, ScreenshotCaptureStatus b) =>
      a.enabled == b.enabled &&
      a.mediaGranted == b.mediaGranted &&
      a.notificationGranted == b.notificationGranted &&
      a.running == b.running;

  void _invalidateSyncStatus() {
    _syncStatus = SyncStatus(
      revision: _syncStatus.revision,
      peers: _syncStatus.peers,
    );
    _syncSnapshot = _snapshotSync(_syncStatus);
  }

  String _message(Object error, String fallback) =>
      error is runtime.RuntimeError
      ? _runtimeMessage(error) ?? error.message
      : error is CaptureServiceUnavailable
      ? 'Clipboard capture permissions must be completed before capture can resume.'
      : error is CaptureServiceStopFailed
      ? 'Clipboard capture is paused. The Android capture service did not confirm that it stopped.'
      : error is NotificationPermissionDenied
      ? 'Notification permission is required to enable capture notifications.'
      : fallback;

  String? _settingsHealthMessage(RuntimeSettingsHealth? health) {
    if (health == null || !health.degraded) return null;
    return health.recordUnreadable
        ? 'CopyPaste could not read saved settings. Protective values are active; review and save your preferences to repair them.'
        : 'CopyPaste recovered unreadable settings with protective values. Review and save your preferences to repair them.';
  }

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
    unawaited(_captureStateSubscription?.cancel());
    unawaited(_screenshotSubscription?.cancel());
    unawaited(_syncSubscription?.cancel());
    unawaited(_repository.dispose());
    super.dispose();
  }
}
