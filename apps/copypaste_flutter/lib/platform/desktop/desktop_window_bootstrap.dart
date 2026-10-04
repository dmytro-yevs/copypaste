import 'dart:io';
import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'desktop_window_controller.dart';
import 'flutter_desktop_window_host.dart';

Future<DesktopWindowController?> initializeDesktopWindow() async {
  if (!Platform.isMacOS && !Platform.isWindows) {
    return null;
  }

  WidgetsFlutterBinding.ensureInitialized();
  final controller = DesktopWindowController(
    host: FlutterDesktopWindowHost(),
    geometryStore: SharedPreferencesDesktopWindowGeometryStore(),
  );
  await controller.initialize();
  return controller;
}

class SharedPreferencesDesktopWindowGeometryStore
    implements DesktopWindowGeometryStore {
  SharedPreferencesDesktopWindowGeometryStore({
    SharedPreferencesAsync? preferences,
  }) : _preferences = preferences ?? SharedPreferencesAsync();

  static const _boundsKey = 'desktop_window.bounds';

  final SharedPreferencesAsync _preferences;

  @override
  Future<DesktopWindowBounds?> read() async {
    final encodedBounds = await _preferences.getString(_boundsKey);
    if (encodedBounds == null) {
      return null;
    }
    final values = jsonDecode(encodedBounds) as Map<String, dynamic>;
    return DesktopWindowBounds(
      left: (values['left'] as num).toDouble(),
      top: (values['top'] as num).toDouble(),
      width: (values['width'] as num).toDouble(),
      height: (values['height'] as num).toDouble(),
    );
  }

  @override
  Future<void> write(DesktopWindowBounds bounds) {
    return _preferences.setString(
      _boundsKey,
      jsonEncode(<String, double>{
        'left': bounds.left,
        'top': bounds.top,
        'width': bounds.width,
        'height': bounds.height,
      }),
    );
  }
}
