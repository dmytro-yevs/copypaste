import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// One device-local choice among enum [values]; never synced.
///
/// Stored by name. Reads fall back to [defaultValue] and a failed write keeps
/// the new value for this session, like `PreferenceToggle`.
class PreferenceChoice<T extends Enum> extends ValueNotifier<T> {
  PreferenceChoice(
    this.key, {
    required this.values,
    required this.defaultValue,
    SharedPreferencesAsync? preferences,
  }) : _preferences = preferences,
       super(defaultValue);

  /// In-memory only, for tests and engines without persistent preferences.
  PreferenceChoice.memory(super.value, {required this.values})
    : key = '',
      defaultValue = value,
      _preferences = null,
      _persistent = false;

  final String key;
  final List<T> values;
  final T defaultValue;
  final SharedPreferencesAsync? _preferences;
  bool _persistent = true;
  Future<void>? _loading;

  SharedPreferencesAsync get _store => _preferences ?? SharedPreferencesAsync();

  Future<void> load() => _loading ??= _read();

  Future<void> _read() async {
    if (!_persistent) return;
    try {
      final name = await _store.getString(key);
      value = values.firstWhere(
        (each) => each.name == name,
        orElse: () => defaultValue,
      );
    } on Object {
      value = defaultValue;
    }
  }

  Future<void> set(T next) async {
    value = next;
    if (!_persistent) return;
    try {
      await _store.setString(key, next.name);
    } on Object {
      // Kept for this session.
    }
  }
}
