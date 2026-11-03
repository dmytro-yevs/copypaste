import 'dart:io';

import '../../../platform/android/android_capture_setup_gateway.dart';
import '../../../platform/macos/macos_setup_gateway.dart';
import '../../../platform/permissions/linux_integration.dart';
import '../repository/android_onboarding_store.dart';
import '../repository/onboarding_store.dart';
import 'android_onboarding_setup.dart';
import 'linux_onboarding_setup.dart';
import 'macos_onboarding_setup.dart';
import 'onboarding_setup.dart';
import 'windows_onboarding_setup.dart';

/// Creates the setup step for the host platform, or null when it has none.
OnboardingSetup? platformOnboardingSetup() {
  if (Platform.isMacOS) {
    return MacosOnboardingSetup(
      store: SharedPreferencesOnboardingStore(
        platform: OnboardingPlatform.macos,
      ),
      setup: MethodChannelMacosSetupGateway(),
    );
  }
  if (Platform.isWindows) {
    return WindowsOnboardingSetup(
      store: SharedPreferencesOnboardingStore(
        platform: OnboardingPlatform.windows,
      ),
    );
  }
  if (Platform.isLinux) {
    return LinuxOnboardingSetup(
      store: SharedPreferencesOnboardingStore(
        platform: OnboardingPlatform.linux,
      ),
      integration: MethodChannelLinuxIntegrationPort(),
    );
  }
  if (Platform.isAndroid) {
    return AndroidOnboardingSetup(
      store: SharedPreferencesAndroidOnboardingStore(),
      setup: MethodChannelAndroidCaptureSetupGateway(),
    );
  }
  return null;
}
