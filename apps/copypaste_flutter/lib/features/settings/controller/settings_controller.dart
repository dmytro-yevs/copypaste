import 'dart:async';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/l10n/l10n.dart';
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

  /// Storage used against the quota; null until the runtime reports it.
  ///
  /// Refreshed by every settings read. While this has listeners it is also
  /// re-read after each history change, coalesced to one request in flight,
  /// so nothing is requested for a readout nobody displays.
  ValueListenable<StorageUsage?> get storageUsage => _storageUsage;
  late final _storageUsage = _ObservedValue<StorageUsage?>(
    null,
    _storageUsageObserved,
  );
  StreamSubscription<void>? _historySubscription;
  bool _storageUsageRefreshing = false;
  bool _storageUsageStale = false;
  int _storageUsageRead = 0;
  int _storageUsageApplied = 0;

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
  final Map<SettingsOperation, String> _operationErrors = {};

  /// The operation whose failure the shared banner shows until the next
  /// change starts; [errorFor] keeps every failure.
  SettingsOperation? _bannerOperation;
  String? _settingsHealthNotice;
  String? _dismissedHealthNotice;
  final Set<SettingsOperation> _pending = {};
  final _dataTransfer = ValueNotifier<DataTransferProgress?>(null);
  Future<void> _settingsWrites = Future<void>.value();
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
  Future<void>? _settingsRefresh;
  bool _settingsConfirmed = false;
  static String get _settingsRefreshError =>
      AppStrings.current.settingsRuntimeRefreshFailed;
  bool _settingsRefreshRequested = false;
  int _settingsMutationEpoch = 0;

  SettingsLoadState get loadState => _loadState;
  RuntimeSettings? get settings => _settings;
  CaptureSettingsState? get capture => _capture;
  SyncStatus get syncStatus => _syncStatus;

  /// The latest failure until the next change starts, else a refresh
  /// failure, else the settings-health notice unless it was dismissed.
  String? get errorMessage =>
      _operationErrors[_bannerOperation] ??
      _errorMessage ??
      (_settingsHealthNotice == _dismissedHealthNotice
          ? null
          : _settingsHealthNotice);

  /// The failure of the last [operation], while it stands.
  String? errorFor(SettingsOperation operation) => _operationErrors[operation];

  /// Any change is in flight.
  bool get busy => _pending.isNotEmpty;
  bool isPending(SettingsOperation operation) => _pending.contains(operation);

  /// The running text export or backup, or null while neither runs. Its
  /// progress notifies here only, not the controller's listeners.
  ValueListenable<DataTransferProgress?> get dataTransfer => _dataTransfer;

  /// Whether [operation] may start now. A restore excludes every other
  /// change; export, backup and restore exclude one another.
  bool canStart(SettingsOperation operation) {
    if (_disposed || _pending.contains(operation)) return false;
    if (_pending.contains(SettingsOperation.restore)) return false;
    if (operation == SettingsOperation.restore) return _pending.isEmpty;
    return !operation.transfersHistory ||
        !_pending.any((pending) => pending.transfersHistory);
  }

  bool get _settingsWriteInFlight =>
      _pending.any((operation) => operation.writesSettings);
  bool get _captureWriteInFlight =>
      _pending.any((operation) => operation.writesCapture);

  /// Whether Settings is presented; readouts it alone shows observe only
  /// then.
  bool get presentationActive => _presentationActive;
  bool get blockScreenshots => _blockScreenshots;
  bool get blockScreenshotsSupported => _screenshotProtection.supported;
  bool get screenshotCaptureSupported => _screenshotCapture.supported;
  ScreenshotCaptureStatus get screenshotCapture => _screenshotStatus;

  Future<bool> setScreenshotCaptureEnabled(
    bool value,
  ) => _run(SettingsOperation.screenshotCapture, () async {
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

  Future<bool> requestScreenshotCapturePermission() =>
      _run(SettingsOperation.screenshotCapture, () async {
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
      final usageRead = ++_storageUsageRead;
      final values = await Future.wait<Object>([
        _repository.settings(),
        _repository.captureState(),
        _screenshotProtection.supported
            ? _screenshotProtection.blocked()
            : Future<bool>.value(false),
      ]);
      if (_disposed) return;
      _settings = values[0] as RuntimeSettings;
      _acceptStorageUsage(_settings!.storageUsage, usageRead);
      _settingsConfirmed = true;
      _setCapture(values[1] as CaptureSettingsState);
      _acceptSyncStatus(_capture!.syncStatus);
      _setHealthNotice(_settingsHealthMessage(_capture!.settingsHealth));
      _blockScreenshots = values[2] as bool;
      if (_screenshotCapture.supported) {
        _screenshotStatus = await _screenshotCapture.status();
      }
      if (_disposed) return;
      _loadState = SettingsLoadState.ready;
      _errorMessage = null;
      _watchHistoryChanges();
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
      _errorMessage = _message(
        error,
        AppStrings.current.settingsRuntimeLoadFailed,
      );
    }
    _notify();
  }

  Future<void> retry() => initialize();

  /// Selects [section] for the next time Settings is shown, or now.
  void showSection(SettingsSectionId section) {
    if (_disposed) return;
    navigation.select(section);
    _notify();
  }

  Future<bool> toggleCapture() async {
    final capture = _capture;
    if (capture == null) return false;
    return setCapturePaused(!capture.controlState.canResume);
  }

  Future<bool> setCapturePaused(bool paused) async {
    if (_capture == null) return false;
    return _run(SettingsOperation.capture, () async {
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
      _setHealthNotice(_settingsHealthMessage(_capture!.settingsHealth));
    });
  }

  Future<bool> setSkipSecret(bool value) => _update(
    SettingsOperation.skipSecret,
    () => RuntimeSettingsChange(skipSecret: value),
  );
  Future<bool> setSkipTransient(bool value) => _update(
    SettingsOperation.skipTransient,
    () => RuntimeSettingsChange(skipTransient: value),
  );

  Future<bool> setRetentionDays(int value) => _update(
    SettingsOperation.retention,
    () => RuntimeSettingsChange(retentionDays: value),
  );

  Future<bool> setStorageQuotaBytes(int value) => _update(
    SettingsOperation.storageQuota,
    () => RuntimeSettingsChange(storageQuotaBytes: value),
  );

  Future<bool> setMaxFileSizeBytes(int value) => _update(
    SettingsOperation.maxFileSize,
    () => RuntimeSettingsChange(maxFileSizeBytes: value),
  );

  Future<bool> addExcludedApp(String rawValue) async {
    final value = rawValue.trim();
    final settings = _settings;
    if (settings == null || value.isEmpty) return false;
    if (settings.excludedAppIds.contains(value)) {
      if (!canStart(SettingsOperation.excludedApps)) return false;
      _clearErrorsFor(SettingsOperation.excludedApps);
      _operationErrors[SettingsOperation.excludedApps] =
          AppStrings.current.settingsRuntimeAppAlreadyExcluded;
      _bannerOperation = SettingsOperation.excludedApps;
      _notify();
      return false;
    }
    // The list is rebuilt when the write runs so an earlier queued change to
    // it is never undone.
    return _update(
      SettingsOperation.excludedApps,
      () => RuntimeSettingsChange(
        excludedAppIds: [
          ..._settings!.excludedAppIds.where((item) => item != value),
          value,
        ],
      ),
    );
  }

  Future<bool> removeExcludedApp(String value) {
    if (_settings == null) return Future.value(false);
    return _update(
      SettingsOperation.excludedApps,
      () => RuntimeSettingsChange(
        excludedAppIds: _settings!.excludedAppIds
            .where((item) => item != value)
            .toList(growable: false),
      ),
    );
  }

  Future<bool> setLanVisibility(bool value) => _update(
    SettingsOperation.lanVisibility,
    () => RuntimeSettingsChange(lanVisibility: value),
  );

  Future<bool> setInstantClipboard(bool value) => _update(
    SettingsOperation.instantClipboard,
    () => RuntimeSettingsChange(instantClipboard: value),
  );

  Future<bool> setSyncEnabled(bool value) => _update(
    SettingsOperation.syncEnabled,
    () => RuntimeSettingsChange(syncEnabled: value),
  );

  Future<bool> setNotifyOnCopy(bool value) {
    return _run(SettingsOperation.notifyOnCopy, () async {
      if (value && !await _notifications.requestPermission()) {
        throw const NotificationPermissionDenied();
      }
      await _writeSettings(() => RuntimeSettingsChange(notifyOnCopy: value));
    });
  }

  Future<bool> setSoundOnCopy(bool value) => _update(
    SettingsOperation.soundOnCopy,
    () => RuntimeSettingsChange(soundOnCopy: value),
  );

  Future<bool> setNotificationPreview(bool value) => _update(
    SettingsOperation.notificationPreview,
    () => RuntimeSettingsChange(notificationPreview: value),
  );

  Future<void> _showCaptureNotification(String? id) async {
    // Configuration events precede subsequent capture events on the shared
    // watch. Honor that configuration before deciding how to display content.
    if (_settingsRefresh case final pending?) await pending;
    if (_disposed ||
        !_settingsConfirmed ||
        !(_settings?.notifyOnCopy ?? false)) {
      return;
    }
    CaptureNotificationPreview? preview;
    try {
      if ((_settings?.notificationPreview ?? false) && id != null) {
        preview = await _repository.capturePreview(id);
      }
    } catch (_) {
      // Deleted clips and unavailable previews still allow generic feedback.
    }
    if (_settingsRefresh case final pending?) await pending;
    if (_disposed ||
        !_settingsConfirmed ||
        !(_settings?.notifyOnCopy ?? false)) {
      return;
    }
    try {
      await _notifications.showCaptured(
        preview: (_settings?.notificationPreview ?? false) ? preview : null,
      );
    } catch (_) {
      // Optional system feedback must not interrupt clipboard capture.
    }
  }

  Future<bool> setBlockScreenshots(bool value) =>
      _run(SettingsOperation.blockScreenshots, () async {
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
    final completed = await _run(SettingsOperation.exportText, () async {
      result = await _tracked(
        DataTransferKind.textExport,
        (onProgress) =>
            _repository.exportTextHistory(path, onProgress: onProgress),
      );
      if (_disposed) return;
      await _filePicker.presentCreatedFile(path, mimeType: 'application/json');
    });
    return completed ? result : null;
  }

  /// Null when the chooser was cancelled or the backup failed.
  Future<BackupResult?> createBackup() async {
    final path = await _filePicker.chooseBackupPath();
    if (path == null) return null;
    BackupResult? result;
    final completed = await _run(SettingsOperation.backup, () async {
      result = await _tracked(
        DataTransferKind.backup,
        (onProgress) => _repository.backupHistory(path, onProgress: onProgress),
      );
      if (_disposed) return;
      await _filePicker.presentCreatedFile(
        path,
        mimeType: 'application/octet-stream',
      );
    });
    return completed ? result : null;
  }

  /// Returns null when the native picker is cancelled without restoring.
  Future<bool?> restoreBackup() async {
    var cancelled = false;
    final restored = await _run(SettingsOperation.restore, () async {
      final path = await _filePicker.chooseRestorePath();
      if (path == null) {
        cancelled = true;
        return;
      }
      await _repository.restoreHistory(path);
    });
    return cancelled ? null : restored;
  }

  /// Dismisses the shown failure and the current settings-health notice. A
  /// notice that is reported again after it cleared is shown again.
  void clearMessages() {
    if (errorMessage == null) return;
    _clearErrors();
    _dismissedHealthNotice = _settingsHealthNotice;
    _notify();
  }

  void _clearErrors() {
    _operationErrors.clear();
    _bannerOperation = null;
    if (_loadState != SettingsLoadState.error) _errorMessage = null;
  }

  /// Other operations keep their failures while [operation] starts again.
  void _clearErrorsFor(SettingsOperation operation) {
    _operationErrors.remove(operation);
    if (_loadState != SettingsLoadState.error) _errorMessage = null;
  }

  void _setHealthNotice(String? notice) {
    _settingsHealthNotice = notice;
    if (notice == null) _dismissedHealthNotice = null;
  }

  Future<bool> _update(
    SettingsOperation operation,
    RuntimeSettingsChange Function() change,
  ) => _run(operation, () => _writeSettings(change));

  /// Each write returns the whole record, so writes run one at a time in the
  /// order they were started and the last answer holds every change.
  Future<void> _writeSettings(RuntimeSettingsChange Function() change) {
    final write = _settingsWrites.then((_) async {
      if (_disposed) return;
      final usageRead = ++_storageUsageRead;
      final next = await _repository.updateSettings(change());
      _settings = next;
      _acceptStorageUsage(next.storageUsage, usageRead);
      _settingsConfirmed = true;
      _setHealthNotice(null);
    });
    _settingsWrites = write.then<void>((_) {}, onError: (Object _) {});
    return write;
  }

  /// Publishes [kind]'s progress to [dataTransfer] while [operation] runs.
  Future<T> _tracked<T>(
    DataTransferKind kind,
    Future<T> Function(DataTransferProgressCallback onProgress) operation,
  ) async {
    _dataTransfer.value = DataTransferProgress(kind);
    try {
      return await operation((processed, total) {
        if (_disposed || _dataTransfer.value?.kind != kind) return;
        _dataTransfer.value = DataTransferProgress(
          kind,
          processed: processed,
          total: total,
        );
      });
    } finally {
      if (!_disposed) _dataTransfer.value = null;
    }
  }

  Future<bool> _run(
    SettingsOperation operation,
    Future<void> Function() action,
  ) async {
    if (!canStart(operation)) return false;
    _settingsMutationEpoch++;
    _pending.add(operation);
    _clearErrorsFor(operation);
    _bannerOperation = null;
    _notify();
    try {
      await action();
      return true;
    } catch (error) {
      _operationErrors
        ..remove(operation)
        ..[operation] = _message(
          error,
          AppStrings.current.settingsRuntimeChangeFailed,
        );
      _bannerOperation = operation;
      return false;
    } finally {
      _pending.remove(operation);
      if (_settingsRefreshRequested && !_settingsWriteInFlight) {
        unawaited(_refreshSettings());
      }
      if (_storageUsageStale) unawaited(_refreshStorageUsage());
      _notify();
    }
  }

  /// Presentation polling is optional; capture and notification streams remain
  /// active while hidden. A desktop tray may retain capture-state polling.
  void setPresentationActive(bool active) {
    if (_disposed || active == _presentationActive) return;
    _presentationActive = active;
    // Readouts shown only in Settings stop or start observing with it.
    _notify();
    if (active) unawaited(_refreshSettings());
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
      unawaited(_refreshSettings());
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
    final revisionChanged = next.captureRevision != _captureRevision;
    _captureRevision = next.captureRevision;
    if (revisionChanged) unawaited(_refreshSettings());
    final previousCapture = _captureSnapshot;
    final previousSync = _syncSnapshot;
    final previousNotice = _settingsHealthNotice;
    _setCapture(next);
    _captureEpochReset = false;
    _acceptSyncStatus(next.syncStatus);
    _setHealthNotice(_settingsHealthMessage(next.settingsHealth));
    _captureWatchReady = _captureStreamConnected && next.captureWatch;
    _updateCapturePush();
    if (!_sameCapture(previousCapture, _captureSnapshot) ||
        !_sameSync(previousSync, _syncSnapshot) ||
        previousNotice != _settingsHealthNotice) {
      _notify();
    }
  }

  /// Coalesce authoritative reads on runtime changes and reconnects. A read
  /// started before a local mutation or reconnect cannot overwrite newer state.
  Future<void> _refreshSettings() {
    if (_disposed) return Future.value();
    _settingsConfirmed = false;
    _settingsRefreshRequested = true;
    if (_settingsWriteInFlight) return Future.value();
    if (_settingsRefresh case final pending?) return pending;
    late final Future<void> pending;
    pending = _readSettings().whenComplete(() {
      if (identical(_settingsRefresh, pending)) _settingsRefresh = null;
      if (_settingsRefreshRequested && !_settingsWriteInFlight && !_disposed) {
        unawaited(_refreshSettings());
      }
    });
    _settingsRefresh = pending;
    return pending;
  }

  Future<void> _readSettings() async {
    while (_settingsRefreshRequested && !_settingsWriteInFlight && !_disposed) {
      _settingsRefreshRequested = false;
      final epoch = _settingsMutationEpoch;
      final generation = _captureWatchGeneration;
      final usageRead = ++_storageUsageRead;
      try {
        final next = await _repository.settings();
        if (_disposed) return;
        if (_settingsWriteInFlight ||
            _settingsRefreshRequested ||
            epoch != _settingsMutationEpoch ||
            generation != _captureWatchGeneration) {
          _settingsRefreshRequested = true;
          continue;
        }
        _settings = next;
        _acceptStorageUsage(next.storageUsage, usageRead);
        _settingsConfirmed = true;
        if (_errorMessage == _settingsRefreshError) _errorMessage = null;
        _notify();
      } catch (_) {
        if (!_disposed) {
          _errorMessage = _settingsRefreshError;
          _notify();
        }
        return;
      }
    }
  }

  void _storageUsageObserved(bool observed) {
    if (_disposed) return;
    if (observed) {
      _watchHistoryChanges();
      // Changes that arrived while nobody displayed the readout were skipped.
      if (_loadState == SettingsLoadState.ready) {
        unawaited(_refreshStorageUsage());
      }
    } else {
      unawaited(_historySubscription?.cancel());
      _historySubscription = null;
      _storageUsageStale = false;
    }
  }

  void _watchHistoryChanges() {
    if (_historySubscription != null ||
        !_storageUsage.observed ||
        _loadState != SettingsLoadState.ready) {
      return;
    }
    if (_repository case final HistoryChangesRepository history) {
      _historySubscription = history.historyChanges().listen(
        (_) => unawaited(_refreshStorageUsage()),
        onError: (Object _, StackTrace _) {
          // The reconnect that follows re-reads settings and usage together.
        },
      );
    }
  }

  /// Re-read only the usage. It takes the settings read the runtime already
  /// serves but leaves [settings] and the notification policy untouched.
  Future<void> _refreshStorageUsage() async {
    if (_disposed) return;
    _storageUsageStale = true;
    if (busy || _storageUsageRefreshing) return;
    _storageUsageRefreshing = true;
    try {
      while (_storageUsageStale &&
          !busy &&
          !_disposed &&
          _storageUsage.observed) {
        _storageUsageStale = false;
        final read = ++_storageUsageRead;
        try {
          final next = await _repository.settings();
          _acceptStorageUsage(next.storageUsage, read);
        } catch (_) {
          // Keep the last confirmed readout; the next change retries.
        }
      }
    } finally {
      _storageUsageRefreshing = false;
    }
  }

  /// Reads are numbered when they start, so a slower, older reply cannot
  /// replace the result of a newer one.
  void _acceptStorageUsage(StorageUsage? usage, int read) {
    if (_disposed || read < _storageUsageApplied) return;
    _storageUsageApplied = read;
    _storageUsage.value = usage;
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
        _captureWriteInFlight ||
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
          _captureWriteInFlight ||
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
      final revisionChanged = next.captureRevision != _captureRevision;
      _captureRevision = next.captureRevision;
      if (revisionChanged) unawaited(_refreshSettings());
      _setCapture(next);
      _captureEpochReset = false;
      _captureWatchReady = _captureStreamConnected && next.captureWatch;
      _updateCapturePush();
      _acceptSyncStatus(next.syncStatus);
      _setHealthNotice(_settingsHealthMessage(next.settingsHealth));
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
    diagnosticsAvailable: status.diagnosticsAvailable,
    log: List.unmodifiable(status.log),
    revision: status.revision,
    phase: status.phase,
    peers: List.unmodifiable(
      status.peers.map(
        (peer) => PeerSyncStatus(
          id: peer.id,
          name: peer.name,
          phase: peer.phase,
          startedAt: peer.startedAt,
          lastSuccess: peer.lastSuccess,
          sent: peer.sent,
          received: peer.received,
          skippedTooLarge: peer.skippedTooLarge,
          error: peer.error,
          connection: peer.connection,
          checking: peer.checking,
          addressKnown: peer.addressKnown,
          issues: List.unmodifiable(peer.issues),
        ),
      ),
    ),
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
        a.diagnosticsAvailable != b.diagnosticsAvailable ||
        !listEquals(a.log, b.log) ||
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
          x.error != y.error ||
          x.connection != y.connection ||
          x.checking != y.checking ||
          x.addressKnown != y.addressKnown ||
          !listEquals(x.issues, y.issues)) {
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
      diagnosticsAvailable: _syncStatus.diagnosticsAvailable,
      log: _syncStatus.log,
      revision: _syncStatus.revision,
      peers: _syncStatus.peers,
    );
    _syncSnapshot = _snapshotSync(_syncStatus);
  }

  String _message(Object error, String fallback) =>
      error is runtime.RuntimeError
      ? _runtimeMessage(error) ?? error.message
      : error is CaptureServiceUnavailable
      ? AppStrings.current.settingsRuntimeCapturePermissionsIncomplete
      : error is CaptureServiceStopFailed
      ? AppStrings.current.settingsRuntimeCaptureStopUnconfirmed
      : error is NotificationPermissionDenied
      ? AppStrings.current.settingsRuntimeNotificationPermissionRequired
      : fallback;

  String? _settingsHealthMessage(RuntimeSettingsHealth? health) {
    if (health == null || !health.degraded) return null;
    return health.recordUnreadable
        ? AppStrings.current.settingsRuntimeSavedSettingsUnreadable
        : AppStrings.current.settingsRuntimeSettingsRecovered;
  }

  String? _runtimeMessage(runtime.RuntimeError error) {
    if (error.code != 'invalid_request') return null;

    return switch (error.message) {
      'excluded_app_bundle_ids contains an entry that is empty or too long' =>
        AppStrings.current.settingsRuntimeExcludedAppIdInvalid,
      'excluded_app_bundle_ids has too many entries; at most 256 are allowed' =>
        AppStrings.current.settingsRuntimeExcludedAppsLimit,
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
    unawaited(_historySubscription?.cancel());
    _storageUsage.dispose();
    _dataTransfer.dispose();
    unawaited(_repository.dispose());
    super.dispose();
  }
}

/// A value that reports when it gains its first and loses its last listener.
class _ObservedValue<T> extends ValueNotifier<T> {
  _ObservedValue(super.value, this._onObservedChanged);

  final void Function(bool observed) _onObservedChanged;

  bool get observed => hasListeners;

  @override
  void addListener(VoidCallback listener) {
    final wasObserved = hasListeners;
    super.addListener(listener);
    if (!wasObserved) _onObservedChanged(true);
  }

  @override
  void removeListener(VoidCallback listener) {
    final wasObserved = hasListeners;
    super.removeListener(listener);
    if (wasObserved && !hasListeners) _onObservedChanged(false);
  }
}
