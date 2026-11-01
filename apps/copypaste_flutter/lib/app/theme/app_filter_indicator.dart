import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app_theme.dart';
import 'app_tokens.dart';

/// Marks active filter controls without changing their dimensions or lifecycle.
class AppFilterIndicator extends StatelessWidget {
  const AppFilterIndicator({
    super.key,
    required this.active,
    required this.child,
    this.tooltip,
  });
  final bool active;
  final Widget child;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final control = Stack(
      clipBehavior: Clip.none,
      children: [
        child,
        if (active)
          PositionedDirectional(
            top: AppSpacing.xs,
            end: AppSpacing.xs,
            child: IgnorePointer(
              child: Semantics(
                label: 'Active filter',
                child: const AvatarBadge(
                  size: AppIconSize.statusIndicator,
                  borderRadius: AppRadius.full,
                  color: AppTheme.navigationAccent,
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
