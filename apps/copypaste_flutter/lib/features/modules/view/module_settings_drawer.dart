import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/format_bytes.dart';
import '../../../shared/state_view.dart';
import '../../../shared/forms/dynamic_form_draft.dart';
import '../../../shared/forms/dynamic_form_fields.dart';
import '../../../platform/clipboard/clipboard_writer.dart';
import '../../applications/repository/application_catalog.dart';
import '../controller/module_view_controller.dart';
import '../controller/modules_controller.dart';
import '../models/module_models.dart';
import '../models/module_view.dart';
import 'module_command_dialogs.dart';
import 'module_picker_fields.dart';
import 'module_view_sections.dart';

/// Shared module management layout for desktop and mobile drawers.
class ModuleSettingsDrawer extends StatefulWidget {
  const ModuleSettingsDrawer({
    super.key,
    required this.controller,
    required this.moduleId,
    required this.onSmsSetup,
    this.applications = const RuntimeApplicationCatalog(),
    this.clipboard = const SystemClipboardWriter(),
  });

  final ModulesController controller;
  final String moduleId;
  final VoidCallback onSmsSetup;
  final ApplicationCatalog applications;
  final ClipboardWriter clipboard;

  @override
  State<ModuleSettingsDrawer> createState() => _ModuleSettingsDrawerState();
}

class _ModuleSettingsDrawerState extends State<ModuleSettingsDrawer> {
  DynamicFormDraft? _draft;
  late ModuleViewController _views = _createViews();
  late ModulePickerFields _pickers = _createPickers();

  ModulesController get controller => widget.controller;
  String get moduleId => widget.moduleId;

  @override
  void initState() {
    super.initState();
    _createDraft();
  }

  ModuleViewController _createViews() =>
      ModuleViewController(modules: controller, moduleId: moduleId);

  ModulePickerFields _createPickers() => ModulePickerFields(
    controller: controller,
    applications: widget.applications,
  );

  void _createDraft() {
    final module = controller.installedModule(moduleId);
    if (module != null && module.preferenceFields.isNotEmpty) {
      _draft = controller.form(
        module.preferenceFields,
        module.preferences,
        module: module,
      );
    }
  }

  @override
  void didUpdateWidget(ModuleSettingsDrawer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != controller || oldWidget.moduleId != moduleId) {
      unawaited(_draft?.close());
      _draft = null;
      _createDraft();
      _views.dispose();
      _views = _createViews();
    }
    if (oldWidget.controller != controller ||
        oldWidget.applications != widget.applications) {
      _pickers = _createPickers();
    }
  }

  Future<void> _invoke(InstalledModule module, ModuleCommand command) async {
    final arguments = controller.form(command.arguments, const {});
    final draft = command.arguments.isEmpty
        ? arguments
        : await showModuleArgumentsDialog(
            context,
            draft: arguments,
            title: command.title,
            action: context.l10n.modulesRun,
            fieldBuilder: (context, field) => _pickers.build(arguments, field),
          );
    if (draft == null) return;
    ModuleResult? result;
    try {
      result = await _views.run(command.id, draft.values);
    } finally {
      await draft.close();
    }
    if (result == null || !mounted) return;
    await showModuleResultDialog(
      context,
      title: command.title,
      text: result.text,
    );
  }

  Future<void> _act(ModuleViewAction action) async {
    ModuleResult? result;
    Future<bool> run() async {
      result = await _views.run(action.command, action.arguments);
      return controller.errorMessage == null;
    }

    if (action.destructive) {
      final done = await AppOverlays.showActionDialog(
        context,
        icon: LucideIcons.puzzle,
        title: Text(action.title),
        actionLabel: action.title,
        changes: controller,
        canConfirm: () => !controller.busy,
        onConfirm: run,
        failureMessage: () =>
            controller.errorMessage ??
            AppStrings.current.modulesOperationIncomplete,
      );
      if (done != true) return;
    } else {
      await run();
    }
    if (result case final result? when mounted) {
      await showModuleResultDialog(
        context,
        title: action.title,
        text: result.text,
      );
    }
  }

  Future<void> _savePreferences() async {
    final draft = _draft;
    if (draft == null || !draft.valid || draft.busy || controller.busy) return;
    await controller.setPreferences(moduleId, draft.values);
    if (!mounted || controller.errorMessage != null) return;
    final module = controller.installedModule(moduleId);
    if (module == null) return;
    setState(() {
      _draft = controller.form(
        module.preferenceFields,
        draft.values,
        module: module,
      );
    });
    await draft.close();
  }

  @override
  void dispose() {
    unawaited(_draft?.close());
    _views.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {
      const SingleActivator(LogicalKeyboardKey.escape): () =>
          unawaited(closeDrawer(context)),
    },
    child: Focus(
      autofocus: true,
      skipTraversal: true,
      child: AnimatedBuilder(
        animation: Listenable.merge([controller, _draft, _views]),
        builder: (context, _) {
          final module = controller.installedModule(moduleId);
          return ConstrainedBox(
            key: ValueKey('module-settings-drawer-$moduleId'),
            constraints: AppOverlays.drawerContentConstraints(context),
            child: Scaffold(
              headers: [
                AppBar(
                  title: Text(
                    module == null
                        ? context.l10n.modulesModuleSettings
                        : context.l10n.modulesSettingsTitle(module.title),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  leading: const [
                    AppIconFrame(child: Icon(LucideIcons.settings)),
                  ],
                  trailing: [
                    Tooltip(
                      tooltip: (context) => TooltipContainer(
                        child: Text(context.l10n.modulesCloseSettings),
                      ),
                      child: Semantics(
                        label: context.l10n.modulesCloseSettings,
                        button: true,
                        child: Button.ghost(
                          key: ValueKey('module-settings-close-$moduleId'),
                          style: AppTheme.controlButtonStyle(
                            const ButtonStyle.ghostIcon(),
                          ),
                          onPressed: () => closeDrawer(context),
                          child: const AppIconFrame(child: Icon(LucideIcons.x)),
                        ),
                      ),
                    ),
                  ],
                ),
                const Divider(),
              ],
              footers: [
                const Divider(),
                Padding(
                  padding: const EdgeInsets.all(AppSpacing.lg),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      if ((module, _draft) case (
                        final module?,
                        final draft?,
                      )) ...[
                        Button.primary(
                          style: AppTheme.actionButtonStyle(
                            const ButtonStyle.primary(),
                          ),
                          key: ValueKey('module-preferences-save-$moduleId'),
                          alignment: AppTheme.moduleSettingsActionAlignment,
                          onPressed:
                              controller.busy ||
                                  module.error != null ||
                                  draft.busy ||
                                  !draft.valid
                              ? null
                              : _savePreferences,
                          child: Text(
                            context.l10n.commonSave,
                            textAlign: TextAlign.center,
                          ),
                        ),
                        const Gap(AppSpacing.md),
                      ],
                      Button.primary(
                        style: AppTheme.actionButtonStyle(
                          const ButtonStyle.primary(),
                        ),
                        key: ValueKey('module-settings-done-$moduleId'),
                        alignment: AppTheme.moduleSettingsActionAlignment,
                        onPressed: () => closeDrawer(context),
                        child: Text(
                          context.l10n.commonDone,
                          textAlign: TextAlign.center,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: module == null
                    ? StateView.empty(title: context.l10n.modulesRemoved)
                    : Column(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          if (_draft == null &&
                              controller.settingsCommands(module).isEmpty &&
                              module.error == null &&
                              controller.errorMessage == null &&
                              !controller.busy &&
                              !(controller.configuringModel &&
                                  controller.activeModuleId == moduleId) &&
                              !(controller.canSetUpSmsAccess &&
                                  module.events.contains(
                                    ModuleEventKind.smsReceived,
                                  )) &&
                              !(module.restartRequired &&
                                  controller.canRestart))
                            StateView.empty(
                              title: context.l10n.modulesNoSettingsTitle,
                              message: context.l10n.modulesNoSettingsMessage,
                            ),
                          if (module.error ?? controller.errorMessage
                              case final error?) ...[
                            StateView.error(
                              title: context.l10n.modulesOperationFailed,
                              message: error,
                            ),
                            const Gap(AppSpacing.md),
                          ],
                          if (controller.configuringModel &&
                              controller.activeModuleId == moduleId) ...[
                            StateView.loading(
                              message: context.l10n.modulesDownloadingModel,
                            ),
                            const Gap(AppSpacing.md),
                          ],
                          if (controller.canSetUpSmsAccess &&
                              module.events.contains(
                                ModuleEventKind.smsReceived,
                              )) ...[
                            const Gap(AppSpacing.md),
                            Button.secondary(
                              style: AppTheme.actionButtonStyle(
                                const ButtonStyle.secondary(),
                              ),
                              alignment: AppTheme.moduleSettingsActionAlignment,
                              leading: const Center(
                                child: AppIconFrame(
                                  child: Icon(LucideIcons.messageSquare),
                                ),
                              ),
                              onPressed: controller.busy
                                  ? null
                                  : widget.onSmsSetup,
                              child: Text(
                                context.l10n.modulesSetUpSmsAccess,
                                textAlign: TextAlign.center,
                              ),
                            ),
                          ],
                          if (_draft case final draft?) ...[
                            const Gap(AppSpacing.md),
                            DynamicFormFields(
                              draft: draft,
                              enabled: !controller.busy && module.error == null,
                              fieldBuilder: (context, field) => _pickers.build(
                                draft,
                                field,
                                enabled:
                                    !controller.busy && module.error == null,
                              ),
                              footer: (context) => _searchModelNote(
                                context,
                                module.searchModelFor(draft.values),
                              ),
                            ),
                          ],
                          if (module.restartRequired &&
                              controller.canRestart) ...[
                            const Gap(AppSpacing.md),
                            Button.secondary(
                              style: AppTheme.actionButtonStyle(
                                const ButtonStyle.secondary(),
                              ),
                              alignment: AppTheme.moduleSettingsActionAlignment,
                              leading: const Center(
                                child: AppIconFrame(
                                  child: Icon(LucideIcons.rotateCw),
                                ),
                              ),
                              onPressed: controller.busy
                                  ? null
                                  : controller.restartApplication,
                              child: Text(
                                context.l10n.modulesRestartApp,
                                textAlign: TextAlign.center,
                              ),
                            ),
                          ],
                          for (final command in controller.settingsCommands(
                            module,
                          )) ...[
                            const Gap(AppSpacing.md),
                            Button.secondary(
                              style: AppTheme.actionButtonStyle(
                                const ButtonStyle.secondary(),
                              ),
                              alignment: AppTheme.moduleSettingsActionAlignment,
                              leading: const Center(
                                child: AppIconFrame(
                                  child: Icon(LucideIcons.play),
                                ),
                              ),
                              onPressed:
                                  controller.busy ||
                                      !module.enabled ||
                                      module.error != null
                                  ? null
                                  : () => _invoke(module, command),
                              child: Text(
                                command.title,
                                textAlign: TextAlign.center,
                              ),
                            ),
                          ],
                          if (_views.view case final view?) ...[
                            const Gap(AppSpacing.lg),
                            ModuleViewSections(
                              key: ValueKey('module-view-$moduleId'),
                              view: view,
                              enabled:
                                  !controller.busy &&
                                  module.enabled &&
                                  module.error == null,
                              onAction: (action) => unawaited(_act(action)),
                              onCopy: widget.clipboard.writeText,
                            ),
                          ],
                        ],
                      ),
              ),
            ),
          );
        },
      ),
    ),
  );
}

Widget? _searchModelNote(BuildContext context, ModuleSearchModel? model) =>
    model == null
    ? null
    : Text(
        model.available
            ? context.l10n.modulesModelReady(model.title)
            : context.l10n.modulesModelDownload(
                model.title,
                formatSizeNumber(model.sizeBytes / (1024 * 1024)),
              ),
      ).small().muted();
