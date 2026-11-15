import 'dart:async';

import 'package:copypaste_flutter/features/content_rules/models/content_rule_models.dart';
import 'package:copypaste_flutter/features/content_rules/repository/content_rule_repository.dart';

ContentRule rule(
  String id, {
  bool builtin = false,
  String definitionJson =
      '{"match":"all","conditions":[],"actions":[{"type":"mark_secret"}]}',
}) => ContentRule(
  id: id,
  name: 'Rule $id',
  definitionJson: definitionJson,
  builtin: builtin,
);

/// An in-memory [ContentRuleRepository] that records the writes it receives.
class FakeContentRuleRepository implements ContentRuleRepository {
  FakeContentRuleRepository({
    List<ContentRule> rules = const [],
    this.problems = _valid,
  }) : rules = [...rules];

  final List<ContentRule> rules;

  /// What the runtime would report for a definition.
  List<RuleProblem> Function(String definitionJson) problems;
  final validated = <String>[];
  final reorders = <(String, String?)>[];
  final _changes = StreamController<void>.broadcast();

  static List<RuleProblem> _valid(String _) => const [];

  @override
  Future<RuleList> list() async => RuleList(rules: List.of(rules));

  @override
  Future<RuleOptions> options() async => const RuleOptions();

  @override
  Future<List<RuleProblem>> validate(String definitionJson) async {
    validated.add(definitionJson);
    return problems(definitionJson);
  }

  @override
  Future<void> reorder(String id, {String? afterId}) async {
    reorders.add((id, afterId));
    final moved = rules.firstWhere((rule) => rule.id == id);
    rules.remove(moved);
    final at = afterId == null
        ? 0
        : rules.indexWhere((rule) => rule.id == afterId) + 1;
    rules.insert(at, moved);
  }

  @override
  Future<ContentRule> save({
    String? id,
    required String name,
    required bool enabled,
    required String definitionJson,
    List<String> deviceIds = const [],
  }) async =>
      ContentRule(id: id ?? 'new', name: name, definitionJson: definitionJson);

  @override
  Stream<void> changes() => _changes.stream;

  @override
  Future<void> delete(String id) async {}

  @override
  Future<void> restore(String id) async {}

  @override
  Future<ContentRule> setEnabled(String id, {required bool enabled}) async =>
      rules.firstWhere((rule) => rule.id == id);

  @override
  Future<ContentRule> resetBuiltin(String id) async =>
      rules.firstWhere((rule) => rule.id == id);

  @override
  Future<ContentRule> restoreConflict(String conflictId) =>
      throw UnimplementedError();

  @override
  Future<RuleTrace> test({
    String? definitionJson,
    String? ruleId,
    required String sampleText,
    bool fullSet = false,
  }) async => const RuleTrace();

  @override
  Future<RuleTrace> clipTrace(String clipId) async => const RuleTrace();

  @override
  Future<RulePreview> preview(
    String requestId, {
    String? definitionJson,
    String? ruleId,
    bool fullSet = false,
    int limit = RuleLimits.previewItems,
  }) async => const RulePreview(token: 'plan');

  @override
  Future<RuleApplyReport> apply(String token, String requestId) async =>
      const RuleApplyReport();

  @override
  Stream<RuleApplyProgress> applyProgress(String requestId) =>
      const Stream.empty();

  @override
  Future<void> cancel(String requestId) async {}

  @override
  Future<List<RuleSkipEvent>> skipLog() async => const [];

  @override
  Future<String?> transformOutput(
    String text,
    List<RuleTransformChoice> transforms,
  ) async => null;
}
