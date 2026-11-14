import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

enum DeviceAuthAvailability {
  available,

  /// The system supports it, but no password, PIN or biometrics is set up.
  notConfigured,

  /// The system has no device authentication; callers confirm in the app.
  unsupported,
}

enum DeviceAuthResult { success, cancelled, unavailable, lockedOut }

/// Why the identity prompt is shown; see [deviceAuthReason].
enum DeviceAuthPurpose { unlockSecrets, useConcealedSnippet, copySecret }

/// Verifies the device owner with Touch ID or the account password on macOS,
/// Windows Hello on Windows, and biometrics or the screen lock on Android.
///
/// Every call prompts; callers own any unlock window that follows a success.
abstract interface class DeviceAuthenticator {
  Future<DeviceAuthAvailability> availability();
  Future<DeviceAuthResult> authenticate({required String reason});
}

/// The localized prompt reason for [purpose] in the form [platform] expects.
String deviceAuthReason(
  AppLocalizations l10n,
  DeviceAuthPurpose purpose, {
  TargetPlatform? platform,
}) {
  // macOS embeds the reason in "<App> is trying to <reason>."
  final form = (platform ?? defaultTargetPlatform) == TargetPlatform.macOS
      ? 'macos'
      : 'other';
  return switch (purpose) {
    DeviceAuthPurpose.unlockSecrets => l10n.deviceAuthReasonUnlockSecrets(form),
    DeviceAuthPurpose.useConcealedSnippet =>
      l10n.deviceAuthReasonUseConcealedSnippet(form),
    DeviceAuthPurpose.copySecret => l10n.deviceAuthReasonCopySecret(form),
  };
}

class MethodChannelDeviceAuthenticator implements DeviceAuthenticator {
  MethodChannelDeviceAuthenticator({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/device_auth',
    ),
  }) : _channel = channel;

  final MethodChannel _channel;
  Future<DeviceAuthResult>? _inFlight;

  bool get _supported =>
      !kIsWeb &&
      switch (defaultTargetPlatform) {
        TargetPlatform.android ||
        TargetPlatform.macOS ||
        TargetPlatform.windows => true,
        _ => false,
      };

  @override
  Future<DeviceAuthAvailability> availability() async {
    if (!_supported) return DeviceAuthAvailability.unsupported;
    try {
      return switch (await _channel.invokeMethod<String>('availability')) {
        'available' => DeviceAuthAvailability.available,
        'notConfigured' => DeviceAuthAvailability.notConfigured,
        _ => DeviceAuthAvailability.unsupported,
      };
    } on MissingPluginException {
      return DeviceAuthAvailability.unsupported;
    } on PlatformException {
      return DeviceAuthAvailability.unsupported;
    }
  }

  /// Concurrent requests share the open prompt instead of stacking prompts.
  @override
  Future<DeviceAuthResult> authenticate({required String reason}) {
    if (reason.trim().isEmpty) {
      throw ArgumentError.value(reason, 'reason', 'must not be empty');
    }
    if (!_supported) return Future.value(DeviceAuthResult.unavailable);
    return _inFlight ??= _authenticate(
      reason,
    ).whenComplete(() => _inFlight = null);
  }

  Future<DeviceAuthResult> _authenticate(String reason) async {
    try {
      final outcome = await _channel.invokeMethod<String>('authenticate', {
        'reason': reason,
      });
      return switch (outcome) {
        'success' => DeviceAuthResult.success,
        'unavailable' => DeviceAuthResult.unavailable,
        'lockedOut' => DeviceAuthResult.lockedOut,
        _ => DeviceAuthResult.cancelled,
      };
    } on MissingPluginException {
      return DeviceAuthResult.unavailable;
    } on PlatformException {
      return DeviceAuthResult.cancelled;
    }
  }
}
