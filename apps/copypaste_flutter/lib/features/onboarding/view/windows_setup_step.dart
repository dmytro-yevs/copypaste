import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/setup_setting_row.dart';
import '../controller/windows_onboarding_setup.dart';

class WindowsSetupStep extends StatelessWidget {
  const WindowsSetupStep({super.key, required this.setup});

  final WindowsOnboardingSetup setup;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Text(
        context.l10n.onboardingWindowsTitle,
        style: Theme.of(context).typography.h1,
      ),
      const Gap(AppSpacing.xxl),
      Card(
        child: SetupSettingRow(
          icon: LucideIcons.power,
          title: context.l10n.onboardingStartAtLogin,
          description: setup.launchAtLoginAvailable
              ? context.l10n.onboardingStartAtLoginDescription
              : context.l10n.onboardingWindowsStartAtLoginUnavailable,
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
      if (setup.disabledInTaskManager && setup.launchAtLogin) ...[
        const Gap(AppSpacing.md),
        Alert(
          title: Text(context.l10n.onboardingTaskManagerDisabledTitle),
          content: Text(context.l10n.onboardingTaskManagerDisabledMessage),
          trailing: Button.secondary(
            onPressed: setup.openSystemSettings,
            child: Text(context.l10n.onboardingOpenSystemSettings),
          ),
        ),
      ],
    ],
  );
}
