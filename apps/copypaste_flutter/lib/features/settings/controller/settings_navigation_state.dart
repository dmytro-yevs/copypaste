import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart' show TargetPlatform;
import 'package:shadcn_flutter/shadcn_flutter.dart' show IconData, LucideIcons;

/// Lightweight settings navigation retained while presentation is unloaded.
class SettingsNavigationState {
  SettingsSectionId _section = SettingsSectionId.clipboard;
  String _searchText = '';
  String? _selectedTargetId;
  String? _moduleId;

  SettingsSectionId get section => _section;

  /// The module whose section [SettingsSectionId.module] shows.
  String? get moduleId => _moduleId;
  String get searchText => _searchText;
  String? get selectedTargetId => _selectedTargetId;

  void search(String value) {
    _searchText = value;
    _selectedTargetId = null;
  }

  void select(SettingsSectionId value, {String? targetId, String? moduleId}) {
    _section = value;
    _selectedTargetId = targetId;
    if (value == SettingsSectionId.module) _moduleId = moduleId;
  }
}

enum SettingsSectionId {
  general(slug: 'general', icon: LucideIcons.settings),
  appearance(slug: 'appearance', icon: LucideIcons.palette),
  clipboard(slug: 'clipboard', icon: LucideIcons.clipboard),
  privacy(slug: 'privacy', icon: LucideIcons.shield),
  rules(slug: 'rules', icon: LucideIcons.listFilter),
  quickPaste(slug: 'quick-paste', icon: LucideIcons.keyboard),
  shortcuts(
    slug: 'shortcuts',
    icon: LucideIcons.command,
    platforms: {
      TargetPlatform.macOS,
      TargetPlatform.windows,
      TargetPlatform.linux,
    },
  ),
  quickAccess(
    slug: 'quick-access',
    icon: LucideIcons.layoutGrid,
    platforms: {TargetPlatform.android},
  ),

  /// One section per enabled module with a settings surface, titled like the
  /// module; [SettingsNavigationState.moduleId] says which.
  module(slug: 'module', icon: LucideIcons.puzzle),
  transforms(slug: 'transforms', icon: LucideIcons.wandSparkles),
  sync(slug: 'sync', icon: LucideIcons.refreshCw),
  notifications(slug: 'notifications', icon: LucideIcons.bell),
  data(slug: 'data', icon: LucideIcons.database),
  modules(slug: 'modules', icon: LucideIcons.puzzle),
  diagnostics(slug: 'diagnostics', icon: LucideIcons.activity),
  about(slug: 'about', icon: LucideIcons.info);

  const SettingsSectionId({
    required this.slug,
    required this.icon,
    this.platforms,
  });

  final String slug;
  final IconData icon;

  /// Platforms that can show the section; `null` means every platform.
  final Set<TargetPlatform>? platforms;

  bool availableOn(TargetPlatform platform) =>
      platforms?.contains(platform) ?? true;

  String get label {
    final strings = AppStrings.current;
    return switch (this) {
      SettingsSectionId.general => strings.settingsSectionGeneral,
      SettingsSectionId.appearance => strings.settingsSectionAppearance,
      SettingsSectionId.clipboard => strings.settingsSectionClipboard,
      SettingsSectionId.privacy => strings.settingsSectionPrivacy,
      SettingsSectionId.rules => strings.settingsSectionRules,
      SettingsSectionId.quickPaste => strings.settingsSectionQuickPaste,
      SettingsSectionId.shortcuts => strings.settingsSectionShortcuts,
      SettingsSectionId.quickAccess => strings.settingsSectionQuickAccess,
      SettingsSectionId.module => strings.settingsSectionModules,
      SettingsSectionId.transforms => strings.settingsSectionTransforms,
      SettingsSectionId.sync => strings.settingsSectionSync,
      SettingsSectionId.notifications => strings.settingsSectionNotifications,
      SettingsSectionId.data => strings.settingsSectionData,
      SettingsSectionId.modules => strings.settingsSectionModules,
      SettingsSectionId.diagnostics => strings.settingsSectionDiagnostics,
      SettingsSectionId.about => strings.settingsSectionAbout,
    };
  }

  String get description {
    final strings = AppStrings.current;
    return switch (this) {
      SettingsSectionId.general => strings.settingsSectionGeneralSummary,
      SettingsSectionId.appearance => strings.settingsSectionAppearanceSummary,
      SettingsSectionId.clipboard => strings.settingsSectionClipboardSummary,
      SettingsSectionId.privacy => strings.settingsSectionPrivacySummary,
      SettingsSectionId.rules => strings.settingsSectionRulesSummary,
      SettingsSectionId.quickPaste => strings.settingsSectionQuickPasteSummary,
      SettingsSectionId.shortcuts => strings.settingsSectionShortcutsSummary,
      SettingsSectionId.quickAccess =>
        strings.settingsSectionQuickAccessSummary,
      SettingsSectionId.module => strings.settingsSectionModulesSummary,
      SettingsSectionId.transforms => strings.settingsSectionTransformsSummary,
      SettingsSectionId.sync => strings.settingsSectionSyncSummary,
      SettingsSectionId.notifications =>
        strings.settingsSectionNotificationsSummary,
      SettingsSectionId.data => strings.settingsSectionDataSummary,
      SettingsSectionId.modules => strings.settingsSectionModulesSummary,
      SettingsSectionId.diagnostics =>
        strings.settingsSectionDiagnosticsSummary,
      SettingsSectionId.about => strings.settingsSectionAboutSummary,
    };
  }
}
