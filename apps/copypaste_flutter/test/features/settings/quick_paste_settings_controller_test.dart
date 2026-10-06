import 'package:copypaste_flutter/features/settings/controller/quick_paste_settings_controller.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/repository/quick_paste_preferences_store.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/platform/desktop/global_shortcut.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
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
}

class _Registrar implements DesktopShortcutRegistrar {
  DesktopShortcut? registered;

  @override
  Future<void> register(
    DesktopShortcut shortcut,
    Future<void> Function() callback,
  ) async {
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
