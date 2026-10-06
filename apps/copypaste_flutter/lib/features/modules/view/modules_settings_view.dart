import 'package:shadcn_flutter/shadcn_flutter.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';
import '../controller/modules_controller.dart';
import '../controller/module_form_draft.dart';
import '../models/module_models.dart';

/// Installed module management and command forms share the host's components.
class ModulesSettingsView extends StatelessWidget {
  const ModulesSettingsView({super.key, required this.controller});
  final ModulesController controller;

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: controller,
    builder: (context, _) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          children: [
            Button.secondary(
              onPressed: controller.busy ? null : controller.install,
              leading: const Icon(LucideIcons.download, size: AppIconSize.sm),
              child: const Text('Install or update module'),
            ),
          ],
        ),
        const Gap(AppSpacing.lg),
        if (controller.errorMessage != null &&
            controller.state == ModulesLoadState.ready) ...[
          StateView.error(
            title: 'Module operation failed',
            message: controller.errorMessage!,
          ),
          const Gap(AppSpacing.lg),
        ],
        switch (controller.state) {
          ModulesLoadState.loading => const StateView.loading(
            message: 'Loading modules.',
          ),
          ModulesLoadState.error => StateView.error(
            title: 'Modules are unavailable',
            message: controller.errorMessage ?? 'Try loading modules again.',
            actionLabel: 'Try again',
            onAction: controller.initialize,
          ),
          ModulesLoadState.ready =>
            controller.modules.isEmpty
                ? const StateView.empty(
                    title: 'No modules installed',
                    message: 'Install a module to add features to CopyPaste.',
                  )
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      for (final module in controller.modules) ...[
                        _moduleCard(context, module),
                        const Gap(AppSpacing.md),
                      ],
                    ],
                  ),
        },
      ],
    ),
  );

  Widget _moduleCard(BuildContext context, InstalledModule module) => Card(
    key: ValueKey('module-${module.id}'),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(module.title).semiBold(),
                  Text(module.version).small().muted(),
                ],
              ),
            ),
            Switch(
              value: module.enabled,
              onChanged: controller.busy || module.error != null
                  ? null
                  : (value) => controller.setEnabled(module.id, value),
            ),
          ],
        ),
        const Gap(AppSpacing.sm),
        Text(module.error ?? module.description).small().muted(),
        const Gap(AppSpacing.md),
        Wrap(
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          children: [
            for (final command in module.commands)
              Button.secondary(
                onPressed:
                    controller.busy || !module.enabled || module.error != null
                    ? null
                    : () => _invoke(context, module, command),
                child: Text(command.title),
              ),
            if (module.preferenceFields.isNotEmpty)
              Button.ghost(
                onPressed: controller.busy || module.error != null
                    ? null
                    : () => _preferences(context, module),
                child: const Text('Settings'),
              ),
            if (module.restartRequired && controller.canRestart)
              Button.secondary(
                onPressed: controller.busy
                    ? null
                    : controller.restartApplication,
                child: const Text('Restart CopyPaste'),
              ),
            Button.ghost(
              onPressed: controller.busy || module.restartRequired
                  ? null
                  : () => _remove(context, module),
              child: const Text('Remove'),
            ),
          ],
        ),
      ],
    ),
  );

  Future<void> _preferences(
    BuildContext context,
    InstalledModule module,
  ) async {
    final draft = await _fieldsDialog(
      context,
      controller: controller,
      title: '${module.title} settings',
      fields: module.preferenceFields,
      values: module.preferences,
      action: 'Save',
    );
    if (draft == null) return;
    try {
      await controller.setPreferences(module.id, draft.values);
    } finally {
      await draft.close();
    }
  }

  Future<void> _invoke(
    BuildContext context,
    InstalledModule module,
    ModuleCommand command,
  ) async {
    final draft = command.arguments.isEmpty
        ? controller.form(const [], const {})
        : await _fieldsDialog(
            context,
            controller: controller,
            title: command.title,
            fields: command.arguments,
            values: const {},
            action: 'Run',
          );
    if (draft == null) return;
    ModuleResult? result;
    try {
      result = await controller.invoke(module.id, command.id, draft.values);
    } finally {
      await draft.close();
    }
    if (result == null || !context.mounted) return;
    await AppOverlays.showDialog<void>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.puzzle,
        title: Text(command.title),
        content: SingleChildScrollView(child: SelectableText(result!.text)),
        actions: [
          Button.primary(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }

  Future<void> _remove(BuildContext context, InstalledModule module) async {
    final confirmed = await AppOverlays.showDialog<bool>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.trash2,
        title: Text('Remove ${module.title}?'),
        content: const Text(
          'The module and its local settings will be removed.',
        ),
        actions: [
          Button.ghost(
            onPressed: () => Navigator.pop(dialogContext, false),
            child: const Text('Cancel'),
          ),
          Button.destructive(
            onPressed: () => Navigator.pop(dialogContext, true),
            child: const Text('Remove'),
          ),
        ],
      ),
    );
    if (confirmed == true) await controller.remove(module.id);
  }
}

Future<ModuleFormDraft?> _fieldsDialog(
  BuildContext context, {
  required ModulesController controller,
  required String title,
  required List<ModuleField> fields,
  required Map<String, Object> values,
  required String action,
}) async {
  final draft = controller.form(fields, values);
  final confirmed = await AppOverlays.showDialog<bool>(
    context,
    builder: (_) =>
        _ModuleFieldsDialog(title: title, draft: draft, action: action),
  );
  if (confirmed == true) return draft;
  await draft.close();
  return null;
}

class _ModuleFieldsDialog extends StatelessWidget {
  const _ModuleFieldsDialog({
    required this.title,
    required this.draft,
    required this.action,
  });
  final String title;
  final ModuleFormDraft draft;
  final String action;
  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: draft,
    builder: (context, _) => AppOverlays.alertDialog(
      icon: LucideIcons.puzzle,
      title: Text(title),
      content: SingleChildScrollView(
        child: Column(
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
                  onChanged: (value) => draft.setValue(field.id, value),
                )
              else ...[
                Text(field.title).small(),
                const Gap(AppSpacing.xs),
                if (field.kind == ModuleFieldKind.file)
                  Button.secondary(
                    onPressed: draft.busy
                        ? null
                        : () => draft.chooseFile(field),
                    leading: const Icon(LucideIcons.file, size: AppIconSize.sm),
                    child: Text(draft.fileName(field.id) ?? 'Choose file'),
                  )
                else
                  TextArea(
                    key: ValueKey(field.id),
                    initialValue: draft.values[field.id] as String,
                    decoration: AppOverlays.dialogFieldDecoration(context),
                    onChanged: (value) => draft.setValue(field.id, value),
                  ),
              ],
              const Gap(AppSpacing.md),
            ],
          ],
        ),
      ),
      actions: [
        Button.ghost(
          onPressed: () => Navigator.pop(context, false),
          child: const Text('Cancel'),
        ),
        Button.primary(
          onPressed: draft.valid && !draft.busy
              ? () => Navigator.pop(context, true)
              : null,
          child: Text(action),
        ),
      ],
    ),
  );
}
