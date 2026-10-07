import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/shell/macos_window_header.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';

/// Shared Focus layout and adaptive navigation for every onboarding platform.
class OnboardingScaffold extends StatelessWidget {
  const OnboardingScaffold({
    super.key,
    required this.stepIndex,
    required this.stepCount,
    required this.platform,
    required this.child,
    required this.action,
    this.onBack,
    this.busy = false,
    this.unifiedTitleBar = false,
    this.errorMessage,
  });

  final int stepIndex;
  final int stepCount;
  final String platform;
  final Widget child;
  final Widget action;
  final VoidCallback? onBack;
  final bool busy;
  final bool unifiedTitleBar;
  final String? errorMessage;

  @override
  Widget build(BuildContext context) => Scaffold(
    headers: [
      if (unifiedTitleBar)
        const MacosWindowHeader(title: _OnboardingTitle())
      else
        const AppBar(title: _OnboardingTitle()),
      const Divider(),
    ],
    footers: [
      const Divider(),
      Padding(
        padding: const EdgeInsets.all(AppSpacing.lg),
        child: LayoutBuilder(
          builder: (context, constraints) {
            final back = onBack == null
                ? null
                : Button.ghost(onPressed: onBack, child: const Text('Back'));
            if (constraints.maxWidth <
                AppLayoutSize.onboardingContentMaxWidth) {
              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  action,
                  if (back != null) ...[const Gap(AppSpacing.sm), back],
                ],
              );
            }
            return Row(children: [?back, const Spacer(), action]);
          },
        ),
      ),
    ],
    loadingProgressIndeterminate: busy,
    child: LayoutBuilder(
      builder: (context, constraints) {
        final compact =
            constraints.maxWidth < AppLayoutSize.onboardingContentMaxWidth;
        final vertical = compact ? AppSpacing.xxl : AppSpacing.huge;
        return SingleChildScrollView(
          padding: EdgeInsets.symmetric(
            horizontal: compact ? AppSpacing.xl : AppSpacing.xxl,
            vertical: vertical,
          ),
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minHeight: (constraints.maxHeight - vertical * 2).clamp(
                AppSpacing.zero,
                double.infinity,
              ),
            ),
            child: Align(
              alignment: compact ? Alignment.topCenter : Alignment.center,
              child: ConstrainedBox(
                constraints: const BoxConstraints(
                  maxWidth: AppLayoutSize.onboardingContentMaxWidth,
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Progress(
                      progress: stepIndex + 1,
                      min: 0,
                      max: stepCount.toDouble(),
                      disableAnimation: MediaQuery.disableAnimationsOf(context),
                    ),
                    const Gap(AppSpacing.xxxl),
                    Text(
                      'STEP ${stepIndex + 1} OF $stepCount · ${platform.toUpperCase()}',
                      style: Theme.of(context).typography.xSmall.copyWith(
                        color: Theme.of(context).colorScheme.mutedForeground,
                      ),
                    ),
                    const Gap(AppSpacing.md),
                    child,
                    if (errorMessage != null) ...[
                      const Gap(AppSpacing.lg),
                      StateView.error(
                        title: 'Setup needs attention',
                        message: errorMessage,
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        );
      },
    ),
  );
}

class _OnboardingTitle extends StatelessWidget {
  const _OnboardingTitle();

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      const Image(
        image: AssetImage('assets/brand/copypaste.png'),
        width: AppIconSize.md,
        height: AppIconSize.md,
      ),
      const Gap(AppSpacing.sm),
      const Flexible(
        child: Text(
          'Set up CopyPaste',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
      ),
    ],
  );
}
