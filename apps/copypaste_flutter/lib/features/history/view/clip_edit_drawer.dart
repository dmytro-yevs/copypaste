import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../app/theme/app_toast.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/dialogs/unsaved_changes_dialog.dart';
import '../../../shared/error_text.dart';
import '../repository/clip_edit_preferences.dart';
import '../repository/clip_edit_repository.dart';
import 'history_selectable_text.dart';

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

  /// The most UTF-8 bytes a clip may have to be edited.
  static const maxEditableBytes = 256 * 1024;

  /// Larger clips open read-only.
  bool get tooLarge =>
      text.length * 3 > maxEditableBytes &&
      utf8.encode(text).length > maxEditableBytes;
}

/// Edits [clip] into a new clip, or with no clip writes new text to
/// History. Resolves to the id written, or null.
Future<String?> showClipEditDrawer(
  BuildContext context, {
  required ClipEditRepository clips,
  required ClipEditPreferences preferences,
  ClipEditTarget? clip,
  Future<bool> Function(String text)? onCopy,
}) => showOverlay<String>(
  context,
  AppOverlays.drawerConfiguration(context, guarded: true),
  builder: (context) => ClipEditDrawer(
    clips: clips,
    preferences: preferences,
    clip: clip,
    onCopy: onCopy,
  ),
).future;

/// One editor for new and edited text. Nothing is written until Save;
/// closing with changes asks first.
class ClipEditDrawer extends StatefulWidget {
  const ClipEditDrawer({
    super.key,
    required this.clips,
    required this.preferences,
    this.clip,
    this.onCopy,
  });

  final ClipEditRepository clips;
  final ClipEditPreferences preferences;
  final ClipEditTarget? clip;

  /// Copies text selected in a read-only clip; resolves to whether it was
  /// written. Without it a read-only selection cannot be copied.
  final Future<bool> Function(String text)? onCopy;

  @override
  State<ClipEditDrawer> createState() => _ClipEditDrawerState();
}

class _ClipEditDrawerState extends State<ClipEditDrawer> {
  /// Read-only clips are shown sliced and never get a text field controller.
  late final bool _readOnly = widget.clip?.tooLarge ?? false;
  TextEditingController? _text;
  String _value = '';
  bool _dirty = false;
  bool _empty = true;
  bool _replaceOriginal = false;
  bool _saving = false;
  bool _closing = false;

  bool get _editing => widget.clip != null;

  @override
  void initState() {
    super.initState();
    if (!_readOnly) {
      _value = widget.clip?.text ?? '';
      _empty = _value.trim().isEmpty;
      _text = TextEditingController(text: _value)
        ..addListener(_handleTextChange);
    }
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
    _text?.dispose();
    super.dispose();
  }

  /// Selection-only changes keep the same text and need no rebuild.
  void _handleTextChange() {
    final value = _text!.text;
    if (value == _value) return;
    _value = value;
    setState(() {
      _dirty = value != (widget.clip?.text ?? '');
      _empty = value.trim().isEmpty;
    });
  }

  Future<bool> _copySelection(String text) async =>
      await widget.onCopy?.call(text) ?? false;

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
    });
    final text = _value.replaceAll('\r\n', '\n').replaceAll('\r', '\n');
    try {
      final String id;
      if (widget.clip case final clip?) {
        await widget.preferences.setReplaceOriginal(_replaceOriginal);
        id = await widget.clips.saveClipEdit(
          clip.id,
          text,
          replaceOriginal: _replaceOriginal,
        );
      } else {
        id = await widget.clips.createManualClip(text);
      }
      if (!mounted) return;
      setState(() => _saving = false);
      _finish(id);
    } on Object catch (error) {
      if (!mounted) return;
      setState(() => _saving = false);
      AppToast.show(
        context,
        title: context.l10n.snippetsSaveFailed,
        message: ErrorText.describe(error),
        tone: AppToastTone.error,
      );
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
                    child: Row(
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
        if (_text case final controller?)
          TextArea(
            key: const ValueKey('clip-edit-text'),
            controller: controller,
            autofocus: true,
            expandableHeight: true,
            initialHeight: AppOverlaySize.editorBodyMinHeight,
            minHeight: AppOverlaySize.editorBodyMinHeight,
            decoration: AppOverlays.dialogFieldDecoration(context),
          )
        else
          HistorySelectableText(
            widget.clip!.text,
            key: const ValueKey('clip-edit-text'),
            onCopy: _copySelection,
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
