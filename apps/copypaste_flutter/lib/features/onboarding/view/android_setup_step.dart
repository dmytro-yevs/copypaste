import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../controller/android_onboarding_setup.dart';
import '../repository/android_onboarding_store.dart';
import '../../../shared/setup_setting_row.dart';
import '../../../shared/android_access_setup.dart';

class AndroidSetupStep extends StatelessWidget {
  const AndroidSetupStep({super.key, required this.setup});
  final AndroidOnboardingSetup setup;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final state = setup.setupState;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          setup.mode == AndroidCaptureMode.full
              ? l10n.onboardingAndroidFullCapture
              : l10n.onboardingAndroidBackgroundCapture,
          style: Theme.of(context).typography.h1,
        ),
        const Gap(AppSpacing.lg),
        RadioGroup<AndroidCaptureMode>(
          value: setup.selectedMode,
          onChanged: (mode) => unawaited(setup.selectMode(mode)),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              RadioCard<AndroidCaptureMode>(
                value: AndroidCaptureMode.full,
                child: _Choice(
                  title: l10n.onboardingAndroidFullCapture,
                  description: l10n.onboardingAndroidFullCaptureDescription,
                ),
              ),
              const Gap(AppSpacing.sm),
              RadioCard<AndroidCaptureMode>(
                value: AndroidCaptureMode.limited,
                child: _Choice(
                  title: l10n.onboardingAndroidLimitedCapture,
                  description: l10n.onboardingAndroidLimitedCaptureDescription,
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
                  title: l10n.onboardingAndroidNotification,
                  description: l10n.onboardingAndroidNotificationDescription,
                  statusIcon: state?.notificationGranted == true
                      ? LucideIcons.circleCheck
                      : null,
                  action: state?.notificationGranted == true
                      ? null
                      : Button.ghost(
                          onPressed: setup.busy
                              ? null
                              : setup.requestNotifications,
                          child: Text(l10n.sharedAllow),
                        ),
                ),
                const Gap(AppSpacing.md),
                SetupSettingRow(
                  icon: LucideIcons.batteryCharging,
                  title: l10n.onboardingAndroidBackgroundActivity,
                  description:
                      l10n.onboardingAndroidBackgroundActivityDescription,
                  statusIcon: state?.batteryExempt == true
                      ? LucideIcons.circleCheck
                      : null,
                  action: state?.batteryExempt == true
                      ? null
                      : Button.ghost(
                          onPressed: setup.busy
                              ? null
                              : setup.requestBatteryExemption,
                          child: Text(l10n.onboardingAndroidOpen),
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
              applyAccessLabel: l10n.onboardingAndroidApplyAccess,
              applyAccessDescription:
                  l10n.onboardingAndroidApplyAccessDescription,
              onOpenShizuku: setup.openShizuku,
              onApplyAccess: setup.applyShizukuGrants,
              onCopyCommands: setup.copyAdbCommands,
            ),
          const Gap(AppSpacing.lg),
          _Verification(setup: setup),
        ] else ...[
          const Gap(AppSpacing.lg),
          Text(l10n.onboardingAndroidFullCaptureLater).muted().textSmall(),
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
                ? context.l10n.onboardingAndroidCaptureWorking
                : context.l10n.onboardingAndroidVerifyCopy,
            description: setup.verified
                ? context.l10n.onboardingAndroidCopyReached
                : context.l10n.onboardingAndroidCopyInstruction,
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
                child: Text(context.l10n.onboardingAndroidStartCapture),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
