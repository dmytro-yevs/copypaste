import 'package:flutter/services.dart';

import '../../../l10n/app_localizations.dart';
import '../../../platform/desktop/global_shortcut.dart';

/// Rules for the two desktop entry points: the application switcher icon
/// (Dock or taskbar) and the status area icon (menu bar or tray).
abstract final class DesktopPresence {
  /// Turning an entry point off while the other is already off leaves no
  /// visible icon.
  static bool hidesLastEntryPoint({
    required bool turningOn,
    required bool otherVisible,
  }) => !turningOn && !otherVisible;

  /// How to come back once no icon is visible: the registered Quick Paste
  /// shortcut when there is one, otherwise starting the application again.
  /// [registeredShortcutDescription] is the trigger a desktop portal
  /// confirmed, which may differ from the requested keys.
  static String hiddenEverywhereMessage(
    AppLocalizations l10n, {
    DesktopShortcut? registeredShortcut,
    String? registeredShortcutDescription,
  }) {
    final shortcut = registeredShortcut == null
        ? null
        : registeredShortcutDescription ??
              shortcutText(registeredShortcut.displayKeys);
    return shortcut == null
        ? l10n.settingsHideEverywhereMessage
        : l10n.settingsHideEverywhereShortcutMessage(shortcut);
  }

  /// The same key legends the shortcut recorder shows.
  static String shortcutText(List<LogicalKeyboardKey> keys) => keys
      .map(
        (key) => switch (key) {
          LogicalKeyboardKey.control => 'Ctrl',
          LogicalKeyboardKey.shift => 'Shift',
          LogicalKeyboardKey.alt => 'Alt',
          LogicalKeyboardKey.meta => '⌘',
          _ => key.keyLabel,
        },
      )
      .join('+');
}
