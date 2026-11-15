import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// One device-local on/off preference; never synced.
///
/// Reads fall back to [defaultValue] and a failed write keeps the new value
/// for this session, so a broken preferences store never blocks a toggle.
class PreferenceToggle extends ValueNotifier<bool> {
  PreferenceToggle(
    this.key, {
    required this.defaultValue,
    SharedPreferencesAsync? preferences,
  }) : _preferences = preferences,
       super(defaultValue);

  /// In-memory only, for tests and engines without persistent preferences.
  PreferenceToggle.memory(super.value)
    : key = '',
      defaultValue = value,
      _preferences = null,
      _persistent = false;

  final String key;
  final bool defaultValue;
  final SharedPreferencesAsync? _preferences;
  bool _persistent = true;
  Future<void>? _loading;

  SharedPreferencesAsync get _store => _preferences ?? SharedPreferencesAsync();

  Future<void> load() => _loading ??= _read();

  Future<void> _read() async {
    if (!_persistent) return;
    try {
      value = await _store.getBool(key) ?? defaultValue;
    } on Object {
      value = defaultValue;
    }
  }

  Future<void> set(bool next) async {
    value = next;
    if (!_persistent) return;
    try {
      await _store.setBool(key, next);
    } on Object {
      // Kept for this session.
    }
  }
}
