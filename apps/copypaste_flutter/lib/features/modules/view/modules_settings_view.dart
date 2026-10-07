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
                : _cards([
                    for (final module in controller.filteredModules)
                      _moduleCard(
                        context,
                        installed: module,
                        marketplace: controller.marketplaceModule(module.id),
                      ),
                  ]),
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
                      ? 'Modules will appear here when they are published.'
                      : 'Try a different search.',
                )
              : _cards([
                  for (final module in controller.catalog)
                    _moduleCard(
                      context,
                      marketplace: module,
                      installed: controller.installedModule(module.id),
                    ),
                ]),
      };

  Widget _cards(List<Widget> cards) => LayoutBuilder(
    builder: (context, constraints) {
      final columns =
          ((constraints.maxWidth + AppSpacing.md) /
                  (AppLayoutSize.marketplaceCardMinWidth + AppSpacing.md))
              .floor()
              .clamp(1, 3);
      final width =
          (constraints.maxWidth - AppSpacing.md * (columns - 1)) / columns;
      return Wrap(
        spacing: AppSpacing.md,
        runSpacing: AppSpacing.md,
        children: [
          for (final card in cards) SizedBox(width: width, child: card),
        ],
      );
    },
  );

  Widget _moduleCard(
    BuildContext context, {
    MarketplaceModule? marketplace,
    InstalledModule? installed,
  }) {
    final id = installed?.id ?? marketplace!.id;
    final active = controller.activeModuleId == id;
    final size = installed == null
        ? marketplace!.downloadSize
        : formatModuleSize(installed.sizeBytes);
    return Card(
      key: ValueKey('module-$id'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Align(
            alignment: Alignment.centerLeft,
            child: Icon(LucideIcons.puzzle, size: AppIconSize.xl),
          ),
          const Gap(AppSpacing.lg),
          Text(installed?.title ?? marketplace!.title).semiBold(),
          const Gap(AppSpacing.sm),
          Text(
            installed?.description ?? marketplace!.description,
          ).small().muted(),
          const Gap(AppSpacing.md),
          Text(
            [
              installed?.version ?? marketplace!.version.toString(),
              ?size,
            ].join(' · '),
          ).small().muted(),
          if (marketplace?.appRequirement case final requirement?) ...[
            const Gap(AppSpacing.sm),
            Text('Requires $requirement.').small().muted(),
          ],
          if (installed?.error case final error?) ...[
            const Gap(AppSpacing.sm),
            Text(error).small().muted(),
          ] else if (installed == null &&
              marketplace?.unavailableReason != null) ...[
            const Gap(AppSpacing.sm),
            Text(marketplace!.unavailableReason!).small().muted(),
          ] else if (marketplace?.systemRequirement
              case final requirement?) ...[
            const Gap(AppSpacing.sm),
            Text('Requires $requirement.').small().muted(),
          ],
          const Gap(AppSpacing.lg),
          if (active) ...[
            LinearProgressIndicator(
              value: controller.installing ? null : controller.downloadProgress,
            ),
            const Gap(AppSpacing.sm),
          ],
          if (installed == null)
            Button.primary(
              key: ValueKey('module-install-$id'),
              onPressed: controller.busy || !marketplace!.canInstall
                  ? null
                  : () => controller.install(marketplace),
              child: Text(
                active
                    ? _installationLabel()
                    : marketplace!.canInstall
                    ? 'Install'
                    : 'Unavailable',
              ),
            )
          else
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                Tooltip(
                  tooltip: (_) =>
                      const TooltipContainer(child: Text('Remove module')),
                  child: Semantics(
                    label: 'Remove module',
                    button: true,
                    child: Button.ghost(
                      key: ValueKey('module-remove-$id'),
                      style: const ButtonStyle.ghostIcon(),
                      onPressed: controller.busy || installed.restartRequired
                          ? null
                          : () => _remove(context, installed),
                      child: const Icon(LucideIcons.trash2),
                    ),
                  ),
                ),
                const Gap(AppSpacing.sm),
                Tooltip(
                  tooltip: (_) =>
                      const TooltipContainer(child: Text('Module settings')),
                  child: Semantics(
                    label: 'Module settings',
                    button: true,
                    child: Button.secondary(
                      key: ValueKey('module-settings-$id'),
                      style: const ButtonStyle.secondaryIcon(),
                      onPressed: controller.busy
                          ? null
                          : () => _settings(context, id),
                      child: const Icon(LucideIcons.settings),
                    ),
                  ),
                ),
              ],
            ),
        ],
      ),
    );
  }

  String _installationLabel() => controller.installing
      ? 'Installing…'
      : 'Downloading ${((controller.downloadProgress ?? 0) * 100).floor()}%';

  Future<void> _settings(
    BuildContext context,
    String id,
  ) => AppOverlays.showDialog<void>(
    context,
    builder: (dialogContext) => AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        final module = controller.installedModule(id);
        final update = module == null ? null : controller.updateFor(module);
        return AppOverlays.alertDialog(
          icon: LucideIcons.settings,
          title: Text('${module?.title ?? 'Module'} settings'),
          content: ConstrainedBox(
            constraints: BoxConstraints(
              maxHeight:
                  MediaQuery.sizeOf(context).height *
                  AppOverlaySize.dialogContentHeightFactor,
            ),
            child: SingleChildScrollView(
              child: module == null
                  ? const StateView.empty(title: 'Module removed')
                  : Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        if (module.error ?? controller.errorMessage
                            case final error?) ...[
                          StateView.error(
                            title: 'Module operation failed',
                            message: error,
                          ),
                          const Gap(AppSpacing.md),
                        ],
                        Switch(
                          key: ValueKey('module-enabled-$id'),
                          value: module.enabled,
                          leading: const Text('Enabled'),
                          onChanged:
                              controller.busy ||
                                  module.error != null ||
                                  module.restartRequired
                              ? null
                              : (value) => controller.setEnabled(id, value),
                        ),
                        if (module.events.contains(
                          ModuleEventKind.smsReceived,
                        )) ...[
                          const Gap(AppSpacing.md),
                          Button.secondary(
                            onPressed: controller.busy
                                ? null
                                : () => _smsSetup(context),
                            child: const Text('Set up SMS access'),
                          ),
                        ],
                        if (module.preferenceFields.isNotEmpty) ...[
                          const Gap(AppSpacing.md),
                          Button.secondary(
                            onPressed: controller.busy || module.error != null
                                ? null
                                : () => _preferences(context, module),
                            child: const Text('Preferences'),
                          ),
                        ],
                        for (final command in controller.settingsCommands(
                          module,
                        )) ...[
                          const Gap(AppSpacing.md),
                          Button.secondary(
                            onPressed:
                                controller.busy ||
                                    !module.enabled ||
                                    module.error != null
                                ? null
                                : () => _invoke(context, module, command),
                            child: Text(command.title),
                          ),
                        ],
                        if (update != null) ...[
                          const Gap(AppSpacing.md),
                          Button.secondary(
                            onPressed: controller.busy || module.restartRequired
                                ? null
                                : () => controller.install(update),
                            child: Text(
                              controller.activeModuleId == id
                                  ? _installationLabel()
                                  : 'Update to ${update.version}',
                            ),
                          ),
                        ],
                        if (module.restartRequired &&
                            controller.canRestart) ...[
                          const Gap(AppSpacing.md),
                          Button.secondary(
                            onPressed: controller.busy
                                ? null
                                : controller.restartApplication,
                            child: const Text('Restart CopyPaste'),
                          ),
                        ],
                      ],
                    ),
            ),
          ),
          actions: [
            Button.primary(
              onPressed: () => Navigator.pop(dialogContext),
              child: const Text('Done'),
            ),
          ],
        );
      },
    ),
  );

  Future<void> _preferences(
    BuildContext context,
    InstalledModule module,
  ) async {
    final draft = await _fieldsDialog(
      context,
      controller: controller,
      title: '${module.title} preferences',
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

  Future<void> _smsSetup(BuildContext context) => AppOverlays.showDialog<void>(
    context,
    builder: (dialogContext) => AnimatedBuilder(
      animation: controller,
      builder: (_, _) => AppOverlays.alertDialog(
        icon: LucideIcons.messageSquare,
        title: const Text('SMS access'),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                controller.smsAccess?.granted == true
                    ? 'Access is ready. Enable SMS Codes to start copying new codes.'
                    : 'Apply access with Shizuku, or run these ADB commands once. Then refresh Installed modules.',
              ),
              if (controller.smsAccess?.granted != true) ...[
                const Gap(AppSpacing.md),
                SelectableText(controller.smsAccess?.adbCommands ?? ''),
              ],
              if (controller.errorMessage case final error?) ...[
                const Gap(AppSpacing.md),
                StateView.error(title: 'SMS setup failed', message: error),
              ],
            ],
          ),
        ),
        actions: [
          if (controller.smsAccess?.granted != true)
            Button.primary(
              onPressed: controller.busy ? null : controller.configureSmsAccess,
              child: const Text('Apply with Shizuku'),
            ),
          Button.ghost(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Done'),
          ),
        ],
      ),
    ),
  );

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
