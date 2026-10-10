import 'package:flutter/services.dart';

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
  });

  final LinuxDesktopSession session;
  final bool globalShortcuts;
  final LinuxRemoteDesktopState remoteDesktop;
  final LinuxCompanionState companion;
  final bool clipboard;
  final bool quickPaste;
  final bool screenshotProtection;

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
}

class MethodChannelLinuxIntegrationPort implements LinuxIntegrationPort {
  MethodChannelLinuxIntegrationPort({MethodChannel? channel})
    : _channel =
          channel ?? const MethodChannel('com.copypaste.app/linux_integration');

  final MethodChannel _channel;

  @override
  Future<LinuxIntegrationStatus> status() async {
    final result = await _channel.invokeMapMethod<String, Object?>('status');
    if (result == null) {
      throw const FormatException('Invalid Linux status.');
    }
    return LinuxIntegrationStatus.fromMap(result);
  }

  @override
  Future<bool> requestRemoteDesktop() async =>
      await _channel.invokeMethod<bool>('requestRemoteDesktop') ?? false;

  @override
  Future<bool> openCompanionSetup() async =>
      await _channel.invokeMethod<bool>('openCompanionSetup') ?? false;
}
