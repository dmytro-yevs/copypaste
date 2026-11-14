import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/runtime_module_prompts_repository.dart';
import 'package:copypaste_flutter/features/modules/repository/runtime_modules_repository.dart';

void main() {
  test('schema 6 operations use the ModuleOperation wire shape', () async {
    final sent = <Map<String, Object?>>[];
    Future<String> send(Map<String, Object?> operation) async {
      sent.add(operation);
      return switch (operation['operation']) {
        'prompts' => jsonEncode([
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
        _ => 'null',
      };
    }

    final modules = RuntimeModulesRepository(send: send);
    await modules.setEnabled(
      'copypaste.ai-tools',
      true,
      capabilities: const [
        ModuleCapability.historyRead,
        ModuleCapability.clipboardWrite,
      ],
    );
    await modules.setLocale('pt-BR');
    final prompts = RuntimeModulePromptsRepository(send: send);
    final pending = await prompts.pending();
    await prompts.answer('p1', null);

    expect(sent, [
      {
        'operation': 'set_enabled',
        'id': 'copypaste.ai-tools',
        'enabled': true,
        'capabilities': ['history_read', 'clipboard_write'],
      },
      {'operation': 'set_locale', 'locale': 'pt-BR'},
      {'operation': 'prompts'},
      {'operation': 'answer_prompt', 'prompt_id': 'p1', 'choice': null},
    ]);
    final prompt = pending.single;
    expect(prompt.moduleId, 'copypaste.ai-tools');
    expect(prompt.choices.map((choice) => choice.destructive), [true, false]);
    expect(prompt.expiresAt, DateTime.fromMillisecondsSinceEpoch(1000));
  });

  test('operations fail as unsupported until the bridge is wired', () async {
    await expectLater(
      RuntimeModulesRepository().setLocale('uk'),
      throwsA(isA<ModulesException>()),
    );
  });
}
