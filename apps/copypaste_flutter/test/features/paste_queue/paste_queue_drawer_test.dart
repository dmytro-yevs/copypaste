import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/paste_queue/controller/paste_queue_controller.dart';
import 'package:copypaste_flutter/features/paste_queue/view/paste_queue_header_action.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'paste_queue_test_support.dart';

Future<PasteQueueController> _open(
  WidgetTester tester,
  FakePasteQueueRepository repository,
) async {
  await tester.binding.setSurfaceSize(const Size(1000, 700));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final controller = PasteQueueController(
    repository: repository,
    pasteHost: FakeForegroundPasteHost(),
    settle: Duration.zero,
  );
  await controller.start();
  await tester.pumpWidget(
    ShadcnApp(
      theme: AppTheme.light,
      builder: (context, child) => AppTheme.builder(context, child!),
      home: Scaffold(
        child: Align(
          alignment: Alignment.topRight,
          child: PasteQueueHeaderAction(controller: controller),
        ),
      ),
    ),
  );
  await tester.tap(find.byKey(const ValueKey('paste-queue-open')));
  await tester.pumpAndSettle();
  return controller;
}

Future<void> _close(
  WidgetTester tester,
  PasteQueueController controller,
) async {
  await tester.pumpWidget(const SizedBox.shrink());
  controller.dispose();
}

void main() {
  testWidgets('the footer stays in place while a long queue scrolls', (
    tester,
  ) async {
    final controller = await _open(
      tester,
      FakePasteQueueRepository(entries: 40),
    );
    final pasteNext = find.byKey(const ValueKey('paste-queue-paste-next'));
    final drawer = find.byKey(const ValueKey('paste-queue-drawer'));
    expect(find.byKey(const ValueKey('paste-queue-row-e1')), findsOne);
    final before = tester.getRect(pasteNext);
    expect(tester.getRect(drawer).contains(before.center), isTrue);

    await tester.drag(
      find.byKey(const ValueKey('paste-queue-body')),
      const Offset(0, -600),
    );
    await tester.pumpAndSettle();

    expect(tester.getRect(pasteNext), before);
    expect(
      tester.widget<Button>(pasteNext).onPressed,
      isNotNull,
      reason: 'Paste next stays usable however long the list is',
    );
    await _close(tester, controller);
  });

  testWidgets('an empty queue offers collecting, then shows the collecting '
      'state', (tester) async {
    final repository = FakePasteQueueRepository();
    final controller = await _open(tester, repository);
    expect(find.byKey(const ValueKey('paste-queue-empty')), findsOne);
    expect(
      tester
          .widget<Button>(find.byKey(const ValueKey('paste-queue-paste-next')))
          .onPressed,
      isNull,
    );

    await tester.tap(find.text('Start collecting').last);
    await tester.pumpAndSettle();

    expect(repository.calls, ['collecting:true']);
    expect(
      find.byKey(const ValueKey('paste-queue-collecting-empty')),
      findsOne,
    );
    expect(find.text('1 pending · Collecting'), findsNothing);
    expect(find.text('0 pending · Collecting'), findsOne);
    await _close(tester, controller);
  });
}
