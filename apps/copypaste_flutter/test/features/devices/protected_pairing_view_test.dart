import 'dart:async';

import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/features/devices/protected_pairing_controller.dart';
import 'package:copypaste_flutter/features/devices/protected_pairing_port.dart';
import 'package:copypaste_flutter/features/devices/protected_pairing_view.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  testWidgets('renders protected invitation material immediately', (
    tester,
  ) async {
    final session = _ViewSession();
    final controller = ProtectedPairingController(
      host: const _ViewHost(active: true),
      session: session,
    );
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        home: ProtectedPairingView(controller: controller, onClosed: () {}),
      ),
    );

    await tester.pump();

    expect(session.revealCalls, 1);
    expect(find.text('Protected test artifact'), findsOneWidget);
  });

  testWidgets('refuses to present pairing content outside a protected host', (
    tester,
  ) async {
    final controller = ProtectedPairingController(
      host: const _ViewHost(active: false),
      session: _ViewSession(),
    );
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        home: ProtectedPairingView(controller: controller, onClosed: () {}),
      ),
    );

    expect(find.text('Protected pairing is unavailable'), findsOneWidget);
    expect(find.text('Scan QR code'), findsNothing);
  });
}

class _ViewHost implements ProtectedPairingHost {
  const _ViewHost({required this.active});

  final bool active;

  @override
  bool get isActive => active;

  @override
  Future<void> close() async {}
}

class _ViewSession implements ProtectedPairingSession {
  @override
  PairingCeremony get ceremony =>
      const PairingCeremony(state: PairingState.waitingForPeer);

  int revealCalls = 0;

  @override
  Stream<PairingCeremony> get updates => const Stream<PairingCeremony>.empty();

  @override
  Future<void> cancel() async {}

  @override
  Future<void> confirm({required bool accept}) async {}

  @override
  Future<void> dispose() async {}

  @override
  Future<ProtectedCameraPreview> openCameraScanner() async =>
      const _ViewPreview();

  @override
  Future<ProtectedPairingArtifact> revealInvitationQr() async {
    revealCalls += 1;
    return const _ViewArtifact();
  }

  @override
  Future<ProtectedPairingArtifact> revealSas() async => const _ViewArtifact();

  @override
  Future<void> submitManualJoinCode(String code) async {}
}

class _ViewArtifact implements ProtectedPairingArtifact {
  const _ViewArtifact();

  @override
  Widget buildProtectedContent(BuildContext context) =>
      const Text('Protected test artifact');
}

class _ViewPreview implements ProtectedCameraPreview {
  const _ViewPreview();

  @override
  Widget buildProtectedPreview(BuildContext context) => const SizedBox();
}
