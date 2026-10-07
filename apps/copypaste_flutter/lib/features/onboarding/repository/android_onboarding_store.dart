import 'package:shared_preferences/shared_preferences.dart';

enum AndroidCaptureMode { full, limited }

enum AndroidCaptureSetupMethod { shizuku, adb }

class AndroidOnboardingProgress {
  const AndroidOnboardingProgress({
    required this.complete,
    required this.mode,
    required this.method,
    this.captureStarted = false,
    this.verificationBaseline,
  });

  final bool complete;
  final AndroidCaptureMode mode;
  final AndroidCaptureSetupMethod method;
  final bool captureStarted;
  final int? verificationBaseline;
}

abstract interface class AndroidOnboardingStore {
  Future<AndroidOnboardingProgress> read();

  Future<void> writeChoice({
    required AndroidCaptureMode mode,
    required AndroidCaptureSetupMethod method,
  });

  Future<void> markComplete({
    required AndroidCaptureMode mode,
    required AndroidCaptureSetupMethod method,
  });

  Future<void> writeCaptureProgress({int? verificationBaseline});
}

class SharedPreferencesAndroidOnboardingStore
    implements AndroidOnboardingStore {
  SharedPreferencesAndroidOnboardingStore({SharedPreferencesAsync? preferences})
    : _preferences = preferences ?? SharedPreferencesAsync();

  static const _versionKey = 'onboarding.android.version';
  static const _modeKey = 'onboarding.android.capture-mode';
  static const _methodKey = 'onboarding.android.capture-method';
  static const _captureStartedKey = 'onboarding.android.capture-started';
  static const _verificationBaselineKey =
      'onboarding.android.verification-baseline';
  static const _currentVersion = 1;

  final SharedPreferencesAsync _preferences;

  @override
  Future<AndroidOnboardingProgress> read() async {
    final version = await _preferences.getInt(_versionKey);
    final mode = switch (await _preferences.getString(_modeKey)) {
      'limited' => AndroidCaptureMode.limited,
      _ => AndroidCaptureMode.full,
    };
    final method = switch (await _preferences.getString(_methodKey)) {
      'adb' => AndroidCaptureSetupMethod.adb,
      _ => AndroidCaptureSetupMethod.shizuku,
    };
    return AndroidOnboardingProgress(
      complete: version != null && version >= _currentVersion,
      mode: mode,
      method: method,
      captureStarted: await _preferences.getBool(_captureStartedKey) ?? false,
      verificationBaseline: await _preferences.getInt(_verificationBaselineKey),
    );
  }

  @override
  Future<void> writeChoice({
    required AndroidCaptureMode mode,
    required AndroidCaptureSetupMethod method,
  }) async {
    await _preferences.setString(_modeKey, mode.name);
    await _preferences.setString(_methodKey, method.name);
  }

  @override
  Future<void> markComplete({
    required AndroidCaptureMode mode,
    required AndroidCaptureSetupMethod method,
  }) async {
    await writeChoice(mode: mode, method: method);
    await _preferences.setInt(_versionKey, _currentVersion);
  }

  @override
  Future<void> writeCaptureProgress({int? verificationBaseline}) async {
    await _preferences.setBool(_captureStartedKey, true);
    if (verificationBaseline != null) {
      await _preferences.setInt(_verificationBaselineKey, verificationBaseline);
    }
  }
}

class MemoryAndroidOnboardingStore implements AndroidOnboardingStore {
  MemoryAndroidOnboardingStore({
    this.complete = false,
    this.mode = AndroidCaptureMode.full,
    this.method = AndroidCaptureSetupMethod.shizuku,
    this.captureStarted = false,
    this.verificationBaseline,
  });

  bool complete;
  AndroidCaptureMode mode;
  AndroidCaptureSetupMethod method;
  bool captureStarted;
  int? verificationBaseline;

  @override
  Future<AndroidOnboardingProgress> read() async => AndroidOnboardingProgress(
    complete: complete,
    mode: mode,
    method: method,
    captureStarted: captureStarted,
    verificationBaseline: verificationBaseline,
  );

  @override
  Future<void> writeChoice({
    required AndroidCaptureMode mode,
    required AndroidCaptureSetupMethod method,
  }) async {
    this.mode = mode;
    this.method = method;
  }

  @override
  Future<void> markComplete({
    required AndroidCaptureMode mode,
    required AndroidCaptureSetupMethod method,
  }) async {
    await writeChoice(mode: mode, method: method);
    complete = true;
  }

  @override
  Future<void> writeCaptureProgress({int? verificationBaseline}) async {
    captureStarted = true;
    if (verificationBaseline != null) {
      this.verificationBaseline = verificationBaseline;
    }
  }
}
