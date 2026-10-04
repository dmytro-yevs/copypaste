import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  test('loads and applies the selected backend settings', () async {
    final repository = FakeSettingsRepository();
    final picker = FakeSettingsFilePicker();
    final controller = SettingsController(
      repository: repository,
      filePicker: picker,
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(controller.dispose);

    await controller.initialize();
    expect(controller.loadState, SettingsLoadState.ready);
    expect(controller.capture?.paused, isFalse);

    expect(await controller.toggleCapture(), isTrue);
    expect(controller.capture?.paused, isTrue);
    expect(await controller.setRetentionDays(30), isTrue);
    expect(controller.settings?.retentionDays, 30);
    expect(await controller.addExcludedApp('com.example.private'), isTrue);
    expect(controller.settings?.excludedAppIds, ['com.example.private']);

    expect(await controller.exportTextHistory(), isTrue);
    expect(await controller.createBackup(), isTrue);
    expect(await controller.restoreBackup(), isTrue);
    expect(repository.exportCalls, 1);
    expect(repository.backupCalls, 1);
    expect(repository.restoreCalls, 1);
    expect(picker.presentedPaths, [picker.exportPath, picker.backupPath]);
  });

  test('posts capture feedback only after notification opt-in', () async {
    final repository = FakeSettingsRepository();
    final notifications = FakeCaptureNotificationPort();
    final controller = SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: notifications,
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    repository.captures.add(null);
    await Future<void>.delayed(Duration.zero);
    expect(notifications.notifications, 0);

    expect(await controller.setNotifyOnCopy(true), isTrue);
    repository.captures.add(null);
    await Future<void>.delayed(Duration.zero);
    expect(notifications.notifications, 1);
  });

  test('does not persist notifications when permission is denied', () async {
    final repository = FakeSettingsRepository();
    final notifications = FakeCaptureNotificationPort()
      ..permissionGranted = false;
    final controller = SettingsController(
      repository: repository,
      filePicker: FakeSettingsFilePicker(),
      notifications: notifications,
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    expect(await controller.setNotifyOnCopy(true), isFalse);
    expect(repository.currentSettings.notifyOnCopy, isFalse);
    expect(controller.errorMessage, contains('permission'));
  });

  testWidgets('renders one settings document with section anchors', (
    tester,
  ) async {
    final controller = SettingsController(
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    await controller.initialize();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      ShadcnApp(home: SettingsScreen(controller: controller)),
    );
    await tester.pump();

    for (final anchor in [
      'settings-anchor-capture',
      'settings-anchor-storage-data',
      'settings-anchor-sync',
      'settings-anchor-feedback',
    ]) {
      expect(find.byKey(ValueKey<String>(anchor)), findsOneWidget);
    }
    expect(find.text('Clipboard capture'), findsOneWidget);
    expect(find.text('Storage quota'), findsOneWidget);

    await tester.tap(
      find.byKey(const ValueKey<String>('settings-anchor-feedback')),
    );
    await tester.pumpAndSettle();
    expect(find.text('Notification on copy'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('keeps the settings document usable at narrow large-text sizes', (
    tester,
  ) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await tester.binding.setSurfaceSize(const Size(320, 480));
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    final controller = SettingsController(
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    await controller.initialize();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      ShadcnApp(home: SettingsScreen(controller: controller)),
    );
    await tester.pump();

    expect(
      find.byKey(const ValueKey<String>('settings-anchor-sync')),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });
}
