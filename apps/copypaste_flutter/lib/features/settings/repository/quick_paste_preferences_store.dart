import 'dart:convert';

import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:shared_preferences/shared_preferences.dart';

class QuickPastePreferences {
  const QuickPastePreferences({
    required this.autoPaste,
    required this.shortcut,
  });

  factory QuickPastePreferences.defaults() => QuickPastePreferences(
    autoPaste: true,
    shortcut: DesktopShortcut.defaultForPlatform(),
  );

  final bool autoPaste;
  final DesktopShortcut shortcut;

  QuickPastePreferences copyWith({
    bool? autoPaste,
    DesktopShortcut? shortcut,
  }) => QuickPastePreferences(
    autoPaste: autoPaste ?? this.autoPaste,
    shortcut: shortcut ?? this.shortcut,
  );
}

abstract interface class QuickPastePreferencesStore {
  Future<QuickPastePreferences> read();

  Future<void> write(QuickPastePreferences preferences);
}

class SharedPreferencesQuickPastePreferencesStore
    implements QuickPastePreferencesStore {
  SharedPreferencesQuickPastePreferencesStore({
    SharedPreferencesAsync? preferences,
  }) : _preferences = preferences ?? SharedPreferencesAsync();

  static const _key = 'quick_paste.preferences.v1';

  final SharedPreferencesAsync _preferences;

  @override
  Future<QuickPastePreferences> read() async {
    final encoded = await _preferences.getString(_key);
    if (encoded == null) return QuickPastePreferences.defaults();
    try {
      final json = jsonDecode(encoded) as Map<String, dynamic>;
      return QuickPastePreferences(
        autoPaste: json['autoPaste'] as bool? ?? true,
        shortcut: DesktopShortcut.fromJson(
          Map<String, Object?>.from(json['shortcut'] as Map),
        ),
      );
    } catch (_) {
      return QuickPastePreferences.defaults();
    }
  }

  @override
  Future<void> write(QuickPastePreferences preferences) {
    return _preferences.setString(
      _key,
      jsonEncode({
        'autoPaste': preferences.autoPaste,
        'shortcut': preferences.shortcut.toJson(),
      }),
    );
  }
}

class MemoryQuickPastePreferencesStore implements QuickPastePreferencesStore {
  MemoryQuickPastePreferencesStore([QuickPastePreferences? initial])
    : value = initial ?? QuickPastePreferences.defaults();

  QuickPastePreferences value;

  @override
  Future<QuickPastePreferences> read() async => value;

  @override
  Future<void> write(QuickPastePreferences preferences) async {
    value = preferences;
  }
}
