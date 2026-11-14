import 'dart:convert';

import '../../../shared/runtime_event_service.dart';
import '../models/module_prompt.dart';
import 'modules_repository.dart';
import 'runtime_modules_repository.dart';

/// [ModulePromptsRepository] over the runtime's module operations and its
/// `module_prompt` events.
class RuntimeModulePromptsRepository implements ModulePromptsRepository {
  RuntimeModulePromptsRepository({
    ModuleOperationSender send = sendModuleOperation,
    RuntimeEventService? events,
  }) : _send = send,
       _events = events ?? RuntimeEventService.instance;

  static const eventKind = 'module_prompt';

  final ModuleOperationSender _send;
  final RuntimeEventService _events;

  @override
  Stream<void> changes() => _events.eventsOf(const {eventKind}).map((_) {});

  @override
  Future<List<ModulePrompt>> pending() => guardModuleRequest(() async {
    final prompts = jsonDecode(await _send({'operation': 'prompts'})) as List;
    return List.unmodifiable(
      prompts.cast<Map<String, dynamic>>().map(
        (prompt) => ModulePrompt(
          id: prompt['prompt_id'] as String,
          moduleId: prompt['module_id'] as String,
          title: prompt['title'] as String,
          body: prompt['body'] as String,
          choices: List.unmodifiable(
            (prompt['choices'] as List).cast<Map<String, dynamic>>().map(
              (choice) => ModulePromptChoice(
                id: choice['id'] as String,
                title: choice['title'] as String,
                destructive: choice['destructive'] as bool? ?? false,
              ),
            ),
          ),
          expiresAt: DateTime.fromMillisecondsSinceEpoch(
            prompt['expires_at_ms'] as int,
          ),
        ),
      ),
    );
  });

  @override
  Future<void> answer(String promptId, String? choice) => guardModuleRequest(
    () => _send({
      'operation': 'answer_prompt',
      'prompt_id': promptId,
      'choice': choice,
    }),
  );
}
