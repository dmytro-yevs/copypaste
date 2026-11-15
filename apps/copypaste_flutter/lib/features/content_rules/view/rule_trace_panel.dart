import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../models/content_rule_models.dart';
import '../presentation/rule_messages.dart';

/// Explains how rules treated a sample or a clip: each evaluated rule with
/// its conditions (matched, not matched or unknown), then the combined
/// result. Used by the rule tester and the History inspector.
class RuleTracePanel extends StatelessWidget {
  const RuleTracePanel({
    super.key,
    required this.trace,
    this.options = const RuleOptions(),
    this.conditionLabel,
    this.transformedOutput,
  });

  final RuleTrace trace;

  /// Names for the tags and transforms the result lists.
  final RuleOptions options;

  /// Describes condition [index] of rule [ruleId]; null shows its number.
  final String? Function(String ruleId, int index)? conditionLabel;

  /// The sample after the rules' transforms, when it can be shown.
  final String? transformedOutput;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Column(
      key: const ValueKey('rule-trace-panel'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (trace.steps.isEmpty)
          Text(l10n.rulesTraceNoRules).small().muted()
        else
          for (final step in trace.steps) ...[
            _step(context, step),
            const Gap(AppSpacing.sm),
          ],
        const Gap(AppSpacing.sm),
        Text(l10n.rulesTraceOutcomeTitle).small().medium(),
        const Gap(AppSpacing.xs),
        ..._outcome(context),
      ],
    );
  }

  Widget _resultLine(
    BuildContext context,
    RuleTraceResult result,
    String label, {
    String? detail,
  }) {
    final l10n = context.l10n;
    final tone = switch (result) {
      RuleTraceResult.matched => AppStatusTone.success,
      RuleTraceResult.error => AppStatusTone.error,
      RuleTraceResult.unknown => AppStatusTone.warning,
      _ => AppStatusTone.muted,
    };
    final state = l10n.ruleTraceResult(result);
    return MergeSemantics(
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          AppIconFrame(
            size: AppIconSize.sm,
            child: Icon(
              RuleIcons.result(result),
              size: AppIconSize.sm,
              color: AppTheme.statusColor(context, tone),
            ),
          ),
          const Gap(AppSpacing.sm),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(l10n.rulesTraceLine(label, state)).small(),
                if (detail != null) Text(detail).xSmall().muted(),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _step(BuildContext context, RuleTraceStep step) {
    final l10n = context.l10n;
    final name = step.ruleName.isEmpty ? l10n.rulesUntitled : step.ruleName;
    return Card(
      theme: AppTheme.settingsGroupCardTheme,
      padding: const EdgeInsets.all(AppSpacing.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _resultLine(
            context,
            step.result,
            name,
            detail: step.result == RuleTraceResult.error
                ? l10n.rulesTraceRuleError
                : null,
          ),
          for (final condition in step.conditions)
            Padding(
              padding: const EdgeInsetsDirectional.only(
                start: AppSpacing.xl,
                top: AppSpacing.xs,
              ),
              child: _resultLine(
                context,
                condition.result,
                conditionLabel?.call(step.ruleId, condition.index) ??
                    l10n.rulesTraceCondition(condition.index + 1),
                detail: condition.result == RuleTraceResult.unknown
                    ? l10n.rulesTraceUnknownReason
                    : null,
              ),
            ),
          if (step.actions.isNotEmpty) ...[
            const Gap(AppSpacing.xs),
            Padding(
              padding: const EdgeInsetsDirectional.only(start: AppSpacing.xl),
              child: Text(
                step.actions.map(l10n.rulesActionType).join(', '),
              ).xSmall().muted(),
            ),
          ],
          if (step.stopped) ...[
            const Gap(AppSpacing.xs),
            Padding(
              padding: const EdgeInsetsDirectional.only(start: AppSpacing.xl),
              child: Text(l10n.rulesTraceStopped).xSmall().muted(),
            ),
          ],
        ],
      ),
    );
  }

  List<Widget> _outcome(BuildContext context) {
    final l10n = context.l10n;
    if (trace.noEffect) return [Text(l10n.rulesTraceNoEffect).small().muted()];
    Widget line(IconData icon, String text) => Padding(
      padding: const EdgeInsets.only(bottom: AppSpacing.xs),
      child: Row(
        children: [
          AppIconFrame(
            size: AppIconSize.sm,
            child: Icon(icon, size: AppIconSize.sm),
          ),
          const Gap(AppSpacing.sm),
          Expanded(child: Text(text).small()),
        ],
      ),
    );
    return [
      if (trace.skipped)
        line(RuleIcons.action(RuleActionType.dontSave), l10n.rulesTraceSkipped),
      if (trace.secret)
        line(
          RuleIcons.action(RuleActionType.markSecret),
          l10n.rulesTraceSecret,
        ),
      if (trace.expiresAfter case final expiry?)
        line(
          RuleIcons.action(RuleActionType.deleteAfter),
          l10n.rulesTraceExpires(l10n.ruleDuration(expiry)),
        ),
      if (trace.tagIds.isNotEmpty)
        line(
          RuleIcons.action(RuleActionType.addTags),
          l10n.rulesTraceTags(
            trace.tagIds.map((id) => options.tagName(id) ?? id).join(', '),
          ),
        ),
      if (trace.transformIds.isNotEmpty)
        line(
          RuleIcons.action(RuleActionType.transform),
          l10n.rulesTraceTransforms(
            trace.transformIds
                .map((id) => options.transform(id)?.name ?? id)
                .join(', '),
          ),
        ),
      if (transformedOutput case final output?) ...[
        const Gap(AppSpacing.xs),
        Text(l10n.rulesTraceOutput).small().medium(),
        const Gap(AppSpacing.xs),
        Card(
          theme: AppTheme.settingsGroupCardTheme,
          padding: const EdgeInsets.all(AppSpacing.md),
          child: Text(
            output,
            key: const ValueKey('rule-trace-output'),
            maxLines: 12,
            overflow: TextOverflow.ellipsis,
          ).small(),
        ),
      ],
    ];
  }
}
