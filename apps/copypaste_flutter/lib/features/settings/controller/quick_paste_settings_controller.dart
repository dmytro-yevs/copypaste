import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/widgets.dart';

import '../../../platform/desktop/global_shortcut.dart';
import '../../../platform/desktop/quick_paste_host.dart';
import '../../../platform/permissions/linux_integration.dart';
import '../repository/quick_paste_preferences_store.dart';

class QuickPasteSettingsController extends ChangeNotifier
    with WidgetsBindingObserver {
  QuickPasteSettingsController({
    required QuickPastePreferencesStore store,
    required DesktopShortcutRegistrar registrar,
    required QuickPasteWindowHost windowHost,
    LinuxIntegrationPort? linuxIntegration,
  }) : _store = store,
       _registrar = registrar,
       _windowHost = windowHost,
       _linuxIntegrationPort = linuxIntegration,
       _preferences = QuickPastePreferences.defaults();

  final QuickPastePreferencesStore _store;
  final DesktopShortcutRegistrar _registrar;
  final QuickPasteWindowHost _windowHost;
  final LinuxIntegrationPort? _linuxIntegrationPort;

  QuickPastePreferences _preferences;
  LinuxIntegrationStatus? _linuxIntegration;
  bool _nativeQuickPasteSupported = false;
  bool _supported = false;
  bool _initialized = false;
  bool _busy = false;
  bool _accessibilityGranted = false;
  bool _observingLifecycle = false;
  bool _recordingShortcut = false;
  DesktopShortcut? _registeredShortcut;
  bool _disposed = false;
  Future<void> _registrationOperation = Future<void>.value();
  String? _errorMessage;
  bool _startFailed = false;
  DesktopShortcutRefusal? _shortcutRefusal;

  bool get supported => _supported;
  bool get initialized => _initialized;
  bool get busy => _busy;
  bool get autoPaste => _preferences.autoPaste;
  DesktopShortcut get shortcut => _preferences.shortcut;

  /// The shortcut that currently opens Quick Paste, or null while none is
  /// registered (unsupported, recording, or registration failed).
  DesktopShortcut? get registeredShortcut =>
      _supported ? _registeredShortcut : null;
  bool get accessibilityGranted => _accessibilityGranted;
  LinuxIntegrationStatus? get linuxIntegration => _linuxIntegration;
  String? get registeredShortcutDescription => switch (_registrar) {
    LinuxPortalDesktopShortcutRegistrar registrar =>
      registrar.registeredTriggerDescription,
    _ => null,
  };

  /// The last failed action, or else why Quick Paste could not start.
  String? get errorMessage =>
      _errorMessage ??
      (_startFailed ? AppStrings.current.settingsQuickPasteStartFailed : null);

  /// Whether Quick Paste could not start; [retry] starts it again.
  bool get startFailed => _startFailed;

  /// The shortcut the system last refused; the rest of Quick Paste still works.
  DesktopShortcutRefusal? get shortcutRefusal => _shortcutRefusal;

  Future<void> initialize() async {
    if (_disposed || _initialized || _busy) return;
    _busy = true;
    notifyListeners();
    try {
      _preferences = await _store.read();
      if (_disposed) return;
      if (_linuxIntegrationPort == null) {
        _nativeQuickPasteSupported = await _windowHost.isSupported();
      } else {
        await _refreshLinuxIntegration();
      }
      if (_disposed) return;
      _supported = _quickPasteSupported;
      if (_supported) {
        if (!await _registerSavedShortcut()) return;
        if (_linuxIntegrationPort != null) {
          await _refreshLinuxIntegration();
        }
        _accessibilityGranted = await _windowHost.accessibilityGranted();
        if (_disposed) return;
      }
      _errorMessage = null;
      _startFailed = false;
    } catch (error) {
      if (!_disposed) {
        _log('could not start', error);
        _startFailed = true;
      }
    } finally {
      _initialized = true;
      if (!_disposed && (_supported || _linuxIntegrationPort != null)) {
        _startObservingLifecycle();
      }
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  /// Starts Quick Paste again after it could not start.
  Future<void> retry() async {
    if (_disposed || _busy || !_startFailed) return;
    _initialized = false;
    await initialize();
  }

  Future<bool> setAutoPaste(bool value) async {
    if (_disposed || _busy || value == _preferences.autoPaste) return false;
    _busy = true;
    _errorMessage = null;
    notifyListeners();
    final before = _preferences;
    final next = before.copyWith(autoPaste: value);
    try {
      if (value && _supported) {
        _accessibilityGranted = await _windowHost.requestAccessibility();
      }
      if (_disposed) return false;
      await _store.write(next);
      _preferences = next;
      return true;
    } catch (_) {
      _preferences = before;
      _errorMessage = AppStrings.current.settingsRuntimeAutoPasteUpdateFailed;
      return false;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> setShortcut(DesktopShortcut shortcut) async {
    if (_disposed || _busy || !shortcut.isValid) {
      return false;
    }
    if (shortcut == _preferences.shortcut) {
      await cancelShortcutRecording();
      return false;
    }
    _recordingShortcut = false;
    _busy = true;
    _errorMessage = null;
    notifyListeners();
    final before = _preferences;
    final next = before.copyWith(shortcut: shortcut);
    try {
      if (!await _changeRegistration(shortcut)) return false;
      await _store.write(next);
      if (_disposed) return false;
      _preferences = next;
      return true;
    } catch (error) {
      if (_disposed) return false;
      try {
        if (!await _changeRegistration(before.shortcut)) return false;
      } catch (_) {
        // The visible error remains the only safe claim when registration fails.
      }
      if (!_disposed && !_recordRefusal(shortcut, error)) {
        _log('could not change the shortcut', error);
        _errorMessage =
            AppStrings.current.settingsRuntimeShortcutRegisterFailed;
      }
      return false;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> resetShortcut() {
    return setShortcut(DesktopShortcut.defaultForPlatform());
  }

  Future<bool> beginShortcutRecording() async {
    if (_disposed || !_supported || _busy || _recordingShortcut) return false;
    _busy = true;
    notifyListeners();
    try {
      if (!await _changeRegistration(null)) return false;
      _recordingShortcut = true;
      return true;
    } catch (_) {
      if (!_disposed) {
        _errorMessage = AppStrings.current.settingsRuntimeShortcutPauseFailed;
      }
      return false;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> cancelShortcutRecording() async {
    if (_disposed ||
        !_supported ||
        _busy ||
        (!_recordingShortcut && _registeredShortcut == _preferences.shortcut)) {
      return;
    }
    _busy = true;
    notifyListeners();
    try {
      if (!await _registerSavedShortcut()) return;
      _recordingShortcut = false;
      _errorMessage = null;
    } catch (_) {
      if (!_disposed) {
        _errorMessage = AppStrings.current.settingsRuntimeShortcutRestoreFailed;
      }
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> _changeRegistration(DesktopShortcut? shortcut) {
    final operation = _registrationOperation.then((_) async {
      if (_disposed) return false;
      _registeredShortcut = null;
      if (shortcut == null) {
        await _registrar.unregister();
      } else {
        await _registrar.register(shortcut, _openWindow);
        _registeredShortcut = shortcut;
        _shortcutRefusal = null;
      }
      return !_disposed;
    });
    // Keep cleanup ordered after failed operations; callers handle their errors.
    _registrationOperation = operation.then<void>(
      (_) {},
      onError: (Object error, StackTrace stackTrace) {},
    );
    return operation;
  }

  /// Registers the saved shortcut. A shortcut the system refuses is recorded
  /// for the shortcut row instead of failing the caller.
  Future<bool> _registerSavedShortcut() async {
    final shortcut = _preferences.shortcut;
    try {
      return await _changeRegistration(shortcut);
    } catch (error) {
      if (_disposed || !_recordRefusal(shortcut, error)) rethrow;
      return true;
    }
  }

  bool _recordRefusal(DesktopShortcut shortcut, Object error) {
    final reason = DesktopShortcutRefusal.reasonOf(error);
    if (reason == null) return false;
    _log('shortcut was refused', error);
    _shortcutRefusal = DesktopShortcutRefusal(shortcut, reason);
    return true;
  }

  void _log(String event, Object error) =>
      debugPrint('Quick Paste $event: $error');

  Future<void> _disposeRegistration() async {
    try {
      await _registrationOperation;
      await _registrar.unregister();
    } catch (_) {
      // Teardown cannot report late native registration errors.
    }
  }

  Future<void> _openWindow() async {
    if (_disposed) return;
    try {
      await _windowHost.open();
      if (_disposed) return;
      _errorMessage = null;
    } catch (_) {
      if (_disposed) return;
      _errorMessage = AppStrings.current.settingsRuntimeQuickPasteOpenFailed;
    }
    notifyListeners();
  }

  Future<void> requestAccessibility() async {
    if (!_supported || _busy) return;
    _busy = true;
    notifyListeners();
    try {
      _accessibilityGranted = await _windowHost.requestAccessibility();
      _errorMessage = null;
    } catch (_) {
      _errorMessage =
          AppStrings.current.settingsRuntimeAccessibilityRequestFailed;
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  Future<void> refreshAccessibility() async {
    if (!_supported) return;
    try {
      _accessibilityGranted = await _windowHost.accessibilityGranted();
      notifyListeners();
    } catch (_) {
      // Keep the last confirmed permission state.
    }
  }

  Future<void> refreshLinuxIntegration() async {
    if (_linuxIntegrationPort == null || _disposed || _busy) return;
    _busy = true;
    _errorMessage = null;
    notifyListeners();
    try {
      await _refreshLinuxIntegration();
    } catch (_) {
      _errorMessage =
          AppStrings.current.settingsRuntimeLinuxIntegrationRefreshFailed;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> requestLinuxRemoteDesktop() async {
    final integration = _linuxIntegrationPort;
    if (integration == null || _disposed || _busy) return false;
    _busy = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final granted = await integration.requestRemoteDesktop();
      await _refreshLinuxIntegration();
      if (!granted ||
          _linuxIntegration?.remoteDesktop != LinuxRemoteDesktopState.active) {
        _errorMessage =
            AppStrings.current.settingsRuntimeClipboardInputNotGranted;
        return false;
      }
      return true;
    } catch (_) {
      _errorMessage =
          AppStrings.current.settingsRuntimeClipboardInputRequestFailed;
      return false;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> openLinuxCompanionSetup() async {
    final integration = _linuxIntegrationPort;
    if (integration == null || _disposed || _busy) return false;
    _busy = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final opened = await integration.openCompanionSetup();
      await _refreshLinuxIntegration();
      if (!opened) {
        _errorMessage =
            AppStrings.current.settingsRuntimeDesktopIntegrationSetupOpenFailed;
      }
      return opened;
    } catch (_) {
      _errorMessage =
          AppStrings.current.settingsRuntimeDesktopIntegrationSetupOpenFailed;
      return false;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  bool get _quickPasteSupported =>
      _nativeQuickPasteSupported &&
      (_linuxIntegrationPort == null ||
          _linuxIntegration?.globalShortcuts == true);

  void _startObservingLifecycle() {
    if (_observingLifecycle) return;
    WidgetsBinding.instance.addObserver(this);
    _observingLifecycle = true;
  }

  Future<void> _refreshLinuxIntegration() async {
    final integration = _linuxIntegrationPort;
    if (integration == null) return;
    final wasSupported = _supported;
    _linuxIntegration = await integration.status();
    _nativeQuickPasteSupported = await _windowHost.isSupported();
    final supported = _quickPasteSupported;
    if (wasSupported == supported || (!_initialized && supported)) {
      _supported = supported;
      return;
    }
    if (!supported) {
      _supported = false;
      await _changeRegistration(null);
      return;
    }
    _supported = true;
    await _registerSavedShortcut();
    _accessibilityGranted = await _windowHost.accessibilityGranted();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(refreshAccessibility());
      unawaited(refreshLinuxIntegration());
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _recordingShortcut = false;
    if (_observingLifecycle) {
      WidgetsBinding.instance.removeObserver(this);
      _observingLifecycle = false;
    }
    unawaited(_disposeRegistration());
    unawaited(_windowHost.dispose());
    super.dispose();
  }
}
