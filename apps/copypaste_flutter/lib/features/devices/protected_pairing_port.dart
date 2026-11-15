import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'devices_gateway.dart';

/// Opaque renderable material supplied only to the dedicated protected engine.
///
/// Implementations must not expose QR payloads, SAS values, or credentials as
/// Dart strings. The artifact is rendered only by [ProtectedPairingView].
abstract interface class ProtectedPairingArtifact {
  Widget buildProtectedContent(BuildContext context);
}

/// The camera preview is separate from decoded QR data, which stays in Rust.
abstract interface class ProtectedCameraPreview {
  Widget buildProtectedPreview(BuildContext context);
}

/// Identifies the dedicated native host after it has enabled capture protection.
abstract interface class ProtectedPairingHost {
  bool get isActive;
  Future<void> close();
}

/// Session methods only available after entering a [ProtectedPairingHost].
///
/// This is deliberately separate from [DevicesPairingSession], which powers
/// the ordinary Devices page and has no way to receive pairing material.
abstract interface class ProtectedPairingSession {
  PairingCeremony get ceremony;
  Stream<PairingCeremony> get updates;

  /// Supplies the invite QR immediately after the protected host becomes active.
  Future<ProtectedPairingArtifact> revealInvitationQr();
  Future<ProtectedPairingArtifact> revealSas();
  Future<ProtectedCameraPreview> openCameraScanner();

  /// Reads code input from this protected route only.
  Future<void> submitManualJoinCode(String code);

  Future<void> confirm({required bool accept});
  Future<void> cancel();
  Future<void> dispose();
}
