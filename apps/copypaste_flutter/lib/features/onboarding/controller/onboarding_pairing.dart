import 'package:flutter/foundation.dart';

/// Which side of the pairing ceremony this device takes in the tour.
enum OnboardingPairingKind {
  /// This device shows an invitation for another device to join.
  addDevice,

  /// This device joins a space by code or QR.
  join,
}

/// How this device enters a space it joins.
enum OnboardingJoinMethod { code, qr }

/// Lifecycle of the pairing ceremony as the tour sees it.
enum OnboardingPairingPhase { idle, inProgress, paired, failed }

/// The pairing ceremony the tour runs inline. The Devices feature owns the
/// ceremony and its protected surface; the tour only drives and observes it.
abstract interface class OnboardingPairing implements Listenable {
  OnboardingPairingPhase get phase;

  /// How a joining device currently enters the space, if it is joining.
  OnboardingJoinMethod? get joinMethod;

  /// The other device's name once known.
  String? get peerName;

  /// Why the ceremony could not start or stopped, when known.
  String? get errorMessage;

  /// Opens the ceremony; false when it could not start.
  Future<bool> open(OnboardingPairingKind kind);

  /// Switches how a joining device enters the space.
  Future<void> useJoinMethod(OnboardingJoinMethod method);

  /// Cancels an unfinished ceremony and releases its protected surface.
  Future<void> close();
}
