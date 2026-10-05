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
