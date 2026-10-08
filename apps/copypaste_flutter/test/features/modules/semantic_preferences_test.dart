import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';
import 'modules_test_support.dart';

void main() {
  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.android,
  ]) {
    testWidgets(
      'language multi-select picks one required model and saves the selection',
      (tester) async {
        await tester.binding.setSurfaceSize(
          Size(platform == TargetPlatform.android ? 360 : 1000, 900),
        );
        addTearDown(() => tester.binding.setSurfaceSize(null));
        const module = InstalledModule(
          id: 'copypaste.semantic-search',
          title: 'Semantic Search',
          description: 'Find clips by meaning.',
          version: '0.1.0',
          enabled: false,
          sizeBytes: 10,
          commands: [],
          preferences: {'languages': <String>[]},
          searchLanguageField: 'languages',
          searchModels: [
            ModuleSearchModel(
              id: 'en',
              title: 'English',
              languages: ['en'],
              sizeBytes: 23684031,
              available: false,
            ),
            ModuleSearchModel(
              id: 'multi',
              title: 'Multilingual',
              languages: ['en', 'uk'],
              sizeBytes: 135390915,
              available: false,
            ),
          ],
          preferenceFields: [
            ModuleField(
              id: 'languages',
              title: 'Search languages',
              kind: ModuleFieldKind.choices,
              defaultValue: <String>[],
              required: true,
              options: [
                ModuleChoice(id: 'en', title: 'English'),
                ModuleChoice(id: 'uk', title: 'Ukrainian'),
              ],
            ),
          ],
        );
        final repository = MemoryModulesRepository()..modules = [module];
        final controller = ModulesController(
          repository: repository,
          marketplace: MemoryModuleMarketplace()..modules = [],
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        controller.selectSection(ModulesSection.installed);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.light,
            builder: AppTheme.builder,
            home: Scaffold(child: ModulesSettingsView(controller: controller)),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(
          find.byKey(
            const ValueKey('module-settings-copypaste.semantic-search'),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('Preferences'));
        await tester.pumpAndSettle();
        await tester.tap(
          find.byKey(const ValueKey('module-choices-languages')),
        );
        // Assert actual option readiness after the opening animation rather
        // than requiring every popup animation to stop scheduling frames.
        await tester.pump(const Duration(milliseconds: 300));
        expect(find.byType(SelectItemButton<String>), findsNWidgets(2));
        await tester.tap(find.text('English').last);
        await tester.pump(const Duration(milliseconds: 300));
        expect(find.text('English model · 22.6 MiB download'), findsOneWidget);
        await tester.tap(find.text('Ukrainian').last);
        await tester.pump(const Duration(milliseconds: 300));
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(
          find.text('Multilingual model · 129.1 MiB download'),
          findsOneWidget,
        );
        await tester.tap(find.text('Save'));
        await tester.pumpAndSettle();
        expect(repository.lastPreferences?['languages'], ['en', 'uk']);
        expect(tester.takeException(), isNull);
      },
      variant: TargetPlatformVariant({platform}),
    );
  }
}
