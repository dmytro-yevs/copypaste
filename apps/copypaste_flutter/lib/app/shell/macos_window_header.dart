import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../theme/app_theme.dart';
import '../theme/app_tokens.dart';

/// Places a title in the macOS native title bar without covering the traffic
/// lights; the title area drags the window.
class MacosWindowHeader extends StatelessWidget {
  const MacosWindowHeader({super.key, required this.title});

  static const nativeControlsWidth = AppControlSize.touch + AppSpacing.xxxl;

  final Widget title;

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
            Expanded(
              child: DragToMoveArea(
                key: const ValueKey<String>('macos-window-drag-region'),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: DefaultTextStyle.merge(
                    style: AppTheme.navigationTextStyle(context),
                    child: title,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
