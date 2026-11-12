import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../platform/startup/launch_at_login.dart';
import '../../../shared/setup_setting_row.dart';
import '../controller/macos_onboarding_setup.dart';

class MacosSetupStep extends StatelessWidget {
  const MacosSetupStep({super.key, required this.setup});

  final MacosOnboardingSetup setup;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Text(
        context.l10n.onboardingMacosTitle,
        style: Theme.of(context).typography.h1,
      ),
      const Gap(AppSpacing.xxl),
      Card(
        child: SetupSettingRow(
          icon: LucideIcons.keyboard,
          title: context.l10n.onboardingAccessibility,
          description: setup.accessibilityGranted
              ? context.l10n.onboardingAutoPasteEnabled
              : context.l10n.onboardingAutoPasteOptional,
          statusIcon: setup.accessibilityGranted
              ? LucideIcons.circleCheck
              : null,
          action: setup.accessibilityGranted
              ? null
              : Button.ghost(
                  onPressed: setup.busy ? null : setup.requestAccessibility,
                  child: Text(context.l10n.commonEnable),
                ),
        ),
      ),
      const Gap(AppSpacing.lg),
      Card(
        child: SetupSettingRow(
          icon: LucideIcons.power,
          title: context.l10n.onboardingStartAtLogin,
          description: setup.launchAtLoginAvailable
              ? context.l10n.onboardingStartAtLoginDescription
              : context.l10n.onboardingMacosStartAtLoginUnavailable,
          action: Semantics(
            label: context.l10n.onboardingStartAtLoginSemantics,
            toggled: setup.launchAtLogin,
            child: AppSwitchFrame(
              child: Switch(
                value: setup.launchAtLogin,
                enabled: !setup.busy && setup.launchAtLoginAvailable,
                onChanged: setup.busy || !setup.launchAtLoginAvailable
                    ? null
                    : setup.setLaunchAtLogin,
              ),
            ),
          ),
        ),
      ),
      if (setup.startupStatus == LaunchAtLoginStatus.requiresApproval) ...[
        const Gap(AppSpacing.md),
        Alert(
          title: Text(context.l10n.onboardingLoginItemApprovalTitle),
          content: Text(context.l10n.onboardingLoginItemApprovalMessage),
          trailing: Button.secondary(
            onPressed: setup.openSystemSettings,
            child: Text(context.l10n.onboardingOpenSystemSettings),
          ),
        ),
      ],
    ],
  );
}
