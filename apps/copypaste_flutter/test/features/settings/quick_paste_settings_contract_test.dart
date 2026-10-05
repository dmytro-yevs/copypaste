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

  @override
  Future<void> register(
    DesktopShortcut shortcut,
    Future<void> Function() callback,
  ) async {
    registerCalls += 1;
    registered = null;
    this.callback = null;
    if (failRegistrations > 0) {
      failRegistrations -= 1;
      throw PlatformException(code: 'registration_failed');
    }
    registered = shortcut;
    this.callback = callback;
  }

  @override
  Future<void> unregister() async {
    registered = null;
    callback = null;
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
