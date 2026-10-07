import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../theme/app_motion.dart';
import '../theme/app_theme.dart';
import '../theme/app_tokens.dart';
import 'macos_window_header.dart';

/// Hosts persistent primary destinations inside the adaptive shadcn navigation.
class AppShell extends StatefulWidget {
  const AppShell({
    super.key,
    required this.controller,
    required this.destinations,
    this.headerActions = const {},
    this.unifiedTitleBar = false,
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
            child: destinations[destination]!,
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
                      style: const ButtonStyle.ghostIcon(),
                      onPressed: _toggleDesktopRail,
                      child: const Icon(LucideIcons.panelLeft),
                    ),
                  )
                : null;
            final content = Scaffold(
              resizeToAvoidBottomInset: true,
              headers: widget.unifiedTitleBar
                  ? const []
                  : [
                      AppBar(
                        leading: [
                          ?railToggle,
                          if (mode == _ShellNavigationMode.bottom)
                            const Image(
                              key: ValueKey<String>('header-brand-logo'),
                              image: AssetImage('assets/brand/copypaste.png'),
                              width: AppIconSize.md,
                              height: AppIconSize.md,
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
              child: AppNavigationHost(
                key: _navigationHostKey,
                controller: widget.controller,
                child: IndexedStack(
                  index: widget.controller.selectedDestination.index,
                  children: _destinationChildren,
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
              _ShellNavigationMode.bottom => Column(
                children: [
                  Expanded(child: content),
                  if (MediaQuery.viewInsetsOf(context).bottom == 0 &&
                      !widget.controller.bottomOverlayOpen)
                    _mobileNavigationDock(context, selectedKey),
                ],
              ),
            };

            return CallbackShortcuts(
              bindings: <ShortcutActivator, VoidCallback>{
                const SingleActivator(LogicalKeyboardKey.escape): () {
                  widget.controller.handleEscape();
                },
              },
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
            );
          },
        );
      },
    );
  }

  Widget _mobileNavigationDock(BuildContext context, Key selectedKey) {
    return ColoredBox(
      color: Theme.of(context).colorScheme.background,
      child: SafeArea(
        key: const ValueKey<String>('mobile-navigation-dock'),
        top: false,
        child: Padding(
          padding: AppTheme.mobileNavigationMargin,
          child: Center(
            heightFactor: 1,
            child: IntrinsicWidth(
              child: OutlinedContainer(
                theme: AppTheme.mobileNavigationSurfaceTheme(context),
                child: NavigationBar(
                  key: const ValueKey<String>('bottom-navigation'),
                  alignment: NavigationBarAlignment.center,
                  labelType: NavigationLabelType.selected,
                  labelPosition: NavigationLabelPosition.end,
                  labelSize: NavigationLabelSize.large,
                  selectedKey: selectedKey,
                  onSelected: _selectDestination,
                  backgroundColor: Colors.transparent,
                  padding: AppTheme.mobileNavigationPadding,
                  spacing: AppSpacing.xs,
                  children: _bottomNavigationItems(),
                ),
              ),
            ),
          ),
        ),
      ),
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
            style: const ButtonStyle.ghost(density: ButtonDensity.icon),
            selectedStyle:
                const ButtonStyle.secondary(
                  density: ButtonDensity.icon,
                ).withBackgroundColor(
                  color: theme.colorScheme.accent,
                  hoverColor: theme.colorScheme.accent,
                  focusColor: theme.colorScheme.accent,
                ),
            alignment: expanded ? Alignment.centerLeft : Alignment.center,
            label: Text(
              destination.label,
              style: theme.typography.large
                  .merge(theme.typography.medium)
                  .copyWith(height: 1),
            ),
            child: SizedBox.square(
              key: ValueKey<String>(
                'navigation-icon-${destination.destination.name}',
              ),
              dimension: AppIconSize.xl,
              child: Center(
                child: Icon(destination.icon, size: AppIconSize.md),
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
    return SizedBox(
      key: const ValueKey<String>('navigation-brand'),
      height: AppIconSize.xl,
      child: Align(
        alignment: expanded ? Alignment.centerLeft : Alignment.center,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            const Image(
              key: ValueKey<String>('copypaste-brand-logo'),
              image: AssetImage('assets/brand/copypaste.png'),
              width: AppIconSize.xl,
              height: AppIconSize.xl,
            ),
            if (expanded) ...[
              const Gap(AppSpacing.sm),
              Text(
                'CopyPaste',
                style: theme.typography.large
                    .merge(theme.typography.medium)
                    .copyWith(color: theme.colorScheme.foreground, height: 1),
              ),
            ],
          ],
        ),
      ),
    );
  }

  List<Widget> _bottomNavigationItems() {
    final style = AppTheme.mobileNavigationButtonStyle(selected: false);
    final selectedStyle = AppTheme.mobileNavigationButtonStyle(selected: true);
    return [
      for (final destination in appNavigationDestinations) ...[
        if (destination != appNavigationDestinations.first)
          const NavigationGap(AppSpacing.xs),
        Flexible(
          flex: widget.controller.selectedDestination == destination.destination
              ? 1
              : 0,
          child: NavigationItem(
            key: ValueKey<AppDestination>(destination.destination),
            style: style,
            selectedStyle: selectedStyle,
            spacing: AppSpacing.xs,
            overflow: NavigationOverflow.ellipsis,
            label: Text(
              destination.label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              textAlign: TextAlign.center,
            ),
            child: Icon(
              destination.icon,
              size: AppIconSize.md,
              semanticLabel:
                  widget.controller.selectedDestination ==
                      destination.destination
                  ? null
                  : destination.label,
            ),
          ),
        ),
      ],
    ];
  }

  void _selectDestination(Key? key) {
    for (final destination in AppDestination.values) {
      if (key == ValueKey<AppDestination>(destination)) {
        widget.controller.selectDestination(destination);
        return;
      }
    }
  }

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
