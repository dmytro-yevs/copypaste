import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_theme.dart';

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
          children: showLabels
              ? _labeledActions(context)
              : _compactActions(context),
        );
      },
    );
  }

  List<Widget> _labeledActions(BuildContext context) {
    final l10n = context.l10n;
    return [
      Button.primary(
        key: const ValueKey<String>('pair-device'),
        onPressed: controller.canChangePairingMode
            ? controller.openInvitation
            : null,
        leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
        child: Text(l10n.devicesAddDevice),
      ),
      Button.secondary(
        key: const ValueKey<String>('scan-pairing-qr'),
        onPressed: controller.canChangePairingMode
            ? controller.openQrScanner
            : null,
        leading: AppIconFrame(
          child: Icon(
            DevicePresentation.pairingEntryIcon(PairingEntryMode.scanQr),
          ),
        ),
        child: Text(l10n.devicesScanQr),
      ),
      Button.secondary(
        key: const ValueKey<String>('enter-pairing-code'),
        onPressed: controller.canChangePairingMode
            ? controller.openCodeEntry
            : null,
        leading: AppIconFrame(
          child: Icon(
            DevicePresentation.pairingEntryIcon(PairingEntryMode.enterCode),
          ),
        ),
        child: Text(l10n.devicesEnterCode),
      ),
    ];
  }

  List<Widget> _compactActions(BuildContext context) {
    final l10n = context.l10n;
    return [
      _compactAction(
        context: context,
        key: const ValueKey<String>('pair-device'),
        label: l10n.devicesAddDevice,
        icon: LucideIcons.plus,
        primary: true,
        onPressed: controller.canChangePairingMode
            ? controller.openInvitation
            : null,
      ),
      _compactAction(
        context: context,
        key: const ValueKey<String>('scan-pairing-qr'),
        label: l10n.devicesScanQr,
        icon: DevicePresentation.pairingEntryIcon(PairingEntryMode.scanQr),
        onPressed: controller.canChangePairingMode
            ? controller.openQrScanner
            : null,
      ),
      _compactAction(
        context: context,
        key: const ValueKey<String>('enter-pairing-code'),
        label: l10n.devicesEnterCode,
        icon: DevicePresentation.pairingEntryIcon(PairingEntryMode.enterCode),
        onPressed: controller.canChangePairingMode
            ? controller.openCodeEntry
            : null,
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
            style: AppTheme.controlButtonStyle(const ButtonStyle.primaryIcon()),
            onPressed: onPressed,
            child: AppIconFrame(child: Icon(icon)),
          )
        : Button.secondary(
            key: key,
            style: AppTheme.controlButtonStyle(
              const ButtonStyle.secondaryIcon(),
            ),
            onPressed: onPressed,
            child: AppIconFrame(child: Icon(icon)),
          );
    return Tooltip(
      showDuration: AppMotion.resolve(context, AppMotion.standard),
      tooltip: (context) => TooltipContainer(child: Text(label)),
      child: Semantics(
        container: true,
        label: label,
        button: true,
        enabled: onPressed != null,
        child: button,
      ),
    );
  }
}
