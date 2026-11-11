import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';

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
          DevicesLoadState.loading => StateView.loading(
            message: context.l10n.devicesLoading,
          ),
          DevicesLoadState.error => StateView.error(
            title: context.l10n.devicesUnavailableTitle,
            message: controller.errorMessage,
            actionLabel: context.l10n.commonTryAgain,
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
      controller: PrimaryScrollController.maybeOf(context),
      primary: false,
      padding: padding.add(
        EdgeInsets.only(bottom: MediaQuery.paddingOf(context).bottom),
      ),
      children: [
        if (controller.errorMessage case final errorMessage?) ...[
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
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
              Expanded(
                child: Semantics(
                  container: true,
                  header: true,
                  child: Text(context.l10n.devicesNearbyTitle).large().medium(),
                ),
              ),
              const Gap(AppSpacing.md),
              Button.text(
                key: const ValueKey<String>('rescan-devices'),
                style: const ButtonStyle.text(density: ButtonDensity.compact),
                onPressed:
                    controller.rescanInFlight || controller.actionInFlight
                    ? null
                    : controller.rescan,
                leading: const AppIconFrame(child: Icon(LucideIcons.radar)),
                child: Text(
                  controller.rescanInFlight
                      ? context.l10n.devicesScanning
                      : context.l10n.devicesScan,
                ),
              ),
            ],
          ),
          const Gap(AppSpacing.md),
          if (devices.isEmpty)
            ConstrainedBox(
              constraints: const BoxConstraints(
                minHeight: AppControlSize.touch * 4,
              ),
              child: StateView.empty(
                title: context.l10n.devicesNoneNearby,
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
        AppOverlays.drawerConfiguration(context),
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
        AppOverlays.drawerConfiguration(context),
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
      statusLabel: context.l10n.devicesThisDevice,
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
    final state = controller.peerConnectionState(peer);
    return _deviceCard(
      context,
      key: ValueKey<String>('peer-device-card-${peer.id}'),
      name: peer.name,
      details: peer.details,
      statusLabel: DevicePresentation.connectionLabel(state),
      statusTone: DevicePresentation.connectionTone(state),
      ping: state == DeviceConnectionState.online
          ? controller.peerLatencyLabel(peer)
          : null,
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
            ? () => controller.connectNearby(device)
            : null,
        child: Text(context.l10n.devicesConnect),
      ),
    );
  }

  Widget _deviceCard(
    BuildContext context, {
    required Key key,
    required String name,
    required DeviceDetails? details,
    required String statusLabel,
    AppStatusTone? statusTone,
    String? ping,
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
      label: statusTone == null ? statusLabel : null,
      selected: selected,
      button: true,
      child: Button.card(
        key: key,
        onPressed: onPressed,
        enabled: onPressed != null,
        alignment: Alignment.centerLeft,
        style: style,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final stackedPing =
                constraints.maxWidth <
                AppLayoutSize.deviceCardInlinePingMinWidth *
                    MediaQuery.textScalerOf(context).scale(1);
            return Row(
              children: [
                _deviceIconTile(context, details?.profile?.deviceClass),
                const Gap(AppSpacing.md),
                Expanded(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          if (statusTone != null) ...[
                            Semantics(
                              label: statusLabel,
                              child: AvatarBadge(
                                size: AppIconSize.statusIndicator,
                                borderRadius: AppRadius.full,
                                color: AppTheme.statusColor(
                                  context,
                                  statusTone,
                                ),
                              ),
                            ),
                            const Gap(AppSpacing.xs),
                          ],
                          Expanded(
                            child: Text(
                              name,
                              style: AppTheme.deviceCardTextStyle(context),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ).semiBold(),
                          ),
                        ],
                      ),
                      const Gap(AppSpacing.xs),
                      Text(
                        DevicePresentation.summary(details),
                        style: AppTheme.deviceCardTextStyle(
                          context,
                          muted: true,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
                if (ping != null) ...[
                  const Gap(AppSpacing.md),
                  Text(
                    stackedPing
                        ? context.l10n.devicesCardPingStacked(ping)
                        : context.l10n.devicesCardPing(ping),
                    style: AppTheme.deviceCardTextStyle(context, muted: true),
                    maxLines: 2,
                    textAlign: TextAlign.end,
                  ),
                ],
              ],
            );
          },
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
          child: AppIconFrame(
            size: compact ? AppIconSize.md : AppIconSize.lg,
            child: Icon(
              DevicePresentation.icon(deviceClass),
              size: compact ? AppIconSize.md : AppIconSize.lg,
            ),
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
    final l10n = context.l10n;
    final nameController = TextEditingController(text: currentName);
    try {
      await AppOverlays.showActionDialog(
        context,
        icon: LucideIcons.pencil,
        title: Text(l10n.devicesRenameDialogTitle),
        actionLabel: l10n.commonSave,
        destructive: false,
        changes: nameController,
        canConfirm: () =>
            nameController.text.trim().isNotEmpty && !controller.actionInFlight,
        content: TextField(
          controller: nameController,
          placeholder: Text(l10n.devicesNamePlaceholder),
          autofocus: true,
          decoration: AppOverlays.dialogFieldDecoration(context),
        ),
        onConfirm: () =>
            controller.setThisDeviceName(nameController.text.trim()),
        failureMessage: () =>
            controller.errorMessage ?? l10n.devicesRenameFailed,
      );
    } finally {
      nameController.dispose();
    }
  }

  Future<void> _confirmPeerRemoval(
    BuildContext context,
    DevicesController controller,
    DevicePeer peer, {
    required bool revoke,
  }) async {
    final l10n = context.l10n;
    await AppOverlays.showActionDialog(
      context,
      icon: revoke ? LucideIcons.ban : LucideIcons.unlink,
      title: Text(
        revoke
            ? l10n.devicesRevokeDialogTitle(peer.name)
            : l10n.devicesUnpairDialogTitle(peer.name),
      ),
      content: Text(
        revoke
            ? l10n.devicesRevokeDialogMessage
            : l10n.devicesUnpairDialogMessage,
      ),
      actionLabel: revoke ? l10n.devicesRevoke : l10n.devicesUnpair,
      changes: controller,
      canConfirm: () => !controller.actionInFlight,
      onConfirm: () =>
          revoke ? controller.revoke(peer.id) : controller.unpair(peer.id),
      failureMessage: () =>
          controller.errorMessage ?? l10n.devicesActionFailedMessage,
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
    final device = _selectedDevice(context.l10n, target);
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
              AppIconFrame(
                size: AppIconSize.xl,
                child: Icon(
                  DevicePresentation.icon(device.details?.profile?.deviceClass),
                  size: AppIconSize.xl,
                ),
              ),
              const Gap(AppSpacing.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      device.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ).h3(),
                    Text(
                      DevicePresentation.summary(device.details),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ).muted(),
                  ],
                ),
              ),
              Tooltip(
                showDuration: AppMotion.resolve(context, AppMotion.standard),
                tooltip: (context) => TooltipContainer(
                  child: Text(context.l10n.devicesCloseDetails),
                ),
                child: Semantics(
                  label: context.l10n.devicesCloseDetails,
                  button: true,
                  child: Button.ghost(
                    key: const ValueKey<String>('close-device-details'),
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
          const Gap(AppSpacing.md),
          Expanded(
            child: SingleChildScrollView(
              key: const ValueKey<String>('device-details-scroll-content'),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Align(
                    alignment: Alignment.centerLeft,
                    child: SecondaryBadge(child: Text(device.relationship)),
                  ),
                  if (controller.errorMessage case final errorMessage?) ...[
                    const Gap(AppSpacing.md),
                    Alert.destructive(
                      key: const ValueKey<String>(
                        'device-details-action-error',
                      ),
                      leading: const AppIconFrame(
                        child: Icon(LucideIcons.circleAlert),
                      ),
                      title: Text(controller.errorTitle),
                      content: Text(errorMessage),
                    ),
                  ],
                  const Gap(AppSpacing.xl),
                  _DeviceDetailsTable(rows: _detailRows(context, device)),
                ],
              ),
            ),
          ),
          const Gap(AppSpacing.xxl),
          _actions(context, device),
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

  _DetailsDevice? _selectedDevice(
    AppLocalizations l10n,
    DeviceDetailsTarget? target,
  ) {
    if (target == null) return null;
    if (target.isThisDevice) {
      final device = snapshot.thisDevice;
      return _DetailsDevice.thisDevice(l10n, device);
    }
    for (final peer in snapshot.peers) {
      if (peer.id == target.id) {
        return _DetailsDevice.peer(
          l10n,
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
    final l10n = context.l10n;
    return [
      (
        label: l10n.devicesDetailType,
        value: DevicePresentation.classLabel(
          device.details?.profile?.deviceClass,
        ),
      ),
      (
        label: l10n.devicesDetailOperatingSystem,
        value: DevicePresentation.osLabel(device.details?.profile),
      ),
      if (device.details?.profile?.model case final model?)
        (label: l10n.devicesDetailModel, value: model),
      if (device.appVersion case final appVersion?)
        (label: l10n.devicesDetailAppVersion, value: appVersion),
      if (device.protocolVersion case final protocolVersion?)
        (label: l10n.devicesDetailProtocolVersion, value: '$protocolVersion'),
      if (!device.isThisDevice)
        (label: l10n.devicesDetailStatus, value: device.state),
      if (!device.isThisDevice)
        (label: l10n.devicesDetailPing, value: device.latency),
      if (device.latencyObservedAt case final measuredAt?)
        (
          label: l10n.devicesDetailMeasured,
          value: formatSystemDateTime(context, measuredAt),
        ),
      if (device.lastSeen case final lastSeen?)
        (
          label: l10n.devicesDetailLastSeen,
          value: formatSystemDateTime(context, lastSeen),
        ),
      if (device.endpoint case final endpoint?)
        (label: l10n.devicesDetailLanEndpoint, value: endpoint),
      (label: device.idLabel, value: device.id ?? l10n.devicesValueUnavailable),
      (
        label: l10n.devicesDetailProfileProvenance,
        value: _provenanceLabel(l10n, device.details?.profile?.provenance),
      ),
      if (!device.isThisDevice)
        (
          label: l10n.devicesDetailProfileTrust,
          value: _trustLabel(l10n, device.details?.profile?.trust),
        ),
      if (device.details?.profile?.observedAt case final observedAt?)
        (
          label: l10n.devicesDetailProfileUpdated,
          value: formatSystemDateTime(context, observedAt),
        ),
    ];
  }

  Widget _actions(BuildContext context, _DetailsDevice device) {
    final l10n = context.l10n;
    return Wrap(
      key: const ValueKey<String>('device-details-actions'),
      alignment: WrapAlignment.center,
      spacing: AppSpacing.sm,
      runSpacing: AppSpacing.sm,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: device.isThisDevice
          ? [
              Button.secondary(
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                key: const ValueKey<String>('rename-device'),
                onPressed: controller.actionInFlight
                    ? null
                    : () => onRename(device.name),
                leading: const AppIconFrame(child: Icon(LucideIcons.pencil)),
                child: Text(l10n.devicesRenameDevice),
              ),
            ]
          : [
              Button.secondary(
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: controller.actionInFlight
                    ? null
                    : () => controller.sync(peerId: device.id),
                leading: const AppIconFrame(child: Icon(LucideIcons.refreshCw)),
                child: Text(l10n.devicesSyncNow),
              ),
              Button.secondary(
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: controller.actionInFlight
                    ? null
                    : () => onRemove(device.peer!, false),
                leading: const AppIconFrame(child: Icon(LucideIcons.unlink)),
                child: Text(l10n.devicesUnpair),
              ),
              Button.destructive(
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.destructive(),
                ),
                onPressed: controller.actionInFlight
                    ? null
                    : () => onRemove(device.peer!, true),
                leading: const AppIconFrame(child: Icon(LucideIcons.ban)),
                child: Text(l10n.devicesRevokePairing),
              ),
            ],
    );
  }

  static String _trustLabel(
    AppLocalizations l10n,
    DeviceObservationTrust? trust,
  ) => switch (trust) {
    DeviceObservationTrust.local => l10n.devicesLocal,
    DeviceObservationTrust.authenticated => l10n.devicesTrustAuthenticated,
    DeviceObservationTrust.unverified => l10n.devicesTrustUnverified,
    null => l10n.devicesValueUnavailable,
  };

  static String _provenanceLabel(
    AppLocalizations l10n,
    DeviceObservationProvenance? provenance,
  ) => switch (provenance) {
    DeviceObservationProvenance.selfReported =>
      l10n.devicesProvenanceSelfReported,
    DeviceObservationProvenance.observed => l10n.devicesProvenanceObserved,
    DeviceObservationProvenance.measured => l10n.devicesProvenanceMeasured,
    null => l10n.devicesValueUnavailable,
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

  factory _DetailsDevice.thisDevice(AppLocalizations l10n, ThisDevice device) =>
      _DetailsDevice(
        name: device.name,
        relationship: l10n.devicesThisDevice,
        state: l10n.devicesLocal,
        latency: l10n.devicesLocal,
        details: device.details,
        id: device.id,
        idLabel: l10n.devicesDetailDeviceId,
        isThisDevice: true,
        endpoint: device.details?.endpoint?.lanEndpoint ?? device.listenAddress,
        appVersion: device.details?.profile?.appVersion ?? device.appVersion,
        protocolVersion:
            device.details?.profile?.protocolVersion ?? device.protocolVersion,
      );

  factory _DetailsDevice.peer(
    AppLocalizations l10n,
    DevicePeer peer, {
    required String state,
    required String latency,
    required DateTime? latencyObservedAt,
  }) => _DetailsDevice(
    name: peer.name,
    relationship: l10n.devicesTrusted,
    state: state,
    latency: latency,
    details: peer.details,
    id: peer.id,
    idLabel: l10n.devicesDetailPairingId,
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
  static const int _pairingCodeLength = 8;

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
              AppIconFrame(
                child: Icon(DevicePresentation.pairingEntryIcon(mode)),
              ),
              const Gap(AppSpacing.md),
              Expanded(
                child: Text(
                  _modeTitle(mode),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ).h3(),
              ),
              Button.ghost(
                key: const ValueKey<String>('close-pairing-inspector'),
                style: AppTheme.controlButtonStyle(
                  const ButtonStyle.ghostIcon(),
                ),
                onPressed: widget.controller.canClosePairing
                    ? widget.onClose
                    : null,
                child: const AppIconFrame(child: Icon(LucideIcons.x)),
              ),
            ],
          ),
          const Gap(AppSpacing.lg),
          Expanded(
            child: SingleChildScrollView(
              key: const ValueKey<String>('pairing-scroll-content'),
              child: switch (mode) {
                PairingEntryMode.invite => _invitationContent(),
                PairingEntryMode.nearby =>
                  widget.controller.pairing != null
                      ? _pairingProgress(widget.controller.pairing!)
                      : widget.controller.pairingInFlight
                      ? StateView.loading(
                          message: context.l10n.pairingSendingRequest,
                        )
                      : _pairingError(),
                PairingEntryMode.scanQr => _scannerContent(),
                PairingEntryMode.enterCode => _codeContent(),
                null => StateView.empty(
                  title: context.l10n.pairingEmptyTitle,
                  message: context.l10n.pairingEmptyMessage,
                ),
              },
            ),
          ),
          if (_pairingActions() case final actions?) ...[
            const Gap(AppSpacing.xl),
            actions,
          ],
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
      return StateView.loading(message: context.l10n.pairingCreatingInvitation);
    }
    final ceremony = widget.controller.pairing;
    if (ceremony == null) return _pairingError();
    return _pairingProgress(ceremony);
  }

  Widget _codeContent() {
    final ceremony = widget.controller.pairing;
    if (ceremony != null) return _pairingProgress(ceremony);
    final l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l10n.pairingCode).semiBold(),
        const Gap(AppSpacing.xs),
        Text(l10n.pairingCodeHint).muted(),
        const Gap(AppSpacing.md),
        Semantics(
          label: l10n.pairingCode,
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
        Text(l10n.pairingDeviceAddress).semiBold(),
        const Gap(AppSpacing.xs),
        Text(l10n.pairingDeviceAddressHint).muted(),
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
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(l10n.pairingCheckDetailsTitle),
            content: Text(_validationMessage!),
          ),
        ],
        if (widget.controller.errorMessage != null) ...[
          const Gap(AppSpacing.md),
          _pairingError(),
        ],
      ],
    );
  }

  Widget _scannerContent() {
    final ceremony = widget.controller.pairing;
    if (ceremony != null) return _pairingProgress(ceremony);
    if (widget.controller.usesSystemScanner) {
      if (widget.controller.errorMessage case final message?) {
        return StateView.error(
          title: context.l10n.pairingActionFailed,
          message: message,
        );
      }
      return StateView.loading(message: context.l10n.pairingConnectingDevice);
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
                    const AppIconFrame(
                      size: AppIconSize.state,
                      child: Icon(
                        LucideIcons.scanLine,
                        size: AppIconSize.state,
                      ),
                    ),
                    const Gap(AppSpacing.md),
                    Text(context.l10n.pairingQrScannerTitle).h4(),
                    const Gap(AppSpacing.xs),
                    Text(context.l10n.pairingCameraPreviewPlaceholder).muted(),
                  ],
                ),
              ),
            ),
          ),
        if (_validationMessage != null) ...[
          const Gap(AppSpacing.md),
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(context.l10n.pairingScannerNeedsAttention),
            content: Text(_validationMessage!),
          ),
        ],
        if (widget.controller.errorMessage != null) ...[
          const Gap(AppSpacing.md),
          _pairingError(),
        ],
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
            ? context.l10n.pairingCodeIncomplete
            : context.l10n.pairingAddressMissing;
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
    final l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (ceremony.state != PairingState.waitingForPeer) ...[
          Row(
            children: [
              AppIconFrame(
                child: Icon(
                  DevicePresentation.pairingStateIcon(ceremony.state),
                ),
              ),
              const Gap(AppSpacing.md),
              Expanded(
                child: Text(
                  widget.controller.locallyConfirmed &&
                          ceremony.state == PairingState.awaitingConfirmation
                      ? l10n.pairingWaitingForOtherDevice
                      : DevicePresentation.pairingStateTitle(ceremony.state),
                ).h4(),
              ),
            ],
          ),
          const Gap(AppSpacing.sm),
          Text(
            ceremony.failureMessage ??
                ceremony.peerName ??
                DevicePresentation.pairingStateDescription(ceremony.state),
          ).muted(),
        ],
        if (widget.controller.invitation case final invitation?) ...[
          if (ceremony.state != PairingState.waitingForPeer)
            const Gap(AppSpacing.lg),
          Text(l10n.pairingInviteInstructions).muted(),
          const Gap(AppSpacing.lg),
          const Divider(key: ValueKey('pairing-invite-divider')),
          const Gap(AppSpacing.lg),
          AppBar(
            title: Text(l10n.pairingDetailsTitle),
            leading: const [AppIconFrame(child: Icon(LucideIcons.info))],
          ),
          const Gap(AppSpacing.md),
          InspectorTable(
            tableKey: const ValueKey<String>('pairing-invite-details'),
            rows: [
              (
                label: l10n.pairingCode,
                value: SelectableText(
                  invitation.code,
                  key: const ValueKey<String>('pairing-invite-code'),
                ),
              ),
              (
                label: l10n.pairingAddress,
                value: SelectableText(
                  invitation.address ?? l10n.devicesValueUnavailable,
                  key: const ValueKey<String>('pairing-invite-address'),
                ),
              ),
            ],
          ),
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
        ],
        if (widget.controller.verificationCode
            case final verificationCode?) ...[
          const Gap(AppSpacing.lg),
          Card(
            key: const ValueKey<String>('pairing-verification-code'),
            child: Column(
              children: [
                Text(l10n.pairingVerificationCode).semiBold(),
                const Gap(AppSpacing.sm),
                Text(l10n.pairingVerificationCodeHint).muted(),
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
      ],
    );
  }

  Widget? _pairingActions() {
    final l10n = context.l10n;
    final ceremony = widget.controller.pairing;
    if (ceremony != null) {
      final terminal = ceremony.state.isTerminal;
      final awaitingConfirmation =
          ceremony.state == PairingState.awaitingConfirmation;
      if (!terminal && !awaitingConfirmation) return null;
      return Wrap(
        key: const ValueKey<String>('pairing-actions'),
        alignment: WrapAlignment.center,
        spacing: AppSpacing.sm,
        runSpacing: AppSpacing.sm,
        children: [
          if (awaitingConfirmation)
            Button.destructive(
              style: AppTheme.actionButtonStyle(
                const ButtonStyle.destructive(),
              ),
              onPressed:
                  widget.controller.decisionInFlight ||
                      !widget.controller.canConfirmPairing
                  ? null
                  : () => widget.controller.confirmPairing(accept: false),
              child: Text(l10n.pairingReject),
            ),
          if (awaitingConfirmation)
            Button.primary(
              style: AppTheme.actionButtonStyle(const ButtonStyle.primary()),
              onPressed:
                  widget.controller.decisionInFlight ||
                      !widget.controller.canConfirmPairing
                  ? null
                  : () => widget.controller.confirmPairing(accept: true),
              child: Text(l10n.pairingConfirm),
            ),
          if (terminal)
            Button.primary(
              style: AppTheme.actionButtonStyle(const ButtonStyle.primary()),
              onPressed: widget.onClose,
              child: Text(l10n.commonDone),
            ),
        ],
      );
    }
    switch (widget.controller.pairingEntryMode) {
      case PairingEntryMode.enterCode:
        return Align(
          alignment: Alignment.center,
          child: Button.primary(
            style: AppTheme.actionButtonStyle(const ButtonStyle.primary()),
            key: const ValueKey<String>('submit-pairing-code'),
            onPressed: widget.controller.pairingInFlight ? null : _submitCode,
            leading: widget.controller.pairingInFlight
                ? const SizedBox.square(
                    dimension: AppIconSize.sm,
                    child: AppIconFrame(
                      size: AppIconSize.sm,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  )
                : const AppIconFrame(child: Icon(LucideIcons.link)),
            child: Text(l10n.pairingJoinDevice),
          ),
        );
      case PairingEntryMode.scanQr:
        if (widget.controller.usesSystemScanner) return null;
        return Align(
          alignment: Alignment.center,
          child: Button.primary(
            style: AppTheme.actionButtonStyle(const ButtonStyle.primary()),
            key: const ValueKey<String>('start-pairing-scanner'),
            onPressed: _startScanner,
            leading: const AppIconFrame(child: Icon(LucideIcons.camera)),
            child: Text(l10n.pairingStartScanner),
          ),
        );
      case PairingEntryMode.invite:
      case PairingEntryMode.nearby:
      case null:
        return null;
    }
  }

  Widget _pairingError() => Alert.destructive(
    leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
    title: Text(context.l10n.pairingActionFailed),
    content: Text(
      widget.controller.errorMessage ?? context.l10n.pairingActionNotStarted,
    ),
  );

  String _modeTitle(PairingEntryMode? mode) {
    final l10n = context.l10n;
    return switch (mode) {
      PairingEntryMode.invite => l10n.pairingPairDevice,
      PairingEntryMode.scanQr => l10n.pairingScanQrCode,
      PairingEntryMode.enterCode => l10n.pairingEnterPairingCode,
      PairingEntryMode.nearby => l10n.pairingConnectNearbyDevice,
      null => l10n.pairingTitle,
    };
  }
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
