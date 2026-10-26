import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/format_bytes.dart';
import '../file_operations_controller.dart';

/// A compact transfer record proposed for the downloads manager preview.
///
/// [speedLabel] is presentation-only until the transfer runtime reports speed.
class CompactCardConcept extends StatelessWidget {
  const CompactCardConcept({
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

  bool get _completed => operation.phase == 'completed';

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final tone = _completed
        ? AppStatusTone.success
        : operation.failed || operation.phase == 'cancelled'
        ? AppStatusTone.error
        : AppStatusTone.info;
    final statusColor = AppTheme.statusColor(context, tone);
    final height = _completed
        ? AppControlSize.touch * 2 + AppSpacing.lg
        : AppControlSize.large * 3;

    return SizedBox(
      height: height,
      child: OutlinedContainer(
        theme: OutlinedContainerTheme(
          backgroundColor: colors.muted,
          borderColor: colors.border.withValues(alpha: 0.08),
          borderRadius: const BorderRadius.all(Radius.circular(AppRadius.md)),
          padding: EdgeInsets.zero,
        ),
        child: Row(
          children: [
            Container(width: AppSpacing.xs, color: statusColor),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.md),
                child: _completed
                    ? _CompletedRecord(
                        operation: operation,
                        statusColor: statusColor,
                        onOpenFile: onOpenFile,
                        onRemove: onRemove,
                      )
                    : _ActiveRecord(
                        operation: operation,
                        statusColor: statusColor,
                        speedLabel: speedLabel,
                        onCancel: onCancel,
                      ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ActiveRecord extends StatelessWidget {
  const _ActiveRecord({
    required this.operation,
    required this.statusColor,
    required this.speedLabel,
    required this.onCancel,
  });

  final FileOperation operation;
  final Color statusColor;
  final String? speedLabel;
  final VoidCallback? onCancel;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final canCancel =
        operation.active &&
        operation.kind == FileOperationKind.receive &&
        onCancel != null;
    final metadataStyle = TextStyle(
      color: colors.mutedForeground,
      fontSize: AppTypographySize.historyMetadata,
    );

    return Column(
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
                style: const TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            if (canCancel) ...[
              const Gap(AppSpacing.sm),
              SizedBox(
                width: AppControlSize.regular,
                height: AppControlSize.regular,
                child: Button.ghost(
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.ghostIcon(),
                  ),
                  onPressed: onCancel,
                  child: const AppIconFrame(child: Icon(LucideIcons.x)),
                ),
              ),
            ],
          ],
        ),
        const Gap(AppSpacing.xs),
        Text(
          '${operation.sourceDevice} → ${operation.destinationDevice}',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: metadataStyle,
        ),
        const Gap(AppSpacing.sm),
        Row(
          children: [
            Text(
              speedLabel ?? operation.label,
              style: Theme.of(
                context,
              ).typography.small.copyWith(color: statusColor),
            ),
            const Spacer(),
            Text(
              '${formatBytes(operation.written)} / ${formatBytes(operation.total)}',
              style: metadataStyle,
            ),
            const Gap(AppSpacing.sm),
            Text(_percent(operation.progress), style: metadataStyle),
          ],
        ),
        const Gap(AppSpacing.xs),
        LinearProgressIndicator(
          value: operation.progress,
          minHeight: AppSpacing.xs,
          color: statusColor,
          showSparks: false,
          disableAnimation: MediaQuery.disableAnimationsOf(context),
          borderRadius: const BorderRadius.all(Radius.circular(AppRadius.full)),
        ),
      ],
    );
  }
}

class _CompletedRecord extends StatelessWidget {
  const _CompletedRecord({
    required this.operation,
    required this.statusColor,
    required this.onOpenFile,
    required this.onRemove,
  });

  final FileOperation operation;
  final Color statusColor;
  final VoidCallback? onOpenFile;
  final VoidCallback? onRemove;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final metadataStyle = TextStyle(
      color: colors.mutedForeground,
      fontSize: AppTypographySize.historyMetadata,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          operation.name,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: Theme.of(
            context,
          ).typography.small.copyWith(fontWeight: FontWeight.w600),
        ),
        const Gap(AppSpacing.xs),
        Text(
          '${operation.label} · ${formatBytes(operation.total)} · '
          '${operation.sourceDevice} → ${operation.destinationDevice}',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: metadataStyle,
        ),
        const Gap(AppSpacing.sm),
        Row(
          children: [
            if (onOpenFile != null)
              SizedBox(
                height: AppControlSize.regular,
                child: Button.link(
                  onPressed: onOpenFile,
                  leading: const AppIconFrame(
                    child: Icon(LucideIcons.folderOpen),
                  ),
                  child: const Text('Show file'),
                ),
              ),
            if (onRemove != null) ...[
              if (onOpenFile != null) const Gap(AppSpacing.sm),
              SizedBox(
                width: AppControlSize.regular,
                height: AppControlSize.regular,
                child: Button.ghost(
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.ghostIcon(),
                  ),
                  onPressed: onRemove,
                  child: const AppIconFrame(child: Icon(LucideIcons.trash2)),
                ),
              ),
            ],
          ],
        ),
      ],
    );
  }
}

String _percent(double? progress) {
  if (progress == null) return '—';
  return '${(progress * 100).round()}%';
}
