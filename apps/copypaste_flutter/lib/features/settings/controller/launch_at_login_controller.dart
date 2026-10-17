import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../platform/startup/launch_at_login.dart';

class LaunchAtLoginController extends ChangeNotifier
    with WidgetsBindingObserver {
  LaunchAtLoginController(this._gateway);

  final LaunchAtLoginGateway _gateway;
  LaunchAtLoginStatus _status = LaunchAtLoginStatus.unavailable;
  bool _busy = false;
  bool _disposed = false;
  bool _observing = false;
  String? _error;

  LaunchAtLoginStatus get status => _status;
  bool get enabled =>
      _status == LaunchAtLoginStatus.enabled ||
      _status == LaunchAtLoginStatus.requiresApproval;
  bool get busy => _busy;
  String? get error => _error;

  Future<void> refresh() async {
    if (_busy || _disposed) return;
    if (!_observing) {
      WidgetsBinding.instance.addObserver(this);
      _observing = true;
    }
    await _perform(_gateway.read);
  }

  Future<void> setEnabled(bool enabled) async {
    if (_busy || _disposed) return;
    await _perform(() async {
      final next = await _gateway.setEnabled(enabled);
      if ((next == LaunchAtLoginStatus.enabled ||
                  next == LaunchAtLoginStatus.requiresApproval) !=
              enabled ||
          next == LaunchAtLoginStatus.unavailable) {
        throw StateError('Startup registration did not change.');
      }
      return next;
    });
  }

  Future<void> _perform(Future<LaunchAtLoginStatus> Function() action) async {
    _busy = true;
    _error = null;
    notifyListeners();
    try {
      final next = await action();
      if (!_disposed) _status = next;
    } catch (_) {
      if (!_disposed) {
        _error = 'Start at login could not be updated. Try again.';
      }
    } finally {
      _busy = false;
      if (!_disposed) notifyListeners();
    }
  }

  Future<void> openSystemSettings() async {
    try {
      await _gateway.openSystemSettings();
    } catch (_) {
      if (!_disposed) {
        _error = 'System startup settings could not be opened.';
        notifyListeners();
      }
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) unawaited(refresh());
  }

  @override
  void dispose() {
    _disposed = true;
    if (_observing) WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }
}
