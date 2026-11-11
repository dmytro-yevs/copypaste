import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_tokens.dart';
import '../../shared/state_view.dart';
import 'device_presentation.dart';
import 'devices_gateway.dart';
import 'protected_pairing_controller.dart';

/// Full pairing flow for the dedicated capture-protected Flutter engine route.
///
/// This view is intentionally not exported from the ordinary Devices feature
/// barrel. Integration may instantiate it only after the native dedicated host
/// is active for the route context supplied by the pairing adapter.
class ProtectedPairingView extends StatefulWidget {
  const ProtectedPairingView({
    super.key,
    required this.controller,
    required this.onClosed,
  });

  final ProtectedPairingController controller;
  final VoidCallback onClosed;

  @override
  State<ProtectedPairingView> createState() => _ProtectedPairingViewState();
}

class _ProtectedPairingViewState extends State<ProtectedPairingView> {
  final TextEditingController _manualCodeController = TextEditingController();

  @override
  void initState() {
    super.initState();
    widget.controller.start();
  }

  @override
  void dispose() {
    _manualCodeController.dispose();
    widget.controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final controller = widget.controller;
        if (!controller.isProtectedHostActive) {
          return StateView.error(
            title: context.l10n.pairingProtectedUnavailableTitle,
            message: context.l10n.pairingProtectedUnavailableMessage,
            actionLabel: context.l10n.commonClose,
            onAction: _close,
          );
        }
        return CallbackShortcuts(
          bindings: <ShortcutActivator, VoidCallback>{
            const SingleActivator(LogicalKeyboardKey.escape): () {
              if (!controller.decisionInFlight) _close();
            },
          },
          child: Center(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(AppSpacing.xxl),
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 520),
                child: Card(child: _content(context, controller)),
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _content(BuildContext context, ProtectedPairingController controller) {
    final ceremony = controller.ceremony;
    final terminal = ceremony.state.isTerminal;
    final awaitingConfirmation =
        ceremony.state == PairingState.awaitingConfirmation;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            AppIconFrame(
              child: Icon(DevicePresentation.pairingStateIcon(ceremony.state)),
            ),
            const Gap(AppSpacing.md),
            Expanded(
              child: Text(
                DevicePresentation.pairingStateTitle(ceremony.state),
              ).h2(),
            ),
            if (ceremony.expiresIn != null && !terminal)
              SecondaryBadge(child: Text(_expiryLabel(ceremony.expiresIn!))),
          ],
        ),
        const Gap(AppSpacing.sm),
        Text(
          ceremony.peerName ??
              DevicePresentation.pairingStateDescription(ceremony.state),
        ).muted(),
        if (controller.errorMessage != null) ...[
          const Gap(AppSpacing.lg),
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(context.l10n.pairingActionFailed),
            content: Text(controller.errorMessage!),
          ),
        ],
        if (controller.artifact != null) ...[
          const Gap(AppSpacing.lg),
          controller.artifact!.buildProtectedContent(context),
        ],
        if (controller.cameraPreview != null) ...[
          const Gap(AppSpacing.lg),
          controller.cameraPreview!.buildProtectedPreview(context),
        ],
        const Gap(AppSpacing.xl),
        if (!terminal && ceremony.state == PairingState.waitingForPeer)
          _joinAndInviteActions(context, controller),
        if (!terminal && ceremony.state == PairingState.handshaking)
          const Center(
            child: AppIconFrame(
              size: AppIconSize.sm,
              child: CircularProgressIndicator(),
            ),
          ),
        if (awaitingConfirmation) _confirmationActions(controller),
        if (terminal)
          Button.primary(
            onPressed: _close,
            child: Text(context.l10n.commonDone),
          )
        else if (!awaitingConfirmation)
          Align(
            alignment: Alignment.centerRight,
            child: Button.ghost(
              onPressed: controller.decisionInFlight ? null : _close,
              child: Text(context.l10n.pairingCancel),
            ),
          ),
      ],
    );
  }

  Widget _joinAndInviteActions(
    BuildContext context,
    ProtectedPairingController controller,
  ) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          children: [
            Button.secondary(
              onPressed: controller.openCameraScanner,
              leading: const AppIconFrame(child: Icon(LucideIcons.scanLine)),
              child: Text(context.l10n.pairingScanQrCode),
            ),
          ],
        ),
        const Gap(AppSpacing.xl),
        Text(context.l10n.pairingOrEnterCode).semiBold(),
        const Gap(AppSpacing.sm),
        TextField(
          controller: _manualCodeController,
          placeholder: Text(context.l10n.pairingCode),
          obscureText: true,
          autocorrect: false,
          enableSuggestions: false,
          textInputAction: TextInputAction.done,
          onSubmitted: controller.submitManualJoinCode,
        ),
        const Gap(AppSpacing.sm),
        Align(
          alignment: Alignment.centerRight,
          child: Button.secondary(
            onPressed: () =>
                controller.submitManualJoinCode(_manualCodeController.text),
            leading: const AppIconFrame(child: Icon(LucideIcons.arrowRight)),
            child: Text(context.l10n.pairingJoinSecurely),
          ),
        ),
      ],
    );
  }

  Widget _confirmationActions(ProtectedPairingController controller) {
    return Wrap(
      alignment: WrapAlignment.end,
      spacing: AppSpacing.sm,
      runSpacing: AppSpacing.sm,
      children: [
        Button.destructive(
          onPressed: controller.decisionInFlight || !controller.canConfirm
              ? null
              : () => controller.confirm(accept: false),
          child: Text(context.l10n.pairingReject),
        ),
        Button.primary(
          onPressed: controller.decisionInFlight || !controller.canConfirm
              ? null
              : () => controller.confirm(accept: true),
          child: Text(context.l10n.pairingAccept),
        ),
      ],
    );
  }

  Future<void> _close() async {
    await widget.controller.close();
    if (mounted) widget.onClosed();
  }

  String _expiryLabel(Duration remaining) =>
      context.l10n.pairingExpiresInSeconds(remaining.inSeconds.clamp(0, 9999));
}
