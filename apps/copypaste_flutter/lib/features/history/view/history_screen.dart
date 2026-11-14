import '../../../app/theme/app_filter_indicator.dart';

import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';

import '../../../platform/files/history_file_drop_target.dart';

import 'package:copypaste_flutter/app/theme/app_overlays.dart';
import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/app/theme/app_toast.dart';
import 'package:copypaste_flutter/features/devices/device_label.dart';
import 'package:copypaste_flutter/features/history/controller/history_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_window_controller.dart';
import 'package:copypaste_flutter/features/history/controller/history_errors.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';

import '../controller/history_selection_gesture.dart';

import 'package:copypaste_flutter/features/history/models/history_models.dart';
import 'package:copypaste_flutter/features/history/presentation/history_code_highlighter.dart';
import 'package:copypaste_flutter/features/history/presentation/history_clip_presentation.dart';
import 'package:copypaste_flutter/features/history/presentation/source_app_label.dart';
import 'package:copypaste_flutter/shared/adaptive_breakpoints.dart';
import 'package:copypaste_flutter/shared/locale_case.dart';
import 'package:copypaste_flutter/shared/state_view.dart';
import 'package:copypaste_flutter/shared/system_date_time.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../presentation/history_expiry_text.dart';
import '../presentation/secret_spoiler.dart';

import 'history_inspector.dart';
import 'history_window_scroll.dart';
import 'history_delete_dialog.dart';
import 'history_bulk_toolbar.dart';
import 'history_date_filter.dart';
import 'history_tags_filter.dart';
import 'history_clip_menu.dart';
import '../presentation/history_color_swatch.dart';
import '../../../platform/desktop/desktop_shortcuts.dart';

/// The shell-owned History destination body. It intentionally does not add an
/// AppBar because AppShell owns the destination title and navigation chrome.
class HistoryScreen extends StatefulWidget {
  const HistoryScreen({
    super.key,
    required this.controller,
    this.onDrawerVisibilityChanged,
    this.findRequests,
  });

  final HistoryController controller;
  final ValueChanged<bool>? onDrawerVisibilityChanged;
  final ValueNotifier<bool>? findRequests;

  @override
  State<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends State<HistoryScreen> {
  final _listKey = GlobalKey<_HistoryListState>();
  final _selectionFocus = FocusNode(
    debugLabel: 'History selection',
    skipTraversal: true,
  );
  late final HistorySelectionGesture _selectionGesture;
  bool _wasSelecting = false;
  late final TextEditingController _searchController;
  ScrollController? _scrollController;
  ScrollController? _fallbackScrollController;
  bool _detailDrawerOpen = false;
  bool _fileDragHover = false;
  late int _operationErrorRevision;

  @override
  void initState() {
    super.initState();
    _operationErrorRevision = widget.controller.errors.operationErrorRevision;
    _searchController = TextEditingController(
      text: widget.controller.window.query.search,
    );
    _selectionGesture = HistorySelectionGesture(
      controller: widget.controller,
      clipAt: (position) => _listKey.currentState?.clipAt(position),
      orderedIds: () => _listKey.currentState?.orderedIds ?? const [],
      viewportBounds: () => _listKey.currentState?.viewportBounds,
    );
    widget.controller.addListener(_selectionChanged);
    widget.controller.window.beforeWindowChange = _captureMutationAnchor;
    widget.controller.window.afterWindowChange = _restoreMutationAnchor;
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
    widget.controller.removeListener(_selectionChanged);
    widget.controller.window.beforeWindowChange = null;
    widget.controller.window.afterWindowChange = null;
    _selectionGesture.dispose();
    _selectionFocus.dispose();
    _searchController.dispose();
    _scrollController?.removeListener(_loadMoreWhenNeeded);
    _fallbackScrollController?.dispose();
    super.dispose();
  }

  void _selectionChanged() {
    final revision = widget.controller.errors.operationErrorRevision;
    if (revision != _operationErrorRevision) {
      _operationErrorRevision = revision;
      final message = widget.controller.errors.operationError;
      final action = widget.controller.errors.failedAction;
      // Pin/delete failures already appear in this screen's attention state.
      if (message != null &&
          action != HistoryAction.pin &&
          action != HistoryAction.delete) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) {
            AppToast.show(
              context,
              title: context.l10n.historyActionFailedTitle,
              message: message,
              tone: AppToastTone.error,
            );
          }
        });
      }
    }
    final selecting = widget.controller.bulk.isBulkSelecting;
    if (selecting && !_wasSelecting) _selectionFocus.requestFocus();
    _wasSelecting = selecting;
  }

  void _deleteBulkSelection() {
    if (widget.controller.bulk.isBulkMutating ||
        widget.controller.bulk.bulkSelectedIds.isEmpty) {
      return;
    }
    _selectionGesture.end();
    unawaited(
      showHistoryBulkDeleteDialog(context, controller: widget.controller),
    );
  }

  List<(String, double)> _mutationAnchors = const [];

  Future<Set<String>> _captureMutationAnchor() async {
    final viewport = _listKey.currentState?.viewportBounds;
    if (viewport == null) return const {};
    final anchors = <(String, double)>[];
    for (final entry in _listKey.currentState!._clipKeys.entries) {
      final box = entry.value.currentContext?.findRenderObject();
      if (box is! RenderBox || !box.attached || !box.hasSize) continue;
      final rect = box.localToGlobal(Offset.zero) & box.size;
      if (rect.overlaps(viewport)) {
        anchors.add((entry.key, rect.top - viewport.top));
      }
    }
    anchors.sort((left, right) => left.$2.compareTo(right.$2));
    _mutationAnchors = List.unmodifiable(anchors);
    return anchors.map((anchor) => anchor.$1).toSet();
  }

  void _restoreMutationAnchor() {
    final anchors = _mutationAnchors;
    if (anchors.isEmpty) return;
    _mutationAnchors = const [];
    WidgetsBinding.instance.addPostFrameCallback((_) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        final viewport = _listKey.currentState?.viewportBounds;
        final scroll = _scrollController;
        if (viewport == null || scroll?.hasClients != true) return;
        for (final anchor in anchors) {
          final box = _listKey
              .currentState
              ?._clipKeys[anchor.$1]
              ?.currentContext
              ?.findRenderObject();
          if (box is! RenderBox || !box.attached || !box.hasSize) continue;
          final delta =
              box.localToGlobal(Offset.zero).dy - viewport.top - anchor.$2;
          final position = scroll!.position;
          scroll.jumpTo(
            (position.pixels + delta).clamp(
              position.minScrollExtent,
              position.maxScrollExtent,
            ),
          );
          break;
        }
      });
    });
  }

  void _loadMoreWhenNeeded() {
    _selectionGesture.updateAfterScroll();
    final controller = _scrollController;
    if (controller == null || !controller.hasClients) return;
    final position = controller.position;
    if (position.pixels <= position.minScrollExtent + 1 &&
        widget.controller.window.canLoadPrevious) {
      unawaited(widget.controller.window.loadPrevious());
    } else if (position.pixels >= position.maxScrollExtent - 360) {
      unawaited(widget.controller.window.loadMore());
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
                  widget.controller.transfers.canImportFiles &&
                  !widget.controller.transfers.isImportingFiles &&
                  !widget.controller.bulk.isBulkSelecting &&
                  !_detailDrawerOpen,
              onFiles: (files) =>
                  unawaited(widget.controller.transfers.importFiles(files)),
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
                    bindings: widget.controller.bulk.isBulkSelecting
                        ? {
                            const SingleActivator(LogicalKeyboardKey.escape):
                                widget.controller.bulk.endBulkSelection,
                            const SingleActivator(LogicalKeyboardKey.delete):
                                _deleteBulkSelection,
                            const SingleActivator(LogicalKeyboardKey.backspace):
                                _deleteBulkSelection,
                          }
                        : {
                            if (usesDesktopShortcuts &&
                                widget.controller.capabilities.edits != null)
                              desktopShortcut(LogicalKeyboardKey.keyN): () =>
                                  unawaited(
                                    showHistoryNewClipDrawer(
                                      context,
                                      controller: widget.controller,
                                    ),
                                  ),
                          },
                    child: Focus(
                      focusNode: _selectionFocus,
                      child: Listener(
                        onPointerDown: _selectionGesture.pointerDown,
                        onPointerMove: _selectionGesture.pointerMove,
                        onPointerUp: _selectionGesture.pointerUp,
                        onPointerCancel: _selectionGesture.pointerCancel,
                        child: body,
                      ),
                    ),
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
    final state = widget.controller.window.state;
    if (state == HistoryLoadState.initial) {
      return StateView.loading(message: context.l10n.historyLoadingMessage);
    }
    final list = _HistoryList(
      key: _listKey,
      controller: widget.controller,
      scrollController: _scrollController!,
      searchController: _searchController,
      findRequests: widget.findRequests,
      showKindLabel: wide,
      onSelected: (clip) => _select(context, clip, wide: wide),
      onDragSelection: _selectionGesture.begin,
    );
    if (!wide) return list;
    if (widget.controller.selection.selectedClip == null) {
      return SizedBox.expand(
        key: const ValueKey<String>('history-clip-list'),
        child: list,
      );
    }
    // Resizable panes clip their content, so the panel reaches past this
    // page by the focus-ring inset and each pane pads it back: the toolbar
    // and inspector rings stay whole and nothing moves.
    const inset = AppTheme.focusRingInset;
    return AppOverlays.focusRingOverflow(
      child: ResizablePanel.horizontal(
        dividerBuilder: (context) => null,
        draggerThickness: 12,
        children: [
          ResizablePane.flex(
            minSize: 440 + inset,
            initialFlex: 1.45,
            child: Padding(
              padding: const EdgeInsetsDirectional.fromSTEB(
                inset,
                inset,
                AppSpacing.lg,
                inset,
              ),
              child: SizedBox.expand(
                key: const ValueKey<String>('history-clip-list'),
                child: list,
              ),
            ),
          ),
          ResizablePane(
            initialSize: 360 + inset,
            minSize: 300 + inset,
            maxSize: 560 + inset,
            child: Padding(
              padding: const EdgeInsetsDirectional.fromSTEB(
                0,
                inset,
                inset,
                inset,
              ),
              child: HistoryInspector(
                controller: widget.controller,
                inDrawer: false,
              ),
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _select(
    BuildContext context,
    HistoryClip clip, {
    required bool wide,
  }) async {
    await widget.controller.selection.select(clip.id);
    if (!context.mounted || wide || _detailDrawerOpen) return;
    _detailDrawerOpen = true;
    widget.onDrawerVisibilityChanged?.call(true);
    try {
      await WidgetsBinding.instance.endOfFrame;
      if (!context.mounted || widget.controller.selection.selectedId == null) {
        return;
      }
      var drawerCloseScheduled = false;
      await showOverlay<void>(
        context,
        AppOverlays.drawerConfiguration(context),
        builder: (context) => ConstrainedBox(
          key: const ValueKey<String>('history-detail-drawer'),
          constraints: AppOverlays.drawerContentConstraints(context),
          child: AnimatedBuilder(
            animation: widget.controller,
            builder: (context, child) {
              if (widget.controller.selection.selectedId == null &&
                  !drawerCloseScheduled) {
                drawerCloseScheduled = true;
                WidgetsBinding.instance.addPostFrameCallback((_) {
                  if (context.mounted) unawaited(closeDrawer(context));
                });
              }
              return HistoryInspector(
                controller: widget.controller,
                inDrawer: true,
              );
            },
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
    super.key,
    required this.controller,
    required this.scrollController,
    required this.searchController,
    this.findRequests,
    required this.showKindLabel,
    required this.onSelected,
    required this.onDragSelection,
  });

  final HistoryController controller;
  final ScrollController scrollController;
  final TextEditingController searchController;
  final ValueNotifier<bool>? findRequests;
  final bool showKindLabel;
  final ValueChanged<HistoryClip> onSelected;
  final void Function(String, Offset, ScrollableState) onDragSelection;

  @override
  State<_HistoryList> createState() => _HistoryListState();
}

class _HistoryListState extends State<_HistoryList> {
  final Map<String, SortableData<String>> _sortableData = {};
  final Map<String, GlobalKey> _clipKeys = {};
  final _viewportKey = GlobalKey();
  List<String> orderedIds = const [];
  double _bottomClearance = 0;

  Rect? get viewportBounds {
    final viewport = _viewportKey.currentContext?.findRenderObject();
    if (viewport is! RenderBox || !viewport.hasSize || !viewport.attached) {
      return null;
    }
    final rect = viewport.localToGlobal(Offset.zero) & viewport.size;
    final visible = Rect.fromLTRB(
      rect.left,
      rect.top,
      rect.right,
      math.max(rect.top, rect.bottom - _bottomClearance),
    );
    return visible.height <= AppSpacing.xs ? null : visible;
  }

  String? clipAt(Offset position) {
    final rect = viewportBounds;
    if (rect == null) return null;
    final point = Offset(
      rect.center.dx,
      position.dy.clamp(
        rect.top + AppSpacing.xxs,
        rect.bottom - AppSpacing.xxs,
      ),
    );
    for (final entry in _clipKeys.entries) {
      final box = entry.value.currentContext?.findRenderObject();
      if (box is RenderBox &&
          box.attached &&
          box.hasSize &&
          (box.localToGlobal(Offset.zero) & box.size).contains(point)) {
        return entry.key;
      }
    }
    return null;
  }

  HistoryController get controller => widget.controller;
  ScrollController get scrollController => widget.scrollController;
  TextEditingController get searchController => widget.searchController;
  bool get showKindLabel => widget.showKindLabel;
  ValueChanged<HistoryClip> get onSelected => widget.onSelected;

  late final _focus = FocusNode(
    debugLabel: 'History list',
    skipTraversal: true,
    onKeyEvent: _handleKey,
  );

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  bool get _editingText =>
      FocusManager.instance.primaryFocus?.context
          ?.findAncestorWidgetOfExactType<EditableText>() !=
      null;

  /// Shortcuts for the selected clip, unless a text field owns them:
  /// copy, and the edit, save-as-snippet and transform actions.
  static final _clipShortcuts = {
    LogicalKeyboardKey.keyE: HistoryClipAction.edit,
    LogicalKeyboardKey.keyS: HistoryClipAction.saveAsSnippet,
    LogicalKeyboardKey.keyT: HistoryClipAction.transform,
  };

  KeyEventResult _handleKey(FocusNode node, KeyEvent event) {
    final id = controller.selection.selectedId;
    if (id == null || controller.bulk.isBulkSelecting || _editingText) {
      return KeyEventResult.ignored;
    }
    final keyboard = HardwareKeyboard.instance;
    if (desktopShortcut(LogicalKeyboardKey.keyC).accepts(event, keyboard)) {
      unawaited(copyHistoryClip(context, controller: controller, clipId: id));
      return KeyEventResult.handled;
    }
    final clip =
        controller.selection.selectedClip ?? controller.window.clipById(id);
    if (clip == null) return KeyEventResult.ignored;
    for (final MapEntry(key: key, value: action) in _clipShortcuts.entries) {
      if (!desktopShortcut(key).accepts(event, keyboard)) continue;
      if (!historyClipActions(controller, clip).contains(action)) {
        return KeyEventResult.ignored;
      }
      unawaited(
        runHistoryClipAction(
          context,
          controller: controller,
          clip: clip,
          action: action,
        ),
      );
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) =>
      Focus(focusNode: _focus, child: _content(context));

  Widget _content(BuildContext context) {
    if (controller.window.state == HistoryLoadState.error) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _HistoryToolbar(
            controller: controller,
            searchController: searchController,
            findRequests: widget.findRequests,
          ),
          const Gap(AppSpacing.md),
          Expanded(
            child: StateView.error(
              title: context.l10n.historyUnavailableTitle,
              message: controller.errors.message,
              actionLabel: context.l10n.commonTryAgain,
              onAction: () => unawaited(controller.window.retryHistory()),
            ),
          ),
        ],
      );
    }
    if (controller.window.state == HistoryLoadState.empty) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _HistoryToolbar(
            controller: controller,
            searchController: searchController,
            findRequests: widget.findRequests,
          ),
          if (controller.window.skippedUndecryptable > 0) ...[
            const Gap(AppSpacing.md),
            _SkippedRowsWarning(count: controller.window.skippedUndecryptable),
          ],
          Expanded(
            child: StateView.empty(
              title: context.l10n.historyEmptyTitle,
              message: context.l10n.historyEmptyMessage,
            ),
          ),
        ],
      );
    }
    if (controller.window.state == HistoryLoadState.loading) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _HistoryToolbar(
            controller: controller,
            searchController: searchController,
            findRequests: widget.findRequests,
          ),
          Expanded(
            child: StateView.loading(
              message: context.l10n.historyLoadingMessage,
            ),
          ),
        ],
      );
    }
    final now = DateTime.now();
    final pages = controller.window.windowPages;
    String? previousSectionKey;
    final pageRows = <List<_HistoryListRow>?>[];
    for (final page in pages) {
      final items = page.items;
      pageRows.add(
        items == null
            ? null
            : _historyListRows(
                items,
                sort: controller.window.query.sort,
                now: now,
                isSectionCollapsed: controller.window.isSectionCollapsed,
                initialSectionKey: previousSectionKey,
              ),
      );
      previousSectionKey = _pageFinalSectionKey(page, now);
    }
    orderedIds = controller.window.visibleVisitedIds;
    final showLoadMore = controller.window.canLoadMore;
    final retainedIds = controller.window.items.map((item) => item.id).toSet();
    _sortableData.removeWhere((id, _) => !retainedIds.contains(id));
    _clipKeys.removeWhere((id, _) => !retainedIds.contains(id));
    final bottomPadding = MediaQuery.paddingOf(context).bottom;
    _bottomClearance = bottomPadding;
    final hasError = controller.errors.message != null;
    final hasLoadMore = controller.window.isLoadingMore || showLoadMore;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _HistoryToolbar(
          controller: controller,
          searchController: searchController,
          findRequests: widget.findRequests,
        ),
        if (controller.window.skippedUndecryptable > 0) ...[
          const Gap(AppSpacing.md),
          _SkippedRowsWarning(count: controller.window.skippedUndecryptable),
        ],
        // The list's own top inset completes the toolbar gap.
        const Gap(AppSpacing.md - AppTheme.focusRingInset),
        Expanded(
          // The list reaches past the column by the focus-ring inset and its
          // rows are inset by the same amount, so a focused card's ring fits
          // the viewport while cards stay aligned with the toolbar.
          child: AppOverlays.focusRingOverflow(
            axis: Axis.horizontal,
            child: SortableLayer(
              clipBehavior: Clip.hardEdge,
              dropDuration: AppMotion.resolve(context, AppMotion.standard),
              dropCurve: AppMotion.standardCurve,
              child: ScrollableSortableLayer(
                controller: scrollController,
                scrollThreshold: AppControlSize.touch,
                child: HistoryWindowScroll(
                  controller: scrollController,
                  viewportKey: _viewportKey,
                  topPadding: AppTheme.focusRingInset,
                  bottomPadding: bottomPadding,
                  pages: [
                    for (
                      var pageIndex = 0;
                      pageIndex < pages.length;
                      pageIndex++
                    )
                      HistoryWindowPageView(
                        id: pageIndex,
                        extent: pages[pageIndex].extent,
                        rows: pageRows[pageIndex]?.length ?? 0,
                        rowBuilder: pageRows[pageIndex] == null
                            ? null
                            : (context, index) =>
                                  _buildRow(pageRows[pageIndex]![index]),
                        onMeasured: (extent) => unawaited(
                          controller.window.reportPageExtent(pageIndex, extent),
                        ),
                        onPlaceholderVisible: () => unawaited(
                          controller.window.materializePage(pageIndex),
                        ),
                      ),
                  ],
                  trailing: hasLoadMore || hasError
                      ? Padding(
                          padding: const EdgeInsets.all(AppSpacing.lg),
                          child: Center(
                            child: hasError
                                ? Alert.destructive(
                                    leading: const AppIconFrame(
                                      child: Icon(LucideIcons.circleAlert),
                                    ),
                                    title: Text(
                                      context.l10n.historyAttentionTitle,
                                    ),
                                    content: Text(controller.errors.message!),
                                  )
                                : controller.window.isLoadingMore
                                ? const AppIconFrame(
                                    size: AppIconSize.sm,
                                    child: CircularProgressIndicator(),
                                  )
                                : Button.ghost(
                                    onPressed: () =>
                                        unawaited(controller.window.loadMore()),
                                    child: Text(context.l10n.historyLoadMore),
                                  ),
                          ),
                        )
                      : null,
                ),
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

  Widget _buildRow(_HistoryListRow row) => switch (row) {
    _HistorySectionRow(:final section) => _HistorySectionHeader(
      key: _rowKey(row),
      section: section,
      count: section.count(controller.window.groupCounts),
      collapsed: controller.window.isSectionCollapsed(section.key),
      onToggle: () => controller.window.toggleSection(section.key),
    ),
    _HistoryClipRow(:final clip) => _clipRow(clip),
  };

  String? _pageFinalSectionKey(dynamic page, DateTime now) {
    if (page.ids.isEmpty) return null;
    final last = page.ids.length - 1;
    if (page.pinnedIds.contains(page.ids[last])) return 'pinned';
    if (controller.window.query.sort == HistorySort.relevance) return null;
    return _historyDateSectionKey(
      DateTime.fromMillisecondsSinceEpoch(page.createdAtMillis[last]),
      now,
    );
  }

  Widget _clipRow(HistoryClip clip) {
    final child = Padding(
      padding: _historyRowPadding,
      child: _HistoryClipCard(
        clip: clip,
        controller: controller,
        selected: controller.bulk.isBulkSelecting
            ? controller.bulk.isBulkSelected(clip.id)
            : controller.selection.selectedId == clip.id,
        showKindLabel: showKindLabel,
        onPressed: () {
          final keys = HardwareKeyboard.instance;
          final range = keys.isShiftPressed;
          // macOS reserves Control-click for the context menu.
          final additive =
              keys.isMetaPressed ||
              (keys.isControlPressed &&
                  Theme.of(context).platform != TargetPlatform.macOS);
          if (controller.bulk.isBulkSelecting || range || additive) {
            controller.bulk.selectBulkClip(
              clip.id,
              orderedIds: orderedIds,
              range: range,
              additive: additive,
            );
          } else {
            if (!_focus.hasFocus || _editingText) _focus.requestFocus();
            onSelected(clip);
          }
        },
        onDragSelection: (position, scrollable) =>
            widget.onDragSelection(clip.id, position, scrollable),
      ),
    );
    final key = ValueKey<String>('history-row-${clip.id}');
    final boundsKey = _clipKeys.putIfAbsent(clip.id, GlobalKey.new);
    Widget content = child;
    if (clip.pinned) {
      content = Sortable<String>(
        data: _sortableData.putIfAbsent(clip.id, () => SortableData(clip.id)),
        // Only the library-owned drag handle can initiate a gesture.
        enabled: false,
        canAcceptTop: (data) =>
            controller.pinnedOrder.canReorderPinned && data.data != clip.id,
        canAcceptBottom: (data) =>
            controller.pinnedOrder.canReorderPinned && data.data != clip.id,
        onAcceptTop: (data) => unawaited(
          controller.pinnedOrder.movePinned(data.data, clip.id, before: true),
        ),
        onAcceptBottom: (data) => unawaited(
          controller.pinnedOrder.movePinned(data.data, clip.id, before: false),
        ),
        onDragStart: () => controller.pinnedOrder.beginPinnedDrag(clip.id),
        onDragEnd: controller.pinnedOrder.endPinnedDrag,
        onDragCancel: controller.pinnedOrder.endPinnedDrag,
        placeholder: Padding(
          padding: _historyRowPadding,
          child: DecoratedBox(
            key: const ValueKey<String>('history-pin-drop-placeholder'),
            decoration: AppTheme.historyPinnedDropDecoration(context),
            child: Center(
              child: AppIconFrame(
                size: AppIconSize.md,
                child: Icon(
                  LucideIcons.moveVertical,
                  size: AppIconSize.md,
                  color: Theme.of(context).colorScheme.primary,
                ),
              ),
            ),
          ),
        ),
        child: child,
      );
    }
    // Sortable duplicates its child into drag feedback. The geometry key must
    // belong to the list row, outside that copied subtree.
    return KeyedSubtree(
      key: key,
      child: SizedBox(key: boundsKey, child: content),
    );
  }
}

/// Rows sit inside the list's focus-ring inset with the shared row gap below.
const _historyRowPadding = EdgeInsets.fromLTRB(
  AppTheme.focusRingInset,
  0,
  AppTheme.focusRingInset,
  AppSpacing.sm,
);

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
    _HistorySectionKind.pinned => context.l10n.historySectionPinned,
    _HistorySectionKind.today => context.l10n.historySectionToday,
    _HistorySectionKind.yesterday => context.l10n.historySectionYesterday,
    _HistorySectionKind.date => formatSystemDate(context, date!),
  };

  /// The exact number of clips in the group; null while it is not known.
  int? count(HistoryGroupCounts? counts) => switch (kind) {
    _HistorySectionKind.pinned => counts?.pinned,
    _ => date == null ? null : counts?.day(date!),
  };

  /// The calendar date behind a relative day label.
  String? detail(BuildContext context) => switch (kind) {
    _HistorySectionKind.today ||
    _HistorySectionKind.yesterday => formatSystemWeekdayDate(context, date!),
    _HistorySectionKind.pinned || _HistorySectionKind.date => null,
  };
}

/// The band that opens a History group and collapses it.
class _HistorySectionHeader extends StatelessWidget {
  const _HistorySectionHeader({
    super.key,
    required this.section,
    required this.collapsed,
    required this.onToggle,
    this.count,
  });

  final _HistorySection section;
  final int? count;
  final bool collapsed;
  final VoidCallback onToggle;

  @override
  Widget build(BuildContext context) {
    final locale =
        Localizations.maybeLocaleOf(context) ?? AppLocalization.fallback;
    final label = section.label(context);
    final detail = section.detail(context);
    final count = switch (this.count) {
      final count? => context.l10n.historySectionClipCount(count),
      null => null,
    };
    return Padding(
      padding: _historyRowPadding,
      child: Semantics(
        header: true,
        button: true,
        expanded: !collapsed,
        label: [label, ?detail, ?count].join(', '),
        child: Button.ghost(
          key: ValueKey<String>('history-section-${section.key}'),
          style: AppTheme.historySectionButtonStyle,
          onPressed: onToggle,
          child: ExcludeSemantics(
            child: Row(
              spacing: AppSpacing.sm,
              children: [
                if (section.kind == _HistorySectionKind.pinned)
                  const AppIconFrame(
                    size: AppIconSize.sm,
                    child: Icon(LucideIcons.pin, size: AppIconSize.sm),
                  ),
                Expanded(
                  // The count, then the date, drops below the label before
                  // any of them wraps.
                  child: Wrap(
                    spacing: AppSpacing.sm,
                    children: [
                      Text(
                        toLocaleUpperCase(label, locale),
                        style: AppTheme.historySectionTextStyle(
                          context,
                          emphasized: true,
                        ),
                      ),
                      if (detail != null)
                        Text(
                          toLocaleUpperCase(detail, locale),
                          style: AppTheme.historySectionTextStyle(
                            context,
                            emphasized: false,
                          ),
                        ),
                      if (count != null)
                        Text(
                          toLocaleUpperCase(count, locale),
                          key: ValueKey<String>(
                            'history-section-count-${section.key}',
                          ),
                          style: AppTheme.historySectionTextStyle(
                            context,
                            emphasized: false,
                          ),
                        ),
                    ],
                  ),
                ),
                AppIconFrame(
                  size: AppIconSize.sm,
                  child: Icon(
                    collapsed
                        ? LucideIcons.chevronRight
                        : LucideIcons.chevronDown,
                    size: AppIconSize.sm,
                  ),
                ),
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
  String? initialSectionKey,
}) {
  final rows = <_HistoryListRow>[];
  final pinned = items.where((item) => item.pinned);
  if (pinned.isNotEmpty) {
    if (initialSectionKey != 'pinned') {
      rows.add(
        const _HistorySectionRow(_HistorySection(_HistorySectionKind.pinned)),
      );
    }
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
  String? previousSectionKey = initialSectionKey;
  for (final clip in unpinned) {
    final date = _calendarDate(clip.createdAt);
    final section = _dateSection(date, today: today);
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

String _historyDateSectionKey(DateTime value, DateTime now) {
  final date = _calendarDate(value);
  final today = _calendarDate(now);
  final yesterday = DateTime(today.year, today.month, today.day - 1);
  if (date == today) return 'today';
  if (date == yesterday) return 'yesterday';
  return 'date-${date.toIso8601String().split('T').first}';
}

_HistorySection _dateSection(DateTime date, {required DateTime today}) {
  return switch (_historyDateSectionKey(date, today)) {
    'today' => _HistorySection(_HistorySectionKind.today, date: date),
    'yesterday' => _HistorySection(_HistorySectionKind.yesterday, date: date),
    _ => _HistorySection(_HistorySectionKind.date, date: date),
  };
}

class _SkippedRowsWarning extends StatelessWidget {
  const _SkippedRowsWarning({required this.count});

  final int count;

  @override
  Widget build(BuildContext context) {
    return Alert(
      leading: const AppIconFrame(child: Icon(LucideIcons.triangleAlert)),
      title: Text(context.l10n.historySkippedRowsTitle(count)),
      content: Text(context.l10n.historySkippedRowsMessage),
    );
  }
}

class _HistoryToolbar extends StatefulWidget {
  const _HistoryToolbar({
    required this.controller,
    required this.searchController,
    this.findRequests,
  });

  final HistoryController controller;
  final TextEditingController searchController;
  final ValueNotifier<bool>? findRequests;

  @override
  State<_HistoryToolbar> createState() => _HistoryToolbarState();
}

class _HistoryToolbarState extends State<_HistoryToolbar> {
  final _searchFocus = FocusNode(debugLabel: 'History search');

  @override
  void initState() {
    super.initState();
    widget.findRequests?.addListener(_find);
    if (widget.findRequests?.value ?? false) _find();
  }

  @override
  void didUpdateWidget(covariant _HistoryToolbar oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.findRequests != widget.findRequests) {
      oldWidget.findRequests?.removeListener(_find);
      widget.findRequests?.addListener(_find);
      if (widget.findRequests?.value ?? false) _find();
    }
  }

  @override
  void dispose() {
    widget.findRequests?.removeListener(_find);
    _searchFocus.dispose();
    super.dispose();
  }

  void _find() {
    if (!(widget.findRequests?.value ?? false)) return;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || !(widget.findRequests?.value ?? false)) return;
      widget.controller.bulk.endBulkSelection();
      setState(() => _searchExpanded = true);
      widget.findRequests!.value = false;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        _searchFocus.requestFocus();
        widget.searchController.selection = TextSelection(
          baseOffset: 0,
          extentOffset: widget.searchController.text.length,
        );
      });
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  double _controlExtent(BuildContext context) =>
      AppTheme.controlHeight(context, minimum: AppControlSize.large);
  static const double _minimumExpandedSearchWidth =
      AppLayoutSize.historySearchMinWidth;
  static const double _toolbarGap = AppSpacing.sm;
  static const int _filterCount = 6;
  bool _searchExpanded = false;

  @override
  Widget build(BuildContext context) {
    if (widget.controller.bulk.isBulkSelecting) {
      return HistoryBulkToolbar(controller: widget.controller);
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        final hasReset = widget.controller.window.hasActiveFilters;
        final newClip = _showsNewClipButton(context);
        final actionCount =
            _filterCount + 2 + (hasReset ? 1 : 0) + (newClip ? 1 : 0);
        final compactFilters =
            constraints.maxWidth < _expandedToolbarMinimumWidth(context);
        final collapseSearch =
            constraints.maxWidth <
            _minimumExpandedSearchWidth +
                (_controlExtent(context) * actionCount) +
                (_toolbarGap * actionCount);
        final naturalWidth =
            _controlExtent(context) * (actionCount + 1) +
            _toolbarGap * actionCount;
        final toolbarScale = math.min(
          AppTheme.historyToolbarScale(context),
          constraints.maxWidth / naturalWidth,
        );
        if (collapseSearch && (_searchExpanded || _searchFocus.hasFocus)) {
          if (toolbarScale <= 0) return const SizedBox.shrink();
          return SizedBox(
            width: constraints.maxWidth,
            child: FittedBox(
              fit: BoxFit.scaleDown,
              alignment: Alignment.centerLeft,
              child: SizedBox(
                width: constraints.maxWidth / toolbarScale,
                child: _searchField(compact: true),
              ),
            ),
          );
        }

        final filters = _filterSelects(
          context,
          compact: compactFilters || collapseSearch,
        );
        if (collapseSearch) {
          return Align(
            alignment: Alignment.centerLeft,
            child: ConstrainedBox(
              constraints: BoxConstraints(
                maxWidth: naturalWidth * toolbarScale,
              ),
              child: FittedBox(
                fit: BoxFit.scaleDown,
                alignment: Alignment.centerLeft,
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  spacing: _toolbarGap,
                  children: [
                    _selectionButton(),
                    AppFilterIndicator(
                      active: widget.controller.window.query.hasSearch,
                      tooltip: context.l10n.historySearch,
                      child: Semantics(
                        label: context.l10n.historySearch,
                        button: true,
                        child: Button.secondary(
                          key: const ValueKey<String>('history-search-toggle'),
                          style: AppTheme.historyToolbarIconStyle,
                          onPressed: () =>
                              setState(() => _searchExpanded = true),
                          child: const AppIconFrame(
                            size: AppIconSize.md,
                            child: Icon(
                              LucideIcons.search,
                              size: AppIconSize.md,
                            ),
                          ),
                        ),
                      ),
                    ),
                    ...filters,
                    if (hasReset) _resetButton(),
                    _importButton(),
                    if (newClip) _newClipButton(),
                  ],
                ),
              ),
            ),
          );
        }

        return Row(
          spacing: _toolbarGap,
          children: [
            _selectionButton(),
            Expanded(
              child: ConstrainedBox(
                constraints: const BoxConstraints(
                  minWidth: _minimumExpandedSearchWidth,
                ),
                child: _searchField(compact: false),
              ),
            ),
            ...filters,
            if (hasReset) _resetButton(),
            _importButton(),
            if (newClip) _newClipButton(),
          ],
        );
      },
    );
  }

  /// Android has no keyboard shortcut for a new clip, so it gets a button.
  bool _showsNewClipButton(BuildContext context) =>
      Theme.of(context).platform == TargetPlatform.android &&
      widget.controller.capabilities.edits != null;

  Widget _newClipButton() => Semantics(
    label: context.l10n.historyNewClip,
    button: true,
    child: Tooltip(
      tooltip: (_) =>
          TooltipContainer(child: Text(context.l10n.historyNewClip)),
      child: Button.secondary(
        key: const ValueKey<String>('history-new-clip'),
        style: AppTheme.historyToolbarIconStyle,
        onPressed: () => unawaited(
          showHistoryNewClipDrawer(context, controller: widget.controller),
        ),
        child: const AppIconFrame(
          size: AppIconSize.md,
          child: Icon(LucideIcons.plus, size: AppIconSize.md),
        ),
      ),
    ),
  );

  Widget _resetButton() => Tooltip(
    tooltip: (_) =>
        TooltipContainer(child: Text(context.l10n.historyResetFilters)),
    child: Semantics(
      label: context.l10n.historyResetFilters,
      button: true,
      child: Button.secondary(
        key: const ValueKey('history-reset-filters'),
        style: AppTheme.historyToolbarIconStyle,
        onPressed: () {
          widget.searchController.clear();
          setState(() => _searchExpanded = false);
          unawaited(widget.controller.window.resetFilters());
        },
        child: const AppIconFrame(
          size: AppIconSize.md,
          child: Icon(LucideIcons.filterX, size: AppIconSize.md),
        ),
      ),
    ),
  );

  Widget _importButton() => Semantics(
    label: context.l10n.historyImportFiles,
    button: true,
    child: Tooltip(
      tooltip: (_) =>
          TooltipContainer(child: Text(context.l10n.historyImportFiles)),
      child: Button.secondary(
        key: const ValueKey<String>('history-import-files'),
        style: AppTheme.historyToolbarIconStyle,
        onPressed:
            widget.controller.transfers.canImportFiles &&
                !widget.controller.transfers.isImportingFiles
            ? () => unawaited(widget.controller.transfers.chooseFiles())
            : null,
        child: widget.controller.transfers.isImportingFiles
            ? const AppIconFrame(
                size: AppIconSize.md,
                child: CircularProgressIndicator(size: AppIconSize.md),
              )
            : const AppIconFrame(
                size: AppIconSize.md,
                child: Icon(LucideIcons.filePlus, size: AppIconSize.md),
              ),
      ),
    ),
  );

  Widget _selectionButton() => Semantics(
    label: context.l10n.historySelectClips,
    button: true,
    child: Tooltip(
      tooltip: (_) =>
          TooltipContainer(child: Text(context.l10n.historySelectClips)),
      child: Button.secondary(
        key: const ValueKey('history-select-clips'),
        style: AppTheme.historyToolbarIconStyle,
        onPressed: widget.controller.bulk.canBeginBulkSelection
            ? () {
                FocusManager.instance.primaryFocus?.unfocus();
                widget.controller.bulk.beginBulkSelection();
              }
            : null,
        child: const AppIconFrame(
          size: AppIconSize.md,
          child: Icon(LucideIcons.squareCheck, size: AppIconSize.md),
        ),
      ),
    ),
  );

  Widget _searchField({required bool compact}) {
    final field = TextField(
      padding: AppTheme.controlFieldPadding(
        context,
        minimum: AppControlSize.large,
      ),
      controller: widget.searchController,
      focusNode: _searchFocus,
      autofocus: compact,
      placeholder: Text(context.l10n.historySearch),
      onChanged: widget.controller.window.updateSearch,
      skipInputFeatureFocusTraversal: true,
      features: compact
          ? [
              const InputFeature.leading(
                AppIconFrame(child: Icon(LucideIcons.search)),
              ),
              InputFeature.trailing(
                Semantics(
                  label: context.l10n.historyCloseSearch,
                  button: true,
                  child: Button.ghost(
                    style: AppTheme.inputFeatureButtonStyle,
                    onPressed: _closeCompactSearch,
                    child: const AppIconFrame(child: Icon(LucideIcons.x)),
                  ),
                ),
              ),
            ]
          : const [
              InputFeature.leading(
                AppIconFrame(child: Icon(LucideIcons.search)),
              ),
              InputFeature.clear(
                icon: AppIconFrame(child: Icon(LucideIcons.x)),
              ),
            ],
    );
    if (!compact) return field;
    // The close button is out of Tab order, so Escape closes the search.
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.escape): _closeCompactSearch,
      },
      child: field,
    );
  }

  void _closeCompactSearch() {
    _searchFocus.unfocus();
    widget.searchController.clear();
    widget.controller.window.updateSearch('');
    setState(() => _searchExpanded = false);
  }

  List<Widget> _filterSelects(BuildContext context, {required bool compact}) {
    final query = widget.controller.window.query;
    final kindOptions = <_HistoryFilterOption<HistoryClipKind?>>[
      _HistoryFilterOption(
        id: 'all',
        label: context.l10n.historyFilterAllClips,
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
    final originOptions = <_HistoryFilterOption<String?>>[
      _HistoryFilterOption(
        id: 'all',
        label: context.l10n.historyFilterAllDevices,
        value: null,
        icon: LucideIcons.monitorSmartphone,
      ),
      for (final facet in widget.controller.window.facets.originDevices)
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
      _HistoryFilterOption(
        id: 'all',
        label: context.l10n.historyFilterAllApps,
        value: null,
        icon: LucideIcons.appWindow,
      ),
      for (final facet in widget.controller.window.facets.sourceApps)
        _HistoryFilterOption(
          id: facet.id,
          label: facet.label,
          value: facet.id,
          contentBuilder: (compact) => Semantics(
            label: facet.label,
            child: SourceAppLabel(
              name: facet.label,
              icon: widget.controller.media.requestSourceIcon(facet.iconId),
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
      HistoryDateFilter(controller: widget.controller),
      HistoryTagsFilter(controller: widget.controller, compact: compact),
      _filterSelect(
        context,
        compact: compact,
        options: kindOptions,
        active: query.kind != null,
        value: query.kind,
        onChanged: (option) {
          final current = widget.controller.window.query;
          unawaited(
            widget.controller.window.updateQuery(
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
        options: originOptions,
        active: query.origin != null,
        value: query.origin,
        onChanged: (option) {
          final current = widget.controller.window.query;
          unawaited(
            widget.controller.window.updateQuery(
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
        active: query.sourceApp != null,
        value: query.sourceApp,
        onChanged: (option) {
          final current = widget.controller.window.query;
          unawaited(
            widget.controller.window.updateQuery(
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
        active: query.sort != HistorySort.newest,
        value: query.sort,
        onChanged: (option) {
          unawaited(
            widget.controller.window.updateQuery(
              widget.controller.window.query.copyWith(sort: option.value),
            ),
          );
        },
      ),
    ];
  }

  Widget _filterSelect<T>(
    BuildContext context, {
    required bool compact,
    bool active = false,
    required List<_HistoryFilterOption<T>> options,
    required T value,
    required ValueChanged<_HistoryFilterOption<T>> onChanged,
  }) => AppFilterIndicator(
    active: active,
    tooltip: _selectedOption(options, value).label,
    child: ComponentTheme<SelectTheme>(
      data: AppTheme.selectTheme(
        context,
        variant: compact
            ? AppSelectVariant.compactToolbar
            : AppSelectVariant.toolbar,
        labels: options.map((option) => option.label),
      ),
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
    final query = widget.controller.window.query;
    final labels = [
      HistoryTagsFilter.selectionLabel(widget.controller),
      query.kind?.label ?? context.l10n.historyFilterAllClips,
      _facetLabel(
        widget.controller.window.facets.originDevices,
        query.origin,
        context.l10n.historyFilterAllDevices,
      ),
      _facetLabel(
        widget.controller.window.facets.sourceApps,
        query.sourceApp,
        context.l10n.historyFilterAllApps,
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
        (_controlExtent(context) *
            (widget.controller.window.hasActiveFilters ? 4 : 3)) +
        (_toolbarGap *
            (_filterCount +
                2 +
                (widget.controller.window.hasActiveFilters ? 1 : 0)));
    for (final label in labels) {
      final painter = TextPainter(
        text: TextSpan(text: label, style: style),
        maxLines: 1,
        textDirection: Directionality.of(context),
        textScaler: textScaler,
      )..layout();
      width += painter.width.ceilToDouble() + filterChromeWidth;
      painter.dispose();
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
        child: AppIconFrame(
          size: AppIconSize.md,
          child: Icon(resolvedIcon, size: AppIconSize.md),
        ),
      );
    }
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        AppIconFrame(
          size: AppIconSize.sm,
          child: Icon(resolvedIcon, size: AppIconSize.sm),
        ),
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
    required this.onDragSelection,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final bool selected;
  final bool showKindLabel;
  final VoidCallback onPressed;
  final void Function(Offset, ScrollableState) onDragSelection;

  @override
  State<_HistoryClipCard> createState() => _HistoryClipCardState();
}

class _HistoryClipCardState extends State<_HistoryClipCard> {
  final _focus = FocusNode(skipTraversal: true);
  late final _buttonFocus = FocusNode(onKeyEvent: _handleKey);
  bool _hovered = false;
  bool _focused = false;
  bool _menuOpen = false;
  PointerDownEvent? _pointerDown;
  PointerDownEvent? _pressDown;
  PointerDownEvent? _lastClick;

  @override
  void dispose() {
    _focus.dispose();
    _buttonFocus.dispose();
    super.dispose();
  }

  bool get _actionsEnabled =>
      !widget.controller.bulk.isBulkSelecting &&
      !widget.controller.bulk.isBulkMutating;

  void _copy() => unawaited(
    copyHistoryClip(
      context,
      controller: widget.controller,
      clipId: widget.clip.id,
    ),
  );

  KeyEventResult _handleKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent ||
        !_actionsEnabled ||
        (event.logicalKey != LogicalKeyboardKey.enter &&
            event.logicalKey != LogicalKeyboardKey.numpadEnter)) {
      return KeyEventResult.ignored;
    }
    _copy();
    return KeyEventResult.handled;
  }

  /// A mouse double-click copies; on macOS Control-click opens the menu.
  void _handlePressed() {
    final down = _pressDown;
    final previous = _lastClick;
    _pressDown = null;
    _lastClick = null;
    if (down != null &&
        down.kind == PointerDeviceKind.mouse &&
        _actionsEnabled) {
      final keys = HardwareKeyboard.instance;
      if (Theme.of(context).platform == TargetPlatform.macOS &&
          keys.isControlPressed) {
        unawaited(_showMenu(context, position: down.position));
        return;
      }
      if (!keys.isShiftPressed &&
          !keys.isControlPressed &&
          !keys.isMetaPressed) {
        if (previous != null &&
            down.timeStamp - previous.timeStamp <= kDoubleTapTimeout &&
            (down.position - previous.position).distance <= kDoubleTapSlop) {
          _copy();
          return;
        }
        _lastClick = down;
      }
    }
    widget.onPressed();
  }

  Future<void> _showMenu(BuildContext anchor, {Offset? position}) async {
    setState(() => _menuOpen = true);
    try {
      await showDropdown<void>(
        context: anchor,
        position: position,
        alignment: position == null
            ? AlignmentDirectional.topEnd
            : AlignmentDirectional.topStart,
        anchorAlignment: position == null
            ? AlignmentDirectional.bottomEnd
            : null,
        builder: (popupContext) => AppTheme.popupSurface(
          popupContext,
          child: DropdownMenu(
            children: historyClipMenuItems(
              context,
              controller: widget.controller,
              clip: widget.clip,
              includeCopy: position != null,
            ),
          ),
        ),
      ).future;
    } finally {
      if (mounted) setState(() => _menuOpen = false);
    }
  }

  Widget _copyButton({required bool touch}) => Tooltip(
    tooltip: (context) =>
        TooltipContainer(child: Text(context.l10n.commonCopy)),
    child: Semantics(
      label: context.l10n.commonCopy,
      button: true,
      child: Button.secondary(
        key: ValueKey<String>('history-row-copy-${widget.clip.id}'),
        style: touch
            ? AppTheme.historyDragHandleStyle(dragging: false, touch: true)
            : const ButtonStyle.secondaryIcon(density: ButtonDensity.iconDense),
        onPressed: _actionsEnabled ? _copy : null,
        child: const AppIconFrame(child: Icon(LucideIcons.copy)),
      ),
    ),
  );

  Widget _moreButton() => Tooltip(
    tooltip: (context) =>
        TooltipContainer(child: Text(context.l10n.historyMoreActions)),
    child: Semantics(
      label: context.l10n.historyMoreActions,
      button: true,
      child: Builder(
        builder: (anchor) => Button.secondary(
          key: ValueKey<String>('history-row-more-${widget.clip.id}'),
          style: const ButtonStyle.secondaryIcon(
            density: ButtonDensity.iconDense,
          ),
          onPressed: _actionsEnabled
              ? () => unawaited(_showMenu(anchor))
              : null,
          child: const AppIconFrame(child: Icon(LucideIcons.ellipsis)),
        ),
      ),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final clip = widget.clip;
    final controller = widget.controller;
    final dragging = controller.pinnedOrder.draggedPinnedId == clip.id;
    final touch = Theme.of(context).platform == TargetPlatform.android;
    final showActions =
        !controller.bulk.isBulkSelecting && (_hovered || _focused || _menuOpen);
    final showTouchCopy = touch && !controller.bulk.isBulkSelecting;
    final showHandle =
        !controller.bulk.isBulkSelecting &&
        clip.pinned &&
        controller.window.hasUnfilteredQuery;
    final card = MouseRegion(
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
                  focusNode: _buttonFocus,
                  onTapDown: (_) => _pressDown = _pointerDown,
                  onTapCancel: () => _pressDown = null,
                  onPressed: controller.bulk.isBulkMutating
                      ? null
                      : _handlePressed,
                  onLongPressStart: controller.bulk.isBulkMutating
                      ? null
                      : (details) => widget.onDragSelection(
                          details.globalPosition,
                          Scrollable.of(context),
                        ),
                  alignment: Alignment.centerLeft,
                  style: AppTheme.historyClipButtonStyle(
                    selected: widget.selected,
                  ),
                  child: Row(
                    children: [
                      if (controller.bulk.isBulkSelecting) ...[
                        IgnorePointer(
                          child: ExcludeSemantics(
                            child: Checkbox(
                              state: widget.selected
                                  ? CheckboxState.checked
                                  : CheckboxState.unchecked,
                              enabled: !controller.bulk.isBulkMutating,
                              onChanged: (_) {},
                            ),
                          ),
                        ),
                        const Gap(AppSpacing.sm),
                      ],
                      Expanded(
                        child: Padding(
                          padding: EdgeInsets.only(
                            right: showTouchCopy
                                ? AppControlSize.touch +
                                      AppSpacing.sm +
                                      (showHandle
                                          ? AppControlSize.touch + AppSpacing.xs
                                          : AppSpacing.zero)
                                : AppSpacing.zero,
                          ),
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              ExcludeFocus(
                                excluding: controller.bulk.isBulkSelecting,
                                child: ExcludeSemantics(
                                  excluding: controller.bulk.isBulkSelecting,
                                  child: IgnorePointer(
                                    ignoring: controller.bulk.isBulkSelecting,
                                    child: _ClipContent(
                                      clip: clip,
                                      controller: controller,
                                    ),
                                  ),
                                ),
                              ),
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
                    ],
                  ),
                ),
              ),
              if (showActions || dragging || showTouchCopy)
                Positioned(
                  top: AppSpacing.zero,
                  bottom: AppSpacing.zero,
                  right: AppSpacing.sm,
                  child: Center(
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      spacing: AppSpacing.xs,
                      children: [
                        if (showActions || showTouchCopy)
                          _copyButton(touch: touch),
                        if (showActions) ...[
                          Tooltip(
                            tooltip: (context) => TooltipContainer(
                              child: Text(
                                clip.pinned
                                    ? context.l10n.historyUnpinClip
                                    : context.l10n.historyPinClip,
                              ),
                            ),
                            child: Semantics(
                              label: clip.pinned
                                  ? context.l10n.historyUnpinClip
                                  : context.l10n.historyPinClip,
                              toggled: clip.pinned,
                              button: true,
                              child: Button.secondary(
                                key: ValueKey<String>(
                                  'history-row-pin-${clip.id}',
                                ),
                                style: const ButtonStyle.secondaryIcon(
                                  density: ButtonDensity.iconDense,
                                ),
                                onPressed:
                                    controller.mutations.isPinPending(clip.id)
                                    ? null
                                    : () =>
                                          controller.mutations.togglePin(clip),
                                child: AppIconFrame(
                                  child: Icon(
                                    clip.pinned
                                        ? LucideIcons.pinOff
                                        : LucideIcons.pin,
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ],
                        if (showActions) _moreButton(),
                        if (showHandle)
                          Tooltip(
                            key: ValueKey<String>(
                              'history-reorder-action-${clip.id}',
                            ),
                            tooltip: (context) => TooltipContainer(
                              child: Text(
                                context.l10n.historyReorderPinnedClip,
                              ),
                            ),
                            child: CallbackShortcuts(
                              bindings: {
                                const SingleActivator(
                                  LogicalKeyboardKey.arrowUp,
                                  alt: true,
                                ): () => unawaited(
                                  controller.pinnedOrder.shiftPinned(
                                    clip.id,
                                    up: true,
                                  ),
                                ),
                                const SingleActivator(
                                  LogicalKeyboardKey.arrowDown,
                                  alt: true,
                                ): () => unawaited(
                                  controller.pinnedOrder.shiftPinned(
                                    clip.id,
                                    up: false,
                                  ),
                                ),
                              },
                              child: Semantics(
                                label: context.l10n.historyReorderPinnedClip,
                                hint: context.l10n.historyReorderPinnedHint,
                                onIncrease:
                                    controller.pinnedOrder.canReorderPinned
                                    ? () => unawaited(
                                        controller.pinnedOrder.shiftPinned(
                                          clip.id,
                                          up: false,
                                        ),
                                      )
                                    : null,
                                onDecrease:
                                    controller.pinnedOrder.canReorderPinned
                                    ? () => unawaited(
                                        controller.pinnedOrder.shiftPinned(
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
                                    onPointerCancel: (_) => controller
                                        .pinnedOrder
                                        .cancelPinnedDrag(),
                                    child: SortableDragHandle(
                                      enabled: controller
                                          .pinnedOrder
                                          .canReorderPinned,
                                      child: Button.secondary(
                                        key: ValueKey<String>(
                                          'history-row-reorder-${clip.id}',
                                        ),
                                        style: AppTheme.historyDragHandleStyle(
                                          dragging: dragging,
                                          touch: touch,
                                        ),
                                        onPressed:
                                            controller
                                                .pinnedOrder
                                                .canReorderPinned
                                            ? () {}
                                            : null,
                                        child: const AppIconFrame(
                                          child: Icon(LucideIcons.gripVertical),
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
    final menu = !touch && _actionsEnabled;
    // The context menu popup inherits the theme around it; the row keeps the
    // app theme.
    final rowTheme = Theme.of(context);
    return Theme(
      data: AppTheme.popupTheme(context),
      child: ContextMenu(
        enabled: menu,
        items: menu
            ? historyClipMenuItems(
                context,
                controller: controller,
                clip: clip,
                includeCopy: true,
              )
            : const [],
        child: Theme(
          data: rowTheme,
          child: Listener(
            onPointerDown: (event) => _pointerDown = event,
            child: card,
          ),
        ),
      ),
    );
  }
}

class _ClipContent extends StatefulWidget {
  const _ClipContent({
    required this.clip,
    required this.controller,
    this.revealed = false,
  });

  final HistoryClip clip;
  final HistoryController controller;
  final bool revealed;

  @override
  State<_ClipContent> createState() => _ClipContentState();
}

class _ClipContentState extends State<_ClipContent> {
  final _highlightMemo = HistoryCodeHighlightMemo();
  String? _highlightClipId;

  HistoryClip get clip => widget.clip;
  HistoryController get controller => widget.controller;
  bool get revealed => widget.revealed;

  @override
  void dispose() {
    _highlightMemo.clear();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (clip.id != _highlightClipId) {
      _highlightMemo.clear();
      _highlightClipId = clip.id;
    }
    if (clip.masked && !revealed) {
      _highlightMemo.clear();
      return FutureBuilder<HistoryClip>(
        future: controller.media.spoilerContent(clip.id),
        builder: (context, snapshot) => Align(
          alignment: Alignment.centerLeft,
          child: SecretSpoiler(
            key: ValueKey('history-spoiler-${clip.id}'),
            clipId: clip.id,
            controller: controller.spoilers,
            child: snapshot.hasError
                ? StateView.error(title: context.l10n.historyClipUnavailable)
                : _ClipContent(
                    clip: snapshot.data ?? clip,
                    controller: controller,
                    revealed: true,
                  ),
          ),
        ),
      );
    }
    final kind = clip.contentKind;
    if (kind != HistoryClipKind.code && kind != HistoryClipKind.json) {
      _highlightMemo.clear();
    }
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
              maxLines: HistoryClipPresentation.previewMaxLines,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      );
    }
    if (kind == HistoryClipKind.code || kind == HistoryClipKind.json) {
      final highlighted = _highlightMemo.highlight(
        clip.preview,
        Theme.of(context),
        json: kind == HistoryClipKind.json,
      );
      return Text.rich(
        highlighted.span,
        maxLines: HistoryClipPresentation.previewMaxLines,
        overflow: TextOverflow.ellipsis,
      );
    }
    if (kind != HistoryClipKind.image) {
      return Text(
        clip.preview.isEmpty ? clip.contentKind.label : clip.preview,
        style: kind == HistoryClipKind.path
            ? Theme.of(context).typography.mono
            : null,
        maxLines: HistoryClipPresentation.previewMaxLines,
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
    linePainter.dispose();
    return LayoutBuilder(
      builder: (context, constraints) {
        final pixelRatio = MediaQuery.devicePixelRatioOf(context);
        // Limit both dimensions in physical pixels before decoding the preview.
        final bounds = HistoryImagePreviewBounds(
          width: (constraints.maxWidth * pixelRatio).ceil().clamp(1, 2048),
          height: (maxHeight * pixelRatio).ceil().clamp(1, 2048),
        );
        return FutureBuilder<HistoryImagePreview?>(
          future: controller.media.requestImagePreview(clip.id, bounds: bounds),
          builder: (context, snapshot) {
            final preview = snapshot.data;
            if (preview == null) {
              final placeholder = const AppIconFrame(
                child: Icon(LucideIcons.image),
              );
              return revealed ? placeholder : Center(child: placeholder);
            }
            if (preview.width <= 0 || preview.height <= 0) {
              final placeholder = const AppIconFrame(
                child: Icon(LucideIcons.imageOff),
              );
              return revealed ? placeholder : Center(child: placeholder);
            }
            final scale = math.min(
              1.0,
              math.min(
                constraints.maxWidth / preview.width,
                maxHeight / preview.height,
              ),
            );
            final image = SizedBox(
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
                    (preview.width * scale * pixelRatio).ceil().clamp(1, 2048),
                  ),
                  cacheHeight: math.min(
                    preview.height,
                    (preview.height * scale * pixelRatio).ceil().clamp(1, 2048),
                  ),
                  errorBuilder: (context, error, stackTrace) => const Center(
                    child: AppIconFrame(child: Icon(LucideIcons.imageOff)),
                  ),
                ),
              ),
            );
            return revealed
                ? image
                : Align(alignment: Alignment.centerLeft, child: image);
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
    Widget icon(IconData data) => AppIconFrame(
      size: AppIconSize.xs,
      child: Icon(data, size: AppIconSize.xs, color: style.color),
    );
    final kindIcon = icon(HistoryClipPresentation.icon(clip.contentKind));
    final annotations = clip.annotations;
    final tags = [
      ...controller.tags.names(clip.tagIds),
      for (final name in controller.tags.names(annotations.ruleTagIds))
        context.l10n.historyRuleTag(name),
    ];
    final provenance = HistoryClipPresentation.provenance(
      context.l10n,
      annotations.provenance,
    );
    final expiresAt = annotations.keep ? null : annotations.expiresAt;
    final cutLines = HistoryClipPresentation.cutLineCount(clip);
    return Text.rich(
      TextSpan(
        children: [
          WidgetSpan(
            alignment: PlaceholderAlignment.middle,
            child: showKindLabel
                ? kindIcon
                : Semantics(
                    label: context.l10n.historyClipKindSemantics(
                      clip.contentKind.label,
                    ),
                    child: kindIcon,
                  ),
          ),
          if (showKindLabel) TextSpan(text: ' ${clip.contentKind.label}'),
          if (expiresAt != null) ...[
            const TextSpan(text: ' • '),
            WidgetSpan(
              alignment: PlaceholderAlignment.middle,
              child: HistoryExpiryText(
                expiresAt: expiresAt,
                style: style.copyWith(
                  color: Theme.of(context).colorScheme.destructive,
                ),
                strutStyle: strutStyle,
              ),
            ),
          ],
          if (clip.sourceApp != null) ...[
            const TextSpan(text: ' • '),
            WidgetSpan(
              alignment: PlaceholderAlignment.middle,
              child: SourceAppLabel(
                name: clip.sourceApp!,
                icon: controller.media.requestSourceIcon(clip.sourceAppIconId),
                iconSize: AppIconSize.xs,
                style: style,
                strutStyle: strutStyle,
              ),
            ),
          ],
          if (provenance != null)
            TextSpan(
              text: clip.sourceApp == null
                  ? ' • $provenance'
                  : ' ($provenance)',
            ),
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
          if (tags.isNotEmpty) ...[
            const TextSpan(text: ' • '),
            WidgetSpan(
              alignment: PlaceholderAlignment.middle,
              child: Semantics(
                label: context.l10n.historyTags,
                child: icon(LucideIcons.tag),
              ),
            ),
            TextSpan(text: ' ${tags.join(', ')}'),
          ],
          if (cutLines != null)
            // Last, so it yields first; the preview is already read in full.
            TextSpan(
              text: ' • ${context.l10n.historyClipLineCount(cutLines)}',
              semanticsLabel: '',
            ),
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
