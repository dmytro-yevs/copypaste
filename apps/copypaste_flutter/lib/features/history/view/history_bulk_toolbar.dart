import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../controller/history_controller.dart';
import 'history_delete_dialog.dart';

/// Actions over the explicit History selection, including pinned clips.
class HistoryBulkToolbar extends StatelessWidget {
  const HistoryBulkToolbar({super.key, required this.controller});

  final HistoryController controller;

  @override
  Widget build(BuildContext context) {
    final busy = controller.isBulkMutating;
    final hasSelection = controller.bulkSelectedIds.isNotEmpty;
    return Wrap(
      spacing: AppSpacing.sm,
      runSpacing: AppSpacing.sm,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        Semantics(
          liveRegion: true,
          child: Text('${controller.bulkSelectedIds.length} selected'),
        ),
        Tooltip(
          tooltip: (_) => const TooltipContainer(child: Text('Exit selection')),
          child: Semantics(
            label: 'Exit selection',
            button: true,
            child: Button.secondary(
              key: const ValueKey('history-bulk-close'),
              style: AppTheme.historyToolbarIconStyle,
              onPressed: busy ? null : controller.endBulkSelection,
              child: const Icon(LucideIcons.x, size: AppIconSize.md),
            ),
          ),
        ),
        Tooltip(
          tooltip: (_) => const TooltipContainer(child: Text('Pin selected')),
          child: Semantics(
            label: 'Pin selected',
            button: true,
            child: Button.secondary(
              key: const ValueKey('history-bulk-pin'),
              style: AppTheme.historyToolbarIconStyle,
              onPressed: busy || !hasSelection
                  ? null
                  : () => unawaited(controller.setBulkPinned(true)),
              child: const Icon(LucideIcons.pin, size: AppIconSize.md),
            ),
          ),
        ),
        Tooltip(
          tooltip: (_) => const TooltipContainer(child: Text('Unpin selected')),
          child: Semantics(
            label: 'Unpin selected',
            button: true,
            child: Button.secondary(
              key: const ValueKey('history-bulk-unpin'),
              style: AppTheme.historyToolbarIconStyle,
              onPressed: busy || !hasSelection
                  ? null
                  : () => unawaited(controller.setBulkPinned(false)),
              child: const Icon(LucideIcons.pinOff, size: AppIconSize.md),
            ),
          ),
        ),
        Tooltip(
          tooltip: (_) =>
              const TooltipContainer(child: Text('Delete selected')),
          child: Semantics(
            label: 'Delete selected',
            button: true,
            child: Button.destructive(
              key: const ValueKey('history-bulk-delete'),
              style: AppTheme.historyToolbarDestructiveIconStyle,
              onPressed: busy || !hasSelection
                  ? null
                  : () => showHistoryBulkDeleteDialog(
                      context,
                      controller: controller,
                    ),
              child: busy
                  ? const CircularProgressIndicator(size: AppIconSize.md)
                  : const Icon(LucideIcons.trash2, size: AppIconSize.md),
            ),
          ),
        ),
      ],
    );
  }
}
