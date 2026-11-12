import 'dart:async';

import 'package:copypaste_flutter/features/transforms/models/transform_models.dart';
import 'package:copypaste_flutter/features/transforms/repository/transform_repository.dart';

const upper = TransformDefinition(
  id: 'case.upper',
  version: 1,
  category: TransformCategory.caseChange,
  name: 'UPPERCASE',
  description: 'Makes every letter uppercase.',
);

const jsonFormat = TransformDefinition(
  id: 'structure.json_format',
  version: 1,
  category: TransformCategory.structure,
  name: 'Format JSON',
  description: 'Indents JSON.',
  params: [
    TransformParam(
      key: 'indent',
      label: 'Indentation',
      kind: TransformParamKind.choice,
      defaultValue: '2',
      options: [
        TransformParamOption(value: '2', label: '2 spaces'),
        TransformParamOption(value: '4', label: '4 spaces'),
      ],
    ),
  ],
);

TransformPreview previewOf(
  List<TransformStep> steps, {
  TransformIssue? failAt0,
}) => TransformPreview(
  inputCounts: const TextCounts(chars: 3, words: 1, lines: 1, bytes: 3),
  input: 'abc',
  steps: [
    for (final (index, step) in steps.indexed)
      index == 0 && failAt0 != null
          ? TransformStepPreview(output: '', failure: failAt0)
          : TransformStepPreview(
              output: '${step.transformId}:$index',
              counts: const TextCounts(chars: 3, words: 1, lines: 1, bytes: 3),
              notRun: failAt0 != null,
            ),
  ],
);

/// Records every runtime call; previews answer at once unless [hold] is set.
class FakeTransformRepository implements TransformRepository {
  FakeTransformRepository({this.catalogueEntries = const [upper, jsonFormat]});

  final List<TransformDefinition> catalogueEntries;
  final calls = <String>[];
  final previews = <(String, List<TransformStep>)>[];
  final cancelled = <String>[];
  final runs = <(TransformRunAction, List<TransformStep>, bool)>[];
  final restored = <String>[];
  TransformIssue? failPreview;
  TransformException? previewError;
  Completer<void>? hold;
  String? restoreToken = 'restore-1';

  @override
  Future<List<TransformDefinition>> catalogue(String locale) async {
    calls.add('catalogue');
    return catalogueEntries;
  }

  @override
  Future<TransformPreview> preview(
    String requestId,
    TransformSource source,
    List<TransformStep> steps, {
    bool withInput = false,
  }) async {
    calls.add('preview');
    previews.add((requestId, steps));
    if (hold case final hold?) await hold.future;
    if (previewError case final error?) throw error;
    return previewOf(steps, failAt0: failPreview);
  }

  @override
  Future<void> cancel(String requestId) async => cancelled.add(requestId);

  @override
  Future<TransformRunResult> run(
    TransformSource source, {
    List<TransformStep> steps = const [],
    String? pipelineId,
    required TransformRunAction action,
    bool bypassDefault = false,
  }) async {
    calls.add('run');
    runs.add((action, steps, bypassDefault));
    return TransformRunResult(
      restoreToken: action == TransformRunAction.paste ? restoreToken : null,
      clipId: action == TransformRunAction.paste ? null : 'new-clip',
    );
  }

  @override
  Future<List<String>> suggest(TransformSource source) async {
    calls.add('suggest');
    return const [];
  }

  @override
  Future<List<TransformPipeline>> pipelines() async {
    calls.add('pipelines');
    return const [];
  }

  @override
  Future<TransformPipeline> savePipeline(TransformPipelineDraft draft) async =>
      TransformPipeline(id: 'p1', name: draft.name, steps: draft.steps);

  @override
  Future<void> deletePipeline(String id) async {}

  @override
  Future<TransformPreferences> preferences() async {
    calls.add('preferences');
    return const TransformPreferences();
  }

  @override
  Future<TransformPreferences> setPreference(String key, Object? value) async =>
      const TransformPreferences();

  @override
  Future<TransformPreferences> setMember(
    TransformSet set,
    String member, {
    required bool present,
  }) async => const TransformPreferences();

  @override
  Future<void> restoreClipboard(String token, Duration after) async =>
      restored.add(token);

  @override
  Stream<void> changes() => const Stream.empty();
}
