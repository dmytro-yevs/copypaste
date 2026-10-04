import 'dart:async';
import 'dart:typed_data';

import 'package:copypaste_flutter/generated/api.dart' as frb;

import 'devices_gateway.dart';

/// Production [DevicesGateway] backed by the generated Rust API.
///
/// [changes] contains only backend runtime invalidations for externally
/// observed peer changes. Local mutations do not synthesize a change event:
/// their caller awaits the operation and performs one authoritative refresh.
class FlutterRustDevicesGateway
    implements DevicesGateway, DisposableDevicesGateway {
  FlutterRustDevicesGateway({GeneratedDevicesApi? api})
    : _api = api ?? FrbGeneratedDevicesApi();

  final GeneratedDevicesApi _api;
  final StreamController<void> _changes = StreamController<void>.broadcast();
  StreamSubscription<frb.RuntimeEvent>? _runtimeWatch;
  Future<void>? _startingRuntimeWatch;
  BigInt? _runtimeWatchId;
  bool _disposed = false;

  @override
  Stream<void> get changes {
    _startRuntimeWatch();
    return _changes.stream;
  }

  @override
  Future<DevicesSnapshot> load() async {
    final thisDevice = await _api.thisDeviceName();
    final peers = await _api.listPeers();
    final discovered = await _api.listDiscoveredDevices();
    return DevicesSnapshot(
      thisDevice: _thisDevice(thisDevice),
      peers: peers
          .map(
            (peer) => DevicePeer(
              id: peer.pairingId,
              name: peer.name,
              lastSeen: _fromEpochMilliseconds(peer.lastSeenMs),
              online: peer.online,
              details: _deviceDetails(peer.details),
            ),
          )
          .toList(growable: false),
      discovered: discovered
          .map(
            (device) => DiscoveredDevice(
              id: device.discoveryId,
              name: device.name,
              address: device.address,
              paired: device.paired,
              lastSeen: _fromEpochMilliseconds(device.lastSeenMs),
              details: _deviceDetails(device.details),
            ),
          )
          .toList(growable: false),
    );
  }

  @override
  Future<void> rescan() async {
    await _api.rescanDevices();
  }

  @override
  Future<void> sync({String? peerId}) async {
    final outcomes = await _api.syncDevices(pairingId: peerId);
    final failures = outcomes
        .where((outcome) => outcome.errorCode != null)
        .map((outcome) => '${outcome.name}: ${outcome.errorCode}')
        .join(', ');
    if (failures.isNotEmpty) throw DevicesSyncException(failures);
  }

  @override
  Future<void> unpair(String peerId) async {
    await _api.unpairDevice(pairingId: peerId);
  }

  @override
  Future<void> revoke(String peerId) async {
    await _api.revokeDevice(pairingId: peerId);
  }

  @override
  Future<void> setThisDeviceName(String name) async {
    await _api.setThisDeviceName(name: name);
  }

  @override
  Future<DevicesPairingSession> createInvitation() async {
    final ceremony = await _api.createPairingCeremony();
    return FlutterRustPairingSession(api: _api, ceremony: ceremony);
  }

  @override
  Future<DevicesPairingSession> joinPairingUri(String uri) async {
    final ceremony = await _api.joinPairingUri(uri: uri);
    return FlutterRustPairingSession(api: _api, ceremony: ceremony);
  }

  @override
  Future<DevicesPairingSession> joinFromProtectedInput({
    required String code,
    required String address,
  }) => _join(code: code, address: address);

  Future<DevicesPairingSession> _join({
    required String code,
    required String address,
  }) async {
    final ceremony = await _api.joinPairingCeremony(
      code: code,
      address: address,
    );
    return FlutterRustPairingSession(api: _api, ceremony: ceremony);
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _startingRuntimeWatch;
    _startingRuntimeWatch = null;
    await _runtimeWatch?.cancel();
    _runtimeWatch = null;
    final watchId = _runtimeWatchId;
    if (watchId != null) {
      await _api.cancelRuntimeWatch(watchId: watchId);
    }
    await _changes.close();
  }

  void _emitExternalPeerChange() {
    if (!_changes.isClosed) _changes.add(null);
  }

  void _startRuntimeWatch() {
    if (_disposed || _runtimeWatch != null || _startingRuntimeWatch != null) {
      return;
    }
    _startingRuntimeWatch = _createRuntimeWatch();
  }

  Future<void> _createRuntimeWatch() async {
    final watchId = await _api.allocateRuntimeWatch();
    if (_disposed) {
      await _api.cancelRuntimeWatch(watchId: watchId);
      return;
    }
    _runtimeWatchId = watchId;
    _runtimeWatch = _api
        .watchRuntime(watchId: watchId)
        .listen(
          (event) {
            if (event.kind == 'peers') _emitExternalPeerChange();
          },
          onError: (Object error, StackTrace stackTrace) {
            if (!_changes.isClosed) _changes.addError(error, stackTrace);
          },
        );
  }

  DateTime _fromEpochMilliseconds(int value) =>
      DateTime.fromMillisecondsSinceEpoch(value, isUtc: true);

  ThisDevice _thisDevice(frb.ThisDevice device) => ThisDevice(
    id: device.deviceId,
    name: device.name,
    appVersion: device.appVersion,
    protocolVersion: device.protocolVersion,
    listenAddress: device.listenAddress,
    details: _deviceDetails(device.details),
  );

  DeviceDetails? _deviceDetails(frb.DeviceDetails? details) {
    if (details == null) return null;
    return DeviceDetails(
      profile: details.profile == null
          ? null
          : _deviceProfile(details.profile!),
      endpoint: details.endpoint == null
          ? null
          : _deviceEndpoint(details.endpoint!),
      latency: details.latency == null
          ? null
          : _deviceLatency(details.latency!),
      presence: details.presence == null
          ? null
          : _devicePresence(details.presence!),
    );
  }

  DeviceProfile _deviceProfile(frb.DeviceProfile profile) => DeviceProfile(
    displayName: profile.displayName,
    appVersion: profile.appVersion,
    protocolVersion: profile.protocolVersion,
    platform: _devicePlatform(profile.platform),
    deviceClass: mapRuntimeDeviceClass(profile.deviceClass),
    osName: profile.osName,
    osVersion: profile.osVersion,
    model: profile.model,
    provenance: _provenance(profile.provenance),
    trust: _trust(profile.trust),
    observedAt: _fromEpochMilliseconds(profile.observedAtMs),
    freshUntil: profile.freshUntilMs == null
        ? null
        : _fromEpochMilliseconds(profile.freshUntilMs!),
  );

  DeviceEndpoint _deviceEndpoint(frb.DeviceEndpoint endpoint) => DeviceEndpoint(
    lanEndpoint: endpoint.lanEndpoint,
    provenance: _provenance(endpoint.provenance),
    trust: _trust(endpoint.trust),
    observedAt: _fromEpochMilliseconds(endpoint.observedAtMs),
    freshUntil: endpoint.freshUntilMs == null
        ? null
        : _fromEpochMilliseconds(endpoint.freshUntilMs!),
  );

  DeviceLatency _deviceLatency(frb.DeviceLatency latency) => DeviceLatency(
    roundTripLatency: Duration(
      milliseconds: latency.roundTripLatencyMs.toInt(),
    ),
    provenance: _provenance(latency.provenance),
    trust: _trust(latency.trust),
    observedAt: _fromEpochMilliseconds(latency.observedAtMs),
    freshUntil: latency.freshUntilMs == null
        ? null
        : _fromEpochMilliseconds(latency.freshUntilMs!),
  );

  DevicePresenceObservation _devicePresence(frb.DevicePresence presence) =>
      DevicePresenceObservation(
        state: _devicePresenceState(presence.state),
        lastSeen: _fromEpochMilliseconds(presence.lastSeenMs),
        provenance: _provenance(presence.provenance),
        trust: _trust(presence.trust),
        observedAt: _fromEpochMilliseconds(presence.observedAtMs),
        freshUntil: presence.freshUntilMs == null
            ? null
            : _fromEpochMilliseconds(presence.freshUntilMs!),
      );

  DevicePlatform _devicePlatform(frb.DevicePlatform platform) =>
      switch (platform) {
        frb.DevicePlatform.macos => DevicePlatform.macos,
        frb.DevicePlatform.windows => DevicePlatform.windows,
        frb.DevicePlatform.android => DevicePlatform.android,
        frb.DevicePlatform.unknown => DevicePlatform.unknown,
      };

  DevicePresence _devicePresenceState(frb.DevicePresenceState state) =>
      switch (state) {
        frb.DevicePresenceState.online => DevicePresence.online,
        frb.DevicePresenceState.offline => DevicePresence.offline,
        frb.DevicePresenceState.unknown => DevicePresence.unknown,
      };

  DeviceObservationProvenance _provenance(
    frb.DeviceObservationProvenance provenance,
  ) => switch (provenance) {
    frb.DeviceObservationProvenance.selfReported =>
      DeviceObservationProvenance.selfReported,
    frb.DeviceObservationProvenance.observed =>
      DeviceObservationProvenance.observed,
    frb.DeviceObservationProvenance.measured =>
      DeviceObservationProvenance.measured,
  };

  DeviceObservationTrust _trust(frb.DeviceObservationTrust trust) =>
      switch (trust) {
        frb.DeviceObservationTrust.local => DeviceObservationTrust.local,
        frb.DeviceObservationTrust.unverified =>
          DeviceObservationTrust.unverified,
        frb.DeviceObservationTrust.authenticated =>
          DeviceObservationTrust.authenticated,
      };
}

DeviceClass mapRuntimeDeviceClass(frb.DeviceClass deviceClass) =>
    switch (deviceClass) {
      frb.DeviceClass.desktop => DeviceClass.desktop,
      frb.DeviceClass.laptop => DeviceClass.laptop,
      frb.DeviceClass.phone => DeviceClass.phone,
      frb.DeviceClass.tablet => DeviceClass.tablet,
      frb.DeviceClass.unknown => DeviceClass.unknown,
    };

/// Thin seam around generated free functions for deterministic gateway tests.
abstract interface class GeneratedDevicesApi {
  Future<frb.ThisDevice> thisDeviceName();
  Future<List<frb.Peer>> listPeers();
  Future<List<frb.DiscoveredDevice>> listDiscoveredDevices();
  Future<List<frb.DiscoveredDevice>> rescanDevices();
  Future<List<frb.SyncOutcome>> syncDevices({String? pairingId});
  Future<void> unpairDevice({required String pairingId});
  Future<void> revokeDevice({required String pairingId});
  Future<void> setThisDeviceName({required String name});
  Future<frb.PairingCeremony> createPairingCeremony();
  Future<frb.PairingCeremony> joinPairingCeremony({
    required String code,
    required String address,
  });
  Future<frb.PairingCeremony> joinPairingUri({required String uri});
  Future<frb.PairingCeremony> pairingCeremonyStatus({
    required String ceremonyId,
  });
  Future<frb.PairingCeremony> confirmPairingCeremony({
    required String ceremonyId,
    required String verificationCode,
    required bool accept,
  });
  Future<Uint8List> revealPairingQr({required String ceremonyId});
  Future<String> revealPairingSas({required String ceremonyId});
  Future<frb.PairingCeremony> cancelPairingCeremony({
    required String ceremonyId,
  });
  Future<void> disposePairingCeremony({required String ceremonyId});
  Future<BigInt> allocateRuntimeWatch();
  Stream<frb.RuntimeEvent> watchRuntime({required BigInt watchId});
  Future<void> cancelRuntimeWatch({required BigInt watchId});
}

class FrbGeneratedDevicesApi implements GeneratedDevicesApi {
  @override
  Future<BigInt> allocateRuntimeWatch() => frb.allocateRuntimeWatch();

  @override
  Future<frb.PairingCeremony> cancelPairingCeremony({
    required String ceremonyId,
  }) => frb.cancelPairingCeremony(ceremonyId: ceremonyId);

  @override
  Future<void> cancelRuntimeWatch({required BigInt watchId}) =>
      frb.cancelRuntimeWatch(watchId: watchId);

  @override
  Future<frb.PairingCeremony> confirmPairingCeremony({
    required String ceremonyId,
    required String verificationCode,
    required bool accept,
  }) => frb.confirmPairingCeremony(
    ceremonyId: ceremonyId,
    verificationCode: verificationCode,
    accept: accept,
  );

  @override
  Future<frb.PairingCeremony> createPairingCeremony() =>
      frb.createPairingCeremony();

  @override
  Future<frb.PairingCeremony> joinPairingCeremony({
    required String code,
    required String address,
  }) => frb.joinPairingCeremony(code: code, address: address);

  @override
  Future<frb.PairingCeremony> joinPairingUri({required String uri}) =>
      frb.joinPairingUri(uri: uri);

  @override
  Future<void> disposePairingCeremony({required String ceremonyId}) =>
      frb.disposePairingCeremony(ceremonyId: ceremonyId);

  @override
  Future<List<frb.DiscoveredDevice>> listDiscoveredDevices() =>
      frb.listDiscoveredDevices();

  @override
  Future<List<frb.Peer>> listPeers() => frb.listPeers();

  @override
  Future<frb.PairingCeremony> pairingCeremonyStatus({
    required String ceremonyId,
  }) => frb.pairingCeremonyStatus(ceremonyId: ceremonyId);

  @override
  Future<List<frb.DiscoveredDevice>> rescanDevices() => frb.rescanDevices();

  @override
  Future<Uint8List> revealPairingQr({required String ceremonyId}) =>
      frb.revealPairingQr(ceremonyId: ceremonyId);

  @override
  Future<String> revealPairingSas({required String ceremonyId}) =>
      frb.revealPairingSas(ceremonyId: ceremonyId);

  @override
  Future<void> revokeDevice({required String pairingId}) =>
      frb.revokeDevice(pairingId: pairingId);

  @override
  Future<void> setThisDeviceName({required String name}) =>
      frb.setThisDeviceName(name: name);

  @override
  Future<List<frb.SyncOutcome>> syncDevices({String? pairingId}) =>
      frb.syncDevices(pairingId: pairingId);

  @override
  Future<frb.ThisDevice> thisDeviceName() => frb.thisDeviceName();

  @override
  Future<void> unpairDevice({required String pairingId}) =>
      frb.unpairDevice(pairingId: pairingId);

  @override
  Stream<frb.RuntimeEvent> watchRuntime({required BigInt watchId}) =>
      frb.watchRuntime(watchId: watchId);
}

class DevicesSyncException implements Exception {
  const DevicesSyncException(this.message);

  final String message;

  @override
  String toString() => message;
}

class DevicesVerificationCodeRequiredException implements Exception {
  const DevicesVerificationCodeRequiredException();

  @override
  String toString() => 'Reveal and compare the verification code first.';
}

/// Opaque generated ceremony state exposed to the ordinary Devices page.
///
/// The generated API intentionally omits QR and SAS material. Their actions
/// can be completed only after the protected presenter binds this ceremony to
/// a dedicated native host.
class FlutterRustPairingSession implements DevicesPairingSession {
  FlutterRustPairingSession({
    required GeneratedDevicesApi api,
    required frb.PairingCeremony ceremony,
  }) : _api = api,
       _ceremonyId = ceremony.ceremonyId,
       _ceremony = _mapCeremony(ceremony);

  final GeneratedDevicesApi _api;
  final String _ceremonyId;
  final StreamController<PairingCeremony> _updates =
      StreamController<PairingCeremony>.broadcast();
  PairingCeremony _ceremony;
  Timer? _pollTimer;
  bool _polling = false;
  bool _disposed = false;
  String? _verificationCode;

  @override
  PairingCeremony get ceremony => _ceremony;

  @override
  Stream<PairingCeremony> get updates {
    _pollTimer ??= Timer.periodic(
      const Duration(milliseconds: 500),
      (_) => unawaited(_poll()),
    );
    return _updates.stream;
  }

  @override
  Future<Uint8List> revealInviteQr() =>
      _api.revealPairingQr(ceremonyId: _ceremonyId);

  @override
  Future<String> revealSas() async {
    final code = await _api.revealPairingSas(ceremonyId: _ceremonyId);
    _verificationCode = code;
    return code;
  }

  @override
  Future<void> confirm({required bool accept}) async {
    final verificationCode = _verificationCode;
    if (verificationCode == null) {
      throw const DevicesVerificationCodeRequiredException();
    }
    final next = await _api.confirmPairingCeremony(
      ceremonyId: _ceremonyId,
      verificationCode: verificationCode,
      accept: accept,
    );
    _verificationCode = null;
    _setCeremony(next);
  }

  @override
  Future<void> cancel() async {
    if (_disposed || _ceremony.state.isTerminal) return;
    final next = await _api.cancelPairingCeremony(ceremonyId: _ceremonyId);
    _setCeremony(next);
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    _pollTimer?.cancel();
    _pollTimer = null;
    await _updates.close();
    try {
      await _api.disposePairingCeremony(ceremonyId: _ceremonyId);
    } catch (_) {
      if (!_ceremony.state.isTerminal) rethrow;
    }
  }

  void _setCeremony(frb.PairingCeremony next) {
    _ceremony = _mapCeremony(next);
    if (_ceremony.state.isTerminal) {
      _pollTimer?.cancel();
      _pollTimer = null;
    }
    if (!_updates.isClosed) _updates.add(_ceremony);
  }

  Future<void> _poll() async {
    if (_disposed || _polling || _ceremony.state.isTerminal) return;
    _polling = true;
    try {
      final next = await _api.pairingCeremonyStatus(ceremonyId: _ceremonyId);
      if (!_disposed) _setCeremony(next);
    } catch (error, stackTrace) {
      _pollTimer?.cancel();
      _pollTimer = null;
      if (!_updates.isClosed) _updates.addError(error, stackTrace);
    } finally {
      _polling = false;
    }
  }

  static PairingCeremony _mapCeremony(frb.PairingCeremony ceremony) {
    return PairingCeremony(
      state: switch (ceremony.state) {
        'idle' => PairingState.idle,
        'waiting_for_peer' => PairingState.waitingForPeer,
        'handshaking' => PairingState.handshaking,
        'awaiting_confirmation' => PairingState.awaitingConfirmation,
        'confirmed' => PairingState.confirmed,
        'rejected' => PairingState.rejected,
        'cancelled' => PairingState.cancelled,
        'timed_out' => PairingState.timedOut,
        'failed' => PairingState.failed,
        final state => throw FormatException('Unknown pairing state: $state'),
      },
      peerName: ceremony.peerName,
      expiresIn: ceremony.expiresInMs == null
          ? null
          : Duration(milliseconds: ceremony.expiresInMs!.toInt()),
    );
  }
}
