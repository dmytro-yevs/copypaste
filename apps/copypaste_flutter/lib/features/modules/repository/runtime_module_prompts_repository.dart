import 'dart:convert';

import '../../../generated/api.dart' as runtime;
import '../../../shared/runtime_event_service.dart';
import '../models/module_prompt.dart';
import 'modules_repository.dart';
import 'runtime_modules_repository.dart';

/// [ModulePromptsRepository] over the runtime's module prompts and its
/// `module_prompt` events.
class RuntimeModulePromptsRepository implements ModulePromptsRepository {
  RuntimeModulePromptsRepository({
    Future<String> Function()? read,
    Future<void> Function(String promptId, String? choice)? answer,
    RuntimeEventService? events,
  }) : _read = read ?? runtime.modulePrompts,
       _answer =
           answer ??
           ((promptId, choice) =>
               runtime.moduleAnswerPrompt(promptId: promptId, choice: choice)),
       _events = events ?? RuntimeEventService.instance;

  final Future<String> Function() _read;
  final Future<void> Function(String promptId, String? choice) _answer;
  final RuntimeEventService _events;

  @override
  Stream<void> changes() =>
      _events.eventsOf(const {RuntimeEventKind.modulePrompt}).map((_) {});

  @override
  Future<List<ModulePrompt>> pending() => guardModuleRequest(() async {
    final prompts = jsonDecode(await _read()) as List;
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
  Future<void> answer(String promptId, String? choice) =>
      guardModuleRequest(() => _answer(promptId, choice));
}
