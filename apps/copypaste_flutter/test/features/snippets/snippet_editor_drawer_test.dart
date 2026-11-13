import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/snippets/controller/snippets_controller.dart';
import 'package:copypaste_flutter/features/snippets/models/snippet_models.dart';
import 'package:copypaste_flutter/features/snippets/view/snippet_editor_drawer.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'snippet_test_support.dart';

const _save = ValueKey('snippet-editor-save');
const _body = ValueKey('snippet-editor-body');
const _drawer = ValueKey('snippet-editor-drawer');

Future<FakeSnippetRepository> _open(
  WidgetTester tester, {
  SnippetDetail? snippet,
}) async {
  await tester.binding.setSurfaceSize(const Size(1000, 700));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final repository = FakeSnippetRepository();
  final controller = SnippetsController(repository: repository);
  addTearDown(controller.dispose);
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      builder: (context, child) => AppTheme.builder(context, child!),
      home: Scaffold(
        child: Builder(
          builder: (context) => Button.primary(
            key: const ValueKey('open-editor'),
            onPressed: () => showSnippetEditorDrawer(
              context,
              controller: controller,
              snippet: snippet,
            ),
            child: const Text('Open'),
          ),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('open-editor')));
  await tester.pumpAndSettle();
  return repository;
}

Future<void> _type(WidgetTester tester, String text) async {
  await tester.enterText(
    find.descendant(of: find.byKey(_body), matching: find.byType(EditableText)),
    text,
  );
  // Lets the lint and preview pauses run out.
  await tester.pump(const Duration(seconds: 1));
  await tester.pumpAndSettle();
}

bool _saveEnabled(WidgetTester tester) =>
    tester.widget<Button>(find.byKey(_save)).onPressed != null;

void main() {
  testWidgets('Save stays disabled with a blank body and says why', (
    tester,
  ) async {
    await _open(tester);
    expect(_saveEnabled(tester), isFalse);
    expect(find.text('Add some text to save.'), findsOne);

    await _type(tester, '  \n\t ');
    expect(_saveEnabled(tester), isFalse);
    expect(find.text('Add some text to save.'), findsOne);

    await _type(tester, 'Kind regards');
    expect(_saveEnabled(tester), isTrue);
    expect(
      find.byKey(const ValueKey('snippet-editor-save-reason')),
      findsNothing,
    );
  });

  testWidgets('closing with unsaved changes asks first, also on system back, '
      'and Discard writes nothing', (tester) async {
    final repository = await _open(tester);
    await _type(tester, 'Draft');

    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('unsaved-discard')), findsOne);
    await tester.tap(find.byKey(const ValueKey('unsaved-cancel')));
    await tester.pumpAndSettle();
    expect(find.byKey(_drawer), findsOne);

    await tester.tap(find.byKey(const ValueKey('snippet-editor-cancel')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('unsaved-discard')));
    await tester.pumpAndSettle();
    expect(find.byKey(_drawer), findsNothing);
    expect(repository.saved, isEmpty);
  });

  testWidgets('the footer stays in place while a long body scrolls', (
    tester,
  ) async {
    final long = List.generate(200, (line) => 'Line $line').join('\n');
    await _open(
      tester,
      snippet: SnippetDetail(summary: summary('s1'), bodyText: long),
    );
    final before = tester.getRect(find.byKey(_save));
    expect(tester.getRect(find.byKey(_drawer)).contains(before.center), isTrue);

    await tester.drag(
      find.byKey(const ValueKey('snippet-editor-title')),
      const Offset(0, -500),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('snippet-editor-title')), findsOne);
    expect(
      tester.getRect(find.byKey(const ValueKey('snippet-editor-title'))).top,
      lessThan(0),
      reason: 'the body scrolled',
    );
    expect(tester.getRect(find.byKey(_save)), before);
  });
}
