import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../platform/system_accent.dart';
import '../../shared/preference_choice.dart';
import 'app_accent.dart';
import 'app_theme.dart';

/// The theme mode and accent every application root of one engine follows.
class AppAppearance extends ChangeNotifier {
  AppAppearance({
    required this.themeMode,
    required this.accent,
    SystemAccentHost? systemAccent,
  }) : _systemAccent = systemAccent;

  final PreferenceChoice<ThemeMode> themeMode;
  final PreferenceChoice<AppAccent> accent;
  final SystemAccentHost? _systemAccent;
  AppLifecycleListener? _lifecycle;
  Color? _systemColor;
  Color? _themedAccent;
  ThemeData? _light;
  ThemeData? _dark;
  bool _started = false;
  bool _disposed = false;

  /// The system's accent, once read; null where the platform has none.
  Color? get systemColor => _systemColor;

  /// The accent in use; the standard one when the system has none.
  Color get accentColor => switch (accent.value) {
    AppAccent.system => _systemColor ?? AppTheme.navigationAccent,
    final preset => preset.color!,
  };

  ThemeData get light {
    _rebuildThemes();
    return _light!;
  }

  ThemeData get dark {
    _rebuildThemes();
    return _dark!;
  }

  void _rebuildThemes() {
    final color = accentColor;
    if (color == _themedAccent) return;
    _themedAccent = color;
    _light = AppTheme.lightWith(color);
    _dark = AppTheme.darkWith(color);
  }

  Future<void> start() async {
    if (_started) return;
    _started = true;
    await Future.wait([themeMode.load(), accent.load()]);
    if (_disposed) return;
    themeMode.addListener(notifyListeners);
    accent.addListener(notifyListeners);
    if (_systemAccent case final host?) {
      host.onChanged = _setSystemColor;
      // A platform that reports no change is read again on return.
      _lifecycle = AppLifecycleListener(
        onResume: () => unawaited(_readSystem()),
      );
      await _readSystem();
    }
    notifyListeners();
  }

  Future<void> _readSystem() async {
    final host = _systemAccent;
    if (host == null) return;
    _setSystemColor(await host.read());
  }

  void _setSystemColor(Color? color) {
    if (_disposed || color == _systemColor) return;
    _systemColor = color;
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _lifecycle?.dispose();
    if (_started) {
      themeMode.removeListener(notifyListeners);
      accent.removeListener(notifyListeners);
      _systemAccent?.onChanged = null;
    }
    super.dispose();
  }
}
