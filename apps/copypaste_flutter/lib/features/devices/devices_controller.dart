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
  /// Covers the backend's two possible five-second probe attempts.
  static const _latencyRefreshLead = Duration(seconds: 10);

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
  bool get usesSystemScanner => _systemScanner != null;
  bool get systemScanInFlight => _systemScanInFlight;

  /// Set only when this controller owns the app-wide runtime gateway.
  final bool disposeGateway;
  StreamSubscription<void>? _changesSubscription;
  StreamSubscription<PairingCeremony>? _pairingSubscription;
  DevicesFreshnessTimer? _freshnessTimer;
  Future<void>? _refreshInFlight;
  bool _refreshQueued = false;
  DevicesPairingSession? _pairingSession;
  DevicesSnapshot? _snapshot;
  PairingCeremony? _pairingCeremony;
  PairingEntryMode? _pairingEntryMode;
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
  int _pairingEpoch = 0;
  Uint8List? _inviteQrPng;
  String? _verificationCode;
  bool _disposed = false;

  DevicesLoadState get loadState => _loadState;
  DevicesSnapshot? get snapshot => _snapshot;
  String? get errorMessage => _errorMessage;
  String get errorTitle =>
      _lastErrorWasAction ? 'Action failed' : 'Device refresh failed';
  PairingCeremony? get pairing => _pairingCeremony;
  PairingEntryMode? get pairingEntryMode => _pairingEntryMode;
  DeviceDetailsTarget? get deviceDetailsTarget => _deviceDetailsTarget;
  bool get decisionInFlight => _decisionInFlight;
  bool get actionInFlight => _actionInFlight;
  bool get rescanInFlight => _rescanInFlight;
  bool get pairingInFlight => _pairingInFlight;
  bool get pairingInspectorOpen => _pairingEntryMode != null;
  bool get deviceDetailsOpen => _deviceDetailsTarget != null;
  bool get canClosePairing => !_decisionInFlight;
  Uint8List? get inviteQrPng => _inviteQrPng;
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
    return '${latency!.roundTripLatency.inMilliseconds} ms';
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
      ..._freshnessDeadlines(snapshot.thisDevice.details, now),
      for (final peer in snapshot.peers)
        ..._freshnessDeadlines(peer.details, now),
      for (final device in snapshot.discovered)
        ..._freshnessDeadlines(device.details, now),
    ].where((deadline) => deadline.isAfter(now));
    if (deadlines.isEmpty) return;
    final deadline = deadlines.reduce(
      (earliest, candidate) =>
          candidate.isBefore(earliest) ? candidate : earliest,
    );
    _freshnessTimer = _freshnessTimerFactory(deadline.difference(now), () {
      _freshnessTimer = null;
      if (!_disposed) unawaited(refresh());
    });
  }

  Iterable<DateTime> _freshnessDeadlines(
    DeviceDetails? details,
    DateTime now,
  ) sync* {
    final presenceDeadline = details?.presence?.freshUntil;
    if (presenceDeadline != null) yield presenceDeadline;
    final latencyDeadline = details?.latency?.freshUntil;
    if (latencyDeadline != null) {
      final remaining = latencyDeadline.difference(now);
      yield remaining.compareTo(_latencyRefreshLead) > 0
          ? latencyDeadline.subtract(_latencyRefreshLead)
          : latencyDeadline;
    }
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
    if (_systemScanInFlight ||
        !await _openPairingInspector(PairingEntryMode.scanQr)) {
      return;
    }
    final scanner = _systemScanner;
    if (scanner == null) return;
    final epoch = ++_pairingEpoch;
    _systemScanInFlight = true;
    _notify();
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
        await _startPairing(() => _gateway.joinPairingUri(uri));
      }
    } on PlatformException catch (error) {
      if (!_disposed && epoch == _pairingEpoch) {
        _errorMessage = error.code == 'invalid_pairing_qr'
            ? 'Scan a CopyPaste pairing QR code.'
            : 'Google scanner is unavailable. Enter the pairing code instead.';
      }
    } catch (_) {
      if (!_disposed && epoch == _pairingEpoch) {
        _errorMessage =
            'The scanner could not open. Enter the pairing code instead.';
      }
    } finally {
      _systemScanInFlight = false;
      _notify();
    }
  }

  Future<void> openCodeEntry({String? address}) async {
    _pairingEpoch++;
    _pendingAddress = address;
    await _openPairingInspector(PairingEntryMode.enterCode);
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

  Future<bool> _openPairingInspector(PairingEntryMode mode) async {
    if (_disposed) return false;
    if ((_pairingSession != null || _pairingInFlight) &&
        _pairingEntryMode != mode) {
      return false;
    }
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
    _deviceDetailsTarget = null;
    _pairingEntryMode = mode;
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
    if (session == null || (_pairingCeremony?.state.isTerminal ?? true)) return;
    try {
      _inviteQrPng = await session.revealInviteQr();
      _errorMessage = null;
      _notify();
    } catch (error) {
      _errorMessage = devicesErrorMessage(error);
      _notify();
    }
  }

  Future<void> revealSas() async {
    final session = _pairingSession;
    if (session == null || (_pairingCeremony?.state.isTerminal ?? true)) return;
    try {
      _verificationCode = await session.revealSas();
      _errorMessage = null;
      _notify();
    } catch (error) {
      _errorMessage = devicesErrorMessage(error);
      _notify();
    }
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
    if (_decisionInFlight) return;
    final closingEpoch = ++_pairingEpoch;
    _pairingInFlight = false;
    if (_systemScanInFlight) {
      unawaited(_systemScanner?.cancel().catchError((Object _) {}));
    }
    final session = _pairingSession;
    if (session == null) {
      _pairingCeremony = null;
      _pairingEntryMode = null;
      _pendingAddress = null;
      _inviteQrPng = null;
      _verificationCode = null;
      _notify();
      await _releaseClosedPairingProtection(closingEpoch);
      return;
    }
    _pairingSession = null;
    _pairingCeremony = null;
    _pairingEntryMode = null;
    _pendingAddress = null;
    _inviteQrPng = null;
    _verificationCode = null;
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
      _inviteQrPng = null;
      _verificationCode = null;
      _pairingSubscription = session.updates.listen(_onPairingUpdate);
      if (_pairingEntryMode == PairingEntryMode.invite &&
          _pairingCeremony?.state == PairingState.waitingForPeer) {
        final qr = await session.revealInviteQr();
        if (!_disposed &&
            epoch == _pairingEpoch &&
            identical(_pairingSession, session)) {
          _inviteQrPng = qr;
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
    _pairingCeremony = ceremony;
    if (ceremony.state != PairingState.waitingForPeer) {
      _inviteQrPng = null;
    }
    if (ceremony.state != PairingState.awaitingConfirmation) {
      _verificationCode = null;
    }
    if (ceremony.state.isTerminal) {
      unawaited(_disposeTerminalPairing());
    }
    _notify();
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
