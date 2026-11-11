import 'dart:ui' show SemanticsRole;

import 'package:flutter/foundation.dart' show listEquals;
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:window_manager/window_manager.dart';

import '../../l10n/l10n.dart';
import '../../platform/desktop/desktop_shortcuts.dart';
import '../navigation/app_destination.dart';
import '../theme/app_brand_logo.dart';
import '../theme/app_motion.dart';
import '../theme/app_theme.dart';
import '../theme/app_tokens.dart';
import 'header_fit.dart';
import 'macos_window_header.dart';

/// The application header: brand, destinations, then on the right the
/// selected destination's commands, the grouped status cells and the [end]
/// controls. One row serves every [HeaderStage], so controls keep their state
/// while the window is resized.
class AppHeader extends StatelessWidget {
  const AppHeader({
    super.key,
    required this.unifiedTitleBar,
    required this.selected,
    required this.onSelected,
    this.actions = const [],
    this.status = const [],
    this.end = const [],
  });

  /// Whether the header shares the macOS title bar with the window buttons.
  final bool unifiedTitleBar;
  final AppDestination selected;
  final ValueChanged<AppDestination> onSelected;
  final List<Widget> actions;
  final List<Widget> status;

  /// Controls at the far end, after the status group, in every stage.
  final List<Widget> end;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final titleStyle = AppTheme.navigationTextStyle(context);
    // Free header space drags a unified window, so header navigation always
    // leaves some; with bottom navigation the title takes that room.
    final lead = Align(
      alignment: AlignmentDirectional.centerStart,
      child: Row(
        children: [
          if (unifiedTitleBar)
            const HeaderCollapsible(
              until: HeaderStage.bottomNavigation,
              child: SizedBox(width: AppSpacing.xxxl),
            ),
          Flexible(
            child: HeaderCollapsible(
              from: HeaderStage.bottomNavigation,
              child: Text(
                selected.navigationDestination.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: titleStyle,
              ),
            ),
          ),
        ],
      ),
    );
    final gap = unifiedTitleBar
        ? const <Widget>[SizedBox(width: AppSpacing.sm)]
        : const <Widget>[
            HeaderCollapsible(
              until: HeaderStage.bottomNavigation,
              child: SizedBox(width: AppSpacing.sm),
            ),
            HeaderCollapsible(
              from: HeaderStage.bottomNavigation,
              child: SizedBox(width: AppSpacing.xxs),
            ),
          ];
    return AppBar(
      key: unifiedTitleBar
          ? const ValueKey<String>('macos-window-header')
          : null,
      backgroundColor: unifiedTitleBar ? theme.colorScheme.muted : null,
      height: AppTheme.headerHeight(context),
      padding: EdgeInsets.zero,
      useSafeArea: false,
      child: DecoratedBox(
        decoration: AppTheme.headerDecoration(context),
        child: Row(
          children: [
            if (unifiedTitleBar)
              const SizedBox(
                key: ValueKey<String>('macos-native-controls-reserve'),
                width: MacosWindowHeader.nativeControlsWidth,
              )
            else ...[
              const SizedBox(width: AppSpacing.lg),
              HeaderCollapsible(
                from: HeaderStage.bottomNavigation,
                child: Padding(
                  padding: const EdgeInsetsDirectional.only(end: AppSpacing.sm),
                  child: Button.ghost(
                    style: AppTheme.brandButtonStyle,
                    onPressed: () => onSelected(AppDestination.history),
                    child: AppBrandLogo(
                      key: const ValueKey<String>('header-brand-logo'),
                      size: AppIconSize.md,
                      semanticLabel: context.l10n.appOpenHistory,
                    ),
                  ),
                ),
              ),
            ],
            HeaderCollapsible(
              until: HeaderStage.bottomNavigation,
              child: Padding(
                padding: const EdgeInsetsDirectional.only(end: AppSpacing.md),
                child: Button.ghost(
                  key: const ValueKey<String>('header-brand'),
                  style: AppTheme.brandButtonStyle,
                  onPressed: () => onSelected(AppDestination.history),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      AppBrandLogo(
                        key: const ValueKey<String>('header-navigation-logo'),
                        size: AppIconSize.md,
                        semanticLabel: context.l10n.appOpenHistory,
                      ),
                      HeaderCollapsible(
                        until: HeaderStage.unbranded,
                        child: Padding(
                          padding: const EdgeInsetsDirectional.only(
                            start: AppSpacing.sm,
                          ),
                          child: ExcludeSemantics(
                            child: Text(
                              'CopyPaste',
                              key: const ValueKey<String>('header-wordmark'),
                              maxLines: 1,
                              style: titleStyle.copyWith(
                                color: theme.colorScheme.foreground,
                              ),
                            ),
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
            HeaderCollapsible(
              until: HeaderStage.bottomNavigation,
              child: _HeaderNavigation(
                selected: selected,
                onSelected: onSelected,
              ),
            ),
            Expanded(
              child: unifiedTitleBar
                  ? DragToMoveArea(
                      key: const ValueKey<String>('macos-window-drag-region'),
                      child: lead,
                    )
                  : lead,
            ),
            for (final action in actions) ...[...gap, action],
            if (status.isNotEmpty) ...[
              ...gap,
              _HeaderStatusGroup(cells: status),
            ],
            for (final control in end) ...[...gap, control],
            const SizedBox(width: AppSpacing.lg),
          ],
        ),
      ),
    );
  }
}

class _HeaderNavigation extends StatelessWidget {
  const _HeaderNavigation({required this.selected, required this.onSelected});

  final AppDestination selected;
  final ValueChanged<AppDestination> onSelected;

  @override
  Widget build(BuildContext context) => Semantics(
    container: true,
    explicitChildNodes: true,
    role: SemanticsRole.navigation,
    label: context.l10n.appNavigationLabel,
    // Items span the header and outline themselves on focus.
    child: Theme(
      data: AppTheme.ownFocusCueTheme(context),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final item in appNavigationDestinations)
            _HeaderNavigationItem(
              item: item,
              selected: item.destination == selected,
              onPressed: () => onSelected(item.destination),
            ),
        ],
      ),
    ),
  );
}

/// Spans the header height so its bottom edge is the header border.
class _HeaderNavigationItem extends StatelessWidget {
  const _HeaderNavigationItem({
    required this.item,
    required this.selected,
    required this.onPressed,
  });

  final AppNavigationDestination item;
  final bool selected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final button = Button.ghost(
      key: ValueKey<String>('header-destination-${item.destination.name}'),
      style: AppTheme.headerNavigationButtonStyle(selected: selected),
      onPressed: onPressed,
      child: Semantics(
        label: item.label,
        button: true,
        selected: selected,
        child: ExcludeSemantics(child: _HeaderNavigationLabel(item.label)),
      ),
    );
    return SizedBox(
      height: double.infinity,
      child: DecoratedBox(
        position: DecorationPosition.foreground,
        decoration: selected
            ? AppTheme.headerNavigationSelectionDecoration
            : const BoxDecoration(),
        child: Center(
          widthFactor: 1,
          child: usesDesktopShortcuts
              ? Tooltip(
                  showDuration: AppMotion.resolve(context, AppMotion.standard),
                  tooltip: (context) => TooltipContainer(
                    child: Text(
                      '${item.label} · '
                      '${desktopShortcutLabel(item.shortcutKey)}',
                    ),
                  ),
                  child: button,
                )
              : button,
        ),
      ),
    );
  }
}

/// Reserves the emphasised width, so selecting a destination never moves its
/// neighbours.
class _HeaderNavigationLabel extends StatelessWidget {
  const _HeaderNavigationLabel(this.label);

  final String label;

  @override
  Widget build(BuildContext context) => Stack(
    alignment: Alignment.center,
    children: [
      Opacity(
        opacity: 0,
        child: RichText(
          maxLines: 1,
          textScaler: MediaQuery.textScalerOf(context),
          text: TextSpan(
            text: label,
            style: DefaultTextStyle.of(context).style.copyWith(
              fontWeight: AppTheme.headerNavigationSelectedWeight,
            ),
          ),
        ),
      ),
      Text(label, maxLines: 1),
    ],
  );
}

/// A status cell the header can leave out, together with its gap and
/// divider: while [present] is false, and from stage [until] on.
abstract interface class OptionalHeaderStatus {
  /// Notifies when [present] may have changed.
  Listenable get presence;
  bool get present;
  HeaderStage? get until;
}

/// The status cells as one object: one surface, cells divided by thin rules,
/// in every stage. Each cell is its own control without a surface of its own.
class _HeaderStatusGroup extends StatefulWidget {
  const _HeaderStatusGroup({required this.cells});

  final List<Widget> cells;

  @override
  State<_HeaderStatusGroup> createState() => _HeaderStatusGroupState();
}

class _HeaderStatusGroupState extends State<_HeaderStatusGroup> {
  List<Listenable> _watched = const [];
  Listenable _presence = Listenable.merge(const []);

  @override
  Widget build(BuildContext context) {
    final cells = widget.cells;
    // A cell the stage leaves out is measured as it is, not watched.
    final stage = HeaderFit.stageOf(context);
    final optional = [
      for (final cell in cells)
        if (cell case final OptionalHeaderStatus optional
            when optional.until == null || stage.index < optional.until!.index)
          optional.presence,
    ];
    // A new merge on every rebuild would drop and re-add each listener, and
    // a readout that loses its last listener stops observing.
    if (!listEquals(optional, _watched)) {
      _watched = optional;
      _presence = Listenable.merge(optional);
    }
    // One element tree in every stage keeps the cells' state.
    return ListenableBuilder(
      listenable: _presence,
      builder: (context, _) => _group(context, [
        for (final cell in cells)
          if (cell is! OptionalHeaderStatus ||
              (cell as OptionalHeaderStatus).present)
            cell,
      ]),
    );
  }

  Widget _group(BuildContext context, List<Widget> cells) {
    final surface = AppTheme.headerStatusGroupDecoration(context);
    // The group clips its rounded corners; cells outline themselves on focus.
    return DecoratedBox(
      key: const ValueKey<String>('header-status-group'),
      decoration: surface,
      child: ClipRRect(
        borderRadius: surface.borderRadius!,
        child: Theme(
          data: AppTheme.ownFocusCueTheme(context),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (final (index, cell) in cells.indexed)
                _part(
                  cell,
                  DecoratedBox(
                    position: DecorationPosition.foreground,
                    decoration: index > 0
                        ? AppTheme.headerStatusDividerDecoration(context)
                        : const BoxDecoration(),
                    child: cell,
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }

  /// A cell the header leaves out from some stage on takes its divider along.
  Widget _part(Widget cell, Widget part) => switch (cell) {
    OptionalHeaderStatus(:final until?) => HeaderCollapsible(
      until: until,
      child: part,
    ),
    _ => part,
  };
}
