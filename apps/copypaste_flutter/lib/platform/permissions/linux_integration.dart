import 'package:flutter/services.dart';

import '../../generated/api.dart' as api;

enum LinuxDesktopSession { x11, wayland, unsupported }

enum LinuxRemoteDesktopState { unsupported, consentRequired, active }

enum LinuxCompanionState { unavailable, disabled, active }

class LinuxIntegrationStatus {
  const LinuxIntegrationStatus({
    required this.session,
    required this.globalShortcuts,
    required this.remoteDesktop,
    required this.companion,
    required this.clipboard,
    required this.quickPaste,
    required this.screenshotProtection,
    this.startAtLogin = false,
    this.uriRegistered = false,
  });

  final LinuxDesktopSession session;
  final bool globalShortcuts;
  final LinuxRemoteDesktopState remoteDesktop;
  final LinuxCompanionState companion;
  final bool clipboard;
  final bool quickPaste;
  final bool screenshotProtection;
  final bool startAtLogin;
  final bool uriRegistered;

  factory LinuxIntegrationStatus.fromMap(Map<String, Object?> values) {
    return LinuxIntegrationStatus(
      session: _enumValue(LinuxDesktopSession.values, values['session']),
      globalShortcuts: _boolValue(values, 'globalShortcuts'),
      remoteDesktop: _enumValue(
        LinuxRemoteDesktopState.values,
        values['remoteDesktop'],
      ),
      companion: _enumValue(LinuxCompanionState.values, values['companion']),
      clipboard: _boolValue(values, 'clipboard'),
      quickPaste: _boolValue(values, 'quickPaste'),
      screenshotProtection: _boolValue(values, 'screenshotProtection'),
      startAtLogin: _boolValue(values, 'startAtLogin'),
      uriRegistered: _boolValue(values, 'uriRegistered'),
    );
  }

  static T _enumValue<T extends Enum>(List<T> values, Object? value) {
    if (value is! String) throw const FormatException('Invalid Linux status.');
    return values.firstWhere(
      (entry) => entry.name == value,
      orElse: () => throw const FormatException('Invalid Linux status.'),
    );
  }

  static bool _boolValue(Map<String, Object?> values, String key) {
    final value = values[key];
    if (value is! bool) throw const FormatException('Invalid Linux status.');
    return value;
  }
}

abstract interface class LinuxIntegrationPort {
  Future<LinuxIntegrationStatus> status();

  Future<bool> requestRemoteDesktop();

  Future<bool> openCompanionSetup();

  Future<bool> setStartAtLogin(bool enabled);

  Future<bool> registerCopypasteUri();
}

class MethodChannelLinuxIntegrationPort implements LinuxIntegrationPort {
  MethodChannelLinuxIntegrationPort({
    MethodChannel? channel,
    Future<bool> Function()? clipboardAvailable,
  }) : _channel =
           channel ??
           const MethodChannel('com.copypaste.app/linux_integration'),
       _clipboardAvailable = clipboardAvailable ?? api.clipboardAvailable;

  final MethodChannel _channel;
  final Future<bool> Function() _clipboardAvailable;

  Future<void> publishRuntimeEndpoint(String endpoint) async {
    if (!_isRuntimeEndpoint(endpoint)) {
      throw ArgumentError('Runtime endpoint must be an absolute path.');
    }
    if (await _channel.invokeMethod<bool>('publishRuntimeEndpoint', {
          'endpoint': endpoint,
        }) !=
        true) {
      throw PlatformException(code: 'runtime_endpoint_unavailable');
    }
  }

  @override
  Future<LinuxIntegrationStatus> status() async {
    final result = await _channel.invokeMapMethod<String, Object?>('status');
    if (result == null) {
      throw const FormatException('Invalid Linux status.');
    }
    final nativeStatus = LinuxIntegrationStatus.fromMap(result);
    final daemonClipboardAvailable = await _daemonClipboardAvailable();
    final clipboard = nativeStatus.clipboard && daemonClipboardAvailable;
    return LinuxIntegrationStatus(
      session: nativeStatus.session,
      globalShortcuts: nativeStatus.globalShortcuts,
      remoteDesktop: nativeStatus.remoteDesktop,
      companion: nativeStatus.companion,
      clipboard: clipboard,
      quickPaste: nativeStatus.quickPaste && clipboard,
      screenshotProtection: nativeStatus.screenshotProtection,
      startAtLogin: nativeStatus.startAtLogin,
      uriRegistered: nativeStatus.uriRegistered,
    );
  }

  Future<bool> _daemonClipboardAvailable() async {
    try {
      return await _clipboardAvailable();
    } catch (_) {
      return false;
    }
  }

  static bool _isRuntimeEndpoint(String value) =>
      value.isNotEmpty && value.startsWith('/') && !value.contains('\u0000');

  @override
  Future<bool> requestRemoteDesktop() async =>
      await _channel.invokeMethod<bool>('requestRemoteDesktop') ?? false;

  @override
  Future<bool> openCompanionSetup() async =>
      await _channel.invokeMethod<bool>('openCompanionSetup') ?? false;

  @override
  Future<bool> setStartAtLogin(bool enabled) async =>
      await _channel.invokeMethod<bool>('setStartAtLogin', <String, Object>{
        'enabled': enabled,
      }) ??
      false;

  @override
  Future<bool> registerCopypasteUri() async =>
      await _channel.invokeMethod<bool>('registerCopypasteUri') ?? false;
}
