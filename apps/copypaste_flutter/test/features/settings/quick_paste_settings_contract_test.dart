import 'dart:async';

import 'package:copypaste_flutter/features/settings/controller/quick_paste_settings_controller.dart';
import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const replacement = DesktopShortcut(
    key: PhysicalKeyboardKey.keyV,
    modifiers: [DesktopShortcutModifier.alt, DesktopShortcutModifier.shift],
  );
  late _Store store;
  late _Registrar registrar;
  late _WindowHost host;
  late QuickPasteSettingsController controller;
  var disposed = false;

  setUp(() async {
    disposed = false;
    store = _Store();
    registrar = _Registrar();
    host = _WindowHost();
    controller = QuickPasteSettingsController(
      store: store,
      registrar: registrar,
      windowHost: host,
    );
    await controller.initialize();
  });

  tearDown(() {
    if (!disposed) controller.dispose();
  });

  test(
    'same shortcut recording restores one callback without rewriting preferences',
    () async {
      final saved = controller.shortcut;
      expect(await controller.beginShortcutRecording(), isTrue);
      expect(registrar.callback, isNull);

      expect(await controller.setShortcut(saved), isFalse);

      expect(registrar.registered, saved);
      expect(registrar.registerCalls, 2);
      expect(store.writes, 0);
      await registrar.callback!();
      expect(host.openCalls, 1);
      expect(controller.errorMessage, isNull);
      await controller.cancelShortcutRecording();
      expect(registrar.registerCalls, 2);
    },
  );

  test(
    'unchanged shortcut outside recording does not replace registration',
    () async {
      expect(await controller.setShortcut(controller.shortcut), isFalse);
      expect(registrar.registerCalls, 1);
      expect(store.writes, 0);
    },
  );

  test(
    'cancel restores the saved chord and later open failures are handled',
    () async {
      await controller.beginShortcutRecording();
      await controller.cancelShortcutRecording();

      expect(registrar.registered, controller.shortcut);
      expect(store.writes, 0);
      host.failOpen = true;
      await registrar.callback!();
      expect(controller.errorMessage, 'Quick Paste could not be opened.');
    },
  );

  test('same shortcut restoration failure remains visible', () async {
    await controller.beginShortcutRecording();
    registrar.failRegistrations = 1;

    expect(await controller.setShortcut(controller.shortcut), isFalse);

    expect(registrar.callback, isNull);
    expect(store.writes, 0);
    expect(controller.errorMessage, 'The shortcut could not be restored.');
  });

  test(
    'different shortcut is persisted and receives the caught open action',
    () async {
      await controller.beginShortcutRecording();
      expect(await controller.setShortcut(replacement), isTrue);

      expect(store.value.shortcut, replacement);
      expect(store.writes, 1);
      expect(registrar.registered, replacement);
      host.failOpen = true;
      await registrar.callback!();
      expect(controller.errorMessage, 'Quick Paste could not be opened.');
    },
  );

  test(
    'rejected registration restores the previous shortcut and caught callback',
    () async {
      final saved = controller.shortcut;
      await controller.beginShortcutRecording();
      registrar.failRegistrations = 1;

      expect(await controller.setShortcut(replacement), isFalse);

      expect(store.writes, 0);
      expect(controller.shortcut, saved);
      expect(registrar.registered, saved);
      expect(controller.errorMessage, 'That shortcut could not be registered.');
      host.failOpen = true;
      await registrar.callback!();
      expect(controller.errorMessage, 'Quick Paste could not be opened.');
    },
  );

  test(
    'failed persistence restores previous preferences and registration',
    () async {
      final saved = controller.shortcut;
      store.failWrite = true;

      expect(await controller.setShortcut(replacement), isFalse);

      expect(store.value.shortcut, saved);
      expect(controller.shortcut, saved);
      expect(registrar.registered, saved);
      expect(controller.errorMessage, 'That shortcut could not be registered.');
    },
  );

  test('failed replacement and restore do not report success', () async {
    final saved = controller.shortcut;
    registrar.failRegistrations = 2;

    expect(await controller.setShortcut(replacement), isFalse);

    expect(store.writes, 0);
    expect(controller.shortcut, saved);
    expect(registrar.callback, isNull);
    expect(controller.errorMessage, 'That shortcut could not be registered.');
  });

  test(
    'late open failure notifies state and subsequent success clears it',
    () async {
      var notifications = 0;
      controller.addListener(() => notifications += 1);
      host.failOpen = true;

      await registrar.callback!();

      expect(controller.errorMessage, 'Quick Paste could not be opened.');
      expect(notifications, 1);
      host.failOpen = false;
      await registrar.callback!();
      expect(controller.errorMessage, isNull);
      expect(notifications, 2);
      expect(host.openCalls, 2);
    },
  );

  test('rapid recording starts issue only one pending pause', () async {
    registrar.unregisterGate = Completer<void>();
    registrar.unregisterStarted = Completer<void>();
    final first = controller.beginShortcutRecording();

    expect(controller.busy, isTrue);
    expect(await controller.beginShortcutRecording(), isFalse);
    await registrar.unregisterStarted!.future;
    expect(registrar.unregisterCalls, 1);
    registrar.unregisterGate!.complete();

    expect(await first, isTrue);
    expect(controller.busy, isFalse);
    expect(registrar.registered, isNull);
    expect(registrar.maxActiveOperations, 1);
    await controller.cancelShortcutRecording();
    expect(registrar.registered, controller.shortcut);
  });

  for (final replace in [false, true]) {
    test(
      'pending cancel blocks a new registration transition (replace: $replace)',
      () async {
        await controller.beginShortcutRecording();
        registrar.registerGate = Completer<void>();
        registrar.registerStarted = Completer<void>();
        final restoring = controller.cancelShortcutRecording();
        await registrar.registerStarted!.future;

        expect(controller.busy, isTrue);
        expect(await controller.beginShortcutRecording(), isFalse);
        expect(await controller.setShortcut(replacement), isFalse);
        expect(registrar.registerCalls, 2);
        expect(registrar.unregisterCalls, 1);
        expect(store.writes, 0);
        registrar.registerGate!.complete();
        await restoring;

        expect(registrar.registered, controller.shortcut);
        expect(controller.busy, isFalse);
        if (replace) {
          expect(await controller.setShortcut(replacement), isTrue);
          expect(registrar.registered, replacement);
          expect(store.writes, 1);
        } else {
          expect(await controller.beginShortcutRecording(), isTrue);
          expect(registrar.registered, isNull);
          await controller.cancelShortcutRecording();
          expect(registrar.registered, controller.shortcut);
          expect(store.writes, 0);
        }
        expect(registrar.maxActiveOperations, 1);
        await registrar.callback!();
        expect(host.openCalls, 1);
      },
    );
  }

  test('same-value completion blocks a second start while restoring', () async {
    await controller.beginShortcutRecording();
    registrar.registerGate = Completer<void>();
    registrar.registerStarted = Completer<void>();
    final restoring = controller.setShortcut(controller.shortcut);
    await registrar.registerStarted!.future;

    expect(controller.busy, isTrue);
    expect(await controller.beginShortcutRecording(), isFalse);
    expect(await controller.setShortcut(replacement), isFalse);
    registrar.registerGate!.complete();

    expect(await restoring, isFalse);
    expect(registrar.registered, controller.shortcut);
    expect(store.writes, 0);
    expect(registrar.maxActiveOperations, 1);
  });

  test('dispose waits for pending pause before its final unregister', () async {
    registrar.unregisterGate = Completer<void>();
    registrar.unregisterStarted = Completer<void>();
    final pausing = controller.beginShortcutRecording();
    await registrar.unregisterStarted!.future;
    registrar.unregisterCompleted = Completer<void>();
    var notifications = 0;
    controller.addListener(() => notifications += 1);
    controller.dispose();
    disposed = true;

    expect(registrar.unregisterCalls, 1);
    registrar.unregisterGate!.complete();
    expect(await pausing, isFalse);
    await registrar.unregisterCompleted!.future;

    expect(registrar.unregisterCalls, 2);
    expect(registrar.callback, isNull);
    expect(registrar.registered, isNull);
    expect(registrar.maxActiveOperations, 1);
    expect(notifications, 0);
  });

  for (final sameValue in [false, true]) {
    for (final fails in [false, true]) {
      test(
        'dispose cleans pending restore (same value: $sameValue, failure: $fails)',
        () async {
          await controller.beginShortcutRecording();
          registrar.registerGate = Completer<void>();
          registrar.registerStarted = Completer<void>();
          registrar.failRegistrations = fails ? 1 : 0;
          final restoring = sameValue
              ? controller.setShortcut(controller.shortcut)
              : controller.cancelShortcutRecording();
          await registrar.registerStarted!.future;
          registrar.unregisterCompleted = Completer<void>();
          var notifications = 0;
          controller.addListener(() => notifications += 1);
          controller.dispose();
          disposed = true;
          expect(registrar.unregisterCalls, 1);

          registrar.registerGate!.complete();
          await restoring;
          await registrar.unregisterCompleted!.future;

          expect(registrar.registerCalls, 2);
          expect(registrar.unregisterCalls, 2);
          expect(registrar.callback, isNull);
          expect(registrar.registered, isNull);
          expect(store.writes, 0);
          expect(registrar.maxActiveOperations, 1);
          expect(notifications, 0);
          expect(await controller.beginShortcutRecording(), isFalse);
          expect(await controller.setShortcut(replacement), isFalse);
        },
      );
    }
  }

  test(
    'dispose cleans pending replacement without persisting or restoring it',
    () async {
      registrar.registerGate = Completer<void>();
      registrar.registerStarted = Completer<void>();
      final replacing = controller.setShortcut(replacement);
      await registrar.registerStarted!.future;
      registrar.unregisterCompleted = Completer<void>();
      var notifications = 0;
      controller.addListener(() => notifications += 1);
      controller.dispose();
      disposed = true;
      registrar.registerGate!.complete();

      expect(await replacing, isFalse);
      await registrar.unregisterCompleted!.future;
      expect(registrar.registerCalls, 2);
      expect(registrar.callback, isNull);
      expect(store.writes, 0);
      expect(registrar.maxActiveOperations, 1);
      expect(notifications, 0);
    },
  );

  test(
    'dispose cleans pending initial registration without late notification',
    () async {
      controller.dispose();
      disposed = true;
      final initialRegistrar = _Registrar()
        ..registerGate = Completer<void>()
        ..registerStarted = Completer<void>()
        ..unregisterCompleted = Completer<void>();
      final initialController = QuickPasteSettingsController(
        store: _Store(),
        registrar: initialRegistrar,
        windowHost: _WindowHost(),
      );
      final initializing = initialController.initialize();
      await initialRegistrar.registerStarted!.future;
      var notifications = 0;
      initialController.addListener(() => notifications += 1);
      initialController.dispose();
      initialRegistrar.registerGate!.complete();

      await initializing;
      await initialRegistrar.unregisterCompleted!.future;
      expect(initialRegistrar.registerCalls, 1);
      expect(initialRegistrar.callback, isNull);
      expect(initialRegistrar.maxActiveOperations, 1);
      expect(notifications, 0);
    },
  );

  for (final fails in [false, true]) {
    test('open completion after disposal is safe (failure: $fails)', () async {
      host.openGate = Completer<void>();
      host.failOpen = fails;
      var notifications = 0;
      controller.addListener(() => notifications += 1);
      final callback = registrar.callback!;
      final opening = callback();
      expect(host.openCalls, 1);
      controller.dispose();
      disposed = true;
      host.openGate!.complete();

      await opening;
      await callback();

      expect(notifications, 0);
      expect(host.openCalls, 1);
    });
  }
}

class _Store extends MemoryQuickPastePreferencesStore {
  int writes = 0;
  bool failWrite = false;

  @override
  Future<void> write(QuickPastePreferences preferences) async {
    writes += 1;
    if (failWrite) throw StateError('Preference write failed.');
    await super.write(preferences);
  }
}

class _Registrar implements DesktopShortcutRegistrar {
  DesktopShortcut? registered;
  Future<void> Function()? callback;
  int registerCalls = 0;
  int failRegistrations = 0;
  int unregisterCalls = 0;
  int activeOperations = 0;
  int maxActiveOperations = 0;
  Completer<void>? registerGate;
  Completer<void>? registerStarted;
  Completer<void>? unregisterGate;
  Completer<void>? unregisterStarted;
  Completer<void>? unregisterCompleted;

  void _startedOperation() {
    activeOperations += 1;
    if (activeOperations > maxActiveOperations) {
      maxActiveOperations = activeOperations;
    }
  }

  @override
  Future<void> register(
    DesktopShortcut shortcut,
    Future<void> Function() callback,
  ) async {
    registerCalls += 1;
    _startedOperation();
    registered = null;
    this.callback = null;
    if (registerStarted?.isCompleted == false) registerStarted!.complete();
    try {
      await registerGate?.future;
      if (failRegistrations > 0) {
        failRegistrations -= 1;
        throw PlatformException(code: 'registration_failed');
      }
      registered = shortcut;
      this.callback = callback;
    } finally {
      activeOperations -= 1;
    }
  }

  @override
  Future<void> unregister() async {
    unregisterCalls += 1;
    _startedOperation();
    final completed = unregisterCompleted;
    if (unregisterStarted?.isCompleted == false) unregisterStarted!.complete();
    try {
      await unregisterGate?.future;
      registered = null;
      callback = null;
    } finally {
      activeOperations -= 1;
      completed?.complete();
    }
  }
}

class _WindowHost implements QuickPasteWindowHost {
  int openCalls = 0;
  bool failOpen = false;
  Completer<void>? openGate;

  @override
  Future<bool> isSupported() async => true;

  @override
  Future<void> prepare() async {}

  @override
  Future<void> open() async {
    openCalls += 1;
    await openGate?.future;
    if (failOpen) throw PlatformException(code: 'window_unavailable');
  }

  @override
  Future<bool> accessibilityGranted() async => true;

  @override
  Future<bool> requestAccessibility() async => true;

  @override
  void setOpenSettingsHandler(VoidCallback? handler) {}

  @override
  Future<void> dispose() async {}
}
