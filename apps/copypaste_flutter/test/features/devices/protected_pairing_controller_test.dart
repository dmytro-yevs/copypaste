import 'dart:async';

import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/features/devices/protected_pairing_controller.dart';
import 'package:copypaste_flutter/features/devices/protected_pairing_port.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  late _FakeProtectedHost host;
  late _FakeProtectedSession session;
  late ProtectedPairingController controller;

  setUp(() {
    host = _FakeProtectedHost();
    session = _FakeProtectedSession();
    controller = ProtectedPairingController(host: host, session: session);
  });

  tearDown(() => controller.dispose());

  test('reveals the invitation immediately in the protected host', () async {
    controller.start();
    await Future<void>.delayed(Duration.zero);

    expect(session.revealInviteCalls, 1);
    expect(controller.artifact, isA<_TestArtifact>());
  });

  test(
    'does not reveal artifacts when the dedicated host is inactive',
    () async {
      host.active = false;
      controller.start();

      await controller.openCameraScanner();

      expect(session.revealInviteCalls, 0);
      expect(session.openCameraCalls, 0);
      expect(controller.artifact, isNull);
    },
  );

  test(
    'clears pairing artifacts as soon as Rust reports a terminal state',
    () async {
      controller.start();
      await Future<void>.delayed(Duration.zero);

      session.emit(const PairingCeremony(state: PairingState.timedOut));
      await Future<void>.delayed(Duration.zero);

      expect(controller.artifact, isNull);
      expect(controller.ceremony.state, PairingState.timedOut);
    },
  );

  test('passes manual code only through the protected session', () async {
    controller.start();

    await controller.submitManualJoinCode('protected route value');

    expect(session.manualJoinCalls, ['protected route value']);
  });

  test('prevents concurrent confirmation decisions', () async {
    final pending = Completer<void>();
    session.confirmation = pending;
    controller.start();
    session.emit(
      const PairingCeremony(state: PairingState.awaitingConfirmation),
    );
    await Future<void>.delayed(Duration.zero);

    final first = controller.confirm(accept: true);
    final second = controller.confirm(accept: false);
    expect(session.confirmations, [true]);

    pending.complete();
    await Future.wait([first, second]);
  });

  test('cancels and closes the protected host exactly once', () async {
    controller.start();

    await controller.close();
    await controller.close();

    expect(session.cancelCalls, 1);
    expect(session.disposeCalls, 1);
    expect(host.closeCalls, 1);
  });
}

class _FakeProtectedHost implements ProtectedPairingHost {
  bool active = true;
  int closeCalls = 0;

  @override
  bool get isActive => active;

  @override
  Future<void> close() async {
    closeCalls += 1;
  }
}

class _FakeProtectedSession implements ProtectedPairingSession {
  final StreamController<PairingCeremony> _updates =
      StreamController<PairingCeremony>.broadcast();
  final List<bool> confirmations = [];
  final List<String> manualJoinCalls = [];
  @override
  PairingCeremony ceremony = const PairingCeremony(
    state: PairingState.waitingForPeer,
  );
  Completer<void>? confirmation;
  int cancelCalls = 0;
  int disposeCalls = 0;
  int openCameraCalls = 0;
  int revealInviteCalls = 0;

  @override
  Stream<PairingCeremony> get updates => _updates.stream;

  @override
  Future<void> cancel() async {
    cancelCalls += 1;
  }

  @override
  Future<void> confirm({required bool accept}) async {
    confirmations.add(accept);
    await confirmation?.future;
  }

  @override
  Future<void> dispose() async {
    disposeCalls += 1;
  }

  void emit(PairingCeremony next) {
    ceremony = next;
    _updates.add(next);
  }

  @override
  Future<ProtectedCameraPreview> openCameraScanner() async {
    openCameraCalls += 1;
    return const _TestCameraPreview();
  }

  @override
  Future<ProtectedPairingArtifact> revealInvitationQr() async {
    revealInviteCalls += 1;
    return const _TestArtifact();
  }

  @override
  Future<ProtectedPairingArtifact> revealSas() async => const _TestArtifact();

  @override
  Future<void> submitManualJoinCode(String code) async {
    manualJoinCalls.add(code);
  }
}

class _TestArtifact implements ProtectedPairingArtifact {
  const _TestArtifact();

  @override
  Widget buildProtectedContent(BuildContext context) => const Text('Artifact');
}

class _TestCameraPreview implements ProtectedCameraPreview {
  const _TestCameraPreview();

  @override
  Widget buildProtectedPreview(BuildContext context) => const Text('Preview');
}
