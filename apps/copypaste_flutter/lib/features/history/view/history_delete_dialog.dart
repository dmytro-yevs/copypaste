import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../controller/history_controller.dart';

Future<void> showHistoryDeleteDialog(
  BuildContext context, {
  required HistoryController controller,
  required String clipId,
}) => AppOverlays.showDialog<void>(
  context,
  builder: (context) => AnimatedBuilder(
    animation: controller,
    builder: (context, child) => AppOverlays.alertDialog(
      icon: LucideIcons.trash2,
      title: const Text('Delete this clip?'),
      actions: [
        Button.ghost(
          onPressed: controller.isDeletePending(clipId)
              ? null
              : () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
        Button.destructive(
          onPressed: controller.isDeletePending(clipId)
              ? null
              : () async {
                  final deleted = await controller.deleteClip(clipId);
                  if (deleted && context.mounted) Navigator.pop(context);
                },
          child: const Text('Delete'),
        ),
      ],
    ),
  ),
);
