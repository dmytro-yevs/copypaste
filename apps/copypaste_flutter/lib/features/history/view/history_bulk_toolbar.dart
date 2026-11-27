import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import 'dart:async';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../app/theme/app_tooltip.dart';
import '../controller/history_controller.dart';
import 'history_clip_menu.dart';
import 'history_delete_dialog.dart';
import 'history_tags_dialog.dart';

/// Actions over the explicit History selection, including pinned clips.
class HistoryBulkToolbar extends StatelessWidget {
  const HistoryBulkToolbar({super.key, required this.controller});

  final HistoryController controller;

  @override
  Widget build(BuildContext context) {
    final busy = controller.bulk.isBulkMutating;
    final hasSelection = controller.bulk.bulkSelectedIds.isNotEmpty;
    final canApply =
        hasSelection && !busy && !controller.bulk.isBulkDragSelecting;
    final canQueue = controller.capabilities.queue != null;
    final actions = Wrap(
      alignment: WrapAlignment.end,
      spacing: AppSpacing.sm,
      runSpacing: AppSpacing.sm,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        AppTooltip(
          tooltip: (context) =>
              TooltipContainer(child: Text(context.l10n.historyBulkTagTooltip)),
          child: Button.secondary(
            key: const ValueKey('history-bulk-tags'),
            style: AppTheme.historyToolbarIconStyle,
            onPressed: canApply && controller.bulk.canTagBulkSelection
                ? () => showHistoryTagsDialog(
                    context,
                    controller: controller,
                    clipIds: controller.bulk.bulkSelectedIds.toList(),
                  )
                : null,
            child: const AppIconFrame(
              size: AppIconSize.md,
              child: Icon(LucideIcons.tags, size: AppIconSize.md),
            ),
          ),
        ),
        if (canQueue)
          AppTooltip(
            tooltip: (context) => TooltipContainer(
              child: Text(context.l10n.historyActionAddToQueue),
            ),
            child: Semantics(
              label: context.l10n.historyActionAddToQueue,
              button: true,
              child: Button.secondary(
                key: const ValueKey('history-bulk-queue'),
                style: AppTheme.historyToolbarIconStyle,
                // The selection keeps the order the clips were selected in.
                onPressed: !canApply
                    ? null
                    : () => unawaited(
                        addHistoryClipsToQueue(
                          context,
                          controller: controller,
                          clipIds: controller.bulk.bulkSelectedIds.toList(),
                        ),
                      ),
                child: const AppIconFrame(
                  size: AppIconSize.md,
                  child: Icon(LucideIcons.listPlus, size: AppIconSize.md),
                ),
              ),
            ),
          ),
        _BulkShareButton(controller: controller, enabled: canApply),
        AppTooltip(
          tooltip: (context) =>
              TooltipContainer(child: Text(context.l10n.historyBulkPin)),
          child: Semantics(
            label: context.l10n.historyBulkPin,
            button: true,
            child: Button.secondary(
              key: const ValueKey('history-bulk-pin'),
              style: AppTheme.historyToolbarIconStyle,
              onPressed: !canApply
                  ? null
                  : () => unawaited(controller.bulk.setBulkPinned(true)),
              child: const AppIconFrame(
                size: AppIconSize.md,
                child: Icon(LucideIcons.pin, size: AppIconSize.md),
              ),
            ),
          ),
        ),
        AppTooltip(
          tooltip: (context) =>
              TooltipContainer(child: Text(context.l10n.historyBulkUnpin)),
          child: Semantics(
            label: context.l10n.historyBulkUnpin,
            button: true,
            child: Button.secondary(
              key: const ValueKey('history-bulk-unpin'),
              style: AppTheme.historyToolbarIconStyle,
              onPressed: !canApply
                  ? null
                  : () => unawaited(controller.bulk.setBulkPinned(false)),
              child: const AppIconFrame(
                size: AppIconSize.md,
                child: Icon(LucideIcons.pinOff, size: AppIconSize.md),
              ),
            ),
          ),
        ),
        AppTooltip(
          tooltip: (context) =>
              TooltipContainer(child: Text(context.l10n.historyBulkDelete)),
          child: Semantics(
            label: context.l10n.historyBulkDelete,
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
                  ? const AppIconFrame(
                      size: AppIconSize.md,
                      child: CircularProgressIndicator(size: AppIconSize.md),
                    )
                  : const AppIconFrame(
                      size: AppIconSize.md,
                      child: Icon(LucideIcons.trash2, size: AppIconSize.md),
                    ),
            ),
          ),
        ),
      ],
    );
    final label = context.l10n.historyBulkSelectedCount(
      controller.bulk.bulkSelectedIds.length,
    );
    final count = Semantics(liveRegion: true, child: Text(label));
    // Leaving selection sits with the count it clears, apart from the actions.
    final exit = AppTooltip(
      tooltip: (context) =>
          TooltipContainer(child: Text(context.l10n.historyBulkExit)),
      child: Semantics(
        label: context.l10n.historyBulkExit,
        button: true,
        child: Button.secondary(
          key: const ValueKey('history-bulk-close'),
          style: AppTheme.historyToolbarIconStyle,
          onPressed: busy ? null : controller.bulk.endBulkSelection,
          child: const AppIconFrame(
            size: AppIconSize.md,
            child: Icon(LucideIcons.x, size: AppIconSize.md),
          ),
        ),
      ),
    );
    final heading = Row(
      children: [
        exit,
        const Gap(AppSpacing.sm),
        Expanded(child: count),
      ],
    );
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
        final buttons = canQueue ? 7 : 6;
        final minimumWidth =
            text.width + extent * buttons + AppSpacing.sm * buttons;
        text.dispose();
        if (constraints.maxWidth >= minimumWidth) {
          return Row(
            children: [
              Expanded(child: heading),
              const Gap(AppSpacing.sm),
              actions,
            ],
          );
        }
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [heading, const Gap(AppSpacing.sm), actions],
        );
      },
    );
  }
}

/// Shares the selection in one share sheet. While files download first it
/// shows how far they are.
class _BulkShareButton extends StatelessWidget {
  const _BulkShareButton({required this.controller, required this.enabled});

  final HistoryController controller;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final transfers = controller.transfers;
    final sharing = transfers.isSharing;
    return ListenableBuilder(
      listenable: transfers.downloadProgressChanges,
      builder: (context, _) {
        final progress = sharing ? transfers.shareFileProgress : null;
        final label = progress == null
            ? context.l10n.historyBulkShare
            : context.l10n.historyDownloadingPercent((progress * 100).floor());
        return AppTooltip(
          tooltip: (context) => TooltipContainer(child: Text(label)),
          child: Semantics(
            label: label,
            button: true,
            child: Button.secondary(
              key: const ValueKey('history-bulk-share'),
              style: AppTheme.historyToolbarIconStyle,
              // The sheet takes the clips in the order they were selected.
              onPressed: !enabled || sharing
                  ? null
                  : () => unawaited(
                      shareHistoryClips(
                        context,
                        controller: controller,
                        clipIds: controller.bulk.bulkSelectedIds.toList(),
                      ),
                    ),
              child: AppIconFrame(
                size: AppIconSize.md,
                child: sharing
                    ? CircularProgressIndicator(
                        value: progress,
                        size: AppIconSize.md,
                      )
                    : const Icon(LucideIcons.share, size: AppIconSize.md),
              ),
            ),
          ),
        );
      },
    );
  }
}
