import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';
import '../controller/history_controller.dart';
import '../controller/history_tags_controller.dart';
import '../models/history_models.dart';

Future<void> showHistoryTagsDialog(
  BuildContext context, {
  required HistoryController controller,
  List<String> clipIds = const [],
  Object? anchor,
}) async {
  if (!context.mounted || !controller.tags.available) return;
  await AppOverlays.showCompactEditor<void>(
    context,
    anchor: anchor,
    maxWidth: (context.size?.width ?? AppOverlaySize.compactEditorMaxWidth)
        .clamp(
          AppOverlaySize.compactEditorMinWidth + AppSpacing.md * 2,
          AppOverlaySize.compactEditorMaxWidth,
        ),
    builder: (_) => _TagsEditor(history: controller, clipIds: clipIds),
  );
  await controller.window.refreshFacets();
}

enum _TagAction { rename, delete }

class _TagsEditor extends StatefulWidget {
  const _TagsEditor({required this.history, required this.clipIds});
  final HistoryController history;
  final List<String> clipIds;
  @override
  State<_TagsEditor> createState() => _TagsEditorState();
}

class _TagsEditorState extends State<_TagsEditor> {
  final _name = TextEditingController();
  final _nameFocus = FocusNode();
  late final HistoryTagsController _tags = widget.history
      .createTagsController();
  late final Listenable _changes = Listenable.merge([_tags, _name]);

  @override
  void initState() {
    super.initState();
    _tags.load(widget.clipIds);
  }

  String? _editingId;
  String? _deletingId;

  void _edit(HistoryTag tag) {
    setState(() {
      _editingId = tag.id;
      _name.text = tag.name;
      _deletingId = null;
    });
  }

  Future<void> _save() async {
    if (_tags.busy || _tags.loading || _name.text.trim().isEmpty) return;
    if (await _tags.save(_name.text, id: _editingId) && mounted) {
      setState(() {
        _editingId = null;
        _name.clear();
      });
    }
  }

  void _escape(BuildContext context) {
    if (_editingId != null && !_tags.busy) {
      setState(() {
        _editingId = null;
        _name.clear();
      });
    } else {
      closeOverlay(context);
    }
  }

  @override
  void dispose() {
    _nameFocus.dispose();
    _name.dispose();
    _tags.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: _changes,
    builder: (context, _) {
      final tags = _tags;
      return CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () =>
              _escape(context),
        },
        child: ConstrainedBox(
          constraints: const BoxConstraints(
            minWidth: AppOverlaySize.compactEditorMinWidth,
          ),
          child: SizedBox(
            width: AppOverlaySize.compactEditorMaxWidth,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const Text('Tags').semiBold(),
                const Gap(AppSpacing.md),
                Flexible(
                  child: SingleChildScrollView(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        if (tags.loading && tags.tags.isEmpty)
                          const StateView.loading()
                        else if (tags.error != null)
                          StateView.error(
                            title: 'Could not update tags',
                            message: tags.error,
                          )
                        else if (tags.tags.isEmpty)
                          const StateView.empty(
                            title: 'No tags yet',
                            compact: true,
                          ),
                        for (final tag in tags.tags)
                          if (_deletingId == tag.id)
                            Column(
                              mainAxisSize: MainAxisSize.min,
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                Text(tag.name),
                                Row(
                                  mainAxisAlignment: MainAxisAlignment.end,
                                  spacing: AppSpacing.sm,
                                  children: [
                                    Button.outline(
                                      onPressed: tags.busy
                                          ? null
                                          : () => setState(
                                              () => _deletingId = null,
                                            ),
                                      child: const Text('Cancel'),
                                    ),
                                    Button.destructive(
                                      onPressed: tags.busy
                                          ? null
                                          : () async {
                                              if (await tags.delete(tag.id) &&
                                                  mounted) {
                                                setState(
                                                  () => _deletingId = null,
                                                );
                                              }
                                            },
                                      child: const Text('Delete'),
                                    ),
                                  ],
                                ),
                              ],
                            )
                          else
                            Row(
                              spacing: AppSpacing.sm,
                              children: [
                                Expanded(
                                  child: tags.clipIds.isEmpty
                                      ? Text(tag.name)
                                      : Checkbox(
                                          state:
                                              tags.assignedCount(tag.id) ==
                                                  tags.clipIds.length
                                              ? CheckboxState.checked
                                              : tags.assignedCount(tag.id) == 0
                                              ? CheckboxState.unchecked
                                              : CheckboxState.indeterminate,
                                          onChanged: tags.busy
                                              ? null
                                              : (_) => tags.assign(
                                                  tag.id,
                                                  tags.assignedCount(tag.id) !=
                                                      tags.clipIds.length,
                                                ),
                                          trailing: Flexible(
                                            child: Text(tag.name),
                                          ),
                                        ),
                                ),
                                OverlayAnchor(
                                  anchor: (
                                    'history-tag-actions',
                                    _tags,
                                    tag.id,
                                  ),
                                  child: ComponentTheme<SelectTheme>(
                                    data: AppTheme.selectTheme(
                                      context,
                                      variant: AppSelectVariant.inlineAction,
                                      labels: const ['Rename', 'Delete'],
                                      anchor: LinkedAnchor(
                                        ('history-tag-actions', _tags, tag.id),
                                        registry: OverlayAnchorRegistry.of(
                                          context,
                                        ),
                                      ),
                                    ),
                                    child: Tooltip(
                                      tooltip: (_) => const TooltipContainer(
                                        child: Text('Tag actions'),
                                      ),
                                      child: Select<_TagAction>(
                                        key: ValueKey(
                                          'history-tag-actions-${tag.id}',
                                        ),
                                        enabled: !tags.busy,
                                        value: null,
                                        expandIcon: null,
                                        placeholder: const AppIconFrame(
                                          size: AppIconSize.sm,
                                          child: Icon(
                                            LucideIcons.ellipsis,
                                            size: AppIconSize.sm,
                                          ),
                                        ),
                                        itemBuilder: (context, _) =>
                                            const AppIconFrame(
                                              size: AppIconSize.sm,
                                              child: Icon(
                                                LucideIcons.ellipsis,
                                                size: AppIconSize.sm,
                                              ),
                                            ),
                                        onChanged: (action) {
                                          if (action == _TagAction.rename) {
                                            _edit(tag);
                                          }
                                          if (action == _TagAction.delete) {
                                            setState(() {
                                              _deletingId = tag.id;
                                              _editingId = null;
                                              _name.clear();
                                            });
                                          }
                                        },
                                        popup: SelectPopup<_TagAction>(
                                          shrinkWrap: true,
                                          items: SelectItemList(
                                            children: [
                                              SelectItemButton<_TagAction>(
                                                value: _TagAction.rename,
                                                child: Row(
                                                  mainAxisSize:
                                                      MainAxisSize.min,
                                                  children: const [
                                                    AppIconFrame(
                                                      size: AppIconSize.sm,
                                                      child: Icon(
                                                        LucideIcons.pencil,
                                                        size: AppIconSize.sm,
                                                      ),
                                                    ),
                                                    Gap(AppSpacing.sm),
                                                    Text('Rename'),
                                                  ],
                                                ),
                                              ),
                                              SelectItemButton<_TagAction>(
                                                value: _TagAction.delete,
                                                child: Row(
                                                  mainAxisSize:
                                                      MainAxisSize.min,
                                                  children: const [
                                                    AppIconFrame(
                                                      size: AppIconSize.sm,
                                                      child: Icon(
                                                        LucideIcons.trash2,
                                                        size: AppIconSize.sm,
                                                      ),
                                                    ),
                                                    Gap(AppSpacing.sm),
                                                    Text('Delete'),
                                                  ],
                                                ),
                                              ),
                                            ],
                                          ),
                                        ).call,
                                      ),
                                    ),
                                  ),
                                ),
                              ],
                            ),
                      ],
                    ),
                  ),
                ),
                const Gap(AppSpacing.md),
                TextField(
                  key: const ValueKey('history-tag-name'),
                  controller: _name,
                  focusNode: _nameFocus,
                  autofocus: widget.clipIds.isEmpty,
                  enabled: !tags.busy && !tags.loading,
                  maxLength: 64,
                  placeholder: const Text('Tag name'),
                  decoration: AppOverlays.dialogFieldDecoration(context),
                  onSubmitted: (_) => _save(),
                  features: [
                    InputFeature.trailing(
                      Semantics(
                        label: _editingId == null ? 'Create tag' : 'Save tag',
                        button: true,
                        child: Button.ghost(
                          key: const ValueKey('history-tag-submit'),
                          style: AppTheme.inputFeatureButtonStyle,
                          onPressed:
                              tags.busy ||
                                  tags.loading ||
                                  _name.text.trim().isEmpty
                              ? null
                              : _save,
                          child: AppIconFrame(
                            child: Icon(
                              _editingId == null
                                  ? LucideIcons.plus
                                  : LucideIcons.check,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      );
    },
  );
}
