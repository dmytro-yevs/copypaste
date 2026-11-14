import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_controller.dart';
import 'package:copypaste_flutter/features/settings/controller/settings_navigation_state.dart';
import 'package:copypaste_flutter/features/settings/view/settings_screen.dart';
import 'package:copypaste_flutter/features/settings/models/settings_models.dart';
import 'package:copypaste_flutter/features/settings/view/settings_search_target.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'settings_test_support.dart';

void main() {
  testWidgets('settings search and category survive presentation unloading', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
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
    Widget app() => ShadcnApp(
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: AppTheme.mode,
      builder: AppTheme.builder,
      home: Scaffold(child: SettingsScreen(controller: controller)),
    );
    await tester.pumpWidget(app());
    await tester.pump(const Duration(milliseconds: 500));
    expect(controller.loadState, SettingsLoadState.ready);
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('settings-section-privacy')),
        matching: find.text('Privacy'),
      ),
    );
    await tester.pump(const Duration(milliseconds: 500));
    expect(controller.navigation.section, SettingsSectionId.privacy);
    await tester.enterText(
      find.byKey(const ValueKey('settings-search')),
      'Screen',
    );
    // Editable fields keep scheduling cursor frames while focused.
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pumpWidget(app());
    await tester.pump(const Duration(milliseconds: 500));
    final field = tester.widget<TextField>(
      find.byKey(const ValueKey('settings-search')),
    );
    expect(field.controller!.text, 'Screen');
    expect(controller.navigation.section, SettingsSectionId.privacy);
    expect(controller.navigation.searchText, 'Screen');
    expect(await controller.toggleCapture(), isTrue);
    expect(
      repository.capture.paused,
      isTrue,
      reason: 'Unloading did not dispose background capture state.',
    );
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('feature sections need a contribution and a supported platform', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1400, 900));
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
    final snippetsKey = GlobalKey();
    final sections = [
      SettingsFeatureSection(
        section: SettingsSectionId.snippets,
        builder: (context, scope) =>
            const SizedBox(key: ValueKey('snippets-content')),
        searchTargets: (context) => [
          SettingsSearchTarget(
            id: 'result-snippet-library',
            section: SettingsSectionId.snippets,
            label: 'Snippet library',
            description: 'Saved texts',
            targetKey: snippetsKey,
          ),
        ],
      ),
      SettingsFeatureSection(
        section: SettingsSectionId.quickAccess,
        builder: (context, scope) => const SizedBox.shrink(),
      ),
    ];
    Future<void> pumpFor(TargetPlatform platform) async {
      debugDefaultTargetPlatformOverride = platform;
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: SettingsScreen(
              controller: controller,
              featureSections: sections,
            ),
          ),
        ),
      );
      await tester.pump(const Duration(milliseconds: 500));
    }

    Finder section(String slug) =>
        find.byKey(ValueKey('settings-section-$slug'));

    await pumpFor(TargetPlatform.macOS);
    expect(section('snippets'), findsOneWidget);
    expect(section('transforms'), findsNothing);
    expect(section('quick-access'), findsNothing);
    await tester.tap(
      find.descendant(of: section('snippets'), matching: find.text('Snippets')),
    );
    await tester.pump(const Duration(milliseconds: 500));
    expect(controller.navigation.section, SettingsSectionId.snippets);
    expect(find.byKey(const ValueKey('snippets-content')), findsOneWidget);
    await tester.enterText(
      find.byKey(const ValueKey('settings-search')),
      'library',
    );
    await tester.pump(const Duration(milliseconds: 500));
    expect(
      find.byKey(const ValueKey('settings-result-snippet-library')),
      findsOneWidget,
    );
    await tester.enterText(find.byKey(const ValueKey('settings-search')), '');
    await tester.pump(const Duration(milliseconds: 500));

    await pumpFor(TargetPlatform.android);
    expect(section('quick-access'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
    debugDefaultTargetPlatformOverride = null;
  });
}
