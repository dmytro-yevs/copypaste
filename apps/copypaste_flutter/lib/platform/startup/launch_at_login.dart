import 'dart:io';

import 'package:flutter/services.dart';

enum LaunchAtLoginStatus { disabled, enabled, requiresApproval, unavailable }

abstract interface class LaunchAtLoginGateway {
  Future<LaunchAtLoginStatus> read();
  Future<LaunchAtLoginStatus> setEnabled(bool enabled);
  Future<void> openSystemSettings();
}

class MethodChannelLaunchAtLoginGateway implements LaunchAtLoginGateway {
  MethodChannelLaunchAtLoginGateway({MethodChannel? channel, String? platform})
    : _platform = platform ?? Platform.operatingSystem,
      _channel =
          channel ??
          MethodChannel(switch (platform ?? Platform.operatingSystem) {
            'macos' => 'com.copypaste.app/macos_setup',
            'linux' => 'com.copypaste.app/linux_integration',
            _ => 'com.copypaste.app/startup',
          });

  final String _platform;
  final MethodChannel _channel;

  @override
  Future<LaunchAtLoginStatus> read() async {
    if (_platform == 'macos') {
      return _macosStatus(
        await _channel.invokeMethod<String>('loginItemStatus'),
      );
    }
    final Object? enabled;
    if (_platform == 'linux') {
      final status = await _channel.invokeMapMethod<String, Object?>('status');
      enabled = status?['startAtLogin'];
    } else {
      enabled = await _channel.invokeMethod<bool>('getEnabled');
    }
    if (enabled is! bool) {
      throw const FormatException('Invalid startup status.');
    }
    return enabled ? LaunchAtLoginStatus.enabled : LaunchAtLoginStatus.disabled;
  }

  @override
  Future<LaunchAtLoginStatus> setEnabled(bool enabled) async {
    if (_platform == 'macos') {
      return _macosStatus(
        await _channel.invokeMethod<String>('setLaunchAtLogin', {
          'enabled': enabled,
        }),
      );
    }
    final applied = await _channel.invokeMethod<bool>(
      _platform == 'linux' ? 'setStartAtLogin' : 'setEnabled',
      {'enabled': enabled},
    );
    if (applied != true) throw PlatformException(code: 'startup_update_failed');
    return read();
  }

  @override
  Future<void> openSystemSettings() =>
      _channel.invokeMethod<void>('openLoginItemsSettings');

  LaunchAtLoginStatus _macosStatus(String? status) => switch (status) {
    'enabled' => LaunchAtLoginStatus.enabled,
    'requires_approval' => LaunchAtLoginStatus.requiresApproval,
    'not_registered' || 'not_found' => LaunchAtLoginStatus.disabled,
    _ => LaunchAtLoginStatus.unavailable,
  };
}
