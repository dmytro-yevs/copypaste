import 'package:copypaste_flutter/app/navigation/navigation.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  testWidgets('a typed relay address is saved and an invalid one is not', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1000, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = FakeSettingsRepository();
    final controller = SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: AppTheme.mode,
        builder: AppTheme.builder,
        home: Scaffold(child: SettingsScreen(controller: controller)),
      ),
    );
    await tester.pump(const Duration(milliseconds: 500));
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey<String>('settings-section-sync')),
        matching: find.text('Sync'),
      ),
    );
    await tester.pump(const Duration(milliseconds: 500));
    final field = find.byKey(const ValueKey<String>('settings-relay-url-'));
    final save = find.byKey(const ValueKey<String>('settings-relay-save'));
    await tester.ensureVisible(field);

    await tester.enterText(field, 'https://relay.example.com');
    await tester.pump();
    expect(tester.widget<Button>(save).onPressed, isNull);

    await tester.enterText(field, 'wss://relay.example.com');
    await tester.pump();
    await tester.tap(save);
    await tester.pump(const Duration(milliseconds: 500));

    expect(repository.currentSettings.relayUrl, 'wss://relay.example.com');
  });

  testWidgets('an unsaved relay address keeps the compact Sync page open', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = SettingsController(
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    final navigation = AppNavigationController();
    addTearDown(navigation.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: AppNavigationHost(
          controller: navigation,
          child: Scaffold(child: SettingsScreen(controller: controller)),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('mobile-settings-section-sync')),
    );
    await tester.pumpAndSettle();
    expect(navigation.overlaysPassive, isTrue);

    final field = find.byKey(const ValueKey<String>('settings-relay-url-'));
    await tester.ensureVisible(field);
    await tester.enterText(field, 'wss://relay.example.com');
    await tester.pump();
    expect(navigation.overlaysPassive, isFalse);

    await tester.enterText(field, '');
    await tester.pump();
    expect(navigation.overlaysPassive, isTrue);
  });
}
