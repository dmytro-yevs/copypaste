import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import '../../shared/format_bytes.dart';
import '../../shared/state_view.dart';
import '../devices/device_presentation.dart';
import '../devices/devices_gateway.dart';
import 'file_operations_button_progress.dart';
import 'file_operations_controller.dart';
import 'file_operation_tile.dart';

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
    this.resolveDeviceClass,
  });
  final FileOperationsController controller;
  final ValueChanged<bool>? onDrawerVisibilityChanged;
  final DeviceClass? Function(String displayName)? resolveDeviceClass;
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
      final button = Button.secondary(
        key: const ValueKey('file-operations-open'),
        style: AppTheme.controlButtonStyle(const ButtonStyle.secondaryIcon()),
        onPressed: _open,
        child: const Icon(LucideIcons.download),
      );
      return Tooltip(
        tooltip: (_) => TooltipContainer(
          child: Text(
            active.isEmpty
                ? 'Downloads and transfers'
                : '${active.length} active transfers · ${formatBytes(written)} / ${formatBytes(total)}',
          ),
        ),
        child: active.isEmpty
            ? button
            : FileOperationsButtonProgress(
                progress: total > 0 ? (written / total).clamp(0, 1) : null,
                child: button,
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
          child: _FileOperationsDrawer(
            controller: widget.controller,
            resolveDeviceClass: widget.resolveDeviceClass,
          ),
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
  const _FileOperationsDrawer({
    required this.controller,
    this.resolveDeviceClass,
  });
  final FileOperationsController controller;
  final DeviceClass? Function(String displayName)? resolveDeviceClass;
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
              Button.outline(
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
                final speed = controller.speedFor(item.id);
                final canRetry =
                    !item.active &&
                    item.phase != 'completed' &&
                    item.kind == FileOperationKind.receive &&
                    item.localPath != null;
                return FileOperationTile(
                  key: ValueKey('file-operation-${item.id}'),
                  operation: item,
                  sourceDeviceIcon: DevicePresentation.icon(
                    resolveDeviceClass?.call(item.sourceDevice),
                  ),
                  destinationDeviceIcon: DevicePresentation.icon(
                    resolveDeviceClass?.call(item.destinationDevice),
                  ),
                  speedLabel: speed == null
                      ? null
                      : '${formatBytes(speed.round())}/s',
                  onCancel:
                      item.active && item.kind == FileOperationKind.receive
                      ? () => controller.cancel(item)
                      : null,
                  onOpenFile:
                      item.phase == 'completed' && item.localPath != null
                      ? () => controller.openFile(item)
                      : null,
                  onRemove: !item.active
                      ? () => controller.removeHistory(item)
                      : null,
                  onRetry:
                      canRetry &&
                          controller.forClip(item.itemId)?.active != true
                      ? () => controller.retry(item)
                      : null,
                );
              },
            ),
    ),
  );
}
