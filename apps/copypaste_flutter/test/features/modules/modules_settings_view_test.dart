import 'package:flutter_test/flutter_test.dart';
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
  testWidgets(
    'unsupported versions render a normal module card with installation disabled',
    (tester) async {
      final marketplace = MemoryModuleMarketplace()
        ..modules = [
          MarketplaceModule(
            id: testMarketplaceModule.id,
            title: testMarketplaceModule.title,
            description: testMarketplaceModule.description,
            version: testMarketplaceModule.version,
            artifact: testMarketplaceModule.artifact,
            appVersions: VersionConstraint.parse('>=1.0.6 <2.0.0'),
            availability: ModuleAvailability.systemVersion,
            unavailableReason: 'Requires macOS 14 or newer.',
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
      expect(find.text('Requires macOS 14 or newer.'), findsOneWidget);
      expect(find.text('Marketplace is unavailable'), findsNothing);
      expect(
        tester
            .widget<Button>(find.widgetWithText(Button, 'Unavailable'))
            .onPressed,
        isNull,
      );
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
      await tester.tap(find.widgetWithText(Button, 'Install'));
      await tester.pumpAndSettle();
      expect(repository.calls, ['install']);
      expect(marketplace.disposedPackages, 1);
      expect(
        tester
            .widget<Button>(find.widgetWithText(Button, 'Installed'))
            .onPressed,
        isNull,
      );
      await tester.tap(find.text('Installed').first);
      await tester.pumpAndSettle();
      expect(find.text('Transform text'), findsOneWidget);
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
    await tester.tap(find.byType(Switch));
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
      await tester.tap(find.text('Settings'));
      await tester.pumpAndSettle();
      expect(find.text('Text Tools settings'), findsOneWidget);
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
      await tester.tap(find.text('Done'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Remove'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Remove').last);
      await tester.pumpAndSettle();
      expect(find.text('No modules installed'), findsOneWidget);
      expect(repository.calls, contains('remove'));
      expect(tester.takeException(), isNull);
    });
  }
}
