import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

bool get usesDesktopShortcuts => switch (defaultTargetPlatform) {
  TargetPlatform.macOS ||
  TargetPlatform.windows ||
  TargetPlatform.linux => true,
  _ => false,
};

SingleActivator desktopShortcut(LogicalKeyboardKey key) => SingleActivator(
  key,
  meta: defaultTargetPlatform == TargetPlatform.macOS,
  control: defaultTargetPlatform != TargetPlatform.macOS,
  includeRepeats: false,
);

Map<ShortcutActivator, VoidCallback> desktopWindowBindings({
  Future<void> Function()? onClose,
  Future<void> Function()? onQuit,
  Future<void> Function()? onToggleFullScreen,
}) => {
  if (usesDesktopShortcuts) ...{
    if (onClose != null)
      desktopShortcut(LogicalKeyboardKey.keyW): () => unawaited(onClose()),
    if (onQuit != null)
      desktopShortcut(LogicalKeyboardKey.keyQ): () => unawaited(onQuit()),
    if (onToggleFullScreen != null &&
        defaultTargetPlatform != TargetPlatform.macOS)
      const SingleActivator(
        LogicalKeyboardKey.f11,
        includeRepeats: false,
      ): () =>
          unawaited(onToggleFullScreen()),
  },
};

/// Supplement Flutter's platform editing defaults only where a convention is
/// missing. EditableText/SelectableText retain ownership of the actual actions.
Map<ShortcutActivator, Intent> get desktopTextEditingShortcuts =>
    switch (defaultTargetPlatform) {
      TargetPlatform.macOS => const {
        SingleActivator(
          LogicalKeyboardKey.keyV,
          meta: true,
          alt: true,
          shift: true,
        ): PasteTextIntent(
          SelectionChangedCause.keyboard,
        ),
      },
      TargetPlatform.windows => const {
        SingleActivator(LogicalKeyboardKey.keyY, control: true): RedoTextIntent(
          SelectionChangedCause.keyboard,
        ),
        ..._plainTextPaste,
      },
      TargetPlatform.linux => _plainTextPaste,
      _ => const {},
    };

const _plainTextPaste = <ShortcutActivator, Intent>{
  SingleActivator(LogicalKeyboardKey.keyV, control: true, shift: true):
      PasteTextIntent(SelectionChangedCause.keyboard),
};
