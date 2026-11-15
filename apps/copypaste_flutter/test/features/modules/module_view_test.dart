import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_view.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';

import 'modules_test_support.dart';

const _module = InstalledModule(
  id: 'copypaste.ai-tools',
  title: 'AI Tools',
  description: 'Let AI apps use your history.',
  version: '1.0.0',
  enabled: true,
  sizeBytes: 10,
  commands: [
    ModuleCommand(
      id: 'connections',
      title: 'Connections',
      description: '',
      arguments: [],
    ),
  ],
  preferenceFields: [],
  preferences: {},
);

ModuleView _connections({required bool connected}) => ModuleView(
  sections: [
    ModuleViewSection(
      title: 'Connected apps',
      items: [
        if (connected)
          const ModuleViewRow(
            title: 'Claude',
            subtitle: 'Reads history',
            actions: [
              ModuleViewAction(
                title: 'Revoke',
                command: 'revoke',
                arguments: {'id': 'claude'},
                destructive: true,
              ),
            ],
          ),
        const ModuleViewBlock(label: 'Config', text: '{"server":"copypaste"}'),
      ],
    ),
  ],
);

void main() {
  for (final (platform, width) in [
    (TargetPlatform.macOS, 1000.0),
    (TargetPlatform.android, 360.0),
  ]) {
    testWidgets('settings drawer renders a module view on $platform', (
      tester,
    ) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = Size(width, 900);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      String? copied;
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async {
          if (call.method == 'Clipboard.setData') {
            copied = (call.arguments as Map)['text'] as String;
          }
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          SystemChannels.platform,
          null,
        ),
      );
      var connected = true;
      Map<String, Object>? revoked;
      final repository = MemoryModulesRepository()
        ..modules = [_module]
        ..results = (command, arguments) => switch (command) {
          'revoke' => () {
            connected = false;
            revoked = arguments;
            return const ModuleResult('Access revoked.');
          }(),
          _ => ModuleResult.view(_connections(connected: connected)),
        };
      final controller = ModulesController(
        repository: repository,
        marketplace: MemoryModuleMarketplace(),
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.selectSection(ModulesSection.installed);
      await tester.pumpWidget(
        ShadcnApp(
          theme: AppTheme.light.copyWith(platform: () => platform),
          builder: AppTheme.builder,
          home: Scaffold(
            child: SingleChildScrollView(
              child: ModulesSettingsView(controller: controller),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('module-settings-copypaste.ai-tools')),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Connections'));
      await tester.pumpAndSettle();

      expect(find.text('Connected apps'), findsOneWidget);
      expect(find.text('Claude'), findsOneWidget);
      expect(find.text('Reads history'), findsOneWidget);
      expect(find.text('Config'), findsOneWidget);

      await tester.ensureVisible(find.bySemanticsLabel('Copy'));
      await tester.tap(find.bySemanticsLabel('Copy'));
      await tester.pumpAndSettle();
      expect(copied, '{"server":"copypaste"}');

      // A destructive action is confirmed, its text result is shown, and
      // the command that produced the view runs again.
      final revoke = find.byKey(
        const ValueKey('module-view-action-revoke-Revoke'),
      );
      await tester.ensureVisible(revoke);
      await tester.tap(revoke);
      await tester.pumpAndSettle();
      expect(repository.calls, ['invoke:connections']);
      await tester.tap(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.widgetWithText(Button, 'Revoke'),
        ),
      );
      await tester.pumpAndSettle();
      expect(revoked, {'id': 'claude'});
      expect(find.text('Access revoked.'), findsOneWidget);
      await tester.tap(find.widgetWithText(Button, 'Done').last);
      await tester.pumpAndSettle();
      expect(repository.calls, [
        'invoke:connections',
        'invoke:revoke',
        'invoke:connections',
      ]);
      expect(find.text('Claude'), findsNothing);
      expect(find.text('Connected apps'), findsOneWidget);
    });
  }
}
