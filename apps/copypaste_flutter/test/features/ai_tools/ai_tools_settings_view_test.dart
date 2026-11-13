import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/ai_tools/controller/ai_tools_controller.dart';
import 'package:copypaste_flutter/features/ai_tools/view/ai_tools_settings_targets.dart';
import 'package:copypaste_flutter/features/ai_tools/view/ai_tools_settings_view.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'ai_tools_test_support.dart';

void main() {
  final l10n = lookupAppLocalizations(AppLocalization.fallback);

  Future<void> pump(
    WidgetTester tester,
    FakeAiToolsRepository repository, {
    required bool serving,
  }) async {
    await tester.binding.setSurfaceSize(const Size(900, 1600));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final controller = AiToolsController(
      repository: repository,
      serving: serving,
    );
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: Scaffold(
          child: SingleChildScrollView(
            child: AiToolsSettingsView(
              controller: controller,
              targets: AiToolsSettingsTargets(serving: serving),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets('Android shows the policy and every desktop grant, without the '
      'local toggle or Connect', (tester) async {
    final repository = FakeAiToolsRepository()..deviceId = 'phone-1';
    await pump(tester, repository, serving: false);

    expect(find.text(l10n.aiToolsEmptyServedTitle), findsOneWidget);
    expect(find.text(l10n.aiToolsPolicyTitle), findsOneWidget);
    expect(find.text(l10n.aiToolsOtherDevicesTitle), findsOneWidget);
    expect(find.byKey(const ValueKey('ai-tools-enabled')), findsNothing);
    expect(find.byKey(const ValueKey('ai-tools-connect')), findsNothing);
    expect(find.text(l10n.aiToolsConnect), findsNothing);

    repository.grantList = [grantOf('grant-1')];
    repository.changeEvents.add(null);
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('ai-grant-grant-1')), findsOneWidget);
    expect(find.text(l10n.aiToolsServedThrough('Studio Mac')), findsOneWidget);
    expect(find.bySemanticsLabel(l10n.aiToolsEdit), findsOneWidget);
    expect(find.bySemanticsLabel(l10n.aiToolsRevoke), findsOneWidget);
    expect(find.bySemanticsLabel(l10n.aiToolsActivity), findsNothing);
  });

  testWidgets('Connect shows the configuration with the command-line path, '
      'and Copy bypasses the system clipboard writer', (tester) async {
    final platformCalls = <String>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        platformCalls.add(call.method);
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        null,
      ),
    );
    final repository = FakeAiToolsRepository()..serving = true;
    await pump(tester, repository, serving: true);

    expect(find.byKey(const ValueKey('ai-tools-enabled')), findsOneWidget);
    await tester.tap(find.text(l10n.aiToolsConnect).first);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('ai-connect-create')));
    await tester.pumpAndSettle();

    final config = find.textContaining('"command": "$cliPath"');
    expect(config, findsOneWidget);
    expect(find.textContaining('"args": ["mcp"]'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('ai-connect-copy')));
    await tester.pumpAndSettle();

    expect(repository.copied, hasLength(1));
    expect(repository.copied.single, contains(cliPath));
    expect(repository.restored.single.$2, const Duration(seconds: 60));
    expect(platformCalls, isNot(contains('Clipboard.setData')));
    expect(find.text(l10n.aiToolsCopied), findsOneWidget);
    await tester.pump(const Duration(seconds: 6));
    await tester.pumpAndSettle();
  });
}
