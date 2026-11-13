import 'dart:async';

import 'package:copypaste_flutter/app/app_features.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import '../features/paste_queue/paste_queue_test_support.dart';
import 'app_features_test_support.dart';

void main() {
  test('a queue delivery in flight keeps the presentation', () async {
    final repository = FakePasteQueueRepository(entries: 1)
      ..holdDelivery = Completer<void>();
    final features = testAppFeatures(queue: repository);
    addTearDown(features.dispose);
    await features.pasteQueue.start();
    expect(features.busy, isFalse);

    final delivery = features.pasteQueue.pasteNext();
    expect(features.busy, isTrue);

    repository.holdDelivery!.complete();
    await delivery;
    expect(features.busy, isFalse);
  });

  test('shortcut text follows the platform convention', () {
    const shortcut = DesktopShortcut(
      key: PhysicalKeyboardKey.keyV,
      modifiers: [
        DesktopShortcutModifier.meta,
        DesktopShortcutModifier.shift,
        DesktopShortcutModifier.control,
      ],
    );
    expect(
      desktopShortcutText(shortcut, platform: TargetPlatform.macOS),
      '⌃⇧⌘V',
    );
    expect(
      desktopShortcutText(shortcut, platform: TargetPlatform.windows),
      'Ctrl+Shift+Win+V',
    );
  });
}
