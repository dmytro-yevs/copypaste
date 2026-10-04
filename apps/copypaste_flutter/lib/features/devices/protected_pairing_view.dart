import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_tokens.dart';
import '../../shared/state_view.dart';
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
            title: 'Protected pairing is unavailable',
            message:
                'This pairing session must open in a dedicated protected window.',
            actionLabel: 'Close',
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
            Icon(_iconFor(ceremony.state)),
            const Gap(AppSpacing.md),
            Expanded(child: Text(_titleFor(ceremony.state)).h2()),
            if (ceremony.expiresIn != null && !terminal)
              SecondaryBadge(child: Text(_expiryLabel(ceremony.expiresIn!))),
          ],
        ),
        const Gap(AppSpacing.sm),
        Text(ceremony.peerName ?? _descriptionFor(ceremony.state)).muted(),
        if (controller.errorMessage != null) ...[
          const Gap(AppSpacing.lg),
          Alert.destructive(
            leading: const Icon(LucideIcons.circleAlert),
            title: const Text('Pairing action failed'),
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
          const Center(child: CircularProgressIndicator()),
        if (awaitingConfirmation) _confirmationActions(controller),
        if (terminal)
          Button.primary(onPressed: _close, child: const Text('Done'))
        else if (!awaitingConfirmation)
          Align(
            alignment: Alignment.centerRight,
            child: Button.ghost(
              onPressed: controller.decisionInFlight ? null : _close,
              child: const Text('Cancel pairing'),
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
              leading: const Icon(LucideIcons.scanLine),
              child: const Text('Scan QR code'),
            ),
          ],
        ),
        const Gap(AppSpacing.xl),
        const Text('Or enter a pairing code').semiBold(),
        const Gap(AppSpacing.sm),
        TextField(
          controller: _manualCodeController,
          placeholder: const Text('Pairing code'),
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
            leading: const Icon(LucideIcons.arrowRight),
            child: const Text('Join securely'),
          ),
        ),
      ],
    );
  }

  Widget _confirmationActions(ProtectedPairingController controller) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Button.secondary(
          onPressed: controller.revealSas,
          leading: const Icon(LucideIcons.shieldCheck),
          child: const Text('Reveal verification code'),
        ),
        const Gap(AppSpacing.md),
        Wrap(
          alignment: WrapAlignment.end,
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          children: [
            Button.destructive(
              onPressed: controller.decisionInFlight
                  ? null
                  : () => controller.confirm(accept: false),
              child: const Text('Reject'),
            ),
            Button.primary(
              onPressed: controller.decisionInFlight
                  ? null
                  : () => controller.confirm(accept: true),
              child: const Text('Confirm both devices'),
            ),
          ],
        ),
      ],
    );
  }

  Future<void> _close() async {
    await widget.controller.close();
    if (mounted) widget.onClosed();
  }

  IconData _iconFor(PairingState state) => switch (state) {
    PairingState.waitingForPeer => LucideIcons.qrCode,
    PairingState.handshaking => LucideIcons.loaderCircle,
    PairingState.awaitingConfirmation => LucideIcons.shieldQuestion,
    PairingState.confirmed => LucideIcons.badgeCheck,
    PairingState.rejected || PairingState.failed => LucideIcons.circleX,
    PairingState.cancelled || PairingState.timedOut => LucideIcons.clockAlert,
    PairingState.idle => LucideIcons.link,
  };

  String _titleFor(PairingState state) => switch (state) {
    PairingState.waitingForPeer => 'Pair a device',
    PairingState.handshaking => 'Verifying device',
    PairingState.awaitingConfirmation => 'Confirm on both devices',
    PairingState.confirmed => 'Device paired',
    PairingState.rejected => 'Pairing rejected',
    PairingState.cancelled => 'Pairing cancelled',
    PairingState.timedOut => 'Pairing timed out',
    PairingState.failed => 'Pairing failed',
    PairingState.idle => 'Pairing',
  };

  String _descriptionFor(PairingState state) => switch (state) {
    PairingState.waitingForPeer =>
      'Reveal a QR code, scan one, or enter a pairing code in this protected window.',
    PairingState.handshaking =>
      'The devices are establishing a secure channel.',
    PairingState.awaitingConfirmation =>
      'Reveal the verification code and compare it on both devices.',
    PairingState.confirmed => 'The device is now trusted.',
    PairingState.rejected ||
    PairingState.cancelled ||
    PairingState.failed => 'No device was added.',
    PairingState.timedOut => 'Start a new pairing attempt to continue.',
    PairingState.idle => 'Start a new pairing attempt.',
  };

  String _expiryLabel(Duration remaining) =>
      'Expires in ${remaining.inSeconds.clamp(0, 9999)}s';
}
