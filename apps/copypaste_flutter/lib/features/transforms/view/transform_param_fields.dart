import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_tokens.dart';
import '../models/transform_models.dart';

/// Editors for one step's parameters, shared by the palette and the
/// pipeline editor. A value the parameter refuses is not applied.
class TransformParamFields extends StatelessWidget {
  const TransformParamFields({
    super.key,
    required this.params,
    required this.valueOf,
    required this.onChanged,
    required this.keyPrefix,
    this.enabled = true,
  });

  final List<TransformParam> params;
  final Object? Function(TransformParam param) valueOf;
  final void Function(TransformParam param, Object? value) onChanged;

  /// Distinguishes the fields of different steps.
  final String keyPrefix;
  final bool enabled;

  @override
  Widget build(BuildContext context) => Column(
    mainAxisSize: MainAxisSize.min,
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: [
      for (final (index, param) in params.indexed) ...[
        if (index > 0) const Gap(AppSpacing.md),
        _field(context, param),
      ],
    ],
  );

  Widget _field(BuildContext context, TransformParam param) {
    final key = ValueKey('$keyPrefix-${param.key}');
    final value = valueOf(param);
    if (param.kind == TransformParamKind.boolean) {
      return MergeSemantics(
        child: Row(
          children: [
            Expanded(child: Text(param.label).small()),
            const Gap(AppSpacing.md),
            AppSwitchFrame(
              child: Switch(
                key: key,
                value: value == true,
                enabled: enabled,
                onChanged: enabled ? (next) => onChanged(param, next) : null,
              ),
            ),
          ],
        ),
      );
    }
    final Widget control = switch (param.kind) {
      TransformParamKind.choice => Select<String>(
        key: key,
        enabled: enabled,
        value: value is String ? value : null,
        onChanged: (next) {
          if (next != null) onChanged(param, next);
        },
        itemBuilder: (context, selected) => Text(
          param.options
                  .where((option) => option.value == selected)
                  .firstOrNull
                  ?.label ??
              selected,
        ),
        popup: SelectPopup<String>(
          items: SelectItemList(
            children: [
              for (final option in param.options)
                SelectItemButton<String>(
                  value: option.value,
                  child: Text(option.label),
                ),
            ],
          ),
        ).call,
      ),
      TransformParamKind.integer => TextField(
        key: key,
        enabled: enabled,
        initialValue: value is int ? '$value' : '',
        keyboardType: TextInputType.number,
        decoration: AppOverlays.dialogFieldDecoration(context),
        onChanged: (text) {
          final parsed = int.tryParse(text.trim());
          if (parsed != null) onChanged(param, parsed);
        },
      ),
      _ => TextField(
        key: key,
        enabled: enabled,
        initialValue: value is String ? value : '',
        autocorrect: false,
        enableSuggestions: false,
        decoration: AppOverlays.dialogFieldDecoration(context),
        onChanged: (text) => onChanged(param, text),
      ),
    };
    return Semantics(
      label: param.label,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          ExcludeSemantics(child: Text(param.label).small()),
          const Gap(AppSpacing.xs),
          control,
        ],
      ),
    );
  }
}
