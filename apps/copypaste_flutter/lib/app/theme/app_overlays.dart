import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../dialog_action_controller.dart';
import '../../shared/adaptive_breakpoints.dart';

import 'app_motion.dart';
import 'app_theme.dart';
import 'app_tokens.dart';

/// Shared presentation and behavior for application-owned overlays.
///
/// Platform file pickers, permission prompts, and system menus remain native.
abstract final class AppOverlays {
  static const drawerBodyAnchor = Symbol('app-drawer-body');
  static const Color scrimColor = Color(0x99000000);

  static DrawerConfiguration drawerConfiguration(BuildContext context) {
    final registry = OverlayAnchorRegistry.of(context);
    final side =
        MediaQuery.sizeOf(context).width >= AdaptiveBreakpoints.inspector;
    return DrawerConfiguration(
      // Header actions target the body overlay through the library registry.
      anchor: registry.find(drawerBodyAnchor) == null
          ? null
          : LinkedAnchor(drawerBodyAnchor, registry: registry),
      position: side ? OverlayPosition.right : OverlayPosition.bottom,
      expands: true,
      draggable: !side,
      barrierDismissible: true,
      transformBackdrop: false,
      showDragHandle: !side,
      dragHandleSize: const Size(
        AppOverlaySize.dragHandleWidth,
        AppOverlaySize.dragHandleHeight,
      ),
      surfaceOpacity: 1,
      surfaceBlur: 0,
      barrierColor: scrimColor,
      borderRadius: side
          ? const BorderRadius.horizontal(left: Radius.circular(AppRadius.xl))
          : const BorderRadius.vertical(top: Radius.circular(AppRadius.xl)),
    );
  }

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
    PopoverConstraint widthConstraint = PopoverConstraint.anchorFixedSize,
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

  /// Uses one adaptive width, with bottom placement on Android and centered
  /// placement on desktop. The library owns safe areas and route transitions;
  /// keyboard insets reduce the available dialog height without hiding actions.
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
        alignment: defaultTargetPlatform == TargetPlatform.android
            ? Alignment.bottomCenter
            : Alignment.center,
      ),
      builder: (dialogContext) => AppMotion.keyboardInsets(
        dialogContext,
        child: SizedBox(
          width: AppOverlaySize.dialogMaxWidth,
          child: builder(dialogContext),
        ),
      ),
    ).future;
  }

  /// Runs an action before closing and keeps failures available for retry.
  /// A null action result leaves the dialog idle after a nested cancellation.
  static Future<bool?> showActionDialog(
    BuildContext context, {
    required IconData icon,
    required Widget title,
    required String actionLabel,
    required Future<bool?> Function() onConfirm,
    required String Function() failureMessage,
    Widget? content,
    bool destructive = true,
    Listenable? changes,
    bool Function()? canConfirm,
  }) async {
    final controller = DialogActionController(
      action: onConfirm,
      failureMessage: failureMessage,
    );
    try {
      return await showDialog<bool>(
        context,
        builder: (context) => AnimatedBuilder(
          animation: Listenable.merge([controller, ?changes]),
          builder: (context, _) {
            final submitting = controller.submitting;
            void cancel() {
              if (!controller.submitting) Navigator.of(context).maybePop();
            }

            Future<void> submit() async {
              if (controller.submitting || !(canConfirm?.call() ?? true)) {
                return;
              }
              final completed = await controller.submit();
              if (completed && context.mounted) Navigator.pop(context, true);
            }

            return PopScope(
              canPop: !submitting,
              child: CallbackShortcuts(
                bindings: {
                  const SingleActivator(LogicalKeyboardKey.escape): cancel,
                },
                child: alertDialog(
                  icon: icon,
                  title: title,
                  content: content == null && controller.errorMessage == null
                      ? null
                      : Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            if (content != null)
                              ExcludeFocus(
                                excluding: submitting,
                                child: AbsorbPointer(
                                  absorbing: submitting,
                                  child: content,
                                ),
                              ),
                            if (controller.errorMessage
                                case final message?) ...[
                              if (content != null) const Gap(AppSpacing.md),
                              Alert.destructive(
                                leading: const AppIconFrame(
                                  child: Icon(LucideIcons.circleAlert),
                                ),
                                title: const Text('Action failed'),
                                content: Text(message),
                              ),
                            ],
                          ],
                        ),
                  actions: [
                    Button.ghost(
                      alignment: Alignment.center,
                      style: AppTheme.dialogButtonStyle(
                        const ButtonStyle.ghost(),
                      ),
                      onPressed: submitting ? null : cancel,
                      child: const Text('Cancel'),
                    ),
                    Button(
                      alignment: Alignment.center,
                      style: AppTheme.dialogButtonStyle(
                        destructive
                            ? const ButtonStyle.destructive()
                            : const ButtonStyle.primary(),
                      ),
                      onPressed: submitting || !(canConfirm?.call() ?? true)
                          ? null
                          : submit,
                      leading: submitting
                          ? const SizedBox.square(
                              dimension: AppIconSize.sm,
                              child: AppIconFrame(
                                size: AppIconSize.sm,
                                child: CircularProgressIndicator(),
                              ),
                            )
                          : null,
                      child: Text(actionLabel),
                    ),
                  ],
                ),
              ),
            );
          },
        ),
      );
    } finally {
      controller.dispose();
    }
  }

  static AlertDialog alertDialog({
    required IconData icon,
    required Widget title,
    Widget? content,
    List<Widget>? actions,
  }) {
    return AlertDialog(
      title: Flexible(
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  AppIconFrame(
                    size: AppIconSize.sm,
                    child: Icon(icon, size: AppIconSize.sm),
                  ).iconMutedForeground(),
                  const Gap(AppSpacing.lg),
                  Flexible(child: title),
                ],
              ),
              if (content != null) ...[
                const Gap(AppSpacing.sm),
                Builder(
                  builder: (context) {
                    final theme = Theme.of(context);
                    return DefaultTextStyle.merge(
                      style: theme.typography.small
                          .merge(theme.typography.normal)
                          .copyWith(color: theme.colorScheme.mutedForeground),
                      child: content,
                    );
                  },
                ),
              ],
            ],
          ),
        ),
      ),
      actions: actions == null || actions.isEmpty
          ? null
          : [Expanded(child: _dialogActions(actions))],
      padding: const EdgeInsets.all(AppSpacing.xl),
      surfaceOpacity: 1,
      surfaceBlur: 0,
      barrierColor: scrimColor,
    );
  }

  static Widget _dialogActions(List<Widget> actions) => DefaultTextStyle.merge(
    maxLines: 1,
    softWrap: false,
    overflow: TextOverflow.ellipsis,
    textAlign: TextAlign.center,
    child: LayoutBuilder(
      builder: (context, constraints) {
        final minimumWidth =
            AppOverlaySize.dialogActionMinWidth *
            MediaQuery.textScalerOf(context).scale(1);
        final stacked =
            constraints.maxWidth <
            actions.length * minimumWidth +
                (actions.length - 1) * AppSpacing.sm;
        if (stacked) {
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (var index = 0; index < actions.length; index++) ...[
                if (index > 0) const Gap(AppSpacing.sm),
                actions[index],
              ],
            ],
          );
        }
        return IntrinsicHeight(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (var index = 0; index < actions.length; index++) ...[
                if (index > 0) const Gap(AppSpacing.sm),
                Expanded(child: actions[index]),
              ],
            ],
          ),
        );
      },
    ),
  );

  static BoxDecoration dialogFieldDecoration(BuildContext context) {
    return BoxDecoration(
      color: Theme.of(context).colorScheme.accent,
      borderRadius: const BorderRadius.all(Radius.circular(AppRadius.md)),
    );
  }
}
