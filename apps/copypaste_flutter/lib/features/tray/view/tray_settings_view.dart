import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../l10n/l10n.dart';
import '../../settings/controller/settings_navigation_state.dart';
import '../../settings/view/settings_search_target.dart';
import '../../settings/view/settings_section.dart';
import '../controller/tray_menu_controller.dart';
import '../models/tray_menu_model.dart';
import '../repository/tray_preferences_store.dart';

/// The tray menu's Recent options, contributed to the General settings.
class TraySettingsContribution {
  TraySettingsContribution(this.controller);

  static const countTargetId = 'result-tray-recent-count';
  static const previewTargetId = 'result-tray-recent-preview';

  final TrayMenuController controller;
  final _countKey = GlobalKey(debugLabel: countTargetId);
  final _previewKey = GlobalKey(debugLabel: previewTargetId);

  SettingsFeatureSection get section => SettingsFeatureSection(
    section: SettingsSectionId.general,
    builder: (context, scope) => TraySettingsView(
      controller: controller,
      scope: scope,
      countKey: _countKey,
      previewKey: _previewKey,
    ),
    searchTargets: (context) {
      final l10n = context.l10n;
      return [
        SettingsSearchTarget(
          id: countTargetId,
          section: SettingsSectionId.general,
          label: l10n.trayRecentCountTitle,
          description: l10n.trayRecentCountDescription,
          keywords: l10n.trayRecentSettingsKeywords,
          targetKey: _countKey,
        ),
        SettingsSearchTarget(
          id: previewTargetId,
          section: SettingsSectionId.general,
          label: l10n.trayRecentPreviewTitle,
          description: l10n.trayRecentPreviewDescription,
          keywords: l10n.trayRecentSettingsKeywords,
          targetKey: _previewKey,
        ),
      ];
    },
  );
}

class TraySettingsView extends StatelessWidget {
  const TraySettingsView({
    super.key,
    required this.controller,
    required this.scope,
    required this.countKey,
    required this.previewKey,
  });

  final TrayMenuController controller;
  final SettingsSectionScope scope;
  final GlobalKey countKey;
  final GlobalKey previewKey;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return ValueListenableBuilder<TrayPreferences>(
      valueListenable: controller.preferences,
      builder: (context, preferences, _) => SettingsSection(
        title: l10n.trayRecentSettingsTitle,
        description: l10n.trayRecentSettingsDescription,
        children: [
          SettingRow(
            key: countKey,
            highlighted: scope.isHighlighted(
              TraySettingsContribution.countTargetId,
            ),
            title: l10n.trayRecentCountTitle,
            description: l10n.trayRecentCountDescription,
            trailing: Select<int>(
              key: const ValueKey('tray-recent-count'),
              value: preferences.recentLimit,
              onChanged: (limit) {
                if (limit == null) return;
                unawaited(
                  controller.setPreferences(
                    preferences.copyWith(recentLimit: limit),
                  ),
                );
              },
              popup: SelectPopup<int>(
                items: SelectItemList(
                  children: [
                    for (final limit in TrayPreferences.recentLimitOptions)
                      SelectItemButton<int>(
                        value: limit,
                        child: Text('$limit'),
                      ),
                  ],
                ),
              ).call,
              itemBuilder: (context, limit) => Text('$limit'),
            ),
          ),
          SettingRow(
            key: previewKey,
            highlighted: scope.isHighlighted(
              TraySettingsContribution.previewTargetId,
            ),
            title: l10n.trayRecentPreviewTitle,
            description: l10n.trayRecentPreviewDescription,
            trailing: Select<TrayPreviewLevel>(
              key: const ValueKey('tray-recent-preview'),
              value: preferences.previewLevel,
              onChanged: (level) {
                if (level == null) return;
                unawaited(
                  controller.setPreferences(
                    preferences.copyWith(previewLevel: level),
                  ),
                );
              },
              popup: SelectPopup<TrayPreviewLevel>(
                items: SelectItemList(
                  children: [
                    for (final level in TrayPreviewLevel.values)
                      SelectItemButton<TrayPreviewLevel>(
                        value: level,
                        child: Text(_levelLabel(l10n, level)),
                      ),
                  ],
                ),
              ).call,
              itemBuilder: (context, level) => Text(_levelLabel(l10n, level)),
            ),
          ),
        ],
      ),
    );
  }

  static String _levelLabel(AppLocalizations l10n, TrayPreviewLevel level) =>
      switch (level) {
        TrayPreviewLevel.full => l10n.trayRecentPreviewFull,
        TrayPreviewLevel.titles => l10n.trayRecentPreviewTitles,
        TrayPreviewLevel.hidden => l10n.trayRecentPreviewHidden,
      };
}
