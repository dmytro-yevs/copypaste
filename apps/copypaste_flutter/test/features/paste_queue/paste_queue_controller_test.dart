import 'dart:async';

import 'package:copypaste_flutter/features/paste_queue/controller/paste_queue_controller.dart';
import 'package:copypaste_flutter/features/paste_queue/controller/paste_queue_shortcuts_controller.dart';
import 'package:copypaste_flutter/features/paste_queue/models/paste_queue_models.dart';
import 'package:copypaste_flutter/features/paste_queue/repository/paste_queue_shortcuts_store.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'paste_queue_test_support.dart';

PasteQueueController _controller(
  FakePasteQueueRepository repository,
  FakeForegroundPasteHost host,
) => PasteQueueController(
  repository: repository,
  pasteHost: host,
  settle: Duration.zero,
);

/// Records registrations; [refused] combinations fail like a key another
/// application holds on macOS.
class _FakeRegistrar implements DesktopShortcutRegistrar {
  final bindings = <String, DesktopShortcut>{};
  final refused = <DesktopShortcut>{};

  @override
  Future<void> register(
    String id,
    DesktopShortcut shortcut,
    Future<void> Function() callback, {
    required String description,
  }) async {
    for (final MapEntry(:key, :value) in bindings.entries) {
      if (key != id && value == shortcut) throw DesktopShortcutConflict(key);
    }
    bindings.remove(id);
    if (refused.contains(shortcut)) {
      throw PlatformException(
        code: 'hotkey_registration_failed',
        details: {'osStatus': -9878},
      );
    }
    bindings[id] = shortcut;
  }

  @override
  Future<void> unregister(String id) async => bindings.remove(id);
}

void main() {
  test('rapid Paste next presses each deliver one entry, in order', () async {
    final repository = FakePasteQueueRepository(entries: 10);
    final host = FakeForegroundPasteHost(calls: repository.calls);
    final controller = _controller(repository, host);
    await controller.start();
    repository.holdDelivery = Completer<void>();

    final presses = [
      for (var press = 0; press < 10; press++)
        controller.pasteNext(trigger: PasteQueueTrigger.shortcut),
    ];
    await pumpEventQueue();
    // A press waits behind the delivery in flight instead of joining it.
    expect(repository.calls, ['deliver:e1']);
    expect(controller.deliveryInFlight, isTrue);

    repository.holdDelivery!.complete();
    await Future.wait(presses);

    expect(repository.calls, [
      for (var index = 1; index <= 10; index++) ...[
        'deliver:e$index',
        'paste',
        'confirm:t$index:true',
      ],
    ]);
    expect(repository.statuses, everyElement('pasted'));
    expect(controller.deliveryInFlight, isFalse);
    controller.dispose();
  });

  test(
    'without Accessibility Paste next copies only and shows the banner',
    () async {
      final repository = FakePasteQueueRepository(entries: 2);
      final host = FakeForegroundPasteHost(
        outcome: PasteOutcome.copiedOnly,
        accessibility: false,
      );
      final controller = _controller(repository, host);
      await controller.start();
      final feedback = <PasteQueueFeedbackKind>[];
      controller.feedback.listen((event) => feedback.add(event.kind));

      await controller.pasteNext(trigger: PasteQueueTrigger.shortcut);
      await pumpEventQueue();

      expect(controller.banner, PasteQueueBanner.accessibilityMissing);
      expect(feedback, [PasteQueueFeedbackKind.accessibilityMissing]);
      // The entry is on the clipboard, so the next press moves on.
      expect(repository.calls.last, 'confirm:t1:true');
      expect(repository.statuses, ['pasted', 'pending']);

      host.outcome = PasteOutcome.pasted;
      await controller.pasteNext(trigger: PasteQueueTrigger.shortcut);
      expect(controller.banner, isNull);
      controller.dispose();
    },
  );

  test('an elevated target keeps the entry pending', () async {
    final repository = FakePasteQueueRepository(entries: 1);
    final host = FakeForegroundPasteHost(outcome: PasteOutcome.elevatedTarget);
    final controller = _controller(repository, host);
    await controller.start();

    await controller.pasteNext(trigger: PasteQueueTrigger.shortcut);

    expect(repository.calls.last, 'confirm:t1:false');
    expect(repository.statuses, ['pending']);
    expect(controller.pendingCount, 1);
    controller.dispose();
  });

  group('shortcuts', () {
    Future<(PasteQueueShortcutsController, _FakeRegistrar)> start(
      TargetPlatform platform,
    ) async {
      final queue = _controller(
        FakePasteQueueRepository(),
        FakeForegroundPasteHost(),
      );
      await queue.start();
      final registrar = _FakeRegistrar();
      final shortcuts = PasteQueueShortcutsController(
        queue: queue,
        registrar: registrar,
        store: MemoryPasteQueueShortcutsStore(),
        platform: platform,
      );
      await shortcuts.start();
      addTearDown(() {
        shortcuts.dispose();
        queue.dispose();
      });
      return (shortcuts, registrar);
    }

    const other = DesktopShortcut(
      key: PhysicalKeyboardKey.keyQ,
      modifiers: [
        DesktopShortcutModifier.control,
        DesktopShortcutModifier.meta,
      ],
    );

    test(
      'a combination another app holds keeps the previous binding',
      () async {
        final (shortcuts, registrar) = await start(TargetPlatform.macOS);
        const action = PasteQueueShortcutAction.pasteNext;
        final previous = action.defaultFor(TargetPlatform.macOS)!;
        expect(registrar.bindings[action.id], previous);
        registrar.refused.add(other);

        expect(await shortcuts.setShortcut(action, other), isFalse);

        expect(registrar.bindings[action.id], previous);
        expect(shortcuts.shortcutFor(action), previous);
        expect(shortcuts.problemOf(action), PasteQueueShortcutProblem.inUse);
      },
    );

    test('Windows starts unbound with a prompt and refuses Ctrl+Alt', () async {
      final (shortcuts, registrar) = await start(TargetPlatform.windows);

      expect(registrar.bindings, isEmpty);
      expect(shortcuts.needsShortcuts, isTrue);
      expect(shortcuts.showsShortcutPrompt, isTrue);
      expect(
        shortcuts.accepts(
          const DesktopShortcut(
            key: PhysicalKeyboardKey.keyV,
            modifiers: [
              DesktopShortcutModifier.control,
              DesktopShortcutModifier.alt,
            ],
          ),
        ),
        isFalse,
      );
    });
  });
}
