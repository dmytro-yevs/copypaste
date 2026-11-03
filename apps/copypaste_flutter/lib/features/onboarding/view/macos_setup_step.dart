import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../platform/macos/macos_setup_gateway.dart';
import '../../../shared/setup_setting_row.dart';
import '../controller/macos_onboarding_setup.dart';

class MacosSetupStep extends StatelessWidget {
  const MacosSetupStep({super.key, required this.setup});

  final MacosOnboardingSetup setup;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Text('Set up your Mac', style: Theme.of(context).typography.h1),
      const Gap(AppSpacing.xxl),
      Card(
        child: SetupSettingRow(
          icon: LucideIcons.keyboard,
          title: 'Accessibility',
          description: setup.accessibilityGranted
              ? 'Auto-paste enabled'
              : 'Optional for auto-paste',
          statusIcon: setup.accessibilityGranted
              ? LucideIcons.circleCheck
              : null,
          action: setup.accessibilityGranted
              ? null
              : Button.ghost(
                  onPressed: setup.busy ? null : setup.requestAccessibility,
                  child: const Text('Enable'),
                ),
        ),
      ),
      const Gap(AppSpacing.lg),
      Card(
        child: SetupSettingRow(
          icon: LucideIcons.power,
          title: 'Start at login',
          description:
              setup.loginItemStatus ==
                  MacosLoginItemStatus.developmentUnavailable
              ? 'Available in the installed CopyPaste app.'
              : 'Open CopyPaste when you sign in',
          action: Semantics(
            label: 'Start CopyPaste at login',
            toggled: setup.launchAtLogin,
            child: Switch(
              value: setup.launchAtLogin,
              onChanged: setup.busy || !setup.launchAtLoginAvailable
                  ? null
                  : setup.setLaunchAtLogin,
            ),
          ),
        ),
      ),
      if (setup.loginItemStatus == MacosLoginItemStatus.requiresApproval) ...[
        const Gap(AppSpacing.md),
        Alert(
          title: const Text('Login Item approval is required'),
          content: const Text(
            'Allow CopyPaste in System Settings, then return.',
          ),
          trailing: Button.secondary(
            onPressed: setup.openSystemSettings,
            child: const Text('Open Settings'),
          ),
        ),
      ],
    ],
  );
}
