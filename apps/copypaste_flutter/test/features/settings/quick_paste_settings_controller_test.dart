import 'package:copypaste_flutter/features/settings/controller/quick_paste_settings_controller.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
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

  test('registers once after Linux integration becomes active', () async {
    final integration = _LinuxIntegration(
      const LinuxIntegrationStatus(
        session: LinuxDesktopSession.wayland,
        globalShortcuts: true,
        remoteDesktop: LinuxRemoteDesktopState.consentRequired,
        companion: LinuxCompanionState.active,
        clipboard: false,
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
    expect(controller.supported, isFalse);
    expect(registrar.registered, isNull);

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
    expect(registrar.registered, controller.shortcut);
    expect(registrar.registerCalls, 1);
  });

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
  DesktopShortcut? registered;
  int registerCalls = 0;

  @override
  Future<void> register(
    DesktopShortcut shortcut,
    Future<void> Function() callback,
  ) async {
    registerCalls += 1;
    registered = shortcut;
  }

  @override
  Future<void> unregister() async {
    registered = null;
  }
}

class _WindowHost implements QuickPasteWindowHost {
  int prepareCalls = 0;
  int permissionRequests = 0;
  bool permissionGranted = false;

  @override
  Future<bool> accessibilityGranted() async => permissionGranted;

  @override
  Future<void> dispose() async {}

  @override
  Future<bool> isSupported() async => true;

  @override
  Future<void> open() async {}

  @override
  Future<void> prepare() async {
    prepareCalls += 1;
  }

  @override
  Future<bool> requestAccessibility() async {
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
  Future<bool> setStartAtLogin(bool enabled) async => true;

  @override
  Future<LinuxIntegrationStatus> status() async => value;
}
