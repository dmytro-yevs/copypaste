import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';
import '../../ai_tools/view/hide_from_ai_action.dart';
import '../../content_rules/view/rule_trace_panel.dart';
import '../../snippets/view/clip_edit_drawer.dart';
import '../../transforms/models/transform_models.dart';
import '../../transforms/view/transform_palette.dart';
import '../controller/history_controller.dart';
import '../models/history_models.dart';
import 'history_delete_dialog.dart';
import 'history_tags_dialog.dart';

/// Copies one clip through [HistoryController] and confirms it with a toast.
Future<void> copyHistoryClip(
  BuildContext context, {
  required HistoryController controller,
  required String clipId,
  bool plainText = false,
}) async {
  if (!await controller.transfers.copyClip(clipId, plainText: plainText) ||
      !context.mounted) {
    return;
  }
  AppToast.show(
    context,
    title: context.l10n.historyCopiedTitle,
    message: plainText
        ? context.l10n.historyPlainTextCopied
        : context.l10n.historyClipCopied,
    tone: AppToastTone.success,
  );
}

/// The actions other features add to a History clip. The row menu and the
/// inspector offer the same set for a clip, built by [historyClipActions].
enum HistoryClipAction {
  edit('edit'),
  saveAsSnippet('save-snippet'),
  transform('transform'),
  addToQueue('add-queue'),
  hideFromAi('hide-ai'),
  keep('keep'),
  explain('explain');

  const HistoryClipAction(this.id);

  /// Stable part of the action's widget keys.
  final String id;

  String label(BuildContext context, HistoryClip clip) => switch (this) {
    edit => context.l10n.historyActionEdit,
    saveAsSnippet => context.l10n.historyActionSaveAsSnippet,
    transform => context.l10n.historyActionTransform,
    addToQueue => context.l10n.historyActionAddToQueue,
    hideFromAi => HideFromAiAction.label(
      context,
      hidden: clip.annotations.hiddenFromAi,
    ),
    keep => context.l10n.historyActionKeep,
    explain => context.l10n.historyActionExplain,
  };

  IconData icon(HistoryClip clip) => switch (this) {
    edit => LucideIcons.pencil,
    saveAsSnippet => LucideIcons.notebookText,
    transform => LucideIcons.wandSparkles,
    addToQueue => LucideIcons.listPlus,
    hideFromAi => HideFromAiAction.icon(hidden: clip.annotations.hiddenFromAi),
    keep => LucideIcons.timerOff,
    explain => LucideIcons.circleHelp,
  };
}

/// The feature actions available for [clip], in display order. An action
/// whose capability the repository lacks is never offered.
List<HistoryClipAction> historyClipActions(
  HistoryController controller,
  HistoryClip clip,
) {
  final capabilities = controller.capabilities;
  final textual = clip.contentKind.isTextual;
  final annotated = capabilities.annotations != null;
  return [
    if (capabilities.edits != null && textual) ...[
      HistoryClipAction.edit,
      HistoryClipAction.saveAsSnippet,
    ],
    if (capabilities.transforms != null && textual) HistoryClipAction.transform,
    if (capabilities.queue != null) HistoryClipAction.addToQueue,
    if (annotated) HistoryClipAction.hideFromAi,
    if (annotated &&
        clip.annotations.expiresAt != null &&
        !clip.annotations.keep)
      HistoryClipAction.keep,
    if (annotated) HistoryClipAction.explain,
  ];
}

/// Menu entries for [actions]; [keyPrefix] names the surface in their keys.
List<MenuItem> historyClipActionMenuItems(
  BuildContext context, {
  required HistoryController controller,
  required HistoryClip clip,
  required Iterable<HistoryClipAction> actions,
  required String keyPrefix,
}) => [
  for (final action in actions)
    MenuButton(
      key: ValueKey<String>('$keyPrefix-${action.id}-${clip.id}'),
      onPressed: (_) => unawaited(
        runHistoryClipAction(
          context,
          controller: controller,
          clip: clip,
          action: action,
        ),
      ),
      leading: AppIconFrame(
        size: AppIconSize.sm,
        child: Icon(action.icon(clip), size: AppIconSize.sm),
      ),
      child: Text(action.label(context, clip)),
    ),
];

/// The actions of a History row menu. [context] belongs to the row, so the
/// actions outlive the menu that starts them. Items stay enabled while a
/// mutation is pending because the library menu cannot disable a shown item.
List<MenuItem> historyClipMenuItems(
  BuildContext context, {
  required HistoryController controller,
  required HistoryClip clip,
  bool includeCopy = false,
}) {
  final strings = context.l10n;
  MenuItem item(
    String id,
    IconData icon,
    String label,
    VoidCallback onPressed, {
    Color? color,
  }) => MenuButton(
    key: ValueKey<String>('history-menu-$id-${clip.id}'),
    onPressed: (_) => onPressed(),
    leading: AppIconFrame(
      size: AppIconSize.sm,
      child: Icon(icon, size: AppIconSize.sm, color: color),
    ),
    child: Text(label, style: TextStyle(color: color)),
  );

  return [
    if (includeCopy)
      item(
        'copy',
        LucideIcons.copy,
        strings.commonCopy,
        () => unawaited(
          copyHistoryClip(context, controller: controller, clipId: clip.id),
        ),
      ),
    if (clip.contentKind.isTextual)
      item(
        'copy-plain',
        LucideIcons.alignLeft,
        strings.historyCopyPlainText,
        () => unawaited(
          copyHistoryClip(
            context,
            controller: controller,
            clipId: clip.id,
            plainText: true,
          ),
        ),
      ),
    if (clip.pinned && controller.tags.available)
      item(
        'tags',
        LucideIcons.tags,
        strings.historyTagsAction,
        () => unawaited(
          showHistoryTagsDialog(
            context,
            controller: controller,
            clipIds: [clip.id],
          ),
        ),
      ),
    item(
      'secret',
      clip.secret ? LucideIcons.lockOpen : LucideIcons.lockKeyhole,
      clip.secret ? strings.historyRemoveSpoiler : strings.historyAddSpoiler,
      () {
        if (!controller.mutations.isSecretPending(clip.id)) {
          unawaited(_toggleSecret(context, controller, clip));
        }
      },
    ),
    ...historyClipActionMenuItems(
      context,
      controller: controller,
      clip: clip,
      actions: historyClipActions(controller, clip),
      keyPrefix: 'history-menu',
    ),
    const MenuDivider(),
    item('delete', LucideIcons.trash2, strings.commonDelete, () {
      if (!controller.mutations.isDeletePending(clip.id)) {
        unawaited(
          showHistoryDeleteDialog(
            context,
            controller: controller,
            clipId: clip.id,
          ),
        );
      }
    }, color: Theme.of(context).colorScheme.destructive),
  ];
}

/// Runs [action] for [clip] and reports its outcome with a toast.
Future<void> runHistoryClipAction(
  BuildContext context, {
  required HistoryController controller,
  required HistoryClip clip,
  required HistoryClipAction action,
}) async {
  final capabilities = controller.capabilities;
  switch (action) {
    case HistoryClipAction.edit:
      await _editClip(context, controller, clip.id);
    case HistoryClipAction.saveAsSnippet:
      final id = await capabilities.saveAsSnippet(clip.id);
      if (!context.mounted) return;
      _toast(
        context,
        id != null,
        success: context.l10n.historySnippetSaved,
        failure: context.l10n.historySnippetSaveFailed,
      );
    case HistoryClipAction.transform:
      final transforms = capabilities.transforms;
      if (transforms == null) return;
      await showTransformPalette(
        context,
        repository: transforms.transforms,
        recents: transforms.transformRecents,
        source: TransformSource.clip(clip.id),
      );
    case HistoryClipAction.addToQueue:
      await addHistoryClipsToQueue(
        context,
        controller: controller,
        clipIds: [clip.id],
      );
    case HistoryClipAction.hideFromAi:
      if (capabilities.isAnnotationPending(clip.id)) return;
      final changed = await capabilities.setHiddenFromAi(
        clip,
        !clip.annotations.hiddenFromAi,
      );
      if (!changed && context.mounted) {
        _failure(context, context.l10n.historyHideFromAiFailed);
      }
    case HistoryClipAction.keep:
      if (capabilities.isAnnotationPending(clip.id)) return;
      final kept = await capabilities.keep(clip);
      if (!kept && context.mounted) {
        _failure(context, context.l10n.historyKeepFailed);
      }
    case HistoryClipAction.explain:
      await _showRuleTraceDialog(context, controller, clip.id);
  }
}

/// Opens the editor for a new clip typed by the user.
Future<void> showHistoryNewClipDrawer(
  BuildContext context, {
  required HistoryController controller,
}) async {
  final edits = controller.capabilities.edits;
  if (edits == null) return;
  final id = await showClipEditDrawer(
    context,
    clips: edits.clipEdits,
    snippets: edits.snippets,
    preferences: edits.clipEditPreferences,
  );
  if (id != null && context.mounted) {
    _toast(context, true, success: context.l10n.historyNewClipSaved);
  }
}

/// Appends [clipIds] to the paste queue in the given order, which is the
/// order the user selected them in.
Future<void> addHistoryClipsToQueue(
  BuildContext context, {
  required HistoryController controller,
  required List<String> clipIds,
}) async {
  final pending = await controller.capabilities.addToQueue(clipIds);
  if (!context.mounted) return;
  if (pending == null) {
    _failure(context, context.l10n.historyQueueAddFailed);
    return;
  }
  AppToast.show(
    context,
    title: context.l10n.pasteQueueAdded(pending),
    message: context.l10n.historyQueueAddedInOrder(clipIds.length),
    tone: AppToastTone.success,
  );
}

Future<void> _editClip(
  BuildContext context,
  HistoryController controller,
  String clipId,
) async {
  final edits = controller.capabilities.edits;
  if (edits == null) return;
  final HistoryClip detail;
  try {
    detail = await controller.capabilities.detail(clipId);
  } catch (_) {
    if (context.mounted) _failure(context, context.l10n.historyErrorOpenClip);
    return;
  }
  if (!context.mounted) return;
  final type = detail.contentType.toLowerCase();
  final id = await showClipEditDrawer(
    context,
    clips: edits.clipEdits,
    snippets: edits.snippets,
    preferences: edits.clipEditPreferences,
    clip: ClipEditTarget(
      id: detail.id,
      text: detail.body ?? detail.preview,
      rich: type.contains('html') || type.contains('rtf'),
    ),
  );
  if (id != null && context.mounted) {
    _toast(context, true, success: context.l10n.historyEditSaved);
  }
}

Future<void> _showRuleTraceDialog(
  BuildContext context,
  HistoryController controller,
  String clipId,
) async {
  if (controller.capabilities.annotations == null) return;
  final explanation = controller.capabilities.explain(clipId);
  // The dialog's builder reports a failure; it subscribes a frame later.
  explanation.ignore();
  await AppOverlays.showDialog<void>(
    context,
    builder: (dialogContext) => AppOverlays.alertDialog(
      icon: LucideIcons.circleHelp,
      title: Text(dialogContext.l10n.historyActionExplain),
      content: ConstrainedBox(
        key: const ValueKey('history-rule-trace'),
        constraints: BoxConstraints(
          maxHeight:
              MediaQuery.sizeOf(dialogContext).height *
              AppOverlaySize.dialogContentHeightFactor,
        ),
        child: FutureBuilder(
          future: explanation,
          builder: (context, snapshot) {
            if (snapshot.hasError) {
              return StateView.error(title: context.l10n.historyExplainFailed);
            }
            final data = snapshot.data;
            if (data == null) return const StateView.loading();
            final (trace, options) = data;
            return SingleChildScrollView(
              child: RuleTracePanel(trace: trace, options: options),
            );
          },
        ),
      ),
      actions: [
        Button.secondary(
          style: AppTheme.dialogButtonStyle(const ButtonStyle.secondary()),
          onPressed: () => Navigator.pop(dialogContext),
          child: Text(dialogContext.l10n.commonClose),
        ),
      ],
    ),
  );
}

void _toast(
  BuildContext context,
  bool succeeded, {
  required String success,
  String? failure,
}) {
  if (!succeeded) {
    if (failure != null) _failure(context, failure);
    return;
  }
  AppToast.show(
    context,
    title: context.l10n.historySavedTitle,
    message: success,
    tone: AppToastTone.success,
  );
}

void _failure(BuildContext context, String message) => AppToast.show(
  context,
  title: context.l10n.historyActionFailedTitle,
  message: message,
  tone: AppToastTone.error,
);

Future<void> _toggleSecret(
  BuildContext context,
  HistoryController controller,
  HistoryClip clip,
) async {
  if (await controller.mutations.toggleSecret(clip) || !context.mounted) return;
  AppToast.show(
    context,
    title: context.l10n.historyUpdateFailedTitle,
    message: context.l10n.historyErrorSecrecyUpdate,
    tone: AppToastTone.error,
  );
}
