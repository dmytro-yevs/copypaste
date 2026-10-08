import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';
import 'modules_test_support.dart';

void main() {
  for (final (width, height, scale, many) in [
    (1000.0, 700.0, 1.0, false),
    (320.0, 600.0, 1.6, true),
  ]) {
    testWidgets(
      'drawer keeps actions visible at width $width and text scale $scale',
      (tester) async {
        await tester.binding.setSurfaceSize(Size(width, height));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final module = InstalledModule(
          id: 'preview',
          title: many ? 'Text Tools' : 'OCR',
          description: 'Extract text from images.',
          version: '0.1.0',
          enabled: true,
          sizeBytes: 100,
          preferenceFields: [],
          preferences: {},
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
            theme: AppTheme.dark,
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
        final done = find.byKey(const ValueKey('module-settings-done-preview'));
        expect(tester.getBottomRight(done).dy, lessThan(height));
        expect(
          tester.getCenter(find.text('Done')).dx,
          closeTo(tester.getCenter(done).dx, 0.01),
        );
        if (many) {
          await tester.ensureVisible(find.text('Module command 7'));
          await tester.pumpAndSettle();
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
        await tester.tap(done);
        await tester.pumpAndSettle();
        expect(
          find.byKey(const ValueKey('module-settings-drawer-preview')),
          findsNothing,
        );
      },
    );
  }
}
