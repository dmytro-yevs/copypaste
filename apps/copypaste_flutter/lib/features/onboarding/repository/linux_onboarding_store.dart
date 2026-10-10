import 'package:shared_preferences/shared_preferences.dart';

abstract interface class LinuxOnboardingStore {
  Future<bool> isComplete();

  Future<void> markComplete();
}

class SharedPreferencesLinuxOnboardingStore implements LinuxOnboardingStore {
  SharedPreferencesLinuxOnboardingStore({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const _versionKey = 'onboarding.linux.version';
  static const _currentVersion = 1;
  final SharedPreferencesAsync _preferences;

  @override
  Future<bool> isComplete() async =>
      (await _preferences.getInt(_versionKey) ?? 0) >= _currentVersion;

  @override
  Future<void> markComplete() =>
      _preferences.setInt(_versionKey, _currentVersion);
}

class MemoryLinuxOnboardingStore implements LinuxOnboardingStore {
  MemoryLinuxOnboardingStore({this.complete = false});

  bool complete;

  @override
  Future<bool> isComplete() async => complete;

  @override
  Future<void> markComplete() async => complete = true;
}
