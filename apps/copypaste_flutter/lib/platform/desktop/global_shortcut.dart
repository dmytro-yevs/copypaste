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
      !_isSystemCopyShortcut &&
      !DesktopShortcutModifier.values.any(
        (modifier) => modifier.physicalKeys.contains(key),
      );

  // A global registration consumes the key in every application, even when
  // Quick Paste cannot open. Never reserve the operating system's copy action.
  bool get _isSystemCopyShortcut =>
      modifiers.length == 1 &&
      ((key == PhysicalKeyboardKey.keyC &&
              (modifiers.single == DesktopShortcutModifier.control ||
                  modifiers.single == DesktopShortcutModifier.meta)) ||
          (key == PhysicalKeyboardKey.insert &&
              modifiers.single == DesktopShortcutModifier.control));

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

/// Stable registration ids of application global shortcuts.
abstract final class DesktopShortcutId {
  static const quickPaste = 'copypaste.quick-paste';
}

/// Another application shortcut already holds the requested key combination.
class DesktopShortcutConflict implements Exception {
  const DesktopShortcutConflict(this.heldBy);

  /// The id whose binding was kept.
  final String heldBy;

  @override
  String toString() => 'Shortcut already registered for $heldBy.';
}

/// Global shortcuts keyed by a stable id; each id holds at most one binding.
abstract interface class DesktopShortcutRegistrar {
  /// Replaces the binding of [id] and runs [callback] on activation.
  ///
  /// Throws [DesktopShortcutConflict] when another id holds [shortcut]; the
  /// current binding of [id] is then kept. [description] names the action
  /// where the system lists registered shortcuts.
  Future<void> register(
    String id,
    DesktopShortcut shortcut,
    Future<void> Function() callback, {
    required String description,
  });

  Future<void> unregister(String id);
}

void _ensureRegistrable(
  Map<String, DesktopShortcut> registered,
  String id,
  DesktopShortcut shortcut,
) {
  if (!shortcut.isValid) {
    throw ArgumentError.value(shortcut, 'shortcut', 'Shortcut is invalid.');
  }
  for (final MapEntry(key: heldBy, value: held) in registered.entries) {
    if (heldBy != id && held == shortcut) throw DesktopShortcutConflict(heldBy);
  }
}

class HotKeyManagerDesktopShortcutRegistrar
    implements DesktopShortcutRegistrar {
  final Map<String, DesktopShortcut> _shortcuts = {};
  final Map<String, HotKey> _hotKeys = {};

  @override
  Future<void> register(
    String id,
    DesktopShortcut shortcut,
    Future<void> Function() callback, {
    required String description,
  }) async {
    _ensureRegistrable(_shortcuts, id, shortcut);
    await unregister(id);
    final hotKey = HotKey(
      identifier: id,
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
    _hotKeys[id] = hotKey;
    _shortcuts[id] = shortcut;
  }

  @override
  Future<void> unregister(String id) async {
    final hotKey = _hotKeys[id];
    if (hotKey == null) return;
    await hotKeyManager.unregister(hotKey);
    _hotKeys.remove(id);
    _shortcuts.remove(id);
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

  final MethodChannel _channel;
  final Map<String, DesktopShortcut> _shortcuts = {};
  final Map<String, Future<void> Function()> _callbacks = {};
  final Map<String, String?> _triggerDescriptions = {};

  /// The trigger the desktop portal confirmed for [id], when it reports one.
  String? registeredTriggerDescription(String id) => _triggerDescriptions[id];

  @override
  Future<void> register(
    String id,
    DesktopShortcut shortcut,
    Future<void> Function() callback, {
    required String description,
  }) async {
    _ensureRegistrable(_shortcuts, id, shortcut);
    await unregister(id);
    final supported = await _channel.invokeMethod<bool>('isSupported');
    if (supported != true) {
      throw PlatformException(code: 'shortcut_unavailable');
    }
    final result = await _channel.invokeMapMethod<String, Object?>('register', {
      'id': id,
      'description': description,
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
    _shortcuts[id] = shortcut;
    _triggerDescriptions[id] = result?['triggerDescription'] as String?;
    _callbacks[id] = callback;
  }

  @override
  Future<void> unregister(String id) async {
    if (!_callbacks.containsKey(id)) return;
    if (await _channel.invokeMethod<bool>('unregister', {'id': id}) != true) {
      throw PlatformException(code: 'shortcut_unregistration_failed');
    }
    _shortcuts.remove(id);
    _triggerDescriptions.remove(id);
    _callbacks.remove(id);
  }

  Future<Object?> _handleMethodCall(MethodCall call) async {
    if (call.method != 'activated') {
      throw MissingPluginException(
        'Unsupported Linux shortcut method: ${call.method}',
      );
    }
    final arguments = call.arguments;
    final callback = arguments is Map<Object?, Object?>
        ? _callbacks[arguments['id']]
        : null;
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
