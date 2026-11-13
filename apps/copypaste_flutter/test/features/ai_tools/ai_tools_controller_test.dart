import 'package:copypaste_flutter/features/ai_tools/controller/ai_approval_controller.dart';
import 'package:copypaste_flutter/features/ai_tools/controller/ai_tools_controller.dart';
import 'package:copypaste_flutter/features/ai_tools/models/ai_tools_models.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';

import 'ai_tools_test_support.dart';

void main() {
  test('a request while the window is hidden posts one notification, and '
      'activating it shows the window', () async {
    final repository = FakeAiToolsRepository();
    final notifier = FakeAiApprovalNotifier();
    final visible = ValueNotifier(false);
    var shownWindow = 0;
    final controller = AiApprovalController(
      repository: repository,
      notifier: notifier,
      visible: visible,
      showWindow: () => shownWindow++,
    );
    addTearDown(controller.dispose);
    await pumpEventQueue();
    expect(notifier.shown, isEmpty);

    repository.pending = [
      AiApprovalRequest(
        requestId: 'request-1',
        kind: AiApprovalKind.access,
        clientName: 'Cursor',
        expiresAt: DateTime(2026, 11, 13),
      ),
    ];
    repository.approvalEvents.add(null);
    repository.approvalEvents.add(null);
    await pumpEventQueue();

    expect(notifier.shown.map((request) => request.requestId), ['request-1']);
    expect(controller.current?.clientName, 'Cursor');

    notifier.openedEvents.add('request-1');
    await pumpEventQueue();
    expect(shownWindow, 1);
    expect(notifier.dismissed, 1);
  });

  test('copying a configuration uses the concealed transient delivery and '
      'clears the clipboard after a minute', () async {
    final repository = FakeAiToolsRepository();
    final controller = AiToolsController(repository: repository, serving: true);
    addTearDown(controller.dispose);

    expect(await controller.copySecret('config'), isTrue);

    expect(repository.copied, ['config']);
    expect(repository.restored, [
      ('restore-1', AiToolsController.secretClipboardLifetime),
    ]);
    expect(AiToolsController.secretClipboardLifetime.inSeconds, 60);
  });
}
