import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

class DesktopVisibilityPreferences {
  const DesktopVisibilityPreferences({
    this.showInTaskbar = true,
    this.showInTray = true,
  });

  final bool showInTaskbar;
  final bool showInTray;

  DesktopVisibilityPreferences copyWith({
    bool? showInTaskbar,
    bool? showInTray,
  }) => DesktopVisibilityPreferences(
    showInTaskbar: showInTaskbar ?? this.showInTaskbar,
    showInTray: showInTray ?? this.showInTray,
  );
}

abstract interface class DesktopVisibilityPreferencesStore {
  Future<DesktopVisibilityPreferences> read();
  Future<void> write(DesktopVisibilityPreferences preferences);
}

class SharedPreferencesDesktopVisibilityStore
    implements DesktopVisibilityPreferencesStore {
  SharedPreferencesDesktopVisibilityStore({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const key = 'desktop_window.visibility.v1';
  final SharedPreferencesAsync _preferences;

  @override
  Future<DesktopVisibilityPreferences> read() async {
    final encoded = await _preferences.getString(key);
    if (encoded == null) return const DesktopVisibilityPreferences();
    try {
      final values = jsonDecode(encoded) as Map<String, dynamic>;
      return DesktopVisibilityPreferences(
        showInTaskbar: values['showInTaskbar'] as bool? ?? true,
        showInTray: values['showInTray'] as bool? ?? true,
      );
    } on FormatException {
      return const DesktopVisibilityPreferences();
    } on TypeError {
      return const DesktopVisibilityPreferences();
    }
  }

  @override
  Future<void> write(DesktopVisibilityPreferences preferences) =>
      _preferences.setString(
        key,
        jsonEncode({
          'showInTaskbar': preferences.showInTaskbar,
          'showInTray': preferences.showInTray,
        }),
      );
}

class MemoryDesktopVisibilityStore
    implements DesktopVisibilityPreferencesStore {
  MemoryDesktopVisibilityStore([
    this.value = const DesktopVisibilityPreferences(),
  ]);

  DesktopVisibilityPreferences value;

  @override
  Future<DesktopVisibilityPreferences> read() async => value;

  @override
  Future<void> write(DesktopVisibilityPreferences preferences) async {
    value = preferences;
  }
}
