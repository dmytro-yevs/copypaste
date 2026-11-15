import 'dart:io';

import 'package:flutter/services.dart';

class BackgroundSyncStatus {
  const BackgroundSyncStatus({
    this.enabled = false,
    this.running = false,
    this.notificationGranted = false,
  });

  final bool enabled;
  final bool running;
  final bool notificationGranted;

  /// Enabled but not running: notifications were denied or Android stopped it.
  bool get stopped => enabled && !running;
}

/// Keeps sync running while the app is in the background, where the
/// platform needs a persistent service for it.
abstract interface class BackgroundSync {
  bool get supported;
  Future<BackgroundSyncStatus> status();
  Future<BackgroundSyncStatus> setEnabled(bool enabled);
}

class MethodChannelBackgroundSync implements BackgroundSync {
  const MethodChannelBackgroundSync({
    MethodChannel channel = const MethodChannel(
      'com.copypaste.app/android_capture',
    ),
    bool? supported,
  }) : _channel = channel,
       _supported = supported;

  final MethodChannel _channel;
  final bool? _supported;

  @override
  bool get supported => _supported ?? Platform.isAndroid;

  @override
  Future<BackgroundSyncStatus> status() => _invoke('state');

  @override
  Future<BackgroundSyncStatus> setEnabled(bool enabled) async {
    if (enabled && !(await status()).notificationGranted) {
      // The persistent service notification is the user's running indicator.
      final granted = await _invoke('requestNotifications');
      if (!granted.notificationGranted) return granted;
    }
    return _invoke('setSyncKeepAliveEnabled', {'enabled': enabled});
  }

  Future<BackgroundSyncStatus> _invoke(
    String method, [
    Map<String, Object?>? arguments,
  ]) async {
    final raw = await _channel.invokeMapMethod<String, Object?>(
      method,
      arguments,
    );
    if (raw == null) {
      throw PlatformException(code: 'background_sync_unavailable');
    }
    return BackgroundSyncStatus(
      enabled: raw['syncKeepAliveEnabled'] as bool? ?? false,
      running: raw['syncKeepAliveRunning'] as bool? ?? false,
      notificationGranted: raw['notificationGranted'] as bool? ?? false,
    );
  }
}
