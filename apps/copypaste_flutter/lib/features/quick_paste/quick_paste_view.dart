import '../transfers/file_operations_view.dart';

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../history/presentation/secret_spoiler.dart';

import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_motion.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_toast.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/state_view.dart';
import '../../platform/desktop/desktop_shortcuts.dart';
import '../history/controller/history_controller.dart';
import '../history/models/history_models.dart';
import '../history/view/history_inspector.dart';
import '../history/presentation/history_clip_presentation.dart';
import '../history/presentation/source_app_label.dart';
import 'quick_paste_controller.dart';
import 'middle_ellipsis.dart';
import 'quick_paste_row_geometry.dart';

HistoryImagePreviewBounds _quickPastePreviewBounds(BuildContext context) {
  final ratio = MediaQuery.devicePixelRatioOf(context);
  return HistoryImagePreviewBounds(
    width: (240 * ratio).ceil().clamp(1, 480),
    height: (120 * ratio).ceil().clamp(1, 480),
  );
}

class QuickPasteView extends StatefulWidget {
  const QuickPasteView({super.key, required this.controller});

  final QuickPasteController controller;

  @override
  State<QuickPasteView> createState() => _QuickPasteViewState();
}

class _QuickPasteViewState extends State<QuickPasteView> {
  final _search = TextEditingController();
  final _searchFocus = FocusNode();
  final _scroll = ScrollController();
  final _pinnedScroll = ScrollController();
  final _rowKeys = <String, GlobalKey>{};
  Future<void> _focusTraversal = Future<void>.value();
  bool _paginationCheckScheduled = false;
  bool _pinnedIsActive = false;
  int _generation = 0;
  int _operationErrorRevision = 0;
  HistoryQuery _query = const HistoryQuery();
  int? _preparingFrame;
  int? _presentedFrame;
  _QuickPastePaneAnchor? _recentAnchor;
  _QuickPastePaneAnchor? _pinnedAnchor;
  bool _restoringWindowAnchors = false;

  void _preparePresentationFrame() {
    final id = controller.renderablePresentationId;
    if (id == null || id == _preparingFrame || id == _presentedFrame) return;
    _preparingFrame = id;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      var attempted = false;
      try {
        if (!mounted || controller.renderablePresentationId != id) return;
        final media = <Future<void>>[];
        for (final clip in controller.items) {
          final row = _rowKeys[clip.id]?.currentContext?.findRenderObject();
          if (row is! RenderBox || !row.attached || !row.hasSize) continue;
          final viewportObject = RenderAbstractViewport.maybeOf(row);
          if (viewportObject is! RenderBox) continue;
          final viewport = viewportObject as RenderBox;
          if (!viewport.hasSize) continue;
          final rowRect = row.localToGlobal(Offset.zero) & row.size;
          final viewportRect =
              viewport.localToGlobal(Offset.zero) & viewport.size;
          if (rowRect.overlaps(viewportRect)) {
            media.add(_prepareClipMedia(clip));
          }
        }
        await Future.wait(media);
        if (!mounted || controller.renderablePresentationId != id) return;
        await WidgetsBinding.instance.endOfFrame;
        if (!mounted || controller.renderablePresentationId != id) return;
        attempted = true;
        if (await controller.presentFrame(id)) _presentedFrame = id;
      } finally {
        if (_preparingFrame == id) _preparingFrame = null;
        if (!attempted && mounted) _preparePresentationFrame();
      }
    });
  }

  Future<void> _prepareClipMedia(HistoryClip clip) async {
    try {
      if (clip.secret) clip = await controller.history.spoilerContent(clip.id);
      final loads = <Future<void>>[];
      if (clip.contentKind == HistoryClipKind.image) {
        loads.add(() async {
          final bounds = _quickPastePreviewBounds(context);
          final preview = await controller.history.requestImagePreview(
            clip.id,
            maxEdge: 480,
            bounds: bounds,
          );
          if (mounted && preview != null) {
            await precacheImage(
              ResizeImage(
                MemoryImage(preview.bytes),
                width: bounds.width,
                height: bounds.height,
              ),
              context,
              onError: (_, _) {},
            );
          }
        }());
      }
      if (clip.sourceAppIconId != null) {
        loads.add(() async {
          final icon = await controller.history.requestSourceIcon(
            clip.sourceAppIconId,
          );
          if (mounted && icon != null) {
            await precacheImage(
              MemoryImage(icon.bytes),
              context,
              onError: (_, _) {},
            );
          }
        }());
      }
      await Future.wait(loads);
    } catch (_) {
      // A missing preview uses the shared row fallback and never prevents opening.
    }
  }

  QuickPasteController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _generation = controller.presentationGeneration;
    _operationErrorRevision = controller.history.operationErrorRevision;
    _query = controller.history.query;
    controller.addListener(_presentationChanged);
    _scroll.addListener(_recentScrolled);
    _pinnedScroll.addListener(_pinnedScrolled);
    controller.history.beforeWindowChange = _captureWindowAnchors;
    controller.history.afterWindowChange = _restoreWindowAnchors;
  }

  void _presentationChanged() {
    final revision = controller.history.operationErrorRevision;
    if (_operationErrorRevision != revision) {
      _operationErrorRevision = revision;
      final message = controller.history.operationError;
      final presentation = controller.presentationGeneration;
      if (message != null) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted && controller.presentationGeneration == presentation) {
            AppToast.show(
              context,
              title: 'Action failed',
              message: message,
              tone: AppToastTone.error,
            );
          }
        });
      }
    }
    final presentationChanged =
        _generation != controller.presentationGeneration;
    final queryChanged = !identical(_query, controller.history.query);
    if (!presentationChanged && !queryChanged) return;
    _generation = controller.presentationGeneration;
    _query = controller.history.query;
    if (presentationChanged) {
      _search.clear();
      _searchFocus.requestFocus();
    }
    if (_scroll.hasClients) _scroll.jumpTo(0);
    if (_pinnedScroll.hasClients) _pinnedScroll.jumpTo(0);
  }

  Future<Set<String>> _captureWindowAnchors() async {
    _recentAnchor = _capturePaneAnchor(_scroll, pinned: false);
    _pinnedAnchor = _capturePaneAnchor(_pinnedScroll, pinned: true);
    return {?_recentAnchor?.id, ?_pinnedAnchor?.id};
  }

  _QuickPastePaneAnchor? _capturePaneAnchor(
    ScrollController scroll, {
    required bool pinned,
  }) {
    if (!scroll.hasClients) return null;
    final viewport = _viewportRect(scroll);
    if (viewport == null) return null;
    final ids = pinned
        ? controller.history.visitedPinnedIds
        : controller.history.visitedRecentIds;
    _QuickPastePaneAnchor? result;
    for (final (index, id) in ids.indexed) {
      final render = _rowKeys[id]?.currentContext?.findRenderObject();
      if (render is! RenderBox || !render.attached || !render.hasSize) {
        continue;
      }
      final rect = render.localToGlobal(Offset.zero) & render.size;
      if (!rect.overlaps(viewport)) continue;
      final dy = rect.top - viewport.top;
      if (result == null || dy < result.viewportDy) {
        result = _QuickPastePaneAnchor(id: id, index: index, viewportDy: dy);
      }
    }
    return result;
  }

  Rect? _viewportRect(ScrollController scroll) {
    if (!scroll.hasClients) return null;
    final render = scroll.position.context.storageContext.findRenderObject();
    if (render is! RenderBox || !render.attached || !render.hasSize) {
      return null;
    }
    return render.localToGlobal(Offset.zero) & render.size;
  }

  void _restoreWindowAnchors() {
    if (_restoringWindowAnchors) return;
    _restoringWindowAnchors = true;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      try {
        await _restorePaneAnchor(_scroll, _recentAnchor, pinned: false);
        await _restorePaneAnchor(_pinnedScroll, _pinnedAnchor, pinned: true);
      } finally {
        _recentAnchor = null;
        _pinnedAnchor = null;
        _restoringWindowAnchors = false;
      }
    });
  }

  Future<void> _restorePaneAnchor(
    ScrollController scroll,
    _QuickPastePaneAnchor? anchor, {
    required bool pinned,
  }) async {
    if (!mounted || anchor == null || !scroll.hasClients) return;
    final ids = pinned
        ? controller.history.visitedPinnedIds
        : controller.history.visitedRecentIds;
    if (ids.isEmpty) return;
    final id = ids.contains(anchor.id)
        ? anchor.id
        : ids[anchor.index.clamp(0, ids.length - 1)];
    if (_rowKeys[id]?.currentContext == null) {
      await controller.history.materializeId(id);
      if (!mounted || !scroll.hasClients) return;
      await WidgetsBinding.instance.endOfFrame;
    }
    final viewport = _viewportRect(scroll);
    final render = _rowKeys[id]?.currentContext?.findRenderObject();
    if (viewport == null ||
        render is! RenderBox ||
        !render.attached ||
        !render.hasSize) {
      return;
    }
    final row = render.localToGlobal(Offset.zero) & render.size;
    final target = (scroll.offset + row.top - viewport.top - anchor.viewportDy)
        .clamp(scroll.position.minScrollExtent, scroll.position.maxScrollExtent)
        .toDouble();
    if ((scroll.offset - target).abs() > 0.5) scroll.jumpTo(target);
  }

  @override
  void dispose() {
    controller.removeListener(_presentationChanged);
    controller.history.beforeWindowChange = null;
    controller.history.afterWindowChange = null;
    _search.dispose();
    _searchFocus.dispose();
    _scroll.dispose();
    _pinnedScroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, child) => CallbackShortcuts(
        bindings: _shortcutBindings(),
        child: ClipRRect(
          borderRadius: const BorderRadius.all(Radius.circular(AppRadius.lg)),
          child: Scaffold(
            backgroundColor: Theme.of(context).colorScheme.popover,
            child: Padding(
              padding: const EdgeInsets.all(AppSpacing.sm),
              child: LayoutBuilder(
                builder: (context, constraints) {
                  _preparePresentationFrame();
                  final menu = _menu(context);
                  final menuWidth =
                      (AppLayoutSize.quickPasteMenuWidth - AppSpacing.sm * 2)
                          .clamp(
                            0.0,
                            constraints.maxWidth *
                                (controller.inspectorOpen ? 0.55 : 1),
                          )
                          .toDouble();
                  return Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      SizedBox(width: menuWidth, child: menu),
                      if (controller.inspectorOpen) ...[
                        const Gap(AppSpacing.sm),
                        Expanded(
                          child: AppMotion.inspectorEntrance(
                            context,
                            child: HistoryInspector(
                              controller: controller.history,
                              presentationRevision:
                                  controller.presentationGeneration,
                              inDrawer: false,
                              compact: true,
                              onClose: () =>
                                  unawaited(controller.toggleInspector()),
                            ),
                          ),
                        ),
                      ],
                    ],
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _menu(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Row(
        key: const ValueKey('quick-paste-header'),
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Button.ghost(
            key: const ValueKey('quick-paste-home'),
            style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
            onPressed: controller.openMainWindow,
            child: const Image(
              key: ValueKey('quick-paste-logo'),
              image: AssetImage('assets/brand/copypaste.png'),
              width: AppIconSize.sm,
              height: AppIconSize.sm,
              semanticLabel: 'Open CopyPaste',
            ),
          ),
          const Gap(AppSpacing.xs),
          Expanded(
            child: Theme(
              data: AppTheme.clipboardSearchTheme(context),
              child: SizedBox(
                height: AppTheme.controlHeight(context),
                child: TextField(
                  theme: AppTheme.clipboardSearchFieldTheme,
                  textAlignVertical: TextAlignVertical.center,
                  key: const ValueKey('quick-paste-search'),
                  controller: _search,
                  focusNode: _searchFocus,
                  autofocus: true,
                  style: AppTheme.clipboardMenuTextStyle(context),
                  placeholder: Text(
                    'Type to search…',
                    style: AppTheme.clipboardMenuTextStyle(context),
                  ),
                  features: const [
                    InputFeature.leading(
                      Icon(LucideIcons.search, size: AppIconSize.sm),
                    ),
                  ],
                  onChanged: (value) => unawaited(controller.search(value)),
                ),
              ),
            ),
          ),
          const Gap(AppSpacing.xs),
          if (controller.history.fileOperations case final transfers?) ...[
            FileOperationsHeaderAction(controller: transfers),
            const Gap(AppSpacing.xs),
          ],
          Button.ghost(
            style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
            key: const ValueKey('quick-paste-inspector-toggle'),
            onPressed: controller.toggleInspector,
            child: const Icon(LucideIcons.panelsLeftBottom),
          ),
        ],
      ),
      const Gap(AppSpacing.xs),
      Expanded(child: _history(context)),
      const Gap(AppSpacing.xs),
      const Divider(),
      const Gap(AppSpacing.xs),
      _Footer(controller: controller),
    ],
  );

  void _recentScrolled() {
    _pinnedIsActive = false;
    _scrolled();
  }

  void _pinnedScrolled() {
    _pinnedIsActive = true;
    _scrolled();
  }

  void _scrolled() {
    final clip = controller.focusedClip;
    if (clip != null) controller.hoverClip(clip, hovered: false);
    _loadMoreWhenNeeded();
  }

  void _loadMoreWhenNeeded() {
    final history = controller.history;
    if (history.isLoadingMore || history.state != HistoryLoadState.ready) {
      return;
    }
    if (_pinnedIsActive &&
        _pinnedScroll.hasClients &&
        _pinnedScroll.position.hasContentDimensions) {
      final position = _pinnedScroll.position;
      if (position.extentBefore <= 1 && history.canLoadPinnedPrevious) {
        unawaited(history.loadPinnedPrevious());
        return;
      }
      if (position.extentAfter <= position.viewportDimension &&
          history.canLoadPinnedMore) {
        unawaited(history.loadPinnedMore());
        return;
      }
    }
    if (!_pinnedIsActive &&
        _scroll.hasClients &&
        _scroll.position.hasContentDimensions &&
        _scroll.position.extentBefore <= 1 &&
        history.canLoadPrevious) {
      unawaited(history.loadPrevious());
      return;
    }
    if (!history.canLoadMore || controller.paginationFailed) return;
    final scroll = history.items.any((clip) => !clip.pinned)
        ? _scroll
        : _pinnedScroll;
    if (!scroll.hasClients || !scroll.position.hasContentDimensions) return;
    final position = scroll.position;
    if (position.viewportDimension > 0 &&
        position.extentAfter <= position.viewportDimension) {
      unawaited(controller.loadMore());
    }
  }

  void _checkPaginationAfterLayout() {
    if (_paginationCheckScheduled) return;
    _paginationCheckScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _paginationCheckScheduled = false;
      if (mounted) _loadMoreWhenNeeded();
    });
  }

  Widget _history(BuildContext context) {
    final history = controller.history;
    if (history.state == HistoryLoadState.loading && controller.items.isEmpty) {
      return const StateView.loading(message: 'Loading clipboard history.');
    }
    if (history.state == HistoryLoadState.error) {
      return StateView.error(
        title: 'History is unavailable',
        message: history.errorMessage,
      );
    }
    final items = controller.items;
    final recentIds = history.visitedRecentIds;
    final pinnedIds = history.visitedPinnedIds;
    if (recentIds.isEmpty && pinnedIds.isEmpty) {
      return const StateView.empty(title: 'No clips found');
    }
    final retainedIds = {...recentIds, ...pinnedIds};
    _rowKeys.removeWhere((id, _) => !retainedIds.contains(id));
    final resident = {for (final clip in items) clip.id: clip};
    _checkPaginationAfterLayout();
    return LayoutBuilder(
      builder: (context, constraints) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: Scrollbar(
              controller: _scroll,
              child: _clipList(
                recentIds,
                resident,
                scroll: _scroll,
                key: 'quick-paste-history-scroll',
                showPagination: recentIds.isNotEmpty,
              ),
            ),
          ),
          if (pinnedIds.isNotEmpty) ...[
            if (recentIds.isNotEmpty) ...[
              const Gap(AppSpacing.xs),
              const Divider(),
              const Gap(AppSpacing.xs),
            ],
            ConstrainedBox(
              constraints: BoxConstraints(
                maxHeight: recentIds.isEmpty
                    ? constraints.maxHeight
                    : constraints.maxHeight / 2,
              ),
              child: _clipList(
                pinnedIds,
                resident,
                scroll: _pinnedScroll,
                key: 'quick-paste-pinned-scroll',
                shrinkWrap: true,
                showPagination: recentIds.isEmpty,
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _clipList(
    List<String> ids,
    Map<String, HistoryClip> resident, {
    required ScrollController scroll,
    required String key,
    bool shrinkWrap = false,
    required bool showPagination,
  }) {
    final history = controller.history;
    final pinned = identical(scroll, _pinnedScroll);
    final indices = {
      for (final (index, id) in ids.indexed)
        ValueKey('quick-paste-row-$id'): index,
    };
    return ListView.builder(
      key: ValueKey(key),
      controller: scroll,
      primary: false,
      padding: EdgeInsets.zero,
      shrinkWrap: shrinkWrap,
      findChildIndexCallback: (key) => indices[key],
      itemCount: ids.length + (showPagination && history.canLoadMore ? 1 : 0),
      itemBuilder: (context, index) {
        if (index >= ids.length) {
          if (history.isLoadingMore) {
            return const Padding(
              padding: EdgeInsets.all(AppSpacing.sm),
              child: Center(child: CircularProgressIndicator()),
            );
          }
          return Button.ghost(
            key: const ValueKey('quick-paste-load-more'),
            style: AppTheme.clipboardMenuButtonStyle(),
            onPressed: controller.loadMore,
            child: Text(
              controller.paginationFailed ? 'Try again' : 'Load more',
            ),
          );
        }
        final id = ids[index];
        final clip = resident[id];
        if (clip == null) {
          return KeyedSubtree(
            key: ValueKey('quick-paste-row-$id'),
            child: _QuickPasteRowPlaceholder(
              id: id,
              extent: history.quickPasteRowExtent(id, pinned: pinned),
              onVisible: () => unawaited(history.materializeId(id)),
            ),
          );
        }
        return KeyedSubtree(
          key: ValueKey('quick-paste-row-$id'),
          child: QuickPasteRowGeometry(
            onExtent: (extent) => history.reportQuickPasteRowExtent(
              id,
              pinned: pinned,
              extent: extent,
            ),
            child: _item(clip),
          ),
        );
      },
    );
  }

  void _moveFocus(int offset) {
    final generation = controller.presentationGeneration;
    final query = controller.history.query;
    _focusTraversal = _focusTraversal.then((_) async {
      if (!mounted ||
          generation != controller.presentationGeneration ||
          !identical(query, controller.history.query)) {
        return;
      }
      await controller.moveFocus(offset);
      if (!mounted || generation != controller.presentationGeneration) return;
      final clip = controller.focusedClip;
      if (clip != null) await _revealFocused(clip, generation, query);
    });
  }

  Future<void> _revealFocused(
    HistoryClip clip,
    int generation,
    HistoryQuery query,
  ) async {
    await WidgetsBinding.instance.endOfFrame;
    final scroll = clip.pinned ? _pinnedScroll : _scroll;
    while (mounted &&
        generation == controller.presentationGeneration &&
        identical(query, controller.history.query) &&
        controller.focusedClip?.id == clip.id &&
        scroll.hasClients) {
      final rowContext = _rowKeys[clip.id]?.currentContext;
      if (rowContext != null && rowContext.mounted) {
        await Scrollable.ensureVisible(rowContext);
        return;
      }
      final clips = clip.pinned
          ? controller.history.visitedPinnedIds
          : controller.history.visitedRecentIds;
      final target = clips.indexOf(clip.id);
      final firstMounted = clips.indexWhere(
        (id) => _rowKeys[id]?.currentContext != null,
      );
      if (target < 0 || firstMounted < 0) return;
      final position = scroll.position;
      final direction = target < firstMounted ? -1 : 1;
      final next = (position.pixels + position.viewportDimension * direction)
          .clamp(position.minScrollExtent, position.maxScrollExtent);
      if (next == position.pixels) return;
      scroll.jumpTo(next.toDouble());
      await WidgetsBinding.instance.endOfFrame;
    }
  }

  Map<ShortcutActivator, VoidCallback> _shortcutBindings() {
    final bindings = <ShortcutActivator, VoidCallback>{
      ...desktopWindowBindings(
        onClose: controller.close,
        onQuit: controller.quit,
      ),
      desktopShortcut(LogicalKeyboardKey.keyF): _find,
      desktopShortcut(LogicalKeyboardKey.keyR): () =>
          unawaited(controller.history.reload()),
      if (defaultTargetPlatform != TargetPlatform.macOS) ...{
        const SingleActivator(
          LogicalKeyboardKey.f4,
          alt: true,
          includeRepeats: false,
        ): () =>
            unawaited(controller.close()),
        const SingleActivator(
          LogicalKeyboardKey.f5,
          includeRepeats: false,
        ): () =>
            unawaited(controller.history.reload()),
      },
      const SingleActivator(LogicalKeyboardKey.escape): () =>
          unawaited(controller.close()),
      const SingleActivator(LogicalKeyboardKey.arrowDown): () => _moveFocus(1),
      const SingleActivator(LogicalKeyboardKey.arrowUp): () => _moveFocus(-1),
      const SingleActivator(LogicalKeyboardKey.pageDown): () => _moveFocus(5),
      const SingleActivator(LogicalKeyboardKey.pageUp): () => _moveFocus(-5),
      const SingleActivator(LogicalKeyboardKey.enter): () =>
          unawaited(controller.activateFocused()),
      const SingleActivator(LogicalKeyboardKey.enter, shift: true): () =>
          unawaited(controller.activateFocused(plainText: true)),
      const SingleActivator(LogicalKeyboardKey.enter, alt: true): () =>
          unawaited(controller.activateFocused(invertAutoPaste: true)),
      const SingleActivator(
        LogicalKeyboardKey.enter,
        alt: true,
        shift: true,
      ): () => unawaited(
        controller.activateFocused(plainText: true, forcePaste: true),
      ),
      const SingleActivator(LogicalKeyboardKey.keyP, alt: true): () =>
          unawaited(controller.toggleFocusedPin()),
      const SingleActivator(LogicalKeyboardKey.backspace, alt: true): () =>
          unawaited(controller.deleteFocused()),
      const SingleActivator(LogicalKeyboardKey.delete, alt: true): () =>
          unawaited(controller.deleteFocused()),
      desktopShortcut(LogicalKeyboardKey.comma): () =>
          unawaited(controller.openSettings()),
    };
    for (final entry in controller.shortcuts.entries) {
      bindings[desktopShortcut(entry.key)] = () =>
          unawaited(controller.activate(entry.value));
    }
    return bindings;
  }

  void _find() {
    _searchFocus.requestFocus();
    _search.selection = TextSelection(
      baseOffset: 0,
      extentOffset: _search.text.length,
    );
  }

  Widget _item(HistoryClip clip) {
    final shortcut = controller.shortcutFor(clip);
    return KeyedSubtree(
      key: ValueKey('quick-paste-${clip.id}'),
      child: Builder(
        key: _rowKeys.putIfAbsent(clip.id, GlobalKey.new),
        builder: (context) => MouseRegion(
          onEnter: (_) => controller.hoverClip(clip, hovered: true),
          onExit: (_) => controller.hoverClip(clip, hovered: false),
          child: Button.ghost(
            style: AppTheme.clipboardMenuButtonStyle(
              selected: controller.focusedClip?.id == clip.id,
            ),
            alignment: Alignment.centerLeft,
            onFocus: (focused) {
              if (focused) controller.focus(clip);
            },
            onPressed: controller.activating
                ? null
                : () {
                    final keyboard = HardwareKeyboard.instance;
                    unawaited(
                      controller.activate(
                        clip,
                        plainText: keyboard.isShiftPressed,
                        invertAutoPaste:
                            keyboard.isAltPressed && !keyboard.isShiftPressed,
                        forcePaste:
                            keyboard.isAltPressed && keyboard.isShiftPressed,
                      ),
                    );
                  },
            child: Row(
              children: [
                SizedBox(
                  width: AppIconSize.sm,
                  child: clip.sourceApp != null
                      ? SourceAppLabel(
                          name: clip.sourceApp!,
                          icon: controller.history.requestSourceIcon(
                            clip.sourceAppIconId,
                          ),
                          showName: false,
                        )
                      : Icon(
                          HistoryClipPresentation.icon(clip.contentKind),
                          size: AppIconSize.sm,
                        ),
                ),
                const Gap(AppSpacing.sm),
                Expanded(
                  child: _ClipContent(clip: clip, controller: controller),
                ),
                const Gap(AppSpacing.sm),
                ListenableBuilder(
                  listenable: controller.history.downloadProgressChanges,
                  builder: (context, _) {
                    final progress = controller.transferProgressFor(clip.id);
                    final status = progress != null
                        ? SizedBox(
                            width: AppLayoutSize.quickPasteTransferStatusWidth,
                            child: Row(
                              children: [
                                CircularProgressIndicator(
                                  value: progress,
                                  theme: const CircularProgressIndicatorTheme(
                                    size: AppIconSize.xs,
                                  ),
                                  animated: !MediaQuery.disableAnimationsOf(
                                    context,
                                  ),
                                ),
                                const Gap(AppSpacing.xs),
                                Text('${(progress * 100).floor()}%'),
                              ],
                            ),
                          )
                        : controller.transferFailedFor(clip.id)
                        ? Text(
                            'Failed',
                            style: TextStyle(
                              color: Theme.of(context).colorScheme.destructive,
                            ),
                          )
                        : const SizedBox.shrink();
                    return shortcut != null &&
                            (progress != null ||
                                controller.transferFailedFor(clip.id))
                        ? Padding(
                            padding: const EdgeInsets.only(
                              right: AppSpacing.sm,
                            ),
                            child: status,
                          )
                        : status;
                  },
                ),
                if (shortcut != null)
                  MenuShortcut(
                    activator: desktopShortcut(shortcut),
                    combiner: const SizedBox.shrink(),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _QuickPastePaneAnchor {
  const _QuickPastePaneAnchor({
    required this.id,
    required this.index,
    required this.viewportDy,
  });

  final String id;
  final int index;
  final double viewportDy;
}

class _QuickPasteRowPlaceholder extends StatefulWidget {
  const _QuickPasteRowPlaceholder({
    required this.id,
    required this.extent,
    required this.onVisible,
  });

  final String id;
  final double? extent;
  final VoidCallback onVisible;

  @override
  State<_QuickPasteRowPlaceholder> createState() =>
      _QuickPasteRowPlaceholderState();
}

class _QuickPasteRowPlaceholderState extends State<_QuickPasteRowPlaceholder> {
  bool _requested = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_requested) return;
    _requested = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) widget.onVisible();
    });
  }

  @override
  Widget build(BuildContext context) =>
      SizedBox(height: widget.extent ?? AppControlSize.compact);
}

class _ClipContent extends StatelessWidget {
  const _ClipContent({
    required this.clip,
    required this.controller,
    this.revealed = false,
  });

  final HistoryClip clip;
  final QuickPasteController controller;
  final bool revealed;

  @override
  Widget build(BuildContext context) {
    if (clip.secret && !revealed) {
      return FutureBuilder<HistoryClip>(
        future: controller.history.spoilerContent(clip.id),
        builder: (context, snapshot) => Align(
          alignment: Alignment.centerLeft,
          child: SecretSpoiler(
            clipId: clip.id,
            controller: controller.history.spoilers,
            key: ValueKey(
              'quick-spoiler-${clip.id}-${controller.presentationGeneration}',
            ),
            child: snapshot.hasError
                ? const StateView.error(title: 'Clip unavailable')
                : _ClipContent(
                    clip: snapshot.data ?? clip,
                    controller: controller,
                    revealed: true,
                  ),
          ),
        ),
      );
    }
    if (clip.contentKind == HistoryClipKind.file) {
      final file = clip.file;
      final reference = file?.sourceReference;
      final path = reference?.trim().isNotEmpty == true
          ? reference!
          : clip.preview.contains(RegExp(r'[/\\]'))
          ? clip.preview
          : '';
      final filename = file?.name?.trim().isNotEmpty == true
          ? file!.name!.trim()
          : (path.isEmpty ? clip.preview : path).split(RegExp(r'[/\\]')).last;
      final theme = Theme.of(context);
      final pathStyle = theme.typography.xSmall.copyWith(
        color: theme.colorScheme.mutedForeground,
      );
      return Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            filename.isEmpty ? 'File' : filename,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          LayoutBuilder(
            builder: (context, constraints) => Text(
              path.isEmpty
                  ? 'Path unavailable'
                  : middleEllipsis(
                      path,
                      maxWidth: constraints.maxWidth,
                      style: pathStyle,
                      direction: Directionality.of(context),
                      textScaler: MediaQuery.textScalerOf(context),
                    ),
              key: ValueKey('quick-paste-file-path-${clip.id}'),
              maxLines: 1,
              softWrap: false,
              style: pathStyle,
            ),
          ),
        ],
      );
    }
    if (clip.contentKind != HistoryClipKind.image) {
      return Text(
        clip.preview.isEmpty ? clip.contentKind.label : clip.preview,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      );
    }
    final bounds = _quickPastePreviewBounds(context);
    return FutureBuilder<HistoryImagePreview?>(
      future: controller.history.requestImagePreview(
        clip.id,
        maxEdge: 480,
        bounds: bounds,
      ),
      builder: (context, snapshot) {
        final preview = snapshot.data;
        final image = ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 240, maxHeight: 120),
          child: preview == null
              ? const SizedBox(height: 120, child: Icon(LucideIcons.image))
              : ClipRRect(
                  borderRadius: const BorderRadius.all(
                    Radius.circular(AppRadius.sm),
                  ),
                  child: AspectRatio(
                    aspectRatio: preview.width / preview.height,
                    child: Image.memory(
                      preview.bytes,
                      fit: BoxFit.contain,
                      cacheWidth: bounds.width,
                      cacheHeight: bounds.height,
                      errorBuilder: (context, error, stackTrace) =>
                          const Icon(LucideIcons.imageOff),
                    ),
                  ),
                ),
        );
        return revealed
            ? image
            : Align(alignment: Alignment.centerLeft, child: image);
      },
    );
  }
}

class _Footer extends StatelessWidget {
  const _Footer({required this.controller});

  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    final style = AppTheme.clipboardMenuButtonStyle();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Button.ghost(
          style: style,
          alignment: Alignment.centerLeft,
          onPressed: () => confirmClear(context, controller),
          child: const Text('Clear'),
        ),
        Button.ghost(
          style: style,
          alignment: Alignment.centerLeft,
          onPressed: controller.openSettings,
          child: Row(
            children: [
              const Expanded(child: Text('Preferences…')),
              MenuShortcut(
                activator: desktopShortcut(LogicalKeyboardKey.comma),
                combiner: const SizedBox.shrink(),
              ),
            ],
          ),
        ),
        Button.ghost(
          style: style,
          alignment: Alignment.centerLeft,
          onPressed: () => showAbout(context),
          child: const Text('About'),
        ),
        Button.ghost(
          style: style,
          alignment: Alignment.centerLeft,
          onPressed: controller.quit,
          child: Row(
            children: [
              const Expanded(child: Text('Quit')),
              MenuShortcut(
                activator: desktopShortcut(LogicalKeyboardKey.keyQ),
                combiner: const SizedBox.shrink(),
              ),
            ],
          ),
        ),
      ],
    );
  }

  static Future<void> confirmClear(
    BuildContext context,
    QuickPasteController controller,
  ) async {
    await AppOverlays.showActionDialog(
      context,
      icon: LucideIcons.trash2,
      title: const Text('Clear history?'),
      content: const Text('Pinned clips will stay in your history.'),
      actionLabel: 'Clear',
      onConfirm: controller.clearUnpinned,
      failureMessage: () =>
          controller.history.errorMessage ?? 'History could not be deleted.',
    );
  }

  static Future<void> showAbout(BuildContext context) {
    return AppOverlays.showDialog<void>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.info,
        title: const Text('CopyPaste'),
        content: const Text(
          'Private clipboard history across macOS, Android, Windows, and Linux.',
        ),
        actions: [
          Button.primary(
            style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }
}
