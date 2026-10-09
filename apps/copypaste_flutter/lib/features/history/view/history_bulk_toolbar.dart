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
    final canApply = hasSelection && !busy && !controller.isBulkDragSelecting;
    final actions = Wrap(
      alignment: WrapAlignment.end,
      spacing: AppSpacing.sm,
      runSpacing: AppSpacing.sm,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
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
              onPressed: !canApply
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
              onPressed: !canApply
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
              onPressed: !canApply
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
    final label = '${controller.bulkSelectedIds.length} selected';
    final count = Semantics(liveRegion: true, child: Text(label));
    return LayoutBuilder(
      builder: (context, constraints) {
        final text = TextPainter(
          text: TextSpan(
            text: label,
            style: DefaultTextStyle.of(context).style,
          ),
          textDirection: Directionality.of(context),
          textScaler: MediaQuery.textScalerOf(context),
        )..layout();
        final extent = AppTheme.controlHeight(
          context,
          minimum: AppControlSize.large,
        );
        final minimumWidth = text.width + extent * 4 + AppSpacing.sm * 4;
        text.dispose();
        if (constraints.maxWidth >= minimumWidth) {
          return Row(
            children: [
              Expanded(child: count),
              const Gap(AppSpacing.sm),
              actions,
            ],
          );
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [count, const Gap(AppSpacing.sm), actions],
        );
      },
    );
  }
}
