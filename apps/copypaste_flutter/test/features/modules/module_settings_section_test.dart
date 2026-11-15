import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_surfaces.dart';
import 'package:copypaste_flutter/features/modules/models/module_view.dart';
import 'package:copypaste_flutter/features/modules/view/module_settings_section.dart';
import 'package:copypaste_flutter/shared/forms/dynamic_form_draft.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'modules_test_support.dart';

const _module = InstalledModule(
  id: 'example.library',
  title: 'Library',
  description: 'Saved texts.',
  version: '1.0.0',
  enabled: true,
  sizeBytes: 10,
  commands: [],
  preferenceFields: [],
  preferences: {},
  surfaces: ModuleSurfaces(
    settings: ModuleSettingsSurface(
      command: 'library',
      icon: ModuleIcon.notebookText,
    ),
  ),
);

ModuleView _library(List<String> ids) => ModuleView(
  sections: [
    ModuleViewSection(
      items: [
        for (final id in ids)
          ModuleViewRow(
            id: id,
            title: 'Entry $id',
            subtitle: 'Body $id',
            open: ModuleViewAction(
              title: 'Edit',
              command: 'edit',
              arguments: {'id': id},
              target: ModuleViewTarget.drawer,
            ),
          ),
      ],
      bulkActions: const [
        ModuleViewAction(title: 'Delete', command: 'delete', destructive: true),
      ],
    ),
  ],
);

const _editor = ModuleView(
  sections: [
    ModuleViewSection(
      items: [
        ModuleViewForm(
          fields: [
            DynamicFormField(
              id: 'title',
              title: 'Title',
              kind: DynamicFormFieldKind.line,
              defaultValue: '',
            ),
          ],
          values: {'title': 'Entry s2'},
          submit: ModuleViewAction(title: 'Save', command: 'save'),
        ),
      ],
    ),
  ],
);

void main() {
  testWidgets('a module section runs bulk actions and drawer editors', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 900);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    var ids = ['s1', 's2'];
    final arguments = <String, Map<String, Object>>{};
    final repository = MemoryModulesRepository()
      ..modules = [_module]
      ..results = (command, args) {
        arguments[command] = args;
        return switch (command) {
          'delete' => () {
            final removed = (args['ids']! as List).cast<String>();
            ids = [
              for (final id in ids)
                if (!removed.contains(id)) id,
            ];
            return const ModuleResult('Deleted.', message: true);
          }(),
          'edit' => const ModuleResult.view(_editor),
          'save' => const ModuleResult('Saved.', message: true),
          _ => ModuleResult.view(_library(ids)),
        };
      };
    final modules = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
    );
    addTearDown(modules.dispose);
    await modules.initialize();
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: SingleChildScrollView(
            child: ModuleSettingsSection(modules: modules, module: _module),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Entry s1'), findsOneWidget);
    expect(find.text('Body s2'), findsOneWidget);

    // Selected rows reach the bulk action as `ids`, after confirmation.
    await tester.tap(find.byKey(const ValueKey('module-view-select-s1')));
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('module-view-action-delete-Delete')),
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.descendant(
        of: find.byType(AlertDialog),
        matching: find.widgetWithText(Button, 'Delete'),
      ),
    );
    await tester.pumpAndSettle();
    expect(arguments['delete'], {
      'ids': ['s1'],
    });
    expect(find.text('Entry s1'), findsNothing);

    // A drawer target opens its view in a drawer, which closes when the
    // form's submit returns a message; the library runs again.
    await tester.tap(find.byKey(const ValueKey('module-view-row-s2')));
    await tester.pumpAndSettle();
    expect(arguments['edit'], {'id': 's2'});
    await tester.enterText(find.byKey(const ValueKey('title')), 'Renamed');
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('module-form-submit-save')));
    await tester.pumpAndSettle();
    expect(arguments['save'], {
      'values': {'title': 'Renamed'},
    });
    expect(find.byKey(const ValueKey('module-form-submit-save')), findsNothing);
    expect(repository.calls.last, 'invoke:library');
    // The status toasts close on their own.
    await tester.pump(const Duration(seconds: 10));
    await tester.pumpAndSettle();
  });
}
