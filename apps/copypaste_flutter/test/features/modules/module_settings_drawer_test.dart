import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';

import 'modules_test_support.dart';

void main() {
  testWidgets(
    'inline edits survive refresh and failed save until drawer closes',
    (tester) async {
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
          home: Scaffold(child: ModulesSettingsView(controller: controller)),
        ),
      );
      await tester.pumpAndSettle();
      final settings = find.byKey(
        const ValueKey('module-settings-copypaste.text-tools'),
      );
      await tester.tap(settings);
      await tester.pumpAndSettle();
      await tester.tap(find.byType(Switch).last);
      await tester.pumpAndSettle();
      expect(repository.lastPreferences, isNull);

      // An independent module operation must not reset unsaved preferences.
      await tester.tap(
        find.byKey(const ValueKey('module-enabled-copypaste.text-tools')),
      );
      await tester.pumpAndSettle();
      expect(tester.widget<Switch>(find.byType(Switch).last).value, isTrue);

      repository.failure = const ModulesException(
        'Preferences could not be saved.',
      );
      await tester.tap(find.widgetWithText(Button, 'Save'));
      await tester.pumpAndSettle();
      expect(
        find.descendant(
          of: find.byKey(
            const ValueKey('module-settings-drawer-copypaste.text-tools'),
          ),
          matching: find.text('Preferences could not be saved.'),
        ),
        findsOneWidget,
      );
      expect(find.byType(AlertDialog), findsNothing);
      expect(tester.widget<Switch>(find.byType(Switch).last).value, isTrue);
      expect(repository.lastPreferences, isNull);

      repository.failure = null;
      await tester.tap(find.widgetWithText(Button, 'Save'));
      await tester.pumpAndSettle();
      expect(repository.lastPreferences, {'uppercase': true});
      expect(find.text('Preferences could not be saved.'), findsNothing);
      expect(
        find.byKey(
          const ValueKey('module-settings-drawer-copypaste.text-tools'),
        ),
        findsOneWidget,
      );

      await tester.tap(find.widgetWithText(Button, 'Done'));
      await tester.pumpAndSettle();
      await tester.tap(settings);
      await tester.pumpAndSettle();
      // The in-memory repository retains its original values; reopening must
      // create a new draft from the repository rather than reuse the old draft.
      expect(tester.widget<Switch>(find.byType(Switch).last).value, isFalse);
      await tester.tap(find.byType(Switch).last);
      await tester.pumpAndSettle();
      final saveCount = repository.calls
          .where((call) => call == 'preferences')
          .length;
      await tester.tap(
        find.byKey(
          const ValueKey('module-settings-close-copypaste.text-tools'),
        ),
      );
      await tester.pumpAndSettle();
      expect(
        repository.calls.where((call) => call == 'preferences').length,
        saveCount,
      );
      expect(tester.takeException(), isNull);
    },
  );

  for (final (platform, width, height, scale, many) in [
    for (final platform in [
      TargetPlatform.macOS,
      TargetPlatform.android,
      TargetPlatform.windows,
    ]) ...[
      (platform, 1000.0, 700.0, 1.0, false),
      (platform, 320.0, 600.0, 1.6, true),
      (platform, 320.0, 600.0, 2.0, true),
    ],
  ]) {
    testWidgets(
      'drawer keeps actions visible on $platform at width $width and text scale $scale',
      (tester) async {
        tester.view.devicePixelRatio = 1;
        tester.view.physicalSize = Size(width, height);
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final module = InstalledModule(
          id: 'preview',
          title: many ? 'Text Tools' : 'OCR',
          description: 'Extract text from images.',
          version: '0.1.0',
          enabled: true,
          sizeBytes: 100,
          preferenceFields: many ? [uppercasePreference] : [],
          preferences: many ? {'uppercase': false} : {},
          commands: [
            if (many)
              for (var i = 0; i < 8; i++)
                ModuleCommand(
                  id: 'command-$i',
                  title: 'Module command $i',
                  description: '',
                  arguments: [],
                ),
          ],
        );
        final controller = ModulesController(
          repository: MemoryModulesRepository()..modules = [module],
          marketplace: MemoryModuleMarketplace(),
        );
        addTearDown(controller.dispose);
        await controller.initialize();
        controller.selectSection(ModulesSection.installed);
        await tester.pumpWidget(
          ShadcnApp(
            theme: AppTheme.dark.copyWith(platform: () => platform),
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
        await tester.ensureVisible(
          find.byKey(const ValueKey('module-settings-preview')),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const ValueKey('module-settings-preview')));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        final drawer = find.byKey(
          const ValueKey('module-settings-drawer-preview'),
        );
        final drawerSize = tester.getSize(drawer);
        expect(
          drawerSize.width,
          closeTo(width >= 800 ? AppOverlaySize.drawerPanelWidth : width, 2),
        );
        expect(
          drawerSize.height,
          closeTo(
            MediaQuery.sizeOf(tester.element(drawer)).height *
                (width >= 800 ? 1 : AppOverlaySize.drawerHeightFactor),
            2,
          ),
        );
        final close = find.byKey(
          const ValueKey('module-settings-close-preview'),
        );
        final headerPosition = tester.getTopLeft(close);
        final done = find.byKey(const ValueKey('module-settings-done-preview'));
        expect(tester.getBottomRight(done).dy, lessThan(height));
        expect(
          tester.getCenter(find.text('Done')).dx,
          closeTo(tester.getCenter(done).dx, 0.01),
        );
        if (many) {
          expect(find.text('Uppercase'), findsOneWidget);
          expect(find.widgetWithText(Button, 'Preferences'), findsNothing);
          expect(find.byType(AlertDialog), findsNothing);
          await tester.ensureVisible(find.widgetWithText(Button, 'Save'));
          await tester.pumpAndSettle();
          expect(tester.getBottomRight(done).dy, lessThan(height));
          await tester.ensureVisible(find.text('Module command 7'));
          await tester.pumpAndSettle();
          expect(tester.getTopLeft(close), headerPosition);
          expect(tester.getBottomRight(done).dy, lessThan(height));
          final command = find.widgetWithText(Button, 'Module command 7');
          final icon = find.descendant(
            of: command,
            matching: find.byIcon(LucideIcons.play),
          );
          expect(
            tester.getCenter(icon).dy,
            closeTo(tester.getCenter(find.text('Module command 7')).dy, 0.01),
          );
        }
        await tester.tap(many ? close : done);
        await tester.pumpAndSettle();
        expect(
          find.byKey(const ValueKey('module-settings-drawer-preview')),
          findsNothing,
        );
      },
    );
  }
}
