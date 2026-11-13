import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/adaptive_breakpoints.dart';
import '../controller/snippet_editor_controller.dart';
import '../controller/snippets_controller.dart';
import '../models/snippet_models.dart';
import '../presentation/snippet_messages.dart';
import 'snippet_dialogs.dart';
import 'snippet_placeholders.dart';

/// Opens the editor for [snippet], or for a new snippet. [onDeleted] runs
/// with the id after the snippet moved to Recently Deleted.
Future<void> showSnippetEditorDrawer(
  BuildContext context, {
  required SnippetsController controller,
  SnippetDetail? snippet,
  String abbreviation = '',
  String? collectionId,
  ValueChanged<String>? onDeleted,
}) => showOverlay<void>(
  context,
  AppOverlays.drawerConfiguration(context, guarded: true),
  builder: (context) => SnippetEditorDrawer(
    controller: controller,
    snippet: snippet,
    abbreviation: abbreviation,
    collectionId: collectionId,
    onDeleted: onDeleted,
  ),
).future;

/// Title, abbreviation, collection, switches and body of one snippet, with
/// Save, Cancel and Delete in a fixed footer. Closing with unsaved changes
/// asks first, including the Android back gesture.
class SnippetEditorDrawer extends StatefulWidget {
  const SnippetEditorDrawer({
    super.key,
    required this.controller,
    this.snippet,
    this.abbreviation = '',
    this.collectionId,
    this.onDeleted,
  });

  final SnippetsController controller;
  final SnippetDetail? snippet;
  final String abbreviation;
  final String? collectionId;
  final ValueChanged<String>? onDeleted;

  @override
  State<SnippetEditorDrawer> createState() => _SnippetEditorDrawerState();
}

class _SnippetEditorDrawerState extends State<SnippetEditorDrawer> {
  late final SnippetEditorController _editor = SnippetEditorController(
    repository: widget.controller.repository,
    initial: widget.snippet,
    collectionId: widget.collectionId,
    abbreviation: widget.abbreviation,
  );
  late final PlaceholderTextController _body = PlaceholderTextController(
    text: widget.snippet?.bodyText ?? '',
  )..highlight = widget.snippet?.summary.template ?? false;
  final _bodyFocus = FocusNode(debugLabel: 'snippet-body');
  late bool _revealed = !(widget.snippet?.summary.concealed ?? false);
  int _tab = 0;
  bool _closing = false;

  @override
  void initState() {
    super.initState();
    _body.addListener(() => _editor.setBody(_body.text));
  }

  @override
  void dispose() {
    _editor.dispose();
    _body.dispose();
    _bodyFocus.dispose();
    super.dispose();
  }

  Future<void> _close() async {
    if (_closing || _editor.saving) return;
    if (_editor.dirty) {
      final choice = await showUnsavedChangesDialog(context);
      if (!mounted) return;
      switch (choice) {
        case UnsavedChoice.cancel:
          return;
        case UnsavedChoice.save:
          await _save();
          return;
        case UnsavedChoice.discard:
          break;
      }
    }
    _dismiss();
  }

  void _dismiss() {
    if (_closing || !mounted) return;
    _closing = true;
    unawaited(closeDrawer(context));
  }

  Future<void> _save() async {
    if (!_editor.canSave) return;
    var result = await _editor.save();
    if (!mounted) return;
    if (result == SnippetEditorSaveResult.needsReferenceChoice) {
      final l10n = context.l10n;
      final update = await showSnippetQuestion(
        context,
        icon: LucideIcons.replace,
        title: l10n.snippetsUpdateReferencesTitle,
        message: l10n.snippetsUpdateReferencesMessage(
          widget.snippet?.summary.abbreviation ?? '',
          _editor.abbreviation.trim(),
        ),
        confirmLabel: l10n.snippetsUpdateReferences,
        declineLabel: l10n.snippetsKeepReferences,
      );
      if (!mounted) return;
      result = await _editor.save(updateReferences: update);
      if (!mounted) return;
    }
    if (result == SnippetEditorSaveResult.saved) _dismiss();
  }

  Future<void> _delete() async {
    final id = widget.snippet?.id;
    if (id == null || !await _editor.delete() || !mounted) return;
    _dismiss();
    widget.onDeleted?.call(id);
  }

  Future<void> _reveal() async {
    final snippet = widget.snippet;
    if (snippet == null) return;
    if (await ensureSnippetAccess(
      context,
      widget.controller,
      snippet.summary,
    )) {
      if (mounted) setState(() => _revealed = true);
    }
  }

  Future<void> _goTo(String id) async {
    await _close();
    if (_closing) widget.controller.requestEdit(id);
  }

  Future<void> _insertPlaceholder() async {
    if (!_revealed) return;
    final l10n = context.l10n;
    if (!_editor.template && _body.text.contains('{{')) {
      final enable = await showSnippetQuestion(
        context,
        icon: LucideIcons.braces,
        title: l10n.snippetsEnableTemplateTitle,
        message: l10n.snippetsEnableTemplateMessage,
        confirmLabel: l10n.snippetsEnableTemplate,
      );
      if (!enable || !mounted) return;
    }
    final abbreviations = [
      for (final row in [
        ...widget.controller.pinned,
        ...widget.controller.rows,
      ])
        if (row.abbreviation case final abbreviation?
            when row.id != widget.snippet?.id && !row.deleted)
          abbreviation,
    ];
    final text = await showPlaceholderPalette(
      context,
      abbreviations: abbreviations,
    );
    if (text == null || !mounted) return;
    _setTemplate(true);
    _body.insertAtSelection(text);
    _bodyFocus.requestFocus();
  }

  void _setTemplate(bool value) {
    _editor.setTemplate(value);
    _body.highlight = value;
    // Re-tints the body without changing its text or selection.
    _body.value = _body.value.copyWith();
  }

  void _focusLint(TemplateLint lint) {
    final offset = lint.textOffsetIn(_body.text);
    _bodyFocus.requestFocus();
    _body.selection = TextSelection.collapsed(offset: offset);
    if (_tab != 0) setState(() => _tab = 0);
  }

  static BoxConstraints _constraints(BuildContext context) {
    final size = MediaQuery.sizeOf(context);
    if (size.width < AdaptiveBreakpoints.inspector) {
      return AppOverlays.drawerContentConstraints(context);
    }
    return BoxConstraints.tightFor(
      width: math.min(
        AppOverlaySize.editorPanelMaxWidth,
        math.max(AppOverlaySize.drawerPanelWidth, size.width * 0.6),
      ),
      height: double.infinity,
    );
  }

  @override
  Widget build(BuildContext context) {
    final mod = defaultTargetPlatform == TargetPlatform.macOS;
    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) unawaited(_close());
      },
      child: AppOverlays.drawerDismissal(
        onDismiss: () => unawaited(_close()),
        child: CallbackShortcuts(
          bindings: {
            SingleActivator(
              LogicalKeyboardKey.keyS,
              meta: mod,
              control: !mod,
            ): () =>
                unawaited(_save()),
            SingleActivator(
              LogicalKeyboardKey.slash,
              meta: mod,
              control: !mod,
            ): () =>
                unawaited(_insertPlaceholder()),
          },
          child: Focus(
            autofocus: true,
            skipTraversal: true,
            child: ListenableBuilder(
              listenable: Listenable.merge([_editor, widget.controller]),
              builder: (context, _) => ConstrainedBox(
                key: const ValueKey('snippet-editor-drawer'),
                constraints: _constraints(context),
                child: Scaffold(
                  headers: [_header(context), const Divider()],
                  footers: [const Divider(), _footer(context)],
                  child: SingleChildScrollView(
                    key: const ValueKey('snippet-editor-body-scroll'),
                    padding: const EdgeInsets.all(AppSpacing.lg),
                    child: _fields(context),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _header(BuildContext context) {
    final l10n = context.l10n;
    return AppBar(
      title: Text(
        _editor.isNew ? l10n.snippetsNewSnippet : l10n.snippetsEditSnippet,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      leading: const [AppIconFrame(child: Icon(LucideIcons.notebookText))],
      trailing: [
        Tooltip(
          tooltip: (context) => TooltipContainer(child: Text(l10n.commonClose)),
          child: Button.ghost(
            key: const ValueKey('snippet-editor-close'),
            style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
            onPressed: () => unawaited(_close()),
            child: Semantics(
              label: l10n.commonClose,
              child: const AppIconFrame(child: Icon(LucideIcons.x)),
            ),
          ),
        ),
      ],
    );
  }

  Widget _footer(BuildContext context) {
    final l10n = context.l10n;
    final block = _editor.saveBlock;
    return Padding(
      padding: const EdgeInsets.all(AppSpacing.lg),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (block != null && (_editor.dirty || _editor.isNew)) ...[
            Semantics(
              liveRegion: true,
              child: Text(
                l10n.snippetSaveBlock(
                  block,
                  owner: _editor.takenBy?.title ?? '',
                ),
                key: const ValueKey('snippet-editor-save-reason'),
              ).small().muted(),
            ),
            const Gap(AppSpacing.sm),
          ],
          Row(
            children: [
              if (!_editor.isNew) ...[
                Button.destructive(
                  key: const ValueKey('snippet-editor-delete'),
                  style: AppTheme.actionButtonStyle(
                    const ButtonStyle.destructive(),
                  ),
                  onPressed: _editor.saving ? null : () => unawaited(_delete()),
                  child: Text(l10n.commonDelete),
                ),
                const Gap(AppSpacing.md),
              ],
              Expanded(
                child: Button.secondary(
                  key: const ValueKey('snippet-editor-cancel'),
                  style: AppTheme.actionButtonStyle(
                    const ButtonStyle.secondary(),
                  ),
                  alignment: Alignment.center,
                  onPressed: _editor.saving ? null : () => unawaited(_close()),
                  child: Text(l10n.commonCancel),
                ),
              ),
              const Gap(AppSpacing.md),
              Expanded(
                child: Button.primary(
                  key: const ValueKey('snippet-editor-save'),
                  style: AppTheme.actionButtonStyle(
                    const ButtonStyle.primary(),
                  ),
                  alignment: Alignment.center,
                  onPressed: _editor.canSave ? () => unawaited(_save()) : null,
                  child: Text(l10n.commonSave),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _label(String text) => Padding(
    padding: const EdgeInsets.only(bottom: AppSpacing.xs),
    child: Text(text).small(),
  );

  Widget _switch(String label, bool value, ValueChanged<bool> onChanged) =>
      MergeSemantics(
        child: Row(
          children: [
            Expanded(child: Text(label)),
            const Gap(AppSpacing.md),
            AppSwitchFrame(
              child: Switch(
                value: value,
                onChanged: _editor.saving ? null : onChanged,
              ),
            ),
          ],
        ),
      );

  Widget _fields(BuildContext context) {
    final l10n = context.l10n;
    final collections = widget.controller.collections;
    final abbreviation = _editor.abbreviation.trim();
    final problem = abbreviation.isEmpty
        ? null
        : abbreviationProblem(abbreviation);
    final owner = _editor.takenBy;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (_editor.errorMessage case final error?) ...[
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(l10n.snippetsSaveFailed),
            content: Text(error),
          ),
          const Gap(AppSpacing.md),
        ],
        if (_editor.formattingNotice) ...[
          Alert(
            leading: const AppIconFrame(child: Icon(LucideIcons.info)),
            content: Text(l10n.snippetsFormattingNotice),
          ),
          const Gap(AppSpacing.md),
        ],
        _label(l10n.snippetsTitleLabel),
        TextField(
          key: const ValueKey('snippet-editor-title'),
          initialValue: _editor.title,
          maxLength: SnippetLimits.maxTitle,
          placeholder: Text(l10n.snippetsTitlePlaceholder),
          decoration: AppOverlays.dialogFieldDecoration(context),
          onChanged: _editor.setTitle,
        ),
        const Gap(AppSpacing.md),
        _label(l10n.snippetsAbbreviationLabel),
        TextField(
          key: const ValueKey('snippet-editor-abbreviation'),
          initialValue: _editor.abbreviation,
          autocorrect: false,
          enableSuggestions: false,
          placeholder: Text(l10n.snippetsAbbreviationPlaceholder),
          decoration: AppOverlays.dialogFieldDecoration(context),
          onChanged: _editor.setAbbreviation,
        ),
        if (problem != null || owner != null) ...[
          const Gap(AppSpacing.xs),
          Row(
            children: [
              Expanded(
                child: Text(
                  problem == AbbreviationProblem.tooLong
                      ? l10n.snippetsAbbreviationTooLong(
                          SnippetLimits.maxAbbreviationGraphemes,
                        )
                      : problem == AbbreviationProblem.whitespace
                      ? l10n.snippetsAbbreviationWhitespace
                      : l10n.snippetsAbbreviationTaken(owner!.title),
                  key: const ValueKey('snippet-editor-abbreviation-problem'),
                ).small().muted(),
              ),
              if (problem == null && owner != null)
                Button.link(
                  key: const ValueKey('snippet-editor-go-to'),
                  onPressed: () => unawaited(_goTo(owner.id)),
                  child: Text(l10n.snippetsGoToSnippet),
                ),
            ],
          ),
        ] else if (widget.snippet?.summary.duplicateAbbreviation ?? false) ...[
          const Gap(AppSpacing.xs),
          Text(l10n.snippetsDuplicateAbbreviation).small().muted(),
        ],
        const Gap(AppSpacing.md),
        _label(l10n.snippetsCollectionLabel),
        Select<String>(
          key: const ValueKey('snippet-editor-collection'),
          value: _editor.collectionId ?? '',
          onChanged: (id) =>
              _editor.setCollection(id == null || id.isEmpty ? null : id),
          itemBuilder: (context, id) => Text(
            id.isEmpty
                ? l10n.snippetsUnfiled
                : widget.controller.collection(id)?.displayName ??
                      l10n.snippetsUnfiled,
          ),
          popup: SelectPopup<String>(
            items: SelectItemList(
              children: [
                SelectItemButton<String>(
                  value: '',
                  child: Text(l10n.snippetsUnfiled),
                ),
                for (final collection in collections)
                  SelectItemButton<String>(
                    value: collection.id,
                    child: Text(collection.displayName),
                  ),
              ],
            ),
          ).call,
        ),
        const Gap(AppSpacing.lg),
        _switch(l10n.snippetsTemplateSwitch, _editor.template, _setTemplate),
        const Gap(AppSpacing.sm),
        _switch(l10n.snippetsPinnedSwitch, _editor.pinned, _editor.setPinned),
        const Gap(AppSpacing.sm),
        _switch(
          l10n.snippetsConcealedSwitch,
          _editor.concealed,
          _editor.setConcealed,
        ),
        const Gap(AppSpacing.lg),
        _bodySection(context),
      ],
    );
  }

  Widget _bodySection(BuildContext context) {
    final l10n = context.l10n;
    if (!_revealed) {
      return Card(
        theme: AppTheme.settingsGroupCardTheme,
        child: Padding(
          padding: const EdgeInsets.all(AppSpacing.lg),
          child: Row(
            children: [
              const AppIconFrame(child: Icon(LucideIcons.lock)),
              const Gap(AppSpacing.md),
              Expanded(child: Text(l10n.snippetsConcealedBody)),
              Button.secondary(
                key: const ValueKey('snippet-editor-reveal'),
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: () => unawaited(_reveal()),
                child: Text(l10n.snippetsShow),
              ),
            ],
          ),
        ),
      );
    }
    return LayoutBuilder(
      builder: (context, constraints) {
        final wide =
            constraints.maxWidth >= AppOverlaySize.editorSideBySideMinWidth;
        _editor.setPreviewVisible(wide || _tab == 1);
        final bodyHeader = Row(
          children: [
            Expanded(child: Text(l10n.snippetsBodyLabel).small()),
            Button.secondary(
              key: const ValueKey('snippet-editor-insert-placeholder'),
              style: AppTheme.controlButtonStyle(const ButtonStyle.secondary()),
              onPressed: () => unawaited(_insertPlaceholder()),
              leading: const AppIconFrame(child: Icon(LucideIcons.braces)),
              child: Text(l10n.snippetsInsertPlaceholder),
            ),
          ],
        );
        final editor = Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextArea(
              key: const ValueKey('snippet-editor-body'),
              controller: _body,
              focusNode: _bodyFocus,
              expandableHeight: true,
              initialHeight: AppOverlaySize.editorBodyMinHeight,
              minHeight: AppOverlaySize.editorBodyMinHeight,
              placeholder: Text(l10n.snippetsBodyPlaceholder),
              decoration: AppOverlays.dialogFieldDecoration(context),
            ),
            ..._lintList(context),
          ],
        );
        final preview = _preview(context);
        if (wide) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              bodyHeader,
              const Gap(AppSpacing.sm),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(child: editor),
                  const Gap(AppSpacing.md),
                  Expanded(child: preview),
                ],
              ),
            ],
          );
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            bodyHeader,
            const Gap(AppSpacing.sm),
            Tabs(
              index: _tab,
              onChanged: (index) => setState(() => _tab = index),
              children: [
                TabItem(child: Text(l10n.snippetsEditTab)),
                TabItem(child: Text(l10n.snippetsPreviewTab)),
              ],
            ),
            const Gap(AppSpacing.sm),
            Offstage(offstage: _tab != 0, child: editor),
            if (_tab == 1) preview,
          ],
        );
      },
    );
  }

  List<Widget> _lintList(BuildContext context) {
    final l10n = context.l10n;
    final lints = _editor.lints;
    if (lints.isEmpty) return const [];
    return [
      const Gap(AppSpacing.sm),
      for (final lint in lints)
        Button.ghost(
          style: AppTheme.controlButtonStyle(const ButtonStyle.ghost()),
          alignment: Alignment.centerLeft,
          onPressed: () => _focusLint(lint),
          leading: AppIconFrame(
            child: Icon(switch (lint.severity) {
              TemplateLintSeverity.error => LucideIcons.circleX,
              TemplateLintSeverity.warning => LucideIcons.triangleAlert,
              TemplateLintSeverity.hint => LucideIcons.info,
            }),
          ),
          child: Text(l10n.snippetLint(lint)).small(),
        ),
    ];
  }

  Widget _preview(BuildContext context) {
    final l10n = context.l10n;
    final preview = _editor.preview;
    final unfilled = preview?.unfilled ?? 0;
    return Column(
      key: const ValueKey('snippet-editor-preview'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (unfilled > 0) ...[
          Text(l10n.snippetsUnfilledWarning(unfilled)).small().muted(),
          const Gap(AppSpacing.xs),
        ],
        Card(
          theme: AppTheme.settingsGroupCardTheme,
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              minHeight: AppOverlaySize.editorBodyMinHeight,
            ),
            child: Padding(
              padding: const EdgeInsets.all(AppSpacing.md),
              child: Text(
                preview == null || preview.text.isEmpty
                    ? l10n.snippetsPreviewEmpty
                    : preview.text,
              ),
            ),
          ),
        ),
      ],
    );
  }
}
