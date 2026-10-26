import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app/theme/app_toast.dart';
import 'app/theme/app_theme.dart';
import 'app/theme/app_tokens.dart';
import 'features/transfers/design_preview/compact_card_concept.dart';
import 'features/transfers/design_preview/compact_ledger_concept.dart';
import 'features/transfers/file_operation_tile.dart';
import 'features/transfers/file_operations_controller.dart';

void main() => runApp(const TransferDesignPreviewApp());

/// Runnable design review; demo callbacks never touch real transfers or files.
class TransferDesignPreviewApp extends StatelessWidget {
  const TransferDesignPreviewApp({super.key, this.concept});

  final int? concept;

  @override
  Widget build(BuildContext context) => ShadcnApp(
    debugShowCheckedModeBanner: false,
    theme: AppTheme.dark.copyWith(
      typography: () => AppTheme.dark.typography.copyWith(
        sans: () => const TextStyle(
          fontFamily: 'GeistSans',
          package: 'shadcn_flutter',
          leadingDistribution: TextLeadingDistribution.even,
        ),
      ),
    ),
    builder: AppTheme.builder,
    home: Scaffold(
      child: LayoutBuilder(
        builder: (context, constraints) {
          final selected = concept ?? (constraints.maxWidth < 1000 ? 0 : null);
          if (selected != null) return _PreviewPanel(concept: selected);
          return const Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Expanded(child: _PreviewPanel(concept: 0)),
              VerticalDivider(),
              Expanded(child: _PreviewPanel(concept: 1)),
              VerticalDivider(),
              Expanded(child: _PreviewPanel(concept: 2)),
            ],
          );
        },
      ),
    ),
  );
}

class _PreviewPanel extends StatefulWidget {
  const _PreviewPanel({required this.concept});
  final int concept;

  @override
  State<_PreviewPanel> createState() => _PreviewPanelState();
}

class _PreviewPanelState extends State<_PreviewPanel> {
  var _phase = 'running';
  var _showCompleted = true;

  @override
  Widget build(BuildContext context) {
    final active = FileOperation(
      id: 'active',
      itemId: 'active',
      name: 'MobLand.2025.S01E01.Stick.or.Twist.mkv',
      kind: FileOperationKind.receive,
      phase: _phase,
      sourceDevice: 'CPH2305',
      destinationDevice: 'MacBook Air',
      written: (22.9 * 1024 * 1024).round(),
      total: (1143.3 * 1024 * 1024).round(),
      startedAt: DateTime(2026, 10, 21, 22, 13),
    );
    final completed = FileOperation(
      id: 'completed',
      itemId: 'completed',
      name: 'MobLand.2025.S01E03.Plan.B.mkv',
      kind: FileOperationKind.receive,
      phase: 'completed',
      sourceDevice: 'CPH2305',
      destinationDevice: 'This device',
      written: (567.5 * 1024 * 1024).round(),
      total: (567.5 * 1024 * 1024).round(),
      startedAt: DateTime.fromMillisecondsSinceEpoch(0),
      localPath: '/demo/MobLand.2025.S01E03.Plan.B.mkv',
    );
    Widget record(FileOperation operation) {
      final speed = operation.active ? '8.4 MB/s' : null;
      final VoidCallback? cancel = operation.active
          ? () => setState(() => _phase = 'cancelled')
          : null;
      final VoidCallback? remove = operation.id == 'completed'
          ? () => setState(() => _showCompleted = false)
          : null;
      void open() => AppToast.show(
        context,
        title: 'Preview only',
        message: 'No file is opened.',
      );
      return KeyedSubtree(
        key: ValueKey('record-${operation.id}'),
        child: switch (widget.concept) {
          0 => CompactCardConcept(
            operation: operation,
            speedLabel: speed,
            onCancel: cancel,
            onOpenFile: operation.active ? null : open,
            onRemove: remove,
          ),
          1 => CompactLedgerConcept(
            operation: operation,
            speedLabel: speed,
            onCancel: cancel,
            onOpenFile: operation.active ? null : open,
            onRemove: remove,
          ),
          _ => FileOperationTile(
            operation: operation,
            speedLabel: speed,
            onCancel: cancel,
            onOpenFile: operation.active ? null : open,
            onRemove: remove,
            sourceDeviceIcon: LucideIcons.smartphone,
            destinationDeviceIcon: LucideIcons.laptop,
          ),
        },
      );
    }

    return Scaffold(
      headers: [
        AppBar(
          title: const Text('Downloads and transfers'),
          subtitle: Text(
            '${widget.concept + 1}. ${['Card', 'Ledger', 'Inline'][widget.concept]} · ${active.active ? 1 : 0} active',
          ),
          trailing: [
            Button.ghost(
              style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
              onPressed: () {},
              child: const AppIconFrame(child: Icon(LucideIcons.x)),
            ),
          ],
        ),
        const Divider(),
      ],
      footers: [
        const Divider(),
        Padding(
          padding: const EdgeInsets.all(AppSpacing.lg),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Button.outline(
              onPressed: _showCompleted
                  ? () => setState(() => _showCompleted = false)
                  : null,
              leading: const AppIconFrame(child: Icon(LucideIcons.trash2)),
              child: const Text('Clear completed history'),
            ),
          ),
        ),
      ],
      child: ListView(
        padding: const EdgeInsets.all(AppSpacing.lg),
        children: [
          record(active),
          const Gap(AppSpacing.lg),
          Text(
            'Completed',
            style: Theme.of(context).typography.xSmall.copyWith(
              color: Theme.of(context).colorScheme.mutedForeground,
            ),
          ),
          const Gap(AppSpacing.sm),
          if (_showCompleted) record(completed),
        ],
      ),
    );
  }
}
