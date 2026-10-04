import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../theme/app_tokens.dart';

/// Places app navigation in the macOS native title bar without covering traffic
/// lights or making controls part of the draggable region.
class MacosWindowHeader extends StatelessWidget {
  const MacosWindowHeader({
    super.key,
    required this.title,
    this.leading,
    this.actions = const [],
  });

  static const nativeControlsWidth = AppControlSize.touch + AppSpacing.xxxl;

  final Widget title;
  final Widget? leading;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: theme.colorScheme.border)),
      ),
      child: AppBar(
        key: const ValueKey<String>('macos-window-header'),
        backgroundColor: theme.colorScheme.muted,
        height: AppControlSize.touch,
        padding: EdgeInsets.zero,
        useSafeArea: false,
        child: Row(
          children: [
            const SizedBox(
              key: ValueKey<String>('macos-native-controls-reserve'),
              width: nativeControlsWidth,
            ),
            if (leading case final leading?) ...[
              leading,
              const Gap(AppSpacing.sm),
            ],
            Expanded(
              child: DragToMoveArea(
                key: const ValueKey<String>('macos-window-drag-region'),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: DefaultTextStyle.merge(
                    style: theme.typography.large
                        .merge(theme.typography.medium)
                        .copyWith(height: 1),
                    child: title,
                  ),
                ),
              ),
            ),
            if (actions.isNotEmpty) ...[
              const Gap(AppSpacing.sm),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: actions,
              ).gap(AppSpacing.sm),
              const Gap(AppSpacing.lg),
            ],
          ],
        ),
      ),
    );
  }
}
