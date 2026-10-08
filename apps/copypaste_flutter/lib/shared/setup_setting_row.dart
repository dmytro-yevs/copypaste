import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../app/theme/app_tokens.dart';

/// Aligns a setup icon with its title while keeping the description beneath it.
class SetupSettingRow extends StatelessWidget {
  const SetupSettingRow({
    super.key,
    required this.icon,
    required this.title,
    required this.description,
    this.action,
    this.statusIcon,
  }) : assert(action == null || statusIcon == null);

  final IconData icon;
  final String title;
  final String description;
  final Widget? action;
  final IconData? statusIcon;

  @override
  Widget build(BuildContext context) {
    final details = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Icon(icon, size: AppIconSize.md),
            const Gap(AppSpacing.md),
            Expanded(child: Text(title).medium()),
            if (statusIcon != null) ...[
              const Gap(AppSpacing.md),
              Icon(statusIcon, size: AppIconSize.md),
            ],
          ],
        ),
        const Gap(AppSpacing.xs),
        Padding(
          padding: const EdgeInsets.only(left: AppIconSize.md + AppSpacing.md),
          child: Text(
            description,
            style: Theme.of(context).typography.xSmall,
          ).muted(),
        ),
      ],
    );
    return LayoutBuilder(
      builder: (context, constraints) {
        if (action == null) return details;
        final scaledWidth =
            constraints.maxWidth / MediaQuery.textScalerOf(context).scale(1);
        if (scaledWidth < AppLayoutSize.onboardingContentMaxWidth / 2) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              details,
              const Gap(AppSpacing.md),
              Align(alignment: Alignment.centerLeft, child: action),
            ],
          );
        }
        return Row(
          children: [
            Expanded(child: details),
            const Gap(AppSpacing.md),
            action!,
          ],
        );
      },
    );
  }
}
