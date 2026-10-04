import 'package:copypaste_flutter/app/theme/app_motion.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/features/update/update.dart';
import 'package:copypaste_flutter/platform/update/app_update_platform.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:pub_semver/pub_semver.dart';

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

  testWidgets('renders desktop settings navigation with separate sections', (
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
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: AppTheme.mode,
        builder: AppTheme.builder,
        home: Scaffold(child: SettingsScreen(controller: controller)),
      ),
    );
    await tester.pump();

    expect(
      find.byKey(const ValueKey<String>('settings-navigation-sidebar')),
      findsOneWidget,
    );
    for (final section in [
      'settings-section-capture',
      'settings-section-storage-data',
      'settings-section-sync',
      'settings-section-feedback',
    ]) {
      expect(find.byKey(ValueKey<String>(section)), findsOneWidget);
    }
    expect(find.text('Clipboard capture'), findsOneWidget);
    expect(find.text('Storage quota'), findsNothing);

    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey<String>('settings-section-feedback')),
        matching: find.text('Feedback'),
      ),
    );
    await tester.pump();
    expect(find.text('Notification on copy'), findsOneWidget);
    expect(find.text('Clipboard capture'), findsNothing);

    final soundCard = find.ancestor(
      of: find.text('Sound on copy'),
      matching: find.byType(Card),
    );
    await tester.tap(
      find.descendant(of: soundCard, matching: find.byType(Switch)),
    );
    await tester.pump();

    expect(find.text('Done'), findsNothing);
    expect(find.text('Settings saved.'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('searches, categorizes, scrolls to, and highlights a setting', (
    tester,
  ) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.binding.setSurfaceSize(const Size(800, 280));
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

    await tester.enterText(
      find.byKey(const ValueKey<String>('settings-search')),
      'backup',
    );
    await tester.pump();

    final result = find.byKey(
      const ValueKey<String>('settings-result-history-files'),
    );
    expect(result, findsOneWidget);
    expect(
      find.descendant(of: result, matching: find.text('Storage & Data')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey<String>('settings-result-retention')),
      findsNothing,
    );

    await tester.tap(
      find.descendant(of: result, matching: find.text('History files')),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pump(AppMotion.standard);
    await tester.pump();

    expect(find.text('History files'), findsWidgets);
    expect(find.text('Clipboard capture'), findsNothing);
    final contentScroll = find.descendant(
      of: find.byKey(
        const PageStorageKey<String>('settings-storage-data-scroll'),
      ),
      matching: find.byType(Scrollable),
    );
    expect(
      tester.state<ScrollableState>(contentScroll).position.pixels,
      greaterThan(0),
    );
    expect(
      tester
          .widgetList<Card>(find.byType(Card))
          .where((card) => card.theme?.filled == true),
      hasLength(1),
    );

    await tester.pump(AppMotion.settingsHighlightHold);
    await tester.pump(AppMotion.quick);

    expect(
      tester
          .widgetList<Card>(find.byType(Card))
          .where((card) => card.theme?.filled == true),
      isEmpty,
    );

    await tester.enterText(
      find.byKey(const ValueKey<String>('settings-search')),
      'setting that does not exist',
    );
    await tester.pump();

    expect(find.text('No settings found'), findsOneWidget);
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
      find.byKey(const ValueKey<String>('settings-mobile-section-select')),
      findsOneWidget,
    );
    expect(find.byType(NavigationSidebar), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('switches settings sections from the mobile selector', (
    tester,
  ) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.binding.setSurfaceSize(const Size(390, 720));
    final controller = SettingsController(
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    await controller.initialize();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: AppTheme.mode,
        builder: AppTheme.builder,
        home: Scaffold(child: SettingsScreen(controller: controller)),
      ),
    );
    await tester.pump();

    final select = find.byKey(
      const ValueKey<String>('settings-mobile-section-select'),
    );
    await tester.tap(
      find.descendant(of: select, matching: find.text('Capture')),
    );
    await tester.pump(const Duration(milliseconds: 500));

    final storageOption = find.byKey(
      const ValueKey<String>('mobile-settings-section-storage-data'),
    );
    final storageLabel = find.descendant(
      of: storageOption,
      matching: find.text('Storage & Data'),
    );
    await tester.ensureVisible(storageLabel);
    await tester.pump();
    await tester.tap(storageLabel);
    await tester.pump();

    expect(find.text('Storage quota'), findsOneWidget);
    expect(find.text('Clipboard capture'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('offers and starts the same application update from settings', (
    tester,
  ) async {
    final settings = SettingsController(
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      captureRefreshInterval: Duration.zero,
    );
    final updater = AppUpdateController(
      repository: _SettingsUpdateRepository(),
      platform: _SettingsUpdatePlatform(),
    );
    await Future.wait([settings.initialize(), updater.initialize()]);
    addTearDown(settings.dispose);
    addTearDown(updater.dispose);

    await tester.pumpWidget(
      ShadcnApp(
        home: SettingsScreen(controller: settings, appUpdate: updater),
      ),
    );
    await tester.pump();
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey<String>('settings-section-feedback')),
        matching: find.text('Feedback'),
      ),
    );
    await tester.pump();
    await tester.ensureVisible(
      find.byKey(const ValueKey<String>('install-app-update')),
    );
    await tester.pump(const Duration(milliseconds: 500));

    expect(find.text('Update now'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey<String>('install-app-update')));
    await tester.pump();

    expect(
      find.text('Continue in the Android system installer.'),
      findsOneWidget,
    );
  });
}

class _SettingsUpdateRepository implements AppUpdateRepository {
  late final AppReleaseAsset asset = AppReleaseAsset(
    name: 'CopyPaste-v1.0.1-android.apk',
    downloadUri: Uri.parse(
      'https://github.com/dmytro-yevs/copypaste/releases/download/v1.0.1/CopyPaste-v1.0.1-android.apk',
    ),
    sha256: 'a' * 64,
    sizeBytes: 1024,
    signatureUri: Uri.parse(
      'https://github.com/dmytro-yevs/copypaste/releases/download/v1.0.1/CopyPaste-v1.0.1-android.apk.sig',
    ),
    signatureSha256: 'b' * 64,
    signatureSizeBytes: 512,
  );

  @override
  Future<DownloadedAppUpdate> download(
    AppRelease release, {
    required void Function(double progress) onProgress,
  }) async {
    onProgress(1);
    return DownloadedAppUpdate(path: '/tmp/${asset.name}', asset: asset);
  }

  @override
  Future<AppRelease?> findUpdate({
    required Version currentVersion,
    required AppUpdateTarget target,
  }) async => AppRelease(
    version: Version.parse('1.0.1'),
    releaseUri: Uri.parse(
      'https://github.com/dmytro-yevs/copypaste/releases/tag/v1.0.1',
    ),
    prerelease: false,
    asset: asset,
  );

  @override
  void dispose() {}
}

class _SettingsUpdatePlatform implements AppUpdatePlatform {
  @override
  AppUpdateTarget get target => AppUpdateTarget.android;

  @override
  Future<AppUpdateAvailability> availability() async =>
      const AppUpdateAvailability.available();

  @override
  Future<String> currentVersion() async => '1.0.0';

  @override
  Future<AppUpdateInstallResult> install({
    required AppRelease release,
    DownloadedAppUpdate? package,
  }) async => AppUpdateInstallResult.started;

  @override
  Future<void> openReleasePage(Uri uri) async {}
}
