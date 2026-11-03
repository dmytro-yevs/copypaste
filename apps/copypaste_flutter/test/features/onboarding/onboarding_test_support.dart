import 'package:copypaste_flutter/features/onboarding/controller/android_onboarding_setup.dart';
import 'package:copypaste_flutter/features/onboarding/controller/linux_onboarding_setup.dart';
import 'package:copypaste_flutter/features/onboarding/controller/macos_onboarding_setup.dart';
import 'package:copypaste_flutter/features/onboarding/controller/onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/controller/windows_onboarding_setup.dart';
import 'package:copypaste_flutter/features/onboarding/repository/android_onboarding_store.dart';
import 'package:copypaste_flutter/features/onboarding/repository/onboarding_store.dart';
import 'package:copypaste_flutter/platform/android/android_capture_setup_gateway.dart';
import 'package:copypaste_flutter/platform/macos/macos_setup_gateway.dart';
import 'package:copypaste_flutter/platform/permissions/linux_integration.dart';
import 'package:copypaste_flutter/platform/startup/launch_at_login.dart';

OnboardingController macosOnboarding({
  required OnboardingStore store,
  required MacosSetupGateway setup,
}) => OnboardingController(
  setup: MacosOnboardingSetup(store: store, setup: setup),
);

OnboardingController windowsOnboarding({
  required OnboardingStore store,
  LaunchAtLoginGateway? startup,
}) => OnboardingController(
  setup: WindowsOnboardingSetup(store: store, startup: startup),
);

OnboardingController linuxOnboarding({
  required OnboardingStore store,
  required LinuxIntegrationPort integration,
}) => OnboardingController(
  setup: LinuxOnboardingSetup(store: store, integration: integration),
);

OnboardingController androidOnboarding({
  required AndroidOnboardingStore store,
  required AndroidCaptureSetupGateway setup,
}) => OnboardingController(
  setup: AndroidOnboardingSetup(store: store, setup: setup),
);
