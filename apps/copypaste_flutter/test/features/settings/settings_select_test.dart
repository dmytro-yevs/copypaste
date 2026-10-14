import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  for (final dark in [false, true]) {
    for (final width in [360.0, 1200.0]) {
      testWidgets(
        'settings select opens and saves at width $width, dark $dark',
        (tester) async {
          await tester.binding.setSurfaceSize(Size(width, 900));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          final repository = FakeSettingsRepository();
          final controller = SettingsController(
            screenshotProtection: FakeScreenshotProtection(),
            repository: repository,
            filePicker: FakeSettingsFilePicker(),
            notifications: FakeCaptureNotificationPort(),
            captureRefreshInterval: Duration.zero,
          );
          await controller.initialize();
          addTearDown(controller.dispose);
          await tester.pumpWidget(
            ShadcnApp(
              theme: dark ? AppTheme.dark : AppTheme.light,
              builder: (context, child) => MediaQuery(
                data: MediaQuery.of(
                  context,
                ).copyWith(textScaler: TextScaler.linear(dark ? 2 : 1)),
                child: Builder(
                  builder: (context) => AppTheme.builder(context, child),
                ),
              ),
              home: Scaffold(child: SettingsScreen(controller: controller)),
            ),
          );
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 300));
          final category = find.byKey(
            const ValueKey('mobile-settings-section-clipboard'),
          );
          if (category.evaluate().isNotEmpty) {
            await tester.ensureVisible(category);
            await tester.tap(category);
            await tester.pump();
            await tester.pump(const Duration(milliseconds: 300));
          }
          final field = find.widgetWithText(Select<int>, 'Keep forever');
          await tester.ensureVisible(field);
          await tester.tap(field);
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 300));
          final popup = find.byType(SelectPopup<int>);
          final rect = tester.getRect(popup);
          expect(rect.width, closeTo(tester.getSize(field).width, 0.01));
          expect(rect.left, greaterThanOrEqualTo(0));
          expect(rect.right, lessThanOrEqualTo(width));
          for (final item in tester.widgetList<SelectItemButton<int>>(
            find.byType(SelectItemButton<int>),
          )) {
            final clickable = tester.widget<Clickable>(
              find
                  .descendant(
                    of: find.byWidget(item),
                    matching: find.byType(Clickable),
                  )
                  .first,
            );
            final decoration =
                clickable.decoration!.resolve(const {}) as BoxDecoration;
            expect(
              decoration.border?.top.style ?? BorderStyle.none,
              BorderStyle.none,
            );
          }
          await tester.tap(find.text('7 days').last);
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 300));
          expect(repository.currentSettings.retentionDays, 7);
          expect(popup, findsNothing);
          expect(tester.takeException(), isNull);
        },
        variant: TargetPlatformVariant({
          TargetPlatform.macOS,
          TargetPlatform.windows,
          TargetPlatform.android,
        }),
      );
    }
  }
}
