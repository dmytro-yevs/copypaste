import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_motion.dart';
import '../../app/theme/app_tokens.dart';
import 'device_presentation.dart';
import 'devices_controller.dart';
import 'devices_gateway.dart';

/// Devices commands displayed by the shared application header.
class DevicesHeaderActions extends StatelessWidget {
  const DevicesHeaderActions({super.key, required this.controller});

  static const double _expandedActionsBreakpoint = 1200;

  final DevicesController controller;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        if (controller.loadState != DevicesLoadState.ready) {
          return const SizedBox.shrink();
        }

        final showLabels =
            MediaQuery.sizeOf(context).width >= _expandedActionsBreakpoint &&
            MediaQuery.textScalerOf(context).scale(1) <= 1.3;
        return Row(
          mainAxisSize: MainAxisSize.min,
          spacing: AppSpacing.sm,
          children: showLabels ? _labeledActions() : _compactActions(context),
        );
      },
    );
  }

  List<Widget> _labeledActions() {
    return [
      Button.secondary(
        key: const ValueKey<String>('rescan-devices'),
        onPressed: controller.rescanInFlight ? null : controller.rescan,
        leading: controller.rescanInFlight
            ? const SizedBox.square(
                dimension: AppIconSize.sm,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : const Icon(LucideIcons.scanSearch),
        child: Text(controller.rescanInFlight ? 'Scanning…' : 'Rescan'),
      ),
      Button.primary(
        key: const ValueKey<String>('pair-device'),
        onPressed: controller.pairingInspectorOpen
            ? null
            : controller.openInvitation,
        leading: Icon(
          DevicePresentation.pairingEntryIcon(PairingEntryMode.invite),
        ),
        child: const Text('Pair device'),
      ),
      Button.secondary(
        key: const ValueKey<String>('scan-pairing-qr'),
        onPressed: controller.pairingInspectorOpen
            ? null
            : controller.openQrScanner,
        leading: Icon(
          DevicePresentation.pairingEntryIcon(PairingEntryMode.scanQr),
        ),
        child: const Text('Scan QR'),
      ),
      Button.secondary(
        key: const ValueKey<String>('enter-pairing-code'),
        onPressed: controller.pairingInspectorOpen
            ? null
            : controller.openCodeEntry,
        leading: Icon(
          DevicePresentation.pairingEntryIcon(PairingEntryMode.enterCode),
        ),
        child: const Text('Enter code'),
      ),
    ];
  }

  List<Widget> _compactActions(BuildContext context) {
    return [
      _compactAction(
        context: context,
        key: const ValueKey<String>('rescan-devices'),
        label: 'Rescan',
        icon: LucideIcons.scanSearch,
        onPressed: controller.rescanInFlight ? null : controller.rescan,
      ),
      _compactAction(
        context: context,
        key: const ValueKey<String>('pair-device'),
        label: 'Pair device',
        icon: DevicePresentation.pairingEntryIcon(PairingEntryMode.invite),
        primary: true,
        onPressed: controller.pairingInspectorOpen
            ? null
            : controller.openInvitation,
      ),
      _compactAction(
        context: context,
        key: const ValueKey<String>('scan-pairing-qr'),
        label: 'Scan QR',
        icon: DevicePresentation.pairingEntryIcon(PairingEntryMode.scanQr),
        onPressed: controller.pairingInspectorOpen
            ? null
            : controller.openQrScanner,
      ),
      _compactAction(
        context: context,
        key: const ValueKey<String>('enter-pairing-code'),
        label: 'Enter code',
        icon: DevicePresentation.pairingEntryIcon(PairingEntryMode.enterCode),
        onPressed: controller.pairingInspectorOpen
            ? null
            : controller.openCodeEntry,
      ),
    ];
  }

  Widget _compactAction({
    required BuildContext context,
    required Key key,
    required String label,
    required IconData icon,
    required VoidCallback? onPressed,
    bool primary = false,
  }) {
    final button = primary
        ? Button.primary(
            key: key,
            style: const ButtonStyle.primaryIcon(),
            onPressed: onPressed,
            child: Icon(icon),
          )
        : Button.secondary(
            key: key,
            style: const ButtonStyle.secondaryIcon(),
            onPressed: onPressed,
            child: Icon(icon),
          );
    return Tooltip(
      showDuration: AppMotion.resolve(context, AppMotion.standard),
      tooltip: (context) => TooltipContainer(child: Text(label)),
      child: button,
    );
  }
}
