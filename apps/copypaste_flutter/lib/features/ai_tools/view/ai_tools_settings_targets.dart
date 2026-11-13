import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../l10n/l10n.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';

/// Rows of the AI tools section that settings search can reveal.
class AiToolsSettingsTargets {
  AiToolsSettingsTargets({required this.serving});

  /// Desktops list the local toggle; Android does not serve AI tools.
  final bool serving;

  static const enabledId = 'result-ai-tools-enabled';
  static const grantsId = 'result-ai-tools-grants';
  static const devicesId = 'result-ai-tools-devices';
  static const kindsId = 'result-ai-tools-kinds';
  static const appsId = 'result-ai-tools-apps';
  static const lookbackId = 'result-ai-tools-lookback';

  final enabled = GlobalKey(debugLabel: enabledId);
  final grants = GlobalKey(debugLabel: grantsId);
  final devices = GlobalKey(debugLabel: devicesId);
  final kinds = GlobalKey(debugLabel: kindsId);
  final apps = GlobalKey(debugLabel: appsId);
  final lookback = GlobalKey(debugLabel: lookbackId);

  List<SettingsSearchTarget> searchTargets(BuildContext context) {
    final l10n = context.l10n;
    SettingsSearchTarget target(
      String id,
      GlobalKey key,
      String label,
      String description,
    ) => SettingsSearchTarget(
      id: id,
      section: SettingsSectionId.aiTools,
      label: label,
      description: description,
      keywords: l10n.aiToolsKeywords,
      targetKey: key,
    );
    return [
      if (serving)
        target(
          enabledId,
          enabled,
          l10n.aiToolsEnableTitle,
          l10n.aiToolsEnableDescription,
        ),
      target(
        grantsId,
        grants,
        l10n.aiToolsGrantsTitle,
        l10n.aiToolsGrantsDescription,
      ),
      target(
        devicesId,
        devices,
        l10n.aiToolsOtherDevicesTitle,
        l10n.aiToolsOtherDevicesDescription,
      ),
      target(
        kindsId,
        kinds,
        l10n.aiToolsKindsTitle,
        l10n.aiToolsKindsDescription,
      ),
      target(
        appsId,
        apps,
        l10n.aiToolsExcludedAppsTitle,
        l10n.aiToolsExcludedAppsDescription,
      ),
      target(
        lookbackId,
        lookback,
        l10n.aiToolsDefaultLookbackTitle,
        l10n.aiToolsDefaultLookbackDescription,
      ),
    ];
  }
}
