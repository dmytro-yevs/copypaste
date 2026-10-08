import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/modules_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/models/module_marketplace_models.dart';
import 'package:copypaste_flutter/features/modules/view/modules_settings_view.dart';
import 'modules_test_support.dart';

class _AccountRepository extends MemoryModulesRepository {
  @override
  Future<ModuleResult> invoke(
    String id,
    String command,
    Map<String, Object> arguments,
  ) async {
    lastArguments = Map.of(arguments);
    return const ModuleResult('Signed in.');
  }
}

void main() {
  for (final (platform, width) in [
    (TargetPlatform.android, 320.0),
    (TargetPlatform.macOS, 1000.0),
    (TargetPlatform.windows, 1000.0),
  ]) {
    testWidgets('account secrets use obscured shared fields on $platform', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(Size(width, 900));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final repository = _AccountRepository()
        ..modules = const [
          InstalledModule(
            id: 'copypaste.supabase',
            title: 'Supabase Sync',
            description: 'Encrypted history sync',
            version: '0.1.0',
            enabled: true,
            sizeBytes: 10,
            commands: [
              ModuleCommand(
                id: 'sign-in',
                title: 'Sign in',
                description: '',
                arguments: [
                  ModuleField(
                    id: 'email',
                    title: 'Email',
                    kind: ModuleFieldKind.text,
                    defaultValue: '',
                    required: true,
                  ),
                  ModuleField(
                    id: 'password',
                    title: 'Account password',
                    kind: ModuleFieldKind.text,
                    defaultValue: '',
                    required: true,
                    secret: true,
                  ),
                  ModuleField(
                    id: 'passphrase',
                    title: 'Sync passphrase',
                    kind: ModuleFieldKind.text,
                    defaultValue: '',
                    required: true,
                    secret: true,
                  ),
                ],
              ),
            ],
            preferenceFields: [],
            preferences: {},
          ),
        ];
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
      await tester.tap(
        find.byKey(const ValueKey('module-settings-copypaste.supabase')),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Sign in'));
      await tester.pumpAndSettle();
      for (final id in ['password', 'passphrase']) {
        final field = tester.widget<TextField>(find.byKey(ValueKey(id)));
        expect(field.obscureText, isTrue);
        expect(field.autocorrect, isFalse);
        expect(field.enableSuggestions, isFalse);
      }
      await tester.enterText(
        find.byKey(const ValueKey('email')),
        'person@example.com',
      );
      await tester.enterText(
        find.byKey(const ValueKey('password')),
        'account password',
      );
      await tester.enterText(
        find.byKey(const ValueKey('passphrase')),
        'shared sync passphrase',
      );
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(Button, 'Run'));
      await tester.pumpAndSettle();
      expect(repository.lastArguments, {
        'email': 'person@example.com',
        'password': 'account password',
        'passphrase': 'shared sync passphrase',
      });
      expect(find.text('Signed in.'), findsOneWidget);
      expect(find.text('shared sync passphrase'), findsNothing);
      expect(tester.takeException(), isNull);
    }, variant: TargetPlatformVariant({platform}));
  }
}
