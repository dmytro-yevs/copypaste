import 'package:flutter/foundation.dart';

import '../../../platform/android/android_quick_access_gateway.dart';

enum QuickAccessFailure { load, action }

/// Android quick access settings state. It reads the native state only when
/// asked, so the section costs nothing until it is shown.
class QuickAccessController extends ChangeNotifier {
  QuickAccessController(this._gateway);

  final AndroidQuickAccessGateway _gateway;
  AndroidQuickAccessState? _state;
  final _tileResults = <AndroidQuickAccessTile, AndroidTileRequestResult>{};
  bool _loading = false;
  bool _busy = false;
  bool _disposed = false;
  QuickAccessFailure? _failure;

  AndroidQuickAccessState? get state => _state;
  bool get loading => _loading;
  bool get busy => _busy;
  QuickAccessFailure? get failure => _failure;

  /// The last placement answer for [tile] in this session.
  AndroidTileRequestResult? tileResult(AndroidQuickAccessTile tile) =>
      _tileResults[tile];

  Future<void> refresh() async {
    if (_loading || _disposed) return;
    _loading = true;
    if (_failure == QuickAccessFailure.load) _failure = null;
    _notify();
    try {
      final next = await _gateway.state();
      if (!_disposed) _state = next;
    } catch (_) {
      if (!_disposed && _state == null) _failure = QuickAccessFailure.load;
    } finally {
      _loading = false;
      _notify();
    }
  }

  Future<void> openKeyboardSettings() =>
      _perform(() async => _gateway.openKeyboardSettings());

  Future<void> showKeyboardPicker() =>
      _perform(() async => _gateway.showKeyboardPicker());

  Future<void> requestPinWidget() =>
      _perform(() async => _gateway.requestPinWidget());

  Future<void> requestAddTile(AndroidQuickAccessTile tile) =>
      _perform(() async {
        final result = await _gateway.requestAddTile(tile);
        if (!_disposed) _tileResults[tile] = result;
        return result != AndroidTileRequestResult.unavailable;
      });

  Future<void> setPanelOption(AndroidPanelOption option, bool enabled) =>
      _perform(() async {
        final next = await _gateway.setPanelOption(option, enabled);
        if (!_disposed) _state = next;
        return true;
      });

  Future<void> _perform(Future<bool> Function() action) async {
    if (_busy || _disposed) return;
    _busy = true;
    _failure = null;
    _notify();
    try {
      if (!await action() && !_disposed) _failure = QuickAccessFailure.action;
    } catch (_) {
      if (!_disposed) _failure = QuickAccessFailure.action;
    } finally {
      _busy = false;
      _notify();
    }
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
