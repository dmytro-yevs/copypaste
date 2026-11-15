import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

/// Device-local History searches, newest first. They are never synced.
abstract interface class RecentSearchesStore {
  Future<List<String>> read();

  Future<void> write(List<String> searches);
}

class SharedPreferencesRecentSearchesStore implements RecentSearchesStore {
  SharedPreferencesRecentSearchesStore({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const _key = 'history.recent_searches.v1';

  final SharedPreferencesAsync _preferences;

  @override
  Future<List<String>> read() async {
    try {
      final encoded = await _preferences.getString(_key);
      if (encoded == null) return const [];
      final decoded = jsonDecode(encoded);
      if (decoded is! List) return const [];
      return [for (final entry in decoded) ?(entry is String ? entry : null)];
    } on Object {
      return const [];
    }
  }

  @override
  Future<void> write(List<String> searches) async {
    try {
      if (searches.isEmpty) {
        await _preferences.remove(_key);
      } else {
        await _preferences.setString(_key, jsonEncode(searches));
      }
    } on Object {
      // Recent searches are a convenience; a failed write keeps them in memory.
    }
  }
}

/// Recent searches for tests and hosts without persistent preferences.
class MemoryRecentSearchesStore implements RecentSearchesStore {
  List<String> _searches = const [];

  @override
  Future<List<String>> read() async => _searches;

  @override
  Future<void> write(List<String> searches) async =>
      _searches = List.unmodifiable(searches);
}
