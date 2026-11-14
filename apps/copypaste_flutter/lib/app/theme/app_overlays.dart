import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../dialog_action_controller.dart';
import '../../l10n/l10n.dart';
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

  static ValueListenable<bool> drawerVisibilityOf(BuildContext context) =>
      _AppDrawerConfiguration.sessionFor(context);

  /// Asks the window's open drawer to close the way its Escape does, so its
  /// close guards apply; false when there is none.
  static bool dismissDrawer(BuildContext context) {
    final session = _AppDrawerConfiguration.sessionFor(context);
    final active = session.active;
    if (active == null || active.isCompleted) return false;
    final dismiss = session.scope?.currentState?.onDismiss;
    if (dismiss != null) {
      dismiss();
    } else {
      active.remove();
    }
    return true;
  }

  /// Binds Escape in drawer content to [onDismiss], which owns the drawer's
  /// close guards; the window's back actions take the same path.
  static Widget drawerDismissal({
    required VoidCallback onDismiss,
    required Widget child,
  }) => _DrawerDismissal(onDismiss: onDismiss, child: child);

  /// [guarded] drawers close only through their own dismissal, so an
  /// unsaved-changes prompt cannot be skipped by a barrier tap or a drag.
  static DrawerConfiguration drawerConfiguration(
    BuildContext context, {
    bool guarded = false,
  }) {
    final registry = OverlayAnchorRegistry.of(context);
    final side =
        MediaQuery.sizeOf(context).width >= AdaptiveBreakpoints.inspector;
    return _AppDrawerConfiguration(
      // Header actions target the body overlay through the library registry.
      anchor: registry.find(drawerBodyAnchor) == null
          ? null
          : LinkedAnchor(drawerBodyAnchor, registry: registry),
      position: side ? OverlayPosition.right : OverlayPosition.bottom,
      expands: true,
      draggable: !side && !guarded,
      barrierDismissible: !guarded,
      transformBackdrop: false,
      showDragHandle: !side && !guarded,
      dragHandleSize: const Size(
        AppOverlaySize.dragHandleWidth,
        AppOverlaySize.dragHandleHeight,
      ),
      surfaceOpacity: 1,
      surfaceBlur: 0,
      barrierColor: scrimColor,
      // A side drawer sits flush under the header, so only its free bottom
      // corner is rounded.
      borderRadius: side ? _sideDrawerRadius : _bottomDrawerRadius,
      // Content is clipped to the corners it reaches; a drag handle keeps it
      // clear of a bottom drawer's corners.
      contentRadius: side
          ? _sideDrawerRadius
          : guarded
          ? _bottomDrawerRadius
          : BorderRadius.zero,
    );
  }

  static const _sideDrawerRadius = BorderRadius.only(
    bottomLeft: Radius.circular(AppRadius.xl),
  );
  static const _bottomDrawerRadius = BorderRadius.vertical(
    top: Radius.circular(AppRadius.xl),
  );

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
    Anchor? anchor,
    bool modal = true,
    bool follow = true,
  }) {
    final parent = OverlayConfiguration.maybeOf(context);
    return _AppPopoverConfiguration(
      regionGroupId: parent is PopoverConfiguration
          ? parent.regionGroupId
          : Object(),
      modal: modal,
      follow: follow,
      anchor: anchor,
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
    double width = AppOverlaySize.dialogMaxWidth,
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
        child: SizedBox(width: width, child: builder(dialogContext)),
      ),
    ).future;
  }

  /// Compact editors anchor to their control and use the shared drawer on
  /// narrow screens. The editor owns its scrollable body and fixed actions.
  static Future<T?> showCompactEditor<T>(
    BuildContext context, {
    Object? anchor,
    double? maxWidth,
    required WidgetBuilder builder,
  }) {
    final compact =
        MediaQuery.sizeOf(context).width < AdaptiveBreakpoints.inspector;
    Widget content(BuildContext context) => ConstrainedBox(
      constraints: BoxConstraints(
        maxWidth:
            (maxWidth ?? AppOverlaySize.dialogMaxWidth) - AppSpacing.md * 2,
        maxHeight:
            MediaQuery.sizeOf(context).height *
            AppOverlaySize.drawerHeightFactor,
      ),
      child: builder(context),
    );
    if (compact) {
      return showOverlay<T>(
        context,
        drawerConfiguration(context),
        builder: (context) => AppMotion.keyboardInsets(
          context,
          child: ConstrainedBox(
            constraints: BoxConstraints(
              maxHeight:
                  MediaQuery.sizeOf(context).height *
                  AppOverlaySize.drawerHeightFactor,
            ),
            child: Padding(
              padding: const EdgeInsets.all(AppSpacing.md),
              child: SizedBox(width: double.infinity, child: builder(context)),
            ),
          ),
        ),
      ).future;
    }
    return showOverlay<T>(
      context,
      selectPopoverConfiguration(
        context,
        widthConstraint: PopoverConstraint.flexible,
        anchor: anchor == null
            ? ContextAnchor(context)
            : LinkedAnchor(anchor, registry: OverlayAnchorRegistry.of(context)),
        follow: anchor != null,
        modal: false,
      ),
      builder: (context) => SurfaceCard(
        theme: const CardTheme(padding: EdgeInsets.all(AppSpacing.md)),
        surfaceOpacity: 1,
        surfaceBlur: 0,
        child: content(context),
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
                                title: Text(context.l10n.appActionFailed),
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
                      child: Text(context.l10n.commonCancel),
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
        child: ClipRect(
          clipper: const _FocusRingClipper(),
          child: SingleChildScrollView(
            clipBehavior: Clip.none,
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

  /// Clips a scroll view while leaving room for the focus ring of a control
  /// at its edge. The scroll view itself must use [Clip.none].
  static Widget focusRingClip({required Widget child}) =>
      ClipRect(clipper: const _FocusRingClipper(), child: child);

  /// Lets a child that clips its content, such as a library panel, reach
  /// [AppTheme.focusRingInset] past its slot so it can pad that room back for
  /// focus rings without moving anything. [axis] limits the reach.
  static Widget focusRingOverflow({required Widget child, Axis? axis}) =>
      LayoutBuilder(
        builder: (context, constraints) {
          const reach = AppTheme.focusRingInset * 2;
          final width = axis == Axis.vertical || !constraints.hasBoundedWidth
              ? null
              : constraints.maxWidth + reach;
          final height =
              axis == Axis.horizontal || !constraints.hasBoundedHeight
              ? null
              : constraints.maxHeight + reach;
          return OverflowBox(
            minWidth: width,
            maxWidth: width,
            minHeight: height,
            maxHeight: height,
            child: child,
          );
        },
      );

  static BoxDecoration dialogFieldDecoration(BuildContext context) {
    return BoxDecoration(
      color: Theme.of(context).colorScheme.accent,
      borderRadius: const BorderRadius.all(Radius.circular(AppRadius.md)),
    );
  }
}

/// Keeps vertical scroll clipping while leaving room for the focus ring that
/// the library paints just outside a control touching the body's edge.
class _FocusRingClipper extends CustomClipper<Rect> {
  const _FocusRingClipper();

  @override
  Rect getClip(Size size) =>
      (Offset.zero & size).inflate(AppTheme.focusRingInset);

  // Visibility checks (semantics, show-on-screen) must see the same rect.
  @override
  Rect getApproximateClipRect(Size size) => getClip(size);

  @override
  bool shouldReclip(_FocusRingClipper oldClipper) => false;
}

/// Applies the shared popup surface to stock shadcn select and menu content.
class _AppPopoverConfiguration extends PopoverConfiguration {
  const _AppPopoverConfiguration({
    super.anchor,
    required super.alignment,
    super.anchorAlignment,
    super.offset,
    super.widthConstraint,
    super.showDuration,
    super.dismissDuration,
    super.overlayBarrier,
    super.modal,
    super.follow,
    super.regionGroupId,
  });

  @override
  OverlayConfiguration get nonAdaptive => this;

  @override
  OverlayConfiguration adaptiveConversion(BuildContext context) => this;

  @override
  OverlayCompleter<T?> show<T>(BuildContext context, WidgetBuilder builder) =>
      super.show<T>(
        context,
        (popupContext) => AppTheme.popupSurface(
          popupContext,
          child: Builder(builder: builder),
        ),
      );
}

/// Each application window has one drawer session, shared by all entry points.
class _AppDrawerConfiguration extends DrawerConfiguration {
  const _AppDrawerConfiguration({
    super.anchor,
    super.position,
    super.expands,
    super.draggable,
    super.barrierDismissible,
    super.transformBackdrop,
    super.showDragHandle,
    super.dragHandleSize,
    super.surfaceOpacity,
    super.surfaceBlur,
    super.barrierColor,
    super.borderRadius,
    this.contentRadius = BorderRadius.zero,
  });

  /// The library paints square content over the drawer's rounded corners.
  final BorderRadius contentRadius;

  static final _sessions = Expando<_DrawerSession>();

  static _DrawerSession sessionFor(BuildContext context) {
    final window = Overlay.of(context, rootOverlay: true);
    return _sessions[window] ??= _DrawerSession();
  }

  /// The library scopes focus around the drawer and the backdrop together and
  /// drops focus on open, so the drawer content gets its own scope (Tab stays
  /// inside) and focus returns to the opener when the drawer closes.
  @override
  OverlayCompleter<T?> show<T>(BuildContext context, WidgetBuilder builder) {
    final session = sessionFor(context);
    final previous = session.active;
    // Only keyboard focus returns: on touch, refocusing a text field would
    // raise the soft keyboard again.
    final focused =
        FocusManager.instance.highlightMode == FocusHighlightMode.traditional
        ? FocusManager.instance.primaryFocus
        : null;
    // A drawer replacing an open one returns focus to the first opener.
    final opener = previous != null && !previous.isCompleted
        ? session.opener
        : focused;
    if (previous != null && !previous.isCompleted) previous.remove();
    final scope = GlobalKey<_DrawerFocusScopeState>();
    final drawer = super.show<T>(context, (context) {
      final content = builder(context);
      return _DrawerFocusScope(
        key: scope,
        child: contentRadius == BorderRadius.zero
            ? content
            : ClipRRect(borderRadius: contentRadius, child: content),
      );
    });
    session.active = drawer;
    session.scope = scope;
    session.opener = opener;
    session.value = true;
    drawer.future.whenComplete(() {
      if (!identical(session.active, drawer)) return;
      session.active = null;
      session.scope = null;
      session.opener = null;
      session.value = false;
      // Focus the user already moved elsewhere stays where it is.
      final focusInDrawer =
          FocusManager.instance.primaryFocus == null ||
          (scope.currentState?.hasFocus ?? false);
      if (focusInDrawer &&
          opener != null &&
          opener.context != null &&
          opener.canRequestFocus) {
        opener.requestFocus();
      }
    });
    return drawer;
  }
}

/// Gives drawer content its own focus scope and moves focus into it once the
/// drawer is mounted; the library's shared scope remembers the opener instead.
class _DrawerFocusScope extends StatefulWidget {
  const _DrawerFocusScope({super.key, required this.child});

  final Widget child;

  @override
  State<_DrawerFocusScope> createState() => _DrawerFocusScopeState();
}

class _DrawerFocusScopeState extends State<_DrawerFocusScope> {
  final _scope = FocusScopeNode(debugLabel: 'AppDrawer');

  /// Set by [_DrawerDismissal] in the content.
  VoidCallback? onDismiss;

  bool get hasFocus => _scope.hasFocus;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && !_scope.hasFocus) _scope.requestFocus();
    });
  }

  @override
  void dispose() {
    _scope.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      FocusScope(node: _scope, child: widget.child);
}

class _DrawerDismissal extends StatefulWidget {
  const _DrawerDismissal({required this.onDismiss, required this.child});

  final VoidCallback onDismiss;
  final Widget child;

  @override
  State<_DrawerDismissal> createState() => _DrawerDismissalState();
}

class _DrawerDismissalState extends State<_DrawerDismissal> {
  _DrawerFocusScopeState? _drawer;

  void _dismiss() => widget.onDismiss();

  @override
  void initState() {
    super.initState();
    _drawer = context.findAncestorStateOfType<_DrawerFocusScopeState>()
      ?..onDismiss = _dismiss;
  }

  @override
  void dispose() {
    if (_drawer?.onDismiss == _dismiss) _drawer!.onDismiss = null;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {const SingleActivator(LogicalKeyboardKey.escape): _dismiss},
    child: widget.child,
  );
}

class _DrawerSession extends ValueNotifier<bool> {
  _DrawerSession() : super(false);
  OverlayCompleter? active;
  GlobalKey<_DrawerFocusScopeState>? scope;
  FocusNode? opener;
}
