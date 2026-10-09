import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:pub_semver/pub_semver.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';

import 'modules_test_support.dart';

void main() {
  for (final (platform, width) in [
    (TargetPlatform.android, 320.0),
    (TargetPlatform.macOS, 1000.0),
    (TargetPlatform.windows, 1000.0),
  ]) {
    testWidgets('OCR uses the same management card in both sections', (
      tester,
    ) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = Size(width, 900);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      const ocr = InstalledModule(
        id: 'copypaste.ocr',
        title: 'OCR',
        description: 'Recognize text from images.',
        version: '0.1.0',
        enabled: true,
        sizeBytes: 10,
        supportedPlatforms: ModulePlatform.values,
        commands: [
          ModuleCommand(
            id: 'recognize-image',
            title: 'Recognize image text',
            description: '',
            arguments: [],
          ),
        ],
        preferenceFields: [],
        preferences: {},
      );
      final repository = MemoryModulesRepository()..modules = [ocr];
      final marketplace = MemoryModuleMarketplace()
        ..modules = [
          MarketplaceModule(
            id: ocr.id,
            title: ocr.title,
            description: ocr.description,
            version: Version.parse(ocr.version),
            artifact: testMarketplaceModule.artifact,
            supportedPlatforms: ModulePlatform.values,
          ),
        ];
      final controller = ModulesController(
        repository: repository,
        marketplace: marketplace,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: SingleChildScrollView(
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: ModulesSettingsView(controller: controller),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final card = find.byKey(const ValueKey('module-copypaste.ocr'));
      final remove = find.byKey(const ValueKey('module-remove-copypaste.ocr'));
      final settings = find.byKey(
        const ValueKey('module-settings-copypaste.ocr'),
      );
      final marketplaceSize = tester.getSize(card);
      expect(
        find.descendant(of: card, matching: find.byType(Button)),
        findsNWidgets(2),
      );
      expect(tester.widget<Button>(remove).child, isA<Icon>());
      expect(tester.widget<Button>(settings).child, isA<Icon>());
      expect(find.text('Recognize image text'), findsNothing);
      expect(find.byType(Switch), findsNothing);
      await tester.tap(find.text('Installed'));
      await tester.pumpAndSettle();
      expect(tester.getSize(card), marketplaceSize);
      expect(
        find.descendant(of: card, matching: find.byType(Button)),
        findsNWidgets(2),
      );
      await tester.tap(settings);
      await tester.pumpAndSettle();
      expect(find.text('OCR settings'), findsOneWidget);
      final drawer = find.byKey(
        const ValueKey('module-settings-drawer-copypaste.ocr'),
      );
      expect(
        tester.getSize(drawer).width,
        closeTo(width >= 800 ? AppOverlaySize.drawerPanelWidth : width, 2),
      );
      expect(
        tester.getSize(drawer).height,
        closeTo(
          MediaQuery.sizeOf(tester.element(drawer)).height *
              (width >= 800 ? 1 : AppOverlaySize.drawerHeightFactor),
          2,
        ),
      );
      final settingsIcon = find.descendant(
        of: drawer,
        matching: find.byIcon(LucideIcons.settings),
      );
      final title = find.text('OCR settings');
      final close = find.byKey(
        const ValueKey('module-settings-close-copypaste.ocr'),
      );
      expect(
        tester.getCenter(settingsIcon).dx,
        lessThan(tester.getTopLeft(title).dx),
      );
      expect(
        tester.getCenter(close).dx,
        greaterThan(tester.getBottomRight(title).dx),
      );
      expect(
        tester.getBottomRight(close).dy,
        lessThan(tester.getBottomRight(drawer).dy),
      );
      final enabledIcon = find.descendant(
        of: drawer,
        matching: find.byIcon(LucideIcons.power),
      );
      final done = find.byKey(
        const ValueKey('module-settings-done-copypaste.ocr'),
      );
      expect(
        tester.getCenter(find.text('Done')).dx,
        closeTo(tester.getCenter(done).dx, 0.01),
      );
      final enabledCenter = tester.getCenter(find.text('Enabled')).dy;
      expect(tester.getCenter(enabledIcon).dy, closeTo(enabledCenter, 0.01));
      expect(
        tester.getCenter(find.byType(Switch)).dy,
        closeTo(enabledCenter, 0.01),
      );
      expect(find.text('Recognize image text'), findsNothing);
      expect(find.byType(Switch), findsOneWidget);
      await tester.tap(find.byType(Switch));
      await tester.pumpAndSettle();
      expect(repository.calls, ['enabled:false']);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      expect(drawer, findsNothing);
      expect(tester.takeException(), isNull);
    }, variant: TargetPlatformVariant({platform}));
  }

  for (final (platform, width) in [
    (TargetPlatform.android, 320.0),
    (TargetPlatform.macOS, 650.0),
    (TargetPlatform.windows, 1000.0),
  ]) {
    for (final scale in [1.0, 1.6, 2.0]) {
      testWidgets('module sizes and actions align at $width with scale $scale', (
        tester,
      ) async {
        await tester.binding.setSurfaceSize(Size(width, 1600));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final marketplace = MemoryModuleMarketplace()
          ..modules = [
            for (var index = 0; index < 4; index++)
              MarketplaceModule(
                id: index == 0 ? testModule.id : 'module-$index',
                title: index == 0 ? testModule.title : 'Module $index',
                description: index == 1
                    ? 'Automatically copy login, verification, and transaction codes from new SMS messages.'
                    : testModule.description,
                version: testMarketplaceModule.version,
                artifact: index == 1 ? null : testMarketplaceModule.artifact,
                supportedPlatforms: index == 1
                    ? [ModulePlatform.android]
                    : ModulePlatform.values,
                appVersions: VersionConstraint.parse('>=1.0.6 <2.0.0'),
                availability: index == 1
                    ? ModuleAvailability.platform
                    : ModuleAvailability.available,
                unavailableReason: index == 1
                    ? 'Not available for this device.'
                    : null,
              ),
          ];
        final controller = ModulesController(
          repository: MemoryModulesRepository()..modules = [testModule],
          marketplace: marketplace,
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: (context, child) => AppTheme.builder(
              context,
              MediaQuery(
                data: MediaQuery.of(
                  context,
                ).copyWith(textScaler: TextScaler.linear(scale)),
                child: child!,
              ),
            ),
            home: Scaffold(
              child: SingleChildScrollView(
                child: ModulesSettingsView(controller: controller),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        final ids = [testModule.id, 'module-1', 'module-2', 'module-3'];
        final size = tester.getSize(
          find.byKey(ValueKey('module-${ids.first}')),
        );
        for (final id in ids) {
          final card = find.byKey(ValueKey('module-$id'));
          expect(tester.getSize(card), size);
          if (id == 'module-1') {
            expect(find.byKey(ValueKey('module-install-$id')), findsNothing);
            expect(
              find.descendant(
                of: card,
                matching: find.text('Platforms: Android'),
              ),
              findsOneWidget,
            );
            continue;
          }
          expect(
            find.descendant(
              of: card,
              matching: find.text('Platforms: macOS · Windows · Android'),
            ),
            findsOneWidget,
          );
          final action = find.byKey(
            ValueKey(
              id == testModule.id
                  ? 'module-settings-$id'
                  : 'module-install-$id',
            ),
          );
          expect(
            tester.getBottomRight(card).dy - tester.getBottomRight(action).dy,
            closeTo(
              tester
                      .getBottomRight(
                        find.byKey(ValueKey('module-${ids.first}')),
                      )
                      .dy -
                  tester
                      .getBottomRight(
                        find.byKey(ValueKey('module-settings-${ids.first}')),
                      )
                      .dy,
              0.01,
            ),
          );
        }
        expect(find.text('Not available for this device.'), findsOneWidget);
        expect(find.text('Unavailable'), findsNothing);
        expect(
          find.byKey(const ValueKey('module-install-module-1')),
          findsNothing,
        );
        expect(tester.takeException(), isNull);
      }, variant: TargetPlatformVariant({platform}));
    }
  }

  testWidgets('restart-required removal is managed through settings', (
    tester,
  ) async {
    final module = InstalledModule(
      id: testModule.id,
      title: testModule.title,
      description: testModule.description,
      version: testModule.version,
      enabled: false,
      sizeBytes: testModule.sizeBytes,
      commands: const [],
      preferenceFields: const [],
      preferences: const {},
      restartRequired: true,
    );
    var restarts = 0;
    final controller = ModulesController(
      repository: MemoryModulesRepository()..modules = [module],
      marketplace: MemoryModuleMarketplace(),
      restart: () async {
        restarts++;
      },
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.selectSection(ModulesSection.installed);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: SingleChildScrollView(
            child: ModulesSettingsView(controller: controller),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(
      tester
          .widget<Button>(
            find.byKey(const ValueKey('module-remove-copypaste.text-tools')),
          )
          .onPressed,
      isNull,
    );
    await tester.tap(
      find.byKey(const ValueKey('module-settings-copypaste.text-tools')),
    );
    await tester.pumpAndSettle();
    expect(tester.widget<Switch>(find.byType(Switch)).onChanged, isNull);
    await tester.tap(find.text('Restart CopyPaste'));
    await tester.pumpAndSettle();
    expect(restarts, 1);
  });

  testWidgets('an installed module is updated from settings', (tester) async {
    final repository = MemoryModulesRepository()..modules = [testModule];
    final marketplace = MemoryModuleMarketplace()
      ..modules = [
        MarketplaceModule(
          id: testModule.id,
          title: testModule.title,
          description: testModule.description,
          version: Version.parse('1.1.0'),
          artifact: testMarketplaceModule.artifact,
        ),
      ];
    final controller = ModulesController(
      repository: repository,
      marketplace: marketplace,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: SingleChildScrollView(
            child: ModulesSettingsView(controller: controller),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Update to 1.1.0'), findsNothing);
    await tester.tap(
      find.byKey(const ValueKey('module-settings-copypaste.text-tools')),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Update to 1.1.0'));
    await tester.pumpAndSettle();
    expect(repository.calls, ['install']);
    expect(marketplace.disposedPackages, 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'unsupported versions retain requirements and omit installation',
    (tester) async {
      final marketplace = MemoryModuleMarketplace()
        ..modules = [
          MarketplaceModule(
            id: testMarketplaceModule.id,
            title: testMarketplaceModule.title,
            description: testMarketplaceModule.description,
            version: testMarketplaceModule.version,
            artifact: testMarketplaceModule.artifact,
            supportedPlatforms: ModulePlatform.values,
            appVersions: VersionConstraint.parse('>=1.0.6 <2.0.0'),
            availability: ModuleAvailability.systemVersion,
            unavailableReason: 'Requires macOS 14 or newer.',
            systemRequirement: 'macOS 14 or newer',
          ),
        ];
      final controller = ModulesController(
        repository: MemoryModulesRepository(),
        marketplace: marketplace,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: SingleChildScrollView(
              child: ModulesSettingsView(controller: controller),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Text Tools'), findsOneWidget);
      expect(find.text('macOS 14 or newer'), findsOneWidget);
      expect(find.text('Requires macOS 14 or newer.'), findsNothing);
      expect(find.text('CopyPaste ≥1.0.6, <2.0.0'), findsOneWidget);
      expect(find.text('Unavailable'), findsNothing);
      expect(find.text('Marketplace is unavailable'), findsNothing);
      expect(find.widgetWithText(Button, 'Install'), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );

  for (final width in [320.0, 1000.0]) {
    testWidgets('marketplace search and installation work at width $width', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(Size(width, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = MemoryModulesRepository();
      final marketplace = MemoryModuleMarketplace();
      final controller = ModulesController(
        repository: repository,
        marketplace: marketplace,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: SingleChildScrollView(
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: ModulesSettingsView(controller: controller),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Install or update module'), findsNothing);
      expect(find.text('Text Tools'), findsOneWidget);
      await tester.enterText(find.byType(TextField), 'not a module');
      await tester.pumpAndSettle();
      expect(find.text('No matching modules'), findsOneWidget);
      await tester.enterText(find.byType(TextField), 'transform');
      await tester.pumpAndSettle();
      expect(find.text('Text Tools'), findsOneWidget);
      final install = find.byKey(
        const ValueKey('module-install-copypaste.text-tools'),
      );
      final card = tester.widget<Card>(
        find.byKey(const ValueKey('module-copypaste.text-tools')),
      );
      expect(
        tester.getSize(install).width,
        tester.getSize(find.byWidget(card.child)).width,
      );
      await tester.tap(find.widgetWithText(Button, 'Install'));
      await tester.pumpAndSettle();
      expect(repository.calls, ['install']);
      expect(marketplace.disposedPackages, 1);
      expect(install, findsNothing);
      expect(
        find.byKey(const ValueKey('module-remove-copypaste.text-tools')),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('module-settings-copypaste.text-tools')),
        findsOneWidget,
      );
      final marketplaceCardSize = tester.getSize(
        find.byKey(const ValueKey('module-copypaste.text-tools')),
      );
      await tester.tap(find.text('Installed').first);
      await tester.pumpAndSettle();
      expect(find.text('Transform text'), findsNothing);
      expect(
        tester.getSize(
          find.byKey(const ValueKey('module-copypaste.text-tools')),
        ),
        marketplaceCardSize,
      );
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('catalog failure preserves installed module management', (
    tester,
  ) async {
    final repository = MemoryModulesRepository()..modules = [testModule];
    final marketplace = MemoryModuleMarketplace()
      ..failure = const ModulesException('Offline.');
    final controller = ModulesController(
      repository: repository,
      marketplace: marketplace,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: SingleChildScrollView(
            child: ModulesSettingsView(controller: controller),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Marketplace is unavailable'), findsOneWidget);
    await tester.tap(find.text('Installed'));
    await tester.pumpAndSettle();
    expect(find.text('Text Tools'), findsOneWidget);
    expect(find.text('Platforms: macOS · Windows · Android'), findsOneWidget);
    await tester.tap(
      find.byKey(const ValueKey('module-settings-copypaste.text-tools')),
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('module-enabled-copypaste.text-tools')),
    );
    await tester.pumpAndSettle();
    expect(repository.calls, ['enabled:false']);
    expect(tester.takeException(), isNull);
  });

  for (final width in [360.0, 1000.0]) {
    testWidgets('module commands and settings work at width $width', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(Size(width, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = MemoryModulesRepository()..modules = [testModule];
      final controller = ModulesController(
        repository: repository,
        marketplace: MemoryModuleMarketplace(),
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.selectSection(ModulesSection.installed);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light,
          builder: AppTheme.builder,
          home: Scaffold(
            child: SingleChildScrollView(
              child: ModulesSettingsView(controller: controller),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Text Tools'), findsOneWidget);
      await tester.tap(
        find.byKey(const ValueKey('module-settings-copypaste.text-tools')),
      );
      await tester.pumpAndSettle();
      expect(find.text('Text Tools settings'), findsOneWidget);
      expect(find.text('Uppercase'), findsOneWidget);
      expect(find.widgetWithText(Button, 'Preferences'), findsNothing);
      expect(find.byType(AlertDialog), findsNothing);
      await tester.tap(find.byType(Switch).last);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(repository.lastPreferences, {'uppercase': true});
      await tester.tap(find.text('Transform text'));
      await tester.pumpAndSettle();
      expect(
        tester.widget<Button>(find.widgetWithText(Button, 'Run')).onPressed,
        isNull,
      );
      await tester.enterText(find.byType(TextArea), 'Україна');
      await tester.pumpAndSettle();
      await tester.tap(find.text('Run'));
      await tester.pumpAndSettle();
      expect(repository.lastArguments, {'text': 'Україна'});
      expect(find.text('УКРАЇНА'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.tap(find.widgetWithText(Button, 'Done').last);
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Done'));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('module-remove-copypaste.text-tools')),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Remove').last);
      await tester.pumpAndSettle();
      expect(find.text('No modules installed'), findsOneWidget);
      expect(repository.calls, contains('remove'));
      expect(tester.takeException(), isNull);
    });
  }
}
