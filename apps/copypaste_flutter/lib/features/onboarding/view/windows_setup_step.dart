import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../shared/setup_setting_row.dart';
import '../controller/windows_onboarding_setup.dart';

class WindowsSetupStep extends StatelessWidget {
  const WindowsSetupStep({super.key, required this.setup});

  final WindowsOnboardingSetup setup;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Text('Set up your PC', style: Theme.of(context).typography.h1),
      const Gap(AppSpacing.xxl),
      Card(
        child: SetupSettingRow(
          icon: LucideIcons.power,
          title: 'Start at login',
          description: setup.launchAtLoginAvailable
              ? 'Open CopyPaste when you sign in'
              : 'Unavailable in this installation. You can continue without it.',
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
      if (setup.disabledInTaskManager && setup.launchAtLogin) ...[
        const Gap(AppSpacing.md),
        Alert(
          title: const Text('Disabled in Task Manager'),
          content: const Text(
            'Enable CopyPaste in Windows startup apps, then return.',
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
