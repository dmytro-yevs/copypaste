import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/format_bytes.dart';
import '../../../shared/inspector_table.dart';
import '../models/module_marketplace_models.dart';
import '../models/module_models.dart';

/// What a module is for: what it adds, the CopyPaste services it uses and
/// its requirements, for installed and marketplace modules alike.
class ModuleDetailsDrawer extends StatelessWidget {
  const ModuleDetailsDrawer({
    super.key,
    this.installed,
    this.marketplace,
    this.action,
  }) : assert(installed != null || marketplace != null);

  final InstalledModule? installed;
  final MarketplaceModule? marketplace;

  /// The module's main action (install or open settings), in the footer.
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final installed = this.installed;
    final title = installed?.title ?? marketplace!.title;
    final platforms =
        installed != null && installed.supportedPlatforms.isNotEmpty
        ? installed.supportedPlatforms
        : marketplace?.supportedPlatforms ?? const <ModulePlatform>[];
    final size = installed == null
        ? marketplace!.downloadSize
        : formatBytes(installed.sizeBytes);
    final requirements = [
      ?marketplace?.appRequirement,
      ?marketplace?.systemRequirement,
    ];
    final status =
        installed?.error ??
        (installed == null ? marketplace?.availabilityNotice : null);
    final features = [
      if (installed?.searchModels.isNotEmpty ?? false)
        l10n.modulesDetailsSearch,
      for (final command in installed?.commands ?? const <ModuleCommand>[])
        command.description.trim().isEmpty
            ? command.title
            : '${command.title}: ${command.description}',
    ];
    return ConstrainedBox(
      key: ValueKey('module-details-${installed?.id ?? marketplace!.id}'),
      constraints: AppOverlays.drawerContentConstraints(context),
      child: Scaffold(
        headers: [
          AppBar(
            title: Text(title, maxLines: 1, overflow: TextOverflow.ellipsis),
            leading: const [AppIconFrame(child: Icon(LucideIcons.puzzle))],
            trailing: [
              Semantics(
                label: l10n.commonClose,
                button: true,
                child: Button.ghost(
                  style: AppTheme.controlButtonStyle(
                    const ButtonStyle.ghostIcon(),
                  ),
                  onPressed: () => closeDrawer(context),
                  child: const AppIconFrame(child: Icon(LucideIcons.x)),
                ),
              ),
            ],
          ),
          const Divider(),
        ],
        footers: [
          if (action case final action?) ...[
            const Divider(),
            Padding(
              padding: const EdgeInsets.all(AppSpacing.lg),
              child: Wrap(children: [action]),
            ),
          ],
        ],
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(AppSpacing.lg),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(installed?.description ?? marketplace!.description),
              if (features.isNotEmpty) ...[
                const Gap(AppSpacing.xl),
                Text(l10n.modulesDetailsFeatures).semiBold(),
                const Gap(AppSpacing.sm),
                for (final feature in features)
                  _Point(icon: LucideIcons.check, text: feature),
              ],
              const Gap(AppSpacing.xl),
              Text(l10n.modulesDetailsPermissions).semiBold(),
              const Gap(AppSpacing.sm),
              if (installed == null)
                Text(l10n.modulesDetailsPermissionsPending).muted()
              else if (installed.capabilities.isEmpty)
                Text(l10n.modulesDetailsNoPermissions).muted()
              else
                for (final capability in installed.capabilities)
                  _Point(
                    icon: LucideIcons.shieldCheck,
                    text: l10n.modulesCapability(capability.wireName),
                  ),
              const Gap(AppSpacing.xl),
              Text(l10n.modulesDetailsInformation).semiBold(),
              const Gap(AppSpacing.sm),
              InspectorTable(
                rows: [
                  (
                    label: l10n.modulesDetailsVersion,
                    value: Text(
                      installed?.version ?? marketplace!.version.toString(),
                    ),
                  ),
                  if (size != null)
                    (label: l10n.modulesDetailsSize, value: Text(size)),
                  (
                    label: l10n.modulesDetailsRequirements,
                    value: Text(
                      [
                        formatModulePlatforms(platforms),
                        ...requirements,
                      ].join('\n'),
                    ),
                  ),
                  if (status != null)
                    (label: l10n.modulesDetailsStatus, value: Text(status)),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Point extends StatelessWidget {
  const _Point({required this.icon, required this.text});

  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: AppSpacing.sm),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        AppIconFrame(
          size: AppIconSize.sm,
          child: Icon(icon, size: AppIconSize.sm),
        ),
        const Gap(AppSpacing.sm),
        Expanded(child: Text(text)),
      ],
    ),
  );
}
