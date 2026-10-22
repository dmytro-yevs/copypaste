import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/format_bytes.dart';
import '../file_operations_controller.dart';

/// A compact transfer row for evaluating the ledger direction in the preview.
///
/// This is deliberately presentation-only: live speed and record-removal stay
/// behind the callbacks supplied by the preview host.
class CompactLedgerConcept extends StatelessWidget {
  const CompactLedgerConcept({
    super.key,
    required this.operation,
    this.speedLabel,
    this.onCancel,
    this.onOpenFile,
    this.onRemove,
  });

  final FileOperation operation;
  final String? speedLabel;
  final VoidCallback? onCancel;
  final VoidCallback? onOpenFile;
  final VoidCallback? onRemove;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final active = operation.active;
    final primaryStyle = theme.typography.small.copyWith(
      color: theme.colorScheme.foreground,
      fontWeight: FontWeight.w600,
    );
    final metadataStyle = theme.typography.xSmall.copyWith(
      color: theme.colorScheme.mutedForeground,
    );

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.sm),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Text(
                  operation.name,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: primaryStyle,
                ),
              ),
              if (active && onCancel != null) ...[
                const Gap(AppSpacing.sm),
                Tooltip(
                  tooltip: (_) =>
                      const TooltipContainer(child: Text('Cancel transfer')),
                  child: Button.ghost(
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.ghostIcon(),
                    ),
                    onPressed: onCancel,
                    child: const Icon(LucideIcons.x),
                  ),
                ),
              ],
              if (!active) ...[
                if (onOpenFile != null) ...[
                  const Gap(AppSpacing.sm),
                  Tooltip(
                    tooltip: (_) =>
                        const TooltipContainer(child: Text('Show saved file')),
                    child: Button.ghost(
                      style: AppTheme.controlButtonStyle(
                        const ButtonStyle.ghostIcon(),
                      ),
                      onPressed: onOpenFile,
                      child: const Icon(LucideIcons.folderOpen),
                    ),
                  ),
                ],
                if (onRemove != null) ...[
                  const Gap(AppSpacing.xs),
                  Tooltip(
                    tooltip: (_) => const TooltipContainer(
                      child: Text('Remove from history'),
                    ),
                    child: Button.ghost(
                      style: AppTheme.controlButtonStyle(
                        const ButtonStyle.ghostIcon(),
                      ),
                      onPressed: onRemove,
                      child: const Icon(LucideIcons.trash2),
                    ),
                  ),
                ],
              ],
            ],
          ),
          const Gap(AppSpacing.xs),
          Row(
            children: [
              Expanded(
                child: Text(
                  '${operation.sourceDevice} → ${operation.destinationDevice}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: metadataStyle,
                ),
              ),
              if (active && speedLabel != null) ...[
                const Gap(AppSpacing.sm),
                Text(
                  speedLabel!,
                  style: theme.typography.small.copyWith(
                    color: AppTheme.statusColor(context, AppStatusTone.info),
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ] else
                Text(
                  operation.label,
                  style: metadataStyle.copyWith(
                    color: operation.phase == 'completed'
                        ? AppTheme.statusColor(context, AppStatusTone.success)
                        : metadataStyle.color,
                  ),
                ),
            ],
          ),
          if (active) ...[
            const Gap(AppSpacing.xs),
            Row(
              children: [
                Text(
                  '${formatBytes(operation.written)} / ${formatBytes(operation.total)}',
                  style: metadataStyle,
                ),
                const Spacer(),
                Text(
                  operation.total > 0
                      ? '${(operation.progress! * 100).round()}%'
                      : operation.label,
                  style: metadataStyle.copyWith(fontWeight: FontWeight.w600),
                ),
              ],
            ),
            const Gap(AppSpacing.xs),
            SizedBox(
              height: AppSpacing.xs,
              child: LinearProgressIndicator(
                value: operation.progress,
                showSparks: false,
                disableAnimation: MediaQuery.disableAnimationsOf(context),
              ),
            ),
          ] else ...[
            const Gap(AppSpacing.xs),
            Text(
              '${operation.label} · ${formatBytes(operation.written)}',
              style: metadataStyle,
            ),
          ],
        ],
      ),
    );
  }
}
