import 'package:flutter/foundation.dart';

import 'desktop_visibility_preferences.dart';

abstract interface class DesktopVisibilityHost {
  Future<void> setTaskbarVisible(bool visible);
  Future<void> setTrayVisible(bool visible);
}

/// Owns local desktop presentation preferences and their native application.
class DesktopVisibilityController extends ChangeNotifier {
  DesktopVisibilityController({
    required DesktopVisibilityHost host,
    required DesktopVisibilityPreferencesStore store,
  }) : _host = host,
       _store = store;

  final DesktopVisibilityHost _host;
  final DesktopVisibilityPreferencesStore _store;
  DesktopVisibilityPreferences _preferences =
      const DesktopVisibilityPreferences();
  bool _busy = false;
  bool _disposed = false;
  String? _error;
  Future<void>? _pending;

  DesktopVisibilityPreferences get preferences => _preferences;
  bool get busy => _busy;
  String? get error => _error;

  Future<void> load() async {
    _preferences = await _store.read();
    _notify();
  }

  Future<bool> setTaskbarVisible(bool visible) => _update(
    _preferences.copyWith(showInTaskbar: visible),
    () => _host.setTaskbarVisible(visible),
    () => _host.setTaskbarVisible(_preferences.showInTaskbar),
  );

  Future<bool> setTrayVisible(bool visible) => _update(
    _preferences.copyWith(showInTray: visible),
    () => _host.setTrayVisible(visible),
    () => _host.setTrayVisible(_preferences.showInTray),
  );

  Future<bool> _update(
    DesktopVisibilityPreferences next,
    Future<void> Function() apply,
    Future<void> Function() rollback,
  ) async {
    if (_busy || _disposed) return false;
    _busy = true;
    _error = null;
    _notify();
    final operation = () async {
      await apply();
      try {
        await _store.write(next);
      } catch (_) {
        await rollback();
        rethrow;
      }
    }();
    _pending = operation;
    try {
      await operation;
      _preferences = next;
      return true;
    } catch (_) {
      _error = 'Application visibility could not be changed. Please try again.';
      return false;
    } finally {
      _pending = null;
      _busy = false;
      _notify();
    }
  }

  Future<void> settle() async => await _pending?.catchError((_) {});

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
