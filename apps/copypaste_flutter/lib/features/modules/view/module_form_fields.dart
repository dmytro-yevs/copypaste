import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';
import '../controller/module_form_draft.dart';
import '../models/module_models.dart';

/// Renders module fields in preferences and command forms.
class ModuleFormFields extends StatelessWidget {
  const ModuleFormFields({super.key, required this.draft, this.enabled = true});

  final ModuleFormDraft draft;
  final bool enabled;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: draft,
    builder: (context, _) => Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (draft.errorMessage != null) ...[
          StateView.error(
            title: 'File selection failed',
            message: draft.errorMessage!,
          ),
          const Gap(AppSpacing.md),
        ],
        for (final field in draft.fields) ...[
          if (field.kind == ModuleFieldKind.boolean)
            Switch(
              value: draft.values[field.id] as bool,
              leading: Text(field.title),
              onChanged: enabled
                  ? (value) => draft.setValue(field.id, value)
                  : null,
            )
          else ...[
            Text(field.title).small(),
            const Gap(AppSpacing.xs),
            if (field.kind == ModuleFieldKind.choices)
              MultiSelect<String>(
                key: ValueKey('module-choices-${field.id}'),
                enabled: enabled,
                value: (draft.values[field.id] as List).cast<String>(),
                placeholder: const Text('Choose languages'),
                onChanged: (values) => draft.setValue(
                  field.id,
                  (values ?? const <String>[]).toList(),
                ),
                itemBuilder: (context, id) => Text(
                  field.options.firstWhere((option) => option.id == id).title,
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
            else if (field.kind == ModuleFieldKind.file)
              Button.secondary(
                onPressed: !enabled || draft.busy
                    ? null
                    : () => draft.chooseFile(field),
                leading: const Icon(LucideIcons.file, size: AppIconSize.sm),
                child: Text(draft.fileName(field.id) ?? 'Choose file'),
              )
            else if (field.secret)
              TextField(
                key: ValueKey(field.id),
                initialValue: draft.values[field.id] as String,
                enabled: enabled,
                obscureText: true,
                autocorrect: false,
                enableSuggestions: false,
                decoration: AppOverlays.dialogFieldDecoration(context),
                onChanged: enabled
                    ? (value) => draft.setValue(field.id, value)
                    : null,
              )
            else
              TextArea(
                key: ValueKey(field.id),
                initialValue: draft.values[field.id] as String,
                enabled: enabled,
                decoration: AppOverlays.dialogFieldDecoration(context),
                onChanged: enabled
                    ? (value) => draft.setValue(field.id, value)
                    : null,
              ),
          ],
          const Gap(AppSpacing.md),
        ],
        if (draft.selectedModel case final model?)
          Text(
            model.available
                ? '${model.title} model is ready'
                : '${model.title} model · ${(model.sizeBytes / (1024 * 1024)).toStringAsFixed(1)} MiB download',
          ).small().muted(),
      ],
    ),
  );
}
