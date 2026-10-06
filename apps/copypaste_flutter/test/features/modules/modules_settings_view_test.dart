import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';
import 'modules_test_support.dart';

void main() {
  for (final width in [360.0, 1000.0]) {
    testWidgets('module commands and settings work at width $width', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(Size(width, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = MemoryModulesRepository()..modules = [testModule];
      final controller = ModulesController(
        repository: repository,
        picker: MemoryModulePicker(),
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
