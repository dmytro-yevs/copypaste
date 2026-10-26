import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

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
import '../../devices/device_label.dart';
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

  SyncStatus get _status => widget.controller.syncStatus;

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
      return Tooltip(
        showDuration: AppMotion.resolve(context, AppMotion.standard),
        tooltip: (context) => TooltipContainer(child: Text(_status.label)),
        child: Button.secondary(
          key: const ValueKey<String>('sync-header-open'),
          style: AppTheme.controlButtonStyle(const ButtonStyle.secondaryIcon()),
          onPressed: widget.devices?.pairingInspectorOpen == true
              ? null
              : _open,
          child: Semantics(
            label: 'Synchronization: ${_status.label}',
            child: AppIconFrame(
              child: Icon(
                LucideIcons.network,
                color: _toneColor(context, _status.tone),
              ),
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
        AppOverlays.drawerConfiguration(context),
        builder: (context) => ConstrainedBox(
          key: const ValueKey<String>('sync-details-drawer'),
          constraints: AppOverlays.drawerContentConstraints(context),
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
                  status: _status,
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

AppStatusTone _statusTone(SyncTone tone) => switch (tone) {
  SyncTone.muted => AppStatusTone.muted,
  SyncTone.info => AppStatusTone.info,
  SyncTone.success => AppStatusTone.success,
  SyncTone.warning => AppStatusTone.warning,
  SyncTone.error => AppStatusTone.error,
};
Color _toneColor(BuildContext context, SyncTone tone) =>
    AppTheme.statusColor(context, _statusTone(tone));

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
        subtitle: Text(
          status.label,
          style: AppTheme.deviceMetadataTextStyle(context),
        ),
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
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.ghostIcon(),
                ),
                onPressed: onClose,
                child: const AppIconFrame(child: Icon(LucideIcons.x)),
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
        : ListView.builder(
            padding: const EdgeInsets.all(AppSpacing.lg),
            itemCount:
                status.peers.length +
                (status.diagnosticsAvailable ? 2 : 3) +
                (status.log.isEmpty ? 1 : status.log.length),
            itemBuilder: (context, index) {
              if (!status.diagnosticsAvailable) {
                if (index == 0) {
                  return const Padding(
                    padding: EdgeInsets.only(bottom: AppSpacing.lg),
                    child: Alert(
                      title: Text('Activity log unavailable'),
                      content: Text(
                        'Sync activity could not be read or saved.',
                      ),
                    ),
                  );
                }
                index -= 1;
              }
              if (index == 0) {
                return Text(
                  'Devices',
                  style: AppTheme.deviceCardTextStyle(context, muted: true),
                ).semiBold();
              }
              index -= 1;
              if (index == status.peers.length) {
                return Padding(
                  padding: const EdgeInsets.only(
                    top: AppSpacing.xxl,
                    bottom: AppSpacing.md,
                  ),
                  child: Text(
                    'Activity',
                    style: AppTheme.deviceCardTextStyle(context, muted: true),
                  ).semiBold(),
                );
              }
              if (index > status.peers.length) {
                if (status.log.isEmpty) {
                  return Text(
                    'No recent synchronization problems',
                    style: AppTheme.deviceMetadataTextStyle(context),
                  );
                }
                final entry = status.log[index - status.peers.length - 1];
                final peer = status.peers
                    .where((peer) => peer.id == entry.peerId)
                    .firstOrNull;
                return Padding(
                  padding: const EdgeInsets.only(bottom: AppSpacing.lg),
                  child: _SyncLogRow(
                    entry: entry,
                    name: peer?.name ?? 'Paired device',
                  ),
                );
              }
              final peer = status.peers[index];
              final device = devices?.snapshot?.peers
                  .where((device) => device.id == peer.id)
                  .firstOrNull;
              return _SyncPeerRow(
                key: ValueKey('sync-peer-${peer.id}'),
                peer: peer,
                deviceClass:
                    device?.details?.profile?.deviceClass ??
                    DeviceClass.unknown,
                phase: status.phase == SyncPhase.unavailable
                    ? SyncPhase.unavailable
                    : peer.phase,
                ping: device == null
                    ? '— ms'
                    : devices!.peerLatencyLabel(device),
                presence: peer.connectionLabel,
                endpoint: device?.details?.endpoint?.lanEndpoint,
                lastSeen: device?.lastSeen,
              );
            },
          ),
  );
}

class _SyncPeerRow extends StatelessWidget {
  const _SyncPeerRow({
    super.key,
    required this.peer,
    required this.deviceClass,
    required this.phase,
    required this.ping,
    required this.presence,
    this.endpoint,
    this.lastSeen,
  });

  final PeerSyncStatus peer;
  final DeviceClass deviceClass;
  final SyncPhase phase;
  final String ping;
  final String presence;
  final String? endpoint;
  final DateTime? lastSeen;

  @override
  Widget build(BuildContext context) {
    final rows = <({String label, String value})>[
      (label: 'Connection', value: presence),
      (label: 'Ping', value: ping),
      if (endpoint case final value?) (label: 'LAN endpoint', value: value),
      if (peer.startedAt case final value?)
        (
          label: 'Last check started',
          value: formatSystemDateTime(context, value),
        ),
      for (final issue in peer.issues)
        (
          label: issue.title,
          value:
              '${issue.code} · ${formatSystemDateTime(context, issue.since)}',
        ),
      if (peer.skippedTooLarge > 0)
        (label: 'Items over the size limit', value: '${peer.skippedTooLarge}'),
    ];
    final statusLabel = peer.blocker != null
        ? peer.connectionLabel
        : phase == SyncPhase.unavailable
        ? phase.label
        : peer.label;
    final time = peer.offline
        ? lastSeen == null
              ? null
              : 'Last seen ${formatSystemRecentTime(context, lastSeen!)}'
        : peer.lastSuccess == null
        ? null
        : 'Last synced ${formatSystemRecentTime(context, peer.lastSuccess!)}';
    if (peer.lastSuccess case final success?) {
      rows.add((
        label: 'Last successful sync',
        value: formatSystemDateTime(context, success),
      ));
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.symmetric(vertical: AppSpacing.md),
          child: Collapsible(
            children: [
              Builder(
                builder: (context) {
                  final expanded = Data.of<CollapsibleStateData>(context);
                  return Row(
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            DeviceLabel(
                              name: peer.name,
                              deviceClass: deviceClass,
                            ).semiBold(),
                            const Gap(AppSpacing.xs),
                            Text.rich(
                              TextSpan(
                                children: [
                                  TextSpan(
                                    text: statusLabel,
                                    style: AppTheme.deviceCardTextStyle(context)
                                        .copyWith(
                                          color: _toneColor(
                                            context,
                                            peer.blocker != null
                                                ? SyncTone.muted
                                                : peer.tone,
                                          ),
                                        ),
                                  ),
                                  if (time != null)
                                    TextSpan(
                                      text: ' · $time',
                                      style: AppTheme.deviceMetadataTextStyle(
                                        context,
                                      ),
                                    ),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                      const Gap(AppSpacing.sm),
                      Tooltip(
                        tooltip: (context) => const TooltipContainer(
                          child: Text('Connection details'),
                        ),
                        child: Semantics(
                          label: 'Connection details for ${peer.name}',
                          button: true,
                          expanded: expanded.isExpanded,
                          child: Button.ghost(
                            key: ValueKey('sync-peer-details-${peer.id}'),
                            style: AppTheme.controlButtonStyle(
                              const ButtonStyle.ghostIcon(),
                            ),
                            onPressed: expanded.handleTap,
                            child: AppIconFrame(
                              child: Icon(
                                expanded.isExpanded
                                    ? LucideIcons.chevronUp
                                    : LucideIcons.chevronDown,
                              ),
                            ),
                          ),
                        ),
                      ),
                    ],
                  );
                },
              ),
              for (final issue in peer.visibleIssues) ...[
                const Gap(AppSpacing.md),
                Alert(
                  destructive: issue.blocking,
                  theme: AppTheme.statusAlertTheme(
                    context,
                    _statusTone(issue.tone),
                  ),
                  leading: AppIconFrame(
                    child: Icon(
                      LucideIcons.circleAlert,
                      color: _toneColor(context, issue.tone),
                    ),
                  ),
                  title: Text(
                    '${issue.title}${issue.count > 0 ? ' (${issue.count})' : ''}',
                    style: AppTheme.deviceCardTextStyle(context)
                        .copyWith(color: _toneColor(context, issue.tone)),
                  ).semiBold(),
                  content: issue.blocking || issue.pending
                      ? Text(
                          issue.message,
                          style: AppTheme.deviceCardTextStyle(context).copyWith(
                            color: Theme.of(context).colorScheme.foreground,
                          ),
                        )
                      : null,
                ),
              ],
              if (peer.legacyProblem) ...[
                const Gap(AppSpacing.md),
                Alert(
                  content: Text(
                    peer.error!,
                    style: AppTheme.deviceCardTextStyle(context),
                  ),
                ),
              ],
              CollapsibleContent(
                child: Padding(
                  padding: const EdgeInsets.only(top: AppSpacing.md),
                  child: InspectorTable(
                    tableKey: ValueKey('sync-peer-metadata-${peer.id}'),
                    rows: [
                      for (final row in rows)
                        (label: row.label, value: SelectableText(row.value)),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
        const Divider(),
      ],
    );
  }
}

class _SyncLogRow extends StatelessWidget {
  const _SyncLogRow({required this.entry, required this.name});
  final SyncLogEntry entry;
  final String name;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      Text(
        '${formatSystemRecentTime(context, entry.timestamp)} · $name',
        style: AppTheme.deviceMetadataTextStyle(context),
      ),
      const Gap(AppSpacing.xs),
      Text(entry.message, style: AppTheme.deviceCardTextStyle(context)),
    ],
  );
}
