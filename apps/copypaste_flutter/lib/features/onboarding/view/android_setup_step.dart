import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../controller/android_onboarding_setup.dart';
import '../repository/android_onboarding_store.dart';
import '../../../shared/setup_setting_row.dart';
import '../../../shared/android_access_setup.dart';

class AndroidSetupStep extends StatelessWidget {
  const AndroidSetupStep({super.key, required this.setup});
  final AndroidOnboardingSetup setup;

  @override
  Widget build(BuildContext context) {
    final state = setup.setupState;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          setup.mode == AndroidCaptureMode.full
              ? 'Full capture'
              : 'Background capture',
          style: Theme.of(context).typography.h1,
        ),
        const Gap(AppSpacing.lg),
        RadioGroup<AndroidCaptureMode>(
          value: setup.selectedMode,
          onChanged: (mode) => unawaited(setup.selectMode(mode)),
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
        if (setup.selectedMode == AndroidCaptureMode.full) ...[
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
                          onPressed: setup.busy
                              ? null
                              : setup.requestNotifications,
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
                          onPressed: setup.busy
                              ? null
                              : setup.requestBatteryExemption,
                          child: const Text('Open'),
                        ),
                ),
              ],
            ),
          ),
          const Gap(AppSpacing.lg),
          if (state != null)
            AndroidAccessSetup(
              methodIndex: setup.method.index,
              onMethodChanged: (index) => unawaited(
                setup.selectMethod(AndroidCaptureSetupMethod.values[index]),
              ),
              shizuku: state.shizuku,
              granted: state.privilegedGrants,
              busy: setup.busy,
              adbCommands: setup.adbCommandText,
              applyAccessLabel: 'Apply capture access',
              applyAccessDescription: 'Apply one-time capture access.',
              onOpenShizuku: setup.openShizuku,
              onApplyAccess: setup.applyShizukuGrants,
              onCopyCommands: setup.copyAdbCommands,
            ),
          const Gap(AppSpacing.lg),
          _Verification(setup: setup),
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
  const _Verification({required this.setup});
  final AndroidOnboardingSetup setup;

  @override
  Widget build(BuildContext context) {
    final state = setup.setupState;
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SetupSettingRow(
            icon: setup.verified ? LucideIcons.circleCheck : LucideIcons.circle,
            title: setup.verified
                ? 'Background capture is working'
                : 'Verify a real copy',
            description: setup.verified
                ? 'A new copy reached your local History.'
                : 'Copy in another app, then return here.',
          ),
          if (!setup.verified && !setup.verifying) ...[
            const Gap(AppSpacing.md),
            Align(
              alignment: Alignment.centerLeft,
              child: Button.primary(
                onPressed:
                    !setup.busy &&
                        state?.privilegedGrants == true &&
                        state?.notificationGranted == true
                    ? setup.beginVerification
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
