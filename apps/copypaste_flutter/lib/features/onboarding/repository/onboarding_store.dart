import 'package:shared_preferences/shared_preferences.dart';

enum OnboardingPlatform { macos, windows, linux }

abstract interface class OnboardingStore {
  Future<bool> isComplete();
  Future<void> markComplete();
}

class SharedPreferencesOnboardingStore implements OnboardingStore {
  SharedPreferencesOnboardingStore({
    required OnboardingPlatform platform,
    int currentVersion = 1,
    SharedPreferencesAsync? preferences,
  }) : _versionKey = 'onboarding.${platform.name}.version',
       _currentVersion = currentVersion,
       _preferences = preferences ?? SharedPreferencesAsync();
  final String _versionKey;
  final int _currentVersion;
  final SharedPreferencesAsync _preferences;
  @override
  Future<bool> isComplete() async =>
      (await _preferences.getInt(_versionKey) ?? 0) >= _currentVersion;
  @override
  Future<void> markComplete() =>
      _preferences.setInt(_versionKey, _currentVersion);
}

class MemoryOnboardingStore implements OnboardingStore {
  MemoryOnboardingStore({this.complete = false});
  bool complete;
  @override
  Future<bool> isComplete() async => complete;
  @override
  Future<void> markComplete() async => complete = true;
}
