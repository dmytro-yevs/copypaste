import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

import '../../../platform/desktop/global_shortcut.dart';
import '../models/paste_queue_models.dart';

/// Device-local queue shortcuts and delivery style; never synced, because
/// shortcuts and input permissions belong to one machine.
abstract interface class PasteQueueShortcutsStore {
  Future<PasteQueueDevicePreferences> read();

  Future<void> write(PasteQueueDevicePreferences preferences);
}

class MemoryPasteQueueShortcutsStore implements PasteQueueShortcutsStore {
  MemoryPasteQueueShortcutsStore([
    this.value = const PasteQueueDevicePreferences(),
  ]);

  PasteQueueDevicePreferences value;

  @override
  Future<PasteQueueDevicePreferences> read() async => value;

  @override
  Future<void> write(PasteQueueDevicePreferences preferences) async {
    value = preferences;
  }
}

class SharedPreferencesPasteQueueShortcutsStore
    implements PasteQueueShortcutsStore {
  SharedPreferencesPasteQueueShortcutsStore({
    SharedPreferencesAsync? preferences,
  }) : _preferences = preferences ?? SharedPreferencesAsync();

  static const _key = 'paste_queue.device_preferences.v1';

  final SharedPreferencesAsync _preferences;

  @override
  Future<PasteQueueDevicePreferences> read() async {
    final encoded = await _preferences.getString(_key);
    if (encoded == null) return const PasteQueueDevicePreferences();
    try {
      return decode(jsonDecode(encoded) as Map<String, Object?>);
    } catch (_) {
      return const PasteQueueDevicePreferences();
    }
  }

  @override
  Future<void> write(PasteQueueDevicePreferences preferences) =>
      _preferences.setString(_key, jsonEncode(encode(preferences)));

  static Map<String, Object?> encode(PasteQueueDevicePreferences value) => {
    'shortcuts': {
      for (final MapEntry(:key, :value) in value.shortcuts.entries)
        key.name: value?.toJson(),
    },
    'pasteNextStyle': value.pasteNextStyle.name,
    'shortcutPromptDismissed': value.shortcutPromptDismissed,
  };

  /// Unknown actions and unreadable shortcuts fall back to the defaults.
  static PasteQueueDevicePreferences decode(Map<String, Object?> json) {
    final shortcuts = <PasteQueueShortcutAction, DesktopShortcut?>{};
    final stored = json['shortcuts'];
    if (stored is Map) {
      for (final action in PasteQueueShortcutAction.values) {
        if (!stored.containsKey(action.name)) continue;
        final value = stored[action.name];
        if (value == null) {
          shortcuts[action] = null;
          continue;
        }
        try {
          shortcuts[action] = DesktopShortcut.fromJson(
            Map<String, Object?>.from(value as Map),
          );
        } catch (_) {
          // An unreadable binding keeps the platform default.
        }
      }
    }
    return PasteQueueDevicePreferences(
      shortcuts: shortcuts,
      pasteNextStyle: PasteNextStyle.values.firstWhere(
        (style) => style.name == json['pasteNextStyle'],
        orElse: () => PasteNextStyle.pasteIntoApp,
      ),
      shortcutPromptDismissed: json['shortcutPromptDismissed'] == true,
    );
  }
}
