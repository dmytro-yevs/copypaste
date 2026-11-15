import 'dart:async';
import 'dart:typed_data';

import '../../shared/error_text.dart';

/// Platform claimed by the device profile.
enum DevicePlatform { macos, windows, android, unknown, linux }

/// Device form factor claimed by the device profile.
enum DeviceClass { desktop, laptop, phone, tablet, unknown }

/// The reachability state observed by the backend.
enum DevicePresence { online, offline, unknown }

/// Display-safe reachability after observation trust and freshness checks.
enum DeviceConnectionState { online, offline, visible, notSeen, unknown }

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
    this.deviceId,
    this.details,
  });

  /// The pairing id.
  final String id;

  /// Known once a session authenticated the peer; keys its sync policy.
  final String? deviceId;
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
enum PairingEntryMode { invite, scanQr, enterCode, nearby }

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
    this.id,
    this.peerName,
    this.expiresIn,
    this.failureMessage,
  });

  final PairingState state;
  final String? id;
  final String? peerName;
  final Duration? expiresIn;
  final String? failureMessage;
}

/// This device's space. Members are the [DevicesSnapshot.peers].
class DeviceSpace {
  const DeviceSpace({
    required this.member,
    this.keyEpoch,
    this.passphraseRecovery = false,
    this.waitingPeerIds = const {},
  });

  /// False after another device removed this one, or after it left.
  final bool member;

  /// The cloud key epoch; null while cloud sync has not set up a key.
  final int? keyEpoch;
  final bool passphraseRecovery;

  /// Peers that have not yet received the current cloud key.
  final Set<String> waitingPeerIds;
}

/// Whether a device syncs, and which content kinds. Rich text is text.
/// [paused] stops automatic sync while a manual send still works.
class DeviceSyncPolicy {
  const DeviceSyncPolicy({
    required this.enabled,
    required this.text,
    required this.images,
    required this.files,
    this.paused = false,
  });

  static const all = DeviceSyncPolicy(
    enabled: true,
    text: true,
    images: true,
    files: true,
  );

  final bool enabled;
  final bool paused;
  final bool text;
  final bool images;
  final bool files;

  DeviceSyncPolicy copyWith({
    bool? enabled,
    bool? paused,
    bool? text,
    bool? images,
    bool? files,
  }) => DeviceSyncPolicy(
    enabled: enabled ?? this.enabled,
    paused: paused ?? this.paused,
    text: text ?? this.text,
    images: images ?? this.images,
    files: files ?? this.files,
  );
}

/// This device's policy and the paired devices that differ from the default.
class DeviceSyncPreferences {
  const DeviceSyncPreferences({
    this.thisDevice = DeviceSyncPolicy.all,
    this.devices = const {},
  });

  final DeviceSyncPolicy thisDevice;
  final Map<String, DeviceSyncPolicy> devices;

  DeviceSyncPolicy forDevice(String deviceId) =>
      devices[deviceId] ?? DeviceSyncPolicy.all;
}

/// A snapshot from the Rust runtime. It contains no pairing secret fields.
class DevicesSnapshot {
  const DevicesSnapshot({
    required this.thisDevice,
    required this.peers,
    required this.discovered,
    this.space,
    this.sync = const DeviceSyncPreferences(),
  });

  final ThisDevice thisDevice;
  final DeviceSyncPreferences sync;

  /// Null on a runtime without spaces.
  final DeviceSpace? space;

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

  /// Sets this device's policy for a null [deviceId], else one peer's.
  Future<void> setSyncPolicy(String? deviceId, DeviceSyncPolicy policy);
  Future<DevicesPairingSession> createInvitation();

  /// Joins the endpoint required by the Rust `PairJoin { code, addr }` contract.
  Future<DevicesPairingSession> joinFromProtectedInput({
    required String code,
    required String address,
  });

  /// Joins one versioned `copypaste://pair` payload decoded by any scanner.
  Future<DevicesPairingSession> joinPairingUri(String uri);
}

/// Space changes beyond pairing: leaving, and the cloud follow-ups of a
/// removal. Optional so a gateway without spaces keeps working.
abstract interface class SpaceGateway {
  Future<void> leaveSpace();
  Future<bool> cloudSignedIn();
  Future<void> changeSyncPassphrase(String passphrase);
  Future<void> signOutOtherDevices();
}

/// Optional discovery pairing boundary; ordinary manual entry remains separate.
abstract interface class NearbyPairingGateway {
  Future<DevicesPairingSession> requestNearbyPairing(String address);
  Future<DevicesPairingSession?> incomingPairing();
}

/// Implemented when a gateway owns a shared runtime subscription.
abstract interface class DisposableDevicesGateway {
  Future<void> dispose();
}

/// The user-facing text of a Devices failure.
String devicesErrorMessage(Object error) => ErrorText.describe(error);
