import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:hotkey_manager/hotkey_manager.dart';

enum DesktopShortcutModifier { control, shift, alt, meta }

class DesktopShortcut {
  const DesktopShortcut({required this.key, required this.modifiers});

  factory DesktopShortcut.defaultForPlatform() => DesktopShortcut(
    key: PhysicalKeyboardKey.keyC,
    modifiers: Platform.isMacOS
        ? const [DesktopShortcutModifier.shift, DesktopShortcutModifier.meta]
        : const [
            DesktopShortcutModifier.control,
            DesktopShortcutModifier.shift,
          ],
  );

  factory DesktopShortcut.fromJson(Map<String, Object?> json) {
    final usage = json['usage'] as int?;
    final key = usage == null ? null : PhysicalKeyboardKey.findKeyByCode(usage);
    final rawModifiers = json['modifiers'] as List<Object?>?;
    if (key == null || rawModifiers == null) {
      throw const FormatException('Invalid desktop shortcut.');
    }
    final modifiers = rawModifiers
        .whereType<String>()
        .map(
          (name) => DesktopShortcutModifier.values.firstWhere(
            (modifier) => modifier.name == name,
          ),
        )
        .toList(growable: false);
    final shortcut = DesktopShortcut(key: key, modifiers: modifiers);
    if (!shortcut.isValid) {
      throw const FormatException('Invalid desktop shortcut.');
    }
    return shortcut;
  }

  final PhysicalKeyboardKey key;
  final List<DesktopShortcutModifier> modifiers;

  bool get isValid =>
      modifiers.isNotEmpty &&
      !DesktopShortcutModifier.values.any(
        (modifier) => modifier.physicalKeys.contains(key),
      );

  List<LogicalKeyboardKey> get displayKeys => [
    for (final modifier in modifiers) modifier.logicalKey,
    HotKey(key: key).logicalKey,
  ];

  Map<String, Object?> toJson() => {
    'usage': key.usbHidUsage,
    'modifiers': modifiers.map((modifier) => modifier.name).toList(),
  };

  @override
  bool operator ==(Object other) =>
      other is DesktopShortcut &&
      other.key == key &&
      _sameModifiers(other.modifiers, modifiers);

  @override
  int get hashCode => Object.hash(key, Object.hashAll(modifiers));

  static bool _sameModifiers(
    List<DesktopShortcutModifier> left,
    List<DesktopShortcutModifier> right,
  ) {
    if (left.length != right.length) return false;
    for (var index = 0; index < left.length; index += 1) {
      if (left[index] != right[index]) return false;
    }
    return true;
  }
}

extension DesktopShortcutModifierPresentation on DesktopShortcutModifier {
  List<PhysicalKeyboardKey> get physicalKeys => switch (this) {
    DesktopShortcutModifier.control => const [
      PhysicalKeyboardKey.controlLeft,
      PhysicalKeyboardKey.controlRight,
    ],
    DesktopShortcutModifier.shift => const [
      PhysicalKeyboardKey.shiftLeft,
      PhysicalKeyboardKey.shiftRight,
    ],
    DesktopShortcutModifier.alt => const [
      PhysicalKeyboardKey.altLeft,
      PhysicalKeyboardKey.altRight,
    ],
    DesktopShortcutModifier.meta => const [
      PhysicalKeyboardKey.metaLeft,
      PhysicalKeyboardKey.metaRight,
    ],
  };

  LogicalKeyboardKey get logicalKey => switch (this) {
    DesktopShortcutModifier.control => LogicalKeyboardKey.control,
    DesktopShortcutModifier.shift => LogicalKeyboardKey.shift,
    DesktopShortcutModifier.alt => LogicalKeyboardKey.alt,
    DesktopShortcutModifier.meta => LogicalKeyboardKey.meta,
  };

  HotKeyModifier get hotKeyModifier => switch (this) {
    DesktopShortcutModifier.control => HotKeyModifier.control,
    DesktopShortcutModifier.shift => HotKeyModifier.shift,
    DesktopShortcutModifier.alt => HotKeyModifier.alt,
    DesktopShortcutModifier.meta => HotKeyModifier.meta,
  };
}

abstract interface class DesktopShortcutRegistrar {
  Future<void> register(
    DesktopShortcut shortcut,
    Future<void> Function() callback,
  );

  Future<void> unregister();
}

class HotKeyManagerDesktopShortcutRegistrar
    implements DesktopShortcutRegistrar {
  HotKey? _registered;

  @override
  Future<void> register(
    DesktopShortcut shortcut,
    Future<void> Function() callback,
  ) async {
    if (!shortcut.isValid) {
      throw ArgumentError.value(shortcut, 'shortcut', 'Shortcut is invalid.');
    }
    await unregister();
    final hotKey = HotKey(
      identifier: 'copypaste.quick-paste',
      key: shortcut.key,
      modifiers: shortcut.modifiers
          .map((modifier) => modifier.hotKeyModifier)
          .toList(growable: false),
      scope: HotKeyScope.system,
    );
    await hotKeyManager.register(
      hotKey,
      keyDownHandler: (_) {
        unawaited(callback());
      },
    );
    _registered = hotKey;
  }

  @override
  Future<void> unregister() async {
    final hotKey = _registered;
    if (hotKey != null) {
      await hotKeyManager.unregister(hotKey);
      _registered = null;
    }
  }
}

/// Registers global shortcuts through the Linux desktop portal.
///
/// The native host owns session consent and Wayland activation tokens. It only
/// reports activation after the portal has accepted the registration.
class LinuxPortalDesktopShortcutRegistrar implements DesktopShortcutRegistrar {
  LinuxPortalDesktopShortcutRegistrar({MethodChannel? channel})
    : _channel =
          channel ?? const MethodChannel('com.copypaste.app/linux_shortcuts') {
    _channel.setMethodCallHandler(_handleMethodCall);
  }

  static const _shortcutId = 'copypaste.quick-paste';

  final MethodChannel _channel;
  Future<void> Function()? _callback;
  String? _registeredId;
  String? _registeredTriggerDescription;

  /// The trigger confirmed by the desktop portal, when it reports one.
  String? get registeredTriggerDescription => _registeredTriggerDescription;

  @override
  Future<void> register(
    DesktopShortcut shortcut,
    Future<void> Function() callback,
  ) async {
    if (!shortcut.isValid) {
      throw ArgumentError.value(shortcut, 'shortcut', 'Shortcut is invalid.');
    }
    await unregister();
    final supported = await _channel.invokeMethod<bool>('isSupported');
    if (supported != true) {
      throw PlatformException(code: 'shortcut_unavailable');
    }
    final result = await _channel.invokeMapMethod<String, Object?>('register', {
      'id': _shortcutId,
      'description': 'Open Quick Paste',
      'preferredTrigger': shortcut.linuxPreferredTrigger,
      'usage': shortcut.key.usbHidUsage,
      'modifiers': shortcut.modifiers.map((modifier) => modifier.name).toList(),
    });
    if (result?['registered'] != true) {
      throw PlatformException(
        code: 'shortcut_registration_failed',
        message: result?['reason'] as String?,
      );
    }
    _registeredId = _shortcutId;
    _registeredTriggerDescription = result?['triggerDescription'] as String?;
    _callback = callback;
  }

  @override
  Future<void> unregister() async {
    final id = _registeredId;
    if (id == null) return;
    if (await _channel.invokeMethod<bool>('unregister', {'id': id}) != true) {
      throw PlatformException(code: 'shortcut_unregistration_failed');
    }
    _registeredId = null;
    _registeredTriggerDescription = null;
    _callback = null;
  }

  Future<Object?> _handleMethodCall(MethodCall call) async {
    if (call.method != 'activated') {
      throw MissingPluginException(
        'Unsupported Linux shortcut method: ${call.method}',
      );
    }
    final arguments = call.arguments;
    if (arguments is! Map<Object?, Object?> ||
        arguments['id'] != _registeredId) {
      return false;
    }
    final callback = _callback;
    if (callback == null) return false;
    unawaited(callback());
    return true;
  }
}

extension LinuxDesktopShortcutPresentation on DesktopShortcut {
  /// A portable XDG/XKB hint; USB HID usage remains the binding source of truth.
  String get linuxPreferredTrigger => [
    for (final modifier in modifiers) modifier.linuxTriggerName,
    _linuxKeySymbol,
  ].join('+');

  String get _linuxKeySymbol {
    final label = HotKey(key: key).logicalKey.keyLabel;
    if (RegExp(r'^[A-Za-z0-9]+$').hasMatch(label)) return label.toUpperCase();
    return switch (key) {
      PhysicalKeyboardKey.escape => 'ESC',
      PhysicalKeyboardKey.backspace => 'BACKSPACE',
      PhysicalKeyboardKey.tab => 'TAB',
      PhysicalKeyboardKey.space => 'SPACE',
      PhysicalKeyboardKey.enter || PhysicalKeyboardKey.numpadEnter => 'ENTER',
      PhysicalKeyboardKey.arrowUp => 'UP',
      PhysicalKeyboardKey.arrowDown => 'DOWN',
      PhysicalKeyboardKey.arrowLeft => 'LEFT',
      PhysicalKeyboardKey.arrowRight => 'RIGHT',
      PhysicalKeyboardKey.home => 'HOME',
      PhysicalKeyboardKey.end => 'END',
      PhysicalKeyboardKey.pageUp => 'PAGEUP',
      PhysicalKeyboardKey.pageDown => 'PAGEDOWN',
      PhysicalKeyboardKey.insert => 'INSERT',
      PhysicalKeyboardKey.delete => 'DELETE',
      PhysicalKeyboardKey.minus => 'MINUS',
      PhysicalKeyboardKey.equal => 'EQUAL',
      PhysicalKeyboardKey.bracketLeft => 'BRACKETLEFT',
      PhysicalKeyboardKey.bracketRight => 'BRACKETRIGHT',
      PhysicalKeyboardKey.backslash => 'BACKSLASH',
      PhysicalKeyboardKey.semicolon => 'SEMICOLON',
      PhysicalKeyboardKey.quote => 'APOSTROPHE',
      PhysicalKeyboardKey.backquote => 'GRAVE',
      PhysicalKeyboardKey.comma => 'COMMA',
      PhysicalKeyboardKey.period => 'PERIOD',
      PhysicalKeyboardKey.slash => 'SLASH',
      _ => 'HID_${key.usbHidUsage.toRadixString(16).toUpperCase()}',
    };
  }
}

extension on DesktopShortcutModifier {
  String get linuxTriggerName => switch (this) {
    DesktopShortcutModifier.control => 'CTRL',
    DesktopShortcutModifier.shift => 'SHIFT',
    DesktopShortcutModifier.alt => 'ALT',
    DesktopShortcutModifier.meta => 'META',
  };
}
