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
  Object? _artifactRequest;
  bool _verificationCodeReady = false;
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
  bool get canConfirm =>
      _verificationCodeReady &&
      _ceremony.state == PairingState.awaitingConfirmation;
  String? get errorMessage => _errorMessage;

  void start() {
    if (_started || _disposed) return;
    _started = true;
    _updates = _session.updates.listen(
      (ceremony) {
        if (ceremony.state != _ceremony.state) _clearMaterial();
        _ceremony = ceremony;
        if (ceremony.state.isTerminal) _clearMaterial();
        if (ceremony.state == PairingState.awaitingConfirmation) {
          _revealVerificationCode();
        }
        _notify();
      },
      onError: (Object error, StackTrace _) {
        _errorMessage = devicesErrorMessage(error);
        _notify();
      },
    );
    if (_host.isActive && _ceremony.state == PairingState.waitingForPeer) {
      unawaited(revealInvitationQr());
    } else if (_ceremony.state == PairingState.awaitingConfirmation) {
      _revealVerificationCode();
    }
  }

  Future<void> revealInvitationQr() =>
      _reveal((session) => session.revealInvitationQr());
  void _revealVerificationCode() {
    if (_artifactRequest == null) {
      unawaited(_reveal((session) => session.revealSas()));
    }
  }

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
    if (!canConfirm || _decisionInFlight) {
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
    if (!_host.isActive || _ceremony.state.isTerminal || _closed || _disposed) {
      return;
    }
    final state = _ceremony.state;
    final request = _artifactRequest = Object();
    try {
      _cameraPreview = null;
      final artifact = await operation(_session);
      if (!_canApplyArtifact(request, state)) return;
      _artifact = artifact;
      _verificationCodeReady = state == PairingState.awaitingConfirmation;
      _errorMessage = null;
    } catch (error) {
      if (!_canApplyArtifact(request, state)) return;
      _errorMessage = devicesErrorMessage(error);
    }
    _notify();
  }

  bool _canApplyArtifact(Object request, PairingState state) =>
      !_disposed &&
      !_closed &&
      _host.isActive &&
      _ceremony.state == state &&
      identical(_artifactRequest, request);

  void _clearMaterial() {
    _artifact = null;
    _artifactRequest = null;
    _verificationCodeReady = false;
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
