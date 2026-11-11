import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_brand_logo.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';

/// Brand and concise introductory content shared by the first and final steps.
class OnboardingIntro extends StatelessWidget {
  const OnboardingIntro.welcome({super.key}) : ready = false;
  const OnboardingIntro.ready({super.key}) : ready = true;

  final bool ready;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      const AppBrandLogo(size: AppIconSize.hero),
      const Gap(AppSpacing.xxl),
      Text(
        ready
            ? context.l10n.onboardingReadyTitle
            : context.l10n.onboardingWelcomeTitle,
        style: Theme.of(context).typography.h1,
      ),
      if (!ready) ...[
        const Gap(AppSpacing.sm),
        Text(context.l10n.onboardingWelcomeSubtitle).muted(),
      ],
    ],
  );
}
