import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../controller/android_onboarding_setup.dart';
import '../controller/linux_onboarding_setup.dart';
import '../controller/macos_onboarding_setup.dart';
import '../controller/onboarding_controller.dart';
import '../controller/onboarding_setup.dart';
import '../controller/windows_onboarding_setup.dart';
import 'android_setup_step.dart';
import 'linux_setup_step.dart';
import 'macos_setup_step.dart';
import 'onboarding_intro.dart';
import 'onboarding_scaffold.dart';
import 'windows_setup_step.dart';

/// Welcome, platform setup, and ready steps for every platform.
class OnboardingScreen extends StatefulWidget {
  const OnboardingScreen({
    super.key,
    required this.controller,
    required this.onFinished,
    this.unifiedTitleBar = false,
  });

  final OnboardingController controller;
  final Future<void> Function() onFinished;
  final bool unifiedTitleBar;

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

class _OnboardingScreenState extends State<OnboardingScreen>
    with WidgetsBindingObserver {
  OnboardingController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    controller.setAttached(true);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    controller.setAttached(false);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    controller.setForeground(state == AppLifecycleState.resumed);
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: controller,
    builder: (context, child) {
      final step = controller.step;
      final setup = controller.setup;
      return OnboardingScaffold(
        stepIndex: step.index,
        stepCount: OnboardingStep.values.length,
        platform: setup.platformLabel,
        unifiedTitleBar: widget.unifiedTitleBar,
        busy: controller.busy,
        errorMessage: controller.errorMessage,
        onBack: controller.canGoBack ? controller.showPreviousStep : null,
        action: Button.primary(
          onPressed: controller.canContinue ? _next : null,
          child: Text(
            step == OnboardingStep.ready ? 'Get started' : 'Continue',
          ),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            switch (step) {
              OnboardingStep.welcome => const OnboardingIntro.welcome(),
              OnboardingStep.setup => _SetupStep(setup: setup),
              OnboardingStep.ready => const OnboardingIntro.ready(),
            },
            if (controller.noticeMessage case final message?) ...[
              const Gap(AppSpacing.lg),
              Alert(
                title: const Text('Optional setup needs attention'),
                content: Text(message),
                trailing:
                    step == OnboardingStep.setup &&
                        setup.noticeOpensSystemSettings
                    ? Button.secondary(
                        onPressed: setup.openSystemSettings,
                        child: const Text('Open Settings'),
                      )
                    : null,
              ),
            ],
          ],
        ),
      );
    },
  );

  Future<void> _next() async {
    switch (controller.step) {
      case OnboardingStep.welcome:
        await controller.showSetup();
      case OnboardingStep.setup:
        await controller.continueFromSetup();
      case OnboardingStep.ready:
        if (await controller.finish()) await widget.onFinished();
    }
  }
}

class _SetupStep extends StatelessWidget {
  const _SetupStep({required this.setup});

  final OnboardingSetup setup;

  @override
  Widget build(BuildContext context) => switch (setup) {
    final MacosOnboardingSetup setup => MacosSetupStep(setup: setup),
    final WindowsOnboardingSetup setup => WindowsSetupStep(setup: setup),
    final LinuxOnboardingSetup setup => LinuxSetupStep(setup: setup),
    final AndroidOnboardingSetup setup => AndroidSetupStep(setup: setup),
    _ => throw UnsupportedError('No setup view for ${setup.platformLabel}.'),
  };
}
