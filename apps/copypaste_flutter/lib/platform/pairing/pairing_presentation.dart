import 'package:flutter/services.dart';

import '../../features/devices/devices_gateway.dart';

const _pairingPresentationHostChannel = MethodChannel(
  'com.copypaste.app/pairing_presentation_host',
);

class MethodChannelPairingCaptureProtection
    implements PairingCaptureProtection {
  // Pairing lifecycle requests reconcile the saved screenshot policy. They
  // never override the user's Security setting, including for QR and SAS.
  MethodChannelPairingCaptureProtection({MethodChannel? channel})
    : _channel = channel ?? _pairingPresentationHostChannel;

  final MethodChannel _channel;

  @override
  Future<bool> setEnabled(bool enabled) async {
    try {
      return await _channel.invokeMethod<bool>(
            'setCaptureProtection',
            <String, Object>{'enabled': enabled},
          ) ??
          false;
    } on PlatformException {
      return false;
    } on MissingPluginException {
      return false;
    }
  }
}

/// A non-secret pairing ceremony identity supplied by the Rust-backed session.
class PairingPresentationRequest {
  const PairingPresentationRequest({required this.ceremonyId});

  final String ceremonyId;
}

enum PairingPresentationAvailability { available, unsupported, failed }

/// Creates a separate native window/activity and Flutter engine for pairing
/// artifacts. Neither this API nor its results contain an invite, QR pixels, or
/// SAS value.
abstract interface class PairingPresentationHost {
  Future<PairingPresentationAvailability> availability();

  Future<PairingPresentationLease?> open(PairingPresentationRequest request);
}

abstract interface class PairingPresentationLease {
  String get contextId;

  Future<void> close();
}

class PairingPresentationController {
  PairingPresentationController({required PairingPresentationHost host})
    : _host = host;

  final PairingPresentationHost _host;
  int _generation = 0;

  Future<PairingPresentationLease?> open(
    PairingPresentationRequest request,
  ) async {
    if (request.ceremonyId.isEmpty) {
      return null;
    }
    final generation = ++_generation;
    final lease = await _host.open(request);
    if (lease == null || generation != _generation) {
      await lease?.close();
      return null;
    }
    return _GuardedPairingPresentationLease(lease, generation, this);
  }

  /// Invalidates any late open completion when its ceremony has been cancelled.
  void invalidate() {
    _generation++;
  }
}

class _GuardedPairingPresentationLease implements PairingPresentationLease {
  _GuardedPairingPresentationLease(this._lease, this._generation, this._owner);

  final PairingPresentationLease _lease;
  final int _generation;
  final PairingPresentationController _owner;

  @override
  String get contextId => _lease.contextId;

  @override
  Future<void> close() async {
    if (_generation != _owner._generation) {
      return;
    }
    await _lease.close();
  }
}

/// Native host implementation. The platform creates and protects the separate
/// context before its Flutter engine receives the protected route.
class MethodChannelPairingPresentationHost implements PairingPresentationHost {
  MethodChannelPairingPresentationHost({MethodChannel? channel})
    : _channel = channel ?? _pairingPresentationHostChannel;

  final MethodChannel _channel;

  @override
  Future<PairingPresentationAvailability> availability() async {
    try {
      final supported = await _channel.invokeMethod<bool>('isSupported');
      return supported == true
          ? PairingPresentationAvailability.available
          : PairingPresentationAvailability.unsupported;
    } on PlatformException {
      return PairingPresentationAvailability.failed;
    } on MissingPluginException {
      return PairingPresentationAvailability.unsupported;
    }
  }

  @override
  Future<PairingPresentationLease?> open(
    PairingPresentationRequest request,
  ) async {
    if (await availability() != PairingPresentationAvailability.available) {
      return null;
    }
    try {
      final result = await _channel.invokeMapMethod<String, Object?>(
        'open',
        <String, Object>{'ceremonyId': request.ceremonyId},
      );
      final contextId = result?['contextId'] as String?;
      return contextId == null || contextId.isEmpty
          ? null
          : _MethodChannelPairingPresentationLease(_channel, contextId);
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }
}

class _MethodChannelPairingPresentationLease
    implements PairingPresentationLease {
  _MethodChannelPairingPresentationLease(this._channel, this.contextId);

  final MethodChannel _channel;
  @override
  final String contextId;
  bool _closed = false;

  @override
  Future<void> close() async {
    if (_closed) {
      return;
    }
    _closed = true;
    try {
      await _channel.invokeMethod<void>('close', <String, Object>{
        'contextId': contextId,
      });
    } on PlatformException {
      // Native teardown still owns a context whose engine has exited.
    } on MissingPluginException {
      // The caller cannot retain a usable presentation after engine teardown.
    }
  }
}
