import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  for (final width in [390.0, 1000.0]) {
    testWidgets(
      'instant clipboard is independent from history sync at $width',
      (tester) async {
        await tester.binding.setSurfaceSize(Size(width, 900));
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
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        final section = find.byKey(
          ValueKey<String>(
            width < 1000
                ? 'mobile-settings-section-sync'
                : 'settings-section-sync',
          ),
        );
        await tester.tap(
          find.descendant(of: section, matching: find.text('Sync')),
        );
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        final toggle = find.byKey(const ValueKey('instant-clipboard-switch'));
        expect(tester.widget<Switch>(toggle).value, isTrue);
        await tester.tap(toggle);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        expect(tester.widget<Switch>(toggle).value, isFalse);
        expect(repository.currentSettings.syncEnabled, isTrue);
        await controller.retry();
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        expect(tester.widget<Switch>(toggle).value, isFalse);
        await tester.tap(toggle);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        expect(repository.currentSettings.instantClipboard, isTrue);
      },
    );
  }
}
