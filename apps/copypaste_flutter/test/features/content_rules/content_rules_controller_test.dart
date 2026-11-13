import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/content_rules/controller/content_rules_controller.dart';
import 'package:copypaste_flutter/features/content_rules/view/content_rules_search_targets.dart';
import 'package:copypaste_flutter/features/content_rules/view/content_rules_settings_view.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'content_rule_test_support.dart';

void main() {
  test(
    'moves user rules within their tier and refuses moves past an end',
    () async {
      final repository = FakeContentRuleRepository(
        rules: [
          rule('builtin.credentials', builtin: true),
          rule('a'),
          rule('b'),
          rule('c'),
        ],
      );
      final controller = ContentRulesController(repository: repository);
      addTearDown(controller.dispose);
      controller.attach();
      await pumpEventQueue();

      expect(controller.canMove('a', RuleMove.up), isFalse);
      expect(controller.canMove('c', RuleMove.bottom), isFalse);
      expect(await controller.move('a', RuleMove.up), isFalse);

      expect(await controller.move('a', RuleMove.down), isTrue);
      await pumpEventQueue();
      expect(await controller.move('a', RuleMove.top), isTrue);
      await pumpEventQueue();

      expect(repository.reorders, [('a', 'b'), ('a', null)]);
      expect(controller.userRules.map((rule) => rule.id), ['a', 'b', 'c']);
      expect(controller.builtins.single.id, 'builtin.credentials');
    },
  );

  testWidgets('the row menu moves a rule up without dragging', (tester) async {
    await tester.binding.setSurfaceSize(const Size(1000, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repository = FakeContentRuleRepository(
      rules: [rule('a'), rule('b'), rule('c')],
    );
    final controller = ContentRulesController(repository: repository);
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: (context, child) => AppTheme.builder(context, child!),
        home: Scaffold(
          child: SingleChildScrollView(
            child: ContentRulesSettingsView(
              controller: controller,
              targets: ContentRulesSettingsTargets(),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('rule-menu-c')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('rule-move-down-c')), findsNothing);
    await tester.tap(find.byKey(const ValueKey('rule-move-up-c')));
    await tester.pumpAndSettle();

    expect(repository.reorders, [('c', 'a')]);
    expect(controller.userRules.map((rule) => rule.id), ['a', 'c', 'b']);
  });
}
