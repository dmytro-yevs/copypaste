import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_motion.dart';
import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import '../../platform/camera/pairing_camera_scanner.dart';
import '../../platform/camera/qr_scanner.dart';
import '../../shared/adaptive_breakpoints.dart';
import '../../shared/inspector_table.dart';
import '../../shared/state_view.dart';
import '../../shared/system_date_time.dart';
import 'device_presentation.dart';
import 'devices_controller.dart';
import 'devices_gateway.dart';

/// Devices management composed from shadcn_flutter primitives.
class DevicesScreen extends StatefulWidget {
  const DevicesScreen({
    super.key,
    required this.controller,
    this.onDrawerVisibilityChanged,
    this.isActive,
  });

  final DevicesController controller;
  final ValueChanged<bool>? onDrawerVisibilityChanged;
  final ValueGetter<bool>? isActive;

  @override
  State<DevicesScreen> createState() => _DevicesScreenState();
}

class _DevicesScreenState extends State<DevicesScreen> {
  bool _pairingDrawerOpen = false;
  bool _deviceDrawerOpen = false;
  OverlayCompleter<void>? _pairingDrawer;
  OverlayCompleter<void>? _deviceDrawer;

  @override
  void initState() {
    super.initState();
    HardwareKeyboard.instance.addHandler(_handleHardwareKey);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(widget.controller.start());
    });
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_handleHardwareKey);
    if (_pairingDrawer?.isCompleted == false) _pairingDrawer!.remove();
    if (_deviceDrawer?.isCompleted == false) _deviceDrawer!.remove();
    super.dispose();
  }

  bool _handleHardwareKey(KeyEvent event) {
    if (!mounted ||
        event is! KeyDownEvent ||
        event.logicalKey != LogicalKeyboardKey.escape ||
        !(widget.isActive?.call() ?? true) ||
        !(ModalRoute.of(context)?.isCurrent ?? true)) {
      return false;
    }
    final controller = widget.controller;
    if (controller.pairingEntryMode != null && controller.canClosePairing) {
      unawaited(controller.closePairing());
      return true;
    }
    if (controller.deviceDetailsOpen) {
      controller.closeDeviceDetails();
      return true;
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final controller = widget.controller;
        return switch (controller.loadState) {
          DevicesLoadState.loading => const StateView.loading(
            message: 'Loading devices…',
          ),
          DevicesLoadState.error => StateView.error(
            title: 'Devices are unavailable',
            message: controller.errorMessage,
            actionLabel: 'Try again',
            onAction: controller.refresh,
          ),
          DevicesLoadState.ready => _readyContent(context, controller),
        };
      },
    );
  }

  Widget _readyContent(BuildContext context, DevicesController controller) {
    final snapshot = controller.snapshot!;
    return LayoutBuilder(
      builder: (context, constraints) {
        final wide = constraints.maxWidth >= AdaptiveBreakpoints.inspector;
        final pairingOpen = controller.pairingInspectorOpen;
        final deviceDetailsOpen = controller.deviceDetailsTarget != null;
        final inspectorOpen = pairingOpen || deviceDetailsOpen;
        if (!wide && pairingOpen && !_pairingDrawerOpen && !_deviceDrawerOpen) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) unawaited(_openPairingDrawer());
          });
        }
        if (!wide &&
            deviceDetailsOpen &&
            !_deviceDrawerOpen &&
            !_pairingDrawerOpen) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) unawaited(_openDeviceDrawer());
          });
        }
        final list = _devicesList(
          context,
          controller,
          snapshot,
          padding: wide && inspectorOpen
              ? const EdgeInsets.all(AppSpacing.sm)
              : const EdgeInsets.all(AppSpacing.xxl),
        );
        if (!wide || !inspectorOpen) return list;
        return Padding(
          padding: const EdgeInsets.all(AppSpacing.lg),
          child: ResizablePanel.horizontal(
            dividerBuilder: (context) => null,
            draggerThickness: AppSpacing.md,
            children: [
              ResizablePane.flex(
                minSize: 440,
                initialFlex: 1.45,
                child: Padding(
                  padding: const EdgeInsets.only(right: AppSpacing.lg),
                  child: SizedBox.expand(
                    key: const ValueKey<String>('devices-list'),
                    child: list,
                  ),
                ),
              ),
              ResizablePane(
                initialSize: 400,
                minSize: 340,
                maxSize: 560,
                child: pairingOpen
                    ? _PairingInspector(
                        key: ValueKey<PairingEntryMode>(
                          controller.pairingEntryMode!,
                        ),
                        controller: controller,
                        inDrawer: false,
                        onClose: controller.closePairing,
                      )
                    : _DeviceDetailsInspector(
                        key: ValueKey<DeviceDetailsTarget>(
                          controller.deviceDetailsTarget!,
                        ),
                        controller: controller,
                        snapshot: snapshot,
                        inDrawer: false,
                        onClose: controller.closeDeviceDetails,
                        onRename: (name) =>
                            _renameThisDevice(context, controller, name),
                        onRemove: (peer, revoke) => _confirmPeerRemoval(
                          context,
                          controller,
                          peer,
                          revoke: revoke,
                        ),
                      ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _devicesList(
    BuildContext context,
    DevicesController controller,
    DevicesSnapshot snapshot, {
    required EdgeInsets padding,
  }) {
    return ListView(
      padding: padding.add(
        EdgeInsets.only(bottom: MediaQuery.paddingOf(context).bottom),
      ),
      children: [
        if (controller.errorMessage case final errorMessage?) ...[
          Alert.destructive(
            leading: const Icon(LucideIcons.circleAlert),
            title: Text(controller.errorTitle),
            content: Text(errorMessage),
          ),
          const Gap(AppSpacing.lg),
        ],
        _deviceCards(context, controller, snapshot),
        const Gap(AppSpacing.xxl),
        _nearbyDevices(context, controller),
      ],
    );
  }

  Widget _nearbyDevices(BuildContext context, DevicesController controller) {
    final devices = controller.nearbyDevices;
    return Card(
      key: const ValueKey<String>('nearby-devices'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(child: const Text('Nearby devices').large().medium()),
              const Gap(AppSpacing.md),
              Button.text(
                key: const ValueKey<String>('rescan-devices'),
                style: const ButtonStyle.text(density: ButtonDensity.compact),
                onPressed:
                    controller.rescanInFlight || controller.actionInFlight
                    ? null
                    : controller.rescan,
                leading: const Icon(LucideIcons.scanLine),
                child: Text(controller.rescanInFlight ? 'Scanning…' : 'Scan'),
              ),
            ],
          ),
          const Gap(AppSpacing.md),
          if (devices.isEmpty)
            ConstrainedBox(
              constraints: const BoxConstraints(
                minHeight: AppControlSize.touch * 4,
              ),
              child: const StateView.empty(
                title: 'No devices nearby',
                compact: true,
                icon: DevicePresentation.collectionIcon,
              ),
            )
          else
            for (var index = 0; index < devices.length; index++) ...[
              if (index > 0) const Divider(),
              _discoveredRow(context, controller, devices[index]),
            ],
        ],
      ),
    );
  }

  Widget _deviceCards(
    BuildContext context,
    DevicesController controller,
    DevicesSnapshot snapshot,
  ) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final columns = constraints.maxWidth >= 680 ? 2 : 1;
        final width =
            (constraints.maxWidth - (columns - 1) * AppSpacing.md) / columns;
        return Wrap(
          spacing: AppSpacing.md,
          runSpacing: AppSpacing.md,
          children: [
            SizedBox(
              width: width,
              child: _thisDeviceCard(context, controller, snapshot.thisDevice),
            ),
            for (final peer in snapshot.peers)
              SizedBox(
                width: width,
                child: _peerCard(context, controller, peer),
              ),
          ],
        );
      },
    );
  }

  Future<void> _openPairingDrawer() async {
    if (_pairingDrawerOpen ||
        _deviceDrawerOpen ||
        !widget.controller.pairingInspectorOpen) {
      return;
    }
    _pairingDrawerOpen = true;
    widget.onDrawerVisibilityChanged?.call(true);
    var closing = false;
    void closeOnce(BuildContext drawerContext) {
      if (closing || !drawerContext.mounted) return;
      closing = true;
      unawaited(closeDrawer(drawerContext));
    }

    try {
      final drawer = _pairingDrawer = showOverlay<void>(
        context,
        AppOverlays.bottomDrawerConfiguration,
        builder: (context) => ConstrainedBox(
          key: const ValueKey<String>('devices-pairing-drawer'),
          constraints: AppOverlays.drawerContentConstraints(context),
          child: Focus(
            autofocus: true,
            child: CallbackShortcuts(
              bindings: <ShortcutActivator, VoidCallback>{
                const SingleActivator(LogicalKeyboardKey.escape): () async {
                  if (widget.controller.canClosePairing) {
                    await widget.controller.closePairing();
                  }
                },
              },
              child: AnimatedBuilder(
                animation: widget.controller,
                builder: (context, _) {
                  if (!widget.controller.pairingInspectorOpen) {
                    WidgetsBinding.instance.addPostFrameCallback((_) {
                      closeOnce(context);
                    });
                    return const SizedBox.shrink();
                  }
                  return _PairingInspector(
                    key: ValueKey<PairingEntryMode?>(
                      widget.controller.pairingEntryMode,
                    ),
                    controller: widget.controller,
                    inDrawer: true,
                    onClose: widget.controller.closePairing,
                  );
                },
              ),
            ),
          ),
        ),
      );
      await drawer.future;
    } finally {
      _pairingDrawer = null;
      _pairingDrawerOpen = false;
      if (mounted) {
        widget.onDrawerVisibilityChanged?.call(false);
        if (!closing && widget.controller.pairingEntryMode != null) {
          await widget.controller.closePairing();
        }
        if (mounted) setState(() {});
      }
    }
  }

  Future<void> _openDeviceDrawer() async {
    if (_deviceDrawerOpen ||
        _pairingDrawerOpen ||
        widget.controller.deviceDetailsTarget == null) {
      return;
    }
    _deviceDrawerOpen = true;
    widget.onDrawerVisibilityChanged?.call(true);
    final target = widget.controller.deviceDetailsTarget;
    var closing = false;
    void closeOnce(BuildContext drawerContext) {
      if (closing || !drawerContext.mounted) return;
      closing = true;
      unawaited(closeDrawer(drawerContext));
    }

    try {
      final drawer = _deviceDrawer = showOverlay<void>(
        context,
        AppOverlays.bottomDrawerConfiguration,
        builder: (context) => ConstrainedBox(
          key: const ValueKey<String>('devices-details-drawer'),
          constraints: AppOverlays.drawerContentConstraints(context),
          child: Focus(
            autofocus: true,
            child: CallbackShortcuts(
              bindings: <ShortcutActivator, VoidCallback>{
                const SingleActivator(LogicalKeyboardKey.escape): () {
                  widget.controller.closeDeviceDetails();
                },
              },
              child: AnimatedBuilder(
                animation: widget.controller,
                builder: (context, _) {
                  final snapshot = widget.controller.snapshot;
                  if (snapshot == null ||
                      widget.controller.deviceDetailsTarget == null) {
                    WidgetsBinding.instance.addPostFrameCallback((_) {
                      closeOnce(context);
                    });
                    return const SizedBox.shrink();
                  }
                  return _DeviceDetailsInspector(
                    key: ValueKey<DeviceDetailsTarget>(
                      widget.controller.deviceDetailsTarget!,
                    ),
                    controller: widget.controller,
                    snapshot: snapshot,
                    inDrawer: true,
                    onClose: widget.controller.closeDeviceDetails,
                    onRename: (name) =>
                        _renameThisDevice(context, widget.controller, name),
                    onRemove: (peer, revoke) => _confirmPeerRemoval(
                      context,
                      widget.controller,
                      peer,
                      revoke: revoke,
                    ),
                  );
                },
              ),
            ),
          ),
        ),
      );
      await drawer.future;
    } finally {
      _deviceDrawer = null;
      _deviceDrawerOpen = false;
      if (mounted) {
        widget.onDrawerVisibilityChanged?.call(false);
        if (!closing && widget.controller.deviceDetailsTarget == target) {
          widget.controller.closeDeviceDetails();
        }
        setState(() {});
      }
    }
  }

  Widget _thisDeviceCard(
    BuildContext context,
    DevicesController controller,
    ThisDevice device,
  ) {
    return _deviceCard(
      context,
      key: const ValueKey<String>('this-device-card'),
      name: device.name,
      details: device.details,
      selected:
          controller.deviceDetailsTarget ==
          const DeviceDetailsTarget.thisDevice(),
      onPressed: controller.pairingInspectorOpen
          ? null
          : controller.openThisDeviceDetails,
    );
  }

  Widget _peerCard(
    BuildContext context,
    DevicesController controller,
    DevicePeer peer,
  ) {
    return _deviceCard(
      context,
      key: ValueKey<String>('peer-device-card-${peer.id}'),
      name: peer.name,
      details: peer.details,
      selected:
          controller.deviceDetailsTarget == DeviceDetailsTarget.peer(peer.id),
      onPressed: controller.pairingInspectorOpen
          ? null
          : () => controller.openPeerDetails(peer.id),
    );
  }

  Widget _discoveredRow(
    BuildContext context,
    DevicesController controller,
    DiscoveredDevice device,
  ) {
    return Basic(
      key: ValueKey<String>('discovered-device-row-${device.id}'),
      theme: AppTheme.deviceListRowTheme,
      leading: _deviceIconTile(
        context,
        device.details?.profile?.deviceClass,
        compact: true,
      ),
      title: Text(device.name),
      subtitle: Text(DevicePresentation.osLabel(device.details?.profile)),
      trailing: Button.primary(
        onPressed:
            !controller.pairingInspectorOpen && controller.canChangePairingMode
            ? () => controller.openCodeEntry(address: device.address)
            : null,
        child: const Text('Pair'),
      ),
    );
  }

  Widget _deviceCard(
    BuildContext context, {
    required Key key,
    required String name,
    required DeviceDetails? details,
    required bool selected,
    required VoidCallback? onPressed,
  }) {
    final colors = Theme.of(context).colorScheme;
    final style = selected
        ? const ButtonStyle.card().withBackgroundColor(
            color: colors.border,
            hoverColor: colors.border,
            focusColor: colors.border,
          )
        : const ButtonStyle.card();
    return Semantics(
      selected: selected,
      button: true,
      child: Button.card(
        key: key,
        onPressed: onPressed,
        enabled: onPressed != null,
        alignment: Alignment.centerLeft,
        style: style,
        child: Row(
          children: [
            _deviceIconTile(context, details?.profile?.deviceClass),
            const Gap(AppSpacing.md),
            Expanded(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ).semiBold(),
                  const Gap(AppSpacing.xs),
                  Text(
                    DevicePresentation.summary(details),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ).muted(),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _deviceIconTile(
    BuildContext context,
    DeviceClass? deviceClass, {
    bool compact = false,
  }) {
    final colors = Theme.of(context).colorScheme;
    return SizedBox.square(
      dimension: compact ? AppControlSize.large : AppControlSize.touch,
      child: Card(
        theme: CardTheme(
          padding: EdgeInsets.zero,
          filled: true,
          fillColor: colors.secondary,
          borderRadius: const BorderRadius.all(Radius.circular(AppRadius.lg)),
          borderWidth: AppSpacing.zero,
        ),
        child: Center(
          child: Icon(
            DevicePresentation.icon(deviceClass),
            size: compact ? AppIconSize.md : AppIconSize.lg,
          ),
        ),
      ),
    );
  }

  Future<void> _renameThisDevice(
    BuildContext context,
    DevicesController controller,
    String currentName,
  ) async {
    final nameController = TextEditingController(text: currentName);
    final nextName = await AppOverlays.showDialog<String>(
      context,
      useRootNavigator: false,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.pencil,
        title: const Text('Rename this device'),
        content: TextField(
          controller: nameController,
          placeholder: const Text('Device name'),
          autofocus: true,
          decoration: AppOverlays.dialogFieldDecoration(dialogContext),
        ),
        actions: [
          Button.ghost(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Cancel'),
          ),
          Button.primary(
            onPressed: () => Navigator.pop(dialogContext, nameController.text),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    nameController.dispose();
    if (nextName != null && nextName.trim().isNotEmpty) {
      await controller.setThisDeviceName(nextName.trim());
    }
  }

  Future<void> _confirmPeerRemoval(
    BuildContext context,
    DevicesController controller,
    DevicePeer peer, {
    required bool revoke,
  }) async {
    await AppOverlays.showDialog<void>(
      context,
      useRootNavigator: false,
      builder: (dialogContext) {
        var submitting = false;
        String? failure;
        return StatefulBuilder(
          builder: (context, setDialogState) => AppOverlays.alertDialog(
            icon: revoke ? LucideIcons.ban : LucideIcons.unlink,
            title: Text(
              revoke ? 'Revoke ${peer.name}?' : 'Unpair ${peer.name}?',
            ),
            content: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  revoke
                      ? 'This permanently prevents this pairing from being used again.'
                      : 'This removes the local pairing key from this device.',
                ),
                if (failure != null) ...[
                  const Gap(AppSpacing.md),
                  Alert.destructive(
                    leading: const Icon(LucideIcons.circleAlert),
                    title: const Text('Action failed'),
                    content: Text(failure!),
                  ),
                ],
              ],
            ),
            actions: [
              Button.ghost(
                onPressed: submitting
                    ? null
                    : () => Navigator.pop(dialogContext),
                child: const Text('Cancel'),
              ),
              Button.destructive(
                onPressed: submitting
                    ? null
                    : () async {
                        setDialogState(() {
                          submitting = true;
                          failure = null;
                        });
                        final succeeded = revoke
                            ? await controller.revoke(peer.id)
                            : await controller.unpair(peer.id);
                        if (!dialogContext.mounted) return;
                        if (succeeded) {
                          Navigator.pop(dialogContext);
                          return;
                        }
                        setDialogState(() {
                          submitting = false;
                          failure =
                              controller.errorMessage ??
                              'The device action could not be completed.';
                        });
                      },
                child: Text(revoke ? 'Revoke pairing' : 'Unpair'),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _DeviceDetailsInspector extends StatelessWidget {
  const _DeviceDetailsInspector({
    super.key,
    required this.controller,
    required this.snapshot,
    required this.inDrawer,
    required this.onClose,
    required this.onRename,
    required this.onRemove,
  });

  final DevicesController controller;
  final DevicesSnapshot snapshot;
  final bool inDrawer;
  final VoidCallback onClose;
  final Future<void> Function(String name) onRename;
  final Future<void> Function(DevicePeer peer, bool revoke) onRemove;

  @override
  Widget build(BuildContext context) {
    final target = controller.deviceDetailsTarget;
    final device = _selectedDevice(target);
    if (device == null) return const SizedBox.shrink();
    final content = Card(
      key: ValueKey<String>(
        inDrawer ? 'devices-details-drawer-card' : 'devices-details-inspector',
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(
                DevicePresentation.icon(device.details?.profile?.deviceClass),
                size: AppIconSize.xl,
              ),
              const Gap(AppSpacing.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(device.name).h3(),
                    Text(DevicePresentation.summary(device.details)).muted(),
                  ],
                ),
              ),
              Tooltip(
                showDuration: AppMotion.resolve(context, AppMotion.standard),
                tooltip: (context) =>
                    const TooltipContainer(child: Text('Close device details')),
                child: Semantics(
                  label: 'Close device details',
                  button: true,
                  child: Button.ghost(
                    key: const ValueKey<String>('close-device-details'),
                    style: const ButtonStyle.ghostIcon(),
                    onPressed: onClose,
                    child: const Icon(LucideIcons.x),
                  ),
                ),
              ),
            ],
          ),
          const Gap(AppSpacing.md),
          Align(
            alignment: Alignment.centerLeft,
            child: SecondaryBadge(child: Text(device.relationship)),
          ),
          if (controller.errorMessage case final errorMessage?) ...[
            const Gap(AppSpacing.md),
            Alert.destructive(
              key: const ValueKey<String>('device-details-action-error'),
              leading: const Icon(LucideIcons.circleAlert),
              title: Text(controller.errorTitle),
              content: Text(errorMessage),
            ),
          ],
          const Gap(AppSpacing.xl),
          Expanded(
            child: SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  _DeviceDetailsTable(rows: _detailRows(context, device)),
                  const Gap(AppSpacing.xxl),
                  _actions(device),
                ],
              ),
            ),
          ),
        ],
      ),
    );
    if (!inDrawer) return content;
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.lg,
        AppSpacing.zero,
        AppSpacing.lg,
        AppSpacing.lg,
      ),
      child: content,
    );
  }

  _DetailsDevice? _selectedDevice(DeviceDetailsTarget? target) {
    if (target == null) return null;
    if (target.isThisDevice) {
      final device = snapshot.thisDevice;
      return _DetailsDevice.thisDevice(device);
    }
    for (final peer in snapshot.peers) {
      if (peer.id == target.id) {
        return _DetailsDevice.peer(
          peer,
          state: controller.peerStateLabel(peer),
          latency: controller.peerLatencyLabel(peer),
          latencyObservedAt: controller.isLatencyFresh(peer.details?.latency)
              ? peer.details?.latency?.observedAt
              : null,
        );
      }
    }
    return null;
  }

  List<_DeviceDetailRow> _detailRows(
    BuildContext context,
    _DetailsDevice device,
  ) {
    return [
      (
        label: 'Device type',
        value: DevicePresentation.classLabel(
          device.details?.profile?.deviceClass,
        ),
      ),
      (
        label: 'Operating system',
        value: DevicePresentation.osLabel(device.details?.profile),
      ),
      if (device.details?.profile?.model case final model?)
        (label: 'Model', value: model),
      if (device.appVersion case final appVersion?)
        (label: 'CopyPaste version', value: appVersion),
      if (device.protocolVersion case final protocolVersion?)
        (label: 'Protocol version', value: '$protocolVersion'),
      if (!device.isThisDevice) (label: 'Status', value: device.state),
      if (!device.isThisDevice) (label: 'Ping', value: device.latency),
      if (device.latencyObservedAt case final measuredAt?)
        (label: 'Measured', value: formatSystemDateTime(context, measuredAt)),
      if (device.lastSeen case final lastSeen?)
        (label: 'Last seen', value: formatSystemDateTime(context, lastSeen)),
      if (device.endpoint case final endpoint?)
        (label: 'LAN endpoint', value: endpoint),
      (label: device.idLabel, value: device.id ?? 'Unavailable'),
      (
        label: 'Profile provenance',
        value: _provenanceLabel(device.details?.profile?.provenance),
      ),
      if (!device.isThisDevice)
        (
          label: 'Profile trust',
          value: _trustLabel(device.details?.profile?.trust),
        ),
      if (device.details?.profile?.observedAt case final observedAt?)
        (
          label: 'Profile updated',
          value: formatSystemDateTime(context, observedAt),
        ),
    ];
  }

  Widget _actions(_DetailsDevice device) {
    return Wrap(
      key: const ValueKey<String>('device-details-actions'),
      alignment: WrapAlignment.center,
      spacing: AppSpacing.sm,
      runSpacing: AppSpacing.sm,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: device.isThisDevice
          ? [
              Button.secondary(
                key: const ValueKey<String>('rename-device'),
                onPressed: controller.actionInFlight
                    ? null
                    : () => onRename(device.name),
                leading: const Icon(LucideIcons.pencil),
                child: const Text('Rename device'),
              ),
            ]
          : [
              Button.secondary(
                onPressed: controller.actionInFlight
                    ? null
                    : () => controller.sync(peerId: device.id),
                leading: const Icon(LucideIcons.refreshCw),
                child: const Text('Sync now'),
              ),
              Button.secondary(
                onPressed: controller.actionInFlight
                    ? null
                    : () => onRemove(device.peer!, false),
                leading: const Icon(LucideIcons.unlink),
                child: const Text('Unpair'),
              ),
              Button.destructive(
                onPressed: controller.actionInFlight
                    ? null
                    : () => onRemove(device.peer!, true),
                leading: const Icon(LucideIcons.ban),
                child: const Text('Revoke pairing'),
              ),
            ],
    );
  }

  static String _trustLabel(DeviceObservationTrust? trust) => switch (trust) {
    DeviceObservationTrust.local => 'Local',
    DeviceObservationTrust.authenticated => 'Authenticated',
    DeviceObservationTrust.unverified => 'Unverified',
    null => 'Unavailable',
  };

  static String _provenanceLabel(DeviceObservationProvenance? provenance) =>
      switch (provenance) {
        DeviceObservationProvenance.selfReported => 'Self-reported',
        DeviceObservationProvenance.observed => 'Observed',
        DeviceObservationProvenance.measured => 'Measured',
        null => 'Unavailable',
      };
}

typedef _DeviceDetailRow = ({String label, String value});

class _DeviceDetailsTable extends StatelessWidget {
  const _DeviceDetailsTable({required this.rows});

  final List<_DeviceDetailRow> rows;

  @override
  Widget build(BuildContext context) {
    return InspectorTable(
      tableKey: const ValueKey<String>('device-details-metadata'),
      rows: [
        for (final row in rows)
          (label: row.label, value: SelectableText(row.value)),
      ],
    );
  }
}

class _DetailsDevice {
  const _DetailsDevice({
    required this.name,
    required this.relationship,
    required this.state,
    required this.latency,
    required this.details,
    required this.id,
    required this.idLabel,
    required this.isThisDevice,
    this.endpoint,
    this.peer,
    this.lastSeen,
    this.latencyObservedAt,
    this.appVersion,
    this.protocolVersion,
  });

  factory _DetailsDevice.thisDevice(ThisDevice device) => _DetailsDevice(
    name: device.name,
    relationship: 'This device',
    state: 'Local',
    latency: 'Local',
    details: device.details,
    id: device.id,
    idLabel: 'Device ID',
    isThisDevice: true,
    endpoint: device.details?.endpoint?.lanEndpoint ?? device.listenAddress,
    appVersion: device.details?.profile?.appVersion ?? device.appVersion,
    protocolVersion:
        device.details?.profile?.protocolVersion ?? device.protocolVersion,
  );

  factory _DetailsDevice.peer(
    DevicePeer peer, {
    required String state,
    required String latency,
    required DateTime? latencyObservedAt,
  }) => _DetailsDevice(
    name: peer.name,
    relationship: 'Trusted',
    state: state,
    latency: latency,
    details: peer.details,
    id: peer.id,
    idLabel: 'Pairing ID',
    isThisDevice: false,
    endpoint: peer.details?.endpoint?.lanEndpoint,
    peer: peer,
    lastSeen: peer.details?.presence?.lastSeen ?? peer.lastSeen,
    latencyObservedAt: latencyObservedAt,
    appVersion: peer.details?.profile?.appVersion,
    protocolVersion: peer.details?.profile?.protocolVersion,
  );

  final String name;
  final String relationship;
  final String state;
  final String latency;
  final DeviceDetails? details;
  final String? id;
  final String idLabel;
  final bool isThisDevice;
  final String? endpoint;
  final DevicePeer? peer;
  final DateTime? lastSeen;
  final DateTime? latencyObservedAt;
  final String? appVersion;
  final int? protocolVersion;
}

class _PairingInspector extends StatefulWidget {
  const _PairingInspector({
    super.key,
    required this.controller,
    required this.inDrawer,
    required this.onClose,
  });

  final DevicesController controller;
  final bool inDrawer;
  final Future<void> Function() onClose;

  @override
  State<_PairingInspector> createState() => _PairingInspectorState();
}

class _PairingInspectorState extends State<_PairingInspector> {
  static const int _pairingCodeLength = 52;

  late final TextEditingController _addressController = TextEditingController(
    text: widget.controller.pendingAddress,
  );
  String _pairingCode = '';
  String? _validationMessage;
  PairingCameraScanner? _scanner;

  @override
  void dispose() {
    _addressController.dispose();
    final scanner = _scanner;
    if (scanner != null) unawaited(scanner.dispose());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final mode = widget.controller.pairingEntryMode;
    final content = Card(
      key: ValueKey<String>(
        widget.inDrawer
            ? 'devices-pairing-drawer-card'
            : 'devices-pairing-inspector',
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(DevicePresentation.pairingEntryIcon(mode)),
              const Gap(AppSpacing.md),
              Expanded(child: Text(_modeTitle(mode)).h3()),
              Button.ghost(
                key: const ValueKey<String>('close-pairing-inspector'),
                style: const ButtonStyle.ghostIcon(),
                onPressed: widget.controller.canClosePairing
                    ? widget.onClose
                    : null,
                child: const Icon(LucideIcons.x),
              ),
            ],
          ),
          const Gap(AppSpacing.lg),
          Expanded(
            child: SingleChildScrollView(
              child: switch (mode) {
                PairingEntryMode.invite => _invitationContent(),
                PairingEntryMode.scanQr => _scannerContent(),
                PairingEntryMode.enterCode => _codeContent(),
                null => const StateView.empty(
                  title: 'Choose a pairing action',
                  message: 'Pairing controls appear here.',
                ),
              },
            ),
          ),
        ],
      ),
    );
    if (!widget.inDrawer) return content;
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.lg,
        AppSpacing.zero,
        AppSpacing.lg,
        AppSpacing.lg,
      ),
      child: content,
    );
  }

  Widget _invitationContent() {
    if (widget.controller.pairingInFlight) {
      return const StateView.loading(message: 'Creating a pairing invitation.');
    }
    final ceremony = widget.controller.pairing;
    if (ceremony == null) return _pairingError();
    return _pairingProgress(ceremony);
  }

  Widget _codeContent() {
    final ceremony = widget.controller.pairing;
    if (ceremony != null) return _pairingProgress(ceremony);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const Text('Pairing code').semiBold(),
        const Gap(AppSpacing.xs),
        const Text(
          'Enter the 52-character code shown on the other device.',
        ).muted(),
        const Gap(AppSpacing.md),
        Semantics(
          label: 'Pairing code',
          textField: true,
          child: SingleChildScrollView(
            key: const ValueKey<String>('pairing-code-scroll'),
            scrollDirection: Axis.horizontal,
            child: Theme(
              data: Theme.of(context).copyWith(
                colorScheme: () => Theme.of(
                  context,
                ).colorScheme.copyWith(border: () => Colors.transparent),
              ),
              child: InputOTP(
                key: const ValueKey<String>('pairing-code-input'),
                onChanged: (value) {
                  setState(() {
                    _pairingCode = value.otpToString();
                    _validationMessage = null;
                  });
                },
                children: _pairingCodeChildren(),
              ),
            ),
          ),
        ),
        const Gap(AppSpacing.xl),
        const Text('Device address').semiBold(),
        const Gap(AppSpacing.xs),
        const Text(
          'Use the host:port address reported by the other device.',
        ).muted(),
        const Gap(AppSpacing.sm),
        TextField(
          key: const ValueKey<String>('pairing-address-input'),
          controller: _addressController,
          placeholder: const Text('192.168.1.25:47654'),
          autocorrect: false,
          enableSuggestions: false,
          textInputAction: TextInputAction.done,
          onSubmitted: (_) => _submitCode(),
        ),
        if (_validationMessage != null) ...[
          const Gap(AppSpacing.md),
          Alert.destructive(
            leading: const Icon(LucideIcons.circleAlert),
            title: const Text('Check pairing details'),
            content: Text(_validationMessage!),
          ),
        ],
        if (widget.controller.errorMessage != null) ...[
          const Gap(AppSpacing.md),
          _pairingError(),
        ],
        const Gap(AppSpacing.xl),
        Align(
          alignment: Alignment.center,
          child: Button.primary(
            key: const ValueKey<String>('submit-pairing-code'),
            onPressed: widget.controller.pairingInFlight ? null : _submitCode,
            leading: widget.controller.pairingInFlight
                ? const SizedBox.square(
                    dimension: AppIconSize.sm,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(LucideIcons.link),
            child: const Text('Join device'),
          ),
        ),
      ],
    );
  }

  Widget _scannerContent() {
    final ceremony = widget.controller.pairing;
    if (ceremony != null) return _pairingProgress(ceremony);
    if (widget.controller.usesSystemScanner) {
      if (widget.controller.errorMessage case final message?) {
        return StateView.error(
          title: 'Pairing action failed',
          message: message,
        );
      }
      return const StateView.loading(message: 'Connecting device…');
    }
    final scanner = _scanner;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (scanner?.preview case final preview?)
          ClipRect(
            child: SizedBox(
              key: const ValueKey<String>('pairing-camera-preview'),
              height: 260,
              child: preview,
            ),
          )
        else
          Card(
            child: SizedBox(
              height: 220,
              child: Center(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Icon(LucideIcons.scanLine, size: AppIconSize.state),
                    const Gap(AppSpacing.md),
                    const Text('QR scanner').h4(),
                    const Gap(AppSpacing.xs),
                    const Text('The camera preview appears here.').muted(),
                  ],
                ),
              ),
            ),
          ),
        if (_validationMessage != null) ...[
          const Gap(AppSpacing.md),
          Alert.destructive(
            leading: const Icon(LucideIcons.circleAlert),
            title: const Text('Scanner needs attention'),
            content: Text(_validationMessage!),
          ),
        ],
        if (widget.controller.errorMessage != null) ...[
          const Gap(AppSpacing.md),
          _pairingError(),
        ],
        const Gap(AppSpacing.lg),
        Align(
          alignment: Alignment.center,
          child: Button.primary(
            key: const ValueKey<String>('start-pairing-scanner'),
            onPressed: _startScanner,
            leading: const Icon(LucideIcons.camera),
            child: const Text('Start scanner'),
          ),
        ),
      ],
    );
  }

  Future<void> _startScanner() async {
    setState(() => _validationMessage = null);
    final scanner = _scanner ??= PairingCameraScanner(
      source: const CameraPluginPairingSource(),
      coordinatorFactory: (onProgress) => QrScanCoordinator(
        decoder: const ZxingQrFrameDecoder(),
        payloadSink: _DevicesQrPayloadSink(controller: widget.controller),
        onProgress: onProgress,
      ),
    );
    await scanner.start();
    if (mounted) setState(() {});
  }

  Future<void> _submitCode() async {
    final address = _addressController.text.trim();
    if (_pairingCode.length != _pairingCodeLength || address.isEmpty) {
      setState(() {
        _validationMessage = _pairingCode.length != _pairingCodeLength
            ? 'Enter the complete 52-character pairing code.'
            : 'Enter the device host:port address.';
      });
      return;
    }
    setState(() => _validationMessage = null);
    await widget.controller.joinFromProtectedInput(
      code: _pairingCode,
      address: address,
    );
  }

  List<InputOTPChild> _pairingCodeChildren() {
    final children = <InputOTPChild>[];
    for (var index = 0; index < _pairingCodeLength; index++) {
      if (index > 0 && index % 4 == 0) children.add(InputOTPChild.separator);
      children.add(
        InputOTPChild.character(
          allowDigit: true,
          allowLowercaseAlphabet: true,
          allowUppercaseAlphabet: true,
          onlyUppercaseAlphabet: true,
          obscured: true,
        ),
      );
    }
    return children;
  }

  Widget _pairingProgress(PairingCeremony ceremony) {
    final terminal = ceremony.state.isTerminal;
    final awaitingConfirmation =
        ceremony.state == PairingState.awaitingConfirmation;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (_pairingTitle(ceremony.state) case final title?) ...[
          Row(
            children: [
              Icon(_pairingIcon(ceremony.state)),
              const Gap(AppSpacing.md),
              Expanded(child: Text(title).h4()),
            ],
          ),
          if (ceremony.failureMessage ??
                  ceremony.peerName ??
                  _pairingDescription(ceremony.state)
              case final description?) ...[
            const Gap(AppSpacing.sm),
            Text(description).muted(),
          ],
        ],
        if (widget.controller.invitation case final invitation?) ...[
          if (ceremony.state != PairingState.waitingForPeer)
            const Gap(AppSpacing.lg),
          Center(
            child: ColoredBox(
              color: Colors.white,
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.md),
                child: Image.memory(
                  invitation.qrPng,
                  key: const ValueKey<String>('pairing-invite-qr'),
                  width: 260,
                  height: 260,
                  filterQuality: FilterQuality.none,
                ),
              ),
            ),
          ),
          const Gap(AppSpacing.lg),
          InspectorTable(
            tableKey: const ValueKey<String>('pairing-invite-details'),
            rows: [
              (
                label: 'Pairing code',
                value: SelectableText(
                  invitation.code,
                  key: const ValueKey<String>('pairing-invite-code'),
                ),
              ),
              (
                label: 'Address',
                value: SelectableText(
                  invitation.address ?? 'Unavailable',
                  key: const ValueKey<String>('pairing-invite-address'),
                ),
              ),
            ],
          ),
        ],
        if (widget.controller.verificationCode
            case final verificationCode?) ...[
          const Gap(AppSpacing.lg),
          Card(
            key: const ValueKey<String>('pairing-verification-code'),
            child: Column(
              children: [
                const Text('Verification code').semiBold(),
                const Gap(AppSpacing.sm),
                Text(
                  verificationCode,
                  textAlign: TextAlign.center,
                  style: Theme.of(
                    context,
                  ).typography.h2.copyWith(letterSpacing: 6),
                ),
              ],
            ),
          ),
        ],
        if (widget.controller.errorMessage != null) ...[
          const Gap(AppSpacing.lg),
          _pairingError(),
        ],
        const Gap(AppSpacing.xl),
        Wrap(
          alignment: WrapAlignment.center,
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          children: [
            if (awaitingConfirmation)
              Button.destructive(
                onPressed:
                    widget.controller.decisionInFlight ||
                        !widget.controller.canConfirmPairing
                    ? null
                    : () => widget.controller.confirmPairing(accept: false),
                child: const Text('Reject'),
              ),
            if (awaitingConfirmation)
              Button.primary(
                onPressed:
                    widget.controller.decisionInFlight ||
                        !widget.controller.canConfirmPairing
                    ? null
                    : () => widget.controller.confirmPairing(accept: true),
                child: const Text('Accept'),
              ),
            if (terminal)
              Button.primary(
                onPressed: widget.onClose,
                child: const Text('Done'),
              ),
          ],
        ),
      ],
    );
  }

  Widget _pairingError() => Alert.destructive(
    leading: const Icon(LucideIcons.circleAlert),
    title: const Text('Pairing action failed'),
    content: Text(
      widget.controller.errorMessage ??
          'The pairing action could not be started.',
    ),
  );

  String _modeTitle(PairingEntryMode? mode) => switch (mode) {
    PairingEntryMode.invite => 'Pair a device',
    PairingEntryMode.scanQr => 'Scan QR code',
    PairingEntryMode.enterCode => 'Enter pairing code',
    null => 'Pairing',
  };

  IconData _pairingIcon(PairingState state) => switch (state) {
    PairingState.waitingForPeer => LucideIcons.qrCode,
    PairingState.handshaking => LucideIcons.loaderCircle,
    PairingState.awaitingConfirmation => LucideIcons.shieldQuestion,
    PairingState.confirmed => LucideIcons.badgeCheck,
    PairingState.rejected || PairingState.failed => LucideIcons.circleX,
    PairingState.cancelled || PairingState.timedOut => LucideIcons.clockAlert,
    PairingState.idle => LucideIcons.link,
  };

  String? _pairingTitle(PairingState state) => switch (state) {
    PairingState.waitingForPeer => null,
    PairingState.handshaking => 'Verifying device',
    PairingState.awaitingConfirmation => 'Confirm on both devices',
    PairingState.confirmed => 'Device paired',
    PairingState.rejected => 'Pairing rejected',
    PairingState.cancelled => 'Pairing cancelled',
    PairingState.timedOut => 'Pairing timed out',
    PairingState.failed => 'Pairing failed',
    PairingState.idle => 'Pairing',
  };

  String? _pairingDescription(PairingState state) => switch (state) {
    PairingState.waitingForPeer => null,
    PairingState.handshaking =>
      'The devices are establishing a secure channel.',
    PairingState.awaitingConfirmation =>
      'Compare the protected verification code, then confirm on both devices.',
    PairingState.confirmed => 'The trusted device is now available for sync.',
    PairingState.rejected ||
    PairingState.cancelled ||
    PairingState.failed => 'No device was added.',
    PairingState.timedOut => 'Start a new pairing attempt to continue.',
    PairingState.idle => 'Start a pairing attempt to connect a device.',
  };
}

class _DevicesQrPayloadSink implements PairingQrPayloadSink {
  const _DevicesQrPayloadSink({required this.controller});

  final DevicesController controller;

  @override
  Future<QrPayloadDisposition> submit(String payload) async {
    if (payload.isEmpty) {
      return QrPayloadDisposition.rejected;
    }
    await controller.joinPairingUri(payload);
    return controller.pairing == null
        ? QrPayloadDisposition.rejected
        : QrPayloadDisposition.accepted;
  }
}
