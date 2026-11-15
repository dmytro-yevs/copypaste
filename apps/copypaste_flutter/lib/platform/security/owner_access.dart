import 'device_authenticator.dart';

enum OwnerAccessResult {
  granted,
  denied,

  /// The device cannot verify its owner; the caller confirms in the app and
  /// then calls [OwnerAccess.grant].
  confirmInApp,
}

/// A per-surface unlock window for secret items. A success unlocks for
/// [window] or until [lock], which the owning surface calls when it hides.
class OwnerAccess {
  OwnerAccess({
    required DeviceAuthenticator authenticator,
    DateTime Function()? clock,
  }) : _authenticator = authenticator,
       _clock = clock ?? DateTime.now;

  static const window = Duration(seconds: 60);

  final DeviceAuthenticator _authenticator;
  final DateTime Function() _clock;
  DateTime? _unlockedUntil;

  bool get unlocked {
    final until = _unlockedUntil;
    return until != null && _clock().isBefore(until);
  }

  /// Verifies the device owner unless the window is still open.
  Future<OwnerAccessResult> request(String reason) async {
    if (unlocked) return OwnerAccessResult.granted;
    final availability = await _authenticator.availability();
    if (availability != DeviceAuthAvailability.available) {
      return OwnerAccessResult.confirmInApp;
    }
    final result = await _authenticator.authenticate(reason: reason);
    if (result != DeviceAuthResult.success) return OwnerAccessResult.denied;
    grant();
    return OwnerAccessResult.granted;
  }

  void grant() => _unlockedUntil = _clock().add(window);

  void lock() => _unlockedUntil = null;
}
