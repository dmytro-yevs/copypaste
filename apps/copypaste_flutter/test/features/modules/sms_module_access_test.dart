import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/app/theme/app_tokens.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/module_access_repository.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';
import 'modules_test_support.dart';

const sms = InstalledModule(
  id: 'copypaste.sms-codes',
  title: 'SMS Codes',
  description: 'Copy SMS codes.',
  version: '0.1.0',
  enabled: false,
  sizeBytes: 100,
  commands: [],
  preferenceFields: [],
  preferences: {},
  events: [ModuleEventKind.smsReceived],
);

class Access implements ModuleAccessRepository {
  bool granted = false;
  bool starts = true;
  int synchronizations = 0;
  @override
  Future<SmsModuleAccessState> smsState() async =>
      SmsModuleAccessState(granted: granted, adbCommands: 'adb shell test');
  @override
  Future<SmsModuleAccessState> configureSms() async {
    granted = true;
    return smsState();
  }

  @override
  Future<bool> synchronize() async {
    synchronizations++;
    return starts;
  }
}

void main() {
  for (final width in [360.0, 1000.0]) {
    testWidgets('SMS setup and enable use shared controls at width $width', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(Size(width, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = MemoryModulesRepository()..modules = [sms];
      final controller = ModulesController(
        repository: repository,
        marketplace: MemoryModuleMarketplace(),
        access: Access(),
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
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: ModulesSettingsView(controller: controller),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('module-settings-copypaste.sms-codes')),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Set up SMS access'));
      await tester.pumpAndSettle();
      expect(find.text('adb shell test'), findsOneWidget);
      await tester.tap(find.widgetWithText(Button, 'Apply with Shizuku'));
      await tester.pumpAndSettle();
      expect(
        find.text(
          'Access is ready. Enable SMS Codes to start copying new codes.',
        ),
        findsOneWidget,
      );
      await tester.tap(find.widgetWithText(Button, 'Done').last);
      await tester.pumpAndSettle();
      await tester.tap(find.byType(Switch));
      await tester.pumpAndSettle();
      expect(repository.calls, ['enabled:true']);
      expect(tester.takeException(), isNull);
    });
  }
  test(
    'requires verified SMS access before enabling and synchronizes disable and removal',
    () async {
      final repository = MemoryModulesRepository()..modules = [sms];
      final access = Access();
      final controller = ModulesController(
        repository: repository,
        marketplace: MemoryModuleMarketplace(),
        access: access,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      await controller.setEnabled(sms.id, true);
      expect(repository.calls, isEmpty);
      expect(controller.errorMessage, contains('Set up SMS access'));
      await controller.configureSmsAccess();
      await controller.setEnabled(sms.id, true);
      expect(controller.modules.single.enabled, isTrue);
      await controller.setEnabled(sms.id, false);
      expect(controller.modules.single.enabled, isFalse);
      await controller.remove(sms.id);
      expect(controller.modules, isEmpty);
      expect(access.synchronizations, greaterThanOrEqualTo(4));
    },
  );
  test('failed monitoring startup rolls back enabled state', () async {
    final repository = MemoryModulesRepository()..modules = [sms];
    final access = Access()..granted = true;
    final controller = ModulesController(
      repository: repository,
      marketplace: MemoryModuleMarketplace(),
      access: access,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    access.starts = false;
    await controller.setEnabled(sms.id, true);
    expect(repository.calls, ['enabled:true', 'enabled:false']);
    expect(controller.modules.single.enabled, isFalse);
    expect(controller.errorMessage, contains('could not be started'));
  });
}
