import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';

import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../theme/app_motion.dart';
import '../theme/app_overlays.dart';
import 'app_header.dart';
import 'app_mobile_navigation.dart';
import 'header_action_content.dart';
import 'header_fit.dart';
import '../navigation/app_destination_viewport.dart';
import '../../platform/desktop/desktop_shortcuts.dart';

/// Hosts persistent primary destinations under the application header, which
/// carries their navigation while it fits and hands it to the bottom bar
/// otherwise.
class AppShell extends StatefulWidget {
  const AppShell({
    super.key,
    required this.controller,
    required this.destinations,
    this.headerActions = const {},
    this.headerStatus = const [],
    this.headerEnd = const [],
    this.unifiedTitleBar = false,
    this.onCloseWindow,
    this.onQuit,
    this.onToggleFullScreen,
    this.onExitFullScreen,
    this.onRefresh,
  }) : assert(
         destinations.length == AppDestination.values.length,
         'Provide a root widget for every app destination.',
       );

  /// Below this width the destinations always use the bottom navigation.
  static const double compactBreakpoint = 640;

  final AppNavigationController controller;
  final Map<AppDestination, Widget> destinations;
  final Map<AppDestination, List<Widget>> headerActions;

  /// Status cells of every destination, shown as one group after the
  /// commands, in order: capture, sync, storage. A cell carries its secondary
  /// text in [HeaderActionContent.detail], which leaves with the others at
  /// [HeaderStage.labeled]; at [HeaderStage.compact] cells keep only their
  /// icons inside the group.
  final List<Widget> headerStatus;

  /// Controls at the far end of every destination's header, after the status
  /// group, such as transfers and notifications.
  final List<Widget> headerEnd;
  final bool unifiedTitleBar;
  final Future<void> Function()? onCloseWindow;
  final Future<void> Function()? onQuit;
  final Future<void> Function()? onToggleFullScreen;
  final Future<void> Function()? onExitFullScreen;
  final VoidCallback? onRefresh;

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  // The scaffold arranges its body differently with a floating footer; the
  // key carries destinations and open panels across that change.
  final _bodyKey = GlobalKey();
  late List<Widget> _destinationChildren;

  @override
  void initState() {
    super.initState();
    _destinationChildren = _childrenFor(widget.destinations);
  }

  @override
  void didUpdateWidget(covariant AppShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller ||
        !identical(oldWidget.destinations, widget.destinations)) {
      _destinationChildren = _childrenFor(widget.destinations);
    }
  }

  List<Widget> _childrenFor(Map<AppDestination, Widget> destinations) {
    return AppDestination.values
        .map(
          (destination) => KeyedSubtree(
            key: ValueKey<AppDestination>(destination),
            child: PrimaryScrollController(
              controller: widget.controller.scrollControllerFor(destination),
              automaticallyInheritForPlatforms: const {},
              child: destinations[destination]!,
            ),
          ),
        )
        .toList(growable: false);
  }

  @override
  Widget build(BuildContext context) {
    final drawerVisibility = AppOverlays.drawerVisibilityOf(context);
    return AnimatedBuilder(
      animation: Listenable.merge([widget.controller, drawerVisibility]),
      builder: (context, child) => HeaderFit(
        minimumWidth: AppShell.compactBreakpoint,
        horizontalInset: MediaQuery.paddingOf(context).horizontal,
        stageBuilder: (context, stage) {
          final bottomNavigation = !stage.showsHeaderNavigation;
          final selectedDestination = widget.controller.selectedDestination;
          final mobileNavigationVisible =
              bottomNavigation &&
              MediaQuery.viewInsetsOf(context).bottom == 0 &&
              !widget.controller.bottomOverlayOpen &&
              !drawerVisibility.value;
          final systemBottomPadding = MediaQuery.paddingOf(context).bottom;
          final content = Scaffold(
            // Keep floating-footer padding visible to scrollables when the
            // scaffold does not need to consume keyboard insets.
            resizeToAvoidBottomInset:
                !bottomNavigation ||
                MediaQuery.viewInsetsOf(context).bottom > 0,
            floatingFooter: bottomNavigation,
            footers: [
              if (mobileNavigationVisible)
                AppMobileNavigation(controller: widget.controller),
            ],
            // Drawer barriers may paint outside their bounds; keep the
            // header visible as well as outside the modal hit-test region.
            child: ClipRect(
              key: _bodyKey,
              // Scaffold owns another overlay; body drawers must be roots.
              child: Data<DrawerLayerData>.boundary(
                child: DrawerOverlay(
                  key: const ValueKey('app-body-drawer-overlay'),
                  child: OverlayAnchor(
                    anchor: AppOverlays.drawerBodyAnchor,
                    child: Builder(
                      builder: (context) {
                        final media = MediaQuery.of(context);
                        // The floating footer already includes the system safe area.
                        final bottom =
                            (media.padding.bottom -
                                    (mobileNavigationVisible
                                        ? systemBottomPadding
                                        : 0))
                                .clamp(0.0, double.infinity)
                                .toDouble();
                        return MediaQuery(
                          data: media.copyWith(
                            padding: media.padding.copyWith(bottom: bottom),
                          ),
                          child: Semantics(
                            container: true,
                            explicitChildNodes: true,
                            child: AppNavigationHost(
                              controller: widget.controller,
                              child: AppDestinationViewport(
                                controller: widget.controller,
                                swipeEnabled: bottomNavigation,
                                children: _destinationChildren,
                              ),
                            ),
                          ),
                        );
                      },
                    ),
                  ),
                ),
              ),
            ),
          );

          return CallbackShortcuts(
            bindings: <ShortcutActivator, VoidCallback>{
              ...desktopWindowBindings(
                onClose: widget.onCloseWindow,
                onQuit: widget.onQuit,
                onToggleFullScreen: widget.onToggleFullScreen,
              ),
              if (usesDesktopShortcuts) ...{
                for (final item in appNavigationDestinations)
                  desktopShortcut(item.shortcutKey): () =>
                      _activateByShortcut(item.destination),
                if (defaultTargetPlatform == TargetPlatform.macOS)
                  desktopShortcut(LogicalKeyboardKey.bracketLeft): () =>
                      unawaited(_back()),
                desktopShortcut(LogicalKeyboardKey.keyF):
                    widget.controller.findInHistory,
                if (widget.onRefresh != null)
                  ...desktopRefreshBindings(widget.onRefresh!),
                if (defaultTargetPlatform != TargetPlatform.macOS) ...{
                  const SingleActivator(
                    LogicalKeyboardKey.browserBack,
                    includeRepeats: false,
                  ): () =>
                      unawaited(_back()),
                  const SingleActivator(
                    LogicalKeyboardKey.arrowLeft,
                    alt: true,
                    includeRepeats: false,
                  ): () =>
                      unawaited(_back()),
                },
              },
              desktopShortcut(LogicalKeyboardKey.comma): () =>
                  _activate(AppDestination.settings),
              const SingleActivator(LogicalKeyboardKey.escape): () async {
                if (!await _back()) await widget.onExitFullScreen?.call();
              },
            },
            // Receives the shell keys before any control is focused; Tab goes
            // straight to the first header control.
            child: Focus(
              autofocus: true,
              skipTraversal: true,
              child: Listener(
                onPointerDown: _usesMouseBack ? _handlePointerDown : null,
                child: OverlayAnchorScope(
                  child: SafeArea(
                    top: !widget.unifiedTitleBar,
                    bottom: !bottomNavigation,
                    child: Column(
                      children: [
                        HeaderFitRow(
                          child: AppHeader(
                            unifiedTitleBar: widget.unifiedTitleBar,
                            selected: selectedDestination,
                            onSelected: _activate,
                            actions:
                                widget.headerActions[selectedDestination] ??
                                const <Widget>[],
                            status: widget.headerStatus,
                            end: widget.headerEnd,
                          ),
                        ),
                        Expanded(child: content),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          );
        },
      ),
    );
  }

  /// The mouse back button follows the Windows and Linux convention only;
  /// Android delivers back through the system.
  static bool get _usesMouseBack =>
      defaultTargetPlatform == TargetPlatform.windows ||
      defaultTargetPlatform == TargetPlatform.linux;

  bool get _dialogOpen => Navigator.of(context, rootNavigator: true).canPop();

  /// A press that starts with another button held, such as a selection drag,
  /// never arrives as a pointer-down of the back button alone.
  void _handlePointerDown(PointerDownEvent event) {
    if (event.kind != PointerDeviceKind.mouse ||
        event.buttons != kBackMouseButton ||
        _dialogOpen) {
      return;
    }
    unawaited(_back());
  }

  /// Closes the open panel, else the nested page, else returns to History.
  Future<bool> _back() async {
    if (AppOverlays.dismissDrawer(context)) return true;
    return widget.controller.handleEscape();
  }

  void _activateByShortcut(AppDestination destination) {
    if (_dialogOpen || _composingText) return;
    _activate(destination);
  }

  /// An input method may use the same keys while it composes text.
  bool get _composingText {
    final composing = FocusManager.instance.primaryFocus?.context
        ?.findAncestorStateOfType<EditableTextState>()
        ?.textEditingValue
        .composing;
    return composing != null && composing.isValid && !composing.isCollapsed;
  }

  void _activate(AppDestination destination) {
    unawaited(
      widget.controller.activateDestination(
        destination,
        disableAnimations: AppMotion.reducedMotionOf(context),
      ),
    );
  }
}
