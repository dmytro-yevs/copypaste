import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../app/theme/app_icon_frame.dart';
import '../../app/theme/app_overlays.dart';
import '../../app/theme/app_switch_frame.dart';
import '../../app/theme/app_theme.dart';
import '../../app/theme/app_tokens.dart';
import '../../l10n/l10n.dart';
import '../state_view.dart';
import 'dynamic_form_draft.dart';

/// Renders the fields of a [DynamicFormDraft], such as module preferences,
/// module commands and template fill-in forms.
class DynamicFormFields extends StatelessWidget {
  const DynamicFormFields({
    super.key,
    required this.draft,
    this.enabled = true,
    this.footer,
    this.focusNodes = const {},
    this.errors = const {},
    this.onSubmitted,
  });

  final DynamicFormDraft draft;
  final bool enabled;

  /// Owner focus nodes by field id, so the owner can move focus to a field.
  final Map<String, FocusNode> focusNodes;

  /// Inline messages by field id, shown under the field.
  final Map<String, String> errors;

  /// Enter in a single-line field.
  final ValueChanged<String>? onSubmitted;

  /// Owner content after the fields, rebuilt with the draft.
  final Widget? Function(BuildContext context)? footer;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: draft,
    builder: (context, _) => Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (draft.errorMessage != null) ...[
          StateView.error(
            title: context.l10n.modulesFileSelectionFailed,
            message: draft.errorMessage!,
          ),
          const Gap(AppSpacing.md),
        ],
        for (final field in draft.fields) ...[
          if (field.kind == DynamicFormFieldKind.boolean)
            MergeSemantics(
              child: Row(
                children: [
                  Expanded(child: Text(field.title)),
                  const Gap(AppSpacing.md),
                  AppSwitchFrame(
                    child: Switch(
                      value: draft.values[field.id] as bool,
                      enabled: enabled,
                      onChanged: enabled
                          ? (value) => draft.setValue(field.id, value)
                          : null,
                    ),
                  ),
                ],
              ),
            )
          else ...[
            Text(field.title).small(),
            const Gap(AppSpacing.xs),
            if (field.kind == DynamicFormFieldKind.choices)
              MultiSelect<String>(
                key: ValueKey('module-choices-${field.id}'),
                enabled: enabled,
                value: (draft.values[field.id] as List).cast<String>(),
                placeholder: Text(context.l10n.modulesChooseLanguages),
                onChanged: (values) => draft.setValue(
                  field.id,
                  (values ?? const <String>[]).toList(),
                ),
                itemBuilder: (context, id) => MultiSelectChip(
                  value: id,
                  style: AppTheme.selectChipStyle,
                  child: Text(
                    field.options.firstWhere((option) => option.id == id).title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                popup: SelectPopup<String>(
                  items: SelectItemList(
                    children: [
                      for (final option in field.options)
                        SelectItemButton<String>(
                          value: option.id,
                          child: Text(option.title),
                        ),
                    ],
                  ),
                ).call,
              )
            else if (field.kind == DynamicFormFieldKind.file)
              Button.secondary(
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                onPressed: !enabled || draft.busy
                    ? null
                    : () => draft.chooseFile(field),
                leading: const AppIconFrame(
                  size: AppIconSize.sm,
                  child: Icon(LucideIcons.file, size: AppIconSize.sm),
                ),
                child: Text(
                  draft.fileName(field.id) ?? context.l10n.modulesChooseFile,
                ),
              )
            else if (field.kind == DynamicFormFieldKind.choice)
              Select<String>(
                key: ValueKey(field.id),
                focusNode: focusNodes[field.id],
                enabled: enabled,
                value: draft.values[field.id] as String?,
                onChanged: (value) {
                  if (value != null) draft.setValue(field.id, value);
                },
                itemBuilder: (context, id) => Text(
                  field.options
                          .where((option) => option.id == id)
                          .firstOrNull
                          ?.title ??
                      id,
                ),
                popup: SelectPopup<String>(
                  items: SelectItemList(
                    children: [
                      for (final option in field.options)
                        SelectItemButton<String>(
                          value: option.id,
                          child: Text(option.title),
                        ),
                    ],
                  ),
                ).call,
              )
            else if (field.secret || field.kind == DynamicFormFieldKind.line)
              TextField(
                key: ValueKey(field.id),
                focusNode: focusNodes[field.id],
                initialValue: draft.values[field.id] as String,
                enabled: enabled,
                obscureText: field.secret,
                autocorrect: !field.secret,
                enableSuggestions: !field.secret,
                decoration: AppOverlays.dialogFieldDecoration(context),
                onChanged: enabled
                    ? (value) => draft.setValue(field.id, value)
                    : null,
                onSubmitted: onSubmitted,
              )
            else
              TextArea(
                key: ValueKey(field.id),
                focusNode: focusNodes[field.id],
                initialValue: draft.values[field.id] as String,
                enabled: enabled,
                decoration: AppOverlays.dialogFieldDecoration(context),
                onChanged: enabled
                    ? (value) => draft.setValue(field.id, value)
                    : null,
              ),
          ],
          if (errors[field.id] case final error?) ...[
            const Gap(AppSpacing.xs),
            Semantics(
              liveRegion: true,
              child: Text(
                error,
                key: ValueKey('${field.id}-error'),
                style: TextStyle(
                  color: Theme.of(context).colorScheme.destructive,
                ),
              ).small(),
            ),
          ],
          const Gap(AppSpacing.md),
        ],
        ?footer?.call(context),
      ],
    ),
  );
}
