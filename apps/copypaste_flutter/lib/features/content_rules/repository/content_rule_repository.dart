import '../models/content_rule_models.dart';

/// Runtime access for content rules.
///
/// Failures are [ContentRuleException]s carrying the runtime error code.
abstract interface class ContentRuleRepository {
  Future<RuleList> list();

  /// Creates a rule when [id] is null. [deviceIds] empty means every device.
  Future<ContentRule> save({
    String? id,
    required String name,
    required bool enabled,
    required String definitionJson,
    List<String> deviceIds = const [],
  });

  /// Moves the rule to Recently deleted for 30 days.
  Future<void> delete(String id);

  Future<void> restore(String id);

  /// Places [id] right after [afterId] in its tier; null moves it first.
  Future<void> reorder(String id, {String? afterId});

  Future<ContentRule> setEnabled(String id, {required bool enabled});

  Future<ContentRule> resetBuiltin(String id);

  Future<ContentRule> restoreConflict(String conflictId);

  /// Problems the runtime finds in a definition; empty when it is valid.
  Future<List<RuleProblem>> validate(String definitionJson);

  /// Evaluates a draft or saved rule against [sampleText] without storing
  /// it. [fullSet] evaluates it in place among every live rule.
  Future<RuleTrace> test({
    String? definitionJson,
    String? ruleId,
    required String sampleText,
    bool fullSet = false,
  });

  /// How the rules this device runs treat a stored clip.
  Future<RuleTrace> clipTrace(String clipId);

  /// What a draft or saved rule would change in the latest [limit] clips.
  /// Cancelling [requestId] stops it and releases the plan it holds.
  Future<RulePreview> preview(
    String requestId, {
    String? definitionJson,
    String? ruleId,
    bool fullSet = false,
    int limit = RuleLimits.previewItems,
  });

  /// Applies a preview's plan; it completes when the work is done.
  Future<RuleApplyReport> apply(String token, String requestId);

  /// Progress of the apply [requestId] while it runs.
  Stream<RuleApplyProgress> applyProgress(String requestId);

  /// Stops a preview or apply, and releases a preview's plan.
  Future<void> cancel(String requestId);

  /// Captures that rules did not save, newest last. Kept in memory only.
  Future<List<RuleSkipEvent>> skipLog();

  /// Devices, source apps, tags and transforms the editor offers.
  Future<RuleOptions> options();

  /// The output of [transforms] run in order on [text], or null when it
  /// cannot be shown.
  Future<String?> transformOutput(
    String text,
    List<RuleTransformChoice> transforms,
  );

  /// Signals that rules changed, here or on a peer.
  Stream<void> changes();
}
