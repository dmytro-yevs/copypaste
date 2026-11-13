import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../settings/view/confirmation_dialog.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../controller/snippets_controller.dart';
import '../controller/template_fill_controller.dart';
import '../models/snippet_models.dart';
import '../presentation/snippet_messages.dart';
import 'snippet_collection_editor.dart';
import 'snippet_dialogs.dart';
import 'snippet_editor_drawer.dart';
import 'snippet_transfer_dialogs.dart';
import 'snippets_search_targets.dart';
import 'template_fill_form.dart';

/// Settings → Snippets: the library with collections, Pinned and Recently
/// Deleted, the attention banner for duplicates and conflict copies, and
/// the snippet preferences. It loads while shown and releases on close.
class SnippetsSettingsView extends StatefulWidget {
  const SnippetsSettingsView({
    super.key,
    required this.controller,
    required this.targets,
    this.scope = const SettingsSectionScope(highlightedTargetId: null),
    this.files = const SnippetTransferFiles(),
  });

  final SnippetsController controller;
  final SnippetsSettingsTargets targets;
  final SettingsSectionScope scope;
  final SnippetTransferFiles files;

  @override
  State<SnippetsSettingsView> createState() => _SnippetsSettingsViewState();
}

class _SnippetsSettingsViewState extends State<SnippetsSettingsView> {
  SnippetsController get _controller => widget.controller;

  /// Android has no modifier keys; a long press starts selection.
  static bool get _touch => defaultTargetPlatform == TargetPlatform.android;

  @override
  void initState() {
    super.initState();
    _controller.attach();
    _controller.editorRequest.addListener(_openRequested);
    WidgetsBinding.instance.addPostFrameCallback((_) => _openRequested());
  }

  @override
  void dispose() {
    _controller.editorRequest.removeListener(_openRequested);
    _controller.detach();
    super.dispose();
  }

  void _openRequested() {
    final request = _controller.editorRequest.value;
    if (request == null || !mounted) return;
    _controller.editorRequest.value = null;
    final id = request.snippetId;
    if (id == null) {
      unawaited(_newSnippet(abbreviation: request.abbreviation ?? ''));
    } else {
      unawaited(_edit(id));
    }
  }

  void _run(Future<bool> Function() change) => unawaited(change());

  Future<void> _newSnippet({String abbreviation = ''}) =>
      showSnippetEditorDrawer(
        context,
        controller: _controller,
        abbreviation: abbreviation,
        collectionId: _controller.scope.collectionId,
        onDeleted: _deleted,
      );

  Future<void> _edit(String id) async {
    final SnippetDetail detail;
    try {
      detail = await _controller.detail(id);
    } on Object {
      if (mounted) _toastFailure();
      return;
    }
    if (!mounted) return;
    await showSnippetEditorDrawer(
      context,
      controller: _controller,
      snippet: detail,
      onDeleted: _deleted,
    );
  }

  void _toastFailure() => AppToast.show(
    context,
    title: context.l10n.appActionFailed,
    message: _controller.errorMessage ?? context.l10n.snippetsLoadFailed,
    tone: AppToastTone.error,
  );

  void _deleted(String id) => _undoToast([id]);

  void _undoToast(List<String> ids) {
    if (!mounted) return;
    final l10n = context.l10n;
    AppToast.show(
      context,
      title: l10n.snippetsDeletedTitle(ids.length),
      message: l10n.snippetsDeletedMessage,
      actionLabel: l10n.snippetsUndo,
      onAction: () => _run(() => _controller.undoDelete(ids)),
    );
  }

  Future<void> _delete(List<String> ids) async {
    if (await _controller.delete(ids)) _undoToast(ids);
  }

  Future<void> _purge(List<String> ids) async {
    final l10n = context.l10n;
    final confirmed = await showDestructiveConfirmation(
      context,
      icon: LucideIcons.trash2,
      title: l10n.snippetsPurgeTitle(ids.length),
      message: l10n.snippetsPurgeMessage,
      confirmLabel: l10n.snippetsPurge,
    );
    if (confirmed) _run(() => _controller.purge(ids));
  }

  Future<void> _copy(SnippetSummary snippet) async {
    if (!await ensureSnippetAccess(context, _controller, snippet)) return;
    if (!mounted) return;
    final l10n = context.l10n;
    var copied = false;
    if (snippet.template) {
      final SnippetDetail detail;
      try {
        detail = await _controller.detail(snippet.id);
      } on Object {
        if (mounted) _toastFailure();
        return;
      }
      if (!mounted) return;
      if (detail.fields.isNotEmpty) {
        final fill = TemplateFillController(
          repository: _controller.repository,
          snippet: detail,
        );
        try {
          copied = await showTemplateFillDrawer(context, controller: fill);
        } finally {
          fill.dispose();
        }
      } else {
        copied = await _controller.copy(snippet.id);
      }
    } else {
      copied = await _controller.copy(snippet.id);
    }
    if (!mounted) return;
    if (copied) {
      AppToast.show(
        context,
        title: l10n.snippetsCopied,
        message: l10n.snippetTitle(snippet),
        tone: AppToastTone.success,
      );
    } else if (_controller.errorMessage != null) {
      _toastFailure();
    }
  }

  Future<void> _import() async {
    final path = await widget.files.chooseImport(context.l10n);
    if (path == null || !mounted) return;
    await showSnippetImportDialog(context, controller: _controller, path: path);
  }

  void _activate(SnippetSummary snippet) {
    final keys = HardwareKeyboard.instance;
    if (_controller.selecting ||
        keys.isMetaPressed ||
        keys.isControlPressed ||
        keys.isShiftPressed) {
      _controller.toggleSelection(snippet.id, range: keys.isShiftPressed);
      return;
    }
    if (snippet.deleted) return;
    unawaited(_edit(snippet.id));
  }

  @override
  Widget build(BuildContext context) {
    final mod = defaultTargetPlatform == TargetPlatform.macOS;
    return CallbackShortcuts(
      bindings: {
        SingleActivator(
          LogicalKeyboardKey.keyN,
          meta: mod,
          control: !mod,
        ): () =>
            unawaited(_newSnippet()),
      },
      child: ListenableBuilder(
        listenable: _controller,
        builder: (context, _) {
          final l10n = context.l10n;
          switch (_controller.status) {
            case SnippetsStatus.idle || SnippetsStatus.loading:
              return const StateView.loading();
            case SnippetsStatus.failed:
              return StateView.error(
                title: l10n.snippetsLoadFailed,
                message: _controller.errorMessage,
                actionLabel: l10n.commonTryAgain,
                onAction: () => unawaited(_controller.retry()),
              );
            case SnippetsStatus.ready:
              break;
          }
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (_controller.errorMessage case final error?) ...[
                Alert.destructive(
                  leading: const AppIconFrame(
                    child: Icon(LucideIcons.circleAlert),
                  ),
                  title: Text(l10n.appActionFailed),
                  content: Text(error),
                ),
                const Gap(AppSpacing.lg),
              ],
              _library(context),
              const Gap(AppSpacing.xxl),
              SettingsGroup(
                children: [
                  _transferRow(context),
                  _recordPastedRow(context),
                  _requireAuthRow(context),
                ],
              ),
            ],
          );
        },
      ),
    );
  }

  Widget _library(BuildContext context) {
    final l10n = context.l10n;
    final attention = _controller.attentionCount;
    return Column(
      key: widget.targets.library,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _toolbar(context),
        const Gap(AppSpacing.md),
        _collectionsBar(context),
        if (_controller.total >= SnippetLimits.softLimit) ...[
          const Gap(AppSpacing.md),
          Alert(
            leading: const AppIconFrame(child: Icon(LucideIcons.info)),
            content: Text(l10n.snippetsSoftLimit(_controller.total)),
          ),
        ],
        if (attention > 0 || _controller.attentionOnly) ...[
          const Gap(AppSpacing.md),
          Alert(
            key: const ValueKey('snippets-attention'),
            leading: const AppIconFrame(child: Icon(LucideIcons.triangleAlert)),
            content: Text(l10n.snippetsAttention(attention)),
            trailing: Button.ghost(
              style: AppTheme.controlButtonStyle(const ButtonStyle.ghost()),
              onPressed: () =>
                  _controller.setAttentionOnly(!_controller.attentionOnly),
              child: Text(
                _controller.attentionOnly
                    ? l10n.snippetsShowAll
                    : l10n.snippetsShowAttention,
              ),
            ),
          ),
        ],
        if (_controller.selecting) ...[
          const Gap(AppSpacing.md),
          _selectionBar(context),
        ],
        const Gap(AppSpacing.md),
        _list(context),
      ],
    );
  }

  Widget _toolbar(BuildContext context) {
    final l10n = context.l10n;
    return Row(
      children: [
        Expanded(
          child: TextField(
            key: const ValueKey('snippets-search'),
            initialValue: _controller.query,
            placeholder: Text(l10n.snippetsSearchPlaceholder),
            onChanged: _controller.setQuery,
            skipInputFeatureFocusTraversal: true,
            features: const [
              InputFeature.leading(
                AppIconFrame(child: Icon(LucideIcons.search)),
              ),
              InputFeature.clear(),
            ],
          ),
        ),
        const Gap(AppSpacing.sm),
        if (_touch)
          Tooltip(
            tooltip: (context) =>
                TooltipContainer(child: Text(l10n.snippetsNewSnippet)),
            child: Button.primary(
              key: const ValueKey('snippets-new'),
              style: AppTheme.controlButtonStyle(
                const ButtonStyle.primaryIcon(),
              ),
              onPressed: () => unawaited(_newSnippet()),
              child: Semantics(
                label: l10n.snippetsNewSnippet,
                child: const AppIconFrame(child: Icon(LucideIcons.plus)),
              ),
            ),
          )
        else
          Button.primary(
            key: const ValueKey('snippets-new'),
            style: AppTheme.controlButtonStyle(const ButtonStyle.primary()),
            onPressed: () => unawaited(_newSnippet()),
            leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
            child: Text(l10n.snippetsNewSnippet),
          ),
      ],
    );
  }

  Widget _scopeButton({
    required Key key,
    required SnippetListScope scope,
    required IconData icon,
    required String label,
    Color? color,
    bool dropTarget = false,
  }) {
    final selected = !_controller.searching && _controller.scope == scope;
    Widget button(bool hovering) => Button(
      key: key,
      style: AppTheme.controlButtonStyle(
        selected || hovering
            ? const ButtonStyle.secondary()
            : const ButtonStyle.ghost(),
      ),
      onPressed: () => _controller.setScope(scope),
      leading: AppIconFrame(child: Icon(icon, color: color)),
      child: Text(label),
    );
    if (!dropTarget || _touch) return button(false);
    return DragTarget<List<String>>(
      onAcceptWithDetails: (details) =>
          _run(() => _controller.move(details.data, scope.collectionId)),
      builder: (context, candidates, _) => button(candidates.isNotEmpty),
    );
  }

  Widget _collectionsBar(BuildContext context) {
    final l10n = context.l10n;
    final current = _controller.collection(_controller.scope.collectionId);
    return Wrap(
      spacing: AppSpacing.xs,
      runSpacing: AppSpacing.xs,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        _scopeButton(
          key: const ValueKey('snippets-scope-all'),
          scope: SnippetListScope.all,
          icon: LucideIcons.notebookText,
          label: l10n.snippetsScopeAll,
        ),
        _scopeButton(
          key: const ValueKey('snippets-scope-pinned'),
          scope: SnippetListScope.pinned,
          icon: LucideIcons.pin,
          label: l10n.snippetsScopePinned,
        ),
        _scopeButton(
          key: const ValueKey('snippets-scope-unfiled'),
          scope: SnippetListScope.unfiled,
          icon: LucideIcons.inbox,
          label: l10n.snippetsUnfiled,
          dropTarget: true,
        ),
        for (final collection in _controller.collections)
          _scopeButton(
            key: ValueKey('snippets-scope-${collection.id}'),
            scope: SnippetListScope.collection(collection.id),
            icon: SnippetCollectionStyle.icon(collection.icon),
            color: SnippetCollectionStyle.accent(context, collection.accent),
            label: collection.displayName,
            dropTarget: true,
          ),
        _scopeButton(
          key: const ValueKey('snippets-scope-deleted'),
          scope: SnippetListScope.recentlyDeleted,
          icon: LucideIcons.trash,
          label: l10n.snippetsScopeRecentlyDeleted,
        ),
        if (_controller.collections.length < SnippetLimits.maxCollections)
          _iconButton(
            l10n.snippetsNewCollection,
            LucideIcons.folderPlus,
            () => unawaited(
              showSnippetCollectionEditor(context, controller: _controller),
            ),
            key: const ValueKey('snippets-new-collection'),
          ),
        if (current != null) ...[
          _iconButton(
            l10n.snippetsEditCollection,
            LucideIcons.pencil,
            () => unawaited(
              showSnippetCollectionEditor(
                context,
                controller: _controller,
                collection: current,
              ),
            ),
          ),
          if (_controller.collections.length > 1)
            _iconButton(
              l10n.snippetsMergeCollection,
              LucideIcons.merge,
              () => unawaited(
                showMergeSnippetCollectionDialog(
                  context,
                  controller: _controller,
                  collection: current,
                ),
              ),
            ),
          _iconButton(
            l10n.snippetsExportCollection,
            LucideIcons.fileOutput,
            () => unawaited(
              showSnippetExportDialog(
                context,
                controller: _controller,
                files: widget.files,
                collectionId: current.id,
              ),
            ),
          ),
          _iconButton(
            l10n.snippetsDeleteCollection,
            LucideIcons.folderX,
            () => unawaited(
              showDeleteSnippetCollectionDialog(
                context,
                controller: _controller,
                collection: current,
              ),
            ),
          ),
        ],
      ],
    );
  }

  Widget _selectionBar(BuildContext context) {
    final l10n = context.l10n;
    final ids = _controller.selection.toList();
    final deletedScope = _controller.scope == SnippetListScope.recentlyDeleted;
    return Card(
      key: const ValueKey('snippets-selection'),
      theme: AppTheme.settingsGroupCardTheme,
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.sm),
        child: Wrap(
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text(l10n.snippetsSelected(ids.length)).medium(),
            if (deletedScope) ...[
              Button.secondary(
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: _controller.busy
                    ? null
                    : () => _run(() => _controller.restore(ids)),
                child: Text(l10n.snippetsRestore),
              ),
              Button.destructive(
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.destructive(),
                ),
                onPressed: _controller.busy
                    ? null
                    : () => unawaited(_purge(ids)),
                child: Text(l10n.snippetsPurge),
              ),
            ] else ...[
              Select<String>(
                key: const ValueKey('snippets-move'),
                enabled: !_controller.busy,
                placeholder: Text(l10n.snippetsMoveTo),
                onChanged: (id) {
                  if (id != null) {
                    _run(() => _controller.move(ids, id.isEmpty ? null : id));
                  }
                },
                itemBuilder: (context, id) => Text(
                  _controller.collection(id)?.displayName ??
                      l10n.snippetsUnfiled,
                ),
                popup: SelectPopup<String>(
                  items: SelectItemList(
                    children: [
                      SelectItemButton<String>(
                        value: '',
                        child: Text(l10n.snippetsUnfiled),
                      ),
                      for (final collection in _controller.collections)
                        SelectItemButton<String>(
                          value: collection.id,
                          child: Text(collection.displayName),
                        ),
                    ],
                  ),
                ).call,
              ),
              Button.secondary(
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: () => unawaited(
                  showSnippetExportDialog(
                    context,
                    controller: _controller,
                    files: widget.files,
                    snippetIds: ids,
                  ),
                ),
                child: Text(l10n.snippetsExport),
              ),
              Button.destructive(
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.destructive(),
                ),
                onPressed: _controller.busy
                    ? null
                    : () => unawaited(_delete(ids)),
                child: Text(l10n.commonDelete),
              ),
            ],
            Button.ghost(
              style: AppTheme.controlButtonStyle(const ButtonStyle.ghost()),
              onPressed: _controller.clearSelection,
              child: Text(l10n.snippetsClearSelection),
            ),
          ],
        ),
      ),
    );
  }

  Widget _list(BuildContext context) {
    final l10n = context.l10n;
    final pinned = _controller.pinned;
    final rows = _controller.rows;
    if (pinned.isEmpty && rows.isEmpty) return _empty(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (pinned.isNotEmpty) ...[
          Text(l10n.snippetsScopePinned).small().muted(),
          const Gap(AppSpacing.xs),
          SettingsGroup(children: [for (final row in pinned) _row(row)]),
          const Gap(AppSpacing.lg),
          if (rows.isNotEmpty) ...[
            Text(l10n.snippetsScopeAll).small().muted(),
            const Gap(AppSpacing.xs),
          ],
        ],
        if (rows.isNotEmpty)
          SettingsGroup(children: [for (final row in rows) _row(row)]),
        if (_controller.hasMore) ...[
          const Gap(AppSpacing.md),
          Center(
            child: Button.secondary(
              key: const ValueKey('snippets-load-more'),
              style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
              onPressed: _controller.loadingMore
                  ? null
                  : () => unawaited(_controller.loadMore()),
              child: Text(l10n.snippetsLoadMore),
            ),
          ),
        ],
      ],
    );
  }

  Widget _empty(BuildContext context) {
    final l10n = context.l10n;
    final query = _controller.query.trim();
    if (query.isNotEmpty) {
      final prefill = abbreviationProblem(query) == null ? query : '';
      return StateView.empty(
        key: const ValueKey('snippets-empty-search'),
        compact: true,
        icon: LucideIcons.searchX,
        title: l10n.snippetsNoMatch(query),
        actionLabel: l10n.snippetsCreateNamed(query),
        onAction: () => unawaited(_newSnippet(abbreviation: prefill)),
      );
    }
    if (_controller.scope == SnippetListScope.all) {
      return StateView.empty(
        key: const ValueKey('snippets-empty'),
        compact: true,
        icon: LucideIcons.notebookText,
        title: l10n.snippetsEmptyTitle,
        message: l10n.snippetsEmptyMessage,
        actionLabel: l10n.snippetsCreate,
        onAction: () => unawaited(_newSnippet()),
      );
    }
    return StateView.empty(
      compact: true,
      icon: LucideIcons.inbox,
      title: _controller.scope == SnippetListScope.recentlyDeleted
          ? l10n.snippetsRecentlyDeletedEmpty
          : l10n.snippetsScopeEmpty,
    );
  }

  Widget _row(SnippetSummary snippet) {
    final row = _SnippetRow(
      key: ValueKey('snippet-row-${snippet.id}'),
      snippet: snippet,
      selected: _controller.selection.contains(snippet.id),
      selecting: _controller.selecting,
      busy: _controller.busy,
      onActivate: () => _activate(snippet),
      onLongPress: _touch
          ? () => _controller.toggleSelection(snippet.id)
          : null,
      onCopy: () => unawaited(_copy(snippet)),
      onPin: () =>
          _run(() => _controller.setPinned(snippet.id, !snippet.pinned)),
      onDelete: () => unawaited(_delete([snippet.id])),
      onRestore: () => _run(() => _controller.restore([snippet.id])),
      onPurge: () => unawaited(_purge([snippet.id])),
    );
    if (_touch || snippet.deleted) return row;
    final ids = _controller.selection.contains(snippet.id)
        ? _controller.selection.toList()
        : [snippet.id];
    return Draggable<List<String>>(
      data: ids,
      dragAnchorStrategy: pointerDragAnchorStrategy,
      feedback: Card(
        child: Text(context.l10n.snippetsDragging(ids.length)).medium(),
      ),
      child: row,
    );
  }

  Widget _iconButton(
    String label,
    IconData icon,
    VoidCallback? onPressed, {
    Key? key,
  }) => Tooltip(
    tooltip: (context) => TooltipContainer(child: Text(label)),
    child: Button.ghost(
      key: key,
      style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
      onPressed: onPressed,
      child: Semantics(
        label: label,
        child: AppIconFrame(child: Icon(icon)),
      ),
    ),
  );

  Widget _transferRow(BuildContext context) {
    final l10n = context.l10n;
    return SettingRow(
      key: widget.targets.transfer,
      highlighted: widget.scope.isHighlighted(
        SnippetsSettingsTargets.transferId,
      ),
      title: l10n.snippetsTransferTitle,
      description: l10n.snippetsTransferDescription,
      trailing: Wrap(
        spacing: AppSpacing.sm,
        runSpacing: AppSpacing.sm,
        children: [
          Button.secondary(
            key: const ValueKey('snippets-import'),
            style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
            onPressed: () => unawaited(_import()),
            leading: const AppIconFrame(child: Icon(LucideIcons.fileInput)),
            child: Text(l10n.snippetsImport),
          ),
          Button.secondary(
            key: const ValueKey('snippets-export'),
            style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
            onPressed: () => unawaited(
              showSnippetExportDialog(
                context,
                controller: _controller,
                files: widget.files,
              ),
            ),
            leading: const AppIconFrame(child: Icon(LucideIcons.fileOutput)),
            child: Text(l10n.snippetsExport),
          ),
        ],
      ),
    );
  }

  Widget _recordPastedRow(BuildContext context) => SettingRow(
    key: widget.targets.recordPasted,
    highlighted: widget.scope.isHighlighted(
      SnippetsSettingsTargets.recordPastedId,
    ),
    title: context.l10n.snippetsRecordPastedTitle,
    description: context.l10n.snippetsRecordPastedDescription,
    trailing: Switch(
      key: const ValueKey('snippets-record-pasted'),
      value: _controller.preferences.recordPasted,
      onChanged: _controller.busy
          ? null
          : (value) => _run(
              () => _controller.setPreference(
                SnippetPreference.recordPasted,
                value,
              ),
            ),
    ),
  );

  Widget _requireAuthRow(BuildContext context) => SettingRow(
    key: widget.targets.requireAuth,
    highlighted: widget.scope.isHighlighted(
      SnippetsSettingsTargets.requireAuthId,
    ),
    title: context.l10n.snippetsRequireAuthTitle,
    description: context.l10n.snippetsRequireAuthDescription,
    trailing: Switch(
      key: const ValueKey('snippets-require-auth'),
      value: _controller.preferences.requireAuth,
      onChanged: _controller.busy
          ? null
          : (value) => unawaited(_setRequireAuth(value)),
    ),
  );

  /// Turning the protection off needs the device owner, like using a
  /// concealed snippet does.
  Future<void> _setRequireAuth(bool value) async {
    if (!value && !await ensureOwnerAccess(context, _controller)) return;
    await _controller.setPreference(SnippetPreference.requireAuth, value);
  }
}

class _SnippetRow extends StatelessWidget {
  const _SnippetRow({
    super.key,
    required this.snippet,
    required this.selected,
    required this.selecting,
    required this.busy,
    required this.onActivate,
    required this.onLongPress,
    required this.onCopy,
    required this.onPin,
    required this.onDelete,
    required this.onRestore,
    required this.onPurge,
  });

  final SnippetSummary snippet;
  final bool selected;
  final bool selecting;
  final bool busy;
  final VoidCallback onActivate;
  final VoidCallback? onLongPress;
  final VoidCallback onCopy;
  final VoidCallback onPin;
  final VoidCallback onDelete;
  final VoidCallback onRestore;
  final VoidCallback onPurge;

  Widget _action(
    BuildContext context,
    String label,
    IconData icon,
    VoidCallback? onPressed, {
    Key? key,
  }) => Tooltip(
    tooltip: (context) => TooltipContainer(child: Text(label)),
    child: Button.ghost(
      key: key,
      style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
      onPressed: onPressed,
      child: Semantics(
        label: label,
        child: AppIconFrame(child: Icon(icon)),
      ),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final abbreviation = snippet.abbreviation;
    final preview = snippet.concealed
        ? l10n.snippetsConcealedPreview
        : snippet.preview.trim().split('\n').first;
    final badges = [
      if (abbreviation != null && abbreviation.isNotEmpty)
        SecondaryBadge(
          child: Semantics(
            label: l10n.snippetsAbbreviationBadge(abbreviation),
            child: Text(abbreviation),
          ),
        ),
      if (snippet.template)
        OutlineBadge(child: Text(l10n.snippetsTemplateBadge)),
      if (snippet.duplicateAbbreviation)
        DestructiveBadge(child: Text(l10n.snippetsDuplicateBadge)),
      if (snippet.conflictCopy)
        DestructiveBadge(child: Text(l10n.snippetsConflictBadge)),
    ];
    final actions = snippet.deleted
        ? [
            _action(
              context,
              l10n.snippetsRestore,
              LucideIcons.rotateCcw,
              busy ? null : onRestore,
              key: ValueKey('snippet-restore-${snippet.id}'),
            ),
            _action(
              context,
              l10n.snippetsPurge,
              LucideIcons.trash2,
              busy ? null : onPurge,
            ),
          ]
        : [
            _action(
              context,
              l10n.commonCopy,
              LucideIcons.copy,
              busy ? null : onCopy,
              key: ValueKey('snippet-copy-${snippet.id}'),
            ),
            _action(
              context,
              snippet.pinned ? l10n.snippetsUnpin : l10n.commonPin,
              snippet.pinned ? LucideIcons.pinOff : LucideIcons.pin,
              busy ? null : onPin,
            ),
            _action(
              context,
              l10n.commonDelete,
              LucideIcons.trash2,
              busy ? null : onDelete,
              key: ValueKey('snippet-delete-${snippet.id}'),
            ),
          ];
    return Row(
      children: [
        Expanded(
          child: Button(
            style: AppTheme.historyClipButtonStyle(selected: selected),
            alignment: Alignment.centerLeft,
            onPressed: onActivate,
            onLongPressStart: onLongPress == null
                ? null
                : (_) => onLongPress!(),
            leading: selecting
                ? AppIconFrame(
                    child: Icon(
                      selected ? LucideIcons.squareCheck : LucideIcons.square,
                    ),
                  )
                : AppIconFrame(
                    child: Icon(
                      snippet.concealed
                          ? LucideIcons.lock
                          : snippet.template
                          ? LucideIcons.braces
                          : LucideIcons.notebookText,
                    ),
                  ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Wrap(
                  spacing: AppSpacing.xs,
                  runSpacing: AppSpacing.xs,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(
                      l10n.snippetTitle(snippet),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ).medium(),
                    ...badges,
                  ],
                ),
                if (preview.isNotEmpty) ...[
                  const Gap(AppSpacing.xxs),
                  Text(
                    preview,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ).small().muted(),
                ],
              ],
            ),
          ),
        ),
        ...actions,
      ],
    );
  }
}
