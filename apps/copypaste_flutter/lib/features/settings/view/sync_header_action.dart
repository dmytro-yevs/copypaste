import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:flutter/services.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/inspector_table.dart';
import '../../../shared/state_view.dart';
import '../../../shared/system_date_time.dart';
import '../../devices/devices_controller.dart';
import '../../devices/devices_gateway.dart';
import '../controller/settings_controller.dart';
import '../models/sync_status.dart';

class SyncHeaderAction extends StatefulWidget {
  const SyncHeaderAction({
    super.key,
    required this.controller,
    this.devices,
    this.onDrawerVisibilityChanged,
  });

  final SettingsController controller;
  final DevicesController? devices;
  final ValueChanged<bool>? onDrawerVisibilityChanged;

  @override
  State<SyncHeaderAction> createState() => _SyncHeaderActionState();
}

class _SyncHeaderActionState extends State<SyncHeaderAction> {
  final _drawerFocus = FocusNode();
  OverlayCompleter<void>? _drawer;
  bool _opening = false;

  @override
  void dispose() {
    if (_drawer?.isCompleted == false) _drawer!.remove();
    _drawerFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: Listenable.merge([widget.controller, ?widget.devices]),
    builder: (context, _) {
      final phase = widget.controller.syncStatus.phase;
      return Tooltip(
        showDuration: AppMotion.resolve(context, AppMotion.standard),
        tooltip: (context) => TooltipContainer(child: Text(phase.label)),
        child: Button.secondary(
          key: const ValueKey<String>('sync-header-open'),
          style: const ButtonStyle.secondaryIcon(),
          onPressed: widget.devices?.pairingInspectorOpen == true
              ? null
              : _open,
          child: Semantics(
            label: 'Synchronization: ${phase.label}',
            child: Icon(
              LucideIcons.network,
              color: _phaseColor(context, phase),
            ),
          ),
        ),
      );
    },
  );

  Future<void> _open() async {
    if (_opening) return;
    _opening = true;
    widget.onDrawerVisibilityChanged?.call(true);
    try {
      unawaited(widget.devices?.start());
      final drawer = _drawer = showOverlay<void>(
        context,
        AppOverlays.bottomDrawerConfiguration,
        builder: (context) => SizedBox(
          key: const ValueKey<String>('sync-details-drawer'),
          width: double.infinity,
          height:
              MediaQuery.sizeOf(context).height *
              AppOverlaySize.drawerHeightFactor,
          child: CallbackShortcuts(
            bindings: <ShortcutActivator, VoidCallback>{
              const SingleActivator(LogicalKeyboardKey.escape): () =>
                  unawaited(closeDrawer(context)),
            },
            child: Focus(
              focusNode: _drawerFocus,
              autofocus: true,
              child: AnimatedBuilder(
                animation: Listenable.merge([
                  widget.controller,
                  ?widget.devices,
                ]),
                builder: (context, _) => _SyncDetails(
                  status: widget.controller.syncStatus,
                  devices: widget.devices,
                  onClose: () => unawaited(closeDrawer(context)),
                ),
              ),
            ),
          ),
        ),
      );
      // The backdrop navigator can take focus as the drawer mounts.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && identical(_drawer, drawer) && !drawer.isCompleted) {
          _drawerFocus.requestFocus();
        }
      });
      await drawer.future;
    } finally {
      _drawer = null;
      _opening = false;
      widget.onDrawerVisibilityChanged?.call(false);
    }
  }
}

Color _phaseColor(BuildContext context, SyncPhase phase) =>
    AppTheme.statusColor(context, switch (phase) {
      SyncPhase.syncing => AppStatusTone.info,
      SyncPhase.synced => AppStatusTone.success,
      SyncPhase.failed => AppStatusTone.error,
      SyncPhase.unavailable ||
      SyncPhase.disabled ||
      SyncPhase.waiting => AppStatusTone.muted,
    });

class _SyncDetails extends StatelessWidget {
  const _SyncDetails({
    required this.status,
    required this.devices,
    required this.onClose,
  });

  final SyncStatus status;
  final DevicesController? devices;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) => Scaffold(
    headers: [
      AppBar(
        title: const Text('Synchronization'),
        subtitle: Text(status.phase.label),
        leading: [
          Icon(LucideIcons.network, color: _phaseColor(context, status.phase)),
        ],
        trailing: [
          Tooltip(
            tooltip: (context) => const TooltipContainer(
              child: Text('Close synchronization details'),
            ),
            child: Semantics(
              label: 'Close synchronization details',
              button: true,
              child: Button.ghost(
                key: const ValueKey<String>('close-sync-details'),
                style: const ButtonStyle.ghostIcon(),
                onPressed: onClose,
                child: const Icon(LucideIcons.x),
              ),
            ),
          ),
        ],
      ),
      const Divider(),
    ],
    child: status.peers.isEmpty
        ? status.phase == SyncPhase.unavailable
              ? const StateView.error(title: 'Sync state unavailable')
              : const StateView.empty(
                  title: 'No paired devices',
                  message: 'Pair a device to synchronize clipboard history.',
                )
        : ListView.separated(
            padding: const EdgeInsets.all(AppSpacing.lg),
            itemCount: status.peers.length,
            separatorBuilder: (context, index) => const Gap(AppSpacing.md),
            itemBuilder: (context, index) {
              final peer = status.peers[index];
              DevicePeer? device;
              for (final candidate
                  in devices?.snapshot?.peers ?? <DevicePeer>[]) {
                if (candidate.id == peer.id) {
                  device = candidate;
                  break;
                }
              }
              return _SyncPeerCard(
                peer: peer,
                phase: status.phase == SyncPhase.unavailable
                    ? SyncPhase.unavailable
                    : peer.phase,
                ping: device == null
                    ? '— ms'
                    : devices!.peerLatencyLabel(device),
                presence: device == null
                    ? 'Status unknown'
                    : devices!.peerStateLabel(device),
                endpoint: device?.details?.endpoint?.lanEndpoint,
              );
            },
          ),
  );
}

class _SyncPeerCard extends StatelessWidget {
  const _SyncPeerCard({
    required this.peer,
    required this.phase,
    required this.ping,
    required this.presence,
    this.endpoint,
  });

  final PeerSyncStatus peer;
  final SyncPhase phase;
  final String ping;
  final String presence;
  final String? endpoint;

  @override
  Widget build(BuildContext context) {
    final rows = <({String label, String value})>[
      (label: 'Sync', value: phase.label),
      (label: 'Connection', value: presence),
      (label: 'Ping', value: ping),
      if (endpoint case final value?) (label: 'LAN endpoint', value: value),
      if (peer.startedAt case final value?)
        (label: 'Session started', value: formatSystemDateTime(context, value)),
      if (peer.lastSuccess case final value?)
        (
          label: 'Last successful sync',
          value: formatSystemDateTime(context, value),
        ),
      (label: 'Clips sent', value: '${peer.sent}'),
      (label: 'Clips received', value: '${peer.received}'),
      if (peer.skippedTooLarge > 0)
        (label: 'Items over the size limit', value: '${peer.skippedTooLarge}'),
    ];
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                LucideIcons.network,
                color: _phaseColor(context, phase),
                size: AppIconSize.sm,
              ),
              const Gap(AppSpacing.sm),
              Expanded(child: Text(peer.name).semiBold()),
            ],
          ),
          const Gap(AppSpacing.md),
          if (peer.error case final error?) ...[
            Alert.destructive(
              leading: const Icon(LucideIcons.circleAlert),
              title: const Text('Last sync problem'),
              content: Text(error),
            ),
            const Gap(AppSpacing.md),
          ],
          InspectorTable(
            tableKey: ValueKey<String>('sync-peer-metadata-${peer.id}'),
            rows: [
              for (final row in rows)
                (label: row.label, value: SelectableText(row.value)),
            ],
          ),
        ],
      ),
    );
  }
}
