import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../controller/onboarding_controller.dart';

/// Whether this device starts a space or joins one. Choosing a card moves on
/// at once; "Later" lives in the footer.
class OnboardingSpaceStep extends StatelessWidget {
  const OnboardingSpaceStep({super.key, required this.onChoose});

  /// Null while the step cannot advance.
  final void Function(OnboardingSpaceChoice choice)? onChoose;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final choose = onChoose;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l10n.onboardingSpaceTitle, style: Theme.of(context).typography.h3),
        const Gap(AppSpacing.sm),
        Text(l10n.onboardingSpaceSubtitle).muted(),
        const Gap(AppSpacing.xxl),
        _SpaceChoice(
          key: const ValueKey<String>('onboarding-space-create'),
          icon: LucideIcons.sparkles,
          title: l10n.onboardingSpaceCreateTitle,
          description: l10n.onboardingSpaceCreateDescription,
          onPressed: choose == null
              ? null
              : () => choose(OnboardingSpaceChoice.create),
        ),
        const Gap(AppSpacing.sm),
        _SpaceChoice(
          key: const ValueKey<String>('onboarding-space-join'),
          icon: LucideIcons.link,
          title: l10n.onboardingSpaceJoinTitle,
          description: l10n.onboardingSpaceJoinDescription,
          onPressed: choose == null
              ? null
              : () => choose(OnboardingSpaceChoice.join),
        ),
      ],
    );
  }
}

class _SpaceChoice extends StatelessWidget {
  const _SpaceChoice({
    super.key,
    required this.icon,
    required this.title,
    required this.description,
    required this.onPressed,
  });

  final IconData icon;
  final String title;
  final String description;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) => Button.card(
    onPressed: onPressed,
    child: Row(
      children: [
        AppIconFrame(
          size: AppIconSize.lg,
          child: Icon(icon, size: AppIconSize.md),
        ),
        const Gap(AppSpacing.md),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title).medium(),
              const Gap(AppSpacing.xxs),
              Text(
                description,
                style: Theme.of(context).typography.xSmall,
              ).muted(),
            ],
          ),
        ),
        const Gap(AppSpacing.md),
        const Icon(LucideIcons.chevronRight, size: AppIconSize.sm),
      ],
    ),
  );
}
