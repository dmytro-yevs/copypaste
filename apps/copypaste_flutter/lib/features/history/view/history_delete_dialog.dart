import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../controller/history_controller.dart';

Future<void> showHistoryBulkDeleteDialog(
  BuildContext context, {
  required HistoryController controller,
}) async {
  final strings = context.l10n;
  await AppOverlays.showActionDialog(
    context,
    icon: LucideIcons.trash2,
    title: AnimatedBuilder(
      animation: controller,
      builder: (context, _) => Text(
        context.l10n.historyBulkDeleteTitle(
          controller.bulk.bulkSelectedIds.length,
        ),
      ),
    ),
    actionLabel: strings.commonDelete,
    changes: controller,
    canConfirm: () =>
        !controller.bulk.isBulkMutating &&
        controller.bulk.bulkSelectedIds.isNotEmpty,
    onConfirm: controller.bulk.deleteBulkSelection,
    failureMessage: () =>
        controller.errors.message ?? strings.historyErrorBulkDeleteFallback,
  );
}

Future<void> showHistoryDeleteDialog(
  BuildContext context, {
  required HistoryController controller,
  required String clipId,
}) async {
  final strings = context.l10n;
  await AppOverlays.showActionDialog(
    context,
    icon: LucideIcons.trash2,
    title: Text(strings.historyDeleteClipTitle),
    actionLabel: strings.commonDelete,
    changes: controller,
    canConfirm: () => !controller.mutations.isDeletePending(clipId),
    onConfirm: () => controller.mutations.deleteClip(clipId),
    failureMessage: () =>
        controller.errors.message ?? strings.historyErrorDeleteClip,
  );
}
