import 'dart:async';
import 'dart:math' as math;

import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/devices/device_label.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/history_code_highlighter.dart';
import 'package:copypaste_flutter/features/history/presentation/history_clip_presentation.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/shared/adaptive_breakpoints.dart';
import 'package:copypaste_flutter/shared/state_view.dart';
import 'package:copypaste_flutter/shared/system_date_time.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

/// The shell-owned History destination body. It intentionally does not add an
/// AppBar because AppShell owns the destination title and navigation chrome.
class HistoryScreen extends StatefulWidget {
  const HistoryScreen({
    super.key,
    required this.controller,
    this.onDrawerVisibilityChanged,
  });

  final HistoryController controller;
  final ValueChanged<bool>? onDrawerVisibilityChanged;

  @override
  State<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends State<HistoryScreen> {
  late final TextEditingController _searchController;
  late final ScrollController _scrollController;
  bool _detailDrawerOpen = false;

  @override
  void initState() {
    super.initState();
    _searchController = TextEditingController(
      text: widget.controller.query.search,
    );
    _scrollController = ScrollController()..addListener(_loadMoreWhenNeeded);
    unawaited(widget.controller.initialize());
  }

  @override
  void dispose() {
    _searchController.dispose();
    _scrollController.dispose();
    super.dispose();
  }

  void _loadMoreWhenNeeded() {
    if (!_scrollController.hasClients) return;
    final position = _scrollController.position;
    if (position.pixels >= position.maxScrollExtent - 360) {
      unawaited(widget.controller.loadMore());
    }
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, child) {
        return LayoutBuilder(
          builder: (context, constraints) {
            final wide = constraints.maxWidth >= AdaptiveBreakpoints.inspector;
            final body = _bodyFor(context, wide: wide);
            return Padding(
              padding: const EdgeInsets.all(AppSpacing.lg),
              child: body,
            );
          },
        );
      },
    );
  }

  Widget _bodyFor(BuildContext context, {required bool wide}) {
    final state = widget.controller.state;
    if (state == HistoryLoadState.initial) {
      return const StateView.loading(message: 'Loading clipboard history.');
    }
    if (state == HistoryLoadState.error) {
      return StateView.error(
        title: 'History is unavailable',
        message: widget.controller.errorMessage,
        actionLabel: 'Try again',
        onAction: () => unawaited(widget.controller.reload()),
      );
    }

    final list = _HistoryList(
      controller: widget.controller,
      scrollController: _scrollController,
      searchController: _searchController,
      showKindLabel: wide,
      onSelected: (clip) => _select(context, clip, wide: wide),
    );
    if (!wide) return list;
    if (widget.controller.selectedClip == null) {
      return SizedBox.expand(
        key: const ValueKey<String>('history-clip-list'),
        child: list,
      );
    }
    return ResizablePanel.horizontal(
      dividerBuilder: (context) => null,
      draggerThickness: 12,
      children: [
        ResizablePane.flex(
          minSize: 440,
          initialFlex: 1.45,
          child: Padding(
            padding: const EdgeInsets.only(right: AppSpacing.lg),
            child: SizedBox.expand(
              key: const ValueKey<String>('history-clip-list'),
              child: list,
            ),
          ),
        ),
        ResizablePane(
          initialSize: 360,
          minSize: 300,
          maxSize: 560,
          child: _HistoryDetail(controller: widget.controller, inDrawer: false),
        ),
      ],
    );
  }

  Future<void> _select(
    BuildContext context,
    HistoryClip clip, {
    required bool wide,
  }) async {
    await widget.controller.select(clip.id);
    if (!context.mounted || wide || _detailDrawerOpen) return;
    _detailDrawerOpen = true;
    widget.onDrawerVisibilityChanged?.call(true);
    try {
      await WidgetsBinding.instance.endOfFrame;
      if (!context.mounted) return;
      await showOverlay<void>(
        context,
        AppOverlays.bottomDrawerConfiguration,
        builder: (context) => SizedBox(
          key: const ValueKey<String>('history-detail-drawer'),
          width: double.infinity,
          height:
              MediaQuery.sizeOf(context).height *
              AppOverlaySize.drawerHeightFactor,
          child: AnimatedBuilder(
            animation: widget.controller,
            builder: (context, child) =>
                _HistoryDetail(controller: widget.controller, inDrawer: true),
          ),
        ),
      ).future;
    } finally {
      _detailDrawerOpen = false;
      widget.onDrawerVisibilityChanged?.call(false);
    }
  }
}

class _HistoryList extends StatelessWidget {
  const _HistoryList({
    required this.controller,
    required this.scrollController,
    required this.searchController,
    required this.showKindLabel,
    required this.onSelected,
  });

  final HistoryController controller;
  final ScrollController scrollController;
  final TextEditingController searchController;
  final bool showKindLabel;
  final ValueChanged<HistoryClip> onSelected;

  @override
  Widget build(BuildContext context) {
    if (controller.state == HistoryLoadState.empty) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _HistoryToolbar(
            controller: controller,
            searchController: searchController,
          ),
          if (controller.skippedUndecryptable > 0) ...[
            const Gap(AppSpacing.md),
            _SkippedRowsWarning(count: controller.skippedUndecryptable),
          ],
          const Expanded(
            child: StateView.empty(
              title: 'No clips found',
              message: 'Captured clips and matching history appear here.',
            ),
          ),
        ],
      );
    }
    if (controller.state == HistoryLoadState.loading) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _HistoryToolbar(
            controller: controller,
            searchController: searchController,
          ),
          const Expanded(
            child: StateView.loading(message: 'Loading clipboard history.'),
          ),
        ],
      );
    }
    final rows = _historyListRows(
      controller.items,
      sort: controller.query.sort,
      now: DateTime.now(),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _HistoryToolbar(
          controller: controller,
          searchController: searchController,
        ),
        if (controller.skippedUndecryptable > 0) ...[
          const Gap(AppSpacing.md),
          _SkippedRowsWarning(count: controller.skippedUndecryptable),
        ],
        const Gap(AppSpacing.md),
        Expanded(
          child: ListView.builder(
            controller: scrollController,
            itemCount: rows.length + (controller.isLoadingMore ? 1 : 0),
            itemBuilder: (context, index) {
              if (index >= rows.length) {
                return const Padding(
                  padding: EdgeInsets.all(AppSpacing.lg),
                  child: Center(child: CircularProgressIndicator()),
                );
              }
              return switch (rows[index]) {
                _HistorySectionRow(:final section) => _HistorySectionDivider(
                  section: section,
                ),
                _HistoryClipRow(:final clip) => Padding(
                  padding: const EdgeInsets.only(bottom: AppSpacing.sm),
                  child: _HistoryClipCard(
                    clip: clip,
                    controller: controller,
                    selected: controller.selectedId == clip.id,
                    showKindLabel: showKindLabel,
                    onPressed: () => onSelected(clip),
                  ),
                ),
              };
            },
          ),
        ),
        if (controller.errorMessage != null &&
            controller.state == HistoryLoadState.ready)
          Padding(
            padding: const EdgeInsets.only(top: AppSpacing.sm),
            child: Alert.destructive(
              leading: const Icon(LucideIcons.circleAlert),
              title: const Text('History needs attention'),
              content: Text(controller.errorMessage!),
            ),
          ),
      ],
    );
  }
}

sealed class _HistoryListRow {
  const _HistoryListRow();
}

class _HistorySectionRow extends _HistoryListRow {
  const _HistorySectionRow(this.section);

  final _HistorySection section;
}

class _HistoryClipRow extends _HistoryListRow {
  const _HistoryClipRow(this.clip);

  final HistoryClip clip;
}

enum _HistorySectionKind { pinned, today, yesterday, date }

class _HistorySection {
  const _HistorySection(this.kind, {this.date});

  final _HistorySectionKind kind;
  final DateTime? date;

  String get key => switch (kind) {
    _HistorySectionKind.pinned => 'pinned',
    _HistorySectionKind.today => 'today',
    _HistorySectionKind.yesterday => 'yesterday',
    _HistorySectionKind.date =>
      'date-${date!.toIso8601String().split('T').first}',
  };

  String label(BuildContext context) => switch (kind) {
    _HistorySectionKind.pinned => 'Pinned',
    _HistorySectionKind.today => 'Today',
    _HistorySectionKind.yesterday => 'Yesterday',
    _HistorySectionKind.date => formatSystemDate(context, date!),
  };
}

class _HistorySectionDivider extends StatelessWidget {
  const _HistorySectionDivider({required this.section});

  final _HistorySection section;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.sm),
      child: Semantics(
        header: true,
        child: Divider(
          key: ValueKey<String>('history-section-${section.key}'),
          child: Text(section.label(context)),
        ),
      ),
    );
  }
}

List<_HistoryListRow> _historyListRows(
  List<HistoryClip> items, {
  required HistorySort sort,
  required DateTime now,
}) {
  final rows = <_HistoryListRow>[];
  final pinned = items.where((item) => item.pinned);
  if (pinned.isNotEmpty) {
    rows.add(
      const _HistorySectionRow(_HistorySection(_HistorySectionKind.pinned)),
    );
    rows.addAll(pinned.map(_HistoryClipRow.new));
  }

  final unpinned = items.where((item) => !item.pinned).toList();
  if (sort == HistorySort.relevance) {
    rows.addAll(unpinned.map(_HistoryClipRow.new));
    return rows;
  }

  unpinned.sort((left, right) {
    final byCreatedAt = left.createdAt.compareTo(right.createdAt);
    final byId = left.id.compareTo(right.id);
    return sort == HistorySort.newest
        ? (byCreatedAt != 0 ? -byCreatedAt : -byId)
        : (byCreatedAt != 0 ? byCreatedAt : byId);
  });

  final today = _calendarDate(now);
  final yesterday = DateTime(today.year, today.month, today.day - 1);
  String? previousSectionKey;
  for (final clip in unpinned) {
    final date = _calendarDate(clip.createdAt);
    final section = _dateSection(date, today: today, yesterday: yesterday);
    if (section.key != previousSectionKey) {
      rows.add(_HistorySectionRow(section));
      previousSectionKey = section.key;
    }
    rows.add(_HistoryClipRow(clip));
  }
  return rows;
}

DateTime _calendarDate(DateTime value) {
  final local = value.toLocal();
  return DateTime(local.year, local.month, local.day);
}

_HistorySection _dateSection(
  DateTime date, {
  required DateTime today,
  required DateTime yesterday,
}) {
  if (date == today) {
    return const _HistorySection(_HistorySectionKind.today);
  }
  if (date == yesterday) {
    return const _HistorySection(_HistorySectionKind.yesterday);
  }
  return _HistorySection(_HistorySectionKind.date, date: date);
}

class _SkippedRowsWarning extends StatelessWidget {
  const _SkippedRowsWarning({required this.count});

  final int count;

  @override
  Widget build(BuildContext context) {
    final noun = count == 1 ? 'row was' : 'rows were';
    return Alert(
      leading: const Icon(LucideIcons.triangleAlert),
      title: Text('$count clipboard $noun skipped'),
      content: const Text(
        'Some retained clips could not be read and are not shown in this list.',
      ),
    );
  }
}

class _HistoryToolbar extends StatefulWidget {
  const _HistoryToolbar({
    required this.controller,
    required this.searchController,
  });

  final HistoryController controller;
  final TextEditingController searchController;

  @override
  State<_HistoryToolbar> createState() => _HistoryToolbarState();
}

class _HistoryToolbarState extends State<_HistoryToolbar> {
  static const double _compactControlExtent = 40;
  static const double _minimumExpandedSearchWidth = 160;
  static const double _toolbarGap = AppSpacing.sm;
  static const int _filterCount = 5;
  bool _searchExpanded = false;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final compactFilters =
            constraints.maxWidth < _expandedToolbarMinimumWidth(context);
        final collapseSearch =
            constraints.maxWidth <
            _minimumExpandedSearchWidth +
                (_compactControlExtent * _filterCount) +
                (_toolbarGap * _filterCount);
        if (collapseSearch && _searchExpanded) {
          return SizedBox(
            width: constraints.maxWidth,
            child: _searchField(compact: true),
          );
        }

        final filters = _filterSelects(context, compact: compactFilters);
        if (collapseSearch) {
          return Row(
            spacing: _toolbarGap,
            children: [
              Semantics(
                label: 'Search history',
                button: true,
                child: Button.secondary(
                  key: const ValueKey<String>('history-search-toggle'),
                  style: const ButtonStyle.secondaryIcon(
                    density: ButtonDensity.iconComfortable,
                  ).copyWith(decoration: AppTheme.softSelectDecoration),
                  onPressed: () => setState(() => _searchExpanded = true),
                  child: const Icon(LucideIcons.search),
                ),
              ),
              ...filters,
            ],
          );
        }

        return Row(
          spacing: _toolbarGap,
          children: [
            Expanded(child: _searchField(compact: false)),
            ...filters,
          ],
        );
      },
    );
  }

  Widget _searchField({required bool compact}) {
    return TextField(
      controller: widget.searchController,
      autofocus: compact,
      placeholder: const Text('Search history'),
      onChanged: widget.controller.updateSearch,
      features: compact
          ? [
              const InputFeature.leading(Icon(LucideIcons.search)),
              InputFeature.trailing(
                Button.ghost(
                  style: const ButtonStyle.ghostIcon(),
                  onPressed: _closeCompactSearch,
                  child: const Icon(LucideIcons.x),
                ),
              ),
            ]
          : const [
              InputFeature.leading(Icon(LucideIcons.search)),
              InputFeature.clear(icon: Icon(LucideIcons.x)),
            ],
    );
  }

  void _closeCompactSearch() {
    widget.searchController.clear();
    widget.controller.updateSearch('');
    setState(() => _searchExpanded = false);
  }

  List<Widget> _filterSelects(BuildContext context, {required bool compact}) {
    final query = widget.controller.query;
    final kindOptions = <_HistoryFilterOption<HistoryClipKind?>>[
      const _HistoryFilterOption(
        id: 'all',
        label: 'All clips',
        value: null,
        icon: LucideIcons.listFilter,
      ),
      for (final kind in HistoryClipKind.values)
        _HistoryFilterOption(
          id: kind.name,
          label: kind.label,
          value: kind,
          icon: HistoryClipPresentation.icon(kind),
        ),
    ];
    final pinnedOptions = <_HistoryFilterOption<bool>>[
      const _HistoryFilterOption(
        id: 'all',
        label: 'All pins',
        value: false,
        icon: LucideIcons.pinOff,
      ),
      const _HistoryFilterOption(
        id: 'pinned',
        label: 'Pinned only',
        value: true,
        icon: LucideIcons.pin,
      ),
    ];
    final originOptions = <_HistoryFilterOption<String?>>[
      const _HistoryFilterOption(
        id: 'all',
        label: 'All devices',
        value: null,
        icon: LucideIcons.monitorSmartphone,
      ),
      for (final facet in widget.controller.facets.originDevices)
        _HistoryFilterOption(
          id: facet.id,
          label: facet.label,
          value: facet.id,
          contentBuilder: (compact) => Semantics(
            label: facet.label,
            child: DeviceLabel(
              name: facet.label,
              deviceClass: facet.deviceClass,
              iconSize: compact ? AppIconSize.md : AppIconSize.sm,
              showName: !compact,
            ),
          ),
        ),
    ];
    final sourceOptions = <_HistoryFilterOption<String?>>[
      const _HistoryFilterOption(
        id: 'all',
        label: 'All apps',
        value: null,
        icon: LucideIcons.appWindow,
      ),
      for (final facet in widget.controller.facets.sourceApps)
        _HistoryFilterOption(
          id: facet.id,
          label: facet.label,
          value: facet.id,
          contentBuilder: (compact) => Semantics(
            label: facet.label,
            child: SourceAppLabel(
              name: facet.label,
              icon: facet.iconItemId == null
                  ? Future<HistorySourceAppIcon?>.value(null)
                  : widget.controller.requestSourceIcon(facet.iconItemId!),
              iconSize: compact ? AppIconSize.md : AppIconSize.sm,
              showName: !compact,
            ),
          ),
        ),
    ];
    final sortOptions = <_HistoryFilterOption<HistorySort>>[
      for (final sort in HistorySort.values)
        if (sort != HistorySort.relevance || query.hasSearch)
          _HistoryFilterOption(
            id: sort.name,
            label: sort.label,
            value: sort,
            icon: LucideIcons.arrowDownUp,
          ),
    ];

    return [
      Select<_HistoryFilterOption<HistoryClipKind?>>(
        filled: true,
        constraints: _selectConstraints(compact),
        value: _selectedOption(kindOptions, query.kind),
        onChanged: (option) {
          if (option == null) return;
          final current = widget.controller.query;
          unawaited(
            widget.controller.updateQuery(
              option.value == null
                  ? current.copyWith(clearKind: true)
                  : current.copyWith(kind: option.value),
            ),
          );
        },
        expandIcon: compact ? null : const SelectExpandIcon(),
        theme: _selectThemeFor(context, kindOptions, compact),
        popup: _selectPopup(kindOptions).call,
        itemBuilder: (context, option) => _selectValue(option, compact),
      ),
      Select<_HistoryFilterOption<bool>>(
        filled: true,
        constraints: _selectConstraints(compact),
        value: _selectedOption(pinnedOptions, query.pinnedOnly),
        onChanged: (option) {
          if (option == null) return;
          unawaited(
            widget.controller.updateQuery(
              widget.controller.query.copyWith(pinnedOnly: option.value),
            ),
          );
        },
        expandIcon: compact ? null : const SelectExpandIcon(),
        theme: _selectThemeFor(context, pinnedOptions, compact),
        popup: _selectPopup(pinnedOptions).call,
        itemBuilder: (context, option) => _selectValue(option, compact),
      ),
      Select<_HistoryFilterOption<String?>>(
        filled: true,
        constraints: _selectConstraints(compact),
        value: _selectedOption(originOptions, query.origin),
        onChanged: (option) {
          if (option == null) return;
          final current = widget.controller.query;
          unawaited(
            widget.controller.updateQuery(
              option.value == null
                  ? current.copyWith(clearOrigin: true)
                  : current.copyWith(origin: option.value),
            ),
          );
        },
        expandIcon: compact ? null : const SelectExpandIcon(),
        theme: _selectThemeFor(context, originOptions, compact),
        popup: _selectPopup(originOptions).call,
        itemBuilder: (context, option) => _selectValue(option, compact),
      ),
      Select<_HistoryFilterOption<String?>>(
        filled: true,
        constraints: _selectConstraints(compact),
        value: _selectedOption(sourceOptions, query.sourceApp),
        onChanged: (option) {
          if (option == null) return;
          final current = widget.controller.query;
          unawaited(
            widget.controller.updateQuery(
              option.value == null
                  ? current.copyWith(clearSourceApp: true)
                  : current.copyWith(sourceApp: option.value),
            ),
          );
        },
        expandIcon: compact ? null : const SelectExpandIcon(),
        theme: _selectThemeFor(context, sourceOptions, compact),
        popup: _selectPopup(sourceOptions).call,
        itemBuilder: (context, option) => _selectValue(option, compact),
      ),
      Select<_HistoryFilterOption<HistorySort>>(
        filled: true,
        constraints: _selectConstraints(compact),
        value: _selectedOption(sortOptions, query.sort),
        onChanged: (option) {
          if (option == null) return;
          unawaited(
            widget.controller.updateQuery(
              widget.controller.query.copyWith(sort: option.value),
            ),
          );
        },
        expandIcon: compact ? null : const SelectExpandIcon(),
        theme: _selectThemeFor(context, sortOptions, compact),
        popup: _selectPopup(sortOptions).call,
        itemBuilder: (context, option) => _selectValue(option, compact),
      ),
    ];
  }

  BoxConstraints? _selectConstraints(bool compact) => compact
      ? const BoxConstraints.tightFor(
          width: _compactControlExtent,
          height: _compactControlExtent,
        )
      : null;

  double _expandedToolbarMinimumWidth(BuildContext context) {
    final query = widget.controller.query;
    final labels = [
      query.kind?.label ?? 'All clips',
      query.pinnedOnly ? 'Pinned only' : 'All pins',
      _facetLabel(
        widget.controller.facets.originDevices,
        query.origin,
        'All devices',
      ),
      _facetLabel(
        widget.controller.facets.sourceApps,
        query.sourceApp,
        'All apps',
      ),
      query.sort.label,
    ];
    final theme = Theme.of(context);
    final style = DefaultTextStyle.of(context).style
        .merge(theme.typography.sans)
        .merge(theme.typography.small)
        .merge(theme.typography.normal);
    final textScaler = MediaQuery.textScalerOf(context);
    var width = 160.0;
    for (final label in labels) {
      final painter = TextPainter(
        text: TextSpan(text: label, style: style),
        maxLines: 1,
        textDirection: Directionality.of(context),
        textScaler: textScaler,
      )..layout();
      width += painter.width + 64;
    }
    return width;
  }

  String _facetLabel(
    List<HistoryFilterFacet> facets,
    String? selectedId,
    String fallback,
  ) {
    for (final facet in facets) {
      if (facet.id == selectedId) return facet.label;
    }
    return fallback;
  }

  SelectTheme _selectThemeFor<T>(
    BuildContext context,
    List<_HistoryFilterOption<T>> options,
    bool compact,
  ) {
    final theme = Theme.of(context);
    final style = DefaultTextStyle.of(context).style
        .merge(theme.typography.sans)
        .merge(theme.typography.small)
        .merge(theme.typography.normal);
    final textScaler = MediaQuery.textScalerOf(context);
    var widestLabel = 0.0;
    for (final option in options) {
      final painter = TextPainter(
        text: TextSpan(text: option.label, style: style),
        maxLines: 1,
        textDirection: Directionality.of(context),
        textScaler: textScaler,
      )..layout();
      if (painter.width > widestLabel) widestLabel = painter.width;
    }
    final popupWidth = widestLabel.ceilToDouble() + 32 + (48 * theme.scaling);
    return SelectTheme(
      adaptiveOverlay: false,
      decoration: AppTheme.softSelectDecoration,
      padding: compact
          ? const EdgeInsets.symmetric(
              horizontal: AppSpacing.xs,
              vertical: AppSpacing.sm,
            )
          : const EdgeInsets.symmetric(
              horizontal: AppSpacing.sm,
              vertical: AppSpacing.sm,
            ),
      popupConstraints: BoxConstraints(
        minWidth: popupWidth,
        maxWidth: popupWidth,
        maxHeight: 240,
      ),
      overlayConfiguration: AppOverlays.selectPopoverConfiguration(context),
    );
  }

  _HistoryFilterOption<T> _selectedOption<T>(
    List<_HistoryFilterOption<T>> options,
    T value,
  ) {
    return options.firstWhere(
      (option) => option.value == value,
      orElse: () => options.first,
    );
  }

  SelectPopup<_HistoryFilterOption<T>> _selectPopup<T>(
    List<_HistoryFilterOption<T>> options,
  ) {
    return SelectPopup<_HistoryFilterOption<T>>(
      items: SelectItemList(
        children: [
          for (final option in options)
            SelectItemButton<_HistoryFilterOption<T>>(
              value: option,
              child: option.build(compact: false),
            ),
        ],
      ),
    );
  }

  Widget _selectValue<T>(_HistoryFilterOption<T> option, bool compact) {
    return option.build(compact: compact);
  }
}

class _HistoryFilterOption<T> {
  const _HistoryFilterOption({
    required this.id,
    required this.label,
    required this.value,
    this.icon,
    this.contentBuilder,
  }) : assert(icon != null || contentBuilder != null);

  final String id;
  final String label;
  final T value;
  final IconData? icon;
  final Widget Function(bool compact)? contentBuilder;

  Widget build({required bool compact}) {
    final builder = contentBuilder;
    if (builder != null) return builder(compact);
    final resolvedIcon = icon!;
    if (compact) {
      return Semantics(
        label: label,
        child: Icon(resolvedIcon, size: AppIconSize.md),
      );
    }
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(resolvedIcon, size: AppIconSize.sm),
        const Gap(AppSpacing.sm),
        Text(label, maxLines: 1),
      ],
    );
  }

  @override
  bool operator ==(Object other) =>
      other is _HistoryFilterOption<T> && other.id == id;

  @override
  int get hashCode => id.hashCode;
}

class _HistoryClipCard extends StatelessWidget {
  const _HistoryClipCard({
    required this.clip,
    required this.controller,
    required this.selected,
    required this.showKindLabel,
    required this.onPressed,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final bool selected;
  final bool showKindLabel;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      selected: selected,
      button: true,
      child: Button(
        key: ValueKey<String>('history-clip-${clip.id}'),
        onPressed: onPressed,
        alignment: Alignment.centerLeft,
        style: selected
            ? const ButtonStyle.secondary()
            : const ButtonStyle.ghost(),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _ClipContent(clip: clip, controller: controller),
            const Gap(AppSpacing.xs),
            Row(
              children: [
                Expanded(
                  child: _ClipMeta(
                    clip: clip,
                    controller: controller,
                    showKindLabel: showKindLabel,
                  ),
                ),
                if (clip.pinned) ...[
                  const Gap(AppSpacing.xs),
                  const Icon(LucideIcons.pin, size: AppIconSize.xs),
                ],
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _ClipContent extends StatelessWidget {
  const _ClipContent({required this.clip, required this.controller});

  final HistoryClip clip;
  final HistoryController controller;

  @override
  Widget build(BuildContext context) {
    final kind = clip.contentKind;
    if (kind == HistoryClipKind.color && clip.colorRgba != null) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _ColorSwatch(rgba: clip.colorRgba!),
          const Gap(AppSpacing.md),
          Expanded(
            child: Text(
              clip.preview,
              style: Theme.of(context).typography.mono,
              maxLines: 4,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      );
    }
    if (kind == HistoryClipKind.code || kind == HistoryClipKind.json) {
      final highlighted = HistoryCodeHighlighter.highlight(
        clip.preview,
        Theme.of(context),
        json: kind == HistoryClipKind.json,
      );
      return Text.rich(
        highlighted.span,
        maxLines: 4,
        overflow: TextOverflow.ellipsis,
      );
    }
    if (kind != HistoryClipKind.image) {
      return Text(
        clip.preview.isEmpty ? clip.contentKind.label : clip.preview,
        style: kind == HistoryClipKind.path
            ? Theme.of(context).typography.mono
            : null,
        maxLines: 4,
        overflow: TextOverflow.ellipsis,
      );
    }
    final textStyle = DefaultTextStyle.of(context).style;
    final linePainter = TextPainter(
      text: TextSpan(text: 'Ag', style: textStyle),
      maxLines: 1,
      textDirection: Directionality.of(context),
      textScaler: MediaQuery.textScalerOf(context),
    )..layout();
    final maxHeight = linePainter.height * 8;
    return FutureBuilder<HistoryImagePreview?>(
      future: controller.requestImagePreview(clip.id),
      builder: (context, snapshot) {
        final preview = snapshot.data;
        return ConstrainedBox(
          constraints: BoxConstraints(maxHeight: maxHeight),
          child: SizedBox(
            width: double.infinity,
            child: preview == null
                ? const Center(child: Icon(LucideIcons.image))
                : ClipRRect(
                    borderRadius: BorderRadius.circular(AppRadius.sm),
                    child: Image.memory(
                      preview.bytes,
                      key: ValueKey<String>('history-card-image-${clip.id}'),
                      fit: BoxFit.contain,
                      alignment: Alignment.centerLeft,
                      errorBuilder: (context, error, stackTrace) =>
                          const Center(child: Icon(LucideIcons.imageOff)),
                    ),
                  ),
          ),
        );
      },
    );
  }
}

class _ClipMeta extends StatelessWidget {
  const _ClipMeta({
    required this.clip,
    required this.controller,
    required this.showKindLabel,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final bool showKindLabel;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final style = DefaultTextStyle.of(context).style
        .merge(theme.typography.xSmall)
        .copyWith(color: theme.colorScheme.mutedForeground);
    final kindIcon = Icon(
      HistoryClipPresentation.icon(clip.contentKind),
      size: AppIconSize.xs,
      color: style.color,
    );
    return Text.rich(
      TextSpan(
        children: [
          WidgetSpan(
            alignment: PlaceholderAlignment.middle,
            child: showKindLabel
                ? kindIcon
                : Semantics(
                    label: '${clip.contentKind.label} clip type',
                    child: kindIcon,
                  ),
          ),
          if (showKindLabel) TextSpan(text: ' ${clip.contentKind.label}'),
          if (clip.sourceApp != null) ...[
            const TextSpan(text: ' • '),
            WidgetSpan(
              alignment: PlaceholderAlignment.middle,
              child: SourceAppLabel(
                name: clip.sourceApp!,
                icon: controller.requestSourceIcon(clip.id),
                style: style,
              ),
            ),
          ],
          TextSpan(text: ' • ${formatSystemDateTime(context, clip.createdAt)}'),
        ],
      ),
      key: ValueKey<String>('history-clip-meta-${clip.id}'),
      style: style,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
  }
}

class _HistoryDetail extends StatelessWidget {
  const _HistoryDetail({required this.controller, required this.inDrawer});

  final HistoryController controller;
  final bool inDrawer;

  @override
  Widget build(BuildContext context) {
    final clip = controller.selectedClip;
    if (clip == null) {
      return const StateView.empty(
        title: 'Select a clip',
        message: 'Its full content and actions appear here.',
      );
    }
    final theme = Theme.of(context);
    final body = (clip.body ?? clip.preview).isEmpty
        ? 'No preview is available.'
        : clip.body ?? clip.preview;
    final highlighted = switch (clip.contentKind) {
      HistoryClipKind.code => HistoryCodeHighlighter.highlight(body, theme),
      HistoryClipKind.json => HistoryCodeHighlighter.highlight(
        body,
        theme,
        json: true,
      ),
      _ => null,
    };
    final content = Card(
      key: ValueKey<String>(
        inDrawer ? 'history-detail-drawer-card' : 'history-detail-inspector',
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.all(AppSpacing.xs),
            child: Row(
              key: const ValueKey<String>('history-detail-heading'),
              children: [
                SizedBox.square(
                  key: const ValueKey<String>('history-detail-kind-icon'),
                  dimension: AppControlSize.large,
                  child: Card(
                    theme: CardTheme(
                      padding: EdgeInsets.zero,
                      filled: true,
                      fillColor: theme.colorScheme.secondary,
                      borderRadius: theme.borderRadiusMd,
                      borderWidth: AppSpacing.zero,
                    ),
                    child: Center(
                      child: Icon(
                        HistoryClipPresentation.icon(clip.contentKind),
                        size: AppIconSize.sm,
                      ),
                    ),
                  ),
                ),
                const Gap(AppSpacing.md),
                Expanded(child: Text(clip.contentKind.label).h4()),
                if (highlighted != null) ...[
                  const Gap(AppSpacing.sm),
                  SecondaryBadge(child: Text(highlighted.language)),
                ],
                if (!inDrawer) ...[
                  const Gap(AppSpacing.sm),
                  Tooltip(
                    showDuration: AppMotion.resolve(
                      context,
                      AppMotion.standard,
                    ),
                    tooltip: (context) => const TooltipContainer(
                      child: Text('Close clip details'),
                    ),
                    child: Semantics(
                      label: 'Close clip details',
                      button: true,
                      child: Button.ghost(
                        key: const ValueKey<String>(
                          'history-detail-inspector-close',
                        ),
                        style: const ButtonStyle.ghostIcon(),
                        onPressed: controller.clearSelection,
                        child: const Icon(LucideIcons.x),
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
          const Gap(AppSpacing.lg),
          Flexible(
            fit: FlexFit.loose,
            child: _HistoryDetailContent(
              clip: clip,
              controller: controller,
              body: body,
              highlighted: highlighted,
              desktop: !inDrawer,
            ),
          ),
          const Gap(AppSpacing.lg),
          Padding(
            padding: const EdgeInsets.fromLTRB(
              AppSpacing.xs,
              AppSpacing.zero,
              AppSpacing.xs,
              AppSpacing.xs,
            ),
            child: Wrap(
              key: const ValueKey<String>('history-detail-actions'),
              spacing: AppSpacing.sm,
              runSpacing: AppSpacing.sm,
              children: [
                Button.primary(
                  onPressed: () => _copy(context, plainText: false),
                  leading: const Icon(LucideIcons.copy),
                  child: const Text('Copy'),
                ),
                if (controller.canDownloadSelected)
                  Button.secondary(
                    onPressed: controller.isDownloadPending(clip.id)
                        ? null
                        : () => _download(context),
                    leading: const Icon(LucideIcons.download),
                    child: const Text('Download'),
                  ),
                if (clip.contentKind.isTextual)
                  Button.secondary(
                    onPressed: () => _copy(context, plainText: true),
                    leading: const Icon(LucideIcons.alignLeft),
                    child: const Text('Copy plain text'),
                  ),
                Semantics(
                  toggled: clip.pinned,
                  child: Button(
                    key: ValueKey<String>('history-pin-${clip.id}'),
                    style: clip.pinned
                        ? const ButtonStyle.secondary()
                        : const ButtonStyle.outline(),
                    onPressed: controller.isPinPending(clip.id)
                        ? null
                        : () => controller.togglePin(clip),
                    leading: const Icon(LucideIcons.pin),
                    child: Text(clip.pinned ? 'Pinned' : 'Pin'),
                  ),
                ),
                Button.destructive(
                  onPressed: () => _confirmDelete(context, clip.id),
                  leading: const Icon(LucideIcons.trash2),
                  child: const Text('Delete'),
                ),
              ],
            ),
          ),
          if (!inDrawer) ...[
            const Gap(AppSpacing.md),
            Padding(
              padding: const EdgeInsets.fromLTRB(
                AppSpacing.xs,
                AppSpacing.zero,
                AppSpacing.xs,
                AppSpacing.xs,
              ),
              child: _HistoryMetadataTable(clip: clip, controller: controller),
            ),
          ],
        ],
      ),
    );
    if (!inDrawer) {
      return content;
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.lg,
        AppSpacing.zero,
        AppSpacing.lg,
        AppSpacing.lg,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Align(
            alignment: Alignment.centerRight,
            child: Button.ghost(
              key: const ValueKey<String>('history-detail-drawer-close'),
              style: const ButtonStyle.ghostIcon(),
              onPressed: () => closeDrawer(context),
              child: const Icon(LucideIcons.x),
            ),
          ),
          const Gap(AppSpacing.xs),
          Expanded(child: content),
        ],
      ),
    );
  }

  Future<void> _copy(BuildContext context, {required bool plainText}) async {
    final copied = await controller.copySelected(plainText: plainText);
    if (!copied || !context.mounted) return;
    showToast(
      context: context,
      builder: (context, overlay) => Alert(
        leading: const Icon(LucideIcons.check),
        title: const Text('Copied'),
        content: Text(plainText ? 'Plain text copied.' : 'Clip copied.'),
      ),
    );
  }

  Future<void> _download(BuildContext context) async {
    final result = await controller.downloadSelected();
    if (result != HistoryFileDownloadResult.saved || !context.mounted) return;
    showToast(
      context: context,
      builder: (context, overlay) => const Alert(
        leading: Icon(LucideIcons.check),
        title: Text('Downloaded'),
        content: Text('File saved.'),
      ),
    );
  }

  Future<void> _confirmDelete(BuildContext context, String clipId) {
    return AppOverlays.showDialog<void>(
      context,
      builder: (context) => AnimatedBuilder(
        animation: controller,
        builder: (context, child) => AppOverlays.alertDialog(
          icon: LucideIcons.trash2,
          title: const Text('Delete this clip?'),
          actions: [
            Button.ghost(
              onPressed: controller.isDeletePending(clipId)
                  ? null
                  : () => Navigator.pop(context),
              child: const Text('Cancel'),
            ),
            Button.destructive(
              onPressed: controller.isDeletePending(clipId)
                  ? null
                  : () async {
                      final deleted = await controller.deleteSelected();
                      if (deleted && context.mounted) Navigator.pop(context);
                    },
              child: const Text('Delete'),
            ),
          ],
        ),
      ),
    );
  }
}

class _HistoryDetailContent extends StatelessWidget {
  const _HistoryDetailContent({
    required this.clip,
    required this.controller,
    required this.body,
    required this.highlighted,
    required this.desktop,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final String body;
  final HighlightedHistoryCode? highlighted;
  final bool desktop;

  @override
  Widget build(BuildContext context) {
    if (!desktop) {
      if (clip.contentKind == HistoryClipKind.image) {
        return Padding(
          key: const ValueKey<String>('history-detail-image-fit-content'),
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Expanded(
                child: _DetailImagePreview(clip: clip, controller: controller),
              ),
              const Gap(AppSpacing.lg),
              ..._mobileMetadata(context),
            ],
          ),
        );
      }
      return SingleChildScrollView(
        key: const ValueKey<String>('history-detail-scroll-content'),
        padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _body(context),
            const Gap(AppSpacing.lg),
            ..._mobileMetadata(context),
          ],
        ),
      );
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final content = clip.contentKind == HistoryClipKind.image
            ? _DetailImagePreview(
                clip: clip,
                controller: controller,
                maxHeight: constraints.maxHeight,
              )
            : SizedBox(width: double.infinity, child: _body(context));
        return SingleChildScrollView(
          key: const ValueKey<String>('history-detail-scroll-content'),
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
          child: content,
        );
      },
    );
  }

  List<Widget> _mobileMetadata(BuildContext context) {
    return [
      for (final row in _historyMetadataRows(context, clip)) ...[
        if (_historyMetadataHasIdentity(row))
          Row(
            children: [
              Text('${row.label}:').muted().textSmall(),
              const Gap(AppSpacing.xs),
              Expanded(
                child: _historyMetadataIdentityLabel(
                  row: row,
                  clip: clip,
                  controller: controller,
                  style: Theme.of(context).typography.textSmall.copyWith(
                    color: Theme.of(context).colorScheme.mutedForeground,
                  ),
                ),
              ),
            ],
          )
        else
          Text(
            '${row.label}: ${row.value}',
            style: Theme.of(context).typography.textSmall.copyWith(
              color: row.warning
                  ? Theme.of(context).colorScheme.destructive
                  : Theme.of(context).colorScheme.mutedForeground,
            ),
          ),
        const Gap(AppSpacing.xs),
      ],
    ];
  }

  Widget _body(BuildContext context) {
    if (clip.contentKind == HistoryClipKind.color && clip.colorRgba != null) {
      return Row(
        children: [
          _ColorSwatch(rgba: clip.colorRgba!, size: 48),
          const Gap(AppSpacing.md),
          Expanded(
            child: SelectableText(
              body,
              style: Theme.of(context).typography.mono,
            ),
          ),
        ],
      );
    }
    if (highlighted != null) {
      return SelectableText.rich(highlighted!.span);
    }
    if (clip.contentKind != HistoryClipKind.image) {
      return SelectableText(
        body,
        style:
            clip.contentKind == HistoryClipKind.path ||
                clip.contentKind == HistoryClipKind.file
            ? Theme.of(context).typography.mono
            : null,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _DetailImagePreview(clip: clip, controller: controller),
        const Gap(AppSpacing.lg),
        SelectableText(body),
      ],
    );
  }
}

class _HistoryMetadataTable extends StatelessWidget {
  const _HistoryMetadataTable({required this.clip, required this.controller});

  final HistoryClip clip;
  final HistoryController controller;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final rows = _historyMetadataRows(context, clip);
    return Table(
      key: const ValueKey<String>('history-detail-metadata'),
      columnWidths: const {0: IntrinsicTableSize(), 1: FlexTableSize()},
      rows: [
        for (final row in rows)
          TableRow(
            cells: [
              TableCell(
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: AppSpacing.sm,
                    vertical: AppSpacing.sm,
                  ),
                  child: Text(
                    row.label,
                    style: theme.typography.xSmall
                        .merge(theme.typography.medium)
                        .copyWith(color: theme.colorScheme.mutedForeground),
                  ),
                ),
              ),
              TableCell(
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: AppSpacing.sm,
                    vertical: AppSpacing.sm,
                  ),
                  child: _historyMetadataHasIdentity(row)
                      ? _historyMetadataIdentityLabel(
                          row: row,
                          clip: clip,
                          controller: controller,
                          style: theme.typography.xSmall,
                        )
                      : SelectableText(
                          row.value,
                          style: theme.typography.xSmall.copyWith(
                            color: row.warning
                                ? theme.colorScheme.destructive
                                : null,
                          ),
                        ),
                ),
              ),
            ],
          ),
      ],
    );
  }
}

typedef _HistoryMetadataRow = ({String label, String value, bool warning});

bool _historyMetadataHasIdentity(_HistoryMetadataRow row) =>
    row.label == 'Source' || row.label == 'Device';

Widget _historyMetadataIdentityLabel({
  required _HistoryMetadataRow row,
  required HistoryClip clip,
  required HistoryController controller,
  required TextStyle style,
}) {
  return switch (row.label) {
    'Source' => SourceAppLabel(
      key: const ValueKey<String>('history-detail-source-app'),
      name: row.value,
      icon: controller.requestSourceIcon(clip.id),
      style: style,
    ),
    'Device' => DeviceLabel(
      key: const ValueKey<String>('history-detail-device'),
      name: row.value,
      deviceClass: clip.originDeviceClass,
      style: style,
    ),
    _ => throw StateError('Metadata row has no identity presentation.'),
  };
}

List<_HistoryMetadataRow> _historyMetadataRows(
  BuildContext context,
  HistoryClip clip,
) {
  final type = clip.file?.mimeType ?? clip.contentType;
  final sizeBytes = clip.image?.sizeBytes ?? clip.file?.sizeBytes;
  return [
    (
      label: 'Captured',
      value: formatSystemDateTime(context, clip.createdAt),
      warning: false,
    ),
    if (clip.sourceApp != null)
      (label: 'Source', value: clip.sourceApp!, warning: false),
    if (clip.origin != null)
      (label: 'Device', value: clip.origin!, warning: false),
    (label: 'Clip type', value: clip.contentKind.label, warning: false),
    (label: 'Type', value: type, warning: false),
    if (clip.file?.sourceReference != null)
      (
        label: clip.file!.sourceReference!.startsWith('content://')
            ? 'URI'
            : 'Path',
        value: clip.file!.sourceReference!,
        warning: false,
      ),
    if (clip.file?.name != null)
      (label: 'File', value: clip.file!.name!, warning: false),
    if (clip.image != null)
      (
        label: 'Resolution',
        value: '${clip.image!.width} × ${clip.image!.height}',
        warning: false,
      ),
    if (sizeBytes != null)
      (label: 'Size', value: _formatBytes(sizeBytes), warning: false),
    if ((clip.file?.fileCount ?? 0) > 1)
      (label: 'Files', value: '${clip.file!.fileCount}', warning: false),
    if (clip.tooLargeToSync)
      (label: 'Sync', value: 'Too large to sync', warning: true),
  ];
}

class _DetailImagePreview extends StatelessWidget {
  const _DetailImagePreview({
    required this.clip,
    required this.controller,
    this.maxHeight,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final double? maxHeight;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) => FutureBuilder<HistoryImagePreview?>(
        future: controller.requestImagePreview(clip.id, maxEdge: 1024),
        builder: (context, snapshot) {
          final preview = snapshot.data;
          if (preview == null) {
            return const Align(
              alignment: Alignment.topLeft,
              child: SizedBox.square(
                dimension: AppIconSize.hero,
                child: Center(
                  child: Icon(LucideIcons.image, size: AppIconSize.hero),
                ),
              ),
            );
          }
          if (preview.width <= 0 || preview.height <= 0) {
            return const Align(
              alignment: Alignment.topLeft,
              child: Icon(LucideIcons.imageOff, size: AppIconSize.hero),
            );
          }
          final sourceWidth = preview.width.toDouble();
          final sourceHeight = preview.height.toDouble();
          final availableWidth = constraints.hasBoundedWidth
              ? constraints.maxWidth
              : sourceWidth;
          final availableHeight =
              maxHeight ??
              (constraints.hasBoundedHeight
                  ? constraints.maxHeight
                  : sourceHeight);
          final scale = math.min(
            1.0,
            math.min(
              availableWidth / sourceWidth,
              availableHeight / sourceHeight,
            ),
          );
          return Align(
            alignment: Alignment.topLeft,
            child: SizedBox(
              key: const ValueKey<String>('history-detail-image-viewport'),
              width: sourceWidth * scale,
              height: sourceHeight * scale,
              child: Image.memory(
                preview.bytes,
                key: ValueKey<String>('history-detail-image-${clip.id}'),
                fit: BoxFit.contain,
                alignment: Alignment.topLeft,
                errorBuilder: (context, error, stackTrace) => const Center(
                  child: Icon(LucideIcons.imageOff, size: AppIconSize.hero),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

class _ColorSwatch extends StatelessWidget {
  const _ColorSwatch({required this.rgba, this.size = 32});

  final int rgba;
  final double size;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      width: size,
      height: size,
      padding: const EdgeInsets.all(AppSpacing.xxs),
      decoration: BoxDecoration(
        color: scheme.muted,
        border: Border.all(color: scheme.border),
        borderRadius: BorderRadius.circular(AppRadius.sm),
      ),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: Color.fromARGB(
            rgba & 0xff,
            (rgba >> 24) & 0xff,
            (rgba >> 16) & 0xff,
            (rgba >> 8) & 0xff,
          ),
          borderRadius: BorderRadius.circular(AppRadius.xs),
        ),
      ),
    );
  }
}

String _formatBytes(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}
