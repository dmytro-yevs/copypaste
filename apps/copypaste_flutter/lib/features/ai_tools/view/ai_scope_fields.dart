import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../models/ai_tools_models.dart';
import '../presentation/ai_tools_labels.dart';

/// Scope checkboxes and the history window of one connection, shared by
/// the Connect, Edit and approval dialogs.
class AiScopeFields extends StatelessWidget {
  const AiScopeFields({
    super.key,
    required this.scopes,
    required this.lookback,
    required this.onScopesChanged,
    required this.onLookbackChanged,
    this.enabled = true,
  });

  final Set<AiToolScope> scopes;
  final AiToolLookback lookback;
  final ValueChanged<Set<AiToolScope>> onScopesChanged;
  final ValueChanged<AiToolLookback> onLookbackChanged;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Semantics(header: true, child: Text(l10n.aiToolsScopesLabel).small()),
        const Gap(AppSpacing.xs),
        for (final scope in AiToolScope.values)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: AppSpacing.xxs),
            child: Checkbox(
              key: ValueKey('ai-scope-${scope.name}'),
              state: scopes.contains(scope)
                  ? CheckboxState.checked
                  : CheckboxState.unchecked,
              onChanged: enabled
                  ? (state) => onScopesChanged({
                      for (final value in AiToolScope.values)
                        if (value == scope
                            ? state == CheckboxState.checked
                            : scopes.contains(value))
                          value,
                    })
                  : null,
              trailing: Flexible(child: Text(l10n.aiToolsScope(scope))),
            ),
          ),
        const Gap(AppSpacing.md),
        Text(l10n.aiToolsLookbackLabel).small(),
        const Gap(AppSpacing.xs),
        AiLookbackSelect(
          key: const ValueKey('ai-lookback'),
          value: lookback,
          enabled: enabled,
          onChanged: onLookbackChanged,
        ),
      ],
    );
  }
}

/// The history-window picker.
class AiLookbackSelect extends StatelessWidget {
  const AiLookbackSelect({
    super.key,
    required this.value,
    required this.onChanged,
    this.enabled = true,
  });

  final AiToolLookback value;
  final ValueChanged<AiToolLookback> onChanged;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Select<AiToolLookback>(
      enabled: enabled,
      value: value,
      onChanged: (next) {
        if (next != null) onChanged(next);
      },
      itemBuilder: (context, lookback) => Text(l10n.aiToolsLookback(lookback)),
      popup: SelectPopup<AiToolLookback>(
        items: SelectItemList(
          children: [
            for (final lookback in AiToolLookback.values)
              SelectItemButton<AiToolLookback>(
                value: lookback,
                child: Text(l10n.aiToolsLookback(lookback)),
              ),
          ],
        ),
      ).call,
    );
  }
}
