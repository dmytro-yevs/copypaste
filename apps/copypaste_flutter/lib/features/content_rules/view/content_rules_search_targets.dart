import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../l10n/l10n.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';
import '../controller/content_rules_controller.dart';
import 'content_rules_settings_view.dart';

/// Rows of the Content rules section that settings search can reveal.
class ContentRulesSettingsTargets {
  static const privacyId = 'result-rules-privacy-defaults';
  static const rulesId = 'result-rules-my-rules';
  static const skippedId = 'result-rules-recently-skipped';

  final privacy = GlobalKey(debugLabel: privacyId);
  final rules = GlobalKey(debugLabel: rulesId);
  final skipped = GlobalKey(debugLabel: skippedId);

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
      section: SettingsSectionId.rules,
      label: label,
      description: description,
      keywords: keywords,
      targetKey: key,
    );
    return [
      target(
        privacyId,
        privacy,
        l10n.rulesPrivacyDefaultsTitle,
        l10n.rulesPrivacyDefaultsDescription,
        l10n.rulesPrivacyDefaultsKeywords,
      ),
      target(
        rulesId,
        rules,
        l10n.rulesMyRulesTitle,
        l10n.rulesMyRulesDescription,
        l10n.rulesMyRulesKeywords,
      ),
      target(
        skippedId,
        skipped,
        l10n.rulesSkipLogTitle,
        l10n.rulesSkipLogDescription,
        l10n.rulesSkipLogKeywords,
      ),
    ];
  }
}

/// The Content rules settings contribution. [onOpenPrivacy] shows the
/// Privacy section, whose settings guard capture before any rule.
SettingsFeatureSection contentRulesSettingsSection(
  ContentRulesController controller, {
  VoidCallback? onOpenPrivacy,
}) {
  final targets = ContentRulesSettingsTargets();
  return SettingsFeatureSection(
    section: SettingsSectionId.rules,
    builder: (context, scope) => ContentRulesSettingsView(
      controller: controller,
      targets: targets,
      scope: scope,
      onOpenPrivacy: onOpenPrivacy,
    ),
    searchTargets: targets.searchTargets,
  );
}
