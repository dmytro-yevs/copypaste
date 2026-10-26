import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../theme/app_motion.dart';
import '../theme/app_overlays.dart';
import '../theme/app_theme.dart';
import '../theme/app_tokens.dart';
import 'macos_window_header.dart';
import 'app_mobile_navigation.dart';
import '../navigation/app_destination_viewport.dart';
import '../../platform/desktop/desktop_shortcuts.dart';

/// Hosts persistent primary destinations inside the adaptive shadcn navigation.
class AppShell extends StatefulWidget {
  const AppShell({
    super.key,
    required this.controller,
    required this.destinations,
    this.headerActions = const {},
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

  static const double compactBreakpoint = 640;
  static const double expandedBreakpoint = 1024;

  final AppNavigationController controller;
  final Map<AppDestination, Widget> destinations;
  final Map<AppDestination, List<Widget>> headerActions;
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
  final _navigationHostKey = GlobalKey();
  late List<Widget> _destinationChildren;
  late AppDestination _lastDestination;
  bool? _desktopRailExpanded;

  @override
  void initState() {
    super.initState();
    _destinationChildren = _childrenFor(widget.destinations);
    _lastDestination = widget.controller.selectedDestination;
    if (_lastDestination == AppDestination.settings) {
      _desktopRailExpanded = false;
    }
    widget.controller.addListener(_handleNavigationChange);
  }

  @override
  void didUpdateWidget(covariant AppShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_handleNavigationChange);
      _lastDestination = widget.controller.selectedDestination;
      if (_lastDestination == AppDestination.settings) {
        _desktopRailExpanded = false;
      }
      widget.controller.addListener(_handleNavigationChange);
      _destinationChildren = _childrenFor(widget.destinations);
    }
    if (!identical(oldWidget.destinations, widget.destinations)) {
      _destinationChildren = _childrenFor(widget.destinations);
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_handleNavigationChange);
    super.dispose();
  }

  void _handleNavigationChange() {
    final destination = widget.controller.selectedDestination;
    if (destination == _lastDestination) return;
    _lastDestination = destination;
    if (destination == AppDestination.settings &&
        _desktopRailExpanded != false) {
      setState(() => _desktopRailExpanded = false);
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
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, child) {
        return LayoutBuilder(
          builder: (context, constraints) {
            final mode = _ShellNavigationMode.forWidth(constraints.maxWidth);
            final theme = Theme.of(context);
            final selectedKey = ValueKey<AppDestination>(
              widget.controller.selectedDestination,
            );
            final selectedDestination = widget.controller.selectedDestination;
            final mobileNavigationVisible =
                mode == _ShellNavigationMode.bottom &&
                MediaQuery.viewInsetsOf(context).bottom == 0 &&
                !widget.controller.bottomOverlayOpen;
            final systemBottomPadding = MediaQuery.paddingOf(context).bottom;
            final railExpanded = mode == _ShellNavigationMode.wide
                ? (_desktopRailExpanded ?? true)
                : false;
            final headerActions =
                widget.headerActions[selectedDestination] ?? const <Widget>[];
            final railToggle = mode == _ShellNavigationMode.wide
                ? Tooltip(
                    showDuration: AppMotion.resolve(
                      context,
                      AppMotion.standard,
                    ),
                    tooltip: (context) => TooltipContainer(
                      child: Text(
                        railExpanded
                            ? 'Collapse navigation'
                            : 'Expand navigation',
                      ),
                    ),
                    child: Button.ghost(
                      key: const ValueKey<String>('navigation-rail-toggle'),
                      style: AppTheme.navigationIconButtonStyle,
                      onPressed: _toggleDesktopRail,
                      child: const AppIconFrame(
                        child: Icon(LucideIcons.panelLeft),
                      ),
                    ),
                  )
                : null;
            final content = Scaffold(
              // Keep floating-footer padding visible to scrollables when the
              // scaffold does not need to consume keyboard insets.
              resizeToAvoidBottomInset:
                  mode != _ShellNavigationMode.bottom ||
                  MediaQuery.viewInsetsOf(context).bottom > 0,
              floatingFooter: mode == _ShellNavigationMode.bottom,
              footers: [
                if (mobileNavigationVisible)
                  AppMobileNavigation(controller: widget.controller),
              ],
              headers: widget.unifiedTitleBar
                  ? const []
                  : [
                      AppBar(
                        leading: [
                          ?railToggle,
                          if (mode == _ShellNavigationMode.bottom)
                            Button.ghost(
                              style: AppTheme.brandButtonStyle,
                              onPressed: _openHistory,
                              child: const Image(
                                key: ValueKey<String>('header-brand-logo'),
                                image: AssetImage('assets/brand/copypaste.png'),
                                width: AppIconSize.md,
                                height: AppIconSize.md,
                                semanticLabel: 'Open History',
                              ),
                            ),
                        ],
                        title: Text(
                          selectedDestination.navigationDestination.label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        trailing: headerActions,
                      ),
                      const Divider(),
                    ],
              // Drawer barriers may paint outside their bounds; keep the
              // header visible as well as outside the modal hit-test region.
              child: ClipRect(
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
                                key: _navigationHostKey,
                                controller: widget.controller,
                                child: AppDestinationViewport(
                                  controller: widget.controller,
                                  swipeEnabled:
                                      mode == _ShellNavigationMode.bottom,
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

            final navigation = switch (mode) {
              _ShellNavigationMode.compact || _ShellNavigationMode.wide => Row(
                children: [
                  NavigationRail(
                    backgroundColor: theme.colorScheme.secondary,
                    padding: EdgeInsets.symmetric(
                      horizontal: railExpanded ? AppSpacing.lg : AppSpacing.sm,
                      vertical: AppSpacing.lg,
                    ),
                    expandedSize: 220,
                    expanded: railExpanded,
                    labelType: NavigationLabelType.expanded,
                    labelPosition: NavigationLabelPosition.end,
                    header: [_navigationBrand(theme, railExpanded)],
                    selectedKey: selectedKey,
                    onSelected: _selectDestination,
                    children: _navigationItems(
                      context: context,
                      theme: theme,
                      expanded: railExpanded,
                      withTooltips: !railExpanded,
                    ),
                  ),
                  const VerticalDivider(),
                  Expanded(child: content),
                ],
              ),
              _ShellNavigationMode.bottom => content,
            };

            return CallbackShortcuts(
              bindings: <ShortcutActivator, VoidCallback>{
                ...desktopWindowBindings(
                  onClose: widget.onCloseWindow,
                  onQuit: widget.onQuit,
                  onToggleFullScreen: widget.onToggleFullScreen,
                ),
                if (usesDesktopShortcuts) ...{
                  if (defaultTargetPlatform == TargetPlatform.macOS)
                    desktopShortcut(LogicalKeyboardKey.bracketLeft): () =>
                        unawaited(widget.controller.handleEscape()),
                  desktopShortcut(LogicalKeyboardKey.keyF):
                      widget.controller.findInHistory,
                  if (widget.onRefresh != null) ...{
                    desktopShortcut(LogicalKeyboardKey.keyR): widget.onRefresh!,
                    if (defaultTargetPlatform != TargetPlatform.macOS)
                      const SingleActivator(
                        LogicalKeyboardKey.f5,
                        includeRepeats: false,
                      ): widget.onRefresh!,
                  },
                  if (defaultTargetPlatform != TargetPlatform.macOS) ...{
                    const SingleActivator(
                      LogicalKeyboardKey.browserBack,
                      includeRepeats: false,
                    ): () =>
                        unawaited(widget.controller.handleEscape()),
                    const SingleActivator(
                      LogicalKeyboardKey.arrowLeft,
                      alt: true,
                      includeRepeats: false,
                    ): () =>
                        unawaited(widget.controller.handleEscape()),
                  },
                },
                SingleActivator(
                  LogicalKeyboardKey.comma,
                  meta: defaultTargetPlatform == TargetPlatform.macOS,
                  control: defaultTargetPlatform != TargetPlatform.macOS,
                ): () => _selectDestination(
                  const ValueKey<AppDestination>(AppDestination.settings),
                ),
                const SingleActivator(LogicalKeyboardKey.escape): () async {
                  if (!await widget.controller.handleEscape()) {
                    await widget.onExitFullScreen?.call();
                  }
                },
              },
              child: Focus(
                autofocus: true,
                child: OverlayAnchorScope(
                  child: Column(
                    children: [
                      if (widget.unifiedTitleBar)
                        MacosWindowHeader(
                          title: Text(
                            selectedDestination.navigationDestination.label,
                          ),
                          leading: railToggle,
                          actions: headerActions,
                        ),
                      Expanded(
                        child: SafeArea(
                          top: !widget.unifiedTitleBar,
                          bottom: mode != _ShellNavigationMode.bottom,
                          child: navigation,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            );
          },
        );
      },
    );
  }

  List<Widget> _navigationItems({
    required BuildContext context,
    required ThemeData theme,
    required bool expanded,
    bool withTooltips = false,
  }) {
    return appNavigationDestinations
        .map((destination) {
          Widget item = NavigationItem(
            key: ValueKey<AppDestination>(destination.destination),
            style: AppTheme.navigationRailButtonStyle(context, selected: false),
            selectedStyle: AppTheme.navigationRailButtonStyle(
              context,
              selected: true,
            ),
            alignment: expanded ? Alignment.centerLeft : Alignment.center,
            label: ExcludeSemantics(
              child: Text(
                destination.label,
                style: AppTheme.navigationTextStyle(context),
              ),
            ),
            child: Semantics(
              label: destination.label,
              button: true,
              selected:
                  widget.controller.selectedDestination ==
                  destination.destination,
              child: ExcludeSemantics(
                child: SizedBox.square(
                  key: ValueKey<String>(
                    'navigation-icon-${destination.destination.name}',
                  ),
                  dimension: AppIconSize.xl,
                  child: Center(
                    child: AppIconFrame(
                      size: AppIconSize.md,
                      child: Icon(destination.icon, size: AppIconSize.md),
                    ),
                  ),
                ),
              ),
            ),
          );
          if (withTooltips) {
            item = Tooltip(
              showDuration: AppMotion.resolve(context, AppMotion.standard),
              tooltip: (context) =>
                  TooltipContainer(child: Text(destination.label)),
              child: item,
            );
          }
          return item;
        })
        .toList(growable: false);
  }

  Widget _navigationBrand(ThemeData theme, bool expanded) {
    return ConstrainedBox(
      key: const ValueKey<String>('navigation-brand'),
      constraints: const BoxConstraints(minHeight: AppIconSize.xl),
      child: Align(
        heightFactor: 1,
        alignment: expanded ? Alignment.centerLeft : Alignment.center,
        child: Button.ghost(
          style: AppTheme.brandButtonStyle,
          onPressed: _openHistory,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              const Image(
                key: ValueKey<String>('copypaste-brand-logo'),
                image: AssetImage('assets/brand/copypaste.png'),
                width: AppIconSize.xl,
                height: AppIconSize.xl,
                semanticLabel: 'Open History',
              ),
              if (expanded) ...[
                const Gap(AppSpacing.sm),
                Text(
                  'CopyPaste',
                  style: AppTheme.navigationTextStyle(context)
                      .copyWith(color: theme.colorScheme.foreground),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  void _selectDestination(Key? key) {
    for (final destination in AppDestination.values) {
      if (key == ValueKey<AppDestination>(destination)) {
        unawaited(
          widget.controller.activateDestination(
            destination,
            disableAnimations: AppMotion.reducedMotionOf(context),
          ),
        );
        return;
      }
    }
  }

  void _openHistory() => _selectDestination(
    const ValueKey<AppDestination>(AppDestination.history),
  );

  void _toggleDesktopRail() {
    setState(() {
      _desktopRailExpanded = !(_desktopRailExpanded ?? true);
    });
  }
}

enum _ShellNavigationMode {
  bottom,
  compact,
  wide;

  static _ShellNavigationMode forWidth(double width) {
    if (width < AppShell.compactBreakpoint) return bottom;
    if (width < AppShell.expandedBreakpoint) return compact;
    return wide;
  }
}
