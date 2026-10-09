import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';

import '../../../platform/files/history_file_drop_target.dart';

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

import '../presentation/secret_spoiler.dart';

import 'history_inspector.dart';
import 'history_delete_dialog.dart';
import 'history_bulk_toolbar.dart';
import '../presentation/history_color_swatch.dart';

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
  ScrollController? _scrollController;
  ScrollController? _fallbackScrollController;
  bool _detailDrawerOpen = false;
  bool _fileDragHover = false;

  @override
  void initState() {
    super.initState();
    _searchController = TextEditingController(
      text: widget.controller.query.search,
    );
    unawaited(widget.controller.initialize());
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final next =
        PrimaryScrollController.maybeOf(context) ??
        (_fallbackScrollController ??= ScrollController());
    if (identical(next, _scrollController)) return;
    _scrollController?.removeListener(_loadMoreWhenNeeded);
    _scrollController = next;
    next.addListener(_loadMoreWhenNeeded);
  }

  @override
  void dispose() {
    _searchController.dispose();
    _scrollController?.removeListener(_loadMoreWhenNeeded);
    _fallbackScrollController?.dispose();
    super.dispose();
  }

  void _loadMoreWhenNeeded() {
    final controller = _scrollController;
    if (controller == null || !controller.hasClients) return;
    final position = controller.position;
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
            return HistoryFileDropTarget(
              enabled:
                  widget.controller.canImportFiles &&
                  !widget.controller.isImportingFiles &&
                  !widget.controller.isBulkSelecting &&
                  !_detailDrawerOpen,
              onFiles: (files) =>
                  unawaited(widget.controller.importFiles(files)),
              onHover: (hover) {
                if (mounted) setState(() => _fileDragHover = hover);
              },
              child: DecoratedBox(
                decoration: _fileDragHover
                    ? AppTheme.historyFileDropDecoration(context)
                    : const BoxDecoration(),
                child: Padding(
                  padding: const EdgeInsets.all(AppSpacing.lg),
                  child: CallbackShortcuts(
                    bindings: widget.controller.isBulkSelecting
                        ? {
                            const SingleActivator(LogicalKeyboardKey.escape):
                                widget.controller.endBulkSelection,
                          }
                        : const {},
                    child: widget.controller.isBulkSelecting
                        ? Focus(autofocus: true, child: body)
                        : body,
                  ),
                ),
              ),
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
      scrollController: _scrollController!,
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
          child: HistoryInspector(
            controller: widget.controller,
            inDrawer: false,
          ),
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
        builder: (context) => ConstrainedBox(
          key: const ValueKey<String>('history-detail-drawer'),
          constraints: AppOverlays.drawerContentConstraints(context),
          child: AnimatedBuilder(
            animation: widget.controller,
            builder: (context, child) =>
                HistoryInspector(controller: widget.controller, inDrawer: true),
          ),
        ),
      ).future;
    } finally {
      _detailDrawerOpen = false;
      widget.onDrawerVisibilityChanged?.call(false);
    }
  }
}

class _HistoryList extends StatefulWidget {
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
  State<_HistoryList> createState() => _HistoryListState();
}

class _HistoryListState extends State<_HistoryList> {
  final Map<String, SortableData<String>> _sortableData = {};

  HistoryController get controller => widget.controller;
  ScrollController get scrollController => widget.scrollController;
  TextEditingController get searchController => widget.searchController;
  bool get showKindLabel => widget.showKindLabel;
  ValueChanged<HistoryClip> get onSelected => widget.onSelected;

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
      isSectionCollapsed: controller.isSectionCollapsed,
    );
    final rowIndices = <Key, int>{
      for (var index = 0; index < rows.length; index++)
        _rowKey(rows[index]): index,
    };
    final showLoadMore =
        controller.canLoadMore &&
        rows.any(
          (row) =>
              row is _HistorySectionRow &&
              controller.isSectionCollapsed(row.section.key),
        );
    final retainedIds = controller.items.map((item) => item.id).toSet();
    _sortableData.removeWhere((id, _) => !retainedIds.contains(id));
    final bottomPadding = MediaQuery.paddingOf(context).bottom;
    final hasError = controller.errorMessage != null;
    final hasLoadMore = controller.isLoadingMore || showLoadMore;
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
          child: SortableLayer(
            clipBehavior: Clip.hardEdge,
            dropDuration: AppMotion.resolve(context, AppMotion.standard),
            dropCurve: AppMotion.standardCurve,
            child: ScrollableSortableLayer(
              controller: scrollController,
              scrollThreshold: AppControlSize.touch,
              child: ListView.builder(
                controller: scrollController,
                padding: EdgeInsets.only(bottom: bottomPadding),
                findChildIndexCallback: (key) => rowIndices[key],
                itemCount:
                    rows.length + (hasLoadMore ? 1 : 0) + (hasError ? 1 : 0),
                itemBuilder: (context, index) {
                  if (hasError &&
                      index == rows.length + (hasLoadMore ? 1 : 0)) {
                    return Padding(
                      padding: const EdgeInsets.only(top: AppSpacing.sm),
                      child: Alert.destructive(
                        leading: const Icon(LucideIcons.circleAlert),
                        title: const Text('History needs attention'),
                        content: Text(controller.errorMessage!),
                      ),
                    );
                  }
                  if (index >= rows.length) {
                    return Padding(
                      padding: const EdgeInsets.all(AppSpacing.lg),
                      child: Center(
                        child: controller.isLoadingMore
                            ? const CircularProgressIndicator()
                            : Button.ghost(
                                onPressed: () =>
                                    unawaited(controller.loadMore()),
                                child: const Text('Load more'),
                              ),
                      ),
                    );
                  }
                  return switch (rows[index]) {
                    _HistorySectionRow(:final section) =>
                      _HistorySectionDivider(
                        key: _rowKey(rows[index]),
                        section: section,
                        collapsed: controller.isSectionCollapsed(section.key),
                        onToggle: () => controller.toggleSection(section.key),
                      ),
                    _HistoryClipRow(:final clip) => _clipRow(clip),
                  };
                },
              ),
            ),
          ),
        ),
      ],
    );
  }

  Key _rowKey(_HistoryListRow row) => switch (row) {
    _HistorySectionRow(:final section) => ValueKey<String>(
      'history-divider-${section.key}',
    ),
    _HistoryClipRow(:final clip) => ValueKey<String>('history-row-${clip.id}'),
  };

  Widget _clipRow(HistoryClip clip) {
    final child = Padding(
      padding: const EdgeInsets.only(bottom: AppSpacing.sm),
      child: _HistoryClipCard(
        clip: clip,
        controller: controller,
        selected: controller.isBulkSelecting
            ? controller.isBulkSelected(clip.id)
            : controller.selectedId == clip.id,
        showKindLabel: showKindLabel,
        onPressed: () => controller.isBulkSelecting
            ? controller.toggleBulkSelection(clip.id)
            : onSelected(clip),
      ),
    );
    final key = ValueKey<String>('history-row-${clip.id}');
    if (!clip.pinned) return KeyedSubtree(key: key, child: child);
    return Sortable<String>(
      key: key,
      data: _sortableData.putIfAbsent(clip.id, () => SortableData(clip.id)),
      // Only the library-owned drag handle can initiate a gesture.
      enabled: false,
      canAcceptTop: (data) =>
          controller.canReorderPinned && data.data != clip.id,
      canAcceptBottom: (data) =>
          controller.canReorderPinned && data.data != clip.id,
      onAcceptTop: (data) =>
          unawaited(controller.movePinned(data.data, clip.id, before: true)),
      onAcceptBottom: (data) =>
          unawaited(controller.movePinned(data.data, clip.id, before: false)),
      onDragStart: () => controller.beginPinnedDrag(clip.id),
      onDragEnd: controller.endPinnedDrag,
      onDragCancel: controller.endPinnedDrag,
      placeholder: Padding(
        padding: const EdgeInsets.only(bottom: AppSpacing.sm),
        child: DecoratedBox(
          key: const ValueKey<String>('history-pin-drop-placeholder'),
          decoration: AppTheme.historyPinnedDropDecoration(context),
          child: Center(
            child: Icon(
              LucideIcons.moveVertical,
              size: AppIconSize.md,
              color: Theme.of(context).colorScheme.primary,
            ),
          ),
        ),
      ),
      child: child,
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
  const _HistorySectionDivider({
    super.key,
    required this.section,
    required this.collapsed,
    required this.onToggle,
  });

  final _HistorySection section;
  final bool collapsed;
  final VoidCallback onToggle;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.xs),
      child: Semantics(
        header: true,
        button: true,
        expanded: !collapsed,
        child: Button.ghost(
          key: ValueKey<String>('history-section-${section.key}'),
          style: AppTheme.historySectionButtonStyle,
          onPressed: onToggle,
          child: Divider(
            child: Row(
              mainAxisSize: MainAxisSize.min,
              spacing: AppSpacing.xs,
              children: [
                Icon(
                  collapsed
                      ? LucideIcons.chevronRight
                      : LucideIcons.chevronDown,
                  size: AppIconSize.sm,
                ),
                Text(section.label(context)),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

List<_HistoryListRow> _historyListRows(
  List<HistoryClip> items, {
  required HistorySort sort,
  required DateTime now,
  required bool Function(String) isSectionCollapsed,
}) {
  final rows = <_HistoryListRow>[];
  final pinned = items.where((item) => item.pinned);
  if (pinned.isNotEmpty) {
    rows.add(
      const _HistorySectionRow(_HistorySection(_HistorySectionKind.pinned)),
    );
    if (!isSectionCollapsed('pinned')) {
      rows.addAll(pinned.map(_HistoryClipRow.new));
    }
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
    if (!isSectionCollapsed(section.key)) {
      rows.add(_HistoryClipRow(clip));
    }
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
  double _controlExtent(BuildContext context) =>
      AppTheme.controlHeight(context, minimum: AppControlSize.large);
  static const double _minimumExpandedSearchWidth =
      AppLayoutSize.historySearchMinWidth;
  static const double _toolbarGap = AppSpacing.sm;
  static const int _filterCount = 5;
  bool _searchExpanded = false;

  @override
  Widget build(BuildContext context) {
    if (widget.controller.isBulkSelecting) {
      return HistoryBulkToolbar(controller: widget.controller);
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        final compactFilters =
            constraints.maxWidth < _expandedToolbarMinimumWidth(context);
        final collapseSearch =
            constraints.maxWidth <
            _minimumExpandedSearchWidth +
                (_controlExtent(context) * (_filterCount + 2)) +
                (_toolbarGap * (_filterCount + 2));
        if (collapseSearch && _searchExpanded) {
          return SizedBox(
            width: constraints.maxWidth,
            child: Row(
              spacing: _toolbarGap,
              children: [
                Expanded(child: _searchField(compact: true)),
                _selectionButton(),
              ],
            ),
          );
        }

        final filters = _filterSelects(context, compact: compactFilters);
        if (collapseSearch) {
          return Wrap(
            spacing: _toolbarGap,
            runSpacing: _toolbarGap,
            children: [
              Semantics(
                label: 'Search history',
                button: true,
                child: Button.secondary(
                  key: const ValueKey<String>('history-search-toggle'),
                  style: AppTheme.historyToolbarIconStyle,
                  onPressed: () => setState(() => _searchExpanded = true),
                  child: const Icon(LucideIcons.search, size: AppIconSize.md),
                ),
              ),
              ...filters,
              _importButton(),
              _selectionButton(),
            ],
          );
        }

        return Row(
          spacing: _toolbarGap,
          children: [
            Expanded(
              child: ConstrainedBox(
                constraints: const BoxConstraints(
                  minWidth: _minimumExpandedSearchWidth,
                ),
                child: _searchField(compact: false),
              ),
            ),
            ...filters,
            _importButton(),
            _selectionButton(),
          ],
        );
      },
    );
  }

  Widget _importButton() => Semantics(
    label: 'Import files',
    button: true,
    child: Tooltip(
      tooltip: (_) => const TooltipContainer(child: Text('Import files')),
      child: Button.secondary(
        key: const ValueKey<String>('history-import-files'),
        style: AppTheme.historyToolbarIconStyle,
        onPressed:
            widget.controller.canImportFiles &&
                !widget.controller.isImportingFiles
            ? () => unawaited(widget.controller.chooseFiles())
            : null,
        child: widget.controller.isImportingFiles
            ? const CircularProgressIndicator(size: AppIconSize.md)
            : const Icon(LucideIcons.filePlus, size: AppIconSize.md),
      ),
    ),
  );

  Widget _selectionButton() => Semantics(
    label: 'Select clips',
    button: true,
    child: Tooltip(
      tooltip: (_) => const TooltipContainer(child: Text('Select clips')),
      child: Button.secondary(
        key: const ValueKey('history-select-clips'),
        style: AppTheme.historyToolbarIconStyle,
        onPressed: widget.controller.canBeginBulkSelection
            ? () {
                FocusManager.instance.primaryFocus?.unfocus();
                widget.controller.beginBulkSelection();
              }
            : null,
        child: const Icon(LucideIcons.squareCheck, size: AppIconSize.md),
      ),
    ),
  );

  Widget _searchField({required bool compact}) {
    return TextField(
      padding: AppTheme.controlFieldPadding(
        context,
        minimum: AppControlSize.large,
      ),
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
              icon: widget.controller.requestSourceIcon(facet.iconId),
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
      _filterSelect(
        context,
        compact: compact,
        options: kindOptions,
        value: query.kind,
        onChanged: (option) {
          final current = widget.controller.query;
          unawaited(
            widget.controller.updateQuery(
              option.value == null
                  ? current.copyWith(clearKind: true)
                  : current.copyWith(kind: option.value),
            ),
          );
        },
      ),
      _filterSelect(
        context,
        compact: compact,
        options: pinnedOptions,
        value: query.pinnedOnly,
        onChanged: (option) {
          unawaited(
            widget.controller.updateQuery(
              widget.controller.query.copyWith(pinnedOnly: option.value),
            ),
          );
        },
      ),
      _filterSelect(
        context,
        compact: compact,
        options: originOptions,
        value: query.origin,
        onChanged: (option) {
          final current = widget.controller.query;
          unawaited(
            widget.controller.updateQuery(
              option.value == null
                  ? current.copyWith(clearOrigin: true)
                  : current.copyWith(origin: option.value),
            ),
          );
        },
      ),
      _filterSelect(
        context,
        compact: compact,
        options: sourceOptions,
        value: query.sourceApp,
        onChanged: (option) {
          final current = widget.controller.query;
          unawaited(
            widget.controller.updateQuery(
              option.value == null
                  ? current.copyWith(clearSourceApp: true)
                  : current.copyWith(sourceApp: option.value),
            ),
          );
        },
      ),
      _filterSelect(
        context,
        compact: compact,
        options: sortOptions,
        value: query.sort,
        onChanged: (option) {
          unawaited(
            widget.controller.updateQuery(
              widget.controller.query.copyWith(sort: option.value),
            ),
          );
        },
      ),
    ];
  }

  Widget _filterSelect<T>(
    BuildContext context, {
    required bool compact,
    required List<_HistoryFilterOption<T>> options,
    required T value,
    required ValueChanged<_HistoryFilterOption<T>> onChanged,
  }) => ComponentTheme<SelectTheme>(
    data: _selectThemeFor(context, options, compact),
    child: Select<_HistoryFilterOption<T>>(
      filled: true,
      constraints: _selectConstraints(context, compact),
      value: _selectedOption(options, value),
      onChanged: (option) {
        if (option != null) onChanged(option);
      },
      expandIcon: compact ? null : const SelectExpandIcon(),
      popup: _selectPopup(options).call,
      itemBuilder: (context, option) => option.build(compact: compact),
    ),
  );

  BoxConstraints? _selectConstraints(BuildContext context, bool compact) =>
      compact
      ? BoxConstraints.tightFor(
          width: _controlExtent(context),
          height: _controlExtent(context),
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
    final style = DefaultTextStyle.of(
      context,
    ).style.merge(const ButtonStyle.secondary().textStyle(context, const {}));
    final textScaler = MediaQuery.textScalerOf(context);
    final filterChromeWidth =
        (AppSpacing.sm * 3) +
        AppIconSize.sm +
        (AppSpacing.sm * theme.scaling) +
        theme.iconTheme.small.size!;
    var width =
        _minimumExpandedSearchWidth +
        (_controlExtent(context) * 2) +
        (_toolbarGap * (_filterCount + 2));
    for (final label in labels) {
      final painter = TextPainter(
        text: TextSpan(text: label, style: style),
        maxLines: 1,
        textDirection: Directionality.of(context),
        textScaler: textScaler,
      )..layout();
      width += painter.width.ceilToDouble() + filterChromeWidth;
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
      padding: AppTheme.controlFieldPadding(
        context,
        horizontal: compact ? AppSpacing.xs : AppSpacing.sm,
        minimum: AppControlSize.large,
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

class _HistoryClipCard extends StatefulWidget {
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
  State<_HistoryClipCard> createState() => _HistoryClipCardState();
}

class _HistoryClipCardState extends State<_HistoryClipCard> {
  final _focus = FocusNode(skipTraversal: true);
  bool _hovered = false;
  bool _focused = false;

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final clip = widget.clip;
    final controller = widget.controller;
    final dragging = controller.draggedPinnedId == clip.id;
    final touch = Theme.of(context).platform == TargetPlatform.android;
    final showActions = !controller.isBulkSelecting && (_hovered || _focused);
    final showHandle =
        !controller.isBulkSelecting &&
        clip.pinned &&
        controller.hasUnfilteredQuery;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: FocusableActionDetector(
        focusNode: _focus,
        includeFocusSemantics: false,
        onFocusChange: (value) => setState(() => _focused = value),
        child: DecoratedBox(
          decoration: dragging
              ? AppTheme.historyPinnedDragDecoration(context)
              : const BoxDecoration(),
          child: Stack(
            children: [
              Semantics(
                selected: widget.selected,
                button: true,
                child: Button(
                  key: ValueKey<String>('history-clip-${clip.id}'),
                  onPressed: controller.isBulkMutating
                      ? null
                      : widget.onPressed,
                  onLongPressStart: controller.isBulkMutating
                      ? null
                      : (_) => controller.beginBulkSelection(clip.id),
                  leading: controller.isBulkSelecting
                      ? IgnorePointer(
                          child: ExcludeSemantics(
                            child: Checkbox(
                              state: widget.selected
                                  ? CheckboxState.checked
                                  : CheckboxState.unchecked,
                              enabled: !controller.isBulkMutating,
                              onChanged: (_) {},
                            ),
                          ),
                        )
                      : null,
                  alignment: Alignment.centerLeft,
                  style: AppTheme.historyClipButtonStyle(
                    selected: widget.selected,
                  ),
                  child: Padding(
                    padding: EdgeInsets.only(
                      right: touch && showHandle
                          ? AppControlSize.touch + AppSpacing.sm
                          : AppSpacing.zero,
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        _ClipContent(clip: clip, controller: controller),
                        const Gap(AppSpacing.xxs),
                        Row(
                          children: [
                            Expanded(
                              child: _ClipMeta(
                                clip: clip,
                                controller: controller,
                                showKindLabel: widget.showKindLabel,
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              if (showActions || dragging || (touch && showHandle))
                Positioned(
                  top: AppSpacing.zero,
                  bottom: AppSpacing.zero,
                  right: AppSpacing.sm,
                  child: Center(
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      spacing: AppSpacing.xs,
                      children: [
                        if (showActions) ...[
                          Tooltip(
                            tooltip: (context) => TooltipContainer(
                              child: Text(
                                clip.pinned ? 'Unpin clip' : 'Pin clip',
                              ),
                            ),
                            child: Semantics(
                              label: clip.pinned ? 'Unpin clip' : 'Pin clip',
                              toggled: clip.pinned,
                              button: true,
                              child: Button.secondary(
                                key: ValueKey<String>(
                                  'history-row-pin-${clip.id}',
                                ),
                                style: const ButtonStyle.secondaryIcon(
                                  density: ButtonDensity.iconDense,
                                ),
                                onPressed: controller.isPinPending(clip.id)
                                    ? null
                                    : () => controller.togglePin(clip),
                                child: Icon(
                                  clip.pinned
                                      ? LucideIcons.pinOff
                                      : LucideIcons.pin,
                                ),
                              ),
                            ),
                          ),
                        ],
                        if (showActions)
                          Tooltip(
                            tooltip: (context) => const TooltipContainer(
                              child: Text('Delete clip'),
                            ),
                            child: Semantics(
                              label: 'Delete clip',
                              button: true,
                              child: Button.destructive(
                                key: ValueKey<String>(
                                  'history-row-delete-${clip.id}',
                                ),
                                style: const ButtonStyle.destructiveIcon(
                                  density: ButtonDensity.iconDense,
                                ),
                                onPressed: controller.isDeletePending(clip.id)
                                    ? null
                                    : () => showHistoryDeleteDialog(
                                        context,
                                        controller: controller,
                                        clipId: clip.id,
                                      ),
                                child: const Icon(LucideIcons.trash2),
                              ),
                            ),
                          ),
                        if (showHandle)
                          Tooltip(
                            key: ValueKey<String>(
                              'history-reorder-action-${clip.id}',
                            ),
                            tooltip: (context) => const TooltipContainer(
                              child: Text('Reorder pinned clip'),
                            ),
                            child: CallbackShortcuts(
                              bindings: {
                                const SingleActivator(
                                  LogicalKeyboardKey.arrowUp,
                                  alt: true,
                                ): () => unawaited(
                                  controller.shiftPinned(clip.id, up: true),
                                ),
                                const SingleActivator(
                                  LogicalKeyboardKey.arrowDown,
                                  alt: true,
                                ): () => unawaited(
                                  controller.shiftPinned(clip.id, up: false),
                                ),
                              },
                              child: Semantics(
                                label: 'Reorder pinned clip',
                                hint:
                                    'Drag to move. Alt + Up or Down moves one position.',
                                onIncrease: controller.canReorderPinned
                                    ? () => unawaited(
                                        controller.shiftPinned(
                                          clip.id,
                                          up: false,
                                        ),
                                      )
                                    : null,
                                onDecrease: controller.canReorderPinned
                                    ? () => unawaited(
                                        controller.shiftPinned(
                                          clip.id,
                                          up: true,
                                        ),
                                      )
                                    : null,
                                // Pan uses twice the touch slop of an axis drag.
                                // Match the list's threshold so its vertical scroll
                                // recognizer cannot steal a touch on this handle.
                                child: MediaQuery(
                                  data: MediaQuery.of(context).copyWith(
                                    gestureSettings: DeviceGestureSettings(
                                      touchSlop:
                                          (MediaQuery.gestureSettingsOf(
                                                context,
                                              ).touchSlop ??
                                              kTouchSlop) /
                                          2,
                                    ),
                                  ),
                                  child: Listener(
                                    onPointerCancel: (_) =>
                                        controller.cancelPinnedDrag(),
                                    child: SortableDragHandle(
                                      enabled: controller.canReorderPinned,
                                      child: Button.secondary(
                                        key: ValueKey<String>(
                                          'history-row-reorder-${clip.id}',
                                        ),
                                        style: AppTheme.historyDragHandleStyle(
                                          dragging: dragging,
                                          touch: touch,
                                        ),
                                        onPressed: controller.canReorderPinned
                                            ? () {}
                                            : null,
                                        child: const Icon(
                                          LucideIcons.gripVertical,
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ClipContent extends StatelessWidget {
  const _ClipContent({
    required this.clip,
    required this.controller,
    this.revealed = false,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final bool revealed;

  @override
  Widget build(BuildContext context) {
    if (clip.secret && !revealed) {
      return SecretSpoiler(
        key: ValueKey('history-spoiler-${clip.id}'),
        reveal: (_) => FutureBuilder<HistoryClip>(
          future: controller.revealClip(clip.id),
          builder: (context, snapshot) {
            if (snapshot.hasError) {
              return const StateView.error(title: 'Clip unavailable');
            }
            final full = snapshot.data;
            if (full == null) return const StateView.loading();
            return _ClipContent(
              clip: full,
              controller: controller,
              revealed: true,
            );
          },
        ),
      );
    }
    final kind = clip.contentKind;
    if (kind == HistoryClipKind.color && clip.colorRgba != null) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          HistoryColorSwatch(rgba: clip.colorRgba!),
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
    return LayoutBuilder(
      builder: (context, constraints) {
        final pixelRatio = MediaQuery.devicePixelRatioOf(context);
        // Limit both dimensions in physical pixels before decoding the preview.
        final bounds = HistoryImagePreviewBounds(
          width: (constraints.maxWidth * pixelRatio).ceil().clamp(1, 2048),
          height: (maxHeight * pixelRatio).ceil().clamp(1, 2048),
        );
        return FutureBuilder<HistoryImagePreview?>(
          future: controller.requestImagePreview(clip.id, bounds: bounds),
          builder: (context, snapshot) {
            final preview = snapshot.data;
            if (preview == null) {
              return const Center(child: Icon(LucideIcons.image));
            }
            if (preview.width <= 0 || preview.height <= 0) {
              return const Center(child: Icon(LucideIcons.imageOff));
            }
            final scale = math.min(
              1.0,
              math.min(
                constraints.maxWidth / preview.width,
                maxHeight / preview.height,
              ),
            );
            return Align(
              alignment: Alignment.centerLeft,
              child: SizedBox(
                width: preview.width * scale,
                height: preview.height * scale,
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(AppRadius.sm),
                  child: Image.memory(
                    preview.bytes,
                    key: ValueKey<String>('history-card-image-${clip.id}'),
                    fit: BoxFit.contain,
                    alignment: Alignment.centerLeft,
                    cacheWidth: math.min(
                      preview.width,
                      (preview.width * scale * pixelRatio).ceil().clamp(
                        1,
                        2048,
                      ),
                    ),
                    cacheHeight: math.min(
                      preview.height,
                      (preview.height * scale * pixelRatio).ceil().clamp(
                        1,
                        2048,
                      ),
                    ),
                    errorBuilder: (context, error, stackTrace) =>
                        const Center(child: Icon(LucideIcons.imageOff)),
                  ),
                ),
              ),
            );
          },
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
    final style = AppTheme.historyMetadataTextStyle(context);
    final strutStyle = AppTheme.historyMetadataStrutStyle(context);
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
                icon: controller.requestSourceIcon(clip.sourceAppIconId),
                iconSize: AppIconSize.xs,
                style: style,
                strutStyle: strutStyle,
              ),
            ),
          ],
          if (clip.origin != null) ...[
            const TextSpan(text: ' • '),
            WidgetSpan(
              alignment: PlaceholderAlignment.middle,
              child: DeviceLabel(
                name: clip.origin!,
                deviceClass: clip.originDeviceClass,
                iconSize: AppIconSize.xs,
                style: style,
                strutStyle: strutStyle,
              ),
            ),
          ],
          TextSpan(text: ' • ${formatSystemDateTime(context, clip.createdAt)}'),
        ],
      ),
      key: ValueKey<String>('history-clip-meta-${clip.id}'),
      style: style,
      strutStyle: strutStyle,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
  }
}
