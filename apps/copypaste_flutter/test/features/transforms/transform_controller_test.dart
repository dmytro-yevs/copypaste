import 'dart:async';

import 'package:copypaste_flutter/features/transforms/controller/transform_controller.dart';
import 'package:copypaste_flutter/features/transforms/controller/transform_entries.dart';
import 'package:copypaste_flutter/features/transforms/models/transform_models.dart';
import 'package:copypaste_flutter/platform/desktop/quick_paste_host.dart';
import 'package:flutter_test/flutter_test.dart';

import 'transform_test_support.dart';

const _upper = TransformEntry.transform(upper);
const _json = TransformEntry.transform(jsonFormat);

void main() {
  test('a non-text clip shows the text-only state and runs nothing', () async {
    final repository = FakeTransformRepository();
    final controller = TransformController(
      repository: repository,
      source: const TransformSource.clip('image', text: false),
    );
    addTearDown(controller.dispose);

    await controller.open();
    controller.highlight(_upper);

    expect(controller.status, TransformPaletteStatus.notText);
    expect(repository.calls, isEmpty);
    expect(controller.canRun, isFalse);
  });

  test(
    'a runtime refusal of non-text content switches to the same state',
    () async {
      final repository = FakeTransformRepository()
        ..previewError = const TransformException(
          TransformException.unsupportedContent,
          'Transforms work on text.',
        );
      final controller = TransformController(
        repository: repository,
        source: const TransformSource.clip('file'),
        debounce: Duration.zero,
      );
      addTearDown(controller.dispose);
      await controller.open();

      controller.highlight(_upper);
      await pumpEventQueue();

      expect(controller.status, TransformPaletteStatus.notText);
      expect(repository.runs, isEmpty);
    },
  );

  test(
    'a failing step disables Paste and Copy and nothing is delivered',
    () async {
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
        debounce: Duration.zero,
      );
      addTearDown(controller.dispose);
      await controller.open();

      controller.highlight(_json);
      await pumpEventQueue();

      expect(controller.preview?.failedStep, 0);
      expect(controller.canPaste, isFalse);
      expect(controller.canCopy, isFalse);
      expect(controller.canSave, isFalse);
      final outcome = await controller.paste();
      expect(outcome.status, TransformActionStatus.failed);
      expect(repository.runs, isEmpty);
    },
  );

  test('Tab keeps a step and the next highlight extends the chain', () async {
    final repository = FakeTransformRepository();
    final controller = TransformController(
      repository: repository,
      source: const TransformSource.text('abc'),
      debounce: Duration.zero,
    );
    addTearDown(controller.dispose);
    await controller.open();

    controller.highlight(_upper);
    expect(controller.commitCandidate(), isTrue);
    controller.highlight(_json);
    controller.setParam(1, jsonFormat.params.single, '4');
    await pumpEventQueue();

    expect(controller.chain.map((step) => step.transformId), [
      'case.upper',
      'structure.json_format',
    ]);
    expect(controller.chain.last.params, {'indent': '4'});
    expect(repository.previews.last.$2, hasLength(2));
    expect(controller.removeLastStep(), isTrue);
    expect(controller.committedCount, 0);
  });

  test(
    'a paste restores the clipboard only when it reached the target',
    () async {
      final repository = FakeTransformRepository();
      var outcome = PasteOutcome.pasted;
      final controller = TransformController(
        repository: repository,
        source: const TransformSource.clip('clip-1'),
        pasteForeground: () async => outcome,
        debounce: Duration.zero,
      );
      addTearDown(controller.dispose);
      await controller.open();
      controller.highlight(_upper);
      await pumpEventQueue();

      final pasted = await controller.paste();
      expect(pasted.status, TransformActionStatus.completed);
      expect(repository.restored, ['restore-1']);

      outcome = PasteOutcome.elevatedTarget;
      final fallback = await controller.paste();
      expect(fallback.status, TransformActionStatus.onClipboard);
      expect(repository.restored, ['restore-1']);
      expect(controller.recents.single.steps.single.transformId, 'case.upper');
    },
  );

  testWidgets('previews are debounced and closing leaves nothing running', (
    tester,
  ) async {
    final repository = FakeTransformRepository();
    final controller = TransformController(
      repository: repository,
      source: const TransformSource.text('abc'),
    );
    await controller.open();

    controller.highlight(_upper);
    await tester.pump(const Duration(milliseconds: 60));
    controller.highlight(_json);
    await tester.pump(const Duration(milliseconds: 130));
    expect(repository.previews.map((p) => p.$2.single.transformId), [
      'structure.json_format',
    ]);

    repository.hold = Completer<void>();
    controller.highlight(_upper);
    await tester.pump(const Duration(milliseconds: 130));
    final running = repository.previews.last.$1;
    await tester.pump(const Duration(milliseconds: 200));
    expect(controller.showSpinner, isTrue);
    controller.dispose();

    expect(repository.cancelled, contains(running));
    repository.hold!.complete();
    await tester.pump();
  });
}
