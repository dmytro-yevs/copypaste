import '../../../platform/security/device_authenticator.dart';

enum ConcealedAccess {
  granted,
  denied,

  /// The device cannot verify its owner; the caller confirms in the app and
  /// then calls [ConcealedSnippetAccess.grant].
  confirmInApp,
}

/// The per-device unlock window for concealed snippets while
/// `snippets.require_auth` is on. A success unlocks for [window] or until
/// [lock], which the owning surface calls when it hides.
class ConcealedSnippetAccess {
  ConcealedSnippetAccess({
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
  Future<ConcealedAccess> request(String reason) async {
    if (unlocked) return ConcealedAccess.granted;
    final availability = await _authenticator.availability();
    if (availability != DeviceAuthAvailability.available) {
      return ConcealedAccess.confirmInApp;
    }
    final result = await _authenticator.authenticate(reason: reason);
    if (result != DeviceAuthResult.success) return ConcealedAccess.denied;
    grant();
    return ConcealedAccess.granted;
  }

  void grant() => _unlockedUntil = _clock().add(window);

  void lock() => _unlockedUntil = null;
}
