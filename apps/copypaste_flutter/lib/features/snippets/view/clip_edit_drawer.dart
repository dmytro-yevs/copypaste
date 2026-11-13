import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../models/snippet_models.dart';
import '../repository/clip_edit_preferences.dart';
import '../repository/snippet_repository.dart';
import 'snippet_dialogs.dart';

/// The text clip [ClipEditDrawer] edits.
class ClipEditTarget {
  const ClipEditTarget({
    required this.id,
    required this.text,
    this.rich = false,
  });

  final String id;
  final String text;

  /// The clip also has a formatted representation.
  final bool rich;

  /// Clips above [SnippetLimits.maxEditableClipBytes] open read-only.
  bool get tooLarge => utf8Length(text) > SnippetLimits.maxEditableClipBytes;
}

enum ClipEditDestination { historyClip, snippet }

/// Edits [clip] into a new clip, or with no clip writes new text to History
/// or to a literal Unfiled snippet. Resolves to the id written, or null.
Future<String?> showClipEditDrawer(
  BuildContext context, {
  required ClipEditRepository clips,
  required SnippetRepository snippets,
  required ClipEditPreferences preferences,
  ClipEditTarget? clip,
}) => showOverlay<String>(
  context,
  AppOverlays.drawerConfiguration(context, guarded: true),
  builder: (context) => ClipEditDrawer(
    clips: clips,
    snippets: snippets,
    preferences: preferences,
    clip: clip,
  ),
).future;

/// One editor for new and edited text (snippets spec S-1, E-1 to E-8).
/// Nothing is written until Save; closing with changes asks first.
class ClipEditDrawer extends StatefulWidget {
  const ClipEditDrawer({
    super.key,
    required this.clips,
    required this.snippets,
    required this.preferences,
    this.clip,
  });

  final ClipEditRepository clips;
  final SnippetRepository snippets;
  final ClipEditPreferences preferences;
  final ClipEditTarget? clip;

  @override
  State<ClipEditDrawer> createState() => _ClipEditDrawerState();
}

class _ClipEditDrawerState extends State<ClipEditDrawer> {
  late final _text = TextEditingController(text: widget.clip?.text ?? '');
  ClipEditDestination _destination = ClipEditDestination.historyClip;
  bool _replaceOriginal = false;
  bool _saving = false;
  bool _closing = false;
  String? _error;

  bool get _editing => widget.clip != null;
  bool get _readOnly => widget.clip?.tooLarge ?? false;
  bool get _dirty => !_readOnly && _text.text != (widget.clip?.text ?? '');
  bool get _empty => _text.text.trim().isEmpty;

  @override
  void initState() {
    super.initState();
    _text.addListener(() => setState(() {}));
    if (_editing) {
      unawaited(
        widget.preferences.replaceOriginal().then((value) {
          if (mounted) setState(() => _replaceOriginal = value);
        }),
      );
    }
  }

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  Future<void> _close() async {
    if (_closing || _saving) return;
    if (_dirty) {
      final choice = await showUnsavedChangesDialog(context);
      if (!mounted) return;
      if (choice == UnsavedChoice.cancel) return;
      if (choice == UnsavedChoice.save) {
        await _save();
        return;
      }
    }
    _finish(null);
  }

  void _finish(String? id) {
    if (_closing || !mounted) return;
    _closing = true;
    unawaited(closeDrawer(context, id));
  }

  Future<void> _save() async {
    if (_saving || _empty || _readOnly) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    final text = normalizeLineEndings(_text.text);
    try {
      final String id;
      if (widget.clip case final clip?) {
        await widget.preferences.setReplaceOriginal(_replaceOriginal);
        id = await widget.clips.saveClipEdit(
          clip.id,
          text,
          replaceOriginal: _replaceOriginal,
        );
      } else if (_destination == ClipEditDestination.snippet) {
        final outcome = await widget.snippets.save(
          SnippetDraft(bodyText: text),
        );
        id = switch (outcome) {
          SnippetSaved(:final detail) => detail.id,
          SnippetAbbreviationTaken() => throw StateError('no abbreviation'),
        };
      } else {
        id = await widget.clips.createManualClip(text);
      }
      if (!mounted) return;
      setState(() => _saving = false);
      _finish(id);
    } on Object catch (error) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _error = error is SnippetException
            ? error.message
            : context.l10n.snippetsSaveFailed;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final mod = defaultTargetPlatform == TargetPlatform.macOS;
    final canSave = !_saving && !_empty && !_readOnly && (_dirty || !_editing);
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
          },
          child: Focus(
            autofocus: true,
            skipTraversal: true,
            child: ConstrainedBox(
              key: const ValueKey('clip-edit-drawer'),
              constraints: AppOverlays.drawerContentConstraints(context),
              child: Scaffold(
                headers: [
                  AppBar(
                    title: Text(
                      _editing ? l10n.snippetsEditClip : l10n.snippetsNewText,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    leading: const [
                      AppIconFrame(child: Icon(LucideIcons.pencil)),
                    ],
                    trailing: [
                      Button.ghost(
                        style: AppTheme.controlButtonStyle(
                          const ButtonStyle.ghostIcon(),
                        ),
                        onPressed: () => unawaited(_close()),
                        child: Semantics(
                          label: l10n.commonClose,
                          child: const AppIconFrame(child: Icon(LucideIcons.x)),
                        ),
                      ),
                    ],
                  ),
                  const Divider(),
                ],
                footers: [
                  const Divider(),
                  Padding(
                    padding: const EdgeInsets.all(AppSpacing.lg),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        if (_empty && !_readOnly) ...[
                          Text(
                            l10n.snippetsSaveBlockEmpty,
                            key: const ValueKey('clip-edit-save-reason'),
                          ).small().muted(),
                          const Gap(AppSpacing.sm),
                        ],
                        Row(
                          children: [
                            Expanded(
                              child: Button.secondary(
                                style: AppTheme.actionButtonStyle(
                                  const ButtonStyle.secondary(),
                                ),
                                alignment: Alignment.center,
                                onPressed: _saving
                                    ? null
                                    : () => unawaited(_close()),
                                child: Text(l10n.commonCancel),
                              ),
                            ),
                            const Gap(AppSpacing.md),
                            Expanded(
                              child: Button.primary(
                                key: const ValueKey('clip-edit-save'),
                                style: AppTheme.actionButtonStyle(
                                  const ButtonStyle.primary(),
                                ),
                                alignment: Alignment.center,
                                onPressed: canSave
                                    ? () => unawaited(_save())
                                    : null,
                                child: Text(l10n.commonSave),
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ],
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(AppSpacing.lg),
                  child: _body(context),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _switch(String label, bool value, ValueChanged<bool> onChanged) =>
      MergeSemantics(
        child: Row(
          children: [
            Expanded(child: Text(label)),
            const Gap(AppSpacing.md),
            AppSwitchFrame(
              child: Switch(
                value: value,
                onChanged: _saving
                    ? null
                    : (next) => setState(() => onChanged(next)),
              ),
            ),
          ],
        ),
      );

  Widget _body(BuildContext context) {
    final l10n = context.l10n;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (_error case final error?) ...[
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(l10n.snippetsSaveFailed),
            content: Text(error),
          ),
          const Gap(AppSpacing.md),
        ],
        if (_readOnly) ...[
          Alert(
            leading: const AppIconFrame(child: Icon(LucideIcons.lock)),
            content: Text(l10n.snippetsClipTooLarge),
          ),
          const Gap(AppSpacing.md),
        ] else if (widget.clip?.rich ?? false) ...[
          Alert(
            leading: const AppIconFrame(child: Icon(LucideIcons.info)),
            content: Text(l10n.snippetsFormattingNotice),
          ),
          const Gap(AppSpacing.md),
        ],
        if (!_editing) ...[
          Text(l10n.snippetsDestination).small(),
          const Gap(AppSpacing.xs),
          Select<ClipEditDestination>(
            key: const ValueKey('clip-edit-destination'),
            value: _destination,
            onChanged: (value) {
              if (value != null) setState(() => _destination = value);
            },
            itemBuilder: (context, value) =>
                Text(l10n.snippetsDestinationName(value.name)),
            popup: SelectPopup<ClipEditDestination>(
              items: SelectItemList(
                children: [
                  for (final value in ClipEditDestination.values)
                    SelectItemButton<ClipEditDestination>(
                      value: value,
                      child: Text(l10n.snippetsDestinationName(value.name)),
                    ),
                ],
              ),
            ).call,
          ),
          const Gap(AppSpacing.md),
        ],
        TextArea(
          key: const ValueKey('clip-edit-text'),
          controller: _text,
          readOnly: _readOnly,
          autofocus: !_readOnly,
          expandableHeight: true,
          initialHeight: AppOverlaySize.editorBodyMinHeight,
          minHeight: AppOverlaySize.editorBodyMinHeight,
          decoration: AppOverlays.dialogFieldDecoration(context),
        ),
        if (_editing && !_readOnly) ...[
          const Gap(AppSpacing.md),
          _switch(
            l10n.snippetsReplaceOriginal,
            _replaceOriginal,
            (value) => _replaceOriginal = value,
          ),
          const Gap(AppSpacing.xs),
          Text(l10n.snippetsReplaceOriginalHint).small().muted(),
        ],
      ],
    );
  }
}
