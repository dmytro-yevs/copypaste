import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';

/// Brand and concise introductory content shared by the first and final steps.
class OnboardingIntro extends StatelessWidget {
  const OnboardingIntro.welcome({super.key}) : ready = false;
  const OnboardingIntro.ready({super.key}) : ready = true;

  final bool ready;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      const Image(
        image: AssetImage('assets/brand/copypaste.png'),
        width: AppIconSize.hero,
        height: AppIconSize.hero,
      ),
      const Gap(AppSpacing.xxl),
      Text(
        ready ? 'CopyPaste is ready' : 'Welcome to CopyPaste',
        style: Theme.of(context).typography.h1,
      ),
      if (!ready) ...[
        const Gap(AppSpacing.sm),
        const Text('Your clipboard, ready when you are.').muted(),
      ],
    ],
  );
}
