import 'dart:convert';

import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:shared_preferences/shared_preferences.dart';

class QuickPastePreferences {
  const QuickPastePreferences({
    required this.autoPaste,
    required this.shortcut,
    this.shortcutEnabled = true,
    this.maskPasswordFields = true,
  });

  factory QuickPastePreferences.defaults() => QuickPastePreferences(
    autoPaste: true,
    shortcut: DesktopShortcut.defaultForPlatform(),
  );

  final bool autoPaste;
  final DesktopShortcut shortcut;

  /// Whether the global shortcut is registered; [shortcut] is kept either way.
  final bool shortcutEnabled;

  /// Whether a focused password field masks previews and turns transforms
  /// off; switched off, it is treated as any other paste target.
  final bool maskPasswordFields;

  QuickPastePreferences copyWith({
    bool? autoPaste,
    DesktopShortcut? shortcut,
    bool? shortcutEnabled,
    bool? maskPasswordFields,
  }) => QuickPastePreferences(
    autoPaste: autoPaste ?? this.autoPaste,
    shortcut: shortcut ?? this.shortcut,
    shortcutEnabled: shortcutEnabled ?? this.shortcutEnabled,
    maskPasswordFields: maskPasswordFields ?? this.maskPasswordFields,
  );
}

abstract interface class QuickPastePreferencesStore {
  Future<QuickPastePreferences> read();

  Future<void> write(QuickPastePreferences preferences);

  Future<bool> accessibilityPromptWasRequested();

  Future<void> markAccessibilityPromptRequested();

  Future<Map<String, String>> readPinnedShortcuts();

  Future<void> writePinnedShortcuts(Map<String, String> shortcuts);
}

class SharedPreferencesQuickPastePreferencesStore
    implements QuickPastePreferencesStore {
  SharedPreferencesQuickPastePreferencesStore({
    SharedPreferencesAsync? preferences,
  }) : _preferences = preferences ?? SharedPreferencesAsync();

  static const _key = 'quick_paste.preferences.v1';
  static const _accessibilityPromptKey =
      'quick_paste.accessibility_requested.v1';
  static const _pinnedShortcutsKey = 'quick_paste.pinned_shortcuts.v1';

  final SharedPreferencesAsync _preferences;

  @override
  Future<QuickPastePreferences> read() async {
    final encoded = await _preferences.getString(_key);
    if (encoded == null) return QuickPastePreferences.defaults();
    try {
      final json = jsonDecode(encoded) as Map<String, dynamic>;
      DesktopShortcut shortcut;
      try {
        shortcut = DesktopShortcut.fromJson(
          Map<String, Object?>.from(json['shortcut'] as Map),
        );
      } catch (_) {
        shortcut = DesktopShortcut.defaultForPlatform();
      }
      return QuickPastePreferences(
        autoPaste: json['autoPaste'] as bool? ?? true,
        shortcut: shortcut,
        shortcutEnabled: json['shortcutEnabled'] as bool? ?? true,
        maskPasswordFields: json['maskPasswordFields'] as bool? ?? true,
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
        'shortcutEnabled': preferences.shortcutEnabled,
        'maskPasswordFields': preferences.maskPasswordFields,
      }),
    );
  }

  @override
  Future<bool> accessibilityPromptWasRequested() async =>
      await _preferences.getBool(_accessibilityPromptKey) ?? false;

  @override
  Future<void> markAccessibilityPromptRequested() =>
      _preferences.setBool(_accessibilityPromptKey, true);

  @override
  Future<Map<String, String>> readPinnedShortcuts() async {
    final encoded = await _preferences.getString(_pinnedShortcutsKey);
    if (encoded == null) return {};
    try {
      return Map<String, String>.from(jsonDecode(encoded) as Map);
    } catch (_) {
      return {};
    }
  }

  @override
  Future<void> writePinnedShortcuts(Map<String, String> shortcuts) =>
      _preferences.setString(_pinnedShortcutsKey, jsonEncode(shortcuts));
}

class MemoryQuickPastePreferencesStore implements QuickPastePreferencesStore {
  MemoryQuickPastePreferencesStore([QuickPastePreferences? initial])
    : value = initial ?? QuickPastePreferences.defaults();

  QuickPastePreferences value;
  bool _accessibilityPromptRequested = false;
  Map<String, String> _pinnedShortcuts = {};

  @override
  Future<QuickPastePreferences> read() async => value;

  @override
  Future<void> write(QuickPastePreferences preferences) async {
    value = preferences;
  }

  @override
  Future<bool> accessibilityPromptWasRequested() async =>
      _accessibilityPromptRequested;

  @override
  Future<void> markAccessibilityPromptRequested() async {
    _accessibilityPromptRequested = true;
  }

  @override
  Future<Map<String, String>> readPinnedShortcuts() async =>
      Map.of(_pinnedShortcuts);

  @override
  Future<void> writePinnedShortcuts(Map<String, String> shortcuts) async {
    _pinnedShortcuts = Map.of(shortcuts);
  }
}
