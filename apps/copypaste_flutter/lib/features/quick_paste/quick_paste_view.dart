import 'dart:math' as math;

import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import '../transfers/file_operations_view.dart';

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'package:copypaste_flutter/l10n/l10n.dart';

import '../history/presentation/secret_spoiler.dart';

import '../../app/theme/app_brand_logo.dart';
import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_motion.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_toast.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/state_view.dart';
import '../../platform/desktop/desktop_shortcuts.dart';
import '../history/controller/history_window_controller.dart';
import '../history/models/history_models.dart';
import '../history/view/history_inspector.dart';
import '../history/presentation/history_clip_presentation.dart';
import '../history/presentation/source_app_label.dart';
import '../paste_queue/view/paste_queue_drawer.dart';
import '../../shared/dialogs/question_dialog.dart';
import '../modules/models/module_view.dart';
import '../modules/view/module_view_form.dart';
import '../transforms/view/transform_palette.dart';
import 'quick_paste_controller.dart';
import 'package:copypaste_flutter/features/history/presentation/file_clip_summary.dart';
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

class _QuickPasteViewState extends State<QuickPasteView>
    with WidgetsBindingObserver {
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
  final _resultScroll = ScrollController();
  final _resultKeys = <String, GlobalKey>{};
  int _noticeRevision = 0;
  QuickPasteModuleForm? _shownForm;

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
        final presented = await controller.presentFrame(id);
        if (!presented ||
            !mounted ||
            controller.renderablePresentationId != id) {
          return;
        }
        _presentedFrame = id;
        // Native window activation can displace autofocus on the prepared frame.
        _searchFocus.requestFocus();
      } finally {
        if (_preparingFrame == id) _preparingFrame = null;
        if (!attempted && mounted) _preparePresentationFrame();
      }
    });
  }

  Future<void> _prepareClipMedia(HistoryClip clip) async {
    try {
      // Masked rows never load content they would not show.
      final masked =
          controller.maskedMode || (clip.secret && controller.secretsLocked);
      if (clip.secret && !masked) {
        clip = await controller.history.media.spoilerContent(clip.id);
      }
      final loads = <Future<void>>[];
      if (!masked && clip.contentKind == HistoryClipKind.image) {
        loads.add(() async {
          final bounds = _quickPastePreviewBounds(context);
          final preview = await controller.history.media.requestImagePreview(
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
          final icon = await controller.history.media.requestSourceIcon(
            clip.sourceAppIconId,
          );
          if (mounted && icon != null) {
            await precacheImage(
              SourceAppLabel.iconImage(context, icon),
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
    _operationErrorRevision = controller.history.errors.operationErrorRevision;
    _query = controller.history.window.query;
    _noticeRevision = controller.noticeRevision;
    controller.confirmUnlock = _confirmUnlock;
    controller.addListener(_presentationChanged);
    _scroll.addListener(_recentScrolled);
    _pinnedScroll.addListener(_pinnedScrolled);
    controller.history.window.beforeWindowChange = _captureWindowAnchors;
    controller.history.window.afterWindowChange = _restoreWindowAnchors;
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // The key nonactivating popup leaves the application inactive, and the
    // desktop FocusManager suspends keyboard focus on that transition.
    if (state != AppLifecycleState.inactive) return;
    scheduleMicrotask(() {
      if (!mounted ||
          _presentedFrame == null ||
          _presentedFrame != controller.renderablePresentationId ||
          FocusManager.instance.primaryFocus !=
              FocusManager.instance.rootScope) {
        return;
      }
      _searchFocus.requestFocus();
    });
  }

  /// Confirms an unlock in the app where the device cannot verify its owner.
  Future<bool> _confirmUnlock() async {
    if (!mounted) return false;
    return confirmOwnerInApp(context);
  }

  void _showNotice(QuickPasteNotice notice) {
    final l10n = context.l10n;
    final (title, message, tone) = switch (notice.kind) {
      QuickPasteNoticeKind.copiedOnly => (
        l10n.historyCopiedTitle,
        l10n.quickPastePasteManually,
        AppToastTone.information,
      ),
      QuickPasteNoticeKind.defaultSkipped => (
        l10n.quickPasteDefaultSkipped,
        l10n.quickPasteDefaultSkippedMessage,
        AppToastTone.information,
      ),
      QuickPasteNoticeKind.addedToQueue => (
        l10n.pasteQueueSettingsTitle,
        l10n.pasteQueueAdded(notice.count),
        AppToastTone.success,
      ),
      QuickPasteNoticeKind.failed => (
        l10n.quickPasteActionFailedTitle,
        notice.message ?? l10n.pasteQueueFailed,
        AppToastTone.error,
      ),
    };
    AppToast.show(context, title: title, message: message, tone: tone);
  }

  /// The module form focuses its first field itself; closing it returns
  /// focus to the search field.
  void _focusOpenedForm() {
    final form = controller.moduleForm;
    if (identical(form, _shownForm)) return;
    final closed = form == null && _shownForm != null;
    _shownForm = form;
    if (closed) _searchFocus.requestFocus();
  }

  void _presentationChanged() {
    _focusOpenedForm();
    if (_noticeRevision != controller.noticeRevision) {
      _noticeRevision = controller.noticeRevision;
      final notice = controller.notice;
      final presentation = controller.presentationGeneration;
      if (notice != null) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted && controller.presentationGeneration == presentation) {
            _showNotice(notice);
          }
        });
      }
    }
    final revision = controller.history.errors.operationErrorRevision;
    if (_operationErrorRevision != revision) {
      _operationErrorRevision = revision;
      final message = controller.history.errors.operationError;
      final presentation = controller.presentationGeneration;
      if (message != null) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted && controller.presentationGeneration == presentation) {
            AppToast.show(
              context,
              title: context.l10n.quickPasteActionFailedTitle,
              message: message,
              tone: AppToastTone.error,
            );
          }
        });
      }
    }
    final presentationChanged =
        _generation != controller.presentationGeneration;
    final queryChanged = !identical(_query, controller.history.window.query);
    if (!presentationChanged && !queryChanged) return;
    _generation = controller.presentationGeneration;
    _query = controller.history.window.query;
    if (presentationChanged) {
      _search.clear();
      _searchFocus.requestFocus();
    }
    if (_scroll.hasClients) _scroll.jumpTo(0);
    if (_pinnedScroll.hasClients) _pinnedScroll.jumpTo(0);
    if (_resultScroll.hasClients) _resultScroll.jumpTo(0);
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
        ? controller.history.window.visitedPinnedIds
        : controller.history.window.visitedRecentIds;
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
        ? controller.history.window.visitedPinnedIds
        : controller.history.window.visitedRecentIds;
    if (ids.isEmpty) return;
    final id = ids.contains(anchor.id)
        ? anchor.id
        : ids[anchor.index.clamp(0, ids.length - 1)];
    if (_rowKeys[id]?.currentContext == null) {
      await controller.history.window.materializeId(id);
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
    WidgetsBinding.instance.removeObserver(this);
    if (controller.confirmUnlock == _confirmUnlock) {
      controller.confirmUnlock = null;
    }
    controller.removeListener(_presentationChanged);
    controller.history.window.beforeWindowChange = null;
    controller.history.window.afterWindowChange = null;
    _search.dispose();
    _searchFocus.dispose();
    _scroll.dispose();
    _pinnedScroll.dispose();
    _resultScroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, child) => CallbackShortcuts(
        bindings: _shortcutBindings(),
        child: ClipRRect(
          // Windows clips the complete native popup to this radius. A second
          // Flutter clip exposes the opaque backing surface at the corners.
          borderRadius: defaultTargetPlatform == TargetPlatform.windows
              ? BorderRadius.zero
              : const BorderRadius.all(Radius.circular(AppRadius.lg)),
          child: Scaffold(
            backgroundColor: AppTheme.quickPasteSurface(context),
            child: Padding(
              padding: const EdgeInsets.all(AppSpacing.sm),
              child: LayoutBuilder(
                builder: (context, constraints) {
                  _preparePresentationFrame();
                  final menu = _MenuScope(child: _menu(context));
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
            child: AppBrandLogo(
              key: const ValueKey('quick-paste-logo'),
              size: AppIconSize.sm,
              semanticLabel: context.l10n.quickPasteOpenAppSemantics,
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
                  skipInputFeatureFocusTraversal: true,
                  style: AppTheme.clipboardMenuTextStyle(context),
                  placeholder: Text(
                    context.l10n.quickPasteSearchPlaceholder,
                    style: AppTheme.clipboardMenuTextStyle(context),
                  ),
                  features: const [
                    InputFeature.leading(
                      AppIconFrame(
                        size: AppIconSize.sm,
                        child: Icon(LucideIcons.search, size: AppIconSize.sm),
                      ),
                    ),
                  ],
                  onChanged: (value) => unawaited(controller.search(value)),
                ),
              ),
            ),
          ),
          const Gap(AppSpacing.xs),
          if (controller.history.transfers.fileOperations
              case final transfers?) ...[
            FileOperationsHeaderAction(controller: transfers),
            const Gap(AppSpacing.xs),
          ],
          Semantics(
            label: context.l10n.quickPasteToggleClipDetails,
            button: true,
            child: Button.ghost(
              style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
              key: const ValueKey('quick-paste-inspector-toggle'),
              onPressed: controller.toggleInspector,
              child: const AppIconFrame(
                child: Icon(LucideIcons.panelsLeftBottom),
              ),
            ),
          ),
        ],
      ),
      if (controller.tabs case final tabs when tabs.length > 1) ...[
        const Gap(AppSpacing.xs),
        Tabs(
          key: const ValueKey('quick-paste-tabs'),
          expand: true,
          index: tabs.indexOf(controller.tab),
          onChanged: (index) => unawaited(controller.selectTab(tabs[index])),
          children: [
            for (final tab in tabs)
              TabItem(child: Text(_tabLabel(context, tab))),
          ],
        ),
      ],
      if (controller.maskedMode) ...[
        const Gap(AppSpacing.xs),
        _Notice(
          key: const ValueKey('quick-paste-masked-mode'),
          icon: LucideIcons.lock,
          text: context.l10n.quickPasteMaskedMode,
        ),
      ],
      if (controller.defaultPipeline case final pipeline?
          when controller.tab == QuickPasteTab.history &&
              controller.moduleForm == null) ...[
        const Gap(AppSpacing.xs),
        _Notice(
          key: const ValueKey('quick-paste-default-pipeline'),
          icon: LucideIcons.wandSparkles,
          text: context.l10n.quickPastePastingWith(pipeline.name),
          hint: context.l10n.quickPastePasteOriginalHint(
            defaultTargetPlatform == TargetPlatform.macOS
                ? '⌘⇧↩'
                : 'Ctrl+Shift+Enter',
          ),
        ),
      ],
      const Gap(AppSpacing.xs),
      Expanded(child: _body(context)),
      if (controller.tab != QuickPasteTab.queue) ...[
        const Gap(AppSpacing.xs),
        const Divider(),
        const Gap(AppSpacing.xs),
        _Footer(controller: controller),
      ],
    ],
  );

  String _tabLabel(BuildContext context, QuickPasteTab tab) =>
      switch (tab.kind) {
        QuickPasteTabKind.history => context.l10n.appDestinationHistory,
        QuickPasteTabKind.module => controller.moduleTabTitle(tab) ?? '',
        QuickPasteTabKind.queue => context.l10n.quickPasteTabQueue,
      };

  Widget _body(BuildContext context) {
    final form = controller.moduleForm;
    if (form != null) {
      return _ModulePanel(
        key: ValueKey('quick-paste-module-form-${form.moduleId}'),
        controller: controller,
        form: form,
      );
    }
    return switch (controller.tab.kind) {
      QuickPasteTabKind.history => _historyWithResults(context),
      QuickPasteTabKind.module => _resultList(context),
      QuickPasteTabKind.queue => _queue(),
    };
  }

  Widget _historyWithResults(BuildContext context) {
    final results = controller.moduleResults;
    _retainResultKeys(results);
    if (results.isEmpty) return _history(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Column(
          key: const ValueKey('quick-paste-module-section'),
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [for (final result in results) _resultRow(result)],
        ),
        const Gap(AppSpacing.xs),
        const Divider(),
        const Gap(AppSpacing.xs),
        Expanded(child: _history(context)),
      ],
    );
  }

  Widget _resultList(BuildContext context) {
    final l10n = context.l10n;
    final rows = controller.moduleResults;
    _retainResultKeys(rows);
    if (rows.isEmpty) {
      if (controller.moduleResultsLoading) {
        return StateView.loading(message: l10n.commonLoading);
      }
      if (controller.moduleResultsFailed) {
        return StateView.error(title: l10n.modulesOperationFailed);
      }
      return StateView.empty(
        icon: LucideIcons.searchX,
        title: l10n.transformsWarningNoMatches,
      );
    }
    return Scrollbar(
      controller: _resultScroll,
      child: ListView.builder(
        key: const ValueKey('quick-paste-module-scroll'),
        controller: _resultScroll,
        primary: false,
        padding: EdgeInsets.zero,
        itemCount: rows.length,
        itemBuilder: (context, index) => _resultRow(rows[index]),
      ),
    );
  }

  static String _resultKey(QuickPasteModuleResult result) =>
      '${result.moduleId}/${result.item.id}';

  void _retainResultKeys(List<QuickPasteModuleResult> rows) {
    final keys = {for (final row in rows) _resultKey(row)};
    _resultKeys.removeWhere((key, _) => !keys.contains(key));
  }

  Widget _resultRow(QuickPasteModuleResult result) {
    final l10n = context.l10n;
    final item = result.item;
    final key = _resultKey(result);
    final focused = result.sameAs(controller.focusedResult);
    final preview = item.secret || controller.maskedMode
        ? _maskDots
        : item.preview.trim().split('\n').first;
    final theme = Theme.of(context);
    final busy = controller.activating;
    Widget hostAction(String label, IconData icon, VoidCallback onPressed) =>
        Tooltip(
          tooltip: (context) => TooltipContainer(child: Text(label)),
          child: Button.ghost(
            style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
            onPressed: busy ? null : onPressed,
            child: Semantics(
              label: label,
              child: AppIconFrame(
                size: AppIconSize.sm,
                child: Icon(icon, size: AppIconSize.sm),
              ),
            ),
          ),
        );
    return KeyedSubtree(
      key: ValueKey('quick-paste-module-result-$key'),
      child: Builder(
        key: _resultKeys.putIfAbsent(key, GlobalKey.new),
        builder: (context) => MouseRegion(
          // Rows that move under a resting pointer must not take the
          // focus, which would undo a preselected result.
          onHover: (_) => controller.focusResult(result),
          child: Row(
            children: [
              Expanded(
                child: Button.ghost(
                  style: AppTheme.clipboardMenuButtonStyle(selected: focused),
                  alignment: Alignment.centerLeft,
                  disableFocusOutline: true,
                  onFocus: (focused) {
                    if (focused) controller.focusResult(result);
                  },
                  onPressed: busy
                      ? null
                      : () => unawaited(
                          controller.activateResult(
                            result,
                            invertAutoPaste:
                                HardwareKeyboard.instance.isAltPressed,
                          ),
                        ),
                  child: Row(
                    children: [
                      AppIconFrame(
                        size: AppIconSize.sm,
                        child: Icon(
                          item.secret ? LucideIcons.lock : LucideIcons.puzzle,
                          size: AppIconSize.sm,
                        ),
                      ),
                      const Gap(AppSpacing.sm),
                      Expanded(
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              item.title,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                            Text(
                              preview,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.typography.xSmall.copyWith(
                                color: theme.colorScheme.mutedForeground,
                              ),
                            ),
                          ],
                        ),
                      ),
                      for (final (index, badge) in item.badges.indexed) ...[
                        const Gap(AppSpacing.xs),
                        if (index == 0 && item.preselect)
                          PrimaryBadge(
                            key: ValueKey('quick-paste-module-preselect-$key'),
                            child: Text(badge),
                          )
                        else
                          SecondaryBadge(child: Text(badge)),
                      ],
                    ],
                  ),
                ),
              ),
              if (focused) ...[
                hostAction(
                  l10n.commonCopy,
                  LucideIcons.copy,
                  () => unawaited(controller.copyFocusedResult()),
                ),
                if (controller.tabs.contains(QuickPasteTab.queue))
                  hostAction(
                    l10n.historyActionAddToQueue,
                    LucideIcons.listPlus,
                    () => unawaited(controller.addFocusedToQueue()),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _queue() {
    final queue = controller.queue;
    if (queue == null) return const SizedBox.shrink();
    return PasteQueueDrawer(
      controller: queue,
      onOpenSettings: () => unawaited(controller.openSettings()),
      onClose: () => unawaited(controller.selectTab(QuickPasteTab.history)),
    );
  }

  Future<void> _openTransforms() async {
    final transforms = controller.transforms;
    if (transforms == null || !controller.canTransform) return;
    final source = await controller.transformSource();
    if (source == null || !mounted) return;
    await showTransformPalette(
      context,
      repository: transforms,
      source: source,
      recents: controller.transformRecents,
      pasteForeground: controller.pasteTransformResult,
    );
  }

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
    if (history.window.isLoadingMore ||
        history.window.state != HistoryLoadState.ready) {
      return;
    }
    if (_pinnedIsActive &&
        _pinnedScroll.hasClients &&
        _pinnedScroll.position.hasContentDimensions) {
      final position = _pinnedScroll.position;
      if (position.extentBefore <= 1 && history.window.canLoadPinnedPrevious) {
        unawaited(history.window.loadPinnedPrevious());
        return;
      }
      if (position.extentAfter <= position.viewportDimension &&
          history.window.canLoadPinnedMore) {
        unawaited(history.window.loadPinnedMore());
        return;
      }
    }
    if (!_pinnedIsActive &&
        _scroll.hasClients &&
        _scroll.position.hasContentDimensions &&
        _scroll.position.extentBefore <= 1 &&
        history.window.canLoadPrevious) {
      unawaited(history.window.loadPrevious());
      return;
    }
    if (!history.window.canLoadMore || controller.paginationFailed) return;
    final scroll = history.window.items.any((clip) => !clip.pinned)
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
    if (history.window.state == HistoryLoadState.loading &&
        controller.items.isEmpty) {
      return StateView.loading(message: context.l10n.quickPasteLoadingHistory);
    }
    if (history.window.state == HistoryLoadState.error) {
      return StateView.error(
        title: context.l10n.quickPasteHistoryUnavailable,
        message: history.errors.message,
      );
    }
    final items = controller.items;
    final recentIds = history.window.visitedRecentIds;
    final pinnedIds = history.window.visitedPinnedIds;
    if (recentIds.isEmpty && pinnedIds.isEmpty) {
      return StateView.empty(title: context.l10n.quickPasteNoClipsFound);
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
      itemCount:
          ids.length + (showPagination && history.window.canLoadMore ? 1 : 0),
      itemBuilder: (context, index) {
        if (index >= ids.length) {
          if (history.window.isLoadingMore) {
            return const Padding(
              padding: EdgeInsets.all(AppSpacing.sm),
              child: Center(
                child: AppIconFrame(
                  size: AppIconSize.sm,
                  child: CircularProgressIndicator(),
                ),
              ),
            );
          }
          // The inset keeps the keyboard focus ring inside the list viewport.
          return Padding(
            padding: const EdgeInsets.all(AppSpacing.xs),
            child: Button.ghost(
              key: const ValueKey('quick-paste-load-more'),
              style: AppTheme.clipboardMenuButtonStyle(),
              onPressed: controller.loadMore,
              child: Text(
                controller.paginationFailed
                    ? context.l10n.commonTryAgain
                    : context.l10n.quickPasteLoadMore,
              ),
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
              extent: history.window.quickPasteRowExtent(id, pinned: pinned),
              onVisible: () => unawaited(history.window.materializeId(id)),
            ),
          );
        }
        return KeyedSubtree(
          key: ValueKey('quick-paste-row-$id'),
          child: QuickPasteRowGeometry(
            onExtent: (extent) => history.window.reportQuickPasteRowExtent(
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
    final query = controller.history.window.query;
    _focusTraversal = _focusTraversal.then((_) async {
      if (!mounted ||
          generation != controller.presentationGeneration ||
          !identical(query, controller.history.window.query)) {
        return;
      }
      await controller.moveFocus(offset);
      if (!mounted || generation != controller.presentationGeneration) return;
      if (controller.focusedResult case final result?) {
        await WidgetsBinding.instance.endOfFrame;
        final row = _resultKeys[_resultKey(result)]?.currentContext;
        if (mounted && row != null && row.mounted) {
          await Scrollable.ensureVisible(row);
        }
        return;
      }
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
        identical(query, controller.history.window.query) &&
        controller.focusedClip?.id == clip.id &&
        scroll.hasClients) {
      final rowContext = _rowKeys[clip.id]?.currentContext;
      if (rowContext != null && rowContext.mounted) {
        await Scrollable.ensureVisible(rowContext);
        return;
      }
      final clips = clip.pinned
          ? controller.history.window.visitedPinnedIds
          : controller.history.window.visitedRecentIds;
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
    final window = <ShortcutActivator, VoidCallback>{
      ...desktopWindowBindings(
        onClose: controller.close,
        onQuit: controller.quit,
      ),
      if (defaultTargetPlatform != TargetPlatform.macOS) ...{
        const SingleActivator(
          LogicalKeyboardKey.f4,
          alt: true,
          includeRepeats: false,
        ): () =>
            unawaited(controller.close()),
      },
    };
    // The form's own fields own Enter, Tab and the arrows; Esc goes back.
    if (controller.moduleForm != null) {
      return {
        ...window,
        const SingleActivator(LogicalKeyboardKey.escape):
            controller.closeModuleForm,
      };
    }
    final bindings = <ShortcutActivator, VoidCallback>{
      ...window,
      desktopShortcut(LogicalKeyboardKey.keyF): _find,
      ...desktopRefreshBindings(
        () => unawaited(controller.history.window.reload()),
      ),
      const SingleActivator(LogicalKeyboardKey.escape): () =>
          unawaited(controller.close()),
      desktopShortcut(LogicalKeyboardKey.keyT): () =>
          unawaited(_openTransforms()),
      desktopShortcut(LogicalKeyboardKey.keyQ, shift: true): () =>
          unawaited(controller.addFocusedToQueue()),
      desktopShortcut(LogicalKeyboardKey.enter, shift: true): () =>
          unawaited(controller.activateFocused(bypassDefault: true)),
      const SingleActivator(LogicalKeyboardKey.tab, control: true): () =>
          unawaited(controller.cycleTab(1)),
      const SingleActivator(
        LogicalKeyboardKey.tab,
        control: true,
        shift: true,
      ): () =>
          unawaited(controller.cycleTab(-1)),
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
      // Word and line deletion stay with the search field; Shift deletes a clip.
      desktopShortcut(LogicalKeyboardKey.backspace, shift: true): () =>
          unawaited(controller.deleteFocused()),
      desktopShortcut(LogicalKeyboardKey.delete, shift: true): () =>
          unawaited(controller.deleteFocused()),
      desktopShortcut(LogicalKeyboardKey.comma): () =>
          unawaited(controller.openSettings()),
    };
    if (controller.tab == QuickPasteTab.history) {
      for (final entry in controller.shortcuts.entries) {
        bindings[desktopShortcut(entry.key)] = () =>
            unawaited(controller.activate(entry.value));
      }
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
          onHover: (_) => controller.hoverClip(clip, hovered: true),
          onExit: (_) => controller.hoverClip(clip, hovered: false),
          child: Button.ghost(
            style: AppTheme.clipboardMenuButtonStyle(
              selected: controller.focusedClip?.id == clip.id,
            ),
            alignment: Alignment.centerLeft,
            // Focus moves the selection, which is this row's only focus cue.
            disableFocusOutline: true,
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
                // The icon stays level with the first line of tall content.
                Expanded(
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      SizedBox(
                        width: AppIconSize.sm,
                        height: _firstLineHeight(context),
                        child: Center(
                          child: clip.sourceApp != null
                              ? SourceAppLabel(
                                  name: clip.sourceApp!,
                                  icon: controller.history.media
                                      .requestSourceIcon(clip.sourceAppIconId),
                                  showName: false,
                                )
                              : AppIconFrame(
                                  size: AppIconSize.sm,
                                  child: Icon(
                                    HistoryClipPresentation.icon(
                                      clip.contentKind,
                                    ),
                                    size: AppIconSize.sm,
                                  ),
                                ),
                        ),
                      ),
                      const Gap(AppSpacing.sm),
                      Expanded(
                        child: _ClipContent(clip: clip, controller: controller),
                      ),
                    ],
                  ),
                ),
                const Gap(AppSpacing.sm),
                ListenableBuilder(
                  listenable:
                      controller.history.transfers.downloadProgressChanges,
                  builder: (context, _) {
                    final progress = controller.transferProgressFor(clip.id);
                    final status = progress != null
                        ? SizedBox(
                            width: AppLayoutSize.quickPasteTransferStatusWidth,
                            child: Row(
                              children: [
                                AppIconFrame(
                                  size: AppIconSize.xs,
                                  child: CircularProgressIndicator(
                                    value: progress,
                                    theme: const CircularProgressIndicatorTheme(
                                      size: AppIconSize.xs,
                                    ),
                                    animated: !MediaQuery.disableAnimationsOf(
                                      context,
                                    ),
                                  ),
                                ),
                                const Gap(AppSpacing.xs),
                                Text(
                                  context.l10n.transfersProgressPercent(
                                    (progress * 100).floor(),
                                  ),
                                ),
                              ],
                            ),
                          )
                        : controller.transferFailedFor(clip.id)
                        ? Text(
                            context.l10n.quickPasteTransferFailed,
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

/// Height of one line of the row text, at least the icon size.
double _firstLineHeight(BuildContext context) {
  final painter = TextPainter(
    text: TextSpan(text: 'M', style: DefaultTextStyle.of(context).style),
    textDirection: Directionality.of(context),
    textScaler: MediaQuery.textScalerOf(context),
    maxLines: 1,
  )..layout();
  final height = painter.height;
  painter.dispose();
  return math.max(height, AppIconSize.sm);
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
    if (controller.maskedMode || (clip.secret && controller.secretsLocked)) {
      return _MaskedClip(clip: clip, controller: controller);
    }
    if (clip.secret && !revealed) {
      return FutureBuilder<HistoryClip>(
        future: controller.history.media.spoilerContent(clip.id),
        builder: (context, snapshot) => Align(
          alignment: Alignment.centerLeft,
          child: SecretSpoiler(
            clipId: clip.id,
            controller: controller.history.spoilers,
            key: ValueKey(
              'quick-spoiler-${clip.id}-${controller.presentationGeneration}',
            ),
            child: snapshot.hasError
                ? StateView.error(title: context.l10n.quickPasteClipUnavailable)
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
      return FileClipSummary(
        clip: clip,
        pathKey: ValueKey('quick-paste-file-path-${clip.id}'),
      );
    }
    if (clip.contentKind != HistoryClipKind.image) {
      return Text(
        clip.preview.isEmpty ? clip.contentKind.label : clip.preview,
        maxLines: 4,
        overflow: TextOverflow.ellipsis,
      );
    }
    final bounds = _quickPastePreviewBounds(context);
    return FutureBuilder<HistoryImagePreview?>(
      future: controller.history.media.requestImagePreview(
        clip.id,
        maxEdge: 480,
        bounds: bounds,
      ),
      builder: (context, snapshot) {
        final preview = snapshot.data;
        final image = ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 240, maxHeight: 120),
          child: preview == null
              ? const SizedBox(
                  height: 120,
                  child: AppIconFrame(child: Icon(LucideIcons.image)),
                )
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
                          const AppIconFrame(child: Icon(LucideIcons.imageOff)),
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

const _maskDots = '••••••••';

/// A row's content while it must stay hidden: its kind only, and for a
/// locked secret the action that unlocks secrets.
class _MaskedClip extends StatelessWidget {
  const _MaskedClip({required this.clip, required this.controller});

  final HistoryClip clip;
  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final kind = clip.contentKind.label;
    final theme = Theme.of(context);
    return Row(
      key: ValueKey('quick-paste-masked-${clip.id}'),
      children: [
        Expanded(
          child: Semantics(
            label: l10n.quickPasteMaskedItem(kind),
            child: ExcludeSemantics(
              child: Text(
                '$_maskDots  $kind',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(color: theme.colorScheme.mutedForeground),
              ),
            ),
          ),
        ),
        if (clip.secret && controller.secretsLocked && !controller.maskedMode)
          Tooltip(
            tooltip: (context) =>
                TooltipContainer(child: Text(l10n.quickPasteUnlockSecrets)),
            child: Button.ghost(
              key: ValueKey('quick-paste-unlock-${clip.id}'),
              style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
              onPressed: () => unawaited(controller.unlockSecrets()),
              child: Semantics(
                label: l10n.quickPasteUnlockSecrets,
                child: const AppIconFrame(child: Icon(LucideIcons.lock)),
              ),
            ),
          ),
      ],
    );
  }
}

/// One line of state under the header, such as masked mode or the default
/// paste transform.
class _Notice extends StatelessWidget {
  const _Notice({super.key, required this.icon, required this.text, this.hint});

  final IconData icon;
  final String text;
  final String? hint;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final muted = theme.typography.xSmall.copyWith(
      color: theme.colorScheme.mutedForeground,
    );
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
      child: Row(
        children: [
          AppIconFrame(
            size: AppIconSize.sm,
            child: Icon(icon, size: AppIconSize.sm),
          ),
          const Gap(AppSpacing.xs),
          Flexible(
            child: Text(
              text,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: theme.typography.small,
            ),
          ),
          if (hint case final hint?) ...[
            const Gap(AppSpacing.sm),
            Expanded(
              child: Text(
                hint,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                textAlign: TextAlign.end,
                style: muted,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// A module's view for one result, such as a template's fill-in form,
/// inside the popup, with Back fixed below it; only the content scrolls.
class _ModulePanel extends StatelessWidget {
  const _ModulePanel({super.key, required this.controller, required this.form});

  final QuickPasteController controller;
  final QuickPasteModuleForm form;

  @override
  Widget build(BuildContext context) {
    final items = [for (final section in form.view.sections) ...section.items];
    final firstForm = items.whereType<ModuleViewForm>().firstOrNull;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: AppSpacing.xs),
          child: Row(
            children: [
              const AppIconFrame(
                size: AppIconSize.sm,
                child: Icon(LucideIcons.braces, size: AppIconSize.sm),
              ),
              const Gap(AppSpacing.sm),
              Expanded(
                child: Text(
                  form.title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
        ),
        const Gap(AppSpacing.sm),
        Expanded(
          child: SingleChildScrollView(
            key: const ValueKey('quick-paste-module-form-body'),
            padding: const EdgeInsets.all(AppSpacing.xs),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (final item in items)
                  switch (item) {
                    ModuleViewForm() => ModuleViewFormCard(
                      key: ObjectKey(item),
                      form: item,
                      enabled: !controller.activating,
                      autofocus: identical(item, firstForm),
                      onSubmit: (action) =>
                          unawaited(controller.submitModuleForm(action)),
                      onPreview: controller.previewModuleForm,
                    ),
                    ModuleViewNotice(:final text) => Padding(
                      padding: const EdgeInsets.only(bottom: AppSpacing.sm),
                      child: _Notice(icon: LucideIcons.info, text: text),
                    ),
                    ModuleViewBlock(:final text) => Padding(
                      padding: const EdgeInsets.only(bottom: AppSpacing.sm),
                      child: Text(text),
                    ),
                    _ => const SizedBox.shrink(),
                  },
              ],
            ),
          ),
        ),
        const Gap(AppSpacing.sm),
        Button.secondary(
          key: const ValueKey('quick-paste-module-form-back'),
          style: AppTheme.actionButtonStyle(const ButtonStyle.secondary()),
          alignment: Alignment.center,
          onPressed: controller.closeModuleForm,
          child: Text(context.l10n.commonBack),
        ),
      ],
    );
  }
}

/// What every row of the menu shares: one divider colour, and shortcut keys
/// in equal slots so the modifiers of stacked rows form a column.
class _MenuScope extends StatelessWidget {
  const _MenuScope({required this.child});

  final Widget child;

  static final _modifiers = {
    LogicalKeyboardKey.control,
    LogicalKeyboardKey.alt,
    LogicalKeyboardKey.meta,
    LogicalKeyboardKey.shift,
  };

  @override
  Widget build(BuildContext context) {
    final display = Data.of<KeyboardShortcutDisplayHandle>(context);
    return ComponentTheme<DividerTheme>(
      data: DividerTheme(color: AppTheme.popupBorder(context)),
      child: KeyboardShortcutDisplayMapper(
        builder: (context, key) {
          final label = display.buildKeyboardDisplay(context, key);
          return _modifiers.contains(key) ? label : _ShortcutKeySlot(label);
        },
        child: child,
      ),
    );
  }
}

/// At least one em of the shortcut text, the width any single key fits in.
class _ShortcutKeySlot extends StatelessWidget {
  const _ShortcutKeySlot(this.label);

  final Widget label;

  @override
  Widget build(BuildContext context) {
    final size = DefaultTextStyle.of(context).style.fontSize ?? 0;
    return ConstrainedBox(
      constraints: BoxConstraints(
        minWidth: MediaQuery.textScalerOf(context).scale(size),
      ),
      child: label,
    );
  }
}

class _Footer extends StatelessWidget {
  const _Footer({required this.controller});

  final QuickPasteController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _action(
          LucideIcons.trash2,
          l10n.quickPasteClearHistory,
          () => confirmClear(context, controller),
        ),
        _action(
          LucideIcons.settings,
          l10n.quickPasteSettings,
          controller.openSettings,
          shortcut: LogicalKeyboardKey.comma,
        ),
        _action(
          LucideIcons.info,
          l10n.quickPasteAbout,
          () => showAbout(context),
        ),
        _action(
          LucideIcons.power,
          l10n.quickPasteQuit,
          controller.quit,
          shortcut: LogicalKeyboardKey.keyQ,
        ),
      ],
    );
  }

  /// Laid out like a clip row, so icons and labels share its columns.
  Widget _action(
    IconData icon,
    String label,
    VoidCallback onPressed, {
    LogicalKeyboardKey? shortcut,
  }) => Button.ghost(
    style: AppTheme.clipboardMenuButtonStyle(),
    alignment: Alignment.centerLeft,
    onPressed: onPressed,
    child: Row(
      children: [
        AppIconFrame(
          size: AppIconSize.sm,
          child: Icon(icon, size: AppIconSize.sm),
        ),
        const Gap(AppSpacing.sm),
        Expanded(child: Text(label)),
        if (shortcut != null)
          MenuShortcut(
            activator: desktopShortcut(shortcut),
            combiner: const SizedBox.shrink(),
          ),
      ],
    ),
  );

  static Future<void> confirmClear(
    BuildContext context,
    QuickPasteController controller,
  ) async {
    await AppOverlays.showActionDialog(
      context,
      icon: LucideIcons.trash2,
      title: Text(context.l10n.quickPasteClearHistoryTitle),
      content: Text(context.l10n.quickPasteClearHistoryMessage),
      actionLabel: context.l10n.commonClear,
      onConfirm: controller.clearUnpinned,
      failureMessage: () =>
          controller.history.errors.message ??
          AppStrings.current.quickPasteClearHistoryFailed,
    );
  }

  static Future<void> showAbout(BuildContext context) {
    return AppOverlays.showDialog<void>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.info,
        title: const Text('CopyPaste'),
        content: Text(dialogContext.l10n.quickPasteAboutDescription),
        actions: [
          Button.primary(
            style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
            onPressed: () => Navigator.pop(dialogContext),
            child: Text(dialogContext.l10n.commonDone),
          ),
        ],
      ),
    );
  }
}
