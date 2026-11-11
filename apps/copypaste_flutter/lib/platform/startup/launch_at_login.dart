import 'dart:io';

import 'package:flutter/services.dart';

import '../macos/macos_setup_gateway.dart';
import '../permissions/linux_integration.dart';

/// Effective start-at-login state shared by every platform.
///
/// [requiresApproval] means CopyPaste is registered, but the operating system
/// will not launch it until the user allows it: macOS Login Items approval, or
/// the Windows Task Manager startup toggle.
enum LaunchAtLoginStatus { disabled, enabled, requiresApproval, unavailable }

/// Typed start-at-login adapter; each platform maps its native shape here.
abstract interface class LaunchAtLoginGateway {
  Future<LaunchAtLoginStatus> read();
  Future<LaunchAtLoginStatus> setEnabled(bool enabled);
  Future<void> openSystemSettings();
}

/// Creates the start-at-login adapter for [platform] (defaults to the host).
LaunchAtLoginGateway platformLaunchAtLoginGateway({
  String? platform,
  MethodChannel? channel,
}) => switch (platform ?? Platform.operatingSystem) {
  'macos' => MacosLaunchAtLoginGateway(
    MethodChannelMacosSetupGateway(channel: channel),
  ),
  'linux' => LinuxLaunchAtLoginGateway(
    MethodChannelLinuxIntegrationPort(channel: channel),
  ),
  _ => StartupChannelLaunchAtLoginGateway(channel: channel),
};

/// macOS Login Item registration through `SMAppService`.
class MacosLaunchAtLoginGateway implements LaunchAtLoginGateway {
  const MacosLaunchAtLoginGateway(this._setup);

  final MacosSetupGateway _setup;

  @override
  Future<LaunchAtLoginStatus> read() async =>
      _status(await _setup.loginItemStatus());

  @override
  Future<LaunchAtLoginStatus> setEnabled(bool enabled) async =>
      _status(await _setup.setLaunchAtLogin(enabled));

  @override
  Future<void> openSystemSettings() => _setup.openLoginItemsSettings();

  LaunchAtLoginStatus _status(MacosLoginItemStatus status) => switch (status) {
    MacosLoginItemStatus.enabled => LaunchAtLoginStatus.enabled,
    MacosLoginItemStatus.requiresApproval =>
      LaunchAtLoginStatus.requiresApproval,
    MacosLoginItemStatus.notRegistered ||
    MacosLoginItemStatus.notFound => LaunchAtLoginStatus.disabled,
    MacosLoginItemStatus.developmentUnavailable ||
    MacosLoginItemStatus.unavailable => LaunchAtLoginStatus.unavailable,
  };
}

/// Linux XDG autostart entry owned by the desktop integration channel.
class LinuxLaunchAtLoginGateway implements LaunchAtLoginGateway {
  const LinuxLaunchAtLoginGateway(this._integration);

  final LinuxIntegrationPort _integration;

  @override
  Future<LaunchAtLoginStatus> read() async => await _integration.startAtLogin()
      ? LaunchAtLoginStatus.enabled
      : LaunchAtLoginStatus.disabled;

  @override
  Future<LaunchAtLoginStatus> setEnabled(bool enabled) async {
    if (!await _integration.setStartAtLogin(enabled)) {
      throw PlatformException(code: 'startup_update_failed');
    }
    return read();
  }

  @override
  Future<void> openSystemSettings() async =>
      throw UnsupportedError('Linux has no startup approval settings.');
}

/// Windows Run-key registration and Android boot restoration.
///
/// `getStatus` returns `enabled`, `disabled` or `requires_approval` (Windows
/// Task Manager disabled the registered entry); `openSettings` opens the
/// system startup settings where the platform has them.
class StartupChannelLaunchAtLoginGateway implements LaunchAtLoginGateway {
  StartupChannelLaunchAtLoginGateway({MethodChannel? channel})
    : _channel = channel ?? const MethodChannel('com.copypaste.app/startup');

  final MethodChannel _channel;

  @override
  Future<LaunchAtLoginStatus> read() async =>
      switch (await _channel.invokeMethod<String>('getStatus')) {
        'enabled' => LaunchAtLoginStatus.enabled,
        'disabled' => LaunchAtLoginStatus.disabled,
        'requires_approval' => LaunchAtLoginStatus.requiresApproval,
        _ => throw const FormatException('Invalid startup status.'),
      };

  @override
  Future<LaunchAtLoginStatus> setEnabled(bool enabled) async {
    final applied = await _channel.invokeMethod<bool>('setEnabled', {
      'enabled': enabled,
    });
    if (applied != true) throw PlatformException(code: 'startup_update_failed');
    return read();
  }

  @override
  Future<void> openSystemSettings() =>
      _channel.invokeMethod<void>('openSettings');
}
