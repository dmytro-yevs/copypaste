import 'dart:convert';

import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/clips.dart' as clips;
import 'package:copypaste_flutter/generated/api/transforms.dart' as api;

import '../../../shared/runtime_event_service.dart';
import '../models/transform_models.dart';
import 'transform_repository.dart';

/// [TransformRepository] over the runtime bridge.
class RuntimeTransformRepository implements TransformRepository {
  RuntimeTransformRepository({RuntimeEventService? events})
    : _events = events ?? RuntimeEventService.instance;

  final RuntimeEventService _events;

  @override
  Future<List<TransformDefinition>> catalogue(String locale) => _guard(
    () async => [
      for (final info in await api.transformCatalogue(locale: locale))
        _definition(info),
    ],
  );

  @override
  Future<TransformPreview> preview(
    String requestId,
    TransformSource source,
    List<TransformStep> steps, {
    bool withInput = false,
  }) => _guard(() async {
    // A disabled step passes its input through, so a leading one returns
    // the shown input prefix without another request.
    final echo =
        withInput &&
        steps.isNotEmpty &&
        steps.length < TransformLimits.maxSteps;
    final view = await api.previewTransform(
      requestId: requestId,
      sourceInput: _source(source),
      steps: [
        if (echo) _step(steps.first.copyWith(enabled: false)),
        ...steps.map(_step),
      ],
    );
    final results = view.steps.map(_stepPreview).toList(growable: false);
    return TransformPreview(
      inputCounts: _counts(view.inputCounts),
      input: echo && results.isNotEmpty ? results.first.output : null,
      steps: echo ? results.skip(1).toList(growable: false) : results,
      truncated: view.truncated,
      noChanges: view.noChanges,
      sensitive: view.sensitive,
    );
  });

  @override
  Future<void> cancel(String requestId) =>
      _guard(() => api.cancelTransform(requestId: requestId));

  @override
  Future<TransformRunResult> run(
    TransformSource source, {
    List<TransformStep> steps = const [],
    String? pipelineId,
    required TransformRunAction action,
    bool bypassDefault = false,
  }) => _guard(() async {
    final result = await api.runTransform(
      sourceInput: _source(source),
      steps: steps.map(_step).toList(growable: false),
      pipelineId: pipelineId,
      action: switch (action) {
        TransformRunAction.paste => api.TransformAction.paste,
        TransformRunAction.copy => api.TransformAction.copy,
        TransformRunAction.saveNewClip => api.TransformAction.saveNewClip,
      },
      bypassDefault: bypassDefault,
    );
    return TransformRunResult(
      restoreToken: result.delivered?.token,
      clipId: result.clip?.id,
    );
  });

  @override
  Future<List<String>> suggest(TransformSource source) =>
      _guard(() => api.suggestTransforms(sourceInput: _source(source)));

  @override
  Future<List<TransformPipeline>> pipelines() => _guard(
    () async => [
      for (final pipeline in await api.transformPipelines())
        if (pipeline.deletedAtMs == null) _pipeline(pipeline),
    ],
  );

  @override
  Future<TransformPipeline> savePipeline(TransformPipelineDraft draft) =>
      _guard(
        () async => _pipeline(
          await api.saveTransformPipeline(
            draft: api.TransformPipelineDraft(
              id: draft.id,
              name: draft.name,
              description: draft.description,
              steps: draft.steps.map(_step).toList(growable: false),
            ),
          ),
        ),
      );

  @override
  Future<void> deletePipeline(String id) =>
      _guard(() => api.deleteTransformPipeline(id: id));

  @override
  Future<TransformPreferences> preferences() =>
      _guard(() async => _preferences(await api.transformPreferences()));

  @override
  Future<TransformPreferences> setPreference(String key, Object? value) =>
      _guard(
        () async => _preferences(
          await api.setTransformPreference(
            key: key,
            valueJson: jsonEncode(value),
          ),
        ),
      );

  @override
  Future<TransformPreferences> setMember(
    TransformSet set,
    String member, {
    required bool present,
  }) => _guard(
    () async => _preferences(
      await api.setTransformSetMember(
        set_: set.wireName,
        member: member,
        present: present,
      ),
    ),
  );

  @override
  Future<void> restoreClipboard(String token, Duration after) => _guard(
    () => clips.restoreClipboard(token: token, afterMs: after.inMilliseconds),
  );

  @override
  Stream<void> changes() => _events.libraryChanges(const {
    LibraryKind.pipelines,
    LibraryKind.preferences,
    LibraryKind.setMembers,
  });

  static Future<T> _guard<T>(Future<T> Function() request) async {
    try {
      return await request();
    } on runtime.RuntimeError catch (error) {
      throw TransformException(error.code, error.message);
    }
  }

  static api.TransformSourceInput _source(TransformSource source) =>
      switch (source) {
        TransformClipSource(:final id) => api.TransformSourceInput(
          kind: api.TransformSourceKind.clip,
          clipId: id,
        ),
        TransformTextSource(:final text) => api.TransformSourceInput(
          kind: api.TransformSourceKind.text,
          text: text,
        ),
        TransformClipboardSource() => const api.TransformSourceInput(
          kind: api.TransformSourceKind.clipboard,
        ),
      };

  static api.TransformStepView _step(TransformStep step) =>
      api.TransformStepView(
        transformId: step.transformId,
        version: step.version,
        paramsJson: step.paramsJson,
        enabled: step.enabled,
      );

  static TransformStep _stepFrom(api.TransformStepView step) => TransformStep(
    transformId: step.transformId,
    version: step.version,
    params: _object(step.paramsJson),
    enabled: step.enabled,
  );

  static Map<String, Object?> _object(String json) {
    if (json.trim().isEmpty) return const {};
    try {
      final value = jsonDecode(json);
      return value is Map ? Map<String, Object?>.from(value) : const {};
    } on FormatException {
      return const {};
    }
  }

  static Object? _value(String json) {
    try {
      return jsonDecode(json);
    } on FormatException {
      return null;
    }
  }

  static TransformDefinition _definition(api.TransformInfoView info) =>
      TransformDefinition(
        id: info.id,
        version: info.version,
        category: TransformCategory.fromKey(info.category),
        name: info.name,
        description: info.description,
        exampleIn: info.exampleIn,
        exampleOut: info.exampleOut,
        keywords: info.keywords,
        params: [
          for (final param in info.params)
            TransformParam(
              key: param.key,
              label: param.label,
              kind: switch (param.kind) {
                api.TransformParamKind.choice => TransformParamKind.choice,
                api.TransformParamKind.bool => TransformParamKind.boolean,
                api.TransformParamKind.integer => TransformParamKind.integer,
                api.TransformParamKind.text => TransformParamKind.text,
              },
              defaultValue: _value(param.defaultJson),
              options: [
                for (final option in param.options)
                  TransformParamOption(
                    value: option.value,
                    label: option.label,
                  ),
              ],
              min: param.min?.toInt(),
              max: param.max?.toInt(),
              maxLength: param.maxLen,
            ),
        ],
        canBeDefault: info.flags.canBeDefault,
        needsFreeText: info.flags.needsFreeText,
        sensitive: info.flags.inheritsSensitivity,
        richInput: info.flags.richInput,
        deterministic: info.flags.deterministic,
      );

  static TextCounts _counts(api.TextCountsView counts) => TextCounts(
    chars: counts.chars.toInt(),
    words: counts.words.toInt(),
    lines: counts.lines.toInt(),
    bytes: counts.bytes.toInt(),
  );

  static TransformStepPreview _stepPreview(api.TransformStepResultView step) =>
      TransformStepPreview(
        output: step.outputPrefix,
        counts: _counts(step.counts),
        warnings: [
          for (final warning in step.warnings)
            TransformWarning(code: warning.code, detail: warning.detail),
        ],
        failure: switch (step.error) {
          final error? => TransformIssue(
            code: error.code,
            offset: error.offset?.toInt(),
            line: error.line,
            column: error.column,
          ),
          null => null,
        },
        notRun: step.notRun,
      );

  static TransformPipeline _pipeline(api.TransformPipelineView pipeline) =>
      TransformPipeline(
        id: pipeline.id,
        name: pipeline.name,
        description: pipeline.description,
        steps: pipeline.steps.map(_stepFrom).toList(growable: false),
      );

  static TransformPreferences _preferences(api.TransformPreferencesView view) =>
      TransformPreferences(
        favorites: [
          for (final favorite in view.favorites)
            TransformFavorite(switch (favorite.kind) {
              api.TransformFavoriteKind.transform =>
                TransformFavoriteKind.transform,
              api.TransformFavoriteKind.pipeline =>
                TransformFavoriteKind.pipeline,
            }, favorite.id),
        ],
        hidden: view.hiddenTransforms.toSet(),
        paramDefaults: {
          for (final entry in view.paramDefaults)
            entry.transformId: _object(entry.paramsJson),
        },
        defaultPastePipelineId: view.defaultPastePipelineId,
        appDefaults: [
          for (final entry in view.perAppDefaults)
            TransformAppDefault(
              platform: entry.platform,
              identifier: entry.identifier,
              pipelineId: entry.pipelineId,
            ),
        ],
        trackingRemove: view.trackingRemove,
        trackingAllow: view.trackingAllow,
        restoreClipboardAfterPaste: view.restoreClipboardAfterPaste,
        suggestionsEnabled: view.suggestionsEnabled,
        disabledDetectors: view.disabledDetectors.toSet(),
      );
}
