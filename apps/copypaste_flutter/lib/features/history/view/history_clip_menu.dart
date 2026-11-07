import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
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
