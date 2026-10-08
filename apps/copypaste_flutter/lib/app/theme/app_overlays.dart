import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app_motion.dart';
import 'app_theme.dart';
import 'app_tokens.dart';

/// Shared presentation and behavior for application-owned overlays.
///
/// Platform file pickers, permission prompts, and system menus remain native.
abstract final class AppOverlays {
  static const Color scrimColor = Color(0x99000000);

  static const DrawerConfiguration bottomDrawerConfiguration =
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

  static BoxConstraints drawerContentConstraints(BuildContext context) =>
      BoxConstraints.tightFor(
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
        child: SizedBox(width: double.infinity, child: builder(dialogContext)),
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
      leading: Icon(icon, size: AppIconSize.sm),
      title: title,
      content: content,
      actions: actions
          ?.map(
            (action) => ButtonStyleOverride(
              decoration: AppTheme.actionButtonDecoration,
              child: action,
            ),
          )
          .toList(),
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
