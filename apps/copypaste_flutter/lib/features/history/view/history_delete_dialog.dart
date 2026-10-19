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
      builder: (context, _) =>
          Text('Delete ${controller.bulkSelectedIds.length} selected clips?'),
    ),
    actionLabel: 'Delete',
    changes: controller,
    canConfirm: () =>
        !controller.isBulkMutating && controller.bulkSelectedIds.isNotEmpty,
    onConfirm: controller.deleteBulkSelection,
    failureMessage: () =>
        controller.errorMessage ?? 'The selected clips could not be deleted.',
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
    canConfirm: () => !controller.isDeletePending(clipId),
    onConfirm: () => controller.deleteClip(clipId),
    failureMessage: () =>
        controller.errorMessage ?? 'This clip could not be deleted.',
  );
}
