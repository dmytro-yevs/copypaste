import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/platform/desktop/desktop_visibility_controller.dart';
import 'package:copypaste_flutter/platform/desktop/desktop_visibility_preferences.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  for (final platform in [TargetPlatform.macOS, TargetPlatform.windows]) {
    for (final width in [500.0, 1400.0]) {
      testWidgets('${platform.name} visibility settings at width $width', (
        tester,
      ) async {
        await tester.binding.setSurfaceSize(Size(width, 900));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final settings = _settingsController();
        addTearDown(settings.dispose);
        await settings.initialize();
        final host = _Host();
        final store = MemoryDesktopVisibilityStore();
        final visibility = DesktopVisibilityController(
          host: host,
          store: store,
        );
        addTearDown(visibility.dispose);
        await visibility.load();
        await tester.pumpWidget(_app(settings, visibility));
        await tester.pump(const Duration(milliseconds: 500));
        await tester.tap(find.text('General'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));
        expect(
          find.text(
            platform == TargetPlatform.macOS
                ? 'Show in Dock'
                : 'Show in taskbar',
          ),
          findsOneWidget,
        );
        expect(
          find.text(
            platform == TargetPlatform.macOS
                ? 'Show in menu bar'
                : 'Show in system tray',
          ),
          findsOneWidget,
        );
        final taskbar = find.byKey(const ValueKey('settings-show-in-taskbar'));
        final tray = find.byKey(const ValueKey('settings-show-in-tray'));
        expect(tester.widget<Switch>(taskbar).value, isTrue);
        expect(tester.widget<Switch>(tray).value, isTrue);
        await tester.tap(taskbar);
        await tester.pump(const Duration(milliseconds: 300));
        await tester.tap(tray);
        await tester.pump(const Duration(milliseconds: 300));
        expect(tester.widget<Switch>(taskbar).value, isFalse);
        expect(tester.widget<Switch>(tray).value, isFalse);
        expect(host.taskbar, isFalse);
        expect(host.tray, isFalse);
        expect(store.value.showInTaskbar, isFalse);
        expect(store.value.showInTray, isFalse);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      }, variant: TargetPlatformVariant({platform}));
    }
  }

  testWidgets('Android omits desktop settings and search results', (
    tester,
  ) async {
    final settings = _settingsController();
    addTearDown(settings.dispose);
    await settings.initialize();
    await tester.pumpWidget(_app(settings, null));
    await tester.pump(const Duration(milliseconds: 500));
    expect(
      find.byKey(const ValueKey('settings-section-general')),
      findsNothing,
    );
    await tester.enterText(
      find.byKey(const ValueKey('settings-search')),
      'Dock',
    );
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('No settings found'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('settings-show-in-taskbar')),
      findsNothing,
    );
    await tester.pumpWidget(const SizedBox.shrink());
  }, variant: TargetPlatformVariant({TargetPlatform.android}));

  testWidgets(
    'desktop visibility is searchable and native errors remain visible',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(1400, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final settings = _settingsController();
      addTearDown(settings.dispose);
      await settings.initialize();
      final host = _Host()..failTray = true;
      final visibility = DesktopVisibilityController(
        host: host,
        store: MemoryDesktopVisibilityStore(),
      );
      addTearDown(visibility.dispose);
      await tester.pumpWidget(_app(settings, visibility));
      await tester.pump(const Duration(milliseconds: 500));
      await tester.enterText(
        find.byKey(const ValueKey('settings-search')),
        'menu bar',
      );
      await tester.pump(const Duration(milliseconds: 500));
      await tester.tap(find.text('Show in menu bar'));
      await tester.pump(const Duration(milliseconds: 500));
      await tester.tap(find.byKey(const ValueKey('settings-show-in-tray')));
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.text('Visibility could not be changed'), findsOneWidget);
      expect(
        tester
            .widget<Switch>(find.byKey(const ValueKey('settings-show-in-tray')))
            .value,
        isTrue,
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
    variant: TargetPlatformVariant({TargetPlatform.macOS}),
  );
}

SettingsController _settingsController() => SettingsController(
  repository: FakeSettingsRepository(),
  filePicker: FakeSettingsFilePicker(),
  notifications: FakeCaptureNotificationPort(),
  screenshotProtection: FakeScreenshotProtection(),
  captureRefreshInterval: Duration.zero,
);

Widget _app(
  SettingsController controller,
  DesktopVisibilityController? visibility,
) => ShadcnApp(
  theme: AppTheme.light,
  darkTheme: AppTheme.dark,
  themeMode: AppTheme.mode,
  builder: AppTheme.builder,
  home: Scaffold(
    child: SettingsScreen(
      controller: controller,
      desktopVisibility: visibility,
    ),
  ),
);

class _Host implements DesktopVisibilityHost {
  bool taskbar = true;
  bool tray = true;
  bool failTray = false;

  @override
  Future<void> setTaskbarVisible(bool visible) async => taskbar = visible;

  @override
  Future<void> setTrayVisible(bool visible) async {
    if (failTray) throw StateError('Tray unavailable');
    tray = visible;
  }
}
