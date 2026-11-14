import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/content_rules/controller/content_rules_controller.dart';
import 'package:copypaste_flutter/features/content_rules/models/content_rule_models.dart';
import 'package:copypaste_flutter/features/content_rules/view/rule_editor_drawer.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'content_rule_test_support.dart';

const _save = ValueKey('rule-editor-save');

Future<FakeContentRuleRepository> _open(
  WidgetTester tester,
  String definitionJson, {
  FakeContentRuleRepository? repository,
}) async {
  await tester.binding.setSurfaceSize(const Size(1000, 1400));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final rules = repository ?? FakeContentRuleRepository();
  final controller = ContentRulesController(repository: rules);
  addTearDown(controller.dispose);
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      builder: (context, child) => AppTheme.builder(context, child!),
      home: Scaffold(
        child: Builder(
          builder: (context) => Button.primary(
            key: const ValueKey('open-editor'),
            onPressed: () => showRuleEditorDrawer(
              context,
              controller: controller,
              rule: ContentRule(
                id: 'r1',
                name: 'Tickets',
                definitionJson: definitionJson,
              ),
            ),
            child: const Text('Open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('open-editor')));
  await tester.pumpAndSettle();
  return rules;
}

bool _saveEnabled(WidgetTester tester) =>
    tester.widget<Button>(find.byKey(_save)).onPressed != null;

void main() {
  testWidgets('shows the runtime regex error inline until the pattern is fixed', (
    tester,
  ) async {
    final repository = FakeContentRuleRepository(
      problems: (json) => json.contains('(?=')
          ? const [
              RuleProblem(
                path: 'conditions[0].value',
                code: 'unsupported_look_around',
              ),
            ]
          : const [],
    );
    await _open(
      tester,
      '{"match":"all","conditions":[{"type":"content_text","op":"matches_regex",'
      '"value":"a(?=b)"}],"actions":[{"type":"mark_secret"}]}',
      repository: repository,
    );

    expect(
      find.byKey(const ValueKey('rule-condition-problem-0')),
      findsOneWidget,
    );
    expect(find.textContaining('Look-ahead and look-behind'), findsOneWidget);

    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('rule-condition-value-0')),
        matching: find.byType(EditableText),
      ),
      'ticket-[0-9]+',
    );
    await tester.pump(const Duration(seconds: 1));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey('rule-condition-problem-0')),
      findsNothing,
    );
    expect(repository.validated.last, contains('ticket-[0-9]+'));
    expect(_saveEnabled(tester), isTrue);
  });

  testWidgets('offers no action beside "Don\'t save" and says why', (
    tester,
  ) async {
    await _open(
      tester,
      '{"match":"all","conditions":[],"actions":[{"type":"dont_save"}]}',
    );

    await tester.tap(find.byKey(const ValueKey('rule-editor-add-action')));
    await tester.pumpAndSettle();

    final secret = tester.widget<MenuButton>(
      find.byKey(const ValueKey('rule-add-action-mark_secret')),
    );
    expect(secret.enabled, isFalse);
    expect(
      find.byKey(const ValueKey('rule-add-action-reason-mark_secret')),
      findsOneWidget,
    );
    expect(
      find.text('“Don\'t save” can\'t be combined with other actions.'),
      findsWidgets,
    );
  });

  testWidgets('conditions other platforms cannot evaluate carry a notice', (
    tester,
  ) async {
    await _open(
      tester,
      '{"match":"any","conditions":['
      '{"type":"source_app","any_of":["com.example.terminal"]},'
      '{"type":"size","unit":"bytes","op":"greater_than","value":1024},'
      '{"type":"content_type","any_of":["image"]}],'
      '"actions":[{"type":"delete_after","duration_ms":600000},'
      '{"type":"add_tags","tag_ids":["t1"]}]}',
    );

    expect(
      find.byKey(const ValueKey('rule-condition-notice-0')),
      findsOneWidget,
    );
    expect(
      find.textContaining('On Android, only copies made with'),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('rule-condition-notice-1')), findsNothing);
    expect(find.byKey(const ValueKey('rule-condition-notice-2')), findsNothing);
  });
}
