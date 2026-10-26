import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../shared/setup_setting_row.dart';

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
      onBack:
          controller.step == WindowsOnboardingStep.welcome || controller.busy
          ? null
          : controller.showPreviousStep,
      action: Button.primary(
        onPressed: controller.busy
            ? null
            : switch (controller.step) {
                WindowsOnboardingStep.welcome => controller.showSetup,
                WindowsOnboardingStep.setup => controller.continueFromSetup,
                WindowsOnboardingStep.sync => _finish,
              },
        child: Text(
          controller.step == WindowsOnboardingStep.sync
              ? 'Get started'
              : 'Continue',
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          switch (controller.step) {
            WindowsOnboardingStep.welcome => const OnboardingIntro.welcome(),
            WindowsOnboardingStep.setup => Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text('Set up your PC', style: Theme.of(context).typography.h1),
                const Gap(AppSpacing.xxl),
                Card(
                  child: SetupSettingRow(
                    icon: LucideIcons.power,
                    title: 'Start at login',
                    description: controller.launchAtLoginAvailable
                        ? 'Open CopyPaste when you sign in'
                        : 'Unavailable in this installation. You can continue without it.',
                    action: Semantics(
                      label: 'Start CopyPaste at login',
                      toggled: controller.launchAtLogin,
                      child: Switch(
                        value: controller.launchAtLogin,
                        onChanged:
                            controller.busy ||
                                !controller.launchAtLoginAvailable
                            ? null
                            : controller.setLaunchAtLogin,
                      ),
                    ),
                  ),
                ),
              ],
            ),
            WindowsOnboardingStep.sync => const OnboardingIntro.ready(),
          },
          if (controller.noticeMessage case final message?) ...[
            const Gap(AppSpacing.lg),
            Alert(
              title: const Text('Optional setup needs attention'),
              content: Text(message),
            ),
          ],
        ],
      ),
    ),
  );

  Future<void> _finish() async {
    if (await controller.finish()) await onFinished();
  }
}
