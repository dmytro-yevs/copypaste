import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../controller/android_onboarding_controller.dart';
import '../repository/android_onboarding_store.dart';
import 'onboarding_intro.dart';
import 'onboarding_scaffold.dart';
import '../../../shared/setup_setting_row.dart';
import '../../../shared/android_access_setup.dart';

class AndroidOnboardingScreen extends StatefulWidget {
  const AndroidOnboardingScreen({
    super.key,
    required this.controller,
    required this.onFinished,
  });

  final AndroidOnboardingController controller;
  final Future<void> Function() onFinished;

  @override
  State<AndroidOnboardingScreen> createState() =>
      _AndroidOnboardingScreenState();
}

class _AndroidOnboardingScreenState extends State<AndroidOnboardingScreen>
    with WidgetsBindingObserver {
  AndroidOnboardingController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    controller.setMonitoring(true);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    controller.setMonitoring(false);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    controller.setMonitoring(state == AppLifecycleState.resumed);
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: controller,
    builder: (context, child) => OnboardingScaffold(
      stepIndex: controller.step.index,
      stepCount: AndroidOnboardingStep.values.length,
      platform: 'Android',
      busy: controller.busy,
      errorMessage: controller.errorMessage,
      onBack:
          controller.step == AndroidOnboardingStep.capture && !controller.busy
          ? controller.showPreviousStep
          : null,
      action: Button.primary(
        onPressed: switch (controller.step) {
          AndroidOnboardingStep.welcome =>
            controller.busy ? null : controller.showCapture,
          AndroidOnboardingStep.capture =>
            controller.canContinueCapture
                ? controller.continueFromCapture
                : null,
          AndroidOnboardingStep.sync => controller.busy ? null : _finish,
        },
        child: Text(
          controller.step == AndroidOnboardingStep.sync
              ? 'Get started'
              : 'Continue',
        ),
      ),
      child: switch (controller.step) {
        AndroidOnboardingStep.welcome => const OnboardingIntro.welcome(),
        AndroidOnboardingStep.capture => _CaptureSetup(controller: controller),
        AndroidOnboardingStep.sync => const OnboardingIntro.ready(),
      },
    ),
  );

  Future<void> _finish() async {
    if (await controller.finish()) await widget.onFinished();
  }
}

class _CaptureSetup extends StatelessWidget {
  const _CaptureSetup({required this.controller});
  final AndroidOnboardingController controller;

  @override
  Widget build(BuildContext context) {
    final state = controller.setupState;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          controller.mode == AndroidCaptureMode.full
              ? 'Full capture'
              : 'Background capture',
          style: Theme.of(context).typography.h1,
        ),
        const Gap(AppSpacing.lg),
        RadioGroup<AndroidCaptureMode>(
          value: controller.selectedMode,
          onChanged: (mode) => unawaited(controller.selectMode(mode)),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const RadioCard<AndroidCaptureMode>(
                value: AndroidCaptureMode.full,
                child: _Choice(
                  title: 'Full capture',
                  description: 'Set up once, then verify a real copy.',
                ),
              ),
              const Gap(AppSpacing.sm),
              const RadioCard<AndroidCaptureMode>(
                value: AndroidCaptureMode.limited,
                child: _Choice(
                  title: 'Limited capture',
                  description: 'Share to CopyPaste or return after copying.',
                ),
              ),
            ],
          ),
        ),
        if (controller.selectedMode == AndroidCaptureMode.full) ...[
          const Gap(AppSpacing.lg),
          Card(
            child: Column(
              children: [
                SetupSettingRow(
                  icon: LucideIcons.bell,
                  title: 'Capture notification',
                  description: 'Required while capture runs',
                  statusIcon: state?.notificationGranted == true
                      ? LucideIcons.circleCheck
                      : null,
                  action: state?.notificationGranted == true
                      ? null
                      : Button.ghost(
                          onPressed: controller.busy
                              ? null
                              : controller.requestNotifications,
                          child: const Text('Allow'),
                        ),
                ),
                const Gap(AppSpacing.md),
                SetupSettingRow(
                  icon: LucideIcons.batteryCharging,
                  title: 'Background activity',
                  description: 'Recommended to reduce capture loss',
                  statusIcon: state?.batteryExempt == true
                      ? LucideIcons.circleCheck
                      : null,
                  action: state?.batteryExempt == true
                      ? null
                      : Button.ghost(
                          onPressed: controller.busy
                              ? null
                              : controller.requestBatteryExemption,
                          child: const Text('Open'),
                        ),
                ),
              ],
            ),
          ),
          const Gap(AppSpacing.lg),
          if (state != null)
            AndroidAccessSetup(
              methodIndex: controller.method.index,
              onMethodChanged: (index) => unawaited(
                controller.selectMethod(
                  AndroidCaptureSetupMethod.values[index],
                ),
              ),
              shizuku: state.shizuku,
              granted: state.privilegedGrants,
              busy: controller.busy,
              adbCommands: controller.adbCommandText,
              applyAccessLabel: 'Apply capture access',
              applyAccessDescription: 'Apply one-time capture access.',
              onOpenShizuku: controller.openShizuku,
              onApplyAccess: controller.applyShizukuGrants,
              onCopyCommands: controller.copyAdbCommands,
            ),
          const Gap(AppSpacing.lg),
          _Verification(controller: controller),
        ] else ...[
          const Gap(AppSpacing.lg),
          const Text('You can set up Full capture later.').muted().textSmall(),
        ],
      ],
    );
  }
}

class _Choice extends StatelessWidget {
  const _Choice({required this.title, required this.description});
  final String title;
  final String description;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(title).medium(),
      const Gap(AppSpacing.xs),
      Text(description, style: Theme.of(context).typography.xSmall).muted(),
    ],
  );
}

class _Verification extends StatelessWidget {
  const _Verification({required this.controller});
  final AndroidOnboardingController controller;

  @override
  Widget build(BuildContext context) {
    final state = controller.setupState;
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SetupSettingRow(
            icon: controller.verified
                ? LucideIcons.circleCheck
                : LucideIcons.circle,
            title: controller.verified
                ? 'Background capture is working'
                : 'Verify a real copy',
            description: controller.verified
                ? 'A new copy reached your local History.'
                : 'Copy in another app, then return here.',
          ),
          if (!controller.verified && !controller.verifying) ...[
            const Gap(AppSpacing.md),
            Align(
              alignment: Alignment.centerLeft,
              child: Button.primary(
                onPressed:
                    !controller.busy &&
                        state?.privilegedGrants == true &&
                        state?.notificationGranted == true
                    ? controller.beginVerification
                    : null,
                child: const Text('Start capture'),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
