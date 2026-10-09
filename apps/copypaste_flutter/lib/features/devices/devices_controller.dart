import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../../platform/camera/system_pairing_scanner.dart';

import 'devices_gateway.dart';

enum DevicesLoadState { loading, ready, error }

/// Schedules one refresh at a declared observation freshness deadline.
abstract interface class DevicesFreshnessTimer {
  void cancel();
}

typedef DevicesFreshnessTimerFactory =
    DevicesFreshnessTimer Function(Duration delay, VoidCallback callback);

class _SystemDevicesFreshnessTimer implements DevicesFreshnessTimer {
  _SystemDevicesFreshnessTimer(Duration delay, VoidCallback callback)
    : _timer = Timer(delay, callback);

  final Timer _timer;

  @override
  void cancel() => _timer.cancel();
}

/// A device selected for the ordinary device-details surface.
///
/// Pairing remains a separate, capture-protected flow. A selected device never
/// exposes pairing data and is cleared when a pairing inspector is opened.
class DeviceDetailsTarget {
  const DeviceDetailsTarget._({required this.id, required this.isThisDevice});

  const DeviceDetailsTarget.thisDevice()
    : this._(id: 'this-device', isThisDevice: true);

  const DeviceDetailsTarget.peer(String id)
    : this._(id: id, isThisDevice: false);

  final String id;
  final bool isThisDevice;

  @override
  bool operator ==(Object other) =>
      other is DeviceDetailsTarget &&
      other.id == id &&
      other.isThisDevice == isThisDevice;

  @override
  int get hashCode => Object.hash(id, isThisDevice);
}

/// Owns devices data and pairing lifecycle outside presentation widgets.
class DevicesController extends ChangeNotifier {
  DevicesController({
    required DevicesGateway gateway,
    required PairingCaptureProtection captureProtection,
    this.disposeGateway = false,
    DateTime Function()? now,
    DevicesFreshnessTimerFactory? freshnessTimerFactory,
    SystemPairingScanner? systemScanner,
  }) : _gateway = gateway,
       _captureProtection = captureProtection,
       _now = now ?? (() => DateTime.now().toUtc()),
       _systemScanner = systemScanner ?? SystemPairingScanner.forPlatform(),
       _freshnessTimerFactory =
           freshnessTimerFactory ?? _SystemDevicesFreshnessTimer.new;

  final DevicesGateway _gateway;
  final PairingCaptureProtection _captureProtection;
  final DateTime Function() _now;
  final DevicesFreshnessTimerFactory _freshnessTimerFactory;
  final SystemPairingScanner? _systemScanner;
  bool _systemScanInFlight = false;
  String? _systemScanError;
  bool get usesSystemScanner => _systemScanner != null;
  bool get systemScanInFlight => _systemScanInFlight;

  /// Set only when this controller owns the app-wide runtime gateway.
  final bool disposeGateway;
  StreamSubscription<void>? _changesSubscription;
  StreamSubscription<PairingCeremony>? _pairingSubscription;
  DevicesFreshnessTimer? _freshnessTimer;
  DevicesFreshnessTimer? _invitationExpiryTimer;
  Future<void>? _refreshInFlight;
  bool _refreshQueued = false;
  DevicesPairingSession? _pairingSession;
  DevicesSnapshot? _snapshot;
  PairingCeremony? _pairingCeremony;
  PairingEntryMode? _pairingEntryMode;
  bool _pairingInspectorVisible = false;
  DeviceDetailsTarget? _deviceDetailsTarget;
  DevicesLoadState _loadState = DevicesLoadState.loading;
  String? _errorMessage;
  bool _lastErrorWasAction = false;
  bool _actionInFlight = false;
  bool _rescanInFlight = false;
  bool _pairingInFlight = false;
  bool _captureProtectionActive = false;
  bool _captureProtectionInFlight = false;
  bool _decisionInFlight = false;
  bool _pairingModeChangeInFlight = false;
  int _pairingEpoch = 0;
  PairingInvitation? _invitation;
  String? _verificationCode;
  Object? _verificationCodeRequest;
  bool _disposed = false;

  DevicesLoadState get loadState => _loadState;
  DevicesSnapshot? get snapshot => _snapshot;
  List<DiscoveredDevice> get nearbyDevices =>
      _snapshot?.discovered
          .where((device) => !device.paired)
          .toList(growable: false) ??
      const [];
  String? get errorMessage => _systemScanError ?? _errorMessage;
  String get errorTitle => _systemScanError != null
      ? 'Scanner unavailable'
      : _lastErrorWasAction
      ? 'Action failed'
      : 'Device refresh failed';
  PairingCeremony? get pairing => _pairingCeremony;
  PairingEntryMode? get pairingEntryMode => _pairingEntryMode;
  DeviceDetailsTarget? get deviceDetailsTarget => _deviceDetailsTarget;
  bool get decisionInFlight => _decisionInFlight;
  bool get actionInFlight => _actionInFlight;
  bool get rescanInFlight => _rescanInFlight;
  bool get pairingInFlight => _pairingInFlight;
  bool get pairingInspectorOpen =>
      _pairingEntryMode != null && _pairingInspectorVisible;
  bool get deviceDetailsOpen => _deviceDetailsTarget != null;
  bool get canClosePairing => !_decisionInFlight && !_pairingModeChangeInFlight;
  bool get canChangePairingMode =>
      !_disposed &&
      !_decisionInFlight &&
      !_pairingInFlight &&
      !_captureProtectionInFlight &&
      !_pairingModeChangeInFlight &&
      !_systemScanInFlight;
  PairingInvitation? get invitation => _invitation;
  String? get verificationCode => _verificationCode;
  bool get canConfirmPairing => _verificationCode != null;

  /// Display-safe peer connection state. Expired observations fail closed.
  String peerStateLabel(DevicePeer peer) =>
      _presenceLabel(peer.details?.presence, fallbackOnline: peer.online);

  /// Display-safe RTT. A missing or expired observation is never rendered.
  String peerLatencyLabel(DevicePeer peer) => _latencyLabel(peer.details);

  bool isLatencyFresh(DeviceLatency? latency) =>
      latency != null &&
      latency.provenance == DeviceObservationProvenance.measured &&
      latency.trust == DeviceObservationTrust.authenticated &&
      _isFresh(latency.freshUntil);
  Future<void> start() async {
    _changesSubscription ??= _gateway.changes.listen(
      (_) => unawaited(refresh()),
      onError: _setError,
    );
    await refresh();
  }

  Future<void> refresh() async {
    if (_disposed) return;
    final inFlight = _refreshInFlight;
    if (inFlight != null) {
      _refreshQueued = true;
      return inFlight;
    }
    final refresh = _performRefresh();
    _refreshInFlight = refresh;
    return refresh;
  }

  Future<void> _performRefresh() async {
    if (_snapshot == null) {
      _loadState = DevicesLoadState.loading;
      _notify();
    }
    try {
      _snapshot = await _gateway.load();
      _reconcileDeviceDetailsTarget(_snapshot!);
      _loadState = DevicesLoadState.ready;
      _errorMessage = null;
      _lastErrorWasAction = false;
      _scheduleFreshnessRefresh(_snapshot!);
    } catch (error) {
      _loadState = _snapshot == null
          ? DevicesLoadState.error
          : DevicesLoadState.ready;
      _errorMessage = devicesErrorMessage(error);
      _lastErrorWasAction = false;
    } finally {
      _refreshInFlight = null;
      if (_refreshQueued && !_disposed) {
        _refreshQueued = false;
        unawaited(refresh());
      }
    }
    _notify();
  }

  String _presenceLabel(
    DevicePresenceObservation? presence, {
    required bool fallbackOnline,
  }) {
    if (presence == null) return fallbackOnline ? 'Visible now' : 'Not seen';
    if (!_isFresh(presence.freshUntil)) return 'Status unknown';
    return switch (presence.state) {
      DevicePresence.online
          when presence.provenance == DeviceObservationProvenance.measured &&
              presence.trust == DeviceObservationTrust.authenticated =>
        'Available',
      DevicePresence.online => 'Visible now',
      DevicePresence.offline
          when presence.provenance == DeviceObservationProvenance.measured &&
              presence.trust == DeviceObservationTrust.authenticated =>
        'Offline',
      _ => 'Status unknown',
    };
  }

  String _latencyLabel(DeviceDetails? details) {
    final latency = details?.latency;
    if (!isLatencyFresh(latency)) return '— ms';
    final milliseconds = latency!.roundTripLatency.inMilliseconds;
    return milliseconds == 0 ? '<1 ms' : '$milliseconds ms';
  }

  bool _isFresh(DateTime? freshUntil) {
    if (freshUntil == null) return false;
    return _now().isBefore(freshUntil);
  }

  void _scheduleFreshnessRefresh(DevicesSnapshot snapshot) {
    _freshnessTimer?.cancel();
    _freshnessTimer = null;
    final now = _now();
    final deadlines = <DateTime>[
      ..._freshnessDeadlines(snapshot.thisDevice.details),
      for (final peer in snapshot.peers) ..._freshnessDeadlines(peer.details),
      for (final device in snapshot.discovered)
        ..._freshnessDeadlines(device.details),
    ].where((deadline) => deadline.isAfter(now));
    if (deadlines.isEmpty) return;
    final deadline = deadlines.reduce(
      (earliest, candidate) =>
          candidate.isBefore(earliest) ? candidate : earliest,
    );
    _freshnessTimer = _freshnessTimerFactory(deadline.difference(now), () {
      _freshnessTimer = null;
      if (!_disposed) {
        // Expiry only invalidates display state. The Rust monitor renews RTT.
        _scheduleFreshnessRefresh(snapshot);
        _notify();
      }
    });
  }

  Iterable<DateTime> _freshnessDeadlines(DeviceDetails? details) sync* {
    final presenceDeadline = details?.presence?.freshUntil;
    if (presenceDeadline != null) yield presenceDeadline;
    final latencyDeadline = details?.latency?.freshUntil;
    if (latencyDeadline != null) yield latencyDeadline;
  }

  void _reconcileDeviceDetailsTarget(DevicesSnapshot snapshot) {
    final target = _deviceDetailsTarget;
    if (target == null || target.isThisDevice) return;
    if (snapshot.peers.any((peer) => peer.id == target.id)) return;
    _deviceDetailsTarget = null;
  }

  Future<void> rescan() async {
    if (_rescanInFlight || _actionInFlight) return;
    _rescanInFlight = true;
    _notify();
    try {
      await _runAndRefresh(_gateway.rescan);
    } finally {
      _rescanInFlight = false;
      _notify();
    }
  }

  Future<void> sync({String? peerId}) =>
      _runAndRefresh(() => _gateway.sync(peerId: peerId));

  Future<bool> unpair(String peerId) =>
      _removePeer(peerId, () => _gateway.unpair(peerId));

  Future<bool> revoke(String peerId) =>
      _removePeer(peerId, () => _gateway.revoke(peerId));
  Future<void> setThisDeviceName(String name) =>
      _runAndRefresh(() => _gateway.setThisDeviceName(name));

  /// Opens ordinary device details only while pairing is not active.
  ///
  /// An active pairing ceremony owns the secure inspector; replacing it from a
  /// regular device card would silently cancel a security-sensitive flow.
  void openThisDeviceDetails() =>
      _openDeviceDetails(const DeviceDetailsTarget.thisDevice());

  void openPeerDetails(String peerId) =>
      _openDeviceDetails(DeviceDetailsTarget.peer(peerId));

  void closeDeviceDetails() {
    if (_deviceDetailsTarget == null) return;
    _deviceDetailsTarget = null;
    _notify();
  }

  Future<void> openInvitation() async {
    if (!await _openPairingInspector(PairingEntryMode.invite)) return;
    await _startPairing(_gateway.createInvitation);
  }

  Future<void> openQrScanner() async {
    final scanner = _systemScanner;
    if (_systemScanInFlight ||
        !await _openPairingInspector(
          PairingEntryMode.scanQr,
          showInspector: scanner == null,
        )) {
      return;
    }
    if (scanner == null) return;
    final epoch = ++_pairingEpoch;
    _systemScanInFlight = true;
    _notify();
    String? scanError;
    try {
      final uri = await scanner.scan();
      if (_disposed ||
          epoch != _pairingEpoch ||
          _pairingEntryMode != PairingEntryMode.scanQr) {
        return;
      }
      if (uri == null) {
        await closePairing();
      } else {
        _pairingInspectorVisible = true;
        await _startPairing(() => _gateway.joinPairingUri(uri));
      }
    } on PlatformException catch (error) {
      if (!_disposed && epoch == _pairingEpoch) {
        scanError = error.code == 'invalid_pairing_qr'
            ? 'Scan a CopyPaste pairing QR code.'
            : 'Google scanner is unavailable. Enter the pairing code instead.';
      }
    } catch (_) {
      if (!_disposed && epoch == _pairingEpoch) {
        scanError =
            'The scanner could not open. Enter the pairing code instead.';
      }
    } finally {
      _systemScanInFlight = false;
      if (scanError != null && !_disposed && epoch == _pairingEpoch) {
        await closePairing();
        if (!_disposed && _pairingEpoch == epoch + 1) {
          _systemScanError = scanError;
        }
      }
      _notify();
    }
  }

  Future<void> openCodeEntry({String? address}) async {
    await _openPairingInspector(PairingEntryMode.enterCode, address: address);
  }

  String? _pendingAddress;
  String? get pendingAddress => _pendingAddress;

  Future<void> joinFromProtectedInput({
    required String code,
    required String address,
  }) => _startPairing(
    () => _gateway.joinFromProtectedInput(code: code, address: address),
  );

  Future<void> joinPairingUri(String uri) async {
    if (!await _openPairingInspector(PairingEntryMode.scanQr)) return;
    await _startPairing(() => _gateway.joinPairingUri(uri));
  }

  Future<bool> _openPairingInspector(
    PairingEntryMode mode, {
    String? address,
    bool showInspector = true,
  }) async {
    if (!canChangePairingMode) return false;
    if (!_captureProtectionActive) {
      if (_captureProtectionInFlight) return false;
      final epoch = _pairingEpoch;
      _captureProtectionInFlight = true;
      try {
        _captureProtectionActive = await _captureProtection.setEnabled(true);
      } catch (_) {
        _captureProtectionActive = false;
      } finally {
        _captureProtectionInFlight = false;
      }
      if (_disposed || epoch != _pairingEpoch) {
        if (_pairingEntryMode == null) await _disableCaptureProtection();
        return false;
      }
      if (!_captureProtectionActive) {
        _pendingAddress = null;
        _errorMessage = 'Secure pairing presentation is unavailable.';
        _notify();
        return false;
      }
    }
    if (_pairingEntryMode != mode) {
      _invitationExpiryTimer?.cancel();
      _invitationExpiryTimer = null;
      _pairingEpoch++;
      final session = _pairingSession;
      final subscription = _pairingSubscription;
      _pairingSession = null;
      _pairingSubscription = null;
      _pairingCeremony = null;
      _invitation = null;
      _clearVerificationCode();
      if (session != null) {
        _pairingModeChangeInFlight = true;
        _notify();
        try {
          await subscription?.cancel();
          try {
            if (!session.ceremony.state.isTerminal) await session.cancel();
          } finally {
            await session.dispose();
          }
        } catch (error) {
          _setError(error);
          return false;
        } finally {
          _pairingModeChangeInFlight = false;
          _notify();
        }
        if (_disposed) return false;
      }
    }
    _deviceDetailsTarget = null;
    _pairingEntryMode = mode;
    _pairingInspectorVisible = showInspector;
    _pendingAddress = address;
    _systemScanError = null;
    _errorMessage = null;
    _notify();
    return true;
  }

  void _openDeviceDetails(DeviceDetailsTarget target) {
    if (_pairingEntryMode != null || _deviceDetailsTarget == target) return;
    _deviceDetailsTarget = target;
    _notify();
  }

  Future<bool> _removePeer(
    String peerId,
    Future<void> Function() operation,
  ) async {
    final succeeded = await _runAndRefresh(operation);
    if (succeeded && _deviceDetailsTarget == DeviceDetailsTarget.peer(peerId)) {
      _deviceDetailsTarget = null;
      _notify();
    }
    return succeeded;
  }

  Future<void> revealInviteQr() async {
    final session = _pairingSession;
    if (session == null ||
        _pairingCeremony?.state != PairingState.waitingForPeer) {
      return;
    }
    try {
      final qr = await session.revealInvitation();
      if (_disposed ||
          !identical(_pairingSession, session) ||
          _pairingCeremony?.state != PairingState.waitingForPeer) {
        return;
      }
      _invitation = qr;
      _errorMessage = null;
      _notify();
    } catch (error) {
      if (_disposed || !identical(_pairingSession, session)) return;
      _errorMessage = devicesErrorMessage(error);
      _notify();
    }
  }

  Future<void> _revealVerificationCode() async {
    final session = _pairingSession;
    if (session == null ||
        _pairingCeremony?.state != PairingState.awaitingConfirmation ||
        _verificationCodeRequest != null) {
      return;
    }
    final request = _verificationCodeRequest = Object();
    try {
      final code = await session.revealSas();
      if (_disposed ||
          !identical(_pairingSession, session) ||
          !identical(_verificationCodeRequest, request)) {
        return;
      }
      _verificationCode = code;
      _errorMessage = null;
      _notify();
    } catch (error) {
      if (_disposed ||
          !identical(_pairingSession, session) ||
          !identical(_verificationCodeRequest, request)) {
        return;
      }
      _errorMessage = devicesErrorMessage(error);
      _notify();
    }
  }

  void _clearVerificationCode() {
    _verificationCode = null;
    _verificationCodeRequest = null;
  }

  Future<void> confirmPairing({required bool accept}) async {
    final session = _pairingSession;
    if (session == null ||
        _pairingCeremony?.state != PairingState.awaitingConfirmation ||
        _verificationCode == null ||
        _decisionInFlight) {
      return;
    }
    _decisionInFlight = true;
    _notify();
    try {
      await session.confirm(accept: accept);
    } catch (error) {
      _errorMessage = devicesErrorMessage(error);
    } finally {
      _decisionInFlight = false;
      _notify();
    }
  }

  /// Cancels active Rust pairing and clears the local opaque session.
  Future<void> closePairing() async {
    if (!canClosePairing) return;
    _invitationExpiryTimer?.cancel();
    _invitationExpiryTimer = null;
    _systemScanError = null;
    final closingEpoch = ++_pairingEpoch;
    _pairingInspectorVisible = false;
    _pairingInFlight = false;
    if (_systemScanInFlight) {
      unawaited(_systemScanner?.cancel().catchError((Object _) {}));
    }
    final session = _pairingSession;
    if (session == null) {
      _pairingCeremony = null;
      _pairingEntryMode = null;
      _pendingAddress = null;
      _invitation = null;
      _clearVerificationCode();
      _notify();
      await _releaseClosedPairingProtection(closingEpoch);
      return;
    }
    _pairingSession = null;
    _pairingCeremony = null;
    _pairingEntryMode = null;
    _pendingAddress = null;
    _invitation = null;
    _clearVerificationCode();
    await _pairingSubscription?.cancel();
    _pairingSubscription = null;
    _notify();
    try {
      if (!session.ceremony.state.isTerminal) await session.cancel();
    } finally {
      try {
        await session.dispose();
      } finally {
        await _releaseClosedPairingProtection(closingEpoch);
      }
    }
  }

  Future<void> _disableCaptureProtection() async {
    if (!_captureProtectionActive) return;
    _captureProtectionActive = false;
    await _captureProtection.setEnabled(false);
  }

  Future<void> _releaseClosedPairingProtection(int closingEpoch) async {
    if (closingEpoch == _pairingEpoch && _pairingEntryMode == null) {
      await _disableCaptureProtection();
    }
  }

  Future<void> _startPairing(
    Future<DevicesPairingSession> Function() operation,
  ) async {
    if (_pairingSession != null || _pairingInFlight) return;
    final epoch = ++_pairingEpoch;
    _pairingInFlight = true;
    _errorMessage = null;
    _notify();
    try {
      final session = await operation();
      if (_disposed || epoch != _pairingEpoch || _pairingEntryMode == null) {
        await session.cancel();
        await session.dispose();
        return;
      }
      _pairingSession = session;
      _pairingCeremony = session.ceremony;
      _invitation = null;
      _clearVerificationCode();
      _pairingSubscription = session.updates.listen(_onPairingUpdate);
      if (_pairingCeremony?.state == PairingState.awaitingConfirmation) {
        unawaited(_revealVerificationCode());
      }
      if (_pairingEntryMode == PairingEntryMode.invite &&
          _pairingCeremony?.state == PairingState.waitingForPeer) {
        final qr = await session.revealInvitation();
        if (!_disposed &&
            epoch == _pairingEpoch &&
            identical(_pairingSession, session) &&
            _pairingCeremony?.state == PairingState.waitingForPeer) {
          _invitation = qr;
          _scheduleInvitationRenewal(session);
        }
      }
      _notify();
    } catch (error) {
      if (!_disposed && epoch == _pairingEpoch) {
        _errorMessage = devicesErrorMessage(error);
        _notify();
      }
    } finally {
      if (!_disposed && epoch == _pairingEpoch) {
        _pairingInFlight = false;
        _notify();
      }
    }
  }

  void _onPairingUpdate(PairingCeremony ceremony) {
    if (_disposed) return;
    final session = _pairingSession;
    if (session != null &&
        _pairingEntryMode == PairingEntryMode.invite &&
        ceremony.state == PairingState.timedOut) {
      unawaited(_renewInvitation(session));
      return;
    }
    _pairingCeremony = ceremony;
    _invitationExpiryTimer?.cancel();
    _invitationExpiryTimer = null;
    if (session != null && ceremony.state == PairingState.waitingForPeer) {
      _scheduleInvitationRenewal(session);
    }
    if (ceremony.state != PairingState.waitingForPeer) {
      _invitation = null;
    }
    if (ceremony.state != PairingState.awaitingConfirmation) {
      _clearVerificationCode();
    } else {
      unawaited(_revealVerificationCode());
    }
    if (ceremony.state.isTerminal) {
      unawaited(_disposeTerminalPairing());
    }
    _notify();
  }

  void _scheduleInvitationRenewal(DevicesPairingSession session) {
    _invitationExpiryTimer?.cancel();
    _invitationExpiryTimer = null;
    final remaining = _pairingCeremony?.expiresIn;
    if (_pairingEntryMode != PairingEntryMode.invite || remaining == null) {
      return;
    }
    final delay = remaining - const Duration(seconds: 1);
    _invitationExpiryTimer = _freshnessTimerFactory(
      delay.isNegative ? Duration.zero : delay,
      () => unawaited(_renewInvitation(session)),
    );
  }

  Future<void> _renewInvitation(DevicesPairingSession session) async {
    if (_disposed ||
        !identical(_pairingSession, session) ||
        _pairingEntryMode != PairingEntryMode.invite ||
        (session.ceremony.state != PairingState.timedOut &&
            (_pairingCeremony?.state != PairingState.waitingForPeer ||
                session.ceremony.state != PairingState.waitingForPeer))) {
      return;
    }
    final epoch = ++_pairingEpoch;
    _invitationExpiryTimer?.cancel();
    _invitationExpiryTimer = null;
    final subscription = _pairingSubscription;
    _pairingSubscription = null;
    _pairingSession = null;
    _pairingCeremony = null;
    _invitation = null;
    _clearVerificationCode();
    _pairingInFlight = true;
    _notify();
    try {
      await subscription?.cancel();
      try {
        if (!session.ceremony.state.isTerminal) await session.cancel();
      } finally {
        await session.dispose();
      }
      if (_disposed ||
          epoch != _pairingEpoch ||
          _pairingEntryMode != PairingEntryMode.invite) {
        return;
      }
      _pairingInFlight = false;
      await _startPairing(_gateway.createInvitation);
    } catch (error) {
      if (!_disposed && epoch == _pairingEpoch) {
        _errorMessage = devicesErrorMessage(error);
        _pairingInFlight = false;
        _notify();
      }
    }
  }

  Future<void> _disposeTerminalPairing() async {
    final session = _pairingSession;
    if (session == null || !(_pairingCeremony?.state.isTerminal ?? false)) {
      return;
    }
    _pairingSession = null;
    await _pairingSubscription?.cancel();
    _pairingSubscription = null;
    await session.dispose();
    await refresh();
  }

  Future<bool> _runAndRefresh(Future<void> Function() operation) async {
    if (_actionInFlight) return false;
    _actionInFlight = true;
    _notify();
    try {
      await operation();
      await refresh();
      return true;
    } catch (error) {
      _errorMessage = devicesErrorMessage(error);
      _lastErrorWasAction = true;
      return false;
    } finally {
      _actionInFlight = false;
      _notify();
    }
  }

  void _setError(Object error, [StackTrace? _]) {
    _errorMessage = devicesErrorMessage(error);
    _lastErrorWasAction = false;
    _notify();
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    if (_systemScanInFlight) {
      unawaited(_systemScanner?.cancel().catchError((Object _) {}));
    }
    _freshnessTimer?.cancel();
    _freshnessTimer = null;
    _invitationExpiryTimer?.cancel();
    _invitationExpiryTimer = null;
    _refreshQueued = false;
    unawaited(_changesSubscription?.cancel());
    unawaited(_pairingSubscription?.cancel());
    final session = _pairingSession;
    _pairingSession = null;
    if (session != null) {
      unawaited(session.cancel().whenComplete(session.dispose));
    }
    if (_captureProtectionActive) {
      _captureProtectionActive = false;
      unawaited(_captureProtection.setEnabled(false));
    }
    if (disposeGateway && _gateway is DisposableDevicesGateway) {
      unawaited((_gateway as DisposableDevicesGateway).dispose());
    }
    super.dispose();
  }
}
