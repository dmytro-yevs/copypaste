import 'package:copypaste_flutter/generated/api.dart' as runtime;
import 'package:copypaste_flutter/generated/api/clips.dart' as clips;
import 'package:copypaste_flutter/generated/api/rules.dart' as api;

import '../../../l10n/l10n.dart';
import '../../../shared/runtime_event_service.dart';
import '../../transforms/models/transform_models.dart';
import '../../transforms/repository/runtime_transform_repository.dart';
import '../../transforms/repository/transform_repository.dart';
import '../models/content_rule_models.dart';
import 'content_rule_repository.dart';

/// [ContentRuleRepository] over the runtime bridge. Transform names and
/// outputs come from the transforms feature's repository.
class RuntimeContentRuleRepository implements ContentRuleRepository {
  RuntimeContentRuleRepository({
    RuntimeEventService? events,
    TransformRepository? transforms,
  }) : _events = events ?? RuntimeEventService.instance,
       _transforms = transforms ?? RuntimeTransformRepository(events: events);

  final RuntimeEventService _events;
  final TransformRepository _transforms;
  int _sequence = 0;

  @override
  Future<RuleList> list() => _guard(() async {
    final list = await api.contentRules();
    return RuleList(
      privacyDefaults: RulePrivacyDefaults(
        skipSecret: list.privacyDefaults.skipSecret,
        skipTransient: list.privacyDefaults.skipTransient,
        excludedAppCount: list.privacyDefaults.excludedAppCount,
      ),
      rules: list.rules.map(_rule).toList(growable: false),
      conflicts: [
        for (final conflict in list.conflicts)
          RuleConflict(
            id: conflict.id,
            ruleId: conflict.ruleId,
            fromDevice: conflict.fromDevice,
            createdAt: _time(conflict.createdAtMs),
          ),
      ],
    );
  });

  @override
  Future<ContentRule> save({
    String? id,
    required String name,
    required bool enabled,
    required String definitionJson,
    List<String> deviceIds = const [],
  }) => _guard(
    () async => _rule(
      await api.saveContentRule(
        draft: api.RuleDraftInput(
          id: id,
          name: name,
          enabled: enabled,
          definitionJson: definitionJson,
          deviceIds: deviceIds,
        ),
      ),
    ),
  );

  @override
  Future<void> delete(String id) => _guard(() => api.deleteContentRule(id: id));

  @override
  Future<void> restore(String id) =>
      _guard(() => api.restoreContentRule(id: id));

  @override
  Future<void> reorder(String id, {String? afterId}) =>
      _guard(() => api.reorderContentRule(id: id, afterId: afterId));

  @override
  Future<ContentRule> setEnabled(String id, {required bool enabled}) => _guard(
    () async =>
        _rule(await api.setContentRuleEnabled(id: id, enabled: enabled)),
  );

  @override
  Future<ContentRule> resetBuiltin(String id) =>
      _guard(() async => _rule(await api.resetBuiltinContentRule(id: id)));

  @override
  Future<ContentRule> restoreConflict(String conflictId) => _guard(
    () async =>
        _rule(await api.restoreContentRuleConflict(conflictId: conflictId)),
  );

  @override
  Future<List<RuleProblem>> validate(String definitionJson) => _guard(
    () async => [
      for (final error in await api.validateContentRule(
        definitionJson: definitionJson,
      ))
        RuleProblem(
          path: error.path,
          code: error.messageCode,
          offset: error.offset,
        ),
    ],
  );

  @override
  Future<RuleTrace> test({
    String? definitionJson,
    String? ruleId,
    required String sampleText,
    bool fullSet = false,
  }) => _guard(
    () async => _trace(
      await api.testContentRule(
        definitionJson: definitionJson,
        ruleId: ruleId,
        sampleText: sampleText,
        fullSet: fullSet,
      ),
    ),
  );

  @override
  Future<RuleTrace> clipTrace(String clipId) =>
      _guard(() async => _trace(await clips.clipRuleTrace(id: clipId)));

  @override
  Future<RulePreview> preview(
    String requestId, {
    String? definitionJson,
    String? ruleId,
    bool fullSet = false,
    int limit = RuleLimits.previewItems,
  }) => _guard(() async {
    final preview = await api.previewContentRule(
      requestId: requestId,
      definitionJson: definitionJson,
      ruleId: ruleId,
      fullSet: fullSet,
      limit: limit,
    );
    final counts = preview.counts;
    return RulePreview(
      token: preview.token,
      counts: RulePreviewCounts(
        matched: counts.matched,
        deleted: counts.deleted,
        secret: counts.secret,
        expiring: counts.expiring,
        tagged: counts.tagged,
      ),
      items: [
        for (final item in preview.items)
          RulePreviewItem(
            id: item.id,
            preview: item.masked ? '' : item.preview,
            masked: item.masked,
            createdAt: _time(item.createdAtMs),
            sourceAppName: item.sourceAppName,
          ),
      ],
      truncated: preview.truncated,
    );
  });

  @override
  Future<RuleApplyReport> apply(String token, String requestId) =>
      _guard(() async {
        final report = await api.applyContentRule(
          previewToken: token,
          requestId: requestId,
        );
        return RuleApplyReport(
          changed: report.changed,
          deleted: report.deleted,
          failed: report.failed,
          cancelled: report.cancelled,
        );
      });

  @override
  Stream<RuleApplyProgress> applyProgress(String requestId) => _events
      .eventsOf(const {RuntimeEventKind.operationProgress})
      .map((event) => event.operation)
      .where((operation) => operation?.requestId == requestId)
      .map(
        (operation) => RuleApplyProgress(
          done: operation!.done.toInt(),
          total: operation.total.toInt(),
        ),
      );

  @override
  Future<void> cancel(String requestId) =>
      _guard(() => api.cancelContentRuleWork(requestId: requestId));

  @override
  Future<List<RuleSkipEvent>> skipLog() => _guard(
    () async => [
      for (final event in await api.contentRuleSkipLog())
        RuleSkipEvent(
          at: _time(event.atMs),
          ruleName: event.ruleName,
          sourceAppName: event.sourceAppName,
        ),
    ],
  );

  @override
  Future<RuleOptions> options() => _guard(() async {
    final facets = await runtime.historyFacets();
    List<TransformDefinition> catalogue;
    try {
      catalogue = await _transforms.catalogue(AppStrings.current.localeName);
    } on TransformException {
      catalogue = const [];
    }
    return RuleOptions(
      devices: [
        for (final device in facets.originDevices)
          RuleChoice(id: device.id, label: device.label),
      ],
      sourceApps: [
        for (final app in facets.sourceApps)
          RuleChoice(id: app.id, label: app.label),
      ],
      tags: [
        for (final tag in facets.tags) RuleChoice(id: tag.id, label: tag.name),
      ],
      transforms: [
        for (final definition in catalogue)
          if (!definition.needsFreeText && !definition.sensitive)
            RuleTransformChoice(
              id: definition.id,
              name: definition.name,
              version: definition.version,
            ),
      ],
    );
  });

  @override
  Future<String?> transformOutput(
    String text,
    List<RuleTransformChoice> transforms,
  ) async {
    if (transforms.isEmpty) return null;
    try {
      final preview = await _transforms.preview(
        'content-rule-test-${identityHashCode(this)}-${++_sequence}',
        TransformSource.text(text),
        [
          for (final transform in transforms)
            TransformStep(
              transformId: transform.id,
              version: transform.version,
            ),
        ],
      );
      if (preview.failure != null || preview.steps.isEmpty) return null;
      return preview.steps.last.output;
    } on TransformException {
      return null;
    }
  }

  @override
  Stream<void> changes() => _events.libraryChanges(const {LibraryKind.rules});

  static Future<T> _guard<T>(Future<T> Function() request) async {
    try {
      return await request();
    } on runtime.RuntimeError catch (error) {
      throw ContentRuleException(error.code, error.message);
    }
  }

  static DateTime _time(int ms) => DateTime.fromMillisecondsSinceEpoch(ms);

  static ContentRule _rule(api.RuleView view) => ContentRule(
    id: view.id,
    name: view.name,
    definitionJson: view.definitionJson,
    builtin: view.builtin,
    enabled: view.enabled,
    orderKey: view.orderKey,
    deviceIds: view.deviceIds,
    errorCode: view.errorBadge,
    newFromDevice: view.newFromDevice,
    unsupported: view.unsupported,
    deletedAt: view.deletedAtMs == null ? null : _time(view.deletedAtMs!),
    matchCount: view.stats.matchCount.toInt(),
    lastMatchedAt: view.stats.lastMatchedAtMs == null
        ? null
        : _time(view.stats.lastMatchedAtMs!),
  );

  static RuleTraceResult _result(api.RuleTraceResult result) =>
      switch (result) {
        api.RuleTraceResult.matched => RuleTraceResult.matched,
        api.RuleTraceResult.notMatched => RuleTraceResult.notMatched,
        api.RuleTraceResult.unknown => RuleTraceResult.unknown,
        api.RuleTraceResult.skipped => RuleTraceResult.skipped,
        api.RuleTraceResult.error => RuleTraceResult.error,
      };

  static RuleTrace _trace(api.RuleTraceView view) => RuleTrace(
    steps: [
      for (final step in view.steps)
        RuleTraceStep(
          ruleId: step.ruleId,
          ruleName: step.ruleName,
          result: _result(step.result),
          conditions: [
            for (final condition in step.conditions)
              RuleTraceCondition(
                index: condition.index,
                result: _result(condition.result),
                unknownReason: condition.unknownReason,
              ),
          ],
          actions: step.actions,
          stopped: step.stopped,
        ),
    ],
    skipped: view.skipped,
    secret: view.secret,
    expiresAfter: view.expiresAfterMs == null
        ? null
        : Duration(milliseconds: view.expiresAfterMs!.toInt()),
    tagIds: view.tagIds,
    transformIds: view.transformIds,
  );
}
