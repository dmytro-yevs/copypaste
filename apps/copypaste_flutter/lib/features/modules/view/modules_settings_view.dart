import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import '../../../shared/format_bytes.dart';

import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_switch_frame.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../app/theme/app_tooltip.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../settings/view/settings_section.dart';
import '../controller/module_view_services.dart';
import '../controller/modules_controller.dart';
import '../models/module_models.dart';
import '../models/module_marketplace_models.dart';
import '../models/module_operation.dart';
import 'sms_access_setup_drawer.dart';
import 'module_details_drawer.dart';
import 'module_settings_drawer.dart';

/// Marketplace and installed modules share the host's components and state.
class ModulesSettingsView extends StatefulWidget {
  const ModulesSettingsView({
    super.key,
    required this.controller,
    this.services = const ModuleViewServices(),
  });
  final ModulesController controller;
  final ModuleViewServices services;

  @override
  State<ModulesSettingsView> createState() => _ModulesSettingsViewState();
}

class _ModulesSettingsViewState extends State<ModulesSettingsView> {
  ModulesController get controller => widget.controller;

  String? _reportedError;
  String? _reportedNotice;

  @override
  void initState() {
    super.initState();
    _reportedError = controller.errorMessage;
    _reportedNotice = controller.monitoringNotice;
    controller.addListener(_reportError);
    // Loading notifies listeners, which must not happen while this view builds.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(controller.ensureMarketplace());
    });
  }

  @override
  void didUpdateWidget(ModulesSettingsView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != controller) {
      oldWidget.controller.removeListener(_reportError);
      controller.addListener(_reportError);
      unawaited(controller.ensureMarketplace());
    }
  }

  @override
  void dispose() {
    controller.removeListener(_reportError);
    super.dispose();
  }

  /// A failed module operation is reported once, as a toast; so is a
  /// monitoring problem found after one succeeded.
  void _reportError() {
    final notice = controller.monitoringNotice;
    if (notice != _reportedNotice) {
      _reportedNotice = notice;
      if (notice != null && mounted) {
        AppToast.show(
          context,
          title: context.l10n.modulesSmsAccessTitle,
          message: notice,
          tone: AppToastTone.information,
        );
      }
    }
    final message = controller.errorMessage;
    if (message == _reportedError) return;
    _reportedError = message;
    if (message == null ||
        controller.state != ModulesLoadState.ready ||
        !mounted) {
      return;
    }
    AppToast.show(
      context,
      title: context.l10n.modulesOperationFailed,
      message: controller.errorReason,
      tone: AppToastTone.error,
    );
  }

  /// What a dialog reports when its operation fails: the text [_reportError]
  /// shows for it, so the two toasts appear as one, or [fallback] when the
  /// operation did not run.
  String _dialogFailure(String fallback) => controller.errorMessage == null
      ? fallback
      : controller.errorReason ?? AppStrings.current.modulesOperationFailed;

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
                  for (final label in [
                    context.l10n.modulesMarketplaceTab,
                    context.l10n.modulesInstalledTab,
                  ])
                    TabItem(
                      child: Text(
                        label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                ],
              ),
            ),
            const Gap(AppSpacing.sm),
            _iconButton(
              key: const ValueKey('modules-refresh'),
              label: context.l10n.modulesRefresh,
              icon: LucideIcons.refreshCw,
              style: const ButtonStyle.ghostIcon(),
              onPressed:
                  controller.busy ||
                      controller.catalogState == ModulesLoadState.loading
                  ? null
                  : controller.loadMarketplace,
            ),
            if (controller.canInstallFromFile) ...[
              const Gap(AppSpacing.sm),
              _iconButton(
                key: const ValueKey('modules-install-from-file'),
                label: context.l10n.modulesInstallFromFile,
                icon: LucideIcons.fileUp,
                style: const ButtonStyle.ghostIcon(),
                onPressed: controller.busy ? null : controller.installFromFile,
              ),
            ],
          ],
        ),
        const Gap(AppSpacing.lg),
        TextField(
          initialValue: controller.query,
          decoration: AppTheme.searchFieldDecoration(context),
          placeholder: Text(context.l10n.modulesSearchPlaceholder),
          onChanged: controller.search,
          skipInputFeatureFocusTraversal: true,
          features: const [
            InputFeature.leading(AppIconFrame(child: Icon(LucideIcons.search))),
            InputFeature.clear(),
          ],
        ),
        const Gap(AppSpacing.lg),
        switch (controller.state) {
          ModulesLoadState.loading => const StateView.loading(),
          ModulesLoadState.error => StateView.error(
            title: context.l10n.modulesUnavailableTitle,
            message: controller.errorMessage,
            onRetry: controller.initialize,
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
        ModulesLoadState.loading => const StateView.loading(),
        ModulesLoadState.error => StateView.error(
          title: context.l10n.modulesMarketplaceUnavailableTitle,
          message: controller.catalogError,
          onRetry: controller.loadMarketplace,
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

  Widget _cards(List<Widget> rows) => SettingsGroup(children: rows);

  /// An icon-only action; its label is the tooltip and the accessible name.
  Widget _iconButton({
    required Key key,
    required String label,
    required IconData icon,
    required ButtonStyle style,
    required VoidCallback? onPressed,
  }) => AppTooltip(
    tooltip: (_) => TooltipContainer(child: Text(label)),
    child: Semantics(
      label: label,
      button: true,
      child: Button(
        key: key,
        style: AppTheme.controlButtonStyle(style),
        onPressed: onPressed,
        child: AppIconFrame(child: Icon(icon)),
      ),
    ),
  );

  /// One compact table row: what the module is, then its actions; Details
  /// explains it in full.
  Widget _moduleCard(
    BuildContext context, {
    MarketplaceModule? marketplace,
    InstalledModule? installed,
  }) {
    final id = installed?.id ?? marketplace!.id;
    final title = installed?.title ?? marketplace!.title;
    final update = installed == null ? null : controller.updateFor(installed);
    final active = controller.activeModuleId == id;
    final operation = controller.operationFor(id);
    final running = operation?.running ?? false;
    final size = installed == null
        ? marketplace!.downloadSize
        : formatBytes(installed.sizeBytes);
    final notice =
        installed?.error ??
        (installed == null ? marketplace?.availabilityNotice : null);
    final toggleable =
        installed != null &&
        !controller.busy &&
        installed.error == null &&
        !installed.restartRequired;
    Widget iconButton({
      required Key key,
      required String label,
      required IconData icon,
      required VoidCallback? onPressed,
      bool ghost = false,
    }) => _iconButton(
      key: key,
      label: label,
      icon: icon,
      style: ghost
          ? const ButtonStyle.ghostIcon()
          : const ButtonStyle.secondaryIcon(),
      onPressed: onPressed,
    );
    final info = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(title, maxLines: 1, overflow: TextOverflow.ellipsis).semiBold(),
        Text(
          installed?.description ?? marketplace!.description,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ).small().muted(),
        Text(
          [
            installed?.version ?? marketplace!.version.toString(),
            ?size,
            ?notice,
          ].join(' · '),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ).xSmall().muted(),
        if (active && !running) ...[
          const Gap(AppSpacing.xs),
          const LinearProgressIndicator(),
        ],
      ],
    );
    final details = iconButton(
      key: ValueKey('module-details-$id'),
      label: context.l10n.modulesDetails,
      icon: LucideIcons.info,
      ghost: true,
      onPressed: () => _details(context, installed, marketplace),
    );
    final Widget? primary = running
        ? _operationButton(context, operation!, title)
        : installed == null && marketplace!.canInstall
        ? _iconButton(
            key: ValueKey('module-install-$id'),
            label: context.l10n.modulesInstall,
            icon: LucideIcons.download,
            style: const ButtonStyle.primaryIcon(),
            onPressed: controller.busy
                ? null
                : () => controller.install(marketplace),
          )
        : null;
    final managed = <Widget>[
      if (!running && installed != null) ...[
        if (update != null)
          iconButton(
            key: ValueKey('module-update-$id'),
            label: context.l10n.modulesUpdateTo(update.version.toString()),
            icon: LucideIcons.download,
            onPressed: controller.busy || installed.restartRequired
                ? null
                : () => controller.install(update),
          ),
        iconButton(
          key: ValueKey('module-settings-$id'),
          label: context.l10n.modulesModuleSettings,
          icon: LucideIcons.settings,
          onPressed: controller.busy ? null : () => _settings(context, id),
        ),
        iconButton(
          key: ValueKey('module-remove-$id'),
          label: context.l10n.modulesRemoveModule,
          icon: LucideIcons.trash2,
          ghost: true,
          onPressed: controller.busy || installed.restartRequired
              ? null
              : () => _remove(context, installed),
        ),
        Semantics(
          label: context.l10n.modulesEnableModule(installed.title),
          child: AppSwitchFrame(
            child: Switch(
              key: ValueKey('module-enabled-$id'),
              value: installed.enabled,
              enabled: toggleable,
              onChanged: toggleable
                  ? (value) => _setEnabled(context, installed, value)
                  : null,
            ),
          ),
        ),
      ],
    ];
    final actions = Row(
      mainAxisSize: MainAxisSize.min,
      spacing: AppSpacing.sm,
      children: [details, ?primary, ...managed],
    );
    // A narrow row lets a running operation take the free width and wraps
    // icon actions instead of overflowing the row.
    final narrowActions = running
        ? Row(
            spacing: AppSpacing.sm,
            children: [
              details,
              Expanded(child: primary!),
            ],
          )
        : Wrap(
            alignment: WrapAlignment.end,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: AppSpacing.sm,
            runSpacing: AppSpacing.sm,
            children: [details, ?primary, ...managed],
          );
    const icon = AppIconFrame(
      size: AppIconSize.lg,
      child: Icon(LucideIcons.puzzle, size: AppIconSize.lg),
    );
    return Padding(
      key: ValueKey('module-$id'),
      padding: AppTheme.settingsRowPadding,
      child: LayoutBuilder(
        builder: (context, constraints) =>
            constraints.maxWidth >= AppLayoutSize.marketplaceCardMinWidth * 1.5
            ? Row(
                children: [
                  icon,
                  const Gap(AppSpacing.md),
                  Expanded(child: info),
                  const Gap(AppSpacing.md),
                  actions,
                ],
              )
            : Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Row(
                    children: [
                      icon,
                      const Gap(AppSpacing.md),
                      Expanded(child: info),
                    ],
                  ),
                  const Gap(AppSpacing.sm),
                  narrowActions,
                ],
              ),
      ),
    );
  }

  Future<void> _details(
    BuildContext context,
    InstalledModule? installed,
    MarketplaceModule? marketplace,
  ) => showOverlay<void>(
    context,
    AppOverlays.drawerConfiguration(context),
    builder: (drawerContext) => ModuleDetailsDrawer(
      installed: installed,
      marketplace: marketplace,
      action: installed != null
          ? Button.primary(
              style: AppTheme.controlButtonStyle(const ButtonStyle.primary()),
              onPressed: () {
                closeDrawer(drawerContext);
                unawaited(_settings(context, installed.id));
              },
              child: Text(context.l10n.modulesModuleSettings),
            )
          : marketplace != null && marketplace.canInstall
          ? Button.primary(
              style: AppTheme.controlButtonStyle(const ButtonStyle.primary()),
              onPressed: () {
                closeDrawer(drawerContext);
                unawaited(controller.install(marketplace));
              },
              child: Text(context.l10n.modulesInstall),
            )
          : null,
    ),
  ).future;

  Widget _operationButton(
    BuildContext context,
    ModuleOperation operation,
    String title,
  ) {
    final strings = context.l10n;
    final downloading = operation.phase == ModuleOperationPhase.download;
    return Semantics(
      label: switch (operation.kind) {
        ModuleOperationKind.install => strings.modulesInstallingModule(title),
        ModuleOperationKind.update => strings.modulesUpdatingModule(title),
      },
      value: downloading
          ? strings.transfersProgressPercent(operation.downloadPercent)
          : null,
      child: Button.secondary(
        key: ValueKey('module-progress-${operation.moduleId}'),
        onPressed: null,
        leading: AppIconFrame(
          size: AppIconSize.sm,
          // A step smaller than its frame, so the ring keeps clear of the label.
          child: CircularProgressIndicator(
            value: downloading ? operation.progress : null,
            size: AppIconSize.xs,
          ),
        ),
        child: Text(
          downloading
              ? '${strings.modulesPhaseDownload} · '
                    '${strings.transfersProgressPercent(operation.downloadPercent)}'
              : strings.modulesPhaseInstall,
        ),
      ),
    );
  }

  Future<void> _settings(BuildContext context, String id) => showOverlay<void>(
    context,
    AppOverlays.drawerConfiguration(context),
    builder: (context) => ModuleSettingsDrawer(
      controller: controller,
      moduleId: id,
      services: widget.services,
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

  /// Enabling a module that uses host services first confirms them.
  Future<void> _setEnabled(
    BuildContext context,
    InstalledModule module,
    bool enabled,
  ) async {
    if (!enabled || module.capabilities.isEmpty) {
      await controller.setEnabled(module.id, enabled);
      return;
    }
    await AppOverlays.showActionDialog(
      context,
      icon: LucideIcons.shieldCheck,
      title: Text(context.l10n.modulesEnableTitle(module.title)),
      content: Column(
        key: ValueKey('module-capabilities-${module.id}'),
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(context.l10n.modulesEnableAccess),
          for (final capability in module.capabilities) ...[
            const Gap(AppSpacing.sm),
            Row(
              children: [
                const AppIconFrame(
                  size: AppIconSize.sm,
                  child: Icon(LucideIcons.check, size: AppIconSize.sm),
                ),
                const Gap(AppSpacing.sm),
                Expanded(
                  child: Text(
                    context.l10n.modulesCapability(capability.wireName),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
      actionLabel: context.l10n.commonEnable,
      destructive: false,
      changes: controller,
      canConfirm: () => !controller.busy,
      onConfirm: () => controller.setEnabled(
        module.id,
        true,
        confirmed: module.capabilities,
      ),
      failureMessage: () =>
          _dialogFailure(AppStrings.current.modulesOperationIncomplete),
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
          _dialogFailure(AppStrings.current.modulesRemoveFailed),
    );
  }
}
