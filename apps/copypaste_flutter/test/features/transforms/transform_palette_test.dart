import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/transforms/controller/transform_controller.dart';
import 'package:copypaste_flutter/features/transforms/models/transform_models.dart';
import 'package:copypaste_flutter/features/transforms/view/transform_palette.dart';
import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'transform_test_support.dart';

Future<List<TransformActionOutcome?>> _pump(
  WidgetTester tester,
  TransformController controller,
) async {
  await tester.binding.setSurfaceSize(const Size(900, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final done = <TransformActionOutcome?>[];
  await controller.open();
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      builder: (context, child) => AppTheme.builder(context, child!),
      home: Scaffold(
        child: Center(
          child: SizedBox(
            width: 640,
            child: TransformPalette(controller: controller, onDone: done.add),
          ),
        ),
      ),
    ),
  );
  // The list appears, highlights its first row, then the preview runs.
  for (var frame = 0; frame < 3; frame++) {
    await tester.pump(const Duration(milliseconds: 150));
  }
  return done;
}

Future<void> _close(WidgetTester tester, TransformController controller) async {
  await tester.pumpWidget(const SizedBox.shrink());
  controller.dispose();
}

Button _button(WidgetTester tester, String key) =>
    tester.widget<Button>(find.byKey(ValueKey(key)));

void main() {
  final strings = lookupAppLocalizations(const Locale('en'));

  testWidgets('a failing preview shows the step error and disables Paste '
      'and Copy', (tester) async {
    final repository = FakeTransformRepository()
      ..failPreview = const TransformIssue(
        code: 'json.trailing_comma',
        line: 1,
        column: 8,
      );
    final controller = TransformController(
      repository: repository,
      source: const TransformSource.text('{"a":1,}'),
      pasteForeground: () async => PasteOutcome.pasted,
    );
    await _pump(tester, controller);

    expect(find.byKey(const ValueKey('transform-preview-error')), findsOne);
    expect(find.textContaining('line 1, column 8'), findsOne);
    expect(_button(tester, 'transform-paste').onPressed, isNull);
    expect(_button(tester, 'transform-copy').onPressed, isNull);
    expect(repository.runs, isEmpty);
    await _close(tester, controller);
  });

  testWidgets('an image item shows that transforms work on text', (
    tester,
  ) async {
    final repository = FakeTransformRepository();
    final controller = TransformController(
      repository: repository,
      source: const TransformSource.clip('image', text: false),
    );
    await _pump(tester, controller);

    expect(find.byKey(const ValueKey('transform-not-text')), findsOne);
    expect(find.text(strings.transformsNotTextTitle), findsOne);
    expect(find.byKey(const ValueKey('transform-copy')), findsNothing);
    expect(repository.calls, isEmpty);
    await _close(tester, controller);
  });

  testWidgets('the palette is operable from the keyboard', (tester) async {
    final repository = FakeTransformRepository();
    final controller = TransformController(
      repository: repository,
      source: const TransformSource.text('abc'),
    );
    final done = await _pump(tester, controller);

    await tester.enterText(find.byType(TextField).first, 'upp');
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(controller.candidate?.definition?.id, 'case.upper');

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    expect(controller.committedCount, 1);
    expect(find.text('1 · UPPERCASE'), findsOne);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();

    expect(repository.runs.single.$1, TransformRunAction.copy);
    expect(repository.runs.single.$2.single.transformId, 'case.upper');
    expect(done.single?.status, TransformActionStatus.completed);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    expect(done.last, isNull);
    await _close(tester, controller);
  });
}
