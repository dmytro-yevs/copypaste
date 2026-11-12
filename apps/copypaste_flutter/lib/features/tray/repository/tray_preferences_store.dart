import 'package:shared_preferences/shared_preferences.dart';

import '../models/tray_menu_model.dart';

/// How many Recent rows the tray shows, and how much of each.
class TrayPreferences {
  const TrayPreferences({
    this.recentLimit = defaultRecentLimit,
    this.previewLevel = TrayPreviewLevel.full,
  });

  static const defaultRecentLimit = 5;
  static const recentLimitOptions = [3, 5, trayRecentMaxItems];

  final int recentLimit;
  final TrayPreviewLevel previewLevel;

  TrayPreferences copyWith({
    int? recentLimit,
    TrayPreviewLevel? previewLevel,
  }) => TrayPreferences(
    recentLimit: recentLimit ?? this.recentLimit,
    previewLevel: previewLevel ?? this.previewLevel,
  );

  @override
  bool operator ==(Object other) =>
      other is TrayPreferences &&
      other.recentLimit == recentLimit &&
      other.previewLevel == previewLevel;

  @override
  int get hashCode => Object.hash(recentLimit, previewLevel);
}

abstract interface class TrayPreferencesStore {
  Future<TrayPreferences> read();
  Future<void> write(TrayPreferences preferences);
}

class MemoryTrayPreferencesStore implements TrayPreferencesStore {
  MemoryTrayPreferencesStore([this._preferences = const TrayPreferences()]);

  TrayPreferences _preferences;

  @override
  Future<TrayPreferences> read() async => _preferences;

  @override
  Future<void> write(TrayPreferences preferences) async {
    _preferences = preferences;
  }
}

/// Device-local, like the Android widget's per-instance options.
class SharedPreferencesTrayPreferencesStore implements TrayPreferencesStore {
  SharedPreferencesTrayPreferencesStore({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const _limitKey = 'tray.recent_limit';
  static const _levelKey = 'tray.preview_level';

  final SharedPreferencesAsync _preferences;

  @override
  Future<TrayPreferences> read() async {
    final limit = await _preferences.getInt(_limitKey);
    final level = await _preferences.getString(_levelKey);
    return TrayPreferences(
      recentLimit: TrayPreferences.recentLimitOptions.contains(limit)
          ? limit!
          : TrayPreferences.defaultRecentLimit,
      previewLevel: TrayPreviewLevel.values.firstWhere(
        (value) => value.name == level,
        orElse: () => TrayPreviewLevel.full,
      ),
    );
  }

  @override
  Future<void> write(TrayPreferences preferences) async {
    await _preferences.setInt(_limitKey, preferences.recentLimit);
    await _preferences.setString(_levelKey, preferences.previewLevel.name);
  }
}
