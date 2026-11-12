import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import '../../../shared/format_bytes.dart';

import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../../shared/equal_height_grid.dart';
import '../../../shared/step_progress.dart';
import '../controller/modules_controller.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import '../../../shared/forms/dynamic_form_fields.dart';
import '../models/module_models.dart';
import '../models/module_marketplace_models.dart';
import '../models/module_operation.dart';
import 'sms_access_setup_drawer.dart';
import 'module_settings_drawer.dart';

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
                children: [
                  TabItem(child: Text(context.l10n.modulesMarketplaceTab)),
                  TabItem(child: Text(context.l10n.modulesInstalledTab)),
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
              leading: const AppIconFrame(
                size: AppIconSize.sm,
                child: Icon(LucideIcons.refreshCw, size: AppIconSize.sm),
              ),
              child: Text(context.l10n.modulesRefresh),
            ),
          ],
        ),
        const Gap(AppSpacing.lg),
        TextField(
          initialValue: controller.query,
          placeholder: Text(context.l10n.modulesSearchPlaceholder),
          onChanged: controller.search,
          skipInputFeatureFocusTraversal: true,
          features: const [
            InputFeature.leading(AppIconFrame(child: Icon(LucideIcons.search))),
            InputFeature.clear(),
          ],
        ),
        const Gap(AppSpacing.lg),
        if (controller.errorMessage != null &&
            controller.state == ModulesLoadState.ready) ...[
          StateView.error(
            title: context.l10n.modulesOperationFailed,
            message: controller.errorMessage!,
          ),
          const Gap(AppSpacing.lg),
        ],
        switch (controller.state) {
          ModulesLoadState.loading => StateView.loading(
            message: context.l10n.modulesLoading,
          ),
          ModulesLoadState.error => StateView.error(
            title: context.l10n.modulesUnavailableTitle,
            message:
                controller.errorMessage ??
                context.l10n.modulesUnavailableMessage,
            actionLabel: context.l10n.commonTryAgain,
            onAction: controller.initialize,
          ),
          ModulesLoadState.ready =>
            controller.section == ModulesSection.marketplace
                ? _marketplace(context)
                : controller.filteredModules.isEmpty
                ? StateView.empty(
                    title: controller.modules.isEmpty
                        ? context.l10n.modulesNoneInstalled
                        : context.l10n.modulesNoMatches,
                    message: controller.modules.isEmpty
                        ? context.l10n.modulesChooseFromMarketplace
                        : context.l10n.modulesTryDifferentSearch,
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
        ModulesLoadState.loading => StateView.loading(
          message: context.l10n.modulesMarketplaceLoading,
        ),
        ModulesLoadState.error => StateView.error(
          title: context.l10n.modulesMarketplaceUnavailableTitle,
          message:
              controller.catalogError ??
              context.l10n.modulesMarketplaceUnavailableMessage,
          actionLabel: context.l10n.commonTryAgain,
          onAction: controller.loadMarketplace,
        ),
        ModulesLoadState.ready =>
          controller.catalog.isEmpty
              ? StateView.empty(
                  title: controller.query.trim().isEmpty
                      ? context.l10n.modulesNoneAvailable
                      : context.l10n.modulesNoMatches,
                  message: controller.query.trim().isEmpty
                      ? context.l10n.modulesNoneAvailableMessage
                      : context.l10n.modulesTryDifferentSearch,
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

  Widget _cards(List<Widget> cards) => EqualHeightGrid(
    minChildWidth: AppLayoutSize.marketplaceCardMinWidth,
    spacing: AppSpacing.md,
    children: cards,
  );

  Widget _moduleCard(
    BuildContext context, {
    MarketplaceModule? marketplace,
    InstalledModule? installed,
  }) {
    final id = installed?.id ?? marketplace!.id;
    final update = installed == null ? null : controller.updateFor(installed);
    final active = controller.activeModuleId == id;
    final platforms =
        installed != null && installed.supportedPlatforms.isNotEmpty
        ? installed.supportedPlatforms
        : marketplace?.supportedPlatforms ?? const <ModulePlatform>[];
    final size = installed == null
        ? marketplace!.downloadSize
        : formatBytes(installed.sizeBytes);
    final toggleable =
        installed != null &&
        !controller.busy &&
        installed.error == null &&
        !installed.restartRequired;
    return Card(
      key: ValueKey('module-$id'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  const AppIconFrame(
                    size: AppIconSize.xl,
                    child: Icon(LucideIcons.puzzle, size: AppIconSize.xl),
                  ),
                  if (installed != null)
                    Flexible(
                      child: Semantics(
                        label: context.l10n.modulesEnableModule(
                          installed.title,
                        ),
                        child: AppSwitchFrame(
                          child: Switch(
                            key: ValueKey('module-enabled-$id'),
                            value: installed.enabled,
                            enabled: toggleable,
                            onChanged: toggleable
                                ? (value) => controller.setEnabled(id, value)
                                : null,
                          ),
                        ),
                      ),
                    ),
                ],
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
              const Gap(AppSpacing.sm),
              Text(formatModulePlatforms(platforms)).small().muted(),
              if (marketplace?.appRequirement case final requirement?) ...[
                const Gap(AppSpacing.sm),
                Text(requirement).small().muted(),
              ],
              if (marketplace?.systemRequirement case final requirement?) ...[
                const Gap(AppSpacing.sm),
                Text(requirement).small().muted(),
              ],
              if (installed?.error case final error?) ...[
                const Gap(AppSpacing.sm),
                Text(error).small().muted(),
              ] else if (installed == null &&
                  marketplace?.availabilityNotice != null) ...[
                const Gap(AppSpacing.sm),
                Text(marketplace!.availabilityNotice!).small().muted(),
              ],
            ],
          ),
          if (installed != null || marketplace!.canInstall)
            Padding(
              padding: const EdgeInsets.only(top: AppSpacing.lg),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (controller.operationFor(id) case final operation?) ...[
                    _operationProgress(
                      context,
                      operation,
                      installed?.title ?? marketplace!.title,
                    ),
                    const Gap(AppSpacing.md),
                  ] else if (active) ...[
                    const LinearProgressIndicator(),
                    const Gap(AppSpacing.sm),
                  ],
                  if (installed == null)
                    Button.primary(
                      key: ValueKey('module-install-$id'),
                      onPressed: controller.busy || !marketplace!.canInstall
                          ? null
                          : () => controller.install(marketplace),
                      child: Text(context.l10n.modulesInstall),
                    )
                  else
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        if (update != null) ...[
                          Tooltip(
                            tooltip: (_) => TooltipContainer(
                              child: Text(
                                context.l10n.modulesUpdateTo(
                                  update.version.toString(),
                                ),
                              ),
                            ),
                            child: Semantics(
                              label: context.l10n.modulesUpdateTo(
                                update.version.toString(),
                              ),
                              button: true,
                              child: Button.secondary(
                                key: ValueKey('module-update-$id'),
                                style: AppTheme.controlButtonStyle(
                                  const ButtonStyle.secondaryIcon(),
                                ),
                                onPressed:
                                    controller.busy || installed.restartRequired
                                    ? null
                                    : () => controller.install(update),
                                child: const AppIconFrame(
                                  child: Icon(LucideIcons.download),
                                ),
                              ),
                            ),
                          ),
                          const Gap(AppSpacing.sm),
                        ],
                        Tooltip(
                          tooltip: (_) => TooltipContainer(
                            child: Text(context.l10n.modulesRemoveModule),
                          ),
                          child: Semantics(
                            label: context.l10n.modulesRemoveModule,
                            button: true,
                            child: Button.ghost(
                              key: ValueKey('module-remove-$id'),
                              style: AppTheme.controlButtonStyle(
                                const ButtonStyle.ghostIcon(),
                              ),
                              onPressed:
                                  controller.busy || installed.restartRequired
                                  ? null
                                  : () => _remove(context, installed),
                              child: const AppIconFrame(
                                child: Icon(LucideIcons.trash2),
                              ),
                            ),
                          ),
                        ),
                        const Gap(AppSpacing.sm),
                        Tooltip(
                          tooltip: (_) => TooltipContainer(
                            child: Text(context.l10n.modulesModuleSettings),
                          ),
                          child: Semantics(
                            label: context.l10n.modulesModuleSettings,
                            button: true,
                            child: Button.secondary(
                              key: ValueKey('module-settings-$id'),
                              style: AppTheme.controlButtonStyle(
                                const ButtonStyle.secondaryIcon(),
                              ),
                              onPressed: controller.busy
                                  ? null
                                  : () => _settings(context, id),
                              child: const AppIconFrame(
                                child: Icon(LucideIcons.settings),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                ],
              ),
            ),
        ],
      ),
    );
  }

  Widget _operationProgress(
    BuildContext context,
    ModuleOperation operation,
    String title,
  ) {
    final strings = context.l10n;
    return StepProgress(
      key: ValueKey('module-progress-${operation.moduleId}'),
      steps: [
        for (final phase in ModuleOperationPhase.values)
          switch (phase) {
            ModuleOperationPhase.download => strings.modulesPhaseDownload,
            ModuleOperationPhase.install => strings.modulesPhaseInstall,
          },
      ],
      current: operation.phase.index,
      failure: operation.failed ? strings.modulesOperationFailed : null,
      value: operation.progress,
      valueLabel: strings.transfersProgressPercent(operation.downloadPercent),
      detail: strings.transfersBytesProgress(
        formatBytes(operation.downloadedBytes),
        formatBytes(operation.totalBytes),
      ),
      semanticLabel: switch (operation.kind) {
        ModuleOperationKind.install => strings.modulesInstallingModule(title),
        ModuleOperationKind.update => strings.modulesUpdatingModule(title),
      },
    );
  }

  Future<void> _settings(BuildContext context, String id) => showOverlay<void>(
    context,
    AppOverlays.drawerConfiguration(context),
    builder: (context) => ModuleSettingsDrawer(
      controller: controller,
      moduleId: id,
      onInvoke: (module, command) => _invoke(context, module, command),
      onSmsSetup: () => _smsSetup(context),
    ),
  ).future;

  Future<void> _smsSetup(BuildContext context) async {
    final setup = controller.smsAccessSetup();
    try {
      await showOverlay<void>(
        context,
        AppOverlays.drawerConfiguration(context),
        builder: (_) => SmsAccessSetupDrawer(controller: setup),
      ).future;
    } finally {
      setup.dispose();
      await controller.refreshSmsAccess();
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
            action: context.l10n.modulesRun,
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
            style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
            onPressed: () => Navigator.pop(dialogContext),
            child: Text(dialogContext.l10n.commonDone),
          ),
        ],
      ),
    );
  }

  Future<void> _remove(BuildContext context, InstalledModule module) async {
    await AppOverlays.showActionDialog(
      context,
      icon: LucideIcons.trash2,
      title: Text(context.l10n.modulesRemoveTitle(module.title)),
      content: Text(context.l10n.modulesRemoveMessage),
      actionLabel: context.l10n.modulesRemove,
      changes: controller,
      canConfirm: () => !controller.busy,
      onConfirm: () => controller.remove(module.id),
      failureMessage: () =>
          controller.errorMessage ?? AppStrings.current.modulesRemoveFailed,
    );
  }
}

Future<DynamicFormDraft?> _fieldsDialog(
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
  final DynamicFormDraft draft;
  final String action;
  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: draft,
    builder: (context, _) => AppOverlays.alertDialog(
      icon: LucideIcons.puzzle,
      title: Text(title),
      content: SingleChildScrollView(
        clipBehavior: Clip.none,
        child: DynamicFormFields(draft: draft),
      ),
      actions: [
        Button.ghost(
          style: AppTheme.dialogButtonStyle(const ButtonStyle.ghost()),
          onPressed: () => Navigator.pop(context, false),
          child: Text(context.l10n.commonCancel),
        ),
        Button.primary(
          style: AppTheme.dialogButtonStyle(const ButtonStyle.primary()),
          onPressed: draft.valid && !draft.busy
              ? () => Navigator.pop(context, true)
              : null,
          child: Text(action),
        ),
      ],
    ),
  );
}
