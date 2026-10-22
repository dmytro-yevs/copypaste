import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/format_bytes.dart';
import '../../shared/state_view.dart';
import 'file_operations_controller.dart';

/// Compact presentation for one file operation in the transfer manager.
class FileOperationTile extends StatelessWidget {
  const FileOperationTile({
    super.key,
    required this.operation,
    this.speedLabel,
    this.onCancel,
    this.onOpenFile,
    this.onRemove,
    this.onRetry,
    this.sourceDeviceIcon,
    this.destinationDeviceIcon,
  });

  final FileOperation operation;
  final String? speedLabel;
  final VoidCallback? onCancel;
  final VoidCallback? onOpenFile;
  final VoidCallback? onRemove;
  final VoidCallback? onRetry;
  final IconData? sourceDeviceIcon;
  final IconData? destinationDeviceIcon;

  @override
  Widget build(BuildContext context) {
    if (operation.error case final error?) {
      return _ErroredFileOperation(
        operation: operation,
        error: error,
        onRetry: onRetry,
        onRemove: onRemove,
        sourceDeviceIcon: sourceDeviceIcon,
        destinationDeviceIcon: destinationDeviceIcon,
      );
    }
    if (operation.active) {
      return _ActiveFileOperation(
        operation: operation,
        speedLabel: speedLabel,
        onCancel: onCancel,
        sourceDeviceIcon: sourceDeviceIcon,
        destinationDeviceIcon: destinationDeviceIcon,
      );
    }
    if (operation.phase == 'completed') {
      return _CompletedFileOperation(
        operation: operation,
        onOpenFile: onOpenFile,
        onRemove: onRemove,
        sourceDeviceIcon: sourceDeviceIcon,
        destinationDeviceIcon: destinationDeviceIcon,
      );
    }
    return _TerminalFileOperation(
      operation: operation,
      onRetry: onRetry,
      onRemove: onRemove,
    );
  }
}

class _ErroredFileOperation extends StatelessWidget {
  const _ErroredFileOperation({
    required this.operation,
    required this.error,
    required this.onRetry,
    required this.onRemove,
    required this.sourceDeviceIcon,
    required this.destinationDeviceIcon,
  });

  final FileOperation operation;
  final String error;
  final VoidCallback? onRetry;
  final VoidCallback? onRemove;
  final IconData? sourceDeviceIcon;
  final IconData? destinationDeviceIcon;

  @override
  Widget build(BuildContext context) => Card(
    padding: EdgeInsets.zero,
    child: Padding(
      padding: const EdgeInsets.all(AppSpacing.md),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(child: _OperationName(operation: operation, lines: 1)),
              if (onRemove != null) ...[
                const Gap(AppSpacing.sm),
                Tooltip(
                  tooltip: (_) => const TooltipContainer(
                    child: Text('Remove from history'),
                  ),
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
          const Gap(AppSpacing.xs),
          _RouteLine(
            operation: operation,
            sourceDeviceIcon: sourceDeviceIcon,
            destinationDeviceIcon: destinationDeviceIcon,
          ),
          StateView.error(
            title: operation.label,
            message: error,
            actionLabel: onRetry == null ? null : 'Retry',
            onAction: onRetry,
          ),
        ],
      ),
    ),
  );
}

class _ActiveFileOperation extends StatelessWidget {
  const _ActiveFileOperation({
    required this.operation,
    required this.speedLabel,
    required this.onCancel,
    required this.sourceDeviceIcon,
    required this.destinationDeviceIcon,
  });

  final FileOperation operation;
  final String? speedLabel;
  final VoidCallback? onCancel;
  final IconData? sourceDeviceIcon;
  final IconData? destinationDeviceIcon;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    final trailingInset = onCancel == null
        ? AppSpacing.zero
        : AppTheme.controlHeight(context) + AppSpacing.sm;
    return Card(
      padding: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.md),
        child: Stack(
          children: [
            Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: EdgeInsetsDirectional.only(end: trailingInset),
                  child: _OperationName(operation: operation),
                ),
                const Gap(AppSpacing.xs),
                Row(
                  children: [
                    Expanded(
                      child: _RouteLine(
                        operation: operation,
                        sourceDeviceIcon: sourceDeviceIcon,
                        destinationDeviceIcon: destinationDeviceIcon,
                      ),
                    ),
                    const Gap(AppSpacing.sm),
                    Text(
                      speedLabel ?? operation.label,
                      style: Theme.of(context).typography.small.copyWith(
                        color: AppTheme.statusColor(
                          context,
                          AppStatusTone.info,
                        ),
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ],
                ),
                const Gap(AppSpacing.sm),
                SizedBox(
                  height: AppSpacing.xs,
                  child: LinearProgressIndicator(
                    value: operation.progress,
                    showSparks: false,
                    disableAnimation: MediaQuery.disableAnimationsOf(context),
                  ),
                ),
                const Gap(AppSpacing.xs),
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        '${formatBytes(operation.written)} / ${formatBytes(operation.total)}',
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context).typography.xSmall.copyWith(
                          color: colors.mutedForeground,
                        ),
                      ),
                    ),
                    const Gap(AppSpacing.sm),
                    Text(
                      _percentage(operation.progress),
                      style: Theme.of(context).typography.xSmall.copyWith(
                        color: colors.mutedForeground,
                      ),
                    ),
                  ],
                ),
              ],
            ),
            if (onCancel != null)
              PositionedDirectional(
                top: -AppSpacing.sm,
                end: -AppSpacing.sm,
                child: Tooltip(
                  tooltip: (_) =>
                      const TooltipContainer(child: Text('Cancel download')),
                  child: Button.ghost(
                    style: AppTheme.controlButtonStyle(
                      const ButtonStyle.ghostIcon(),
                    ),
                    onPressed: onCancel,
                    child: const AppIconFrame(child: Icon(LucideIcons.x)),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _CompletedFileOperation extends StatelessWidget {
  const _CompletedFileOperation({
    required this.operation,
    required this.onOpenFile,
    required this.onRemove,
    required this.sourceDeviceIcon,
    required this.destinationDeviceIcon,
  });

  final FileOperation operation;
  final VoidCallback? onOpenFile;
  final VoidCallback? onRemove;
  final IconData? sourceDeviceIcon;
  final IconData? destinationDeviceIcon;

  @override
  Widget build(BuildContext context) {
    final colors = Theme.of(context).colorScheme;
    return Card(
      padding: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(AppSpacing.md),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                AppIconFrame(
                  size: AppIconSize.sm,
                  child: Icon(
                    LucideIcons.circleCheck,
                    size: AppIconSize.sm,
                    color: AppTheme.statusColor(context, AppStatusTone.success),
                  ),
                ),
                const Gap(AppSpacing.sm),
                Expanded(child: _OperationName(operation: operation, lines: 1)),
              ],
            ),
            const Gap(AppSpacing.xs),
            _RouteLine(
              operation: operation,
              sourceDeviceIcon: sourceDeviceIcon,
              destinationDeviceIcon: destinationDeviceIcon,
            ),
            const Gap(AppSpacing.sm),
            Row(
              children: [
                Text(
                  formatBytes(operation.total),
                  style: Theme.of(
                    context,
                  ).typography.xSmall.copyWith(color: colors.mutedForeground),
                ),
                const Spacer(),
                if (onOpenFile != null)
                  Button.secondary(
                    onPressed: onOpenFile,
                    leading: const AppIconFrame(
                      child: Icon(LucideIcons.folderOpen),
                    ),
                    child: const Text('Show file'),
                  ),
                if (onOpenFile != null && onRemove != null)
                  const Gap(AppSpacing.sm),
                if (onRemove != null)
                  Tooltip(
                    tooltip: (_) => const TooltipContainer(
                      child: Text('Remove from history'),
                    ),
                    child: Button.ghost(
                      style: AppTheme.controlButtonStyle(
                        const ButtonStyle.ghostIcon(),
                      ),
                      onPressed: onRemove,
                      child: const AppIconFrame(
                        child: Icon(LucideIcons.trash2),
                      ),
                    ),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _TerminalFileOperation extends StatelessWidget {
  const _TerminalFileOperation({
    required this.operation,
    required this.onRetry,
    required this.onRemove,
  });

  final FileOperation operation;
  final VoidCallback? onRetry;
  final VoidCallback? onRemove;

  @override
  Widget build(BuildContext context) => Card(
    padding: EdgeInsets.zero,
    child: Padding(
      padding: const EdgeInsets.all(AppSpacing.md),
      child: Row(
        children: [
          AppIconFrame(
            size: AppIconSize.sm,
            child: Icon(
              LucideIcons.circleAlert,
              size: AppIconSize.sm,
              color: AppTheme.statusColor(context, AppStatusTone.muted),
            ),
          ),
          const Gap(AppSpacing.sm),
          Expanded(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _OperationName(operation: operation, lines: 1),
                const Gap(AppSpacing.xs),
                Text(operation.label).muted().textSmall(),
              ],
            ),
          ),
          if (onRetry != null) ...[
            const Gap(AppSpacing.sm),
            Button.secondary(
              onPressed: onRetry,
              leading: const AppIconFrame(child: Icon(LucideIcons.rotateCcw)),
              child: const Text('Retry'),
            ),
          ],
          if (onRemove != null) ...[
            const Gap(AppSpacing.sm),
            Tooltip(
              tooltip: (_) =>
                  const TooltipContainer(child: Text('Remove from history')),
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
    ),
  );
}

class _OperationName extends StatelessWidget {
  const _OperationName({required this.operation, this.lines = 2});

  final FileOperation operation;
  final int lines;

  @override
  Widget build(BuildContext context) => Text(
    operation.name,
    maxLines: lines,
    overflow: TextOverflow.ellipsis,
    style: Theme.of(
      context,
    ).typography.small.copyWith(fontWeight: FontWeight.w600),
  );
}

class _RouteLine extends StatelessWidget {
  const _RouteLine({
    required this.operation,
    required this.sourceDeviceIcon,
    required this.destinationDeviceIcon,
  });

  final FileOperation operation;
  final IconData? sourceDeviceIcon;
  final IconData? destinationDeviceIcon;

  @override
  Widget build(BuildContext context) {
    final textStyle = Theme.of(context).typography.xSmall.copyWith(
      color: Theme.of(context).colorScheme.mutedForeground,
    );
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        AppIconFrame(
          size: AppIconSize.sm,
          child: Icon(
            sourceDeviceIcon ?? LucideIcons.circleHelp,
            size: AppIconSize.sm,
          ),
        ),
        const Gap(AppSpacing.xs),
        Flexible(
          child: Text(
            operation.sourceDevice,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: textStyle,
          ),
        ),
        const Gap(AppSpacing.xs),
        const AppIconFrame(
          size: AppIconSize.sm,
          child: Icon(LucideIcons.arrowRight, size: AppIconSize.sm),
        ),
        const Gap(AppSpacing.xs),
        AppIconFrame(
          size: AppIconSize.sm,
          child: Icon(
            destinationDeviceIcon ?? LucideIcons.circleHelp,
            size: AppIconSize.sm,
          ),
        ),
        const Gap(AppSpacing.xs),
        Flexible(
          child: Text(
            operation.destinationDevice,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: textStyle,
          ),
        ),
      ],
    );
  }
}

String _percentage(double? progress) {
  if (progress == null) return '—';
  return '${(progress * 100).round()}%';
}
