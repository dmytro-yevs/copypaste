import 'package:copypaste_flutter/features/settings/controller/quick_paste_settings_controller.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/desktop_presence.dart';
import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:copypaste_flutter/platform/permissions/linux_integration.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  test(
    'a permission error cannot persist an auto-paste value hidden by rollback',
    () async {
      final store = MemoryQuickPastePreferencesStore(
        QuickPastePreferences.defaults().copyWith(autoPaste: false),
      );
      final controller = QuickPasteSettingsController(
        store: store,
        registrar: _Registrar(),
        windowHost: _WindowHost()..failPermission = true,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      expect(await controller.setAutoPaste(true), isFalse);
      expect(controller.autoPaste, isFalse);
      expect((await store.read()).autoPaste, isFalse);
    },
  );

  test(
    'the saved shortcut can be retried after cancellation restoration fails',
    () async {
      final store = MemoryQuickPastePreferencesStore();
      final registrar = _Registrar();
      final controller = QuickPasteSettingsController(
        store: store,
        registrar: registrar,
        windowHost: _WindowHost(),
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.beginShortcutRecording();
      registrar.failRegister = true;
      await controller.cancelShortcutRecording();
      expect(registrar.registered, isNull);
      expect(controller.errorMessage, isNotNull);
      registrar.failRegister = false;
      expect(await controller.setShortcut(store.value.shortcut), isFalse);
      expect(registrar.registered, store.value.shortcut);
      expect(controller.errorMessage, isNull);
    },
  );

  test('persists auto-paste and replaces the registered shortcut', () async {
    final store = MemoryQuickPastePreferencesStore();
    final registrar = _Registrar();
    final host = _WindowHost();
    final controller = QuickPasteSettingsController(
      store: store,
      registrar: registrar,
      windowHost: host,
    );

    await controller.initialize();
    expect(controller.autoPaste, isTrue);
    expect(registrar.registered, DesktopShortcut.defaultForPlatform());
    expect(host.prepareCalls, 0);

    await controller.setAutoPaste(false);
    expect(store.value.autoPaste, isFalse);
    await controller.setAutoPaste(true);
    expect(host.permissionRequests, 1);

    const replacement = DesktopShortcut(
      key: PhysicalKeyboardKey.keyV,
      modifiers: [DesktopShortcutModifier.alt, DesktopShortcutModifier.shift],
    );
    await controller.beginShortcutRecording();
    expect(registrar.registered, isNull);
    expect(await controller.setShortcut(replacement), isTrue);
    expect(store.value.shortcut, replacement);
    expect(registrar.registered, replacement);
    controller.dispose();
  });

  group('startup failures', () {
    const replacement = DesktopShortcut(
      key: PhysicalKeyboardKey.keyV,
      modifiers: [DesktopShortcutModifier.alt, DesktopShortcutModifier.shift],
    );
    late _Registrar registrar;
    late _WindowHost host;
    late MemoryQuickPastePreferencesStore store;
    late QuickPasteSettingsController controller;

    setUp(() {
      registrar = _Registrar();
      host = _WindowHost()..permissionGranted = true;
      store = MemoryQuickPastePreferencesStore();
      controller = QuickPasteSettingsController(
        store: store,
        registrar: registrar,
        windowHost: host,
      );
      addTearDown(controller.dispose);
    });

    test('a shortcut in use is reported on the shortcut only', () async {
      registrar.nextRegisterError = PlatformException(
        code: 'hotkey_registration_failed',
        details: {'osStatus': -9878},
      );

      await controller.initialize();

      expect(
        controller.shortcutRefusal,
        DesktopShortcutRefusal(
          DesktopShortcut.defaultForPlatform(),
          DesktopShortcutRefusalReason.inUse,
        ),
      );
      expect(controller.supported, isTrue);
      expect(controller.errorMessage, isNull);
      expect(controller.startFailed, isFalse);
      expect(controller.accessibilityGranted, isTrue);

      expect(await controller.setShortcut(replacement), isTrue);
      expect(controller.shortcutRefusal, isNull);
      expect(registrar.registered, replacement);
    });

    test('a rejected change keeps the saved shortcut working', () async {
      await controller.initialize();
      registrar.nextRegisterError = PlatformException(
        code: 'hotkey_registration_failed',
        details: {'win32Error': 87},
      );

      expect(await controller.setShortcut(replacement), isFalse);

      expect(
        controller.shortcutRefusal,
        const DesktopShortcutRefusal(
          replacement,
          DesktopShortcutRefusalReason.rejected,
        ),
      );
      expect(controller.errorMessage, isNull);
      expect(registrar.registered, DesktopShortcut.defaultForPlatform());
      expect(store.value.shortcut, DesktopShortcut.defaultForPlatform());
    });

    test('an unsupported system is not an error', () async {
      host.supported = false;

      await controller.initialize();

      expect(controller.supported, isFalse);
      expect(controller.startFailed, isFalse);
      expect(controller.errorMessage, isNull);
      expect(registrar.registerCalls, 0);
    });

    test('an unexpected failure is logged and can be retried', () async {
      final logs = <String?>[];
      final previous = debugPrint;
      debugPrint = (message, {wrapWidth}) => logs.add(message);
      addTearDown(() => debugPrint = previous);
      registrar.nextRegisterError = PlatformException(
        code: 'hotkey_registration_failed',
        details: {'osStatus': -50},
      );

      await controller.initialize();

      expect(controller.startFailed, isTrue);
      expect(controller.shortcutRefusal, isNull);
      expect(
        controller.errorMessage,
        'Quick Paste could not start, so its shortcut does not work.',
      );
      expect(logs.single, contains('osStatus: -50'));

      await controller.retry();

      expect(controller.startFailed, isFalse);
      expect(controller.errorMessage, isNull);
      expect(registrar.registered, DesktopShortcut.defaultForPlatform());
    });
  });

  testWidgets('a shortcut in use is a warning in the shortcut row', (
    tester,
  ) async {
    final registrar = _Registrar()
      ..nextRegisterError = PlatformException(
        code: 'hotkey_registration_failed',
        details: {'osStatus': -9878},
      );
    final controller = QuickPasteSettingsController(
      store: MemoryQuickPastePreferencesStore(),
      registrar: registrar,
      windowHost: _WindowHost()..permissionGranted = true,
    );
    await controller.initialize();
    addTearDown(controller.dispose);
    final settingsController = SettingsController(
      screenshotProtection: FakeScreenshotProtection(),
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    await settingsController.initialize();
    addTearDown(settingsController.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        home: SettingsScreen(
          controller: settingsController,
          quickPaste: controller,
        ),
      ),
    );
    await tester.pump();
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey<String>('settings-section-quick-paste')),
        matching: find.text('Quick Paste'),
      ),
    );
    await tester.pump();

    final shortcut = DesktopPresence.shortcutText(
      DesktopShortcut.defaultForPlatform().displayKeys,
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey<String>('settings-row-Open Quick Paste')),
        matching: find.textContaining(
          '$shortcut is used by another app. Choose another shortcut.',
          findRichText: true,
        ),
      ),
      findsOneWidget,
    );
    expect(
      find.byWidgetPredicate((widget) => widget is Alert && widget.destructive),
      findsNothing,
    );
    expect(tester.takeException(), isNull);
  });

  test('rejects modifier-only shortcuts', () {
    const shortcut = DesktopShortcut(
      key: PhysicalKeyboardKey.shiftLeft,
      modifiers: [DesktopShortcutModifier.shift],
    );
    expect(shortcut.isValid, isFalse);
  });

  testWidgets('renders the desktop shortcut and auto-paste setting', (
    tester,
  ) async {
    final controller = QuickPasteSettingsController(
      store: MemoryQuickPastePreferencesStore(),
      registrar: _Registrar(),
      windowHost: _WindowHost()..permissionGranted = true,
    );
    await controller.initialize();
    addTearDown(controller.dispose);
    final settingsController = SettingsController(
      screenshotProtection: FakeScreenshotProtection(),
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    await settingsController.initialize();
    addTearDown(settingsController.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        home: SettingsScreen(
          controller: settingsController,
          quickPaste: controller,
        ),
      ),
    );
    await tester.pump();

    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey<String>('settings-section-quick-paste')),
        matching: find.text('Quick Paste'),
      ),
    );
    await tester.pump();

    expect(find.text('Quick Paste'), findsWidgets);
    expect(find.text('Open Quick Paste'), findsOneWidget);
    expect(find.text('Paste automatically'), findsOneWidget);
    expect(find.byType(KeyboardDisplay), findsOneWidget);
    expect(find.byType(Switch), findsWidgets);

    await tester.ensureVisible(find.text('Paste automatically'));
    final pasteCard = find.ancestor(
      of: find.text('Paste automatically'),
      matching: find.byType(Card),
    );
    await tester.tap(
      find.descendant(of: pasteCard, matching: find.byType(Switch)),
    );
    await tester.pump();
    expect(controller.autoPaste, isFalse);
    expect(tester.takeException(), isNull);
  });

  test(
    'clears the shortcut when Linux capability is lost during initialization',
    () async {
      final integration =
          _LinuxIntegration(
              const LinuxIntegrationStatus(
                session: LinuxDesktopSession.wayland,
                globalShortcuts: true,
                remoteDesktop: LinuxRemoteDesktopState.consentRequired,
                companion: LinuxCompanionState.active,
                clipboard: true,
                quickPaste: false,
                screenshotProtection: false,
              ),
            )
            ..statusSequence.addAll(<LinuxIntegrationStatus>[
              const LinuxIntegrationStatus(
                session: LinuxDesktopSession.wayland,
                globalShortcuts: true,
                remoteDesktop: LinuxRemoteDesktopState.consentRequired,
                companion: LinuxCompanionState.active,
                clipboard: true,
                quickPaste: false,
                screenshotProtection: false,
              ),
              const LinuxIntegrationStatus(
                session: LinuxDesktopSession.wayland,
                globalShortcuts: false,
                remoteDesktop: LinuxRemoteDesktopState.consentRequired,
                companion: LinuxCompanionState.active,
                clipboard: true,
                quickPaste: false,
                screenshotProtection: false,
              ),
            ]);
      final registrar = _Registrar();
      final controller = QuickPasteSettingsController(
        store: MemoryQuickPastePreferencesStore(),
        registrar: registrar,
        windowHost: _WindowHost(),
        linuxIntegration: integration,
      );
      addTearDown(controller.dispose);

      await controller.initialize();
      expect(controller.supported, isFalse);
      expect(controller.linuxIntegration?.quickPaste, isFalse);
      expect(registrar.registered, isNull);
      expect(registrar.registerCalls, 1);
      expect(registrar.unregisterCalls, 1);
      expect(integration.statusReads, 2);

      await controller.refreshLinuxIntegration();
      expect(registrar.registerCalls, 1);
      expect(registrar.registered, isNull);
      expect(registrar.unregisterCalls, 1);

      integration.value = const LinuxIntegrationStatus(
        session: LinuxDesktopSession.wayland,
        globalShortcuts: true,
        remoteDesktop: LinuxRemoteDesktopState.active,
        companion: LinuxCompanionState.active,
        clipboard: true,
        quickPaste: true,
        screenshotProtection: false,
      );
      await controller.refreshLinuxIntegration();

      expect(controller.supported, isTrue);
      expect(controller.linuxIntegration?.quickPaste, isTrue);
      expect(registrar.registered, controller.shortcut);
      expect(registrar.registerCalls, 2);
    },
  );

  test('registers once while Linux execution readiness is pending', () async {
    final integration = _LinuxIntegration(
      const LinuxIntegrationStatus(
        session: LinuxDesktopSession.wayland,
        globalShortcuts: true,
        remoteDesktop: LinuxRemoteDesktopState.consentRequired,
        companion: LinuxCompanionState.active,
        clipboard: true,
        quickPaste: false,
        screenshotProtection: false,
      ),
    );
    final registrar = _Registrar();
    final controller = QuickPasteSettingsController(
      store: MemoryQuickPastePreferencesStore(),
      registrar: registrar,
      windowHost: _WindowHost(),
      linuxIntegration: integration,
    );
    addTearDown(controller.dispose);

    await controller.initialize();

    expect(controller.supported, isTrue);
    expect(controller.linuxIntegration?.quickPaste, isFalse);
    expect(registrar.registered, controller.shortcut);
    expect(registrar.registerCalls, 1);
    expect(integration.statusReads, 2);

    await controller.refreshLinuxIntegration();

    expect(controller.supported, isTrue);
    expect(registrar.registered, controller.shortcut);
    expect(registrar.registerCalls, 1);
  });

  test(
    'does not unregister before Linux shortcut capability is available',
    () async {
      final integration = _LinuxIntegration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.wayland,
          globalShortcuts: false,
          remoteDesktop: LinuxRemoteDesktopState.consentRequired,
          companion: LinuxCompanionState.active,
          clipboard: true,
          quickPaste: false,
          screenshotProtection: false,
        ),
      );
      final registrar = _Registrar();
      final controller = QuickPasteSettingsController(
        store: MemoryQuickPastePreferencesStore(),
        registrar: registrar,
        windowHost: _WindowHost(),
        linuxIntegration: integration,
      );
      addTearDown(controller.dispose);

      await controller.initialize();
      await controller.refreshLinuxIntegration();

      expect(controller.supported, isFalse);
      expect(registrar.registerCalls, 0);
      expect(registrar.unregisterCalls, 0);
    },
  );

  test('Linux setup actions refresh confirmed integration state', () async {
    final integration =
        _LinuxIntegration(
            const LinuxIntegrationStatus(
              session: LinuxDesktopSession.x11,
              globalShortcuts: true,
              remoteDesktop: LinuxRemoteDesktopState.consentRequired,
              companion: LinuxCompanionState.disabled,
              clipboard: false,
              quickPaste: false,
              screenshotProtection: false,
            ),
          )
          ..remoteDesktopResult = true
          ..companionSetupResult = true;
    final controller = QuickPasteSettingsController(
      store: MemoryQuickPastePreferencesStore(),
      registrar: _Registrar(),
      windowHost: _WindowHost(),
      linuxIntegration: integration,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    expect(await controller.requestLinuxRemoteDesktop(), isFalse);
    expect(
      controller.errorMessage,
      'Clipboard input permission was not granted.',
    );
    expect(await controller.openLinuxCompanionSetup(), isTrue);
    expect(integration.remoteDesktopRequests, 1);
    expect(integration.companionSetupRequests, 1);
  });
}

class _Registrar implements DesktopShortcutRegistrar {
  bool failRegister = false;
  Object? nextRegisterError;
  DesktopShortcut? registered;
  int registerCalls = 0;
  int unregisterCalls = 0;

  @override
  Future<void> register(
    String id,
    DesktopShortcut shortcut,
    Future<void> Function() callback, {
    required String description,
  }) async {
    if (failRegister) throw PlatformException(code: 'registration_failed');
    if (nextRegisterError case final error?) {
      nextRegisterError = null;
      throw error;
    }
    registerCalls += 1;
    registered = shortcut;
  }

  @override
  Future<void> unregister(String id) async {
    unregisterCalls += 1;
    registered = null;
  }
}

class _WindowHost implements QuickPasteWindowHost {
  bool supported = true;
  bool failPermission = false;
  int prepareCalls = 0;
  int permissionRequests = 0;
  bool permissionGranted = false;

  @override
  Future<bool> accessibilityGranted() async => permissionGranted;

  @override
  Future<void> dispose() async {}

  @override
  Future<bool> isSupported() async => supported;

  @override
  Future<void> open() async {}

  @override
  Future<void> prepare() async {
    prepareCalls += 1;
  }

  @override
  Future<bool> requestAccessibility() async {
    if (failPermission) throw PlatformException(code: 'permission_failed');
    permissionRequests += 1;
    permissionGranted = true;
    return true;
  }

  @override
  void setOpenSettingsHandler(VoidCallback? handler) {}
}

class _LinuxIntegration implements LinuxIntegrationPort {
  _LinuxIntegration(this.value);

  LinuxIntegrationStatus value;
  bool remoteDesktopResult = false;
  bool companionSetupResult = false;
  int remoteDesktopRequests = 0;
  int companionSetupRequests = 0;
  int statusReads = 0;
  final List<LinuxIntegrationStatus> statusSequence =
      <LinuxIntegrationStatus>[];

  @override
  Future<bool> openCompanionSetup() async {
    companionSetupRequests += 1;
    return companionSetupResult;
  }

  @override
  Future<bool> requestRemoteDesktop() async {
    remoteDesktopRequests += 1;
    return remoteDesktopResult;
  }

  @override
  Future<bool> registerCopypasteUri() async => true;

  @override
  Future<bool> startAtLogin() async => value.startAtLogin;

  @override
  Future<bool> setStartAtLogin(bool enabled) async => true;

  @override
  Future<LinuxIntegrationStatus> status() async {
    statusReads += 1;
    if (statusSequence.isNotEmpty) {
      value = statusSequence.removeAt(0);
    }
    return value;
  }
}
