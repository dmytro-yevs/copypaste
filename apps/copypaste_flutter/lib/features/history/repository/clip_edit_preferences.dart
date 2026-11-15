import 'package:shared_preferences/shared_preferences.dart';

/// The device-local "Replace original" choice of the clip editor; it is
/// remembered per device and never synced.
abstract interface class ClipEditPreferences {
  Future<bool> replaceOriginal();

  Future<void> setReplaceOriginal(bool value);
}

class SharedPreferencesClipEditPreferences implements ClipEditPreferences {
  SharedPreferencesClipEditPreferences({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const _key = 'snippets.clip_edit.replace_original';

  final SharedPreferencesAsync _preferences;

  @override
  Future<bool> replaceOriginal() async {
    try {
      return await _preferences.getBool(_key) ?? false;
    } on Object {
      return false;
    }
  }

  @override
  Future<void> setReplaceOriginal(bool value) async {
    try {
      await _preferences.setBool(_key, value);
    } on Object {
      // A failed write keeps the choice for this edit only.
    }
  }
}

/// For tests and engines without persistent preferences.
class MemoryClipEditPreferences implements ClipEditPreferences {
  MemoryClipEditPreferences([this._value = false]);

  bool _value;

  @override
  Future<bool> replaceOriginal() async => _value;

  @override
  Future<void> setReplaceOriginal(bool value) async => _value = value;
}
