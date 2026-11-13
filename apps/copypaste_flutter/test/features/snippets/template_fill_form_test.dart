import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/snippets/controller/template_fill_controller.dart';
import 'package:copypaste_flutter/features/snippets/models/snippet_models.dart';
import 'package:copypaste_flutter/features/snippets/view/template_fill_form.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'snippet_test_support.dart';

void main() {
  testWidgets('a choice without a default starts on its first option, and an '
      'empty required field blocks use and takes focus', (tester) async {
    final repository = FakeSnippetRepository();
    final controller = TemplateFillController(
      repository: repository,
      snippet: SnippetDetail(
        summary: summary('t1'),
        bodyText: 'unused',
        fields: const [
          TemplateField(
            name: 'tone',
            label: 'Tone',
            kind: TemplateFieldKind.choice,
            options: ['warm', 'formal'],
          ),
          TemplateField(name: 'name', label: 'Name', required: true),
        ],
      ),
    );
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: (context, child) => AppTheme.builder(context, child!),
        home: Scaffold(
          child: SingleChildScrollView(
            child: TemplateFillForm(controller: controller, onSubmit: () {}),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(controller.values['tone'], 'warm');

    expect(await controller.submit(SnippetDelivery.paste), isNull);
    await tester.pumpAndSettle();
    expect(repository.used, isEmpty);
    expect(controller.focusNodes['name']!.hasFocus, isTrue);
    expect(find.byKey(const ValueKey('name-error')), findsOne);

    await tester.enterText(find.byKey(const ValueKey('name')), 'Ada');
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('name-error')), findsNothing);
    expect(await controller.submit(SnippetDelivery.paste), isNotNull);
    expect(repository.used.single.$2, {'tone': 'warm', 'name': 'Ada'});

    await tester.pumpWidget(const SizedBox.shrink());
    controller.dispose();
  });
}
