import 'dart:async';
import 'dart:typed_data';

/// Platform claimed by the device profile.
enum DevicePlatform { macos, windows, android, unknown }

/// Device form factor claimed by the device profile.
enum DeviceClass { desktop, laptop, phone, tablet, unknown }

/// The reachability state observed by the backend.
enum DevicePresence { online, offline, unknown }

/// How the backend learned an observation value.
enum DeviceObservationProvenance { selfReported, observed, measured }

/// How strongly the backend trusts an observation value.
enum DeviceObservationTrust { local, unverified, authenticated }

/// A display-safe device profile with its source and freshness metadata.
class DeviceProfile {
  const DeviceProfile({
    required this.displayName,
    this.appVersion,
    this.protocolVersion,
    required this.platform,
    required this.deviceClass,
    this.osName,
    this.osVersion,
    this.model,
    required this.provenance,
    required this.trust,
    required this.observedAt,
    this.freshUntil,
  });

  final String displayName;
  final String? appVersion;
  final int? protocolVersion;
  final DevicePlatform platform;
  final DeviceClass deviceClass;
  final String? osName;
  final String? osVersion;
  final String? model;
  final DeviceObservationProvenance provenance;
  final DeviceObservationTrust trust;
  final DateTime observedAt;
  final DateTime? freshUntil;
}

/// A LAN endpoint with the source and freshness of the observation.
class DeviceEndpoint {
  const DeviceEndpoint({
    required this.lanEndpoint,
    required this.provenance,
    required this.trust,
    required this.observedAt,
    this.freshUntil,
  });

  final String lanEndpoint;
  final DeviceObservationProvenance provenance;
  final DeviceObservationTrust trust;
  final DateTime observedAt;
  final DateTime? freshUntil;
}

/// Authenticated Probe-to-ProbeAck latency. A missing value means it has not
/// been measured.
class DeviceLatency {
  const DeviceLatency({
    required this.roundTripLatency,
    required this.provenance,
    required this.trust,
    required this.observedAt,
    this.freshUntil,
  });

  final Duration roundTripLatency;
  final DeviceObservationProvenance provenance;
  final DeviceObservationTrust trust;
  final DateTime observedAt;
  final DateTime? freshUntil;
}

/// Backend reachability observation. It is separate from pairing trust.
class DevicePresenceObservation {
  const DevicePresenceObservation({
    required this.state,
    required this.lastSeen,
    required this.provenance,
    required this.trust,
    required this.observedAt,
    this.freshUntil,
  });

  final DevicePresence state;
  final DateTime lastSeen;
  final DeviceObservationProvenance provenance;
  final DeviceObservationTrust trust;
  final DateTime observedAt;
  final DateTime? freshUntil;
}

/// All non-secret observations associated with a device.
class DeviceDetails {
  const DeviceDetails({
    this.profile,
    this.endpoint,
    this.latency,
    this.presence,
  });

  final DeviceProfile? profile;
  final DeviceEndpoint? endpoint;
  final DeviceLatency? latency;
  final DevicePresenceObservation? presence;
}

/// Stable local device display data. [id] can be null only for an older
/// backend; it is never a pairing identifier.
class ThisDevice {
  const ThisDevice({
    this.id,
    required this.name,
    required this.appVersion,
    required this.protocolVersion,
    this.listenAddress,
    this.details,
  });

  final String? id;
  final String name;
  final String appVersion;
  final int protocolVersion;
  final String? listenAddress;
  final DeviceDetails? details;
}

/// A trusted peer stored by the Rust pairing service.
class DevicePeer {
  const DevicePeer({
    required this.id,
    required this.name,
    required this.lastSeen,
    required this.online,
    this.details,
  });

  final String id;
  final String name;
  final DateTime lastSeen;

  /// A discovery observation. False means not seen, not unreachable.
  final bool online;
  final DeviceDetails? details;
}

/// An unauthenticated device announced by local discovery.
class DiscoveredDevice {
  const DiscoveredDevice({
    required this.id,
    required this.name,
    required this.address,
    required this.paired,
    required this.lastSeen,
    this.details,
  });

  final String id;
  final String name;
  final String address;
  final bool paired;
  final DateTime lastSeen;
  final DeviceDetails? details;
}

/// The entry point selected for the active pairing inspector.
enum PairingEntryMode { invite, scanQr, enterCode }

/// The Rust-owned pairing lifecycle represented without pairing material.
enum PairingState {
  idle,
  waitingForPeer,
  handshaking,
  awaitingConfirmation,
  confirmed,
  rejected,
  cancelled,
  timedOut,
  failed,
}

extension PairingStateX on PairingState {
  bool get isTerminal => switch (this) {
    PairingState.confirmed ||
    PairingState.rejected ||
    PairingState.cancelled ||
    PairingState.timedOut ||
    PairingState.failed => true,
    _ => false,
  };
}

/// Safe pairing metadata that Flutter can render outside protected surfaces.
class PairingCeremony {
  const PairingCeremony({
    required this.state,
    this.peerName,
    this.expiresIn,
    this.failureMessage,
  });

  final PairingState state;
  final String? peerName;
  final Duration? expiresIn;
  final String? failureMessage;
}

/// A snapshot from the Rust runtime. It contains no pairing secret fields.
class DevicesSnapshot {
  const DevicesSnapshot({
    required this.thisDevice,
    required this.peers,
    required this.discovered,
  });

  final ThisDevice thisDevice;

  /// Compatibility accessor for existing callers during Devices UI migration.
  String get thisDeviceName => thisDevice.name;
  final List<DevicePeer> peers;
  final List<DiscoveredDevice> discovered;
}

/// Invitation material displayed only during an active pairing invitation.
class PairingInvitation {
  const PairingInvitation({
    required this.qrPng,
    required this.code,
    this.address,
  });

  final Uint8List qrPng;
  final String code;
  final String? address;
}

/// Pairing work that exposes invitation material only to the active inspector.
abstract interface class DevicesPairingSession {
  PairingCeremony get ceremony;
  Stream<PairingCeremony> get updates;

  /// Returns the matching QR, code, and address for the current invitation.
  Future<PairingInvitation> revealInvitation();

  /// Opens the native protected SAS presentation after an explicit user request.
  Future<String> revealSas();

  /// Rust accepts or rejects only while it permits a bilateral decision.
  Future<void> confirm({required bool accept});
  Future<void> cancel();
  Future<void> dispose();
}

/// Protects the application surface before pairing material enters Flutter UI.
abstract interface class PairingCaptureProtection {
  Future<bool> setEnabled(bool enabled);
}

/// Typed feature boundary implemented by the generated Rust runtime adapter.
///
/// Join methods accept the backend-required code and endpoint only while the
/// caller owns a capture-protected pairing surface. They never return secrets.
abstract interface class DevicesGateway {
  Stream<void> get changes;

  Future<DevicesSnapshot> load();
  Future<void> rescan();
  Future<void> sync({String? peerId});
  Future<void> unpair(String peerId);
  Future<void> revoke(String peerId);
  Future<void> setThisDeviceName(String name);
  Future<DevicesPairingSession> createInvitation();

  /// Joins the endpoint required by the Rust `PairJoin { code, addr }` contract.
  Future<DevicesPairingSession> joinFromProtectedInput({
    required String code,
    required String address,
  });

  /// Joins one versioned `copypaste://pair` payload decoded by any scanner.
  Future<DevicesPairingSession> joinPairingUri(String uri);
}

/// Implemented when a gateway owns a shared runtime subscription.
abstract interface class DisposableDevicesGateway {
  Future<void> dispose();
}

/// Converts a terminal stream error into a message suitable for [StateView].
String devicesErrorMessage(Object error) => error.toString();
