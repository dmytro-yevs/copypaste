import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';
import '../controller/modules_controller.dart';
import '../controller/module_form_draft.dart';
import '../models/module_models.dart';
import '../models/module_marketplace_models.dart';

/// Marketplace and installed modules share the host's components and state.
class ModulesSettingsView extends StatefulWidget {
  const ModulesSettingsView({super.key, required this.controller});
  final ModulesController controller;

  @override
  State<ModulesSettingsView> createState() => _ModulesSettingsViewState();
}

class _ModulesSettingsViewState extends State<ModulesSettingsView> {
  ModulesController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    unawaited(controller.ensureMarketplace());
  }

  @override
  void didUpdateWidget(ModulesSettingsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != controller) {
      unawaited(controller.ensureMarketplace());
    }
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: controller,
    builder: (context, _) => Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Tabs(
                expand: true,
                index: controller.section.index,
                onChanged: (index) =>
                    controller.selectSection(ModulesSection.values[index]),
                children: const [
                  TabItem(child: Text('Marketplace')),
                  TabItem(child: Text('Installed')),
                ],
              ),
            ),
            const Gap(AppSpacing.sm),
            Button.ghost(
              onPressed:
                  controller.busy ||
                      controller.catalogState == ModulesLoadState.loading
                  ? null
                  : controller.loadMarketplace,
              leading: const Icon(LucideIcons.refreshCw, size: AppIconSize.sm),
              child: const Text('Refresh'),
            ),
          ],
        ),
        const Gap(AppSpacing.lg),
        TextField(
          initialValue: controller.query,
          placeholder: const Text('Search modules'),
          onChanged: controller.search,
          features: const [
            InputFeature.leading(Icon(LucideIcons.search)),
            InputFeature.clear(),
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
            controller.section == ModulesSection.marketplace
                ? _marketplace(context)
                : controller.filteredModules.isEmpty
                ? StateView.empty(
                    title: controller.modules.isEmpty
                        ? 'No modules installed'
                        : 'No matching modules',
                    message: controller.modules.isEmpty
                        ? 'Choose a module from the marketplace.'
                        : 'Try a different search.',
                  )
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      for (final module in controller.filteredModules) ...[
                        _moduleCard(context, module),
                        const Gap(AppSpacing.md),
                      ],
                    ],
                  ),
        },
      ],
    ),
  );

  Widget _marketplace(BuildContext context) =>
      switch (controller.catalogState) {
        ModulesLoadState.loading => const StateView.loading(
          message: 'Loading marketplace.',
        ),
        ModulesLoadState.error => StateView.error(
          title: 'Marketplace is unavailable',
          message:
              controller.catalogError ?? 'Try loading the marketplace again.',
          actionLabel: 'Try again',
          onAction: controller.loadMarketplace,
        ),
        ModulesLoadState.ready =>
          controller.catalog.isEmpty
              ? StateView.empty(
                  title: controller.query.trim().isEmpty
                      ? 'No modules available'
                      : 'No matching modules',
                  message: controller.query.trim().isEmpty
                      ? 'Check back for modules for this device.'
                      : 'Try a different search.',
                )
              : LayoutBuilder(
                  builder: (context, constraints) {
                    final columns =
                        ((constraints.maxWidth + AppSpacing.md) /
                                (AppLayoutSize.marketplaceCardMinWidth +
                                    AppSpacing.md))
                            .floor()
                            .clamp(1, 3);
                    final width =
                        (constraints.maxWidth - AppSpacing.md * (columns - 1)) /
                        columns;
                    return Wrap(
                      spacing: AppSpacing.md,
                      runSpacing: AppSpacing.md,
                      children: [
                        for (final module in controller.catalog)
                          SizedBox(
                            width: width,
                            child: _marketplaceCard(context, module),
                          ),
                      ],
                    );
                  },
                ),
      };

  Widget _marketplaceCard(BuildContext context, MarketplaceModule module) {
    final installed = controller.installedModule(module.id);
    final update = installed != null && controller.updateFor(installed) != null;
    final active = controller.activeModuleId == module.id;
    return Card(
      key: ValueKey('marketplace-${module.id}'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Align(
            alignment: Alignment.centerLeft,
            child: Icon(LucideIcons.puzzle, size: AppIconSize.xl),
          ),
          const Gap(AppSpacing.lg),
          Text(module.title).semiBold(),
          const Gap(AppSpacing.sm),
          Text(module.description).small().muted(),
          const Gap(AppSpacing.md),
          Text('${module.version} · ${module.downloadSize}').small().muted(),
          const Gap(AppSpacing.lg),
          if (active) ...[
            LinearProgressIndicator(
              value: controller.installing ? null : controller.downloadProgress,
            ),
            const Gap(AppSpacing.sm),
          ],
          Button.primary(
            onPressed:
                controller.busy ||
                    (installed != null &&
                        (!update || installed.restartRequired))
                ? null
                : () => controller.install(module),
            child: Text(
              active
                  ? controller.installing
                        ? 'Installing…'
                        : 'Downloading ${(controller.downloadProgress! * 100).floor()}%'
                  : installed == null
                  ? 'Install'
                  : update
                  ? 'Update'
                  : 'Installed',
            ),
          ),
        ],
      ),
    );
  }

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
            if (controller.updateFor(module) case final update?)
              Button.secondary(
                onPressed: controller.busy || module.restartRequired
                    ? null
                    : () => controller.install(update),
                child: Text(
                  controller.activeModuleId == module.id
                      ? controller.installing
                            ? 'Installing…'
                            : 'Downloading ${(controller.downloadProgress! * 100).floor()}%'
                      : 'Update to ${update.version}',
                ),
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
