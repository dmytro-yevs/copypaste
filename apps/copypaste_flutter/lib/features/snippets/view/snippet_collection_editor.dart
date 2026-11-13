import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../controller/snippets_controller.dart';
import '../models/snippet_models.dart';
import '../presentation/snippet_messages.dart';

/// Creates a collection, or renames and restyles [collection].
Future<void> showSnippetCollectionEditor(
  BuildContext context, {
  required SnippetsController controller,
  SnippetCollection? collection,
}) => AppOverlays.showDialog<void>(
  context,
  builder: (context) =>
      _CollectionEditor(controller: controller, collection: collection),
);

class _CollectionEditor extends StatefulWidget {
  const _CollectionEditor({required this.controller, this.collection});

  final SnippetsController controller;
  final SnippetCollection? collection;

  @override
  State<_CollectionEditor> createState() => _CollectionEditorState();
}

class _CollectionEditorState extends State<_CollectionEditor> {
  late String _name = widget.collection?.name ?? '';
  late SnippetCollectionIcon _icon =
      widget.collection?.icon ?? SnippetCollectionIcon.folder;
  late SnippetCollectionAccent _accent =
      widget.collection?.accent ?? SnippetCollectionAccent.neutral;

  String? _problem(AppLocalizations l10n) {
    final name = _name.trim();
    if (name.isEmpty) return null;
    if (name.length > SnippetLimits.maxCollectionName) {
      return l10n.snippetsCollectionNameTooLong(
        SnippetLimits.maxCollectionName,
      );
    }
    if (widget.controller.collectionNameTaken(
      name,
      exceptId: widget.collection?.id,
    )) {
      return l10n.snippetsCollectionNameTaken;
    }
    return null;
  }

  Widget _label(String text) => Padding(
    padding: const EdgeInsets.only(bottom: AppSpacing.xs),
    child: Text(text).small(),
  );

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final problem = _problem(l10n);
    final valid = _name.trim().isNotEmpty && problem == null;
    return AppOverlays.alertDialog(
      icon: LucideIcons.folder,
      title: Text(
        widget.collection == null
            ? l10n.snippetsNewCollection
            : l10n.snippetsEditCollection,
      ),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _label(l10n.snippetsCollectionName),
          TextField(
            key: const ValueKey('snippet-collection-name'),
            initialValue: _name,
            maxLength: SnippetLimits.maxCollectionName,
            decoration: AppOverlays.dialogFieldDecoration(context),
            onChanged: (value) => setState(() => _name = value),
          ),
          if (problem != null) ...[
            const Gap(AppSpacing.xs),
            Text(problem).small().muted(),
          ],
          const Gap(AppSpacing.md),
          _label(l10n.snippetsCollectionIcon),
          Select<SnippetCollectionIcon>(
            value: _icon,
            onChanged: (value) {
              if (value != null) setState(() => _icon = value);
            },
            itemBuilder: (context, icon) => AppIconFrame(
              child: Icon(SnippetCollectionStyle.icon(icon)),
            ),
            popup: SelectPopup<SnippetCollectionIcon>(
              items: SelectItemList(
                children: [
                  for (final icon in SnippetCollectionIcon.values)
                    SelectItemButton<SnippetCollectionIcon>(
                      value: icon,
                      child: AppIconFrame(
                        child: Icon(SnippetCollectionStyle.icon(icon)),
                      ),
                    ),
                ],
              ),
            ).call,
          ),
          const Gap(AppSpacing.md),
          _label(l10n.snippetsCollectionAccentLabel),
          Select<SnippetCollectionAccent>(
            value: _accent,
            onChanged: (value) {
              if (value != null) setState(() => _accent = value);
            },
            itemBuilder: (context, accent) => _AccentLabel(accent: accent),
            popup: SelectPopup<SnippetCollectionAccent>(
              items: SelectItemList(
                children: [
                  for (final accent in SnippetCollectionAccent.values)
                    SelectItemButton<SnippetCollectionAccent>(
                      value: accent,
                      child: _AccentLabel(accent: accent),
                    ),
                ],
              ),
            ).call,
          ),
        ],
      ),
      actions: [
        Button.ghost(
          alignment: Alignment.center,
          onPressed: () => Navigator.pop(context),
          child: Text(l10n.commonCancel),
        ),
        Button.primary(
          key: const ValueKey('snippet-collection-save'),
          alignment: Alignment.center,
          onPressed: !valid || widget.controller.busy
              ? null
              : () async {
                  final saved = await widget.controller.saveCollection(
                    SnippetCollectionDraft(
                      id: widget.collection?.id,
                      name: _name.trim(),
                      icon: _icon,
                      accent: _accent,
                    ),
                  );
                  if (saved && context.mounted) Navigator.pop(context);
                },
          child: Text(l10n.commonSave),
        ),
      ],
    );
  }
}

class _AccentLabel extends StatelessWidget {
  const _AccentLabel({required this.accent});

  final SnippetCollectionAccent accent;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      AppIconFrame(
        size: AppIconSize.sm,
        child: Icon(
          LucideIcons.circle,
          size: AppIconSize.sm,
          color: SnippetCollectionStyle.accent(context, accent),
        ),
      ),
      const Gap(AppSpacing.sm),
      Text(SnippetCollectionStyle.accentLabel(context.l10n, accent)),
    ],
  );
}

/// Asks what happens to a deleted collection's snippets: they move to
/// Unfiled by default, or go to Recently Deleted with it.
Future<void> showDeleteSnippetCollectionDialog(
  BuildContext context, {
  required SnippetsController controller,
  required SnippetCollection collection,
}) async {
  var deleteSnippets = false;
  await AppOverlays.showActionDialog(
    context,
    icon: LucideIcons.trash2,
    title: Text(
      context.l10n.snippetsDeleteCollectionTitle(collection.displayName),
    ),
    content: StatefulBuilder(
      builder: (context, setState) {
        final l10n = context.l10n;
        return RadioGroup<bool>(
          value: deleteSnippets,
          onChanged: (value) => setState(() => deleteSnippets = value),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              RadioItem<bool>(
                value: false,
                trailing: Text(l10n.snippetsDeleteCollectionMove),
              ),
              const Gap(AppSpacing.sm),
              RadioItem<bool>(
                key: const ValueKey('snippet-collection-delete-snippets'),
                value: true,
                trailing: Text(l10n.snippetsDeleteCollectionWithSnippets),
              ),
            ],
          ),
        );
      },
    ),
    actionLabel: context.l10n.commonDelete,
    failureMessage: () => controller.errorMessage ?? '',
    onConfirm: () =>
        controller.deleteCollection(
          collection.id,
          deleteSnippets: deleteSnippets,
        ),
  );
}

/// Merges [collection] into another one the user picks.
Future<void> showMergeSnippetCollectionDialog(
  BuildContext context, {
  required SnippetsController controller,
  required SnippetCollection collection,
}) async {
  final targets = [
    for (final other in controller.collections)
      if (other.id != collection.id) other,
  ];
  if (targets.isEmpty) return;
  var into = targets.first.id;
  await AppOverlays.showActionDialog(
    context,
    icon: LucideIcons.merge,
    destructive: false,
    title: Text(
      context.l10n.snippetsMergeCollectionTitle(collection.displayName),
    ),
    content: StatefulBuilder(
      builder: (context, setState) => Select<String>(
        value: into,
        onChanged: (value) {
          if (value != null) setState(() => into = value);
        },
        itemBuilder: (context, id) => Text(
          targets.firstWhere((target) => target.id == id).displayName,
        ),
        popup: SelectPopup<String>(
          items: SelectItemList(
            children: [
              for (final target in targets)
                SelectItemButton<String>(
                  value: target.id,
                  child: Text(target.displayName),
                ),
            ],
          ),
        ).call,
      ),
    ),
    actionLabel: context.l10n.snippetsMerge,
    failureMessage: () => controller.errorMessage ?? '',
    onConfirm: () => controller.mergeCollections(collection.id, into),
  );
}
