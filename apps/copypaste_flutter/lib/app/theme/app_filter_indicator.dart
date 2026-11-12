import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../l10n/l10n.dart';
import 'app_theme.dart';
import 'app_tokens.dart';

/// Marks active filter controls without changing their dimensions or lifecycle.
class AppFilterIndicator extends StatelessWidget {
  const AppFilterIndicator({
    super.key,
    required this.active,
    required this.child,
    this.tooltip,
  }) : count = null;

  /// Shows [count] on the control instead of a dot; zero shows nothing.
  const AppFilterIndicator.count({
    super.key,
    required int this.count,
    required this.child,
    this.tooltip,
  }) : active = count > 0;

  static const _maxShownCount = 99;

  final bool active;
  final int? count;
  final Widget child;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final count = this.count;
    final control = Stack(
      clipBehavior: Clip.none,
      children: [
        child,
        if (active)
          PositionedDirectional(
            top: count == null ? AppSpacing.xs : AppSpacing.zero,
            end: count == null ? AppSpacing.xs : AppSpacing.zero,
            child: IgnorePointer(
              child: count == null
                  ? Semantics(
                      label: context.l10n.appActiveFilter,
                      child: const AvatarBadge(
                        size: AppIconSize.statusIndicator,
                        borderRadius: AppRadius.full,
                        color: AppTheme.navigationAccent,
                      ),
                    )
                  : Semantics(
                      value: '$count',
                      child: ExcludeSemantics(
                        child: PrimaryBadge(
                          style: AppTheme.countBadgeStyle,
                          child: Text(
                            count > _maxShownCount
                                ? '$_maxShownCount+'
                                : '$count',
                          ),
                        ),
                      ),
                    ),
            ),
          ),
      ],
    );
    return tooltip == null
        ? control
        : Tooltip(
            tooltip: (_) => TooltipContainer(child: Text(tooltip!)),
            child: control,
          );
  }
}
