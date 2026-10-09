import 'dart:async';

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
  bool _disposed = false;
  Future<void> _registrationOperation = Future<void>.value();
  String? _errorMessage;

  bool get supported => _supported;
  bool get initialized => _initialized;
  bool get busy => _busy;
  bool get autoPaste => _preferences.autoPaste;
  DesktopShortcut get shortcut => _preferences.shortcut;
  bool get accessibilityGranted => _accessibilityGranted;
  LinuxIntegrationStatus? get linuxIntegration => _linuxIntegration;
  String? get registeredShortcutDescription => switch (_registrar) {
    LinuxPortalDesktopShortcutRegistrar registrar =>
      registrar.registeredTriggerDescription,
    _ => null,
  };
  String? get errorMessage => _errorMessage;

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
        if (!await _changeRegistration(_preferences.shortcut)) return;
        _accessibilityGranted = await _windowHost.accessibilityGranted();
        if (_disposed) return;
      }
      _errorMessage = null;
    } catch (_) {
      if (!_disposed) _errorMessage = 'Quick Paste could not be prepared.';
    } finally {
      _initialized = true;
      if (!_disposed && (_supported || _linuxIntegrationPort != null)) {
        _startObservingLifecycle();
      }
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> setAutoPaste(bool value) async {
    if (_busy || value == _preferences.autoPaste) return false;
    _busy = true;
    _errorMessage = null;
    notifyListeners();
    final before = _preferences;
    final next = before.copyWith(autoPaste: value);
    try {
      await _store.write(next);
      _preferences = next;
      if (value && _supported) {
        _accessibilityGranted = await _windowHost.requestAccessibility();
      }
      return true;
    } catch (_) {
      _preferences = before;
      _errorMessage = 'Auto-paste could not be updated.';
      return false;
    } finally {
      _busy = false;
      notifyListeners();
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
    } catch (_) {
      if (_disposed) return false;
      try {
        if (!await _changeRegistration(before.shortcut)) return false;
      } catch (_) {
        // The visible error remains the only safe claim when registration fails.
      }
      if (!_disposed) _errorMessage = 'That shortcut could not be registered.';
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
        _errorMessage = 'The current shortcut could not be paused.';
      }
      return false;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> cancelShortcutRecording() async {
    if (_disposed || !_supported || _busy || !_recordingShortcut) return;
    _recordingShortcut = false;
    _busy = true;
    notifyListeners();
    try {
      if (!await _changeRegistration(_preferences.shortcut)) return;
      _errorMessage = null;
    } catch (_) {
      if (!_disposed) _errorMessage = 'The shortcut could not be restored.';
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<bool> _changeRegistration(DesktopShortcut? shortcut) {
    final operation = _registrationOperation.then((_) async {
      if (_disposed) return false;
      if (shortcut == null) {
        await _registrar.unregister();
      } else {
        await _registrar.register(shortcut, _openWindow);
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
      _errorMessage = 'Quick Paste could not be opened.';
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
      _errorMessage = 'Accessibility permission could not be requested.';
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
      _errorMessage = 'Linux integration status could not be refreshed.';
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
        _errorMessage = 'Clipboard input permission was not granted.';
        return false;
      }
      return true;
    } catch (_) {
      _errorMessage = 'Clipboard input permission could not be requested.';
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
        _errorMessage = 'Desktop integration setup could not be opened.';
      }
      return opened;
    } catch (_) {
      _errorMessage = 'Desktop integration setup could not be opened.';
      return false;
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  bool get _quickPasteSupported =>
      _nativeQuickPasteSupported &&
      (_linuxIntegrationPort == null || _linuxIntegration?.quickPaste == true);

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
    if (!_initialized || wasSupported == supported) {
      _supported = supported;
      return;
    }
    if (!supported) {
      _supported = false;
      await _changeRegistration(null);
      return;
    }
    _supported = true;
    await _changeRegistration(_preferences.shortcut);
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
