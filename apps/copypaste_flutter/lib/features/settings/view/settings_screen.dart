import 'dart:async';
import 'dart:io';

import '../../modules/controller/modules_controller.dart';
import '../../modules/view/modules_settings_view.dart';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/navigation/app_page_route.dart';
import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../platform/desktop/global_shortcut.dart';
import '../../../platform/permissions/linux_integration.dart';
import '../../../shared/adaptive_breakpoints.dart';
import '../../../shared/state_view.dart';
import '../controller/quick_paste_settings_controller.dart';
import '../controller/settings_controller.dart';
import '../controller/settings_navigation_state.dart';
import '../models/settings_models.dart';
import '../../update/controller/app_update_controller.dart';
import '../../update/models/app_update_models.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.controller,
    this.quickPaste,
    this.appUpdate,
    this.modules,
    this.onOpenAndroidCaptureSetup,
  });

  final SettingsController controller;
  final QuickPasteSettingsController? quickPaste;
  final AppUpdateController? appUpdate;
  final ModulesController? modules;
  final Future<void> Function()? onOpenAndroidCaptureSetup;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  String get _screenshotProtectionDescription =>
      widget.controller.blockScreenshotsSupported
      ? 'Prevent screenshots and screen recording of CopyPaste.'
      : 'Screenshot blocking is unavailable on this platform.';
  static const _retentionOptions = <int>[0, 7, 30, 90, 365];
  static const _quotaOptions = <int>[
    1024 * 1024 * 1024,
    5 * 1024 * 1024 * 1024,
    10 * 1024 * 1024 * 1024,
    25 * 1024 * 1024 * 1024,
    50 * 1024 * 1024 * 1024,
  ];

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
  final _retentionKey = GlobalKey();
  final _storageQuotaKey = GlobalKey();
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
      animation: widget.controller,
      builder: (context, _) => _loadedContent(() => _content(context)),
    );
  }

  Widget _loadedContent(Widget Function() ready) =>
      switch (widget.controller.loadState) {
        SettingsLoadState.loading => const StateView.loading(
          message: 'Loading settings.',
        ),
        SettingsLoadState.error => StateView.error(
          title: 'Settings are unavailable',
          message: widget.controller.errorMessage,
          actionLabel: 'Try again',
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
            ? const StateView.empty(
                title: 'No settings found',
                message: 'Try a different search.',
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
                        padding: const EdgeInsets.all(AppSpacing.lg),
                        child: _searchField(),
                      ),
                      Expanded(
                        child: NavigationSidebar(
                          key: const ValueKey<String>(
                            'settings-navigation-sidebar',
                          ),
                          backgroundColor: Theme.of(
                            context,
                          ).colorScheme.secondary,
                          padding: const EdgeInsets.symmetric(
                            horizontal: AppSpacing.md,
                            vertical: AppSpacing.sm,
                          ),
                          spacing: AppSpacing.xs,
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
                                label: _navigationTargetLabel(target),
                                child: Icon(
                                  target.section.icon,
                                  size: AppIconSize.sm,
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
              padding: const EdgeInsets.all(AppSpacing.lg),
              child: _searchField(),
            ),
            Expanded(
              child: noSearchResults
                  ? content
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
                  leading: const Icon(LucideIcons.circleAlert),
                  title: const Text('Settings need attention'),
                  content: Text(message),
                ),
                const Gap(AppSpacing.xxxl),
              ],
              switch (selectedSection) {
                SettingsSectionId.modules => _SettingsSection(
                  key: _modulesSectionKey,
                  title: 'Modules',
                  showHeading: showHeading,
                  groupContent: false,
                  description: 'Optional features for CopyPaste.',
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
              },
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
      placeholder: const Text('Search settings'),
      onChanged: _updateSearch,
      features: const [
        InputFeature.leading(Icon(LucideIcons.search)),
        InputFeature.clear(),
      ],
    );
  }

  Widget _compactNavigationList(
    BuildContext context,
    List<_SettingsNavigationTarget> targets,
  ) {
    return ListView.separated(
      key: const ValueKey<String>('settings-category-list'),
      controller: PrimaryScrollController.maybeOf(context),
      primary: false,
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.lg,
        AppSpacing.zero,
        AppSpacing.lg,
        AppSpacing.lg,
      ).add(EdgeInsets.only(bottom: MediaQuery.paddingOf(context).bottom)),
      itemCount: targets.length,
      separatorBuilder: (context, index) => const Divider(),
      itemBuilder: (context, index) {
        final target = targets[index];
        return Button.ghost(
          key: ValueKey<String>('mobile-${target.widgetKey}'),
          style: AppTheme.settingsCategoryButtonStyle,
          alignment: Alignment.centerLeft,
          onPressed: () => _openCompactSection(context, target),
          child: Row(
            children: [
              Icon(target.section.icon, size: AppIconSize.md),
              const Gap(AppSpacing.md),
              Expanded(child: _navigationTargetLabel(target)),
              const Gap(AppSpacing.sm),
              const Icon(LucideIcons.chevronRight, size: AppIconSize.sm),
            ],
          ),
        );
      },
    );
  }

  void _openCompactSection(
    BuildContext context,
    _SettingsNavigationTarget target,
  ) {
    _compactRouteOpen = true;
    _activateTarget(target);
    final route = AppPageRoute<void>(
      disableAnimations: MediaQuery.disableAnimationsOf(context),
      settings: RouteSettings(name: 'settings/${target.section.slug}'),
      builder: (context) => AnimatedBuilder(
        animation: Listenable.merge([widget.controller, _detailRevision]),
        builder: (context, _) => Scaffold(
          headers: [
            AppBar(
              leading: [
                Semantics(
                  label: 'Back to Settings',
                  child: Button.ghost(
                    key: const ValueKey<String>('settings-back'),
                    style: AppTheme.navigationIconButtonStyle,
                    onPressed: () => Navigator.of(context).pop(),
                    child: const Icon(LucideIcons.arrowLeft),
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

  Widget _navigationTargetLabel(_SettingsNavigationTarget target) {
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

  SettingsSectionId get _effectiveSelectedSection {
    if ((_selectedSection == SettingsSectionId.quickPaste &&
            widget.quickPaste == null) ||
        (_selectedSection == SettingsSectionId.modules &&
            widget.modules == null)) {
      return SettingsSectionId.clipboard;
    }
    return _selectedSection;
  }

  List<_SettingsNavigationTarget> _navigationTargets() {
    if (_searchQuery.isEmpty) return _sectionTargets();
    return _searchTargets()
        .where((target) => target.matches(_searchQuery))
        .toList(growable: false);
  }

  List<_SettingsNavigationTarget> _sectionTargets() {
    return [
      for (final section in SettingsSectionId.values)
        if ((section != SettingsSectionId.quickPaste ||
                widget.quickPaste != null) &&
            (section != SettingsSectionId.modules || widget.modules != null))
          _SettingsNavigationTarget(
            id: 'section-${section.slug}',
            section: section,
            label: section.label,
            description: section.description,
            targetKey: _sectionKey(section),
          ),
    ];
  }

  List<_SettingsNavigationTarget> _searchTargets() {
    return [
      if (widget.modules != null)
        _SettingsNavigationTarget(
          id: 'result-modules',
          section: SettingsSectionId.modules,
          label: 'Modules',
          description: 'Install, update, and remove optional modules.',
          keywords: 'features commands extensions',
          targetKey: _modulesSectionKey,
        ),
      if (widget.onOpenAndroidCaptureSetup != null)
        _SettingsNavigationTarget(
          id: _SettingsTargetId.androidBackgroundCapture,
          section: SettingsSectionId.clipboard,
          label: 'Android background capture',
          description: 'Full or Limited mode with Shizuku or ADB setup.',
          keywords: 'background permissions setup',
          targetKey: _androidCaptureKey,
        ),
      if (widget.controller.screenshotCaptureSupported)
        _SettingsNavigationTarget(
          id: _SettingsTargetId.screenshotCapture,
          section: SettingsSectionId.clipboard,
          label: 'Save screenshots',
          description: 'Automatically save new Android screenshots to History.',
          keywords: 'images photos capture permissions',
          targetKey: _screenshotCaptureKey,
        ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.clipboardCapture,
        section: SettingsSectionId.clipboard,
        label: 'Clipboard capture',
        description: 'Pause or resume clipboard capture.',
        targetKey: _clipboardCaptureKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.excludedApplications,
        section: SettingsSectionId.privacy,
        label: 'Excluded applications',
        description: 'Skip automatic capture from excluded applications.',
        keywords: 'privacy app identifiers',
        targetKey: _excludedApplicationsKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.retention,
        section: SettingsSectionId.clipboard,
        label: 'Retention',
        description: 'Automatically remove old unpinned clipboard items.',
        targetKey: _retentionKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.storageQuota,
        section: SettingsSectionId.clipboard,
        label: 'Storage quota',
        description: 'Maximum local storage used by unpinned history.',
        keywords: 'disk space limit',
        targetKey: _storageQuotaKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.historyFiles,
        section: SettingsSectionId.data,
        label: 'History files',
        description: 'Export history, create backups, or restore a backup.',
        keywords: 'text encrypted backup data',
        targetKey: _historyFilesKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.sync,
        section: SettingsSectionId.sync,
        label: 'Sync',
        description: 'Paired-device synchronization.',
        targetKey: _syncEnabledKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.instantClipboard,
        section: SettingsSectionId.sync,
        label: 'Instant clipboard',
        description: 'Automatically copy new clips from other devices.',
        keywords: 'automatic copy paste sync received',
        targetKey: _instantClipboardKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.lanVisibility,
        section: SettingsSectionId.sync,
        label: 'LAN visibility',
        description: 'Allow nearby devices to discover this device.',
        keywords: 'local network discovery',
        targetKey: _lanVisibilityKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.notificationOnCopy,
        section: SettingsSectionId.notifications,
        label: 'Notification on copy',
        description: 'Show a notification after a background capture.',
        targetKey: _notificationOnCopyKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.notificationPreview,
        section: SettingsSectionId.notifications,
        label: 'Show clipboard content',
        description: 'Include a clip preview in copy notifications.',
        keywords: 'notification preview text image privacy',
        targetKey: _notificationPreviewKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.skipSecret,
        section: SettingsSectionId.privacy,
        label: 'Skip confidential clipboard',
        description: 'Keep producer-marked secrets out of History.',
        keywords: 'secret password sensitive concealed spoiler',
        targetKey: _skipSecretKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.skipTransient,
        section: SettingsSectionId.privacy,
        label: 'Skip temporary clipboard',
        description: 'Keep producer-marked temporary copies out of History.',
        keywords: 'transient temporary privacy',
        targetKey: _skipTransientKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.blockScreenshots,
        section: SettingsSectionId.privacy,
        label: 'Block screenshots',
        description: _screenshotProtectionDescription,
        keywords: 'privacy screen capture protection pairing qr security code',
        targetKey: _blockScreenshotsKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.soundOnCopy,
        section: SettingsSectionId.notifications,
        label: 'Sound on copy',
        description: 'Play platform feedback after a successful capture.',
        targetKey: _soundOnCopyKey,
      ),
      if (widget.appUpdate != null)
        _SettingsNavigationTarget(
          id: _SettingsTargetId.applicationUpdates,
          section: SettingsSectionId.about,
          label: 'Application updates',
          description: 'Check for and install CopyPaste updates.',
          keywords: 'version github release',
          targetKey: _applicationUpdatesKey,
        ),
      if (widget.quickPaste != null) ...[
        _SettingsNavigationTarget(
          id: _SettingsTargetId.quickPasteShortcut,
          section: SettingsSectionId.quickPaste,
          label: 'Open Quick Paste',
          description: 'Configure the global Quick Paste shortcut.',
          keywords: 'keyboard hotkey',
          targetKey: _quickPasteShortcutKey,
        ),
        _SettingsNavigationTarget(
          id: _SettingsTargetId.quickPasteAutoPaste,
          section: SettingsSectionId.quickPaste,
          label: 'Paste automatically',
          description: 'Paste the selected clip into the previous app.',
          keywords: 'accessibility automatic',
          targetKey: _quickPasteAutoPasteKey,
        ),
      ],
    ];
  }

  GlobalKey _sectionKey(SettingsSectionId section) => switch (section) {
    SettingsSectionId.clipboard => _clipboardSectionKey,
    SettingsSectionId.modules => _modulesSectionKey,
    SettingsSectionId.privacy => _privacySectionKey,
    SettingsSectionId.data => _dataSectionKey,
    SettingsSectionId.sync => _syncSectionKey,
    SettingsSectionId.notifications => _notificationsSectionKey,
    SettingsSectionId.quickPaste => _quickPasteSectionKey,
    SettingsSectionId.about => _aboutSectionKey,
  };

  Key? _selectedNavigationKey(
    List<_SettingsNavigationTarget> targets,
    SettingsSectionId selectedSection,
  ) {
    final selected = _selectedTarget(targets, selectedSection);
    return selected == null ? null : ValueKey<String>(selected.widgetKey);
  }

  _SettingsNavigationTarget? _selectedTarget(
    List<_SettingsNavigationTarget> targets,
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

  void _selectNavigationTarget(
    Key? key,
    List<_SettingsNavigationTarget> targets,
  ) {
    for (final target in targets) {
      if (key == ValueKey<String>(target.widgetKey)) {
        _activateTarget(target);
        return;
      }
    }
  }

  void _activateTarget(_SettingsNavigationTarget target) {
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

  Future<void> _revealSearchTarget(_SettingsNavigationTarget target) async {
    await _scrollTo(target.targetKey);
  }

  bool _isHighlighted(String targetId) => _highlightedTargetId == targetId;

  void _updateSearch(String value) {
    setState(() {
      widget.controller.navigation.search(value);
    });
  }

  Widget _clipboardSection(
    RuntimeSettings settings, {
    required bool showHeading,
  }) {
    final capture = widget.controller.capture;
    return _SettingsSection(
      showHeading: showHeading,
      key: _clipboardSectionKey,
      title: 'Clipboard',
      description: 'Capture clipboard changes and manage history limits.',
      children: [
        if (widget.onOpenAndroidCaptureSetup != null) ...[
          _SettingRow(
            key: _androidCaptureKey,
            highlighted: _isHighlighted(
              _SettingsTargetId.androidBackgroundCapture,
            ),
            title: 'Android background capture',
            description:
                'Choose Full or Limited mode and manage the one-time Shizuku or ADB setup.',
            trailing: Button.secondary(
              onPressed: widget.controller.busy
                  ? null
                  : widget.onOpenAndroidCaptureSetup,
              leading: const Icon(LucideIcons.shieldCheck),
              child: const Text('Open setup'),
            ),
          ),
        ],
        if (widget.controller.screenshotCaptureSupported) ...[
          _SettingRow(
            key: _screenshotCaptureKey,
            highlighted: _isHighlighted(_SettingsTargetId.screenshotCapture),
            title: 'Save screenshots',
            description: 'Automatically add new screenshots to History.',
            trailing: Switch(
              key: const ValueKey<String>('save-screenshots-switch'),
              value: widget.controller.screenshotCapture.enabled,
              onChanged: widget.controller.busy
                  ? null
                  : widget.controller.setScreenshotCaptureEnabled,
            ),
          ),
          if (widget.controller.screenshotCapture.enabled &&
              widget.controller.screenshotCapture.needsPermission)
            _SettingRow(
              title: 'Screenshot access',
              description:
                  'Allow access to all photos and notifications to save new screenshots in the background.',
              trailing: Button.secondary(
                key: const ValueKey<String>('screenshot-capture-permission'),
                onPressed: widget.controller.busy
                    ? null
                    : widget.controller.requestScreenshotCapturePermission,
                child: const Text('Allow access'),
              ),
            ),
        ],
        _SettingRow(
          key: _clipboardCaptureKey,
          highlighted: _isHighlighted(_SettingsTargetId.clipboardCapture),
          title: 'Clipboard capture',
          description: capture?.label ?? 'State unavailable',
          trailing: Switch(
            value: !(capture?.paused ?? true),
            onChanged: widget.controller.busy || capture == null
                ? null
                : (_) => widget.controller.toggleCapture(),
          ),
        ),
        if (capture != null && !capture.running && !capture.paused) ...[
          const Alert(
            leading: Icon(LucideIcons.circleAlert),
            title: Text('Capture is not running'),
            content: Text(
              'CopyPaste is not currently receiving clipboard changes on this device.',
            ),
          ),
        ],
        _SettingRow(
          key: _retentionKey,
          highlighted: _isHighlighted(_SettingsTargetId.retention),
          title: 'Retention',
          description: 'Automatically remove old unpinned clipboard items.',
          trailing: _valueSelect<int>(
            value: settings.retentionDays,
            values: _retentionOptions,
            label: (value) => value == 0 ? 'Keep forever' : '$value days',
            onChanged: widget.controller.setRetentionDays,
          ),
        ),
        _SettingRow(
          key: _storageQuotaKey,
          highlighted: _isHighlighted(_SettingsTargetId.storageQuota),
          title: 'Storage quota',
          description: 'Maximum local storage used by unpinned history.',
          trailing: _valueSelect<int>(
            value: settings.storageQuotaBytes,
            values: _quotaOptions,
            label: _formatQuota,
            onChanged: widget.controller.setStorageQuotaBytes,
          ),
        ),
      ],
    );
  }

  Widget _dataSection({required bool showHeading}) {
    return _SettingsSection(
      showHeading: showHeading,
      key: _dataSectionKey,
      title: 'Data',
      description: 'Export, back up, and restore clipboard history.',
      children: [
        Column(
          key: _historyFilesKey,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _SettingRow(
              highlighted: _isHighlighted(_SettingsTargetId.historyFiles),
              title: 'Export text history',
              description: 'Save text clips in a portable file.',
              trailing: Button.secondary(
                onPressed: widget.controller.busy
                    ? null
                    : widget.controller.exportTextHistory,
                leading: const Icon(LucideIcons.fileOutput),
                child: const Text('Export'),
              ),
            ),
            const Divider(),
            _SettingRow(
              highlighted: _isHighlighted(_SettingsTargetId.historyFiles),
              title: 'Encrypted backup',
              description: 'Save the complete local history for this device.',
              trailing: Button.secondary(
                onPressed: widget.controller.busy
                    ? null
                    : widget.controller.createBackup,
                leading: const Icon(LucideIcons.archive),
                child: const Text('Create backup'),
              ),
            ),
            const Divider(),
            _SettingRow(
              highlighted: _isHighlighted(_SettingsTargetId.historyFiles),
              title: 'Restore backup',
              description: 'Replace local history with an encrypted backup.',
              trailing: Button.destructive(
                onPressed: widget.controller.busy ? null : _confirmRestore,
                leading: const Icon(LucideIcons.history),
                child: const Text('Restore'),
              ),
            ),
          ],
        ),
      ],
    );
  }

  Widget _syncSection(RuntimeSettings settings, {required bool showHeading}) {
    return _SettingsSection(
      showHeading: showHeading,
      key: _syncSectionKey,
      title: 'Sync',
      description: 'Control synchronization with paired devices.',
      children: [
        _SettingRow(
          key: _syncEnabledKey,
          highlighted: _isHighlighted(_SettingsTargetId.sync),
          title: 'Sync',
          description: 'Master switch for paired-device synchronization.',
          trailing: Switch(
            value: settings.syncEnabled,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setSyncEnabled,
          ),
        ),
        _SettingRow(
          key: _instantClipboardKey,
          highlighted: _isHighlighted(_SettingsTargetId.instantClipboard),
          title: 'Instant clipboard',
          description:
              'Automatically copy newer clips from other devices to this clipboard.',
          trailing: Switch(
            key: const ValueKey('instant-clipboard-switch'),
            value: settings.instantClipboard,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setInstantClipboard,
          ),
        ),
        _SettingRow(
          key: _lanVisibilityKey,
          highlighted: _isHighlighted(_SettingsTargetId.lanVisibility),
          title: 'LAN visibility',
          description: 'Allow nearby devices to discover this device.',
          trailing: Switch(
            value: settings.lanVisibility,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setLanVisibility,
          ),
        ),
      ],
    );
  }

  Widget _privacySection(
    RuntimeSettings settings, {
    required bool showHeading,
  }) {
    return _SettingsSection(
      showHeading: showHeading,
      key: _privacySectionKey,
      title: 'Privacy',
      description:
          'Control which apps are captured and protect clipboard content.',
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
                        const Text('Excluded applications').medium(),
                        const Gap(AppSpacing.xs),
                        Text(switch (defaultTargetPlatform) {
                          TargetPlatform.macOS =>
                            'Skip automatic capture during activity from these apps. Background copies may bypass exclusions.',
                          TargetPlatform.windows =>
                            'Skip automatic capture from identified clipboard owners in this list.',
                          TargetPlatform.android =>
                            'Android skips automatic capture while exclusions are set because it cannot identify source apps.',
                          TargetPlatform.linux =>
                            'Skip automatic capture from identified clipboard owners. Capture is paused when exclusions are set and the source cannot be identified.',
                          TargetPlatform.iOS || TargetPlatform.fuchsia =>
                            'Application exclusions are supported on macOS, Windows, and Android.',
                        }).muted().textSmall(),
                      ],
                    ),
                  ),
                  const Gap(AppSpacing.md),
                  Button.secondary(
                    onPressed: widget.controller.busy
                        ? null
                        : _addExcludedApplication,
                    leading: const Icon(LucideIcons.plus),
                    child: const Text('Add'),
                  ),
                ],
              ),
              if (settings.excludedAppIds.isEmpty) ...[
                const Gap(AppSpacing.lg),
                const Text('No applications are excluded.').muted().textSmall(),
              ] else ...[
                const Gap(AppSpacing.md),
                for (final appId in settings.excludedAppIds)
                  Padding(
                    padding: const EdgeInsets.only(top: AppSpacing.xs),
                    child: Row(
                      children: [
                        const Icon(
                          LucideIcons.shieldCheck,
                          size: AppIconSize.sm,
                        ),
                        const Gap(AppSpacing.sm),
                        Expanded(
                          child: Text(
                            appId,
                            style: Theme.of(context).typography.mono,
                          ).textSmall(),
                        ),
                        Tooltip(
                          showDuration: AppMotion.resolve(
                            context,
                            AppMotion.standard,
                          ),
                          tooltip: (context) =>
                              TooltipContainer(child: Text('Remove $appId')),
                          child: Button.ghost(
                            style: AppTheme.controlButtonStyle(
                              const ButtonStyle.ghostIcon(),
                            ),
                            onPressed: widget.controller.busy
                                ? null
                                : () => widget.controller.removeExcludedApp(
                                    appId,
                                  ),
                            child: const Icon(LucideIcons.x),
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            ],
          ),
        ),
        _SettingRow(
          key: _skipSecretKey,
          highlighted: _isHighlighted(_SettingsTargetId.skipSecret),
          title: 'Skip confidential clipboard',
          description: 'Keep producer-marked secrets out of History.',
          trailing: Switch(
            key: const ValueKey('skip-secret-switch'),
            value: settings.skipSecret,
            enabled: !widget.controller.busy,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setSkipSecret,
          ),
        ),
        _SettingRow(
          key: _skipTransientKey,
          highlighted: _isHighlighted(_SettingsTargetId.skipTransient),
          title: 'Skip temporary clipboard',
          description: 'Keep producer-marked temporary copies out of History.',
          trailing: Switch(
            key: const ValueKey('skip-transient-switch'),
            value: settings.skipTransient,
            enabled: !widget.controller.busy,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setSkipTransient,
          ),
        ),
        _SettingRow(
          key: _blockScreenshotsKey,
          highlighted: _isHighlighted(_SettingsTargetId.blockScreenshots),
          title: 'Block screenshots',
          description: _screenshotProtectionDescription,
          trailing: Switch(
            key: const ValueKey('block-screenshots-switch'),
            value: widget.controller.blockScreenshots,
            enabled:
                !widget.controller.busy &&
                widget.controller.blockScreenshotsSupported,
            onChanged:
                widget.controller.busy ||
                    !widget.controller.blockScreenshotsSupported
                ? null
                : widget.controller.setBlockScreenshots,
          ),
        ),
      ],
    );
  }

  Widget _notificationsSection(
    RuntimeSettings settings, {
    required bool showHeading,
  }) {
    return _SettingsSection(
      showHeading: showHeading,
      key: _notificationsSectionKey,
      title: 'Notifications',
      description: 'Choose notifications and sounds for captured clips.',
      children: [
        _SettingRow(
          key: _notificationOnCopyKey,
          highlighted: _isHighlighted(_SettingsTargetId.notificationOnCopy),
          title: 'Notification on copy',
          description: 'Show a notification after a background capture.',
          trailing: Switch(
            value: settings.notifyOnCopy,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setNotifyOnCopy,
          ),
        ),
        _SettingRow(
          key: _notificationPreviewKey,
          highlighted: _isHighlighted(_SettingsTargetId.notificationPreview),
          title: 'Show clipboard content',
          description: 'Include a clip preview in copy notifications.',
          trailing: Switch(
            value: settings.notificationPreview,
            enabled: settings.notifyOnCopy && !widget.controller.busy,
            onChanged: !settings.notifyOnCopy || widget.controller.busy
                ? null
                : widget.controller.setNotificationPreview,
          ),
        ),
        _SettingRow(
          key: _soundOnCopyKey,
          highlighted: _isHighlighted(_SettingsTargetId.soundOnCopy),
          title: 'Sound on copy',
          description: 'Play platform feedback after a successful capture.',
          trailing: Switch(
            value: settings.soundOnCopy,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setSoundOnCopy,
          ),
        ),
      ],
    );
  }

  Widget _aboutSection({required bool showHeading}) {
    return _SettingsSection(
      showHeading: showHeading,
      key: _aboutSectionKey,
      title: 'About',
      description: 'CopyPaste version and application updates.',
      children: [
        _SettingRow(
          title: 'Version',
          description: 'Installed CopyPaste version.',
          trailing: Text(
            widget.appUpdate?.currentVersion?.toString() ?? 'Unavailable',
          ),
        ),
        if (widget.appUpdate case final controller?) ...[
          const Gap(AppSpacing.md),
          AnimatedBuilder(
            animation: controller,
            builder: (context, _) => _SettingRow(
              key: _applicationUpdatesKey,
              highlighted: _isHighlighted(_SettingsTargetId.applicationUpdates),
              title: 'Application updates',
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
    return switch (controller.phase) {
      AppUpdatePhase.idle => 'Check GitHub for a newer CopyPaste release.',
      AppUpdatePhase.checking => 'Checking GitHub for updates.',
      AppUpdatePhase.upToDate =>
        current == null
            ? 'CopyPaste is up to date.'
            : 'Version $current is up to date.',
      AppUpdatePhase.available =>
        'Version $available is available. This device has version $current.',
      AppUpdatePhase.downloading =>
        'Downloading version $available · ${(controller.downloadProgress * 100).round()}%.',
      AppUpdatePhase.installing =>
        controller.message ?? 'Installing version $available.',
      AppUpdatePhase.permissionRequired ||
      AppUpdatePhase.restartRequired ||
      AppUpdatePhase.unavailable ||
      AppUpdatePhase.error =>
        controller.message ?? 'Application updates need attention.',
    };
  }

  Widget _updateAction(AppUpdateController controller) {
    return switch (controller.phase) {
      AppUpdatePhase.available => Button.primary(
        key: const ValueKey<String>('install-app-update'),
        onPressed: controller.install,
        leading: const Icon(LucideIcons.download),
        child: const Text('Update now'),
      ),
      AppUpdatePhase.permissionRequired => Button.primary(
        key: const ValueKey<String>('continue-app-update'),
        onPressed: controller.install,
        leading: const Icon(LucideIcons.settings),
        child: const Text('Continue'),
      ),
      AppUpdatePhase.unavailable => Button.secondary(
        key: const ValueKey<String>('open-update-release'),
        onPressed: controller.openReleasePage,
        leading: const Icon(LucideIcons.externalLink),
        child: const Text('Open release'),
      ),
      AppUpdatePhase.checking => const Button.secondary(
        onPressed: null,
        leading: Icon(LucideIcons.refreshCw),
        child: Text('Checking'),
      ),
      AppUpdatePhase.downloading => const Button.secondary(
        onPressed: null,
        leading: Icon(LucideIcons.download),
        child: Text('Downloading'),
      ),
      AppUpdatePhase.installing => const Button.secondary(
        onPressed: null,
        leading: Icon(LucideIcons.loaderCircle),
        child: Text('Installing'),
      ),
      AppUpdatePhase.restartRequired => Button.primary(
        key: const ValueKey<String>('restart-after-app-update'),
        onPressed: controller.busy || !controller.canRestart
            ? null
            : controller.restartApplication,
        leading: const Icon(LucideIcons.refreshCw),
        child: const Text('Restart CopyPaste'),
      ),
      AppUpdatePhase.idle ||
      AppUpdatePhase.upToDate ||
      AppUpdatePhase.error => Button.secondary(
        key: const ValueKey<String>('check-app-update'),
        onPressed: () => _checkForUpdates(controller),
        leading: const Icon(LucideIcons.refreshCw),
        child: const Text('Check again'),
      ),
    };
  }

  Future<void> _checkForUpdates(AppUpdateController controller) async {
    await controller.check();
    if (!mounted) return;

    switch (controller.phase) {
      case AppUpdatePhase.upToDate:
        final version = controller.currentVersion?.toString();
        AppToast.show(
          context,
          title: 'No updates available',
          message: version == null
              ? 'CopyPaste is the latest version.'
              : 'CopyPaste $version is the latest version.',
          tone: AppToastTone.success,
        );
        break;
      case AppUpdatePhase.available:
        final version = controller.release?.version.toString();
        AppToast.show(
          context,
          title: 'Update available',
          message: version == null
              ? 'A newer CopyPaste version is ready to install.'
              : 'CopyPaste $version is ready to install.',
        );
        break;
      case AppUpdatePhase.unavailable:
        AppToast.show(
          context,
          title: 'Update unavailable',
          message:
              controller.message ??
              'This installation cannot update automatically.',
          tone: AppToastTone.error,
        );
        break;
      case AppUpdatePhase.error:
        AppToast.show(
          context,
          title: 'Update check failed',
          message:
              controller.message ?? 'CopyPaste could not check for updates.',
          tone: AppToastTone.error,
        );
        break;
      case AppUpdatePhase.idle ||
          AppUpdatePhase.checking ||
          AppUpdatePhase.downloading ||
          AppUpdatePhase.installing ||
          AppUpdatePhase.permissionRequired ||
          AppUpdatePhase.restartRequired:
        break;
    }
  }

  Widget _valueSelect<T>({
    required T value,
    required List<T> values,
    required String Function(T value) label,
    required Future<bool> Function(T value) onChanged,
  }) {
    final options = <T>{value, ...values}.toList(growable: false);
    return Select<T>(
      value: value,
      onChanged: widget.controller.busy
          ? null
          : (next) {
              if (next != null) unawaited(onChanged(next));
            },
      popup: SelectPopup<T>(
        items: SelectItemList(
          children: [
            for (final option in options)
              SelectItemButton<T>(value: option, child: Text(label(option))),
          ],
        ),
      ).call,
      itemBuilder: (context, selected) => Text(label(selected)),
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
    final input = TextEditingController();
    final value = await AppOverlays.showDialog<String>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.shieldPlus,
        title: const Text('Exclude application'),
        content: TextField(
          controller: input,
          autofocus: true,
          placeholder: const Text('Application identifier'),
          decoration: AppOverlays.dialogFieldDecoration(dialogContext),
        ),
        actions: [
          Button.ghost(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Cancel'),
          ),
          Button.primary(
            onPressed: () => Navigator.pop(dialogContext, input.text),
            child: const Text('Exclude'),
          ),
        ],
      ),
    );
    input.dispose();
    if (value != null) await widget.controller.addExcludedApp(value);
  }

  Future<void> _confirmRestore() async {
    final confirmed = await AppOverlays.showDialog<bool>(
      context,
      builder: (dialogContext) => AppOverlays.alertDialog(
        icon: LucideIcons.history,
        title: const Text('Replace local history?'),
        content: const Text(
          'The selected encrypted backup will replace the clipboard history on this device. Current settings and device identity are kept.',
        ),
        actions: [
          Button.ghost(
            onPressed: () => Navigator.pop(dialogContext, false),
            child: const Text('Cancel'),
          ),
          Button.destructive(
            onPressed: () => Navigator.pop(dialogContext, true),
            child: const Text('Choose backup'),
          ),
        ],
      ),
    );
    if (confirmed == true) await widget.controller.restoreBackup();
  }

  String _formatQuota(int bytes) {
    final gib = bytes / (1024 * 1024 * 1024);
    return '${gib.toStringAsFixed(gib == gib.roundToDouble() ? 0 : 1)} GB';
  }
}

class _SettingsSection extends StatelessWidget {
  const _SettingsSection({
    super.key,
    required this.title,
    required this.description,
    required this.children,
    this.groupContent = true,
    this.showHeading = true,
  });

  final String title;
  final String description;
  final List<Widget> children;
  final bool groupContent;
  final bool showHeading;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (showHeading) ...[
          Text(title, style: Theme.of(context).typography.h3),
          const Gap(AppSpacing.xs),
        ],
        Text(description).muted(),
        const Gap(AppSpacing.lg),
        if (groupContent)
          Card(
            theme: AppTheme.settingsGroupCardTheme,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                for (var i = 0; i < children.length; i++) ...[
                  if (i > 0) const Divider(),
                  children[i],
                ],
              ],
            ),
          )
        else
          ...children,
      ],
    );
  }
}

class _SettingRow extends StatelessWidget {
  const _SettingRow({
    super.key,
    required this.title,
    required this.description,
    required this.trailing,
    this.highlighted = false,
  });

  final String title;
  final String description;
  final Widget trailing;
  final bool highlighted;

  @override
  Widget build(BuildContext context) {
    final actionControl = ButtonStyleOverride(
      decoration: AppTheme.actionButtonDecoration,
      child: trailing,
    );
    final copy = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title).medium(),
        const Gap(AppSpacing.xs),
        Text(description).muted().textSmall(),
      ],
    );
    return OutlinedContainer(
      key: ValueKey<String>('settings-row-$title'),
      duration: AppMotion.resolve(context, AppMotion.quick),
      theme: AppTheme.settingsRowTheme(context, highlighted: highlighted),
      clipBehavior: Clip.none,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final stack =
              trailing is! Switch &&
              (constraints.maxWidth <
                      AppLayoutSize.settingsStackedControlWidth ||
                  MediaQuery.textScalerOf(context).scale(1) > 1.3);
          if (stack) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                copy,
                const Gap(AppSpacing.md),
                Align(alignment: Alignment.centerRight, child: actionControl),
              ],
            );
          }
          return Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Expanded(child: copy),
              const Gap(AppSpacing.lg),
              actionControl,
            ],
          );
        },
      ),
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
        return _SettingsSection(
          showHeading: showHeading,
          title: 'Quick Paste',
          description:
              'Open clipboard history from anywhere without switching windows.',
          children: [
            if (controller.linuxIntegration case final integration?)
              if (integration.session == LinuxDesktopSession.wayland &&
                  !integration.quickPaste)
                Alert(
                  leading: const Icon(LucideIcons.keyboard),
                  title: const Text('Set up Quick Paste'),
                  content: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text(
                        'Enable the GNOME or KDE integration and allow keyboard control to paste into the previous app.',
                      ),
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
                              child: const Text('Desktop integration'),
                            ),
                          if (integration.remoteDesktop ==
                              LinuxRemoteDesktopState.consentRequired)
                            Button.secondary(
                              key: const ValueKey('linux-keyboard-permission'),
                              onPressed: controller.busy
                                  ? null
                                  : controller.requestLinuxRemoteDesktop,
                              child: const Text('Allow keyboard control'),
                            ),
                          Button.ghost(
                            key: const ValueKey('linux-integration-refresh'),
                            onPressed: controller.busy
                                ? null
                                : controller.refreshLinuxIntegration,
                            child: const Text('Check again'),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
            if (!controller.initialized && controller.busy)
              const StateView.loading(message: 'Loading Quick Paste settings.')
            else if (!controller.supported)
              const Alert(
                leading: Icon(LucideIcons.circleAlert),
                title: Text('Quick Paste is unavailable'),
                content: Text(
                  'This device does not provide the Quick Paste window.',
                ),
              )
            else ...[
              _SettingRow(
                key: shortcutKey,
                highlighted: shortcutHighlighted,
                title: 'Open Quick Paste',
                description: controller.registeredShortcutDescription == null
                    ? 'The shortcut works while CopyPaste is running in the background.'
                    : 'Registered shortcut: ${controller.registeredShortcutDescription}',
                trailing: _ShortcutRecorder(controller: controller),
              ),
              _SettingRow(
                key: autoPasteKey,
                highlighted: autoPasteHighlighted,
                title: 'Paste automatically',
                description:
                    'Selecting a clip returns to the previous app and pastes it.',
                trailing: Switch(
                  value: controller.autoPaste,
                  onChanged: controller.busy ? null : controller.setAutoPaste,
                ),
              ),
              if (Platform.isMacOS &&
                  controller.autoPaste &&
                  !controller.accessibilityGranted) ...[
                Alert(
                  leading: const Icon(LucideIcons.accessibility),
                  title: const Text('Accessibility is required'),
                  content: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text(
                        'Until permission is granted, selecting a clip copies it without pasting.',
                      ),
                      const Gap(AppSpacing.sm),
                      Align(
                        alignment: Alignment.centerRight,
                        child: Button.secondary(
                          onPressed: controller.busy
                              ? null
                              : controller.requestAccessibility,
                          child: const Text('Enable'),
                        ),
                      ),
                    ],
                  ),
                ),
              ],
              if (controller.errorMessage case final message?) ...[
                Alert.destructive(
                  leading: const Icon(LucideIcons.circleAlert),
                  title: const Text('Quick Paste needs attention'),
                  content: Text(message),
                ),
              ],
            ],
          ],
        );
      },
    );
  }
}

class _ShortcutRecorder extends StatefulWidget {
  const _ShortcutRecorder({required this.controller});

  final QuickPasteSettingsController controller;

  @override
  State<_ShortcutRecorder> createState() => _ShortcutRecorderState();
}

class _ShortcutRecorderState extends State<_ShortcutRecorder> {
  final FocusNode _focusNode = FocusNode(debugLabel: 'quick-paste-recorder');
  bool _recording = false;

  @override
  void dispose() {
    if (_recording) widget.controller.cancelShortcutRecording();
    _focusNode.dispose();
    super.dispose();
  }

  Future<void> _startRecording() async {
    if (!await widget.controller.beginShortcutRecording() || !mounted) return;
    setState(() => _recording = true);
    _focusNode.requestFocus();
  }

  Future<void> _cancelRecording() async {
    if (!_recording) return;
    setState(() => _recording = false);
    await widget.controller.cancelShortcutRecording();
  }

  KeyEventResult _handleKey(FocusNode node, KeyEvent event) {
    if (!_recording || event is! KeyDownEvent) return KeyEventResult.ignored;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _cancelRecording();
      return KeyEventResult.handled;
    }
    final modifiers = <DesktopShortcutModifier>[
      for (final modifier in DesktopShortcutModifier.values)
        if (modifier.physicalKeys.any(
          HardwareKeyboard.instance.physicalKeysPressed.contains,
        ))
          modifier,
    ];
    final shortcut = DesktopShortcut(
      key: event.physicalKey,
      modifiers: modifiers,
    );
    if (!shortcut.isValid) return KeyEventResult.handled;
    setState(() => _recording = false);
    widget.controller.setShortcut(shortcut);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    return Focus(
      focusNode: _focusNode,
      onKeyEvent: _handleKey,
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Button.secondary(
            onPressed: widget.controller.busy
                ? null
                : _recording
                ? _cancelRecording
                : _startRecording,
            child: _recording
                ? const Text('Press shortcut…')
                : KeyboardDisplay(keys: widget.controller.shortcut.displayKeys),
          ),
          const Gap(AppSpacing.sm),
          Tooltip(
            showDuration: AppMotion.resolve(context, AppMotion.standard),
            tooltip: (context) =>
                const TooltipContainer(child: Text('Reset shortcut')),
            child: Button.ghost(
              style: AppTheme.controlButtonStyle(const ButtonStyle.ghostIcon()),
              onPressed: widget.controller.busy || _recording
                  ? null
                  : widget.controller.resetShortcut,
              child: const Icon(LucideIcons.rotateCcw),
            ),
          ),
        ],
      ),
    );
  }
}

class _SettingsNavigationTarget {
  const _SettingsNavigationTarget({
    required this.id,
    required this.section,
    required this.label,
    required this.description,
    required this.targetKey,
    this.keywords = '',
  });

  final String id;
  final SettingsSectionId section;
  final String label;
  final String description;
  final String keywords;
  final GlobalKey targetKey;

  String get widgetKey => 'settings-$id';
  bool get isSearchResult => id.startsWith('result-');

  bool matches(String query) {
    final searchable = [
      section.label,
      label,
      description,
      keywords,
    ].join(' ').toLowerCase();
    return query
        .split(RegExp(r'\s+'))
        .where((token) => token.isNotEmpty)
        .every(searchable.contains);
  }
}

abstract final class _SettingsTargetId {
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
