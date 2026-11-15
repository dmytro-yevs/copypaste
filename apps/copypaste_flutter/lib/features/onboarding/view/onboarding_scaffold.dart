import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/shell/macos_window_header.dart';
import '../../../app/theme/app_brand_logo.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/step_progress.dart';

/// Shared Focus layout and adaptive navigation for every onboarding platform.
class OnboardingScaffold extends StatelessWidget {
  const OnboardingScaffold({
    super.key,
    required this.steps,
    required this.stepIndex,
    required this.child,
    required this.action,
    this.onBack,
    this.busy = false,
    this.unifiedTitleBar = false,
  });

  /// Short names of every step, shown above the content of the current one.
  final List<String> steps;
  final int stepIndex;
  final Widget child;
  final Widget action;
  final VoidCallback? onBack;
  final bool busy;
  final bool unifiedTitleBar;

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
                : Button.ghost(
                    onPressed: onBack,
                    child: Text(context.l10n.commonBack),
                  );
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
                    StepProgress(steps: steps, current: stepIndex),
                    const Gap(AppSpacing.xxl),
                    child,
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
      const AppBrandLogo(size: AppIconSize.md),
      const Gap(AppSpacing.sm),
      Flexible(
        child: Text(
          context.l10n.onboardingTitle,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
      ),
    ],
  );
}
