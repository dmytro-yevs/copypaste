import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:copypaste_flutter/app/theme/app_theme.dart';
import 'package:copypaste_flutter/features/modules/controller/module_prompt_controller.dart';
import 'package:copypaste_flutter/features/modules/models/module_prompt.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';
import 'package:copypaste_flutter/features/modules/view/module_prompt_presenter.dart';

class _Prompts implements ModulePromptsRepository {
  final events = StreamController<void>.broadcast();
  List<ModulePrompt> prompts = [];
  final answers = <(String, String?)>[];

  void publish(List<ModulePrompt> next) {
    prompts = next;
    events.add(null);
  }

  @override
  Stream<void> changes() => events.stream;

  @override
  Future<List<ModulePrompt>> pending() async => prompts;

  @override
  Future<void> answer(String promptId, String? choice) async {
    answers.add((promptId, choice));
    prompts = [
      for (final prompt in prompts)
        if (prompt.id != promptId) prompt,
    ];
  }
}

class _Notifier implements ModulePromptNotifier {
  final openedEvents = StreamController<String>.broadcast();
  final shown = <String>[];
  var dismissed = 0;

  @override
  Stream<String> get opened => openedEvents.stream;

  @override
  Future<void> show(ModulePrompt prompt) async => shown.add(prompt.id);

  @override
  Future<void> dismiss() async => dismissed++;
}

ModulePrompt _prompt(String id) => ModulePrompt(
  id: id,
  moduleId: 'copypaste.ai-tools',
  title: 'Allow Claude?',
  body: 'Claude wants to read your history.',
  choices: const [
    ModulePromptChoice(id: 'deny', title: 'Deny'),
    ModulePromptChoice(id: 'allow', title: 'Allow'),
  ],
  expiresAt: DateTime.now().add(const Duration(minutes: 5)),
);

void main() {
  testWidgets('answers a module prompt and closes one resolved elsewhere', (
    tester,
  ) async {
    final repository = _Prompts();
    final controller = ModulePromptController(
      repository: repository,
      moduleTitle: (id) => id == 'copypaste.ai-tools' ? 'AI Tools' : null,
    );
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: ModulePromptPresenter(
          controller: controller,
          child: const Scaffold(child: SizedBox.expand()),
        ),
      ),
    );
    await tester.pumpAndSettle();

    repository.publish([_prompt('one')]);
    await tester.pumpAndSettle();
    expect(find.text('Allow Claude?'), findsOneWidget);
    expect(find.text('From AI Tools'), findsOneWidget);
    expect(find.text('Claude wants to read your history.'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('module-prompt-choice-allow')));
    await tester.pumpAndSettle();
    expect(repository.answers, [('one', 'allow')]);
    expect(find.text('Allow Claude?'), findsNothing);

    // A prompt that expires or is answered elsewhere closes unanswered.
    repository.publish([_prompt('two')]);
    await tester.pumpAndSettle();
    expect(find.text('Allow Claude?'), findsOneWidget);
    repository.publish([]);
    await tester.pumpAndSettle();
    expect(find.text('Allow Claude?'), findsNothing);
    expect(repository.answers, [('one', 'allow')]);
  });

  testWidgets('drops a prompt locally when it expires', (tester) async {
    var now = DateTime(2026, 11, 14);
    final repository = _Prompts()
      ..prompts = [
        ModulePrompt(
          id: 'late',
          moduleId: 'copypaste.gone',
          title: 'Continue?',
          body: '',
          choices: const [ModulePromptChoice(id: 'ok', title: 'OK')],
          expiresAt: now.add(const Duration(seconds: 2)),
        ),
      ];
    final controller = ModulePromptController(
      repository: repository,
      moduleTitle: (_) => null,
      now: () => now,
    );
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      ShadcnApp(
        theme: AppTheme.light,
        builder: AppTheme.builder,
        home: ModulePromptPresenter(
          controller: controller,
          child: const Scaffold(child: SizedBox.expand()),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('From Unknown module'), findsOneWidget);
    now = now.add(const Duration(seconds: 3));
    await tester.pump(const Duration(seconds: 3));
    await tester.pumpAndSettle();
    expect(find.text('Continue?'), findsNothing);
    expect(repository.answers, isEmpty);
  });

  test('notifies while hidden and shows the app when opened', () async {
    final repository = _Prompts();
    final notifier = _Notifier();
    final visible = ValueNotifier(false);
    var shownApp = 0;
    final controller = ModulePromptController(
      repository: repository,
      moduleTitle: (_) => null,
      notifier: notifier,
      visible: visible,
      showApp: () {
        shownApp++;
        visible.value = true;
      },
    );
    addTearDown(controller.dispose);
    repository.publish([_prompt('one')]);
    await pumpEventQueue();
    repository.publish([_prompt('one')]);
    await pumpEventQueue();
    expect(notifier.shown, ['one']);

    notifier.openedEvents.add('one');
    await pumpEventQueue();
    expect(shownApp, 1);
    expect(notifier.dismissed, 1);
    expect(controller.current?.id, 'one');

    await controller.resolve(controller.current!, 'allow');
    visible.value = false;
    expect(notifier.shown, ['one']);
  });
}
