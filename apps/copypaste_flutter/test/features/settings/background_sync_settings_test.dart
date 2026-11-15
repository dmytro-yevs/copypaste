import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/platform/android/background_sync.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  testWidgets('background sync switch starts sync and reports a stop', (
    tester,
  ) async {
    final sync = _BackgroundSync();
    final settings = SettingsController(
      repository: FakeSettingsRepository(),
      filePicker: FakeSettingsFilePicker(),
      notifications: FakeCaptureNotificationPort(),
      screenshotProtection: FakeScreenshotProtection(),
      backgroundSync: sync,
      captureRefreshInterval: Duration.zero,
    );
    addTearDown(settings.dispose);
    await settings.initialize();
    await tester.binding.setSurfaceSize(const Size(390, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(child: SettingsScreen(controller: settings)),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('mobile-settings-section-sync')),
    );
    await tester.pumpAndSettle();

    final toggle = find.byKey(const ValueKey('background-sync-switch'));
    await tester.ensureVisible(toggle);
    await tester.pumpAndSettle();
    await tester.tap(toggle);
    await tester.pumpAndSettle();
    expect(sync.current.running, isTrue);
    expect(find.text('Background sync is not running'), findsNothing);

    sync.current = const BackgroundSyncStatus(enabled: true);
    settings.setPresentationActive(false);
    settings.setPresentationActive(true);
    await tester.pumpAndSettle();
    expect(find.text('Background sync is not running'), findsOneWidget);
  });
}

class _BackgroundSync implements BackgroundSync {
  BackgroundSyncStatus current = const BackgroundSyncStatus();

  @override
  bool get supported => true;

  @override
  Future<BackgroundSyncStatus> status() async => current;

  @override
  Future<BackgroundSyncStatus> setEnabled(bool enabled) async {
    current = BackgroundSyncStatus(
      enabled: enabled,
      running: enabled,
      notificationGranted: true,
    );
    return current;
  }
}
