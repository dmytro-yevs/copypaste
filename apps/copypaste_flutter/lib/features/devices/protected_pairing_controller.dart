import 'dart:async';

import 'package:flutter/foundation.dart';

import 'devices_gateway.dart';
import 'protected_pairing_port.dart';

/// Owns protected-route pairing state and releases all artifacts on close.
class ProtectedPairingController extends ChangeNotifier {
  ProtectedPairingController({
    required ProtectedPairingHost host,
    required ProtectedPairingSession session,
  }) : _host = host,
       _session = session,
       _ceremony = session.ceremony;

  final ProtectedPairingHost _host;
  final ProtectedPairingSession _session;
  late PairingCeremony _ceremony;
  StreamSubscription<PairingCeremony>? _updates;
  ProtectedPairingArtifact? _artifact;
  ProtectedCameraPreview? _cameraPreview;
  bool _decisionInFlight = false;
  String? _errorMessage;
  bool _started = false;
  bool _closed = false;
  bool _disposed = false;

  PairingCeremony get ceremony => _ceremony;
  ProtectedPairingArtifact? get artifact => _artifact;
  ProtectedCameraPreview? get cameraPreview => _cameraPreview;
  bool get isProtectedHostActive => _host.isActive;
  bool get decisionInFlight => _decisionInFlight;
  String? get errorMessage => _errorMessage;

  void start() {
    if (_started || _disposed) return;
    _started = true;
    _updates = _session.updates.listen(
      (ceremony) {
        _ceremony = ceremony;
        if (ceremony.state.isTerminal) _clearMaterial();
        _notify();
      },
      onError: (Object error, StackTrace _) {
        _errorMessage = devicesErrorMessage(error);
        _notify();
      },
    );
    if (_host.isActive && _ceremony.state == PairingState.waitingForPeer) {
      unawaited(revealInvitationQr());
    }
  }

  Future<void> revealInvitationQr() =>
      _reveal((session) => session.revealInvitationQr());
  Future<void> revealSas() => _reveal((session) => session.revealSas());

  Future<void> openCameraScanner() async {
    if (!_host.isActive || _ceremony.state.isTerminal) return;
    try {
      _clearMaterial();
      _cameraPreview = await _session.openCameraScanner();
    } catch (error) {
      _errorMessage = devicesErrorMessage(error);
    }
    _notify();
  }

  Future<void> submitManualJoinCode(String code) async {
    if (!_host.isActive || code.trim().isEmpty || _ceremony.state.isTerminal) {
      return;
    }
    try {
      await _session.submitManualJoinCode(code.trim());
      _errorMessage = null;
    } catch (error) {
      _errorMessage = devicesErrorMessage(error);
    }
    _notify();
  }

  Future<void> confirm({required bool accept}) async {
    if (_ceremony.state != PairingState.awaitingConfirmation ||
        _decisionInFlight) {
      return;
    }
    _decisionInFlight = true;
    _notify();
    try {
      await _session.confirm(accept: accept);
    } catch (error) {
      _errorMessage = devicesErrorMessage(error);
    } finally {
      _decisionInFlight = false;
      _notify();
    }
  }

  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    _clearMaterial();
    await _updates?.cancel();
    _updates = null;
    try {
      if (!_ceremony.state.isTerminal) await _session.cancel();
    } finally {
      await _session.dispose();
      await _host.close();
    }
  }

  Future<void> _reveal(
    Future<ProtectedPairingArtifact> Function(ProtectedPairingSession session)
    operation,
  ) async {
    if (!_host.isActive || _ceremony.state.isTerminal) return;
    try {
      _cameraPreview = null;
      _artifact = await operation(_session);
      _errorMessage = null;
    } catch (error) {
      _errorMessage = devicesErrorMessage(error);
    }
    _notify();
  }

  void _clearMaterial() {
    _artifact = null;
    _cameraPreview = null;
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    unawaited(_updates?.cancel());
    _clearMaterial();
    if (!_closed) {
      unawaited(_session.cancel().whenComplete(_session.dispose));
      unawaited(_host.close());
    }
    super.dispose();
  }
}
