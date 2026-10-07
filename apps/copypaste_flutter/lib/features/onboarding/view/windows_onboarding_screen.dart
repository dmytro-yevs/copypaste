import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../controller/windows_onboarding_controller.dart';
import 'onboarding_intro.dart';
import 'onboarding_scaffold.dart';

class WindowsOnboardingScreen extends StatelessWidget {
  const WindowsOnboardingScreen({
    super.key,
    required this.controller,
    required this.onFinished,
  });

  final WindowsOnboardingController controller;
  final Future<void> Function() onFinished;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: controller,
    builder: (context, child) => OnboardingScaffold(
      stepIndex: controller.step.index,
      stepCount: WindowsOnboardingStep.values.length,
      platform: 'Windows',
      busy: controller.busy,
      errorMessage: controller.errorMessage,
      action: Button.primary(
        onPressed: controller.busy
            ? null
            : controller.step == WindowsOnboardingStep.welcome
            ? controller.showReady
            : _finish,
        child: Text(
          controller.step == WindowsOnboardingStep.welcome
              ? 'Continue'
              : 'Get started',
        ),
      ),
      child: controller.step == WindowsOnboardingStep.welcome
          ? const OnboardingIntro.welcome()
          : const OnboardingIntro.ready(),
    ),
  );

  Future<void> _finish() async {
    if (await controller.finish()) await onFinished();
  }
}
