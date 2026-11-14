import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../l10n/l10n.dart';
import '../models/content_rule_models.dart';

/// Rule summaries and labels. Wire names arrive in snake_case and select
/// messages on the same names.
extension RuleMessages on AppLocalizations {
  String ruleDuration(Duration duration) {
    if (duration.inMinutes < 60 || duration.inMinutes % 60 != 0) {
      return rulesMinutes(duration.inMinutes);
    }
    if (duration.inHours < 24 || duration.inHours % 24 != 0) {
      return rulesHours(duration.inHours);
    }
    return rulesDays(duration.inDays);
  }

  String ruleConditionValue(RuleConditionType type, String wire) =>
      switch (type) {
        RuleConditionType.contentType => rulesContentKind(wire),
        RuleConditionType.detectedKind => rulesDetectedKind(wire),
        RuleConditionType.privacyMarker => rulesPrivacyMarker(wire),
        RuleConditionType.captureOrigin => rulesCaptureOrigin(wire),
        _ => wire,
      };

  String ruleTextOperator(RuleTextOp op, {required bool negate}) =>
      negate ? rulesTextOperatorNegated(op.wire) : rulesTextOperator(op.wire);

  String ruleSizeOperator(RuleComparison op, {required bool negate}) =>
      negate ? rulesSizeOperatorNegated(op.wire) : rulesSizeOperator(op.wire);

  String ruleCondition(RuleCondition condition, RuleOptions options) {
    final type = condition.type;
    if (type == null) return rulesConditionType(condition.typeName);
    final attribute = rulesConditionType(type.wire);
    final (operator, value) = switch (type) {
      RuleConditionType.contentText => (
        ruleTextOperator(condition.textOp, negate: condition.negate),
        '“${condition.value}”',
      ),
      RuleConditionType.size => (
        ruleSizeOperator(condition.comparison, negate: condition.negate),
        rulesSizeValue(condition.size, condition.sizeUnit.wire),
      ),
      RuleConditionType.sourceApp => (
        rulesListOperator(condition.negate.toString()),
        condition.values
            .map((id) => options.sourceAppName(id) ?? id)
            .join(', '),
      ),
      _ => (
        rulesListOperator(condition.negate.toString()),
        condition.values
            .map((wire) => ruleConditionValue(type, wire))
            .join(', '),
      ),
    };
    return rulesConditionSummary(attribute, operator, value);
  }

  String ruleAction(RuleAction action, RuleOptions options) {
    final type = action.type;
    final name = rulesActionType(action.typeName);
    return switch (type) {
      RuleActionType.deleteAfter => rulesActionSummary(
        name,
        ruleDuration(action.expiry),
      ),
      RuleActionType.addTags => rulesActionSummary(
        name,
        action.tagIds.map((id) => options.tagName(id) ?? id).join(', '),
      ),
      RuleActionType.transform => rulesActionSummary(
        name,
        options.transform(action.transformId)?.name ?? action.transformId,
      ),
      _ => name,
    };
  }

  /// "… → …", one line per rule in the list.
  String ruleSummary(RuleDefinition definition, RuleOptions options) {
    final conditions = definition.matchesEverything
        ? rulesMatchesEverything
        : definition.conditions
              .map((condition) => _briefCondition(condition, options))
              .join(rulesConditionJoin(definition.matchMode.wire));
    final actions = definition.actions
        .map((action) => ruleAction(action, options))
        .join(', ');
    final summary = rulesSummary(conditions, actions);
    return definition.stopProcessing ? rulesSummaryStop(summary) : summary;
  }

  /// A kind condition reads as its values alone; others keep the attribute.
  String _briefCondition(RuleCondition condition, RuleOptions options) =>
      switch (condition.type) {
        final type?
            when !condition.negate &&
                const {
                  RuleConditionType.contentType,
                  RuleConditionType.detectedKind,
                  RuleConditionType.privacyMarker,
                  RuleConditionType.captureOrigin,
                }.contains(type) =>
          condition.values
              .map((wire) => ruleConditionValue(type, wire))
              .join(', '),
        _ => ruleCondition(condition, options),
      };

  /// Null when the rule runs on every device.
  String? ruleDevices(List<String> ids, RuleOptions options) => ids.isEmpty
      ? null
      : rulesRunsOn(ids.map((id) => options.deviceName(id) ?? id).join(', '));

  String ruleProblem(RuleProblem problem) => rulesProblem(problem.code);

  String ruleActionBlock(RuleActionBlock block) =>
      rulesActionBlock(switch (block) {
        RuleActionBlock.dontSaveAlone => 'dont_save_alone',
        RuleActionBlock.present => 'present',
        RuleActionBlock.full => 'full',
      });

  String ruleTraceResult(RuleTraceResult result) =>
      rulesTraceResult(switch (result) {
        RuleTraceResult.matched => 'matched',
        RuleTraceResult.notMatched => 'not_matched',
        RuleTraceResult.unknown => 'unknown',
        RuleTraceResult.skipped => 'skipped',
        RuleTraceResult.error => 'error',
      });

  /// A condition another platform cannot evaluate (content-rules R-6).
  String? ruleConditionNotice(RuleConditionType? type) => switch (type) {
    RuleConditionType.sourceApp => rulesSourceAppNotice,
    RuleConditionType.privacyMarker => rulesPrivacyMarkerNotice,
    _ => null,
  };
}

/// Icons of rule actions and trace results.
abstract final class RuleIcons {
  static IconData action(RuleActionType? type) => switch (type) {
    RuleActionType.dontSave => LucideIcons.ban,
    RuleActionType.markSecret => LucideIcons.lock,
    RuleActionType.hideFromAi => LucideIcons.eyeOff,
    RuleActionType.deleteAfter => LucideIcons.timer,
    RuleActionType.addTags => LucideIcons.tag,
    RuleActionType.transform => LucideIcons.wandSparkles,
    null => LucideIcons.circleHelp,
  };

  static IconData result(RuleTraceResult result) => switch (result) {
    RuleTraceResult.matched => LucideIcons.circleCheck,
    RuleTraceResult.notMatched => LucideIcons.circleMinus,
    RuleTraceResult.unknown => LucideIcons.circleHelp,
    RuleTraceResult.skipped => LucideIcons.circleDashed,
    RuleTraceResult.error => LucideIcons.circleAlert,
  };
}
