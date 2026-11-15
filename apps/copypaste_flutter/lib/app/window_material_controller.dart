import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:shadcn_flutter/shadcn_flutter.dart' show ThemeMode;

import '../platform/desktop/window_material.dart';
import '../shared/preference_toggle.dart';

/// Applies the glass and theme preferences to this engine's window and
/// publishes the material in use, so the theme keeps surfaces opaque whenever
/// the system shows none.
class WindowMaterialController extends ValueNotifier<WindowMaterialState> {
  WindowMaterialController({
    required this.preference,
    required WindowMaterialHost host,
    this.appearance,
  }) : _host = host,
       super(const WindowMaterialState());

  final PreferenceToggle preference;

  /// The theme the window's system parts follow; the system's without one.
  final ValueListenable<ThemeMode>? appearance;
  final WindowMaterialHost _host;
  int _request = 0;
  bool _started = false;
  bool _disposed = false;

  Future<void> start() async {
    if (_started) return;
    _started = true;
    await preference.load();
    if (_disposed) return;
    preference.addListener(_onPreferenceChanged);
    appearance?.addListener(_onAppearanceChanged);
    _host.onStateChanged = _publish;
    _onAppearanceChanged();
    await _apply();
  }

  void _onPreferenceChanged() => unawaited(_apply());

  void _onAppearanceChanged() {
    if (appearance case final appearance?) {
      unawaited(_host.setAppearance(appearance.value));
    }
  }

  Future<void> _apply() async {
    final request = ++_request;
    final state = await _host.setEnabled(preference.value);
    // A later toggle supersedes this answer.
    if (request == _request) _publish(state);
  }

  void _publish(WindowMaterialState state) {
    if (!_disposed) value = state;
  }

  @override
  void dispose() {
    _disposed = true;
    if (_started) {
      preference.removeListener(_onPreferenceChanged);
      appearance?.removeListener(_onAppearanceChanged);
      _host.onStateChanged = null;
    }
    super.dispose();
  }
}
