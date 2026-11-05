import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../controller/history_controller.dart';

Future<void> showHistoryBulkDeleteDialog(
  BuildContext context, {
  required HistoryController controller,
}) async {
  await AppOverlays.showActionDialog(
    context,
    icon: LucideIcons.trash2,
    title: AnimatedBuilder(
      animation: controller,
      builder: (context, _) => Text(
        'Delete ${controller.bulk.bulkSelectedIds.length} selected clips?',
      ),
    ),
    actionLabel: 'Delete',
    changes: controller,
    canConfirm: () =>
        !controller.bulk.isBulkMutating &&
        controller.bulk.bulkSelectedIds.isNotEmpty,
    onConfirm: controller.bulk.deleteBulkSelection,
    failureMessage: () =>
        controller.errors.message ?? 'The selected clips could not be deleted.',
  );
}

Future<void> showHistoryDeleteDialog(
  BuildContext context, {
  required HistoryController controller,
  required String clipId,
}) async {
  await AppOverlays.showActionDialog(
    context,
    icon: LucideIcons.trash2,
    title: const Text('Delete this clip?'),
    actionLabel: 'Delete',
    changes: controller,
    canConfirm: () => !controller.mutations.isDeletePending(clipId),
    onConfirm: () => controller.mutations.deleteClip(clipId),
    failureMessage: () =>
        controller.errors.message ?? 'This clip could not be deleted.',
  );
}
