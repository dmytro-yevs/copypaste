import 'dart:convert';

import 'package:flutter/services.dart';

import '../../generated/api.dart' as api;

enum LinuxDesktopSession { x11, wayland, unsupported }

enum LinuxRemoteDesktopState { unsupported, consentRequired, active }

enum LinuxCompanionState { unavailable, disabled, active }

enum LinuxCapabilityKind {
  clipboardCapture,
  clipboardReadWrite,
  sourceAttribution,
  privacyEnforcement,
  globalShortcuts,
  quickPaste,
  secureStorage,
  desktopIntegration,
  updates,
}

enum LinuxCapabilityState {
  available,
  needsPermission,
  needsCompanion,
  unsupported,
  error,
}

enum LinuxCapabilityGuarantee { none, bestEffort, verified }

class LinuxCapabilityStatus {
  const LinuxCapabilityStatus({
    required this.state,
    required this.backend,
    required this.guarantee,
    this.reason,
    this.permissions = const [],
    this.mimeTypes = const [],
    this.maxBytes,
  });

  final LinuxCapabilityState state;
  final String? backend;
  final LinuxCapabilityGuarantee guarantee;
  final String? reason;
  final List<String> permissions;
  final List<String> mimeTypes;
  final int? maxBytes;

  factory LinuxCapabilityStatus.fromMap(Map<String, Object?> values) {
    final permissions = values['permissions'];
    if (permissions is! List || permissions.any((value) => value is! String)) {
      throw const FormatException('Invalid Linux capability permissions.');
    }
    final mimeTypes = values['mime_types'] ?? const <String>[];
    if (mimeTypes is! List || mimeTypes.any((value) => value is! String)) {
      throw const FormatException('Invalid Linux clipboard formats.');
    }
    final maxBytes = values['max_bytes'];
    if (maxBytes != null && (maxBytes is! int || maxBytes < 0)) {
      throw const FormatException('Invalid Linux clipboard size limit.');
    }
    final backend = values['backend'];
    final reason = values['reason'];
    if (backend != null && backend is! String ||
        reason != null && reason is! String) {
      throw const FormatException('Invalid Linux capability details.');
    }
    final state = switch (values['state']) {
      'available' => LinuxCapabilityState.available,
      'needs_permission' => LinuxCapabilityState.needsPermission,
      'needs_companion' => LinuxCapabilityState.needsCompanion,
      'unsupported' => LinuxCapabilityState.unsupported,
      'error' => LinuxCapabilityState.error,
      _ => throw const FormatException('Invalid Linux capability state.'),
    };
    final guarantee = switch (values['guarantee']) {
      'none' => LinuxCapabilityGuarantee.none,
      'best_effort' => LinuxCapabilityGuarantee.bestEffort,
      'verified' => LinuxCapabilityGuarantee.verified,
      _ => throw const FormatException('Invalid Linux capability guarantee.'),
    };
    if (state == LinuxCapabilityState.available &&
        (backend == null ||
            (backend as String).isEmpty ||
            guarantee == LinuxCapabilityGuarantee.none)) {
      throw const FormatException(
        'Available capability lacks a backend guarantee.',
      );
    }
    return LinuxCapabilityStatus(
      state: state,
      backend: backend as String?,
      guarantee: guarantee,
      reason: reason as String?,
      permissions: List<String>.unmodifiable(permissions.cast<String>()),
      mimeTypes: List<String>.unmodifiable(mimeTypes.cast<String>()),
      maxBytes: maxBytes as int?,
    );
  }
}

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
    this.capabilities = const {},
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
  final Map<LinuxCapabilityKind, LinuxCapabilityStatus> capabilities;

  LinuxCapabilityStatus capability(LinuxCapabilityKind kind) =>
      capabilities[kind] ??
      const LinuxCapabilityStatus(
        state: LinuxCapabilityState.error,
        backend: null,
        guarantee: LinuxCapabilityGuarantee.none,
        reason: 'Native capability probe unavailable.',
      );

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
      capabilities: _capabilities(values['capabilities']),
    );
  }

  static Map<LinuxCapabilityKind, LinuxCapabilityStatus> _capabilities(
    Object? value,
  ) {
    if (value == null) return const {};
    if (value is! Map) {
      throw const FormatException('Invalid Linux capabilities.');
    }
    final result = <LinuxCapabilityKind, LinuxCapabilityStatus>{};
    for (final entry in value.entries) {
      final kind = _enumValue(LinuxCapabilityKind.values, entry.key);
      if (entry.value is! Map) {
        throw const FormatException('Invalid Linux capability.');
      }
      result[kind] = LinuxCapabilityStatus.fromMap(
        Map<String, Object?>.from(entry.value as Map),
      );
    }
    return Map.unmodifiable(result);
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
    Future<String> Function()? platformCapabilitiesJson,
  }) : _channel =
           channel ??
           const MethodChannel('com.copypaste.app/linux_integration'),
       _clipboardAvailable = clipboardAvailable,
       _platformCapabilitiesJson =
           platformCapabilitiesJson ??
           (clipboardAvailable == null ? api.platformCapabilitiesJson : null);

  final MethodChannel _channel;
  final Future<bool> Function()? _clipboardAvailable;
  final Future<String> Function()? _platformCapabilitiesJson;

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
    final daemon = await _daemonReport();
    final clipboard = daemon.available;
    const daemonClipboardKinds = <LinuxCapabilityKind>[
      LinuxCapabilityKind.clipboardCapture,
      LinuxCapabilityKind.clipboardReadWrite,
      LinuxCapabilityKind.sourceAttribution,
      LinuxCapabilityKind.privacyEnforcement,
    ];
    final capabilities = <LinuxCapabilityKind, LinuxCapabilityStatus>{
      ...nativeStatus.capabilities,
    };
    for (final kind in daemonClipboardKinds) {
      capabilities.remove(kind);
    }
    for (final entry in daemon.capabilities.entries) {
      if (daemonClipboardKinds.contains(entry.key) ||
          entry.key == LinuxCapabilityKind.secureStorage) {
        capabilities[entry.key] = entry.value;
      }
    }
    if (clipboard && daemon.capabilities.isEmpty) {
      for (final kind in daemonClipboardKinds.take(2)) {
        capabilities[kind] = const LinuxCapabilityStatus(
          state: LinuxCapabilityState.available,
          backend: 'daemonClipboard',
          guarantee: LinuxCapabilityGuarantee.bestEffort,
        );
      }
      capabilities[LinuxCapabilityKind.sourceAttribution] =
          const LinuxCapabilityStatus(
            state: LinuxCapabilityState.unsupported,
            backend: 'daemonClipboard',
            guarantee: LinuxCapabilityGuarantee.none,
            reason: 'The source application cannot be verified.',
          );
      capabilities[LinuxCapabilityKind.privacyEnforcement] =
          const LinuxCapabilityStatus(
            state: LinuxCapabilityState.available,
            backend: 'daemonClipboard',
            guarantee: LinuxCapabilityGuarantee.verified,
            reason: 'Unknown sources are rejected when exclusions are set.',
          );
    }
    if (!clipboard) {
      for (final kind in daemonClipboardKinds) {
        capabilities[kind] = const LinuxCapabilityStatus(
          state: LinuxCapabilityState.error,
          backend: null,
          guarantee: LinuxCapabilityGuarantee.none,
          reason: 'Clipboard daemon unavailable.',
        );
      }
    }
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
      capabilities: Map.unmodifiable(capabilities),
    );
  }

  Future<
    ({
      bool available,
      Map<LinuxCapabilityKind, LinuxCapabilityStatus> capabilities,
    })
  >
  _daemonReport() async {
    try {
      final probe = _platformCapabilitiesJson;
      if (probe == null) {
        return (
          available: await _clipboardAvailable!(),
          capabilities: const <LinuxCapabilityKind, LinuxCapabilityStatus>{},
        );
      }
      final decoded = jsonDecode(await probe());
      if (decoded is! Map ||
          decoded['clipboard_available'] is! bool ||
          decoded['capabilities'] is! Map) {
        throw const FormatException('Invalid daemon capability report.');
      }
      final capabilities = <LinuxCapabilityKind, LinuxCapabilityStatus>{};
      for (final entry in (decoded['capabilities'] as Map).entries) {
        final kind = switch (entry.key) {
          'clipboard_capture' => LinuxCapabilityKind.clipboardCapture,
          'clipboard_read_write' => LinuxCapabilityKind.clipboardReadWrite,
          'source_attribution' => LinuxCapabilityKind.sourceAttribution,
          'privacy_enforcement' => LinuxCapabilityKind.privacyEnforcement,
          'global_shortcuts' => LinuxCapabilityKind.globalShortcuts,
          'quick_paste' => LinuxCapabilityKind.quickPaste,
          'secure_storage' => LinuxCapabilityKind.secureStorage,
          'desktop_integration' => LinuxCapabilityKind.desktopIntegration,
          'updates' => LinuxCapabilityKind.updates,
          _ => throw const FormatException('Unknown daemon capability.'),
        };
        if (entry.value is! Map) {
          throw const FormatException('Invalid daemon capability.');
        }
        capabilities[kind] = LinuxCapabilityStatus.fromMap(
          Map<String, Object?>.from(entry.value as Map),
        );
      }
      return (
        available: decoded['clipboard_available'] as bool,
        capabilities:
            Map<LinuxCapabilityKind, LinuxCapabilityStatus>.unmodifiable(
              capabilities,
            ),
      );
    } catch (_) {
      return (
        available: false,
        capabilities: const <LinuxCapabilityKind, LinuxCapabilityStatus>{},
      );
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
