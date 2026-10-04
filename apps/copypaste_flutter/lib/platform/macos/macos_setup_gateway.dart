import 'package:flutter/services.dart';

enum MacosLoginItemStatus {
  notRegistered,
  enabled,
  requiresApproval,
  developmentUnavailable,
  unavailable,
}

abstract interface class MacosSetupGateway {
  Future<bool> accessibilityGranted();

  Future<bool> requestAccessibility();

  Future<MacosLoginItemStatus> loginItemStatus();

  Future<MacosLoginItemStatus> setLaunchAtLogin(bool enabled);

  Future<void> openLoginItemsSettings();
}

class MethodChannelMacosSetupGateway implements MacosSetupGateway {
  MethodChannelMacosSetupGateway({MethodChannel? channel})
    : _channel =
          channel ?? const MethodChannel('com.copypaste.app/macos_setup');

  final MethodChannel _channel;

  @override
  Future<bool> accessibilityGranted() async {
    return await _channel.invokeMethod<bool>('accessibilityGranted') ?? false;
  }

  @override
  Future<bool> requestAccessibility() async {
    return await _channel.invokeMethod<bool>('requestAccessibility') ?? false;
  }

  @override
  Future<MacosLoginItemStatus> loginItemStatus() async {
    final value = await _channel.invokeMethod<String>('loginItemStatus');
    return _parseStatus(value);
  }

  @override
  Future<MacosLoginItemStatus> setLaunchAtLogin(bool enabled) async {
    final value = await _channel.invokeMethod<String>(
      'setLaunchAtLogin',
      <String, Object>{'enabled': enabled},
    );
    return _parseStatus(value);
  }

  @override
  Future<void> openLoginItemsSettings() {
    return _channel.invokeMethod<void>('openLoginItemsSettings');
  }

  MacosLoginItemStatus _parseStatus(String? value) => switch (value) {
    'enabled' => MacosLoginItemStatus.enabled,
    'requires_approval' => MacosLoginItemStatus.requiresApproval,
    'development_unavailable' => MacosLoginItemStatus.developmentUnavailable,
    'unavailable' || 'not_found' => MacosLoginItemStatus.unavailable,
    _ => MacosLoginItemStatus.notRegistered,
  };
}
