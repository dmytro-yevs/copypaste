import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../history/presentation/secret_spoiler.dart';

import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/state_view.dart';
import '../history/controller/history_controller.dart';
import '../history/models/history_models.dart';
import '../history/view/history_inspector.dart';
import '../history/presentation/history_clip_presentation.dart';
import '../history/presentation/source_app_label.dart';
import 'quick_paste_controller.dart';

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
  int _generation = 0;
  HistoryQuery _query = const HistoryQuery();

  QuickPasteController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _generation = controller.presentationGeneration;
    _query = controller.history.query;
    controller.addListener(_presentationChanged);
    _scroll.addListener(_scrolled);
    _pinnedScroll.addListener(_scrolled);
  }

  void _presentationChanged() {
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

  @override
  void dispose() {
    controller.removeListener(_presentationChanged);
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
                  final menu = _menu(context);
                  if (!controller.inspectorOpen) return menu;
                  final menuWidth =
                      (AppLayoutSize.quickPasteMenuWidth - AppSpacing.sm * 2)
                          .clamp(0.0, constraints.maxWidth * 0.55)
                          .toDouble();
                  return Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      SizedBox(width: menuWidth, child: menu),
                      const Gap(AppSpacing.sm),
                      Expanded(
                        child: HistoryInspector(
                          controller: controller.history,
                          presentationRevision:
                              controller.presentationGeneration,
                          inDrawer: false,
                          showActions: false,
                          compact: true,
                          onClose: () =>
                              unawaited(controller.toggleInspector()),
                        ),
                      ),
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
          const Image(
            key: ValueKey('quick-paste-logo'),
            image: AssetImage('assets/brand/copypaste.png'),
            width: AppIconSize.sm,
            height: AppIconSize.sm,
            semanticLabel: 'CopyPaste',
          ),
          const Gap(AppSpacing.xs),
          Expanded(
            child: Theme(
              data: AppTheme.clipboardSearchTheme(context),
              child: SizedBox(
                height: AppControlSize.compact,
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
          Button.ghost(
            style: const ButtonStyle.ghostIcon(),
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

  void _scrolled() {
    final clip = controller.focusedClip;
    if (clip != null) controller.hoverClip(clip, hovered: false);
    _loadMoreWhenNeeded();
  }

  void _loadMoreWhenNeeded() {
    final history = controller.history;
    if (!history.canLoadMore ||
        history.isLoadingMore ||
        history.state != HistoryLoadState.ready ||
        controller.paginationFailed) {
      return;
    }
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
    if (items.isEmpty) {
      return const StateView.empty(title: 'No clips found');
    }
    final retainedIds = items.map((clip) => clip.id).toSet();
    _rowKeys.removeWhere((id, _) => !retainedIds.contains(id));
    final recent = items.where((clip) => !clip.pinned).toList();
    final pinned = items.where((clip) => clip.pinned).toList();
    _checkPaginationAfterLayout();
    return LayoutBuilder(
      builder: (context, constraints) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(
            child: Scrollbar(
              controller: _scroll,
              child: _clipList(
                recent,
                scroll: _scroll,
                key: 'quick-paste-history-scroll',
                showPagination: recent.isNotEmpty,
              ),
            ),
          ),
          if (pinned.isNotEmpty) ...[
            if (recent.isNotEmpty) ...[
              const Gap(AppSpacing.xs),
              const Divider(),
              const Gap(AppSpacing.xs),
            ],
            ConstrainedBox(
              constraints: BoxConstraints(
                maxHeight: recent.isEmpty
                    ? constraints.maxHeight
                    : constraints.maxHeight / 2,
              ),
              child: _clipList(
                pinned,
                scroll: _pinnedScroll,
                key: 'quick-paste-pinned-scroll',
                shrinkWrap: true,
                showPagination: recent.isEmpty,
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _clipList(
    List<HistoryClip> clips, {
    required ScrollController scroll,
    required String key,
    bool shrinkWrap = false,
    required bool showPagination,
  }) {
    final history = controller.history;
    final indices = {
      for (final (index, clip) in clips.indexed)
        ValueKey('quick-paste-${clip.id}'): index,
    };
    return ListView.builder(
      key: ValueKey(key),
      controller: scroll,
      primary: false,
      padding: EdgeInsets.zero,
      shrinkWrap: shrinkWrap,
      findChildIndexCallback: (key) => indices[key],
      itemCount: clips.length + (showPagination && history.canLoadMore ? 1 : 0),
      itemBuilder: (context, index) {
        if (index < clips.length) return _item(clips[index]);
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
          child: Text(controller.paginationFailed ? 'Try again' : 'Load more'),
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
      final clips = controller.items
          .where((item) => item.pinned == clip.pinned)
          .toList();
      final target = clips.indexWhere((item) => item.id == clip.id);
      final firstMounted = clips.indexWhere(
        (item) => _rowKeys[item.id]?.currentContext != null,
      );
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
      _desktopShortcut(LogicalKeyboardKey.comma): () =>
          unawaited(controller.openSettings()),
      _desktopShortcut(LogicalKeyboardKey.keyQ): () =>
          unawaited(controller.quit()),
    };
    for (final entry in controller.shortcuts.entries) {
      bindings[_desktopShortcut(entry.key)] = () =>
          unawaited(controller.activate(entry.value));
    }
    return bindings;
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
                if (shortcut != null)
                  MenuShortcut(
                    activator: _desktopShortcut(shortcut),
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

SingleActivator _desktopShortcut(LogicalKeyboardKey key) => SingleActivator(
  key,
  meta: Platform.isMacOS,
  control: Platform.isWindows || Platform.isLinux,
);

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
      return SecretSpoiler(
        key: ValueKey(
          'quick-spoiler-${clip.id}-${controller.presentationGeneration}',
        ),
        reveal: (_) => FutureBuilder<HistoryClip>(
          future: controller.history.revealClip(clip.id),
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
    if (clip.contentKind != HistoryClipKind.image) {
      return Text(
        clip.preview.isEmpty ? clip.contentKind.label : clip.preview,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      );
    }
    return FutureBuilder<HistoryImagePreview?>(
      future: controller.history.requestImagePreview(clip.id, maxEdge: 480),
      builder: (context, snapshot) {
        final preview = snapshot.data;
        return Align(
          alignment: Alignment.centerLeft,
          child: ConstrainedBox(
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
                        errorBuilder: (context, error, stackTrace) =>
                            const Icon(LucideIcons.imageOff),
                      ),
                    ),
                  ),
          ),
        );
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
    return ButtonStyleOverride(
      decoration: AppTheme.actionButtonDecoration,
      child: Column(
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
                  activator: _desktopShortcut(LogicalKeyboardKey.comma),
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
                  activator: _desktopShortcut(LogicalKeyboardKey.keyQ),
                  combiner: const SizedBox.shrink(),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  static Future<void> confirmClear(
    BuildContext context,
    QuickPasteController controller,
  ) {
    return AppOverlays.showDialog<void>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.trash2,
        title: const Text('Clear history?'),
        content: const Text('Pinned clips will stay in your history.'),
        actions: [
          Button.secondary(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Cancel'),
          ),
          Button.destructive(
            onPressed: () async {
              final cleared = await controller.clearUnpinned();
              if (cleared && dialogContext.mounted) {
                Navigator.pop(dialogContext);
              }
            },
            child: const Text('Clear'),
          ),
        ],
      ),
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
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }
}
