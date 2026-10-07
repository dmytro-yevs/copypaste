import 'dart:async';
import 'dart:convert';

import 'package:copypaste_flutter/features/devices/devices.dart';
import 'package:copypaste_flutter/platform/camera/system_pairing_scanner.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  late _FakeDevicesGateway gateway;
  late _CaptureProtection captureProtection;
  late DevicesController controller;

  setUp(() {
    gateway = _FakeDevicesGateway();
    captureProtection = _CaptureProtection();
    controller = DevicesController(
      gateway: gateway,
      captureProtection: captureProtection,
    );
  });

  tearDown(() => controller.dispose());

  test(
    'switches an invitation to code entry while keeping protection',
    () async {
      await controller.openInvitation();
      final cancellation = Completer<void>();
      gateway.session.cancelPending = cancellation;

      final switching = controller.openCodeEntry(address: '192.0.2.10:47654');
      await Future<void>.delayed(Duration.zero);
      expect(controller.canChangePairingMode, isFalse);
      expect(controller.canClosePairing, isFalse);
      expect(controller.pairingInspectorOpen, isTrue);
      expect(controller.invitation?.qrPng, isNull);
      await controller.openQrScanner();
      expect(controller.pairingEntryMode, PairingEntryMode.invite);

      cancellation.complete();
      await switching;
      expect(gateway.session.cancelCalls, 1);
      expect(gateway.session.disposeCalls, 1);
      expect(controller.pairingEntryMode, PairingEntryMode.enterCode);
      expect(controller.pendingAddress, '192.0.2.10:47654');
      expect(controller.pairing, isNull);
      expect(controller.canChangePairingMode, isTrue);
      expect(captureProtection.values, [true]);
    },
  );

  test('discards a verification code returned after switching modes', () async {
    await controller.openInvitation();
    final code = Completer<String>();
    gateway.session.sasPending = code;
    gateway.session.emit(
      const PairingCeremony(state: PairingState.awaitingConfirmation),
    );
    await Future<void>.delayed(Duration.zero);
    expect(gateway.session.revealSasCalls, 1);

    await controller.openCodeEntry();
    code.complete('123456');
    await Future<void>.delayed(Duration.zero);

    expect(controller.pairingEntryMode, PairingEntryMode.enterCode);
    expect(controller.verificationCode, isNull);
    expect(controller.canConfirmPairing, isFalse);
  });

  test(
    'old pairing cleanup cannot unprotect a newly opened inspector',
    () async {
      await controller.openInvitation();
      final cancellation = Completer<void>();
      gateway.session.cancelPending = cancellation;
      final closing = controller.closePairing();
      await Future<void>.delayed(Duration.zero);
      await controller.openCodeEntry();
      cancellation.complete();
      await closing;
      expect(controller.pairingEntryMode, PairingEntryMode.enterCode);
      expect(captureProtection.values, [true]);
      await controller.closePairing();
      expect(captureProtection.values, [true, false]);
    },
  );

  test('late capture protection cannot reopen closed pairing', () async {
    final enabled = Completer<bool>();
    captureProtection.pendingEnable = enabled;
    final opening = controller.openCodeEntry();
    await Future<void>.delayed(Duration.zero);
    await controller.closePairing();
    enabled.complete(true);
    await opening;
    expect(controller.pairingInspectorOpen, isFalse);
    expect(captureProtection.values, [true, false]);
  });

  test(
    'system QR scanning protects the surface and submits only its result',
    () async {
      controller.dispose();
      final scanner = _SystemScanner();
      controller = DevicesController(
        gateway: gateway,
        captureProtection: captureProtection,
        systemScanner: scanner,
      );
      final scan = controller.openQrScanner();
      await Future<void>.delayed(Duration.zero);
      expect(captureProtection.values, [true]);
      expect(controller.systemScanInFlight, isTrue);
      expect(controller.pairingInspectorOpen, isFalse);
      scanner.result.complete('copypaste://pair/v1?test=invitation');
      await scan;
      expect(gateway.joinedUris, ['copypaste://pair/v1?test=invitation']);
      expect(controller.systemScanInFlight, isFalse);
      expect(controller.pairingInspectorOpen, isTrue);
    },
  );

  test(
    'cancelling the system scanner releases pairing and permits details',
    () async {
      controller.dispose();
      final scanner = _SystemScanner();
      controller = DevicesController(
        gateway: gateway,
        captureProtection: captureProtection,
        systemScanner: scanner,
      );
      final scan = controller.openQrScanner();
      await Future<void>.delayed(Duration.zero);
      scanner.result.complete(null);
      await scan;
      expect(controller.pairingInspectorOpen, isFalse);
      expect(controller.errorMessage, isNull);
      expect(captureProtection.values, [true, false]);
      controller.openThisDeviceDetails();
      expect(controller.deviceDetailsOpen, isTrue);
    },
  );

  test('closing pairing discards a late system scanner result', () async {
    controller.dispose();
    final scanner = _SystemScanner();
    controller = DevicesController(
      gateway: gateway,
      captureProtection: captureProtection,
      systemScanner: scanner,
    );
    final scan = controller.openQrScanner();
    await Future<void>.delayed(Duration.zero);
    await controller.closePairing();
    scanner.result.complete('copypaste://pair/v1?test=stale');
    await scan;
    expect(gateway.joinedUris, isEmpty);
    expect(controller.pairingInspectorOpen, isFalse);
  });

  test('unavailable system scanner allows manual code entry', () async {
    controller.dispose();
    final scanner = _SystemScanner();
    controller = DevicesController(
      gateway: gateway,
      captureProtection: captureProtection,
      systemScanner: scanner,
    );
    final scan = controller.openQrScanner();
    await Future<void>.delayed(Duration.zero);
    scanner.result.completeError(
      PlatformException(code: 'scanner_unavailable'),
    );
    await scan;
    expect(controller.errorMessage, contains('Enter the pairing code'));
    expect(controller.pairingInspectorOpen, isFalse);
    expect(controller.pairingEntryMode, isNull);
    expect(captureProtection.values, [true, false]);
    await controller.refresh();
    expect(controller.errorMessage, contains('Enter the pairing code'));
    await controller.openCodeEntry();
    expect(controller.pairingEntryMode, PairingEntryMode.enterCode);
    expect(controller.errorMessage, isNull);
  });

  test('loads real-contract-shaped peers and discovery results', () async {
    gateway.snapshot = _snapshot(peers: [_peer('trusted')]);

    await controller.start();

    expect(controller.loadState, DevicesLoadState.ready);
    expect(controller.snapshot!.peers.single.name, 'trusted');
    expect(controller.snapshot!.discovered.single.name, 'Nearby device');
  });

  test('refreshes when the Rust watch signal changes', () async {
    await controller.start();
    gateway.snapshot = _snapshot(peers: [_peer('Updated peer')]);

    gateway.emitChange();
    await Future<void>.delayed(Duration.zero);

    expect(controller.snapshot!.peers.single.name, 'Updated peer');
  });

  test(
    'coalesces overlapping refreshes without retaining an older snapshot',
    () async {
      final first = Completer<DevicesSnapshot>();
      final second = Completer<DevicesSnapshot>();
      gateway.pendingLoads.addAll([first, second]);

      final initialLoad = controller.start();
      await Future<void>.delayed(Duration.zero);
      final overlappingRefresh = controller.refresh();
      first.complete(_snapshot(peers: [_peer('Older peer')]));
      await Future.wait([initialLoad, overlappingRefresh]);
      await Future<void>.delayed(Duration.zero);

      second.complete(_snapshot(peers: [_peer('Newest peer')]));
      await Future<void>.delayed(Duration.zero);

      expect(gateway.loadCalls, 2);
      expect(controller.snapshot!.peers.single.name, 'Newest peer');
    },
  );

  test(
    'closes selected peer details when an external refresh removes the peer',
    () async {
      gateway.snapshot = _snapshot(peers: [_peer('Trusted peer')]);
      await controller.start();
      controller.openPeerDetails('peer-id');

      gateway.snapshot = _snapshot();
      gateway.emitChange();
      await Future<void>.delayed(Duration.zero);

      expect(controller.deviceDetailsTarget, isNull);
    },
  );

  test(
    'reveals the invitation only after the explicit Pair device action',
    () async {
      await controller.openInvitation();

      expect(gateway.session.revealInviteCalls, 1);
      expect(controller.pairing!.state, PairingState.waitingForPeer);
      expect(gateway.session.revealSasCalls, 0);
      expect(controller.invitation?.qrPng, isNotNull);
    },
  );

  test(
    'allows decisions only while Rust reports confirmation is pending',
    () async {
      await controller.openInvitation();

      await controller.confirmPairing(accept: true);
      expect(gateway.session.confirmations, isEmpty);

      gateway.session.emit(
        const PairingCeremony(state: PairingState.awaitingConfirmation),
      );
      await Future<void>.delayed(Duration.zero);
      expect(controller.verificationCode, '123456');
      await controller.confirmPairing(accept: true);

      expect(gateway.session.confirmations, [true]);
    },
  );

  for (final accept in [true, false]) {
    test(
      'automatically shows SAS once before an explicit decision: $accept',
      () async {
        await controller.openInvitation();
        final code = Completer<String>();
        gateway.session.sasPending = code;
        for (var update = 0; update < 3; update++) {
          gateway.session.emit(
            const PairingCeremony(state: PairingState.awaitingConfirmation),
          );
        }
        await Future<void>.delayed(Duration.zero);
        expect(gateway.session.revealSasCalls, 1);
        expect(controller.canConfirmPairing, isFalse);
        await controller.confirmPairing(accept: accept);
        expect(gateway.session.confirmations, isEmpty);

        code.complete('123456');
        await Future<void>.delayed(Duration.zero);
        expect(controller.verificationCode, '123456');
        expect(gateway.session.confirmations, isEmpty);
        await controller.confirmPairing(accept: accept);
        expect(gateway.session.confirmations, [accept]);
      },
    );
  }

  test(
    'automatically shows SAS from an already ready joined session',
    () async {
      gateway.session.emit(
        const PairingCeremony(state: PairingState.awaitingConfirmation),
      );
      await controller.joinPairingUri('copypaste://pair/v1?test=invitation');
      await Future<void>.delayed(Duration.zero);
      expect(controller.verificationCode, '123456');
      expect(gateway.session.revealSasCalls, 1);
      expect(gateway.session.confirmations, isEmpty);
    },
  );

  test('discards an automatic SAS result after confirmation expires', () async {
    await controller.openInvitation();
    final code = Completer<String>();
    gateway.session.sasPending = code;
    gateway.session.emit(
      const PairingCeremony(state: PairingState.awaitingConfirmation),
    );
    await Future<void>.delayed(Duration.zero);
    gateway.session.emit(const PairingCeremony(state: PairingState.timedOut));
    await Future<void>.delayed(Duration.zero);
    code.complete('123456');
    await Future<void>.delayed(Duration.zero);
    expect(controller.verificationCode, isNull);
    expect(controller.canConfirmPairing, isFalse);
  });

  test(
    'preserves a terminal status until dismissed and cleans the session',
    () async {
      await controller.openInvitation();
      gateway.session.emit(const PairingCeremony(state: PairingState.timedOut));
      await Future<void>.delayed(Duration.zero);

      expect(controller.pairing!.state, PairingState.timedOut);
      expect(gateway.session.disposeCalls, 1);

      await controller.closePairing();
      expect(controller.pairing, isNull);
    },
  );

  test('cancels an unfinished session during disposal', () async {
    await controller.openInvitation();

    controller.dispose();
    await Future<void>.delayed(Duration.zero);

    expect(gateway.session.cancelCalls, 1);
    expect(gateway.session.disposeCalls, 1);
  });

  test(
    'keeps unpair and permanent revoke as distinct backend actions',
    () async {
      await controller.unpair('peer-id');
      await controller.revoke('peer-id');

      expect(gateway.unpairedIds, ['peer-id']);
      expect(gateway.revokedIds, ['peer-id']);
    },
  );

  test('prevents duplicate device mutations while one is pending', () async {
    final pending = Completer<void>();
    gateway.rescanPending = pending;

    final first = controller.rescan();
    final second = controller.rescan();

    expect(gateway.rescanCalls, 1);
    expect(controller.actionInFlight, isTrue);
    pending.complete();
    await Future.wait([first, second]);
    expect(controller.actionInFlight, isFalse);
  });

  test('invalidates expired observations without a metadata request', () async {
    final clock = _TestClock(DateTime.utc(2026, 10, 3, 12));
    final scheduler = _FreshnessTimerFactory();
    final deadline = clock.now.add(const Duration(seconds: 5));
    gateway.snapshot = _snapshot(peers: [_observedPeer(deadline)]);
    final freshnessController = DevicesController(
      gateway: gateway,
      captureProtection: captureProtection,
      now: () => clock.now,
      freshnessTimerFactory: scheduler.schedule,
    );
    addTearDown(freshnessController.dispose);

    await freshnessController.start();
    final peer = freshnessController.snapshot!.peers.single;

    expect(freshnessController.peerStateLabel(peer), 'Available');
    expect(freshnessController.peerLatencyLabel(peer), '24 ms');
    expect(
      freshnessController.peerStateLabel(_mdnsPeer(deadline)),
      'Visible now',
    );
    expect(freshnessController.peerLatencyLabel(_mdnsPeer(deadline)), '— ms');
    expect(scheduler.timers, hasLength(1));
    expect(scheduler.timers.single.delay, const Duration(seconds: 5));

    clock.now = deadline.add(const Duration(milliseconds: 1));
    expect(freshnessController.peerStateLabel(peer), 'Status unknown');
    expect(freshnessController.peerLatencyLabel(peer), '— ms');

    scheduler.timers.single.fire();
    await Future<void>.delayed(Duration.zero);

    expect(gateway.loadCalls, 1);
  });

  test('runtime events renew latency without a UI renewal poll', () async {
    final clock = _TestClock(DateTime.utc(2026, 10, 3, 12));
    final scheduler = _FreshnessTimerFactory();
    final deadline = clock.now.add(const Duration(seconds: 30));
    gateway.snapshot = _snapshot(peers: [_observedPeer(deadline)]);
    final freshnessController = DevicesController(
      gateway: gateway,
      captureProtection: captureProtection,
      now: () => clock.now,
      freshnessTimerFactory: scheduler.schedule,
    );
    addTearDown(freshnessController.dispose);
    await freshnessController.start();
    expect(scheduler.timers.single.delay, const Duration(seconds: 30));
    clock.now = deadline.subtract(const Duration(seconds: 10));
    gateway.snapshot = _snapshot(
      peers: [_observedPeer(clock.now.add(const Duration(seconds: 30)))],
    );
    gateway.emitChange();
    await Future<void>.delayed(Duration.zero);
    expect(scheduler.timers.first._cancelled, isTrue);
    expect(scheduler.timers.last.delay, const Duration(seconds: 30));
    expect(gateway.loadCalls, 2);
    expect(
      freshnessController.peerLatencyLabel(
        freshnessController.snapshot!.peers.single,
      ),
      '24 ms',
    );
  });

  test('displays sub-millisecond RTT as less than one millisecond', () async {
    final now = DateTime.now().toUtc();
    final peer = DevicePeer(
      id: 'peer',
      name: 'Phone',
      lastSeen: now,
      online: true,
      details: DeviceDetails(
        latency: DeviceLatency(
          roundTripLatency: Duration.zero,
          provenance: DeviceObservationProvenance.measured,
          trust: DeviceObservationTrust.authenticated,
          observedAt: now,
          freshUntil: now.add(const Duration(seconds: 30)),
        ),
      ),
    );
    expect(controller.peerLatencyLabel(peer), '<1 ms');
  });

  test(
    'keeps device details separate from the protected pairing inspector',
    () async {
      controller.openThisDeviceDetails();
      expect(
        controller.deviceDetailsTarget,
        const DeviceDetailsTarget.thisDevice(),
      );

      await controller.openCodeEntry();
      expect(controller.deviceDetailsTarget, isNull);
      expect(controller.pairingEntryMode, PairingEntryMode.enterCode);

      controller.openPeerDetails('peer-id');
      expect(controller.deviceDetailsTarget, isNull);

      await controller.closePairing();
      controller.openPeerDetails('peer-id');
      expect(
        controller.deviceDetailsTarget,
        const DeviceDetailsTarget.peer('peer-id'),
      );
    },
  );

  test('closes selected peer details after successful removal', () async {
    controller.openPeerDetails('peer-id');

    await controller.unpair('peer-id');

    expect(controller.deviceDetailsTarget, isNull);
  });

  test(
    'keeps a successful mutation successful when the follow-up refresh fails',
    () async {
      await controller.start();
      controller.openPeerDetails('peer-id');
      gateway.loadError = StateError('refresh unavailable');

      final succeeded = await controller.unpair('peer-id');

      expect(succeeded, isTrue);
      expect(controller.deviceDetailsTarget, isNull);
      expect(controller.errorMessage, contains('refresh unavailable'));
    },
  );

  test(
    'passes the backend-required code and address to a manual join',
    () async {
      await controller.openCodeEntry();

      await controller.joinFromProtectedInput(
        code: 'PAIRING-CODE',
        address: '192.0.2.10:47654',
      );

      expect(gateway.joinedCodes, ['PAIRING-CODE']);
      expect(gateway.joinedAddresses, ['192.0.2.10:47654']);
      expect(controller.pairingEntryMode, PairingEntryMode.enterCode);
    },
  );

  test(
    'protects the app surface for the complete inspector lifetime',
    () async {
      await controller.openCodeEntry();
      expect(captureProtection.values, [true]);

      await controller.closePairing();
      expect(captureProtection.values, [true, false]);
    },
  );

  test('does not expose pairing input when capture protection fails', () async {
    final blocked = DevicesController(
      gateway: gateway,
      captureProtection: _CaptureProtection(result: false),
    );
    addTearDown(blocked.dispose);

    await blocked.openCodeEntry();

    expect(blocked.pairingEntryMode, isNull);
    expect(blocked.errorMessage, 'Secure pairing presentation is unavailable.');
  });
}

DevicesSnapshot _snapshot({List<DevicePeer> peers = const []}) =>
    DevicesSnapshot(
      thisDevice: const ThisDevice(
        name: 'This device',
        appVersion: '1.0.0',
        protocolVersion: 1,
      ),
      peers: peers,
      discovered: [
        DiscoveredDevice(
          id: 'nearby',
          name: 'Nearby device',
          address: '192.0.2.10:47654',
          paired: false,
          lastSeen: DateTime.utc(2026, 10, 3),
        ),
      ],
    );

DevicePeer _peer(String name) => DevicePeer(
  id: 'peer-id',
  name: name,
  lastSeen: DateTime.utc(2026, 10, 3),
  online: false,
);

DevicePeer _observedPeer(DateTime deadline) => DevicePeer(
  id: 'observed-peer',
  name: 'Observed peer',
  lastSeen: deadline.subtract(const Duration(seconds: 1)),
  online: true,
  details: DeviceDetails(
    latency: DeviceLatency(
      roundTripLatency: const Duration(milliseconds: 24),
      provenance: DeviceObservationProvenance.measured,
      trust: DeviceObservationTrust.authenticated,
      observedAt: deadline.subtract(const Duration(seconds: 5)),
      freshUntil: deadline,
    ),
    presence: DevicePresenceObservation(
      state: DevicePresence.online,
      lastSeen: deadline.subtract(const Duration(seconds: 1)),
      provenance: DeviceObservationProvenance.measured,
      trust: DeviceObservationTrust.authenticated,
      observedAt: deadline.subtract(const Duration(seconds: 5)),
      freshUntil: deadline,
    ),
  ),
);

DevicePeer _mdnsPeer(DateTime deadline) => DevicePeer(
  id: 'mdns-peer',
  name: 'mDNS peer',
  lastSeen: deadline.subtract(const Duration(seconds: 1)),
  online: true,
  details: DeviceDetails(
    latency: DeviceLatency(
      roundTripLatency: const Duration(milliseconds: 24),
      provenance: DeviceObservationProvenance.observed,
      trust: DeviceObservationTrust.local,
      observedAt: deadline.subtract(const Duration(seconds: 5)),
      freshUntil: deadline,
    ),
    presence: DevicePresenceObservation(
      state: DevicePresence.online,
      lastSeen: deadline.subtract(const Duration(seconds: 1)),
      provenance: DeviceObservationProvenance.observed,
      trust: DeviceObservationTrust.local,
      observedAt: deadline.subtract(const Duration(seconds: 5)),
      freshUntil: deadline,
    ),
  ),
);

class _TestClock {
  _TestClock(this.now);

  DateTime now;
}

class _FreshnessTimerFactory {
  final List<_ManualFreshnessTimer> timers = [];

  DevicesFreshnessTimer schedule(Duration delay, void Function() callback) {
    final timer = _ManualFreshnessTimer(delay, callback);
    timers.add(timer);
    return timer;
  }
}

class _ManualFreshnessTimer implements DevicesFreshnessTimer {
  _ManualFreshnessTimer(this.delay, this._callback);

  final Duration delay;
  final void Function() _callback;
  bool _cancelled = false;

  @override
  void cancel() => _cancelled = true;

  void fire() {
    if (!_cancelled) _callback();
  }
}

class _FakeDevicesGateway implements DevicesGateway {
  final StreamController<void> _changes = StreamController<void>.broadcast();
  final _FakePairingSession session = _FakePairingSession();
  final List<String> unpairedIds = [];
  final List<String> revokedIds = [];
  final List<String> joinedCodes = [];
  final List<String> joinedAddresses = [];
  final List<String> joinedUris = [];
  Completer<void>? rescanPending;
  int rescanCalls = 0;
  int loadCalls = 0;
  Object? loadError;
  final List<Completer<DevicesSnapshot>> pendingLoads = [];
  DevicesSnapshot snapshot = _snapshot();

  @override
  Stream<void> get changes => _changes.stream;

  @override
  Future<DevicesPairingSession> createInvitation() async => session;

  void emitChange() => _changes.add(null);

  @override
  Future<DevicesPairingSession> joinFromProtectedInput({
    required String code,
    required String address,
  }) async {
    joinedCodes.add(code);
    joinedAddresses.add(address);
    return session;
  }

  @override
  Future<DevicesPairingSession> joinPairingUri(String uri) async {
    joinedUris.add(uri);
    return session;
  }

  @override
  Future<DevicesSnapshot> load() async {
    loadCalls += 1;
    if (loadError case final error?) throw error;
    if (pendingLoads.isNotEmpty) return pendingLoads.removeAt(0).future;
    return snapshot;
  }

  @override
  Future<void> rescan() async {
    rescanCalls += 1;
    await rescanPending?.future;
  }

  @override
  Future<void> revoke(String peerId) async => revokedIds.add(peerId);

  @override
  Future<void> setThisDeviceName(String name) async {}

  @override
  Future<void> sync({String? peerId}) async {}

  @override
  Future<void> unpair(String peerId) async => unpairedIds.add(peerId);
}

class _FakePairingSession implements DevicesPairingSession {
  final StreamController<PairingCeremony> _updates =
      StreamController<PairingCeremony>.broadcast();
  final List<bool> confirmations = [];
  PairingCeremony _ceremony = const PairingCeremony(
    state: PairingState.waitingForPeer,
  );
  int cancelCalls = 0;
  int disposeCalls = 0;
  int revealInviteCalls = 0;
  int revealSasCalls = 0;
  Completer<void>? cancelPending;

  @override
  PairingCeremony get ceremony => _ceremony;

  @override
  Stream<PairingCeremony> get updates => _updates.stream;

  @override
  Future<void> cancel() async {
    cancelCalls += 1;
    await cancelPending?.future;
  }

  @override
  Future<void> confirm({required bool accept}) async {
    confirmations.add(accept);
  }

  @override
  Future<void> dispose() async {
    disposeCalls += 1;
  }

  void emit(PairingCeremony ceremony) {
    _ceremony = ceremony;
    _updates.add(ceremony);
  }

  @override
  Future<PairingInvitation> revealInvitation() async {
    revealInviteCalls += 1;
    return PairingInvitation(
      qrPng: _testPng(),
      code: 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRST',
      address: '192.168.50.232:62951',
    );
  }

  @override
  Future<String> revealSas() async {
    revealSasCalls += 1;
    return sasPending?.future ?? '123456';
  }

  Completer<String>? sasPending;
}

Uint8List _testPng() => base64Decode(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
);

class _CaptureProtection implements PairingCaptureProtection {
  _CaptureProtection({this.result = true});

  final bool result;
  final List<bool> values = [];
  Completer<bool>? pendingEnable;

  @override
  Future<bool> setEnabled(bool enabled) async {
    values.add(enabled);
    if (enabled && pendingEnable != null) return pendingEnable!.future;
    return result;
  }
}

class _SystemScanner implements SystemPairingScanner {
  final result = Completer<String?>();

  @override
  Future<String?> scan() => result.future;

  @override
  Future<void> cancel() async {}
}
