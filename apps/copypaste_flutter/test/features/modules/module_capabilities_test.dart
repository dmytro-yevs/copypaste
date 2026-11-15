import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';

import 'modules_test_support.dart';

const _module = InstalledModule(
  id: 'copypaste.ai-tools',
  title: 'AI Tools',
  description: 'Let AI apps use your history.',
  version: '1.0.0',
  enabled: false,
  sizeBytes: 10,
  commands: [],
  preferenceFields: [],
  preferences: {},
  capabilities: [ModuleCapability.historyRead, ModuleCapability.prompts],
);

void main() {
  testWidgets('enabling confirms the declared capabilities and echoes them', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1000, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = MemoryModulesRepository()..modules = [_module];
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
    final toggle = find.byKey(
      const ValueKey('module-enabled-copypaste.ai-tools'),
    );

    // Cancelling leaves the module off without a request.
    await tester.tap(toggle);
    await tester.pumpAndSettle();
    expect(find.text('Enable AI Tools?'), findsOneWidget);
    expect(find.text('Read your clipboard history'), findsOneWidget);
    expect(find.text('Ask you to confirm actions'), findsOneWidget);
    await tester.tap(find.widgetWithText(Button, 'Cancel'));
    await tester.pumpAndSettle();
    expect(repository.calls, isEmpty);

    await tester.tap(toggle);
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(Button, 'Enable'));
    await tester.pumpAndSettle();
    expect(repository.calls, ['enabled:true']);
    expect(repository.lastCapabilities, _module.capabilities);
    expect(controller.modules.single.enabled, isTrue);
    expect(find.text('Enable AI Tools?'), findsNothing);

    // Disabling needs no confirmation and echoes nothing.
    await tester.tap(toggle);
    await tester.pumpAndSettle();
    expect(repository.calls, ['enabled:true', 'enabled:false']);
    expect(repository.lastCapabilities, isEmpty);
  });
}
