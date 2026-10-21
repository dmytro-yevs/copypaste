import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/format_bytes.dart';
import '../../shared/state_view.dart';
import '../../shared/system_date_time.dart';
import 'file_operations_controller.dart';

/// Shared byte and phase presentation for the manager and clip inspector.
class FileOperationStatus extends StatelessWidget {
  const FileOperationStatus({super.key, required this.operation});
  final FileOperation operation;
  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      if (operation.error case final error?)
        StateView.error(title: operation.label, message: error)
      else
        Text(
          operation.kind == FileOperationKind.import
              ? '${operation.label} · ${formatBytes(operation.total)}'
              : '${operation.label} · ${formatBytes(operation.written)} / ${formatBytes(operation.total)}',
        ),
      if (operation.active) ...[
        const Gap(AppSpacing.sm),
        LinearProgressIndicator(
          value: operation.progress,
          showSparks: false,
          disableAnimation: MediaQuery.disableAnimationsOf(context),
        ),
      ],
    ],
  );
}

class FileOperationsHeaderAction extends StatefulWidget {
  const FileOperationsHeaderAction({
    super.key,
    required this.controller,
    this.onDrawerVisibilityChanged,
  });
  final FileOperationsController controller;
  final ValueChanged<bool>? onDrawerVisibilityChanged;
  @override
  State<FileOperationsHeaderAction> createState() =>
      _FileOperationsHeaderActionState();
}

class _FileOperationsHeaderActionState
    extends State<FileOperationsHeaderAction> {
  OverlayCompleter<void>? _drawer;
  @override
  void dispose() {
    if (_drawer?.isCompleted == false) _drawer!.remove();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: widget.controller,
    builder: (context, _) {
      final active = widget.controller.active.toList(growable: false);
      final total = active.fold<int>(0, (sum, item) => sum + item.total);
      final written = active.fold<int>(0, (sum, item) => sum + item.written);
      return Tooltip(
        tooltip: (_) => TooltipContainer(
          child: Text(
            active.isEmpty
                ? 'Downloads and transfers'
                : '${active.length} active transfers · ${formatBytes(written)} / ${formatBytes(total)}',
          ),
        ),
        child: Stack(
          alignment: Alignment.center,
          children: [
            Button.secondary(
              key: const ValueKey('file-operations-open'),
              style: AppTheme.controlButtonStyle(
                const ButtonStyle.secondaryIcon(),
              ),
              onPressed: _open,
              child: const Icon(LucideIcons.download),
            ),
            if (active.isNotEmpty)
              IgnorePointer(
                child: CircularProgressIndicator(
                  value: total > 0 ? (written / total).clamp(0, 1) : null,
                  animated: !MediaQuery.disableAnimationsOf(context),
                  theme: CircularProgressIndicatorTheme(
                    size: AppTheme.controlHeight(context) - AppSpacing.xs,
                  ),
                ),
              ),
          ],
        ),
      );
    },
  );
  Future<void> _open() async {
    if (_drawer?.isCompleted == false) return;
    widget.onDrawerVisibilityChanged?.call(true);
    unawaited(widget.controller.refresh());
    try {
      final drawer = _drawer = showOverlay<void>(
        context,
        AppOverlays.drawerConfiguration(context),
        builder: (context) => ConstrainedBox(
          constraints: AppOverlays.drawerContentConstraints(context),
          child: _FileOperationsDrawer(controller: widget.controller),
        ),
      );
      await drawer.future;
    } finally {
      _drawer = null;
      widget.onDrawerVisibilityChanged?.call(false);
    }
  }
}

class _FileOperationsDrawer extends StatelessWidget {
  const _FileOperationsDrawer({required this.controller});
  final FileOperationsController controller;
  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: controller,
    builder: (context, _) => Scaffold(
      headers: [
        AppBar(
          title: const Text('Downloads and transfers'),
          subtitle: Text('${controller.active.length} active'),
          trailing: [
            Button.ghost(
              style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
              onPressed: () => closeDrawer(context),
              child: const Icon(LucideIcons.x),
            ),
          ],
        ),
        const Divider(),
      ],
      footers: [
        const Divider(),
        Padding(
          padding: const EdgeInsets.all(AppSpacing.lg),
          child: Wrap(
            spacing: AppSpacing.sm,
            runSpacing: AppSpacing.sm,
            children: [
              if (controller.showingEarlier)
                Button.secondary(
                  onPressed: controller.showLatest,
                  child: const Text('Latest'),
                ),
              if (controller.hasEarlier)
                Button.secondary(
                  onPressed: controller.loadEarlier,
                  child: const Text('Earlier'),
                ),
              Button.secondary(
                onPressed: controller.items.any((item) => !item.active)
                    ? controller.clearHistory
                    : null,
                leading: const Icon(LucideIcons.trash2),
                child: const Text('Clear completed history'),
              ),
            ],
          ),
        ),
      ],
      child: controller.error != null
          ? StateView.error(
              title: 'Transfers',
              message: controller.error,
              actionLabel: 'Try again',
              onAction: controller.refresh,
            )
          : controller.items.isEmpty
          ? const StateView.empty(
              title: 'No transfers',
              message:
                  'File imports, downloads and outgoing transfers appear here.',
            )
          : ListView.separated(
              padding: const EdgeInsets.all(AppSpacing.lg),
              itemCount: controller.items.length,
              separatorBuilder: (_, _) => const Padding(
                padding: EdgeInsets.symmetric(vertical: AppSpacing.md),
                child: Divider(),
              ),
              itemBuilder: (context, index) {
                final item = controller.items[index];
                return Column(
                  key: ValueKey('file-operation-${item.id}'),
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Row(
                      children: [
                        Icon(switch (item.kind) {
                          FileOperationKind.send => LucideIcons.upload,
                          FileOperationKind.receive => LucideIcons.download,
                          FileOperationKind.import => LucideIcons.filePlus,
                        }),
                        const Gap(AppSpacing.sm),
                        Expanded(
                          child: Text(
                            item.name,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        if (item.active &&
                            item.kind == FileOperationKind.receive)
                          Button.ghost(
                            style: AppTheme.controlButtonStyle(
                              const ButtonStyle.ghostIcon(),
                            ),
                            onPressed: () => controller.cancel(item),
                            child: const Icon(LucideIcons.x),
                          ),
                      ],
                    ),
                    const Gap(AppSpacing.sm),
                    Text('${item.sourceDevice} → ${item.destinationDevice}'),
                    const Gap(AppSpacing.sm),
                    FileOperationStatus(operation: item),
                    const Gap(AppSpacing.sm),
                    Text(
                      item.startedAt.millisecondsSinceEpoch == 0
                          ? 'Saved before transfer history was available'
                          : 'Started ${formatSystemDateTime(context, item.startedAt)}',
                    ),
                    if (item.finishedAt case final time?)
                      Text('Finished ${formatSystemDateTime(context, time)}'),
                    if (!item.active &&
                        item.phase != 'completed' &&
                        item.kind == FileOperationKind.receive &&
                        item.localPath != null) ...[
                      const Gap(AppSpacing.sm),
                      Align(
                        alignment: Alignment.centerLeft,
                        child: Button.secondary(
                          onPressed:
                              controller.forClip(item.itemId)?.active == true
                              ? null
                              : () => controller.retry(item),
                          leading: const Icon(LucideIcons.rotateCcw),
                          child: const Text('Retry'),
                        ),
                      ),
                    ],
                    if (item.phase == 'completed' &&
                        item.localPath != null) ...[
                      const Gap(AppSpacing.sm),
                      Align(
                        alignment: Alignment.centerLeft,
                        child: Button.link(
                          onPressed: () => controller.openFile(item),
                          leading: const Icon(LucideIcons.folderOpen),
                          child: Text(
                            item.kind == FileOperationKind.import
                                ? 'Show source file'
                                : 'Show saved file',
                          ),
                        ),
                      ),
                    ],
                  ],
                );
              },
            ),
    ),
  );
}
