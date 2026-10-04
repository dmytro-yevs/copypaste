import 'package:shared_preferences/shared_preferences.dart';

abstract interface class MacosOnboardingStore {
  Future<bool> isComplete();

  Future<void> markComplete();
}

class SharedPreferencesMacosOnboardingStore implements MacosOnboardingStore {
  SharedPreferencesMacosOnboardingStore({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const _versionKey = 'onboarding.macos.version';
  static const _currentVersion = 1;

  final SharedPreferencesAsync _preferences;

  @override
  Future<bool> isComplete() async {
    final version = await _preferences.getInt(_versionKey);
    return version != null && version >= _currentVersion;
  }

  @override
  Future<void> markComplete() {
    return _preferences.setInt(_versionKey, _currentVersion);
  }
}

class MemoryMacosOnboardingStore implements MacosOnboardingStore {
  MemoryMacosOnboardingStore({this.complete = false});

  bool complete;

  @override
  Future<bool> isComplete() async => complete;

  @override
  Future<void> markComplete() async {
    complete = true;
  }
}
