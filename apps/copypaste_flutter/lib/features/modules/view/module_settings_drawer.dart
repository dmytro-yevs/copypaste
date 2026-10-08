import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';
import '../controller/modules_controller.dart';
import '../models/module_models.dart';

/// Shared module management layout for desktop and mobile drawers.
class ModuleSettingsDrawer extends StatelessWidget {
  const ModuleSettingsDrawer({
    super.key,
    required this.controller,
    required this.moduleId,
    required this.onPreferences,
    required this.onInvoke,
    required this.onSmsSetup,
  });

  final ModulesController controller;
  final String moduleId;
  final ValueChanged<InstalledModule> onPreferences;
  final void Function(InstalledModule, ModuleCommand) onInvoke;
  final VoidCallback onSmsSetup;

  @override
  Widget build(BuildContext context) => CallbackShortcuts(
    bindings: {
      const SingleActivator(LogicalKeyboardKey.escape): () =>
          unawaited(closeDrawer(context)),
    },
    child: Focus(
      autofocus: true,
      child: AnimatedBuilder(
        animation: controller,
        builder: (context, _) {
          final module = controller.installedModule(moduleId);
          final update = module == null ? null : controller.updateFor(module);
          return ConstrainedBox(
            key: ValueKey('module-settings-drawer-$moduleId'),
            constraints: AppOverlays.drawerContentConstraints(context),
            child: Scaffold(
              headers: [
                AppBar(
                  title: Text('${module?.title ?? 'Module'} settings'),
                  leading: const [Icon(LucideIcons.settings)],
                  trailing: [
                    Tooltip(
                      tooltip: (context) => const TooltipContainer(
                        child: Text('Close module settings'),
                      ),
                      child: Semantics(
                        label: 'Close module settings',
                        button: true,
                        child: Button.ghost(
                          key: ValueKey('module-settings-close-$moduleId'),
                          style: const ButtonStyle.ghostIcon(),
                          onPressed: () => closeDrawer(context),
                          child: const Icon(LucideIcons.x),
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
                  child: Button.primary(
                    key: ValueKey('module-settings-done-$moduleId'),
                    alignment: AppTheme.moduleSettingsActionAlignment,
                    onPressed: () => closeDrawer(context),
                    child: const Text('Done', textAlign: TextAlign.center),
                  ),
                ),
              ],
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(AppSpacing.lg),
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
                          if (controller.configuringModel &&
                              controller.activeModuleId == moduleId) ...[
                            const StateView.loading(
                              message: 'Downloading language model…',
                            ),
                            const Gap(AppSpacing.md),
                          ],
                          Card(
                            child: Basic(
                              theme: AppTheme.moduleSettingsRowTheme,
                              leading: const Icon(
                                LucideIcons.power,
                                size: AppIconSize.md,
                              ),
                              title: const Text('Enabled'),
                              trailing: Semantics(
                                label: 'Enabled',
                                child: Switch(
                                  key: ValueKey('module-enabled-$moduleId'),
                                  value: module.enabled,
                                  onChanged:
                                      controller.busy ||
                                          module.error != null ||
                                          module.restartRequired
                                      ? null
                                      : (value) => controller.setEnabled(
                                          moduleId,
                                          value,
                                        ),
                                ),
                              ),
                            ),
                          ),
                          if (controller.canSetUpSmsAccess &&
                              module.events.contains(
                                ModuleEventKind.smsReceived,
                              )) ...[
                            const Gap(AppSpacing.md),
                            Button.secondary(
                              alignment: AppTheme.moduleSettingsActionAlignment,
                              leading: const Center(
                                child: Icon(LucideIcons.messageSquare),
                              ),
                              onPressed: controller.busy ? null : onSmsSetup,
                              child: const Text(
                                'Set up SMS access',
                                textAlign: TextAlign.center,
                              ),
                            ),
                          ],
                          if (module.preferenceFields.isNotEmpty) ...[
                            const Gap(AppSpacing.md),
                            Button.secondary(
                              alignment: AppTheme.moduleSettingsActionAlignment,
                              leading: const Center(
                                child: Icon(LucideIcons.slidersHorizontal),
                              ),
                              onPressed: controller.busy || module.error != null
                                  ? null
                                  : () => onPreferences(module),
                              child: const Text(
                                'Preferences',
                                textAlign: TextAlign.center,
                              ),
                            ),
                          ],
                          if (update != null) ...[
                            const Gap(AppSpacing.md),
                            Button.secondary(
                              alignment: AppTheme.moduleSettingsActionAlignment,
                              leading: const Center(
                                child: Icon(LucideIcons.download),
                              ),
                              onPressed:
                                  controller.busy || module.restartRequired
                                  ? null
                                  : () => controller.install(update),
                              child: Text(
                                controller.activeModuleId == moduleId
                                    ? controller.installing
                                          ? 'Installing…'
                                          : 'Downloading ${((controller.downloadProgress ?? 0) * 100).floor()}%'
                                    : 'Update to ${update.version}',
                                textAlign: TextAlign.center,
                              ),
                            ),
                          ],
                          if (module.restartRequired &&
                              controller.canRestart) ...[
                            const Gap(AppSpacing.md),
                            Button.secondary(
                              alignment: AppTheme.moduleSettingsActionAlignment,
                              leading: const Center(
                                child: Icon(LucideIcons.rotateCw),
                              ),
                              onPressed: controller.busy
                                  ? null
                                  : controller.restartApplication,
                              child: const Text(
                                'Restart CopyPaste',
                                textAlign: TextAlign.center,
                              ),
                            ),
                          ],
                          for (final command in controller.settingsCommands(
                            module,
                          )) ...[
                            const Gap(AppSpacing.md),
                            Button.secondary(
                              alignment: AppTheme.moduleSettingsActionAlignment,
                              leading: const Center(
                                child: Icon(LucideIcons.play),
                              ),
                              onPressed:
                                  controller.busy ||
                                      !module.enabled ||
                                      module.error != null
                                  ? null
                                  : () => onInvoke(module, command),
                              child: Text(
                                command.title,
                                textAlign: TextAlign.center,
                              ),
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
