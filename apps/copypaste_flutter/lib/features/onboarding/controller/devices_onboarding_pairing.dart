import 'package:flutter/foundation.dart';

import '../../devices/devices_controller.dart';
import '../../devices/devices_gateway.dart';
import 'onboarding_pairing.dart';

/// Runs the tour's pairing on the Devices controller, which applies the same
/// capture protection and ceremony rules as the Devices screen.
class DevicesOnboardingPairing implements OnboardingPairing {
  DevicesOnboardingPairing(this.devices);

  final DevicesController devices;

  /// Hides the closed gap while a dismissed system scanner falls back to
  /// code entry, so the tour does not read it as a cancelled ceremony.
  bool _switching = false;

  @override
  OnboardingPairingPhase get phase {
    if (_switching) return OnboardingPairingPhase.inProgress;
    if (devices.pairingEntryMode == null) return OnboardingPairingPhase.idle;
    final state = devices.pairing?.state;
    if (state == PairingState.confirmed) return OnboardingPairingPhase.paired;
    if (state != null && state.isTerminal) {
      return OnboardingPairingPhase.failed;
    }
    return OnboardingPairingPhase.inProgress;
  }

  @override
  OnboardingJoinMethod? get joinMethod => switch (devices.pairingEntryMode) {
    PairingEntryMode.enterCode => OnboardingJoinMethod.code,
    PairingEntryMode.scanQr => OnboardingJoinMethod.qr,
    PairingEntryMode.invite || PairingEntryMode.nearby || null => null,
  };

  @override
  String? get peerName => devices.pairing?.peerName;

  @override
  String? get errorMessage => devices.errorMessage;

  @override
  Future<bool> open(OnboardingPairingKind kind) async {
    switch (kind) {
      case OnboardingPairingKind.addDevice:
        await devices.openInvitation();
      case OnboardingPairingKind.join:
        await devices.openCodeEntry();
    }
    return devices.pairingEntryMode != null;
  }

  @override
  Future<void> useJoinMethod(OnboardingJoinMethod method) async {
    switch (method) {
      case OnboardingJoinMethod.code:
        await devices.openCodeEntry();
      case OnboardingJoinMethod.qr:
        _switching = true;
        try {
          await devices.openQrScanner();
          // A dismissed system scanner closes pairing without an error.
          if (devices.pairingEntryMode == null &&
              devices.errorMessage == null) {
            await devices.openCodeEntry();
          }
        } finally {
          _switching = false;
        }
    }
  }

  @override
  Future<void> close() async {
    if (devices.pairingEntryMode != null && devices.canClosePairing) {
      await devices.closePairing();
    }
  }

  @override
  void addListener(VoidCallback listener) => devices.addListener(listener);

  @override
  void removeListener(VoidCallback listener) =>
      devices.removeListener(listener);
}
