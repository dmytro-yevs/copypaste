import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../platform/desktop/global_shortcut.dart';
import '../../../platform/desktop/quick_paste_host.dart';
import '../repository/quick_paste_preferences_store.dart';

class QuickPasteSettingsController extends ChangeNotifier
    with WidgetsBindingObserver {
  QuickPasteSettingsController({
    required QuickPastePreferencesStore store,
    required DesktopShortcutRegistrar registrar,
    required QuickPasteWindowHost windowHost,
  }) : _store = store,
       _registrar = registrar,
       _windowHost = windowHost,
       _preferences = QuickPastePreferences.defaults();

  final QuickPastePreferencesStore _store;
  final DesktopShortcutRegistrar _registrar;
  final QuickPasteWindowHost _windowHost;

  QuickPastePreferences _preferences;
  bool _supported = false;
  bool _initialized = false;
  bool _busy = false;
  bool _accessibilityGranted = false;
  bool _observingLifecycle = false;
  String? _errorMessage;

  bool get supported => _supported;
  bool get initialized => _initialized;
  bool get busy => _busy;
  bool get autoPaste => _preferences.autoPaste;
  DesktopShortcut get shortcut => _preferences.shortcut;
  bool get accessibilityGranted => _accessibilityGranted;
  String? get errorMessage => _errorMessage;

  Future<void> initialize() async {
    if (_initialized) return;
    _busy = true;
    notifyListeners();
    try {
      _supported = await _windowHost.isSupported();
      _preferences = await _store.read();
      if (_supported) {
        WidgetsBinding.instance.addObserver(this);
        _observingLifecycle = true;
        await _windowHost.prepare();
        await _registrar.register(_preferences.shortcut, _windowHost.open);
        _accessibilityGranted = await _windowHost.accessibilityGranted();
      }
      _errorMessage = null;
    } catch (_) {
      _errorMessage = 'Quick Paste could not be prepared.';
    } finally {
      _initialized = true;
      _busy = false;
      notifyListeners();
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
    if (_busy || shortcut == _preferences.shortcut || !shortcut.isValid) {
      return false;
    }
    _busy = true;
    _errorMessage = null;
    notifyListeners();
    final before = _preferences;
    final next = before.copyWith(shortcut: shortcut);
    try {
      await _registrar.register(shortcut, _windowHost.open);
      await _store.write(next);
      _preferences = next;
      return true;
    } catch (_) {
      try {
        await _registrar.register(before.shortcut, _windowHost.open);
      } catch (_) {
        // The visible error remains the only safe claim when registration fails.
      }
      _errorMessage = 'That shortcut could not be registered.';
      return false;
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  Future<bool> resetShortcut() {
    return setShortcut(DesktopShortcut.defaultForPlatform());
  }

  Future<bool> beginShortcutRecording() async {
    if (!_supported || _busy) return false;
    try {
      await _registrar.unregister();
      return true;
    } catch (_) {
      _errorMessage = 'The current shortcut could not be paused.';
      notifyListeners();
      return false;
    }
  }

  Future<void> cancelShortcutRecording() async {
    if (!_supported) return;
    try {
      await _registrar.register(_preferences.shortcut, _windowHost.open);
    } catch (_) {
      _errorMessage = 'The shortcut could not be restored.';
      notifyListeners();
    }
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

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(refreshAccessibility());
    }
  }

  @override
  void dispose() {
    if (_observingLifecycle) {
      WidgetsBinding.instance.removeObserver(this);
      _observingLifecycle = false;
    }
    unawaited(_registrar.unregister());
    unawaited(_windowHost.dispose());
    super.dispose();
  }
}
