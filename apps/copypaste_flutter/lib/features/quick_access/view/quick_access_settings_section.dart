import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../l10n/l10n.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';
import '../controller/quick_access_controller.dart';
import 'quick_access_settings_view.dart';

/// Rows of the Quick access section that settings search can reveal.
class QuickAccessSettingsTargets {
  static const keyboardId = 'result-quick-access-keyboard';
  static const captureTierId = 'result-quick-access-capture-tier';
  static const autoReturnId = 'result-quick-access-auto-return';
  static const tilesId = 'result-quick-access-tiles';
  static const widgetId = 'result-quick-access-widget';

  final keyboard = GlobalKey(debugLabel: keyboardId);
  final captureTier = GlobalKey(debugLabel: captureTierId);
  final autoReturn = GlobalKey(debugLabel: autoReturnId);
  final tiles = GlobalKey(debugLabel: tilesId);
  final widget = GlobalKey(debugLabel: widgetId);

  List<SettingsSearchTarget> searchTargets(BuildContext context) {
    final l10n = context.l10n;
    SettingsSearchTarget target(
      String id,
      GlobalKey key,
      String label,
      String description,
      String keywords,
    ) => SettingsSearchTarget(
      id: id,
      section: SettingsSectionId.quickAccess,
      label: label,
      description: description,
      keywords: keywords,
      targetKey: key,
    );
    return [
      target(
        keyboardId,
        keyboard,
        l10n.quickAccessKeyboardTitle,
        l10n.quickAccessKeyboardSearchDescription,
        l10n.quickAccessKeyboardKeywords,
      ),
      target(
        captureTierId,
        captureTier,
        l10n.quickAccessCaptureTierTitle,
        l10n.quickAccessCaptureTierSearchDescription,
        l10n.quickAccessCaptureTierKeywords,
      ),
      target(
        autoReturnId,
        autoReturn,
        l10n.quickAccessAutoReturnTitle,
        l10n.quickAccessAutoReturnDescription,
        l10n.quickAccessAutoReturnKeywords,
      ),
      target(
        tilesId,
        tiles,
        l10n.quickAccessTilesTitle,
        l10n.quickAccessTilesSearchDescription,
        l10n.quickAccessTilesKeywords,
      ),
      target(
        widgetId,
        widget,
        l10n.quickAccessWidgetTitle,
        l10n.quickAccessWidgetDescription,
        l10n.quickAccessWidgetKeywords,
      ),
    ];
  }
}

/// The Quick access settings contribution; [capturePaused] is read on every
/// build of the settings screen so the capture tier never shows stale pause.
SettingsFeatureSection quickAccessSettingsSection({
  required QuickAccessController controller,
  required bool Function() capturePaused,
}) {
  final targets = QuickAccessSettingsTargets();
  return SettingsFeatureSection(
    section: SettingsSectionId.quickAccess,
    builder: (context, scope) => QuickAccessSettingsView(
      controller: controller,
      targets: targets,
      scope: scope,
      capturePaused: capturePaused(),
    ),
    searchTargets: targets.searchTargets,
  );
}
