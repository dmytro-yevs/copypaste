import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/shell/header_action_content.dart';
import '../../app/shell/header_fit.dart';
import '../../app/theme/app_theme.dart';

import '../../app/theme/app_motion.dart';
import '../../app/theme/app_tokens.dart';
import 'device_presentation.dart';
import 'devices_controller.dart';
import 'devices_gateway.dart';

/// Devices commands displayed by the shared application header. They carry
/// labels while the header has room and are the first to give them up.
class DevicesHeaderActions extends StatelessWidget {
  const DevicesHeaderActions({super.key, required this.controller});

  final DevicesController controller;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        if (controller.loadState != DevicesLoadState.ready) {
          return const SizedBox.shrink();
        }
        final l10n = context.l10n;
        final enabled = controller.canChangePairingMode;
        return Row(
          mainAxisSize: MainAxisSize.min,
          spacing: AppSpacing.sm,
          children: [
            _command(
              context: context,
              key: const ValueKey<String>('pair-device'),
              label: l10n.devicesAddDevice,
              icon: LucideIcons.plus,
              primary: true,
              onPressed: enabled ? controller.openInvitation : null,
            ),
            _command(
              context: context,
              key: const ValueKey<String>('scan-pairing-qr'),
              label: l10n.devicesScanQr,
              icon: DevicePresentation.pairingEntryIcon(
                PairingEntryMode.scanQr,
              ),
              onPressed: enabled ? controller.openQrScanner : null,
            ),
            _command(
              context: context,
              key: const ValueKey<String>('enter-pairing-code'),
              label: l10n.devicesEnterCode,
              icon: DevicePresentation.pairingEntryIcon(
                PairingEntryMode.enterCode,
              ),
              onPressed: enabled ? controller.openCodeEntry : null,
            ),
          ],
        );
      },
    );
  }

  /// The commands as labelled buttons for surfaces outside the header.
  static List<Widget> labeledActions(
    BuildContext context,
    DevicesController controller, {
    bool enabled = true,
    String keyPrefix = '',
  }) {
    final l10n = context.l10n;
    final active = enabled && controller.canChangePairingMode;
    return [
      Button.primary(
        key: ValueKey<String>('${keyPrefix}pair-device'),
        onPressed: active ? controller.openInvitation : null,
        leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
        child: Text(l10n.devicesAddDevice),
      ),
      Button.secondary(
        key: ValueKey<String>('${keyPrefix}scan-pairing-qr'),
        onPressed: active ? controller.openQrScanner : null,
        leading: AppIconFrame(
          child: Icon(
            DevicePresentation.pairingEntryIcon(PairingEntryMode.scanQr),
          ),
        ),
        child: Text(l10n.devicesScanQr),
      ),
      Button.secondary(
        key: ValueKey<String>('${keyPrefix}enter-pairing-code'),
        onPressed: active ? controller.openCodeEntry : null,
        leading: AppIconFrame(
          child: Icon(
            DevicePresentation.pairingEntryIcon(PairingEntryMode.enterCode),
          ),
        ),
        child: Text(l10n.devicesEnterCode),
      ),
    ];
  }

  Widget _command({
    required BuildContext context,
    required Key key,
    required String label,
    required IconData icon,
    required VoidCallback? onPressed,
    bool primary = false,
  }) {
    final content = ExcludeSemantics(
      child: HeaderActionContent(
        icon: AppIconFrame(child: Icon(icon)),
        label: label,
        labelUntil: HeaderStage.iconCommands,
      ),
    );
    final button = primary
        ? Button.primary(
            key: key,
            style: AppTheme.headerActionButtonStyle(
              const ButtonStyle.primaryIcon(),
            ),
            onPressed: onPressed,
            child: content,
          )
        : Button.secondary(
            key: key,
            style: AppTheme.headerActionButtonStyle(
              const ButtonStyle.secondaryIcon(),
            ),
            onPressed: onPressed,
            child: content,
          );
    final labelled = Semantics(
      container: true,
      label: label,
      button: true,
      enabled: onPressed != null,
      child: button,
    );
    // A visible label needs no tooltip repeating it. The tooltip stays in
    // every stage, so the button keeps its focus when the label leaves.
    final labelShown = HeaderFit.stageOf(context) == HeaderStage.detailed;
    return Tooltip(
      showDuration: AppMotion.resolve(context, AppMotion.standard),
      tooltip: (context) => labelShown
          ? const SizedBox.shrink()
          : TooltipContainer(child: Text(label)),
      child: labelled,
    );
  }
}
