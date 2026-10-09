import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../shared/adaptive_breakpoints.dart';

import 'app_motion.dart';
import 'app_theme.dart';
import 'app_tokens.dart';

/// Shared presentation and behavior for application-owned overlays.
///
/// Platform file pickers, permission prompts, and system menus remain native.
abstract final class AppOverlays {
  static const Color scrimColor = Color(0x99000000);

  static const DrawerConfiguration _bottomDrawerConfiguration =
      DrawerConfiguration(
        position: OverlayPosition.bottom,
        expands: true,
        draggable: true,
        barrierDismissible: true,
        transformBackdrop: false,
        showDragHandle: true,
        dragHandleSize: Size(
          AppOverlaySize.dragHandleWidth,
          AppOverlaySize.dragHandleHeight,
        ),
        surfaceOpacity: 1,
        surfaceBlur: 0,
        barrierColor: scrimColor,
        borderRadius: BorderRadius.vertical(top: Radius.circular(AppRadius.xl)),
      );

  static const DrawerConfiguration _sideDrawerConfiguration =
      DrawerConfiguration(
        position: OverlayPosition.right,
        expands: true,
        draggable: false,
        barrierDismissible: true,
        transformBackdrop: false,
        showDragHandle: false,
        surfaceOpacity: 1,
        surfaceBlur: 0,
        barrierColor: scrimColor,
        borderRadius: BorderRadius.horizontal(
          left: Radius.circular(AppRadius.xl),
        ),
      );

  static DrawerConfiguration drawerConfiguration(BuildContext context) =>
      MediaQuery.sizeOf(context).width >= AdaptiveBreakpoints.inspector
      ? _sideDrawerConfiguration
      : _bottomDrawerConfiguration;

  static BoxConstraints drawerContentConstraints(BuildContext context) =>
      MediaQuery.sizeOf(context).width >= AdaptiveBreakpoints.inspector
      ? const BoxConstraints.tightFor(
          width: AppOverlaySize.drawerPanelWidth,
          height: double.infinity,
        )
      : BoxConstraints.tightFor(
          width: double.infinity,
          height:
              MediaQuery.sizeOf(context).height *
              AppOverlaySize.drawerHeightFactor,
        );

  static PopoverConfiguration selectPopoverConfiguration(
    BuildContext context, {
    PopoverConstraint widthConstraint = PopoverConstraint.anchorMinSize,
  }) {
    return PopoverConfiguration(
      alignment: Alignment.topLeft,
      anchorAlignment: Alignment.bottomLeft,
      offset: const Offset(0, AppSpacing.xs),
      widthConstraint: widthConstraint,
      showDuration: AppMotion.resolve(context, AppMotion.standard),
      dismissDuration: AppMotion.resolve(context, AppMotion.quick),
      overlayBarrier: const OverlayBarrier(
        padding: EdgeInsets.symmetric(vertical: AppSpacing.xs),
        borderRadius: BorderRadius.all(Radius.circular(AppRadius.lg)),
      ),
    );
  }

  static Future<T?> showDialog<T>(
    BuildContext context, {
    required WidgetBuilder builder,
    bool useRootNavigator = true,
    bool barrierDismissible = true,
  }) {
    return showOverlay<T>(
      context,
      DialogConfiguration(
        useRootNavigator: useRootNavigator,
        barrierDismissible: barrierDismissible,
      ),
      builder: (dialogContext) => ConstrainedBox(
        constraints: const BoxConstraints(
          maxWidth: AppOverlaySize.dialogMaxWidth,
        ),
        child: builder(dialogContext),
      ),
    ).future;
  }

  static AlertDialog alertDialog({
    required IconData icon,
    required Widget title,
    Widget? content,
    List<Widget>? actions,
  }) {
    return AlertDialog(
      title: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: AppIconSize.sm).iconMutedForeground(),
          const Gap(AppSpacing.lg),
          Flexible(child: title),
        ],
      ),
      content: content,
      actions: actions == null || actions.isEmpty
          ? null
          : [
              Flexible(
                child: Wrap(
                  alignment: WrapAlignment.end,
                  spacing: AppSpacing.sm,
                  runSpacing: AppSpacing.sm,
                  children: [
                    for (final action in actions)
                      ButtonStyleOverride(
                        decoration: AppTheme.actionButtonDecoration,
                        child: action,
                      ),
                  ],
                ),
              ),
            ],
      padding: const EdgeInsets.all(AppSpacing.xl),
      surfaceOpacity: 1,
      surfaceBlur: 0,
      barrierColor: scrimColor,
    );
  }

  static BoxDecoration dialogFieldDecoration(BuildContext context) {
    return BoxDecoration(
      color: Theme.of(context).colorScheme.accent,
      borderRadius: const BorderRadius.all(Radius.circular(AppRadius.md)),
    );
  }
}
