import 'dart:async';

import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/features/devices/protected_pairing_controller.dart';
import 'package:copypaste_flutter/features/devices/protected_pairing_port.dart';
import 'package:copypaste_flutter/features/devices/protected_pairing_view.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  for (final accept in [true, false]) {
    testWidgets(
      'shows protected SAS immediately with Accept and Reject: $accept',
      (tester) async {
        final session = _ViewSession(
          ceremony: const PairingCeremony(
            state: PairingState.awaitingConfirmation,
          ),
        );
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
        await tester.pumpAndSettle();
        expect(session.revealSasCalls, 1);
        expect(find.text('123456'), findsOneWidget);
        expect(find.text('Reveal verification code'), findsNothing);
        expect(find.text('Accept'), findsOneWidget);
        expect(find.text('Reject'), findsOneWidget);
        expect(find.byType(Button), findsNWidgets(2));
        expect(session.confirmations, isEmpty);
        await tester.tap(find.text(accept ? 'Accept' : 'Reject'));
        await tester.pumpAndSettle();
        expect(session.confirmations, [accept]);
      },
    );
  }

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
  _ViewSession({
    this.ceremony = const PairingCeremony(state: PairingState.waitingForPeer),
  });

  @override
  final PairingCeremony ceremony;

  int revealCalls = 0;
  int revealSasCalls = 0;
  final List<bool> confirmations = [];

  @override
  Stream<PairingCeremony> get updates => const Stream<PairingCeremony>.empty();

  @override
  Future<void> cancel() async {}

  @override
  Future<void> confirm({required bool accept}) async {
    confirmations.add(accept);
  }

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
  Future<ProtectedPairingArtifact> revealSas() async {
    revealSasCalls += 1;
    return const _ViewArtifact(text: '123456');
  }

  @override
  Future<void> submitManualJoinCode(String code) async {}
}

class _ViewArtifact implements ProtectedPairingArtifact {
  const _ViewArtifact({this.text = 'Protected test artifact'});

  final String text;

  @override
  Widget buildProtectedContent(BuildContext context) => Text(text);
}

class _ViewPreview implements ProtectedCameraPreview {
  const _ViewPreview();

  @override
  Widget buildProtectedPreview(BuildContext context) => const SizedBox();
}
