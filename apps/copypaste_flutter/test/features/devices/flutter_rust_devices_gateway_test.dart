import 'dart:async';
import 'dart:typed_data';

import 'package:copypaste_flutter/features/devices/device_presentation.dart';
import 'package:copypaste_flutter/features/devices/devices_gateway.dart';
import 'package:copypaste_flutter/features/devices/flutter_rust_devices_gateway.dart';
import 'package:copypaste_flutter/generated/api.dart' as frb;
import 'package:flutter_test/flutter_test.dart';

void main() {
  late _FakeGeneratedDevicesApi api;
  late FlutterRustDevicesGateway gateway;

  setUp(() {
    api = _FakeGeneratedDevicesApi();
    gateway = FlutterRustDevicesGateway(api: api);
  });

  tearDown(() => gateway.dispose());

  test(
    'maps generated device metadata without changing presence meaning',
    () async {
      final snapshot = await gateway.load();

      expect(snapshot.thisDeviceName, 'Desktop');
      expect(snapshot.thisDevice.id, 'local-device-id');
      expect(
        snapshot.thisDevice.details!.profile!.platform,
        DevicePlatform.macos,
      );
      expect(snapshot.peers.single.id, 'peer-id');
      expect(snapshot.peers.single.online, isFalse);
      expect(
        snapshot.peers.single.details!.profile!.deviceClass,
        DeviceClass.phone,
      );
      expect(snapshot.peers.single.details!.profile!.osName, 'Android');
      expect(
        snapshot.peers.single.details!.endpoint!.lanEndpoint,
        '192.0.2.10:47654',
      );
      expect(
        snapshot.peers.single.details!.latency!.roundTripLatency,
        const Duration(milliseconds: 24),
      );
      expect(
        snapshot.peers.single.details!.presence!.state,
        DevicePresence.online,
      );
      expect(snapshot.discovered.single.id, 'discovery-id');
      expect(
        snapshot.discovered.single.lastSeen,
        DateTime.fromMillisecondsSinceEpoch(20, isUtc: true),
      );
    },
  );

  test('maps Linux profiles from the generated platform contract', () async {
    api.thisDevicePlatform = frb.DevicePlatform.linux;

    final snapshot = await gateway.load();

    expect(
      snapshot.thisDevice.details!.profile!.platform,
      DevicePlatform.linux,
    );
    expect(DevicePresentation.platformLabel(DevicePlatform.linux), 'Linux');
  });

  test(
    'routes unpair and permanent revoke to separate generated actions',
    () async {
      await gateway.unpair('peer-id');
      await gateway.revoke('peer-id');

      expect(api.unpairedIds, ['peer-id']);
      expect(api.revokedIds, ['peer-id']);
    },
  );

  test('surfaces generated sync outcome failures', () async {
    api.syncOutcomes = const [
      frb.SyncOutcome(
        pairingId: 'peer-id',
        name: 'Phone',
        sent: 0,
        received: 0,
        errorCode: 'offline',
      ),
    ];

    expect(
      gateway.sync(peerId: 'peer-id'),
      throwsA(isA<DevicesSyncException>()),
    );
  });

  test('does not synthesize changes for local mutations', () async {
    final changes = <void>[];
    final subscription = gateway.changes.listen(changes.add);
    addTearDown(subscription.cancel);
    await Future<void>.delayed(Duration.zero);

    await gateway.rescan();
    await gateway.sync(peerId: 'peer-id');
    await gateway.unpair('peer-id');
    await gateway.revoke('peer-id');
    await gateway.setThisDeviceName('Renamed desktop');
    await Future<void>.delayed(Duration.zero);

    expect(changes, isEmpty);
  });

  test('maps the generated opaque inviter ceremony and releases it', () async {
    final session = await gateway.createInvitation();

    expect(session.ceremony.state, PairingState.waitingForPeer);
    await session.cancel();
    await session.dispose();

    expect(api.cancelCeremonyIds, ['ceremony-id']);
    expect(api.disposedCeremonyIds, ['ceremony-id']);
  });

  test('maps one matching invitation result from Rust', () async {
    final session = await gateway.createInvitation();
    final invitation = await session.revealInvitation();
    expect(invitation.qrPng, [1, 2, 3]);
    expect(invitation.code, 'PAIRING-CODE');
    expect(invitation.address, '192.168.50.232:62951');
    await session.dispose();
  });

  test('passes code and address through the generated join contract', () async {
    final session = await gateway.joinFromProtectedInput(
      code: 'PAIRING-CODE',
      address: '192.0.2.10:47654',
    );

    expect(api.joinedCodes, ['PAIRING-CODE']);
    expect(api.joinedAddresses, ['192.0.2.10:47654']);
    expect(session.ceremony.state, PairingState.handshaking);
  });

  test('passes a decoded versioned pairing URI to Rust unchanged', () async {
    const uri = 'copypaste://pair/v1?code=SECRET&address=192.0.2.10%3A47654';

    final session = await gateway.joinPairingUri(uri);

    expect(api.joinedUris, [uri]);
    expect(session.ceremony.state, PairingState.handshaking);
  });

  test(
    'binds confirmation to the freshly revealed verification code',
    () async {
      final session = await gateway.createInvitation();

      expect(await session.revealSas(), '123456');
      await session.confirm(accept: true);

      expect(api.confirmedVerificationCodes, ['123456']);
      await session.dispose();
    },
  );

  test('reloads peers from the generated content-free runtime watch', () async {
    final changes = <void>[];
    final subscription = gateway.changes.listen(changes.add);
    addTearDown(subscription.cancel);
    await Future<void>.delayed(Duration.zero);

    api.emitWatch(
      frb.RuntimeEvent(kind: 'peers', itemCount: BigInt.zero, captured: false),
    );
    await Future<void>.delayed(Duration.zero);

    expect(changes, hasLength(1));
    await gateway.dispose();
    expect(api.cancelWatchIds, [BigInt.one]);
  });

  test('cancels only the watch allocated for this gateway', () async {
    final otherGateway = FlutterRustDevicesGateway(api: api);
    final firstSubscription = gateway.changes.listen((_) {});
    final secondSubscription = otherGateway.changes.listen((_) {});
    addTearDown(firstSubscription.cancel);
    addTearDown(secondSubscription.cancel);
    await Future<void>.delayed(Duration.zero);

    await gateway.dispose();

    expect(api.cancelWatchIds, [BigInt.one]);
    await otherGateway.dispose();
    expect(api.cancelWatchIds, [BigInt.one, BigInt.from(2)]);
  });
}

class _FakeGeneratedDevicesApi implements GeneratedDevicesApi {
  final List<String> revokedIds = [];
  final List<String> unpairedIds = [];
  final List<String> cancelCeremonyIds = [];
  final List<String> disposedCeremonyIds = [];
  final List<BigInt> cancelWatchIds = [];
  final List<String> joinedCodes = [];
  final List<String> joinedAddresses = [];
  final List<String> joinedUris = [];
  final List<String> confirmedVerificationCodes = [];
  final StreamController<frb.RuntimeEvent> _watch =
      StreamController<frb.RuntimeEvent>.broadcast();
  int _nextWatchId = 1;
  List<frb.SyncOutcome> syncOutcomes = const [];
  frb.DevicePlatform thisDevicePlatform = frb.DevicePlatform.macos;

  @override
  Future<List<frb.DiscoveredDevice>> listDiscoveredDevices() async => [
    frb.DiscoveredDevice(
      discoveryId: 'discovery-id',
      name: 'Nearby',
      address: '192.0.2.20:47654',
      paired: false,
      lastSeenMs: 20,
      details: _details(),
    ),
  ];

  @override
  Future<List<frb.Peer>> listPeers() async => [
    frb.Peer(
      pairingId: 'peer-id',
      name: 'Phone',
      lastSeenMs: 10,
      online: false,
      details: _details(),
    ),
  ];

  @override
  Future<frb.PairingCeremony> joinPairingCeremony({
    required String code,
    required String address,
  }) async {
    joinedCodes.add(code);
    joinedAddresses.add(address);
    return const frb.PairingCeremony(
      ceremonyId: 'join-ceremony-id',
      state: 'handshaking',
    );
  }

  @override
  Future<frb.PairingCeremony> joinPairingUri({required String uri}) async {
    joinedUris.add(uri);
    return const frb.PairingCeremony(
      ceremonyId: 'uri-ceremony-id',
      state: 'handshaking',
    );
  }

  @override
  Future<frb.PairingCeremony> pairingCeremonyStatus({
    required String ceremonyId,
  }) async => const frb.PairingCeremony(
    ceremonyId: 'ceremony-id',
    state: 'waiting_for_peer',
  );

  @override
  Future<List<frb.DiscoveredDevice>> rescanDevices() async => const [];

  @override
  Future<frb.PairingInvitation> revealPairingInvitation({
    required String ceremonyId,
  }) async => frb.PairingInvitation(
    qrPng: Uint8List.fromList([1, 2, 3]),
    code: 'PAIRING-CODE',
    address: '192.168.50.232:62951',
  );

  @override
  Future<String> revealPairingSas({required String ceremonyId}) async =>
      '123456';

  @override
  Future<void> revokeDevice({required String pairingId}) async {
    revokedIds.add(pairingId);
  }

  @override
  Future<void> setThisDeviceName({required String name}) async {}

  @override
  Future<BigInt> allocateRuntimeWatch() async => BigInt.from(_nextWatchId++);

  @override
  Future<void> cancelRuntimeWatch({required BigInt watchId}) async {
    cancelWatchIds.add(watchId);
  }

  @override
  Future<frb.PairingCeremony> cancelPairingCeremony({
    required String ceremonyId,
  }) async {
    cancelCeremonyIds.add(ceremonyId);
    return const frb.PairingCeremony(
      ceremonyId: 'ceremony-id',
      state: 'cancelled',
    );
  }

  @override
  Future<frb.PairingCeremony> confirmPairingCeremony({
    required String ceremonyId,
    required String verificationCode,
    required bool accept,
  }) async {
    confirmedVerificationCodes.add(verificationCode);
    return const frb.PairingCeremony(
      ceremonyId: 'ceremony-id',
      state: 'confirmed',
    );
  }

  @override
  Future<frb.PairingCeremony> createPairingCeremony() async =>
      frb.PairingCeremony(
        ceremonyId: 'ceremony-id',
        state: 'waiting_for_peer',
        expiresInMs: BigInt.from(60000),
      );

  @override
  Future<void> disposePairingCeremony({required String ceremonyId}) async {
    disposedCeremonyIds.add(ceremonyId);
  }

  @override
  Future<List<frb.SyncOutcome>> syncDevices({String? pairingId}) async =>
      syncOutcomes;

  @override
  Future<frb.ThisDevice> thisDeviceName() async => frb.ThisDevice(
    deviceId: 'local-device-id',
    name: 'Desktop',
    appVersion: '1.2.3',
    protocolVersion: 7,
    details: _details(platform: thisDevicePlatform),
  );

  @override
  Future<void> unpairDevice({required String pairingId}) async {
    unpairedIds.add(pairingId);
  }

  void emitWatch(frb.RuntimeEvent event) => _watch.add(event);

  @override
  Stream<frb.RuntimeEvent> watchRuntime({required BigInt watchId}) =>
      _watch.stream;

  frb.DeviceDetails _details({
    frb.DevicePlatform platform = frb.DevicePlatform.android,
  }) => frb.DeviceDetails(
    profile: frb.DeviceProfile(
      displayName: 'Phone',
      appVersion: '1.2.3',
      protocolVersion: 7,
      platform: platform,
      deviceClass: frb.DeviceClass.phone,
      osName: 'Android',
      osVersion: '16',
      model: 'Pixel 9',
      provenance: frb.DeviceObservationProvenance.selfReported,
      trust: frb.DeviceObservationTrust.authenticated,
      observedAtMs: 10,
      freshUntilMs: 20,
    ),
    endpoint: frb.DeviceEndpoint(
      lanEndpoint: '192.0.2.10:47654',
      provenance: frb.DeviceObservationProvenance.observed,
      trust: frb.DeviceObservationTrust.authenticated,
      observedAtMs: 11,
      freshUntilMs: 21,
    ),
    latency: frb.DeviceLatency(
      roundTripLatencyMs: BigInt.from(24),
      provenance: frb.DeviceObservationProvenance.measured,
      trust: frb.DeviceObservationTrust.authenticated,
      observedAtMs: 12,
      freshUntilMs: 22,
    ),
    presence: frb.DevicePresence(
      state: frb.DevicePresenceState.online,
      lastSeenMs: 13,
      provenance: frb.DeviceObservationProvenance.observed,
      trust: frb.DeviceObservationTrust.local,
      observedAtMs: 14,
      freshUntilMs: 24,
    ),
  );
}
