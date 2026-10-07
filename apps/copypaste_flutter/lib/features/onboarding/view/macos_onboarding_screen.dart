import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../platform/macos/macos_setup_gateway.dart';
import '../controller/macos_onboarding_controller.dart';
import 'onboarding_intro.dart';
import 'onboarding_scaffold.dart';
import 'onboarding_setting_row.dart';

class MacosOnboardingScreen extends StatefulWidget {
  const MacosOnboardingScreen({
    super.key,
    required this.controller,
    required this.onFinished,
    this.unifiedTitleBar = false,
  });

  final MacosOnboardingController controller;
  final Future<void> Function() onFinished;
  final bool unifiedTitleBar;

  @override
  State<MacosOnboardingScreen> createState() => _MacosOnboardingScreenState();
}

class _MacosOnboardingScreenState extends State<MacosOnboardingScreen>
    with WidgetsBindingObserver {
  MacosOnboardingController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed &&
        controller.step == MacosOnboardingStep.setup) {
      unawaited(controller.refreshSystemState());
    }
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: controller,
    builder: (context, child) => OnboardingScaffold(
      stepIndex: controller.step.index,
      stepCount: MacosOnboardingStep.values.length,
      platform: 'macOS',
      unifiedTitleBar: widget.unifiedTitleBar,
      busy: controller.busy,
      errorMessage: controller.errorMessage,
      onBack: controller.step == MacosOnboardingStep.setup && !controller.busy
          ? controller.showPreviousStep
          : null,
      action: Button.primary(
        onPressed: controller.busy
            ? null
            : switch (controller.step) {
                MacosOnboardingStep.welcome => controller.showSetup,
                MacosOnboardingStep.setup => controller.continueFromSetup,
                MacosOnboardingStep.sync => _finish,
              },
        child: Text(
          controller.step == MacosOnboardingStep.sync
              ? 'Get started'
              : 'Continue',
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          switch (controller.step) {
            MacosOnboardingStep.welcome => const OnboardingIntro.welcome(),
            MacosOnboardingStep.setup => _Setup(controller: controller),
            MacosOnboardingStep.sync => const OnboardingIntro.ready(),
          },
          if (controller.noticeMessage case final message?) ...[
            const Gap(AppSpacing.lg),
            Alert(
              title: const Text('Optional setup needs attention'),
              content: Text(message),
              trailing:
                  controller.step == MacosOnboardingStep.setup &&
                      controller.loginItemNeedsAttention
                  ? Button.secondary(
                      onPressed: controller.openLoginItemsSettings,
                      child: const Text('Open Settings'),
                    )
                  : null,
            ),
          ],
        ],
      ),
    ),
  );

  Future<void> _finish() async {
    if (await controller.finish()) await widget.onFinished();
  }
}

class _Setup extends StatelessWidget {
  const _Setup({required this.controller});
  final MacosOnboardingController controller;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Text('Set up your Mac', style: Theme.of(context).typography.h1),
      const Gap(AppSpacing.xxl),
      Card(
        child: OnboardingSettingRow(
          icon: LucideIcons.keyboard,
          title: 'Accessibility',
          description: controller.accessibilityGranted
              ? 'Auto-paste enabled'
              : 'Optional for auto-paste',
          statusIcon: controller.accessibilityGranted
              ? LucideIcons.circleCheck
              : null,
          action: controller.accessibilityGranted
              ? null
              : Button.ghost(
                  onPressed: controller.busy
                      ? null
                      : controller.requestAccessibility,
                  child: const Text('Enable'),
                ),
        ),
      ),
      const Gap(AppSpacing.lg),
      Card(
        child: OnboardingSettingRow(
          icon: LucideIcons.power,
          title: 'Start at login',
          description:
              controller.loginItemStatus ==
                  MacosLoginItemStatus.developmentUnavailable
              ? 'Available in the installed CopyPaste app.'
              : 'Open CopyPaste when you sign in',
          action: Semantics(
            label: 'Start CopyPaste at login',
            toggled: controller.launchAtLogin,
            child: Switch(
              value: controller.launchAtLogin,
              onChanged: controller.busy || !controller.launchAtLoginAvailable
                  ? null
                  : controller.setLaunchAtLogin,
            ),
          ),
        ),
      ),
      if (controller.loginItemStatus ==
          MacosLoginItemStatus.requiresApproval) ...[
        const Gap(AppSpacing.md),
        Alert(
          title: const Text('Login Item approval is required'),
          content: const Text(
            'Allow CopyPaste in System Settings, then return.',
          ),
          trailing: Button.secondary(
            onPressed: controller.openLoginItemsSettings,
            child: const Text('Open Settings'),
          ),
        ),
      ],
    ],
  );
}
