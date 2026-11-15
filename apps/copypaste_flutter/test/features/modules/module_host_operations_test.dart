import 'dart:async';
import 'dart:convert';

import 'package:copypaste_flutter/features/modules/repository/runtime_module_prompts_repository.dart';
import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/shared/runtime_event_service.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('prompts map the runtime JSON, events and answers', () async {
    final source = StreamController<runtime.RuntimeEvent>();
    final answers = <(String, String?)>[];
    final prompts = RuntimeModulePromptsRepository(
      read: () async => jsonEncode([
        {
          'prompt_id': 'p1',
          'module_id': 'copypaste.ai-tools',
          'title': 'Allow?',
          'body': 'Claude asks.',
          'choices': [
            {'id': 'deny', 'title': 'Deny', 'destructive': true},
            {'id': 'allow', 'title': 'Allow'},
          ],
          'expires_at_ms': 1000,
        },
      ]),
      answer: (id, choice) async => answers.add((id, choice)),
      events: RuntimeEventService(
        allocate: () async => BigInt.one,
        cancel: (_) async {},
        watch: (_) => source.stream,
      ),
    );
    var changes = 0;
    final subscription = prompts.changes().listen((_) => changes++);
    await Future<void>.delayed(Duration.zero);
    for (final kind in ['ready', 'items', 'module_prompt']) {
      source.add(
        runtime.RuntimeEvent(
          kind: kind,
          itemCount: BigInt.zero,
          captured: false,
          runtimeWatch: true,
        ),
      );
    }
    await Future<void>.delayed(Duration.zero);
    expect(changes, 2);

    final prompt = (await prompts.pending()).single;
    expect(prompt.moduleId, 'copypaste.ai-tools');
    expect(prompt.choices.map((choice) => choice.destructive), [true, false]);
    expect(prompt.expiresAt, DateTime.fromMillisecondsSinceEpoch(1000));
    await prompts.answer('p1', null);
    expect(answers, [('p1', null)]);
    await subscription.cancel();
    await source.close();
  });
}
