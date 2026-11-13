import 'dart:async';

import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../../shared/system_date_time.dart';
import '../../history/models/history_models.dart';
import '../../history/presentation/source_app_label.dart';
import '../controller/paste_queue_controller.dart';
import '../controller/paste_queue_shortcuts_controller.dart';
import '../models/paste_queue_models.dart';
import '../presentation/paste_queue_messages.dart';

/// The queue: numbered entries in paste order, notices above them, and the
/// queue actions in a fixed footer. Only the list scrolls.
class PasteQueueDrawer extends StatefulWidget {
  const PasteQueueDrawer({
    super.key,
    required this.controller,
    this.shortcuts,
    this.onOpenSettings,
    this.onClose,
  });

  final PasteQueueController controller;
  final PasteQueueShortcutsController? shortcuts;

  /// Opens the queue settings, where shortcuts are set.
  final VoidCallback? onOpenSettings;

  /// Set when the queue is embedded in another surface instead of a drawer:
  /// it then fills its parent, and Close calls this.
  final VoidCallback? onClose;

  @override
  State<PasteQueueDrawer> createState() => _PasteQueueDrawerState();
}

class _PasteQueueDrawerState extends State<PasteQueueDrawer> {
  final _listFocus = FocusNode(debugLabel: 'paste-queue-list');
  final Map<String, Future<HistorySourceAppIcon?>> _icons = {};
  final Map<String, SortableData<String>> _sortable = {};
  late final VoidCallback _release;
  StreamSubscription<PasteQueueFeedback>? _feedback;
  String? _selected;

  PasteQueueController get _controller => widget.controller;

  @override
  void initState() {
    super.initState();
    _release = _controller.retainEntries();
    _feedback = _controller.feedback.listen(_report);
  }

  @override
  void dispose() {
    _release();
    unawaited(_feedback?.cancel());
    _icons.clear();
    _listFocus.dispose();
    super.dispose();
  }

  void _close() {
    final onClose = widget.onClose;
    if (onClose != null) return onClose();
    unawaited(closeDrawer(context));
  }

  void _report(PasteQueueFeedback feedback) {
    if (!mounted) return;
    final l10n = context.l10n;
    final message = l10n.pasteQueueFeedback(feedback);
    unawaited(
      SemanticsService.sendAnnouncement(
        View.of(context),
        message,
        Directionality.of(context),
      ),
    );
    if (feedback.trigger == PasteQueueTrigger.window && feedback.problem) {
      AppToast.show(
        context,
        title: l10n.pasteQueueSettingsTitle,
        message: message,
        tone: feedback.kind == PasteQueueFeedbackKind.failed
            ? AppToastTone.error
            : AppToastTone.information,
      );
    }
  }

  Future<HistorySourceAppIcon?> _icon(String? iconId) {
    if (iconId == null) return Future.value();
    return _icons.putIfAbsent(iconId, () => _controller.sourceAppIcon(iconId));
  }

  Future<void> _toggleCollecting() async {
    final snapshot = _controller.snapshot;
    if (!snapshot.collecting && snapshot.phase == PasteQueuePhase.finished) {
      final append = await _askRestart();
      if (append == null || !mounted) return;
      await _controller.setCollecting(true, append: append);
      return;
    }
    await _controller.toggleCollecting();
  }

  /// Start fresh (false, the default) or Append (true); null cancels.
  Future<bool?> _askRestart() {
    final l10n = context.l10n;
    return AppOverlays.showDialog<bool>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.listRestart,
        title: Text(l10n.pasteQueueRestartTitle),
        content: Text(l10n.pasteQueueRestartMessage),
        actions: [
          Button.ghost(
            alignment: Alignment.center,
            style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: Text(l10n.commonCancel),
          ),
          Button.secondary(
            key: const ValueKey('paste-queue-restart-append'),
            alignment: Alignment.center,
            style: AppTheme.dialogButtonStyle(const ButtonStyle.secondary()),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.pasteQueueAppend),
          ),
          Button.primary(
            key: const ValueKey('paste-queue-restart-fresh'),
            alignment: Alignment.center,
            style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.pasteQueueStartFresh),
          ),
        ],
      ),
    );
  }

  Future<void> _edit(PasteQueueEntry entry) async {
    final l10n = context.l10n;
    final text = TextEditingController(text: entry.preview);
    try {
      await AppOverlays.showActionDialog(
        context,
        icon: LucideIcons.pencil,
        title: Text(l10n.pasteQueueEditTitle),
        actionLabel: l10n.commonSave,
        destructive: false,
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(l10n.pasteQueueEditMessage),
            const Gap(AppSpacing.sm),
            TextArea(
              key: const ValueKey('paste-queue-edit-text'),
              controller: text,
              autofocus: true,
              decoration: AppOverlays.dialogFieldDecoration(context),
              initialHeight: AppOverlaySize.editorBodyMinHeight,
              minHeight: AppOverlaySize.editorBodyMinHeight,
            ),
          ],
        ),
        onConfirm: () => _controller.edit(entry.id, text.text),
        failureMessage: () => l10n.pasteQueueFailed,
      );
    } finally {
      text.dispose();
    }
  }

  Future<void> _move(PasteQueueEntry entry, int toIndex) async {
    final entries = _controller.snapshot.entries;
    if (toIndex < 0 || toIndex >= entries.length) return;
    if (await _controller.move(entry.id, toIndex) && mounted) {
      unawaited(
        SemanticsService.sendAnnouncement(
          View.of(context),
          context.l10n.pasteQueueMoved(toIndex + 1),
          Directionality.of(context),
        ),
      );
    }
  }

  KeyEventResult _handleKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final entries = _controller.snapshot.entries;
    final keyboard = HardwareKeyboard.instance;
    final key = event.logicalKey;
    final command = keyboard.isMetaPressed || keyboard.isControlPressed;
    if (command && key == LogicalKeyboardKey.keyZ) {
      if (_controller.undoAvailable) unawaited(_controller.undoClear());
      return KeyEventResult.handled;
    }
    if (entries.isEmpty) return KeyEventResult.ignored;
    final index = entries.indexWhere((entry) => entry.id == _selected);
    final entry = index < 0 ? null : entries[index];
    if (key == LogicalKeyboardKey.arrowUp ||
        key == LogicalKeyboardKey.arrowDown) {
      final delta = key == LogicalKeyboardKey.arrowUp ? -1 : 1;
      if (keyboard.isAltPressed && entry != null && entry.pending) {
        unawaited(_move(entry, index + delta));
      } else {
        final next = (index < 0 ? 0 : index + delta).clamp(
          0,
          entries.length - 1,
        );
        setState(() => _selected = entries[next].id);
      }
      return KeyEventResult.handled;
    }
    if (entry == null) return KeyEventResult.ignored;
    if (key == LogicalKeyboardKey.delete ||
        key == LogicalKeyboardKey.backspace) {
      unawaited(_controller.remove(entry.id));
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.enter && entry.pending) {
      unawaited(_controller.pasteEntry(entry.id));
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.keyE && !command && entry.editable) {
      unawaited(_edit(entry));
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {const SingleActivator(LogicalKeyboardKey.escape): _close},
    child: Focus(
      autofocus: true,
      skipTraversal: true,
      child: ListenableBuilder(
        listenable: Listenable.merge([_controller, ?widget.shortcuts]),
        builder: (context, _) => ConstrainedBox(
          key: const ValueKey('paste-queue-drawer'),
          constraints: widget.onClose == null
              ? AppOverlays.drawerContentConstraints(context)
              : const BoxConstraints(),
          child: Scaffold(
            headers: [_header(context), const Divider()],
            footers: [const Divider(), _footer(context)],
            child: _body(context),
          ),
        ),
      ),
    ),
  );

  Widget _header(BuildContext context) {
    final l10n = context.l10n;
    final snapshot = _controller.snapshot;
    final collecting = snapshot.collecting;
    final summary = snapshot.phase == PasteQueuePhase.finished
        ? l10n.pasteQueueFinishedSummary
        : collecting
        ? l10n.pasteQueueSummaryCollecting(_controller.pendingCount)
        : l10n.pasteQueueSummary(_controller.pendingCount);
    return AppBar(
      title: Text(l10n.pasteQueueSettingsTitle, maxLines: 1),
      subtitle: Text(
        summary,
        key: const ValueKey('paste-queue-summary'),
        style: AppTheme.deviceMetadataTextStyle(context),
      ),
      leading: const [AppIconFrame(child: Icon(LucideIcons.listOrdered))],
      trailing: [
        Button.secondary(
          key: const ValueKey('paste-queue-collect'),
          style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
          onPressed:
              _controller.enabled &&
                  _controller.status == PasteQueueLoadStatus.ready
              ? () => unawaited(_toggleCollecting())
              : null,
          leading: AppIconFrame(
            child: Icon(
              collecting ? LucideIcons.circleStop : LucideIcons.circleDot,
            ),
          ),
          child: Text(
            collecting
                ? l10n.pasteQueueStopCollecting
                : l10n.pasteQueueStartCollecting,
          ),
        ),
        _iconButton(
          context,
          label: l10n.commonClose,
          icon: LucideIcons.x,
          onPressed: _close,
        ),
      ],
    );
  }

  Widget _body(BuildContext context) {
    final l10n = context.l10n;
    final snapshot = _controller.snapshot;
    final notices = _notices(context);
    final Widget list;
    if (_controller.status == PasteQueueLoadStatus.failed) {
      list = StateView.error(
        title: l10n.pasteQueueLoadFailed,
        actionLabel: l10n.commonRetry,
        onAction: () => unawaited(_controller.reload()),
      );
    } else if (_controller.status == PasteQueueLoadStatus.loading) {
      list = _controller.loadingVisible
          ? StateView.loading(message: l10n.pasteQueueLoading)
          : const SizedBox.shrink();
    } else if (snapshot.entries.isEmpty) {
      list = snapshot.collecting
          ? StateView.empty(
              key: const ValueKey('paste-queue-collecting-empty'),
              icon: LucideIcons.circleDot,
              title: l10n.pasteQueueCollectingEmptyTitle,
              message: l10n.pasteQueueCollectingEmptyMessage,
              actionLabel: l10n.pasteQueueStopCollecting,
              onAction: () => unawaited(_controller.setCollecting(false)),
            )
          : StateView.empty(
              key: const ValueKey('paste-queue-empty'),
              icon: LucideIcons.listOrdered,
              title: l10n.pasteQueueEmptyTitle,
              message: l10n.pasteQueueEmptyMessage,
              actionLabel: _controller.enabled
                  ? l10n.pasteQueueStartCollecting
                  : null,
              onAction: _controller.enabled
                  ? () => unawaited(_controller.setCollecting(true))
                  : null,
            );
    } else {
      list = _entries(context, snapshot.entries);
    }
    return SortableLayer(
      dropDuration: AppMotion.resolve(context, AppMotion.standard),
      dropCurve: AppMotion.standardCurve,
      child: SingleChildScrollView(
        key: const ValueKey('paste-queue-body'),
        padding: const EdgeInsets.all(AppSpacing.lg),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final notice in notices) ...[notice, const Gap(AppSpacing.md)],
            list,
          ],
        ),
      ),
    );
  }

  List<Widget> _notices(BuildContext context) {
    final l10n = context.l10n;
    final snapshot = _controller.snapshot;
    final shortcuts = widget.shortcuts;
    return [
      if (_controller.banner case final banner?)
        Alert(
          key: const ValueKey('paste-queue-banner'),
          leading: AppIconFrame(
            child: Icon(
              banner == PasteQueueBanner.accessibilityMissing
                  ? LucideIcons.accessibility
                  : LucideIcons.clipboard,
            ),
          ),
          title: Text(l10n.pasteQueueCopyOnlyTitle),
          content: _withActions(
            banner == PasteQueueBanner.accessibilityMissing
                ? l10n.pasteQueueAccessibilityMessage
                : l10n.pasteQueuePasteUnsupported,
            [
              Button.ghost(
                onPressed: _controller.dismissBanner,
                child: Text(l10n.commonDismiss),
              ),
              if (banner == PasteQueueBanner.accessibilityMissing)
                Button.secondary(
                  key: const ValueKey('paste-queue-allow-accessibility'),
                  onPressed: () =>
                      unawaited(_controller.requestAccessibility()),
                  child: Text(l10n.commonOpenSettings),
                ),
            ],
          ),
        ),
      if (shortcuts != null && shortcuts.showsShortcutPrompt)
        Alert(
          key: const ValueKey('paste-queue-shortcut-prompt'),
          leading: const AppIconFrame(child: Icon(LucideIcons.keyboard)),
          title: Text(l10n.pasteQueueShortcutPromptTitle),
          content: _withActions(l10n.pasteQueueShortcutPromptMessage, [
            Button.ghost(
              onPressed: () => unawaited(shortcuts.dismissShortcutPrompt()),
              child: Text(l10n.commonDismiss),
            ),
            if (widget.onOpenSettings case final open?)
              Button.secondary(
                key: const ValueKey('paste-queue-set-shortcuts'),
                onPressed: () {
                  _close();
                  open();
                },
                child: Text(l10n.pasteQueueShortcutPromptTitle),
              ),
          ]),
        ),
      if (_controller.undoAvailable)
        Alert(
          key: const ValueKey('paste-queue-undo'),
          leading: const AppIconFrame(child: Icon(LucideIcons.undo2)),
          title: Text(l10n.pasteQueueCleared),
          content: _withActions(null, [
            Button.secondary(
              key: const ValueKey('paste-queue-undo-clear'),
              onPressed: () => unawaited(_controller.undoClear()),
              child: Text(l10n.pasteQueueUndo),
            ),
          ]),
        ),
      if (snapshot.full)
        Alert(
          key: const ValueKey('paste-queue-full'),
          leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
          title: Text(l10n.pasteQueueFull(snapshot.settings.maxEntries)),
          content: Text(l10n.pasteQueueFullMessage),
        ),
      if (snapshot.staleSince case final since?)
        Alert(
          key: const ValueKey('paste-queue-stale'),
          leading: const AppIconFrame(child: Icon(LucideIcons.clock)),
          title: Text(
            l10n.pasteQueueStale(formatSystemDateTime(context, since)),
          ),
          content: _withActions(null, [
            Button.secondary(
              onPressed: () => unawaited(_controller.clear()),
              child: Text(l10n.commonClear),
            ),
          ]),
        ),
      if (_controller.errorCode != null)
        Alert.destructive(
          key: const ValueKey('paste-queue-error'),
          leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
          title: Text(l10n.pasteQueueFailed),
        ),
    ];
  }

  static Widget _withActions(String? message, List<Widget> actions) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      if (message != null) ...[Text(message), const Gap(AppSpacing.sm)],
      Wrap(
        alignment: WrapAlignment.end,
        spacing: AppSpacing.sm,
        runSpacing: AppSpacing.sm,
        children: actions,
      ),
    ],
  );

  Widget _entries(BuildContext context, List<PasteQueueEntry> entries) {
    final ids = {for (final entry in entries) entry.id};
    _sortable.removeWhere((id, _) => !ids.contains(id));
    if (_selected != null && !ids.contains(_selected)) _selected = null;
    return Focus(
      focusNode: _listFocus,
      onKeyEvent: _handleKey,
      child: Column(
        key: const ValueKey('paste-queue-list'),
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final (index, entry) in entries.indexed)
            Padding(
              padding: const EdgeInsets.only(bottom: AppSpacing.sm),
              child: _sortableRow(context, entries, index, entry),
            ),
        ],
      ),
    );
  }

  Widget _sortableRow(
    BuildContext context,
    List<PasteQueueEntry> entries,
    int index,
    PasteQueueEntry entry,
  ) {
    final row = _row(context, entries, index, entry);
    if (!entry.pending) {
      return KeyedSubtree(key: ValueKey(entry.id), child: row);
    }
    bool accepts(SortableData<String> data) => data.data != entry.id;
    int targetIndex(String id, {required bool before}) {
      final from = entries.indexWhere((candidate) => candidate.id == id);
      final to = before ? index : index + 1;
      return from < to ? to - 1 : to;
    }

    void accept(SortableData<String> data, {required bool before}) {
      final moved = entries.firstWhere(
        (candidate) => candidate.id == data.data,
      );
      unawaited(_move(moved, targetIndex(data.data, before: before)));
    }

    return Sortable<String>(
      key: ValueKey(entry.id),
      data: _sortable.putIfAbsent(entry.id, () => SortableData(entry.id)),
      enabled: false,
      canAcceptTop: accepts,
      canAcceptBottom: accepts,
      onAcceptTop: (data) => accept(data, before: true),
      onAcceptBottom: (data) => accept(data, before: false),
      placeholder: const SizedBox(height: AppControlSize.regular),
      child: row,
    );
  }

  Widget _row(
    BuildContext context,
    List<PasteQueueEntry> entries,
    int index,
    PasteQueueEntry entry,
  ) {
    final l10n = context.l10n;
    final status = l10n.pasteQueueStatusName(entry.status);
    final content = l10n.pasteQueueContent(entry);
    final selected = entry.id == _selected;
    final dimmed = !entry.pending;
    final details = [
      ?status,
      if (entry.edited) l10n.pasteQueueEdited,
      if (entry.renderedAt case final at?)
        l10n.pasteQueueRenderedAt(formatSystemRecentTime(context, at)),
    ];
    final last = entries.length - 1;
    return Semantics(
      label: l10n.pasteQueueRowLabel(entry.position, content),
      value: details.join(', '),
      selected: selected,
      customSemanticsActions: entry.pending
          ? {
              if (index > 0)
                CustomSemanticsAction(label: l10n.pasteQueueMoveUp): () =>
                    unawaited(_move(entry, index - 1)),
              if (index < last)
                CustomSemanticsAction(label: l10n.pasteQueueMoveDown): () =>
                    unawaited(_move(entry, index + 1)),
              if (index > 0)
                CustomSemanticsAction(label: l10n.pasteQueueMoveToTop): () =>
                    unawaited(_move(entry, 0)),
              if (index < last)
                CustomSemanticsAction(label: l10n.pasteQueueMoveToBottom): () =>
                    unawaited(_move(entry, last)),
            }
          : null,
      child: Card(
        key: ValueKey('paste-queue-row-${entry.id}'),
        theme: AppTheme.settingsGroupCardTheme,
        filled: selected,
        padding: const EdgeInsets.all(AppSpacing.sm),
        child: Row(
          children: [
            SizedBox(
              width: AppIconSize.lg,
              child: Text(
                entry.position,
                textAlign: TextAlign.center,
              ).medium().muted(),
            ),
            const Gap(AppSpacing.sm),
            Expanded(
              child: ExcludeSemantics(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    dimmed
                        ? Text(
                            content,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ).muted()
                        : Text(
                            content,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                    if (entry.sourceAppName != null || details.isNotEmpty)
                      Row(
                        children: [
                          if (entry.sourceAppName case final app?)
                            Flexible(
                              child: SourceAppLabel(
                                name: app,
                                icon: _icon(entry.sourceAppIconId),
                                style: AppTheme.historyMetadataTextStyle(
                                  context,
                                ),
                              ),
                            ),
                          if (details.isNotEmpty)
                            Flexible(
                              child: Text(
                                '${entry.sourceAppName == null ? '' : ' · '}'
                                '${details.join(' · ')}',
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: AppTheme.historyMetadataTextStyle(
                                  context,
                                ),
                              ),
                            ),
                        ],
                      ),
                  ],
                ),
              ),
            ),
            if (entry.pending)
              SortableDragHandle(
                child: _iconButton(
                  context,
                  label: l10n.pasteQueueReorder,
                  icon: LucideIcons.gripVertical,
                  onPressed: () {},
                ),
              ),
            Builder(
              builder: (anchor) => _iconButton(
                context,
                key: ValueKey('paste-queue-row-menu-${entry.id}'),
                label: l10n.pasteQueueMoreActions,
                icon: LucideIcons.ellipsis,
                onPressed: () =>
                    unawaited(_showRowMenu(anchor, entries, index, entry)),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _showRowMenu(
    BuildContext anchor,
    List<PasteQueueEntry> entries,
    int index,
    PasteQueueEntry entry,
  ) {
    final l10n = context.l10n;
    final last = entries.length - 1;
    MenuButton item(String label, IconData icon, VoidCallback onPressed) =>
        MenuButton(
          onPressed: (_) => onPressed(),
          leading: AppIconFrame(
            size: AppIconSize.sm,
            child: Icon(icon, size: AppIconSize.sm),
          ),
          child: Text(label),
        );
    return showDropdown<void>(
      context: anchor,
      alignment: AlignmentDirectional.topEnd,
      anchorAlignment: AlignmentDirectional.bottomEnd,
      builder: (popupContext) => AppTheme.popupSurface(
        popupContext,
        child: DropdownMenu(
          children: [
            if (entry.pending) ...[
              item(
                l10n.pasteQueuePasteNow,
                LucideIcons.clipboardPaste,
                () => unawaited(_controller.pasteEntry(entry.id)),
              ),
              if (index > 0) ...[
                item(
                  l10n.pasteQueueMoveUp,
                  LucideIcons.arrowUp,
                  () => unawaited(_move(entry, index - 1)),
                ),
                item(
                  l10n.pasteQueueMoveToTop,
                  LucideIcons.arrowUpToLine,
                  () => unawaited(_move(entry, 0)),
                ),
              ],
              if (index < last) ...[
                item(
                  l10n.pasteQueueMoveDown,
                  LucideIcons.arrowDown,
                  () => unawaited(_move(entry, index + 1)),
                ),
                item(
                  l10n.pasteQueueMoveToBottom,
                  LucideIcons.arrowDownToLine,
                  () => unawaited(_move(entry, last)),
                ),
              ],
            ],
            if (entry.editable)
              item(
                l10n.pasteQueueEdit,
                LucideIcons.pencil,
                () => unawaited(_edit(entry)),
              ),
            if (entry.edited)
              item(
                l10n.pasteQueueUseOriginal,
                LucideIcons.rotateCcw,
                () => unawaited(_controller.edit(entry.id, null)),
              ),
            item(
              l10n.pasteQueueRemove,
              LucideIcons.x,
              () => unawaited(_controller.remove(entry.id)),
            ),
          ],
        ),
      ),
    ).future;
  }

  Widget _footer(BuildContext context) {
    final l10n = context.l10n;
    final snapshot = _controller.snapshot;
    final ready =
        _controller.enabled && _controller.status == PasteQueueLoadStatus.ready;
    final copyOnly = _controller.pasteNextStyle == PasteNextStyle.clipboardOnly;
    final block = snapshot.pasteAllBlock;
    final pending = _controller.pendingCount > 0;
    final finished = snapshot.hasFinishedEntries;
    final settings = snapshot.settings;
    return Padding(
      padding: const EdgeInsets.all(AppSpacing.lg),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Button.primary(
                  key: const ValueKey('paste-queue-paste-next'),
                  style: AppTheme.actionButtonStyle(
                    const ButtonStyle.primary(),
                  ),
                  alignment: Alignment.center,
                  onPressed: ready && pending
                      ? () => unawaited(_controller.pasteNext())
                      : null,
                  child: Text(
                    copyOnly
                        ? l10n.pasteQueueCopyNext
                        : l10n.pasteQueuePasteNext,
                    maxLines: 1,
                  ),
                ),
              ),
              const Gap(AppSpacing.md),
              Expanded(
                child: Tooltip(
                  tooltip: (context) => TooltipContainer(
                    child: Text(
                      block == null
                          ? l10n.pasteQueuePasteAll
                          : l10n.pasteQueueBlock(block),
                    ),
                  ),
                  child: Button.secondary(
                    key: const ValueKey('paste-queue-paste-all'),
                    style: AppTheme.actionButtonStyle(
                      const ButtonStyle.secondary(),
                    ),
                    alignment: Alignment.center,
                    onPressed: ready && block == null
                        ? () => unawaited(
                            copyOnly
                                ? _controller.copyAll()
                                : _controller.pasteAll(),
                          )
                        : null,
                    child: Text(
                      copyOnly
                          ? l10n.pasteQueueCopyAll
                          : l10n.pasteQueuePasteAll,
                      maxLines: 1,
                    ),
                  ),
                ),
              ),
            ],
          ),
          if (block != null && block != PasteAllBlock.nothingPending) ...[
            const Gap(AppSpacing.xs),
            Text(
              l10n.pasteQueueBlock(block),
              key: const ValueKey('paste-queue-paste-all-reason'),
            ).small().muted(),
          ],
          const Gap(AppSpacing.md),
          Row(
            children: [
              Expanded(
                child: Select<PasteQueueSeparator>(
                  key: const ValueKey('paste-queue-separator'),
                  value: settings.separator,
                  enabled: ready,
                  onChanged: (separator) {
                    if (separator == null) return;
                    unawaited(
                      _controller.updateSettings(
                        settings.copyWith(separator: separator),
                      ),
                    );
                  },
                  itemBuilder: (context, separator) =>
                      Text(context.l10n.pasteQueueSeparatorName(separator)),
                  popup: SelectPopup<PasteQueueSeparator>(
                    items: SelectItemList(
                      children: [
                        for (final separator in PasteQueueSeparator.values)
                          if (separator != PasteQueueSeparator.custom ||
                              settings.customSeparator.isNotEmpty)
                            SelectItemButton<PasteQueueSeparator>(
                              value: separator,
                              child: Text(
                                l10n.pasteQueueSeparatorName(separator),
                              ),
                            ),
                      ],
                    ),
                  ).call,
                ),
              ),
              const Gap(AppSpacing.sm),
              _iconButton(
                context,
                key: const ValueKey('paste-queue-reverse'),
                label: l10n.pasteQueueReverse,
                icon: LucideIcons.arrowDownUp,
                onPressed: ready && _controller.pendingCount > 1
                    ? () => unawaited(_controller.reverse())
                    : null,
              ),
              _iconButton(
                context,
                key: const ValueKey('paste-queue-step-back'),
                label: l10n.pasteQueueStepBack,
                icon: LucideIcons.undo2,
                onPressed: ready && finished
                    ? () => unawaited(_controller.stepBack())
                    : null,
              ),
              _iconButton(
                context,
                key: const ValueKey('paste-queue-skip'),
                label: l10n.pasteQueueSkip,
                icon: LucideIcons.skipForward,
                onPressed: ready && pending
                    ? () => unawaited(_controller.skip())
                    : null,
              ),
              _iconButton(
                context,
                key: const ValueKey('paste-queue-remove-pasted'),
                label: l10n.pasteQueueRemovePasted,
                icon: LucideIcons.listX,
                onPressed: ready && finished
                    ? () => unawaited(_controller.removeFinished())
                    : null,
              ),
              _iconButton(
                context,
                key: const ValueKey('paste-queue-clear'),
                label: l10n.commonClear,
                icon: LucideIcons.trash2,
                onPressed: ready && snapshot.entries.isNotEmpty
                    ? () => unawaited(_controller.clear())
                    : null,
              ),
            ],
          ),
        ],
      ),
    );
  }

  static Widget _iconButton(
    BuildContext context, {
    Key? key,
    required String label,
    required IconData icon,
    required VoidCallback? onPressed,
  }) => Tooltip(
    tooltip: (context) => TooltipContainer(child: Text(label)),
    child: Semantics(
      label: label,
      button: true,
      child: Button.ghost(
        key: key,
        style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
        onPressed: onPressed,
        child: AppIconFrame(child: Icon(icon)),
      ),
    ),
  );
}
