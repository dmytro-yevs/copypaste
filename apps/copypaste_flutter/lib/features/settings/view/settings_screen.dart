import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import 'dart:async';
import 'dart:io';

import '../../modules/controller/modules_controller.dart';
import '../../modules/view/modules_settings_view.dart';

import 'package:flutter/foundation.dart';
import 'package:flutter/semantics.dart' show SemanticsRole;
import 'package:intl/intl.dart' show NumberFormat;
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/localization/app_language.dart';
import '../../../app/navigation/app_page_route.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../platform/desktop/global_shortcut.dart';
import '../../../platform/desktop/desktop_visibility_controller.dart';
import '../../../platform/permissions/linux_integration.dart';
import '../../../shared/adaptive_breakpoints.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/format_bytes.dart';
import '../../../shared/state_view.dart';
import '../../../shared/system_date_time.dart';
import '../../applications/controller/application_directory.dart';
import '../../applications/repository/application_catalog.dart';
import '../../applications/view/application_picker.dart';
import '../../transfers/file_operations_button_progress.dart';
import '../controller/quick_paste_settings_controller.dart';
import '../controller/settings_controller.dart';
import '../controller/launch_at_login_controller.dart';
import '../../../platform/startup/launch_at_login.dart';
import '../controller/settings_navigation_state.dart';
import '../models/desktop_presence.dart';
import '../models/settings_limits.dart';
import '../models/settings_models.dart';
import '../models/settings_outcomes.dart';
import '../models/storage_usage_text.dart';
import '../../update/controller/app_update_controller.dart';
import '../../update/models/app_update_models.dart';
import 'confirmation_dialog.dart';
import 'settings_search_target.dart';
import 'settings_section.dart';
import 'shortcut_recorder.dart';
import 'stop_scale.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.controller,
    this.quickPaste,
    this.desktopVisibility,
    this.launchAtLogin,
    this.appUpdate,
    this.modules,
    this.onOpenAndroidCaptureSetup,
    this.language,
    this.featureSections = const [],
    this.applications = const RuntimeApplicationCatalog(),
  });

  final SettingsController controller;
  final QuickPasteSettingsController? quickPaste;
  final DesktopVisibilityController? desktopVisibility;
  final LaunchAtLoginController? launchAtLogin;
  final AppUpdateController? appUpdate;
  final ModulesController? modules;
  final Future<void> Function()? onOpenAndroidCaptureSetup;
  final AppLanguageController? language;

  /// Feature-owned section content, in the order it renders.
  final List<SettingsFeatureSection> featureSections;

  /// Applications the excluded applications list picks from and names.
  final ApplicationCatalog applications;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  String get _screenshotProtectionDescription =>
      widget.controller.blockScreenshotsSupported
      ? context.l10n.settingsBlockScreenshotsDescription
      : context.l10n.settingsBlockScreenshotsUnsupported;

  final _generalSectionKey = GlobalKey();
  final _languageKey = GlobalKey();
  final _launchAtLoginKey = GlobalKey();
  final _taskbarVisibilityKey = GlobalKey();
  final _trayVisibilityKey = GlobalKey();
  final _clipboardSectionKey = GlobalKey();
  final _modulesSectionKey = GlobalKey();
  final _privacySectionKey = GlobalKey();
  final _skipSecretKey = GlobalKey();
  final _skipTransientKey = GlobalKey();
  final _blockScreenshotsKey = GlobalKey();
  final _dataSectionKey = GlobalKey();
  final _syncSectionKey = GlobalKey();
  final _notificationsSectionKey = GlobalKey();
  final _quickPasteSectionKey = GlobalKey();
  final _androidCaptureKey = GlobalKey();
  final _clipboardCaptureKey = GlobalKey();
  final _screenshotCaptureKey = GlobalKey();
  final _excludedApplicationsKey = GlobalKey();
  late final _applications = ApplicationDirectory(widget.applications);
  final _retentionKey = GlobalKey();
  final _storageQuotaKey = GlobalKey();
  final _maxFileSizeKey = GlobalKey();
  final _historyFilesKey = GlobalKey();
  final _syncEnabledKey = GlobalKey();
  final _instantClipboardKey = GlobalKey();
  final _lanVisibilityKey = GlobalKey();
  final _notificationOnCopyKey = GlobalKey();
  final _notificationPreviewKey = GlobalKey();
  final _soundOnCopyKey = GlobalKey();
  final _applicationUpdatesKey = GlobalKey();
  final _quickPasteShortcutKey = GlobalKey();
  final _quickPasteAutoPasteKey = GlobalKey();
  final _aboutSectionKey = GlobalKey();
  final _featureSectionKeys = <SettingsSectionId, GlobalKey>{};
  final _detailRevision = ValueNotifier<int>(0);
  final _searchFocus = FocusNode();
  late final TextEditingController _searchController;
  Timer? _highlightTimer;

  SettingsSectionId get _selectedSection =>
      widget.controller.navigation.section;
  String get _searchQuery =>
      widget.controller.navigation.searchText.trim().toLowerCase();
  String? get _selectedTargetId =>
      widget.controller.navigation.selectedTargetId;

  @override
  void initState() {
    super.initState();
    _searchController = TextEditingController(
      text: widget.controller.navigation.searchText,
    );
  }

  String? _highlightedTargetId;
  bool _compactRouteOpen = false;

  @override
  void dispose() {
    _highlightTimer?.cancel();
    _searchController.dispose();
    _searchFocus.dispose();
    _detailRevision.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: Listenable.merge([
        widget.controller,
        widget.desktopVisibility,
        widget.launchAtLogin,
      ]),
      builder: (context, _) => _loadedContent(() => _content(context)),
    );
  }

  Widget _loadedContent(Widget Function() ready) =>
      switch (widget.controller.loadState) {
        SettingsLoadState.loading => StateView.loading(
          message: context.l10n.settingsLoadingMessage,
        ),
        SettingsLoadState.error => StateView.error(
          title: context.l10n.settingsUnavailableTitle,
          message: widget.controller.errorMessage,
          actionLabel: context.l10n.commonTryAgain,
          onAction: widget.controller.retry,
        ),
        SettingsLoadState.ready => ready(),
      };

  Widget _content(BuildContext context) {
    final controller = widget.controller;
    final settings = controller.settings!;
    final selectedSection = _effectiveSelectedSection;
    final targets = _navigationTargets();
    final noSearchResults = _searchQuery.isNotEmpty && targets.isEmpty;

    return LayoutBuilder(
      builder: (context, constraints) {
        final showSidebar =
            constraints.maxWidth >= AdaptiveBreakpoints.settingsNavigation;
        final content = _compactRouteOpen
            ? const SizedBox.shrink()
            : noSearchResults
            ? StateView.empty(
                title: context.l10n.settingsNoResultsTitle,
                message: context.l10n.settingsNoResultsMessage,
              )
            : _sectionContent(context, settings, selectedSection);

        if (showSidebar) {
          return Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                width: AppLayoutSize.settingsNavigationWidth,
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.secondary,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Padding(
                        padding: AppTheme.settingsSearchPadding,
                        child: _searchField(),
                      ),
                      Expanded(
                        child: NavigationSidebar(
                          key: const ValueKey<String>(
                            'settings-navigation-sidebar',
                          ),
                          // The pane paints the background; a sidebar fill
                          // would cover the search field's focus ring.
                          padding: AppTheme.settingsNavigationPadding,
                          spacing: AppTheme.settingsNavigationSpacing,
                          labelSize: NavigationLabelSize.large,
                          selectedKey: _selectedNavigationKey(
                            targets,
                            selectedSection,
                          ),
                          onSelected: (key) =>
                              _selectNavigationTarget(key, targets),
                          children: [
                            for (final target in targets)
                              NavigationItem(
                                key: ValueKey<String>(target.widgetKey),
                                style: AppTheme.settingsNavigationButtonStyle,
                                selectedStyle: AppTheme
                                    .settingsNavigationSelectedButtonStyle,
                                // The default marquee keeps a ticker running
                                // for as long as a label is too long.
                                overflow: NavigationOverflow.ellipsis,
                                label: _navigationTargetLabel(target),
                                child: AppIconFrame(
                                  size: AppIconSize.sm,
                                  child: Icon(
                                    target.section.icon,
                                    size: AppIconSize.sm,
                                  ),
                                ),
                              ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              const VerticalDivider(),
              Expanded(child: content),
            ],
          );
        }

        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: AppTheme.settingsSearchPadding,
              child: _searchField(),
            ),
            Expanded(
              child: noSearchResults
                  ? Padding(
                      padding: const EdgeInsets.only(top: AppSpacing.lg),
                      child: content,
                    )
                  : _compactNavigationList(context, targets),
            ),
          ],
        );
      },
    );
  }

  Widget _sectionContent(
    BuildContext context,
    RuntimeSettings settings,
    SettingsSectionId selectedSection, {
    bool showHeading = true,
  }) {
    final controller = widget.controller;
    return SingleChildScrollView(
      key: PageStorageKey<String>('settings-${selectedSection.slug}-scroll'),
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.lg,
        AppSpacing.lg,
        AppSpacing.lg,
        AppSpacing.huge,
      ).add(EdgeInsets.only(bottom: MediaQuery.paddingOf(context).bottom)),
      child: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(
            maxWidth: AppLayoutSize.settingsContentMaxWidth,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (controller.errorMessage case final message?) ...[
                Alert.destructive(
                  leading: const AppIconFrame(
                    child: Icon(LucideIcons.circleAlert),
                  ),
                  title: Text(context.l10n.settingsAttentionTitle),
                  content: Text(message),
                  trailing: Tooltip(
                    showDuration: AppMotion.resolve(
                      context,
                      AppMotion.standard,
                    ),
                    tooltip: (context) => TooltipContainer(
                      child: Text(context.l10n.commonDismiss),
                    ),
                    child: Semantics(
                      label: context.l10n.commonDismiss,
                      child: Button.ghost(
                        key: const ValueKey<String>('settings-error-dismiss'),
                        style: AppTheme.controlButtonStyle(
                          const ButtonStyle.ghostIcon(),
                        ),
                        onPressed: controller.clearMessages,
                        child: const AppIconFrame(child: Icon(LucideIcons.x)),
                      ),
                    ),
                  ),
                ),
                const Gap(AppSpacing.xxxl),
              ],
              _withContributions(selectedSection, switch (selectedSection) {
                SettingsSectionId.general => _generalSection(
                  showHeading: showHeading,
                ),
                SettingsSectionId.modules => SettingsSection(
                  key: _modulesSectionKey,
                  title: context.l10n.settingsSectionModules,
                  showHeading: showHeading,
                  groupContent: false,
                  description: context.l10n.settingsModulesDescription,
                  children: [ModulesSettingsView(controller: widget.modules!)],
                ),
                SettingsSectionId.clipboard => _clipboardSection(
                  settings,
                  showHeading: showHeading,
                ),
                SettingsSectionId.privacy => _privacySection(
                  settings,
                  showHeading: showHeading,
                ),
                SettingsSectionId.data => _dataSection(
                  showHeading: showHeading,
                ),
                SettingsSectionId.sync => _syncSection(
                  settings,
                  showHeading: showHeading,
                ),
                SettingsSectionId.notifications => _notificationsSection(
                  settings,
                  showHeading: showHeading,
                ),
                SettingsSectionId.about => AnimatedBuilder(
                  animation: widget.appUpdate ?? widget.controller,
                  builder: (context, _) =>
                      _aboutSection(showHeading: showHeading),
                ),
                SettingsSectionId.quickPaste => _QuickPasteSection(
                  key: _quickPasteSectionKey,
                  controller: widget.quickPaste!,
                  showHeading: showHeading,
                  shortcutKey: _quickPasteShortcutKey,
                  autoPasteKey: _quickPasteAutoPasteKey,
                  shortcutHighlighted: _isHighlighted(
                    _SettingsTargetId.quickPasteShortcut,
                  ),
                  autoPasteHighlighted: _isHighlighted(
                    _SettingsTargetId.quickPasteAutoPaste,
                  ),
                ),
                SettingsSectionId.rules ||
                SettingsSectionId.quickAccess ||
                SettingsSectionId.snippets ||
                SettingsSectionId.transforms ||
                SettingsSectionId.aiTools => _featureSection(
                  selectedSection,
                  showHeading: showHeading,
                ),
              }),
            ],
          ),
        ),
      ),
    );
  }

  Widget _searchField() {
    return TextField(
      key: const ValueKey<String>('settings-search'),
      controller: _searchController,
      focusNode: _searchFocus,
      placeholder: Text(context.l10n.settingsSearchPlaceholder),
      onChanged: _updateSearch,
      // The icon slots would otherwise be empty focus scopes that stop Tab.
      skipInputFeatureFocusTraversal: true,
      features: const [
        InputFeature.leading(AppIconFrame(child: Icon(LucideIcons.search))),
        InputFeature.clear(),
      ],
    );
  }

  Widget _compactNavigationList(
    BuildContext context,
    List<SettingsSearchTarget> targets,
  ) {
    return ListView.separated(
      key: const ValueKey<String>('settings-category-list'),
      controller: PrimaryScrollController.maybeOf(context),
      primary: false,
      padding: AppTheme.settingsNavigationPadding.add(
        EdgeInsets.only(bottom: MediaQuery.paddingOf(context).bottom),
      ),
      itemCount: targets.length,
      separatorBuilder: (context, index) =>
          const Divider(theme: AppTheme.settingsNavigationDividerTheme),
      itemBuilder: (context, index) {
        final target = targets[index];
        return Button.ghost(
          key: ValueKey<String>('mobile-${target.widgetKey}'),
          style: AppTheme.settingsCategoryButtonStyle,
          alignment: Alignment.centerLeft,
          onPressed: () => _openCompactSection(context, target),
          child: Row(
            children: [
              AppIconFrame(
                size: AppIconSize.md,
                child: Icon(target.section.icon, size: AppIconSize.md),
              ),
              const Gap(AppSpacing.md),
              Expanded(child: _navigationTargetLabel(target)),
              const Gap(AppSpacing.sm),
              const AppIconFrame(
                size: AppIconSize.sm,
                child: Icon(LucideIcons.chevronRight, size: AppIconSize.sm),
              ),
            ],
          ),
        );
      },
    );
  }

  void _openCompactSection(BuildContext context, SettingsSearchTarget target) {
    _compactRouteOpen = true;
    _activateTarget(target);
    final route = AppPageRoute<void>(
      disableAnimations: MediaQuery.disableAnimationsOf(context),
      settings: RouteSettings(name: 'settings/${target.section.slug}'),
      builder: (context) => AnimatedBuilder(
        animation: Listenable.merge([
          widget.controller,
          widget.desktopVisibility,
          widget.launchAtLogin,
          _detailRevision,
        ]),
        builder: (context, _) => Scaffold(
          headers: [
            AppBar(
              leading: [
                Semantics(
                  label: context.l10n.settingsBackToSettings,
                  child: Button.ghost(
                    key: const ValueKey<String>('settings-back'),
                    style: AppTheme.navigationIconButtonStyle,
                    onPressed: () => Navigator.of(context).pop(),
                    child: const AppIconFrame(
                      child: Icon(LucideIcons.arrowLeft),
                    ),
                  ),
                ),
              ],
              title: Text(target.section.label),
            ),
            const Divider(),
          ],
          child: _loadedContent(
            () => _sectionContent(
              context,
              widget.controller.settings!,
              target.section,
              showHeading: false,
            ),
          ),
        ),
      ),
    );
    unawaited(Navigator.of(context).push<void>(route));
    unawaited(
      route.completed.then<void>((_) {
        if (mounted) setState(() => _compactRouteOpen = false);
      }),
    );
  }

  Widget _navigationTargetLabel(SettingsSearchTarget target) {
    if (!target.isSearchResult) return Text(target.label);
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(target.label, maxLines: 1, overflow: TextOverflow.ellipsis),
        Text(
          target.section.label,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ).muted().textSmall(),
      ],
    );
  }

  SettingsSectionId get _effectiveSelectedSection =>
      _sectionAvailable(_selectedSection)
      ? _selectedSection
      : SettingsSectionId.clipboard;

  bool _sectionAvailable(SettingsSectionId section) {
    if (!section.availableOn(defaultTargetPlatform)) return false;
    return switch (section) {
      SettingsSectionId.general => _hasGeneralSection,
      SettingsSectionId.quickPaste => widget.quickPaste != null,
      SettingsSectionId.modules => widget.modules != null,
      SettingsSectionId.rules ||
      SettingsSectionId.quickAccess ||
      SettingsSectionId.snippets ||
      SettingsSectionId.transforms ||
      SettingsSectionId.aiTools => _contributions(section).isNotEmpty,
      SettingsSectionId.clipboard ||
      SettingsSectionId.privacy ||
      SettingsSectionId.sync ||
      SettingsSectionId.notifications ||
      SettingsSectionId.data ||
      SettingsSectionId.about => true,
    };
  }

  Iterable<SettingsFeatureSection> _contributions(SettingsSectionId section) =>
      widget.featureSections.where(
        (contribution) => contribution.section == section,
      );

  SettingsSectionScope get _sectionScope =>
      SettingsSectionScope(highlightedTargetId: _highlightedTargetId);

  /// Appends feature contributions after a built-in section's content.
  Widget _withContributions(SettingsSectionId section, Widget content) {
    if (_featureSectionIds.contains(section)) return content;
    final contributions = _contributions(section).toList(growable: false);
    if (contributions.isEmpty) return content;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        content,
        for (final contribution in contributions) ...[
          const Gap(AppSpacing.xxxl),
          contribution.builder(context, _sectionScope),
        ],
      ],
    );
  }

  Widget _featureSection(
    SettingsSectionId section, {
    required bool showHeading,
  }) {
    final contributions = _contributions(section).toList(growable: false);
    return SettingsSection(
      key: _sectionKey(section),
      title: section.label,
      description: section.description,
      showHeading: showHeading,
      groupContent: false,
      children: [
        for (var i = 0; i < contributions.length; i++) ...[
          if (i > 0) const Gap(AppSpacing.xxxl),
          contributions[i].builder(context, _sectionScope),
        ],
      ],
    );
  }

  static const _featureSectionIds = {
    SettingsSectionId.rules,
    SettingsSectionId.quickAccess,
    SettingsSectionId.snippets,
    SettingsSectionId.transforms,
    SettingsSectionId.aiTools,
  };

  List<SettingsSearchTarget> _navigationTargets() {
    if (_searchQuery.isEmpty) return _sectionTargets();
    return _searchTargets()
        .where((target) => target.matches(_searchQuery))
        .toList(growable: false);
  }

  List<SettingsSearchTarget> _sectionTargets() {
    return [
      for (final section in SettingsSectionId.values)
        if (_sectionAvailable(section))
          SettingsSearchTarget(
            id: 'section-${section.slug}',
            section: section,
            label: section.label,
            description: section.description,
            targetKey: _sectionKey(section),
          ),
    ];
  }

  bool get _hasGeneralSection =>
      widget.language != null ||
      widget.desktopVisibility != null ||
      widget.launchAtLogin != null;

  List<SettingsSearchTarget> _searchTargets() {
    return [
      if (widget.language != null)
        SettingsSearchTarget(
          id: _SettingsTargetId.language,
          section: SettingsSectionId.general,
          label: context.l10n.settingsLanguageTitle,
          description: context.l10n.settingsLanguageDescription,
          keywords: context.l10n.settingsLanguageKeywords,
          targetKey: _languageKey,
        ),
      if (widget.launchAtLogin != null)
        SettingsSearchTarget(
          id: 'result-start-at-login',
          section: SettingsSectionId.general,
          label: _startupLabel,
          description: context.l10n.settingsStartupSearchDescription,
          keywords: context.l10n.settingsStartupKeywords,
          targetKey: _launchAtLoginKey,
        ),
      if (widget.desktopVisibility != null) ...[
        SettingsSearchTarget(
          id: 'result-taskbar-visibility',
          section: SettingsSectionId.general,
          label: _taskbarVisibilityLabel,
          description: context.l10n.settingsTaskbarVisibilityDescription,
          keywords: context.l10n.settingsTaskbarVisibilityKeywords,
          targetKey: _taskbarVisibilityKey,
        ),
        SettingsSearchTarget(
          id: 'result-tray-visibility',
          section: SettingsSectionId.general,
          label: _trayVisibilityLabel,
          description: context.l10n.settingsTrayVisibilityDescription,
          keywords: context.l10n.settingsTrayVisibilityKeywords,
          targetKey: _trayVisibilityKey,
        ),
      ],
      if (widget.modules != null)
        SettingsSearchTarget(
          id: 'result-modules',
          section: SettingsSectionId.modules,
          label: context.l10n.settingsSectionModules,
          description: context.l10n.settingsModulesSearchDescription,
          keywords: context.l10n.settingsModulesKeywords,
          targetKey: _modulesSectionKey,
        ),
      if (widget.onOpenAndroidCaptureSetup != null)
        SettingsSearchTarget(
          id: _SettingsTargetId.androidBackgroundCapture,
          section: SettingsSectionId.clipboard,
          label: context.l10n.settingsAndroidCaptureTitle,
          description: context.l10n.settingsAndroidCaptureSearchDescription,
          keywords: context.l10n.settingsAndroidCaptureKeywords,
          targetKey: _androidCaptureKey,
        ),
      if (widget.controller.screenshotCaptureSupported)
        SettingsSearchTarget(
          id: _SettingsTargetId.screenshotCapture,
          section: SettingsSectionId.clipboard,
          label: context.l10n.settingsSaveScreenshotsTitle,
          description: context.l10n.settingsSaveScreenshotsSearchDescription,
          keywords: context.l10n.settingsSaveScreenshotsKeywords,
          targetKey: _screenshotCaptureKey,
        ),
      SettingsSearchTarget(
        id: _SettingsTargetId.clipboardCapture,
        section: SettingsSectionId.clipboard,
        label: context.l10n.settingsClipboardCaptureTitle,
        description: context.l10n.settingsClipboardCaptureSearchDescription,
        targetKey: _clipboardCaptureKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.excludedApplications,
        section: SettingsSectionId.privacy,
        label: context.l10n.settingsExcludedAppsTitle,
        description: context.l10n.settingsExcludedAppsSearchDescription,
        keywords: context.l10n.settingsExcludedAppsKeywords,
        targetKey: _excludedApplicationsKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.retention,
        section: SettingsSectionId.clipboard,
        label: context.l10n.settingsRetentionTitle,
        description: context.l10n.settingsRetentionDescription,
        targetKey: _retentionKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.storageQuota,
        section: SettingsSectionId.clipboard,
        label: context.l10n.settingsStorageQuotaTitle,
        description: context.l10n.settingsStorageQuotaDescription,
        keywords: context.l10n.settingsStorageQuotaKeywords,
        targetKey: _storageQuotaKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.historyFiles,
        section: SettingsSectionId.data,
        label: context.l10n.settingsHistoryFilesTitle,
        description: context.l10n.settingsHistoryFilesSearchDescription,
        keywords: context.l10n.settingsHistoryFilesKeywords,
        targetKey: _historyFilesKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.maxFileSize,
        section: SettingsSectionId.clipboard,
        label: context.l10n.settingsMaxFileSizeTitle,
        description: context.l10n.settingsMaxFileSizeSearchDescription,
        keywords: context.l10n.settingsMaxFileSizeKeywords,
        targetKey: _maxFileSizeKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.sync,
        section: SettingsSectionId.sync,
        label: context.l10n.settingsSyncSwitchTitle,
        description: context.l10n.settingsSyncSearchDescription,
        targetKey: _syncEnabledKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.instantClipboard,
        section: SettingsSectionId.sync,
        label: context.l10n.settingsInstantClipboardTitle,
        description: context.l10n.settingsInstantClipboardSearchDescription,
        keywords: context.l10n.settingsInstantClipboardKeywords,
        targetKey: _instantClipboardKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.lanVisibility,
        section: SettingsSectionId.sync,
        label: context.l10n.settingsLanVisibilityTitle,
        description: context.l10n.settingsLanVisibilityDescription,
        keywords: context.l10n.settingsLanVisibilityKeywords,
        targetKey: _lanVisibilityKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.notificationOnCopy,
        section: SettingsSectionId.notifications,
        label: context.l10n.settingsNotifyOnCopyTitle,
        description: context.l10n.settingsNotifyOnCopyDescription,
        targetKey: _notificationOnCopyKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.notificationPreview,
        section: SettingsSectionId.notifications,
        label: context.l10n.settingsNotificationPreviewTitle,
        description: context.l10n.settingsNotificationPreviewDescription,
        keywords: context.l10n.settingsNotificationPreviewKeywords,
        targetKey: _notificationPreviewKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.skipSecret,
        section: SettingsSectionId.privacy,
        label: context.l10n.settingsSkipSecretTitle,
        description: context.l10n.settingsSkipSecretDescription,
        keywords: context.l10n.settingsSkipSecretKeywords,
        targetKey: _skipSecretKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.skipTransient,
        section: SettingsSectionId.privacy,
        label: context.l10n.settingsSkipTransientTitle,
        description: context.l10n.settingsSkipTransientDescription,
        keywords: context.l10n.settingsSkipTransientKeywords,
        targetKey: _skipTransientKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.blockScreenshots,
        section: SettingsSectionId.privacy,
        label: context.l10n.settingsBlockScreenshotsTitle,
        description: _screenshotProtectionDescription,
        keywords: context.l10n.settingsBlockScreenshotsKeywords,
        targetKey: _blockScreenshotsKey,
      ),
      SettingsSearchTarget(
        id: _SettingsTargetId.soundOnCopy,
        section: SettingsSectionId.notifications,
        label: context.l10n.settingsSoundOnCopyTitle,
        description: context.l10n.settingsSoundOnCopyDescription,
        targetKey: _soundOnCopyKey,
      ),
      if (widget.appUpdate != null)
        SettingsSearchTarget(
          id: _SettingsTargetId.applicationUpdates,
          section: SettingsSectionId.about,
          label: context.l10n.settingsUpdatesTitle,
          description: context.l10n.settingsUpdatesSearchDescription,
          keywords: context.l10n.settingsUpdatesKeywords,
          targetKey: _applicationUpdatesKey,
        ),
      if (widget.quickPaste != null) ...[
        SettingsSearchTarget(
          id: _SettingsTargetId.quickPasteShortcut,
          section: SettingsSectionId.quickPaste,
          label: context.l10n.settingsQuickPasteShortcutTitle,
          description: context.l10n.settingsQuickPasteShortcutSearchDescription,
          keywords: context.l10n.settingsQuickPasteShortcutKeywords,
          targetKey: _quickPasteShortcutKey,
        ),
        SettingsSearchTarget(
          id: _SettingsTargetId.quickPasteAutoPaste,
          section: SettingsSectionId.quickPaste,
          label: context.l10n.settingsAutoPasteTitle,
          description: context.l10n.settingsAutoPasteSearchDescription,
          keywords: context.l10n.settingsAutoPasteKeywords,
          targetKey: _quickPasteAutoPasteKey,
        ),
      ],
      for (final contribution in widget.featureSections)
        if (_sectionAvailable(contribution.section))
          ...contribution.searchTargets(context),
    ];
  }

  GlobalKey _sectionKey(SettingsSectionId section) => switch (section) {
    SettingsSectionId.general => _generalSectionKey,
    SettingsSectionId.clipboard => _clipboardSectionKey,
    SettingsSectionId.modules => _modulesSectionKey,
    SettingsSectionId.privacy => _privacySectionKey,
    SettingsSectionId.data => _dataSectionKey,
    SettingsSectionId.sync => _syncSectionKey,
    SettingsSectionId.notifications => _notificationsSectionKey,
    SettingsSectionId.quickPaste => _quickPasteSectionKey,
    SettingsSectionId.about => _aboutSectionKey,
    SettingsSectionId.rules ||
    SettingsSectionId.quickAccess ||
    SettingsSectionId.snippets ||
    SettingsSectionId.transforms ||
    SettingsSectionId.aiTools => _featureSectionKeys.putIfAbsent(
      section,
      GlobalKey.new,
    ),
  };

  Key? _selectedNavigationKey(
    List<SettingsSearchTarget> targets,
    SettingsSectionId selectedSection,
  ) {
    final selected = _selectedTarget(targets, selectedSection);
    return selected == null ? null : ValueKey<String>(selected.widgetKey);
  }

  SettingsSearchTarget? _selectedTarget(
    List<SettingsSearchTarget> targets,
    SettingsSectionId selectedSection,
  ) {
    for (final target in targets) {
      if (_searchQuery.isEmpty && target.section == selectedSection) {
        return target;
      }
      if (_searchQuery.isNotEmpty && target.id == _selectedTargetId) {
        return target;
      }
    }
    return null;
  }

  void _selectNavigationTarget(Key? key, List<SettingsSearchTarget> targets) {
    for (final target in targets) {
      if (key == ValueKey<String>(target.widgetKey)) {
        _activateTarget(target);
        return;
      }
    }
  }

  void _activateTarget(SettingsSearchTarget target) {
    _searchFocus.unfocus();
    final highlight = target.isSearchResult;
    _highlightTimer?.cancel();
    setState(() {
      widget.controller.navigation.select(
        target.section,
        targetId: highlight ? target.id : null,
      );
      _highlightedTargetId = highlight ? target.id : null;
    });
    if (!highlight) return;
    _highlightTimer = Timer(AppMotion.settingsHighlightHold, () {
      if (!mounted || _highlightedTargetId != target.id) return;
      setState(() => _highlightedTargetId = null);
      _detailRevision.value++;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(_revealSearchTarget(target));
    });
  }

  Future<void> _revealSearchTarget(SettingsSearchTarget target) async {
    await _scrollTo(target.targetKey);
  }

  bool _isHighlighted(String targetId) => _highlightedTargetId == targetId;

  void _updateSearch(String value) {
    setState(() {
      widget.controller.navigation.search(value);
    });
  }

  bool get _isMacOS => defaultTargetPlatform == TargetPlatform.macOS;
  String get _startupLabel => defaultTargetPlatform == TargetPlatform.android
      ? context.l10n.settingsStartOnBootTitle
      : context.l10n.settingsStartAtLoginTitle;
  String get _taskbarVisibilityLabel => _isMacOS
      ? context.l10n.settingsShowInDock
      : context.l10n.settingsShowInTaskbar;
  String get _trayVisibilityLabel => _isMacOS
      ? context.l10n.settingsShowInMenuBar
      : context.l10n.settingsShowInSystemTray;

  String _languageLabel(BuildContext context, AppLanguage language) =>
      language == AppLanguage.system
      ? context.l10n.settingsLanguageSystem
      : language.nativeName;

  Widget _generalSection({required bool showHeading}) {
    final controller = widget.desktopVisibility;
    final preferences = controller?.preferences;
    return SettingsSection(
      key: _generalSectionKey,
      title: context.l10n.settingsSectionGeneral,
      showHeading: showHeading,
      description: context.l10n.settingsGeneralDescription,
      children: [
        if (widget.language case final language?)
          ListenableBuilder(
            listenable: language,
            builder: (context, _) => SettingRow(
              key: _languageKey,
              highlighted: _isHighlighted(_SettingsTargetId.language),
              title: context.l10n.settingsLanguageTitle,
              description: context.l10n.settingsLanguageDescription,
              trailing: Select<AppLanguage>(
                key: const ValueKey('settings-language'),
                value: language.language,
                onChanged: (next) {
                  if (next != null) unawaited(language.select(next));
                },
                popup: SelectPopup<AppLanguage>(
                  items: SelectItemList(
                    children: [
                      for (final option in AppLanguage.values)
                        SelectItemButton<AppLanguage>(
                          value: option,
                          child: Text(_languageLabel(context, option)),
                        ),
                    ],
                  ),
                ).call,
                itemBuilder: (context, selected) =>
                    Text(_languageLabel(context, selected)),
              ),
            ),
          ),
        if (widget.launchAtLogin case final startup?)
          SettingRow(
            key: _launchAtLoginKey,
            highlighted: _isHighlighted('result-start-at-login'),
            title: _startupLabel,
            description:
                startup.error ??
                switch (startup.status) {
                  LaunchAtLoginStatus.requiresApproval =>
                    defaultTargetPlatform == TargetPlatform.windows
                        ? context.l10n.settingsStartupWindowsApproval
                        : context.l10n.settingsStartupMacApproval,
                  LaunchAtLoginStatus.unavailable =>
                    context.l10n.settingsStartupUnavailable,
                  _ =>
                    defaultTargetPlatform == TargetPlatform.android
                        ? context.l10n.settingsStartupAndroidDescription
                        : context.l10n.settingsStartupDesktopDescription,
                },
            trailing: Wrap(
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: AppSpacing.sm,
              children: [
                if (startup.status == LaunchAtLoginStatus.requiresApproval)
                  Button.secondary(
                    onPressed: startup.openSystemSettings,
                    child: Text(context.l10n.commonOpenSettings),
                  ),
                SettingSwitch(
                  label: _startupLabel,
                  toggle: Switch(
                    key: const ValueKey('settings-start-at-login'),
                    value: startup.enabled,
                    enabled: startup.status != LaunchAtLoginStatus.unavailable,
                    onChanged:
                        startup.busy ||
                            startup.status == LaunchAtLoginStatus.unavailable
                        ? null
                        : startup.setEnabled,
                  ),
                ),
              ],
            ),
          ),
        if (controller?.error case final message?)
          Alert.destructive(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(context.l10n.settingsVisibilityErrorTitle),
            content: Text(message),
          ),
        if (controller != null && preferences != null) ...[
          SettingRow(
            key: _taskbarVisibilityKey,
            highlighted: _isHighlighted('result-taskbar-visibility'),
            title: _taskbarVisibilityLabel,
            description: context.l10n.settingsTaskbarVisibilityDescription,
            trailing: Switch(
              key: const ValueKey('settings-show-in-taskbar'),
              value: preferences.showInTaskbar,
              onChanged: controller.busy
                  ? null
                  : (value) => unawaited(
                      _setEntryPointVisible(taskbar: true, visible: value),
                    ),
            ),
          ),
          SettingRow(
            key: _trayVisibilityKey,
            highlighted: _isHighlighted('result-tray-visibility'),
            title: _trayVisibilityLabel,
            description: context.l10n.settingsTrayVisibilityDescription,
            trailing: Switch(
              key: const ValueKey('settings-show-in-tray'),
              value: preferences.showInTray,
              onChanged: controller.busy
                  ? null
                  : (value) => unawaited(
                      _setEntryPointVisible(taskbar: false, visible: value),
                    ),
            ),
          ),
        ],
      ],
    );
  }

  Widget _clipboardSection(
    RuntimeSettings settings, {
    required bool showHeading,
  }) {
    final controller = widget.controller;
    final capture = controller.capture;
    final l10n = context.l10n;
    return SettingsSection(
      showHeading: showHeading,
      key: _clipboardSectionKey,
      title: context.l10n.settingsSectionClipboard,
      description: context.l10n.settingsClipboardDescription,
      children: [
        if (widget.onOpenAndroidCaptureSetup != null) ...[
          SettingRow(
            key: _androidCaptureKey,
            highlighted: _isHighlighted(
              _SettingsTargetId.androidBackgroundCapture,
            ),
            title: context.l10n.settingsAndroidCaptureTitle,
            description: context.l10n.settingsAndroidCaptureDescription,
            trailing: Button.secondary(
              style: AppTheme.actionButtonStyle(const ButtonStyle.secondary()),
              onPressed: controller.canStart(SettingsOperation.capture)
                  ? widget.onOpenAndroidCaptureSetup
                  : null,
              leading: const AppIconFrame(child: Icon(LucideIcons.shieldCheck)),
              child: Text(context.l10n.settingsOpenSetup),
            ),
          ),
        ],
        if (widget.controller.screenshotCaptureSupported) ...[
          SettingRow(
            key: _screenshotCaptureKey,
            highlighted: _isHighlighted(_SettingsTargetId.screenshotCapture),
            title: context.l10n.settingsSaveScreenshotsTitle,
            description: context.l10n.settingsSaveScreenshotsDescription,
            trailing: Switch(
              key: const ValueKey<String>('save-screenshots-switch'),
              value: controller.screenshotCapture.enabled,
              onChanged:
                  controller.canStart(SettingsOperation.screenshotCapture)
                  ? controller.setScreenshotCaptureEnabled
                  : null,
            ),
          ),
          if (widget.controller.screenshotCapture.enabled &&
              widget.controller.screenshotCapture.needsPermission)
            SettingRow(
              title: context.l10n.settingsScreenshotAccessTitle,
              description: context.l10n.settingsScreenshotAccessDescription,
              trailing: Button.secondary(
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.secondary(),
                ),
                key: const ValueKey<String>('screenshot-capture-permission'),
                onPressed:
                    controller.canStart(SettingsOperation.screenshotCapture)
                    ? controller.requestScreenshotCapturePermission
                    : null,
                child: Text(context.l10n.settingsAllowAccess),
              ),
            ),
          if (widget.controller.screenshotCapture.enabled &&
              !widget.controller.screenshotCapture.needsPermission &&
              !widget.controller.screenshotCapture.running)
            StateView.error(
              title: context.l10n.settingsScreenshotCaptureStoppedTitle,
              message: context.l10n.settingsScreenshotCaptureStoppedMessage,
              actionLabel:
                  controller.canStart(SettingsOperation.screenshotCapture)
                  ? context.l10n.settingsResumeCapture
                  : null,
              onAction: controller.canStart(SettingsOperation.screenshotCapture)
                  ? () => controller.setScreenshotCaptureEnabled(true)
                  : null,
            ),
        ],
        SettingRow(
          key: _clipboardCaptureKey,
          highlighted: _isHighlighted(_SettingsTargetId.clipboardCapture),
          title: context.l10n.settingsClipboardCaptureTitle,
          description:
              capture?.label ?? context.l10n.settingsCaptureStateUnavailable,
          trailing: SettingSwitch(
            label: context.l10n.settingsClipboardCaptureTitle,
            showState: false,
            toggle: Switch(
              value: !(capture?.paused ?? true),
              enabled: capture != null,
              onChanged:
                  capture == null ||
                      !controller.canStart(SettingsOperation.capture)
                  ? null
                  : (enabled) => controller.setCapturePaused(!enabled),
            ),
          ),
        ),
        if (capture != null && !capture.running && !capture.paused) ...[
          Alert(
            leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
            title: Text(context.l10n.settingsCaptureStoppedTitle),
            content: Text(context.l10n.settingsCaptureStoppedMessage),
          ),
        ],
        SettingRow(
          key: _retentionKey,
          highlighted: _isHighlighted(_SettingsTargetId.retention),
          title: l10n.settingsRetentionTitle,
          description: l10n.settingsRetentionDescription,
          scale: (header) => StopScale(
            key: const ValueKey<String>('settings-retention-scale'),
            header: header,
            label: l10n.settingsRetentionTitle,
            stops: SettingsLimits.retentionStops(settings.retentionDays),
            value: settings.retentionDays,
            stopLabel: (days) => SettingsLimits.retentionLabel(l10n, days),
            pending: controller.isPending(SettingsOperation.retention),
            onCommit: _canChange(SettingsOperation.retention)
                ? _changeRetention
                : null,
          ),
        ),
        SettingRow(
          key: _storageQuotaKey,
          highlighted: _isHighlighted(_SettingsTargetId.storageQuota),
          title: l10n.settingsStorageQuotaTitle,
          description: l10n.settingsStorageQuotaDescription,
          status: controller.presentationActive
              ? ValueListenableBuilder<StorageUsage?>(
                  valueListenable: controller.storageUsage,
                  builder: (context, usage, _) => _storageUsage(usage),
                )
              : _storageUsage(controller.storageUsage.value),
          scale: (header) => StopScale(
            key: const ValueKey<String>('settings-storage-quota-scale'),
            header: header,
            label: l10n.settingsStorageQuotaTitle,
            stops: SettingsLimits.sizeStops(
              SettingsLimits.storageQuotaPresets,
              settings.storageQuotaBytes,
            ),
            value: settings.storageQuotaBytes,
            stopLabel: (bytes) => SettingsLimits.limitLabel(l10n, bytes),
            pending: controller.isPending(SettingsOperation.storageQuota),
            onCommit: _canChange(SettingsOperation.storageQuota)
                ? _changeStorageQuota
                : null,
          ),
        ),
        SettingRow(
          key: _maxFileSizeKey,
          highlighted: _isHighlighted(_SettingsTargetId.maxFileSize),
          title: l10n.settingsMaxFileSizeTitle,
          description: l10n.settingsMaxFileSizeDescription,
          scale: (header) => StopScale(
            key: const ValueKey<String>('settings-max-file-size-scale'),
            header: header,
            label: l10n.settingsMaxFileSizeTitle,
            stops: SettingsLimits.sizeStops(
              SettingsLimits.maxFileSizePresets,
              settings.maxFileSizeBytes,
            ),
            value: settings.maxFileSizeBytes,
            stopLabel: (bytes) => SettingsLimits.limitLabel(l10n, bytes),
            pending: controller.isPending(SettingsOperation.maxFileSize),
            onCommit: _canChange(SettingsOperation.maxFileSize)
                ? (bytes) => controller.setMaxFileSizeBytes(bytes)
                : null,
          ),
        ),
      ],
    );
  }

  Widget _dataSection({required bool showHeading}) {
    return SettingsSection(
      showHeading: showHeading,
      key: _dataSectionKey,
      title: context.l10n.settingsSectionData,
      description: context.l10n.settingsDataDescription,
      children: [
        Column(
          key: _historyFilesKey,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            SettingRow(
              highlighted: _isHighlighted(_SettingsTargetId.historyFiles),
              title: context.l10n.settingsExportTextTitle,
              description: context.l10n.settingsExportTextDescription,
              trailing: _transferAction(
                operation: SettingsOperation.exportText,
                kind: DataTransferKind.textExport,
                icon: LucideIcons.fileOutput,
                label: context.l10n.settingsExportAction,
                name: context.l10n.settingsExportTextTitle,
                onPressed: _exportTextHistory,
              ),
            ),
            const Divider(),
            SettingRow(
              highlighted: _isHighlighted(_SettingsTargetId.historyFiles),
              title: context.l10n.settingsBackupTitle,
              description: context.l10n.settingsBackupDescription,
              trailing: _transferAction(
                operation: SettingsOperation.backup,
                kind: DataTransferKind.backup,
                icon: LucideIcons.archive,
                label: context.l10n.settingsCreateBackupAction,
                name: context.l10n.settingsBackupTitle,
                onPressed: _createBackup,
              ),
            ),
            const Divider(),
            SettingRow(
              highlighted: _isHighlighted(_SettingsTargetId.historyFiles),
              title: context.l10n.settingsRestoreTitle,
              description: context.l10n.settingsRestoreDescription,
              trailing: Button.destructive(
                style: AppTheme.actionButtonStyle(
                  const ButtonStyle.destructive(),
                ),
                onPressed: widget.controller.canStart(SettingsOperation.restore)
                    ? _confirmRestore
                    : null,
                leading: _actionIcon(
                  SettingsOperation.restore,
                  LucideIcons.clipboardList,
                ),
                child: Text(context.l10n.settingsRestoreAction),
              ),
            ),
          ],
        ),
      ],
    );
  }

  Widget _syncSection(RuntimeSettings settings, {required bool showHeading}) {
    return SettingsSection(
      showHeading: showHeading,
      key: _syncSectionKey,
      title: context.l10n.settingsSectionSync,
      description: context.l10n.settingsSyncDescription,
      children: [
        SettingRow(
          key: _syncEnabledKey,
          highlighted: _isHighlighted(_SettingsTargetId.sync),
          title: context.l10n.settingsSyncSwitchTitle,
          description: context.l10n.settingsSyncSwitchDescription,
          trailing: Switch(
            value: settings.syncEnabled,
            onChanged: widget.controller.canStart(SettingsOperation.syncEnabled)
                ? widget.controller.setSyncEnabled
                : null,
          ),
        ),
        SettingRow(
          key: _instantClipboardKey,
          highlighted: _isHighlighted(_SettingsTargetId.instantClipboard),
          title: context.l10n.settingsInstantClipboardTitle,
          description: context.l10n.settingsInstantClipboardDescription,
          trailing: Switch(
            key: const ValueKey('instant-clipboard-switch'),
            value: settings.instantClipboard,
            onChanged:
                widget.controller.canStart(SettingsOperation.instantClipboard)
                ? widget.controller.setInstantClipboard
                : null,
          ),
        ),
        SettingRow(
          key: _lanVisibilityKey,
          highlighted: _isHighlighted(_SettingsTargetId.lanVisibility),
          title: context.l10n.settingsLanVisibilityTitle,
          description: context.l10n.settingsLanVisibilityDescription,
          trailing: Switch(
            value: settings.lanVisibility,
            onChanged:
                widget.controller.canStart(SettingsOperation.lanVisibility)
                ? widget.controller.setLanVisibility
                : null,
          ),
        ),
      ],
    );
  }

  Widget _privacySection(
    RuntimeSettings settings, {
    required bool showHeading,
  }) {
    return SettingsSection(
      showHeading: showHeading,
      key: _privacySectionKey,
      title: context.l10n.settingsSectionPrivacy,
      description: context.l10n.settingsPrivacyDescription,
      children: [
        OutlinedContainer(
          key: _excludedApplicationsKey,
          duration: AppMotion.resolve(context, AppMotion.quick),
          theme: AppTheme.settingsRowTheme(
            context,
            highlighted: _isHighlighted(_SettingsTargetId.excludedApplications),
          ),
          clipBehavior: Clip.none,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(context.l10n.settingsExcludedAppsTitle).medium(),
                        const Gap(AppSpacing.xs),
                        Text(switch (defaultTargetPlatform) {
                          TargetPlatform.macOS ||
                          TargetPlatform.windows ||
                          TargetPlatform.linux =>
                            context.l10n.settingsExcludedAppsDesktopDescription,
                          TargetPlatform.android =>
                            context.l10n.settingsExcludedAppsAndroidDescription,
                          TargetPlatform.iOS || TargetPlatform.fuchsia =>
                            context
                                .l10n
                                .settingsExcludedAppsUnsupportedDescription,
                        }).muted().textSmall(),
                      ],
                    ),
                  ),
                  const Gap(AppSpacing.md),
                  Button.secondary(
                    onPressed:
                        widget.controller.canStart(
                          SettingsOperation.excludedApps,
                        )
                        ? _addExcludedApplication
                        : null,
                    leading: const AppIconFrame(child: Icon(LucideIcons.plus)),
                    child: Text(context.l10n.settingsExcludedAppsAdd),
                  ),
                ],
              ),
              if (settings.excludedAppIds.isEmpty) ...[
                const Gap(AppSpacing.lg),
                Text(
                  context.l10n.settingsExcludedAppsEmpty,
                ).muted().textSmall(),
              ] else ...[
                const Gap(AppSpacing.md),
                for (final appId in settings.excludedAppIds)
                  Padding(
                    padding: const EdgeInsets.only(top: AppSpacing.xs),
                    child: ApplicationIdentity(
                      key: ValueKey('excluded-app-$appId'),
                      id: appId,
                      directory: _applications,
                      builder: (context, app, label) => Row(
                        children: [
                          Expanded(child: label.small()),
                          Tooltip(
                            showDuration: AppMotion.resolve(
                              context,
                              AppMotion.standard,
                            ),
                            tooltip: (context) => TooltipContainer(
                              child: Text(
                                context.l10n.settingsExcludedAppsRemove(
                                  app.name,
                                ),
                              ),
                            ),
                            child: Button.ghost(
                              style: AppTheme.controlButtonStyle(
                                const ButtonStyle.ghostIcon(),
                              ),
                              onPressed:
                                  widget.controller.canStart(
                                    SettingsOperation.excludedApps,
                                  )
                                  ? () => widget.controller.removeExcludedApp(
                                      appId,
                                    )
                                  : null,
                              child: const AppIconFrame(
                                child: Icon(LucideIcons.x),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
              ],
            ],
          ),
        ),
        SettingRow(
          key: _skipSecretKey,
          highlighted: _isHighlighted(_SettingsTargetId.skipSecret),
          title: context.l10n.settingsSkipSecretTitle,
          description: context.l10n.settingsSkipSecretDescription,
          trailing: Switch(
            key: const ValueKey('skip-secret-switch'),
            value: settings.skipSecret,
            onChanged: widget.controller.canStart(SettingsOperation.skipSecret)
                ? widget.controller.setSkipSecret
                : null,
          ),
        ),
        SettingRow(
          key: _skipTransientKey,
          highlighted: _isHighlighted(_SettingsTargetId.skipTransient),
          title: context.l10n.settingsSkipTransientTitle,
          description: context.l10n.settingsSkipTransientDescription,
          trailing: Switch(
            key: const ValueKey('skip-transient-switch'),
            value: settings.skipTransient,
            onChanged:
                widget.controller.canStart(SettingsOperation.skipTransient)
                ? widget.controller.setSkipTransient
                : null,
          ),
        ),
        SettingRow(
          key: _blockScreenshotsKey,
          highlighted: _isHighlighted(_SettingsTargetId.blockScreenshots),
          title: context.l10n.settingsBlockScreenshotsTitle,
          description: _screenshotProtectionDescription,
          trailing: Switch(
            key: const ValueKey('block-screenshots-switch'),
            value: widget.controller.blockScreenshots,
            enabled: widget.controller.blockScreenshotsSupported,
            onChanged:
                widget.controller.blockScreenshotsSupported &&
                    widget.controller.canStart(
                      SettingsOperation.blockScreenshots,
                    )
                ? widget.controller.setBlockScreenshots
                : null,
          ),
        ),
      ],
    );
  }

  Widget _notificationsSection(
    RuntimeSettings settings, {
    required bool showHeading,
  }) {
    return SettingsSection(
      showHeading: showHeading,
      key: _notificationsSectionKey,
      title: context.l10n.settingsSectionNotifications,
      description: context.l10n.settingsNotificationsDescription,
      children: [
        SettingRow(
          key: _notificationOnCopyKey,
          highlighted: _isHighlighted(_SettingsTargetId.notificationOnCopy),
          title: context.l10n.settingsNotifyOnCopyTitle,
          description: context.l10n.settingsNotifyOnCopyDescription,
          trailing: Switch(
            value: settings.notifyOnCopy,
            onChanged:
                widget.controller.canStart(SettingsOperation.notifyOnCopy)
                ? widget.controller.setNotifyOnCopy
                : null,
          ),
        ),
        SettingRow(
          key: _notificationPreviewKey,
          highlighted: _isHighlighted(_SettingsTargetId.notificationPreview),
          title: context.l10n.settingsNotificationPreviewTitle,
          description: context.l10n.settingsNotificationPreviewDescription,
          trailing: Switch(
            value: settings.notificationPreview,
            enabled: settings.notifyOnCopy,
            onChanged:
                settings.notifyOnCopy &&
                    widget.controller.canStart(
                      SettingsOperation.notificationPreview,
                    )
                ? widget.controller.setNotificationPreview
                : null,
          ),
        ),
        SettingRow(
          key: _soundOnCopyKey,
          highlighted: _isHighlighted(_SettingsTargetId.soundOnCopy),
          title: context.l10n.settingsSoundOnCopyTitle,
          description: context.l10n.settingsSoundOnCopyDescription,
          trailing: Switch(
            value: settings.soundOnCopy,
            onChanged: widget.controller.canStart(SettingsOperation.soundOnCopy)
                ? widget.controller.setSoundOnCopy
                : null,
          ),
        ),
      ],
    );
  }

  Widget _aboutSection({required bool showHeading}) {
    return SettingsSection(
      showHeading: showHeading,
      key: _aboutSectionKey,
      title: context.l10n.settingsSectionAbout,
      description: context.l10n.settingsAboutDescription,
      children: [
        SettingRow(
          title: context.l10n.settingsVersionTitle,
          description: context.l10n.settingsVersionDescription,
          trailing: Text(
            widget.appUpdate?.currentVersion?.toString() ??
                context.l10n.settingsVersionUnavailable,
          ),
        ),
        if (widget.appUpdate case final controller?) ...[
          const Gap(AppSpacing.md),
          AnimatedBuilder(
            animation: controller,
            builder: (context, _) => SettingRow(
              key: _applicationUpdatesKey,
              highlighted: _isHighlighted(_SettingsTargetId.applicationUpdates),
              title: context.l10n.settingsUpdatesTitle,
              description: _updateDescription(controller),
              trailing: _updateAction(controller),
            ),
          ),
        ],
      ],
    );
  }

  String _updateDescription(AppUpdateController controller) {
    final current = controller.currentVersion?.toString();
    final available = controller.release?.version.toString();
    final l10n = context.l10n;
    return switch (controller.phase) {
      AppUpdatePhase.idle => l10n.settingsUpdateIdle,
      AppUpdatePhase.checking => l10n.settingsUpdateChecking,
      AppUpdatePhase.upToDate =>
        current == null
            ? l10n.settingsUpdateUpToDate
            : l10n.settingsUpdateVersionUpToDate(current),
      AppUpdatePhase.available => l10n.settingsUpdateAvailable(
        '$available',
        '$current',
      ),
      AppUpdatePhase.downloading => l10n.settingsUpdateDownloading(
        '$available',
        (controller.downloadProgress * 100).round(),
      ),
      AppUpdatePhase.preparingModules => l10n.settingsUpdatePreparingModules,
      AppUpdatePhase.installing =>
        controller.message ?? l10n.settingsUpdateInstalling('$available'),
      AppUpdatePhase.permissionRequired ||
      AppUpdatePhase.restartRequired ||
      AppUpdatePhase.unavailable ||
      AppUpdatePhase.error =>
        controller.message ?? l10n.settingsUpdateNeedsAttention,
    };
  }

  Widget _updateAction(AppUpdateController controller) {
    return switch (controller.phase) {
      AppUpdatePhase.available => Button.primary(
        key: const ValueKey<String>('install-app-update'),
        onPressed: controller.install,
        leading: const AppIconFrame(child: Icon(LucideIcons.download)),
        child: Text(context.l10n.settingsUpdateNowAction),
      ),
      AppUpdatePhase.permissionRequired => Button.primary(
        key: const ValueKey<String>('continue-app-update'),
        onPressed: controller.install,
        leading: const AppIconFrame(child: Icon(LucideIcons.settings)),
        child: Text(context.l10n.commonContinue),
      ),
      AppUpdatePhase.unavailable => Button.secondary(
        key: const ValueKey<String>('open-update-release'),
        onPressed: controller.openReleasePage,
        leading: const AppIconFrame(child: Icon(LucideIcons.externalLink)),
        child: Text(context.l10n.settingsUpdateOpenReleaseAction),
      ),
      AppUpdatePhase.checking => Button.secondary(
        onPressed: null,
        leading: const AppIconFrame(child: Icon(LucideIcons.refreshCw)),
        child: Text(context.l10n.settingsUpdateCheckingAction),
      ),
      AppUpdatePhase.downloading => Button.secondary(
        onPressed: null,
        leading: const AppIconFrame(child: Icon(LucideIcons.download)),
        child: Text(context.l10n.settingsUpdateDownloadingAction),
      ),
      AppUpdatePhase.preparingModules ||
      AppUpdatePhase.installing => Button.secondary(
        onPressed: null,
        leading: const AppIconFrame(child: Icon(LucideIcons.loaderCircle)),
        child: Text(context.l10n.settingsUpdateInstallingAction),
      ),
      AppUpdatePhase.restartRequired => Button.primary(
        key: const ValueKey<String>('restart-after-app-update'),
        onPressed: controller.busy || !controller.canRestart
            ? null
            : controller.restartApplication,
        leading: const AppIconFrame(child: Icon(LucideIcons.refreshCw)),
        child: Text(context.l10n.settingsUpdateRestartAction),
      ),
      AppUpdatePhase.idle ||
      AppUpdatePhase.upToDate ||
      AppUpdatePhase.error => Button.secondary(
        key: const ValueKey<String>('check-app-update'),
        onPressed: () => _checkForUpdates(controller),
        leading: const AppIconFrame(child: Icon(LucideIcons.refreshCw)),
        child: Text(context.l10n.settingsUpdateCheckAgainAction),
      ),
    };
  }

  Future<void> _checkForUpdates(AppUpdateController controller) async {
    await controller.check();
    if (!mounted) return;

    final l10n = context.l10n;
    switch (controller.phase) {
      case AppUpdatePhase.upToDate:
        final version = controller.currentVersion?.toString();
        AppToast.show(
          context,
          title: l10n.settingsUpdateNoneTitle,
          message: version == null
              ? l10n.settingsUpdateLatest
              : l10n.settingsUpdateVersionLatest(version),
          tone: AppToastTone.success,
        );
        break;
      case AppUpdatePhase.available:
        final version = controller.release?.version.toString();
        AppToast.show(
          context,
          title: l10n.settingsUpdateAvailableTitle,
          message: version == null
              ? l10n.settingsUpdateReady
              : l10n.settingsUpdateVersionReady(version),
        );
        break;
      case AppUpdatePhase.unavailable:
        AppToast.show(
          context,
          title: l10n.settingsUpdateUnavailableTitle,
          message: controller.message ?? l10n.settingsUpdateUnavailableMessage,
          tone: AppToastTone.error,
        );
        break;
      case AppUpdatePhase.error:
        AppToast.show(
          context,
          title: l10n.settingsUpdateCheckFailedTitle,
          message: controller.message ?? l10n.settingsUpdateCheckFailedMessage,
          tone: AppToastTone.error,
        );
        break;
      case AppUpdatePhase.idle ||
          AppUpdatePhase.checking ||
          AppUpdatePhase.downloading ||
          AppUpdatePhase.preparingModules ||
          AppUpdatePhase.installing ||
          AppUpdatePhase.permissionRequired ||
          AppUpdatePhase.restartRequired:
        break;
    }
  }

  bool _canChange(SettingsOperation operation) =>
      widget.controller.canStart(operation) ||
      widget.controller.isPending(operation);

  Future<void> _changeRetention(int days) async {
    final l10n = context.l10n;
    final current = widget.controller.settings?.retentionDays;
    if (current == null) return;
    if (SettingsLimits.retentionTightens(from: current, to: days) &&
        !await showDestructiveConfirmation(
          context,
          icon: LucideIcons.trash2,
          title: l10n.settingsRetentionConfirmTitle,
          message: l10n.settingsRetentionConfirmMessage(
            SettingsLimits.retentionLabel(l10n, days),
          ),
          confirmLabel: l10n.settingsLimitConfirmAction,
        )) {
      return;
    }
    if (mounted) await widget.controller.setRetentionDays(days);
  }

  Future<void> _changeStorageQuota(int bytes) async {
    final l10n = context.l10n;
    final current = widget.controller.settings?.storageQuotaBytes;
    if (current == null) return;
    if (SettingsLimits.storageQuotaTightens(from: current, to: bytes) &&
        !await showDestructiveConfirmation(
          context,
          icon: LucideIcons.trash2,
          title: l10n.settingsStorageQuotaConfirmTitle,
          message: switch (widget.controller.storageUsage.value) {
            final usage? when usage.usedBytes > bytes =>
              l10n.settingsStorageQuotaConfirmRemovesMessage(
                SettingsLimits.limitLabel(l10n, bytes),
                formatBytes(usage.usedBytes),
              ),
            _ => l10n.settingsStorageQuotaConfirmMessage(
              SettingsLimits.limitLabel(l10n, bytes),
            ),
          },
          confirmLabel: l10n.settingsLimitConfirmAction,
        )) {
      return;
    }
    if (mounted) await widget.controller.setStorageQuotaBytes(bytes);
  }

  Future<void> _setEntryPointVisible({
    required bool taskbar,
    required bool visible,
  }) async {
    final controller = widget.desktopVisibility!;
    final preferences = controller.preferences;
    final otherVisible = taskbar
        ? preferences.showInTray
        : preferences.showInTaskbar;
    if (DesktopPresence.hidesLastEntryPoint(
      turningOn: visible,
      otherVisible: otherVisible,
    )) {
      final l10n = context.l10n;
      final quickPaste = widget.quickPaste;
      final confirmed = await showDestructiveConfirmation(
        context,
        icon: LucideIcons.eyeOff,
        title: l10n.settingsHideEverywhereTitle,
        message: DesktopPresence.hiddenEverywhereMessage(
          l10n,
          registeredShortcut: quickPaste?.registeredShortcut,
          registeredShortcutDescription:
              quickPaste?.registeredShortcutDescription,
        ),
        confirmLabel: l10n.settingsHideEverywhereAction,
      );
      if (!confirmed || !mounted) return;
    }
    await (taskbar
        ? controller.setTaskbarVisible(visible)
        : controller.setTrayVisible(visible));
  }

  Widget _actionIcon(SettingsOperation operation, IconData icon) =>
      widget.controller.isPending(operation)
      ? const SizedBox.square(
          dimension: AppIconSize.sm,
          child: AppIconFrame(
            size: AppIconSize.sm,
            child: CircularProgressIndicator(),
          ),
        )
      : AppIconFrame(child: Icon(icon));

  Widget _storageUsage(StorageUsage? usage) => usage == null
      ? const SizedBox.shrink()
      : Text(
          StorageUsageText.withOldest(
            context.l10n,
            usage,
            switch (usage.oldestUnpinnedAt) {
              final at? => formatSystemDate(context, at),
              null => null,
            },
          ),
          key: const ValueKey<String>('settings-storage-usage'),
        ).textSmall();

  /// A history file action that becomes its own progress indicator while it
  /// runs: same place and size, the share done in place of its label, or the
  /// label while the total is unknown.
  Widget _transferAction({
    required SettingsOperation operation,
    required DataTransferKind kind,
    required IconData icon,
    required String label,
    required String name,
    required VoidCallback onPressed,
  }) => ValueListenableBuilder<DataTransferProgress?>(
    valueListenable: widget.controller.dataTransfer,
    builder: (context, transfer, _) => _transferButton(
      operation: operation,
      fraction: transfer?.kind == kind ? transfer!.fraction : null,
      icon: icon,
      label: label,
      name: name,
      onPressed: onPressed,
    ),
  );

  Widget _transferButton({
    required SettingsOperation operation,
    required double? fraction,
    required IconData icon,
    required String label,
    required String name,
    required VoidCallback onPressed,
  }) {
    final controller = widget.controller;
    final percent = fraction == null
        ? null
        : NumberFormat.percentPattern(
            context.l10n.localeName,
          ).format((fraction * 100).floor() / 100);
    final button = Button.secondary(
      style: AppTheme.actionButtonStyle(const ButtonStyle.secondary()),
      onPressed: controller.canStart(operation) ? onPressed : null,
      leading: _actionIcon(operation, icon),
      child: percent == null
          ? Text(label)
          : Stack(
              alignment: Alignment.center,
              children: [
                Visibility.maintain(visible: false, child: Text(label)),
                Positioned.fill(
                  child: FittedBox(
                    fit: BoxFit.scaleDown,
                    child: Text(percent, maxLines: 1),
                  ),
                ),
              ],
            ),
    );
    if (!controller.isPending(operation)) return button;
    return Semantics(
      container: true,
      role: percent == null
          ? SemanticsRole.loadingSpinner
          : SemanticsRole.progressBar,
      label: name,
      value: percent,
      minValue: percent == null ? null : '0',
      maxValue: percent == null ? null : '100',
      child: ExcludeSemantics(
        child: FileOperationsButtonProgress(progress: fraction, child: button),
      ),
    );
  }

  Future<void> _scrollTo(GlobalKey key) async {
    final target = key.currentContext;
    if (target == null) return;
    await Scrollable.ensureVisible(
      target,
      duration: AppMotion.resolve(context, AppMotion.standard),
      curve: AppMotion.enterCurve,
      alignment: 0,
    );
  }

  Future<void> _addExcludedApplication() async {
    final app = await showApplicationPicker(
      context,
      catalog: widget.applications,
      exclude: widget.controller.settings?.excludedAppIds ?? const [],
    );
    if (app == null || !mounted) return;
    _applications.remember(app);
    await widget.controller.addExcludedApp(app.id);
  }

  Future<void> _confirmRestore() async {
    final l10n = context.l10n;
    final restored = await AppOverlays.showActionDialog(
      context,
      icon: LucideIcons.clipboardList,
      title: Text(l10n.settingsRestoreDialogTitle),
      content: Text(l10n.settingsRestoreDialogMessage),
      actionLabel: l10n.settingsRestoreAction,
      changes: widget.controller,
      canConfirm: () => widget.controller.canStart(SettingsOperation.restore),
      onConfirm: widget.controller.restoreBackup,
      failureMessage: () =>
          widget.controller.errorFor(SettingsOperation.restore) ??
          l10n.settingsRestoreFailed,
    );
    if (restored != true || !mounted) return;
    AppToast.show(
      context,
      title: l10n.settingsRestoreDoneTitle,
      message: l10n.settingsRestoreDoneMessage,
      tone: AppToastTone.success,
    );
  }

  Future<void> _exportTextHistory() async {
    final result = await widget.controller.exportTextHistory();
    if (!mounted || result == null) return;
    final l10n = context.l10n;
    AppToast.show(
      context,
      title: l10n.settingsExportDoneTitle,
      message: SettingsOutcomes.exported(l10n, result),
      tone: result.skippedUndecryptable == 0
          ? AppToastTone.success
          : AppToastTone.information,
    );
  }

  Future<void> _createBackup() async {
    final result = await widget.controller.createBackup();
    if (!mounted || result == null) return;
    final l10n = context.l10n;
    AppToast.show(
      context,
      title: l10n.settingsBackupDoneTitle,
      message: SettingsOutcomes.backedUp(l10n, result),
      tone: AppToastTone.success,
    );
  }
}

class _QuickPasteSection extends StatelessWidget {
  const _QuickPasteSection({
    super.key,
    required this.controller,
    required this.showHeading,
    required this.shortcutKey,
    required this.autoPasteKey,
    required this.shortcutHighlighted,
    required this.autoPasteHighlighted,
  });

  final QuickPasteSettingsController controller;
  final bool showHeading;
  final Key shortcutKey;
  final Key autoPasteKey;
  final bool shortcutHighlighted;
  final bool autoPasteHighlighted;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        return SettingsSection(
          showHeading: showHeading,
          title: 'Quick Paste',
          description: context.l10n.settingsQuickPasteDescription,
          children: [
            if (controller.linuxIntegration case final integration?)
              if (integration.session == LinuxDesktopSession.wayland &&
                  !integration.quickPaste)
                Alert(
                  leading: const AppIconFrame(
                    child: Icon(LucideIcons.keyboard),
                  ),
                  title: Text(context.l10n.settingsQuickPasteSetupTitle),
                  content: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(context.l10n.settingsQuickPasteSetupMessage),
                      const Gap(AppSpacing.sm),
                      Wrap(
                        spacing: AppSpacing.sm,
                        runSpacing: AppSpacing.sm,
                        children: [
                          if (integration.companion !=
                              LinuxCompanionState.active)
                            Button.secondary(
                              key: const ValueKey('linux-companion-setup'),
                              onPressed: controller.busy
                                  ? null
                                  : controller.openLinuxCompanionSetup,
                              child: Text(
                                context
                                    .l10n
                                    .settingsQuickPasteDesktopIntegration,
                              ),
                            ),
                          if (integration.remoteDesktop ==
                              LinuxRemoteDesktopState.consentRequired)
                            Button.secondary(
                              key: const ValueKey('linux-keyboard-permission'),
                              onPressed: controller.busy
                                  ? null
                                  : controller.requestLinuxRemoteDesktop,
                              child: Text(
                                context.l10n.settingsQuickPasteAllowKeyboard,
                              ),
                            ),
                          Button.ghost(
                            key: const ValueKey('linux-integration-refresh'),
                            onPressed: controller.busy
                                ? null
                                : controller.refreshLinuxIntegration,
                            child: Text(
                              context.l10n.settingsQuickPasteCheckAgain,
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
            if (!controller.initialized && controller.busy)
              StateView.loading(message: context.l10n.settingsQuickPasteLoading)
            else if (!controller.supported && !controller.startFailed)
              Alert(
                leading: const AppIconFrame(
                  child: Icon(LucideIcons.circleAlert),
                ),
                title: Text(context.l10n.settingsQuickPasteUnavailableTitle),
                content: Text(
                  context.l10n.settingsQuickPasteUnavailableMessage,
                ),
              )
            else ...[
              if (controller.supported) ...[
                SettingRow(
                  key: shortcutKey,
                  highlighted: shortcutHighlighted,
                  title: context.l10n.settingsQuickPasteShortcutTitle,
                  description: switch (controller
                      .registeredShortcutDescription) {
                    final shortcut? =>
                      context.l10n.settingsQuickPasteRegisteredShortcut(
                        shortcut,
                      ),
                    null => context.l10n.settingsQuickPasteShortcutDescription,
                  },
                  status: switch (controller.shortcutRefusal) {
                    final refusal? => _SettingWarning(
                      key: const ValueKey<String>(
                        'quick-paste-shortcut-refused',
                      ),
                      message: _refusalMessage(context, refusal),
                    ),
                    null => null,
                  },
                  trailing: ShortcutRecorder(
                    shortcut: controller.shortcut,
                    enabled: !controller.busy,
                    onBeginRecording: controller.beginShortcutRecording,
                    onCancelRecording: controller.cancelShortcutRecording,
                    onRecorded: controller.setShortcut,
                    onReset: controller.resetShortcut,
                  ),
                ),
                SettingRow(
                  key: autoPasteKey,
                  highlighted: autoPasteHighlighted,
                  title: context.l10n.settingsAutoPasteTitle,
                  description: context.l10n.settingsAutoPasteDescription,
                  trailing: Switch(
                    value: controller.autoPaste,
                    onChanged: controller.busy ? null : controller.setAutoPaste,
                  ),
                ),
                // A failed start never read the permission.
                if (Platform.isMacOS &&
                    controller.autoPaste &&
                    !controller.accessibilityGranted &&
                    !controller.startFailed)
                  Alert(
                    leading: const AppIconFrame(
                      child: Icon(LucideIcons.accessibility),
                    ),
                    title: Text(
                      context.l10n.settingsAccessibilityRequiredTitle,
                    ),
                    content: _messageWithAction(
                      context.l10n.settingsAccessibilityRequiredMessage,
                      Button.secondary(
                        onPressed: controller.busy
                            ? null
                            : controller.requestAccessibility,
                        child: Text(context.l10n.commonEnable),
                      ),
                    ),
                  ),
              ],
              if (controller.errorMessage case final message?)
                Alert.destructive(
                  key: const ValueKey<String>('quick-paste-error'),
                  leading: const AppIconFrame(
                    child: Icon(LucideIcons.circleAlert),
                  ),
                  title: Text(context.l10n.settingsQuickPasteAttentionTitle),
                  content: controller.startFailed
                      ? _messageWithAction(
                          message,
                          Button.secondary(
                            key: const ValueKey<String>('quick-paste-retry'),
                            onPressed: controller.busy
                                ? null
                                : controller.retry,
                            child: Text(context.l10n.commonTryAgain),
                          ),
                        )
                      : Text(message),
                ),
            ],
          ],
        );
      },
    );
  }

  static Widget _messageWithAction(String message, Widget action) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(message),
      const Gap(AppSpacing.sm),
      Align(alignment: Alignment.centerRight, child: action),
    ],
  );

  static String _refusalMessage(
    BuildContext context,
    DesktopShortcutRefusal refusal,
  ) => switch (refusal.reason) {
    DesktopShortcutRefusalReason.inUse =>
      context.l10n.settingsQuickPasteShortcutInUse(
        DesktopPresence.shortcutText(refusal.shortcut.displayKeys),
      ),
    DesktopShortcutRefusalReason.rejected =>
      context.l10n.settingsRuntimeShortcutRegisterFailed,
  };
}

/// A warning about a row's own setting, under its description.
class _SettingWarning extends StatelessWidget {
  const _SettingWarning({super.key, required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    final color = AppTheme.statusColor(context, AppStatusTone.warning);
    return Text.rich(
      TextSpan(
        children: [
          WidgetSpan(
            alignment: PlaceholderAlignment.middle,
            child: Padding(
              padding: const EdgeInsetsDirectional.only(end: AppSpacing.xs),
              child: AppIconFrame(
                size: AppIconSize.sm,
                child: Icon(
                  LucideIcons.triangleAlert,
                  size: AppIconSize.sm,
                  color: color,
                ),
              ),
            ),
          ),
          TextSpan(text: message),
        ],
      ),
      style: TextStyle(color: color),
    ).textSmall();
  }
}

abstract final class _SettingsTargetId {
  static const String language = 'result-language';
  static const String maxFileSize = 'result-max-file-size';
  static const String skipSecret = 'result-skip-secret';
  static const String skipTransient = 'result-skip-transient';
  static const String blockScreenshots = 'result-block-screenshots';
  static const String androidBackgroundCapture =
      'result-android-background-capture';
  static const String clipboardCapture = 'result-clipboard-capture';
  static const String screenshotCapture = 'result-screenshot-capture';
  static const String excludedApplications = 'result-excluded-applications';
  static const String retention = 'result-retention';
  static const String storageQuota = 'result-storage-quota';
  static const String historyFiles = 'result-history-files';
  static const String sync = 'result-sync';
  static const String instantClipboard = 'result-instant-clipboard';
  static const String lanVisibility = 'result-lan-visibility';
  static const String notificationOnCopy = 'result-notification-on-copy';
  static const String notificationPreview = 'result-notification-preview';
  static const String soundOnCopy = 'result-sound-on-copy';
  static const String applicationUpdates = 'result-application-updates';
  static const String quickPasteShortcut = 'result-quick-paste-shortcut';
  static const String quickPasteAutoPaste = 'result-quick-paste-auto-paste';
}
