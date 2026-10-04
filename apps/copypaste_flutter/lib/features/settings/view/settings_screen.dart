import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../platform/desktop/global_shortcut.dart';
import '../../../shared/adaptive_breakpoints.dart';
import '../../../shared/state_view.dart';
import '../controller/quick_paste_settings_controller.dart';
import '../controller/settings_controller.dart';
import '../models/settings_models.dart';
import '../../update/controller/app_update_controller.dart';
import '../../update/models/app_update_models.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    required this.controller,
    this.quickPaste,
    this.appUpdate,
    this.onQuitForUpdate,
    this.onOpenAndroidCaptureSetup,
  });

  final SettingsController controller;
  final QuickPasteSettingsController? quickPaste;
  final AppUpdateController? appUpdate;
  final Future<void> Function()? onQuitForUpdate;
  final Future<void> Function()? onOpenAndroidCaptureSetup;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  static const _retentionOptions = <int>[0, 7, 30, 90, 365];
  static const _quotaOptions = <int>[
    1024 * 1024 * 1024,
    5 * 1024 * 1024 * 1024,
    10 * 1024 * 1024 * 1024,
    25 * 1024 * 1024 * 1024,
    50 * 1024 * 1024 * 1024,
  ];

  final _captureSectionKey = GlobalKey();
  final _storageSectionKey = GlobalKey();
  final _syncSectionKey = GlobalKey();
  final _feedbackSectionKey = GlobalKey();
  final _quickPasteSectionKey = GlobalKey();
  final _androidCaptureKey = GlobalKey();
  final _clipboardCaptureKey = GlobalKey();
  final _excludedApplicationsKey = GlobalKey();
  final _retentionKey = GlobalKey();
  final _storageQuotaKey = GlobalKey();
  final _historyFilesKey = GlobalKey();
  final _syncEnabledKey = GlobalKey();
  final _lanVisibilityKey = GlobalKey();
  final _notificationOnCopyKey = GlobalKey();
  final _soundOnCopyKey = GlobalKey();
  final _applicationUpdatesKey = GlobalKey();
  final _quickPasteShortcutKey = GlobalKey();
  final _quickPasteAutoPasteKey = GlobalKey();
  final _searchController = TextEditingController();
  Timer? _highlightTimer;

  _SettingsSectionId _selectedSection = _SettingsSectionId.capture;
  String _searchQuery = '';
  String? _selectedTargetId;
  String? _highlightedTargetId;

  @override
  void dispose() {
    _highlightTimer?.cancel();
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) => switch (widget.controller.loadState) {
        SettingsLoadState.loading => const StateView.loading(
          message: 'Loading settings.',
        ),
        SettingsLoadState.error => StateView.error(
          title: 'Settings are unavailable',
          message: widget.controller.errorMessage,
          actionLabel: 'Try again',
          onAction: widget.controller.retry,
        ),
        SettingsLoadState.ready => _content(context),
      },
    );
  }

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
        final content = noSearchResults
            ? const StateView.empty(
                title: 'No settings found',
                message: 'Try a different search.',
              )
            : _sectionContent(settings, selectedSection);

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
            DecoratedBox(
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.secondary,
              ),
              child: Padding(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _searchField(),
                    if (targets.isNotEmpty) ...[
                      const Gap(AppSpacing.md),
                      _mobileNavigationSelect(targets, selectedSection),
                    ],
                  ],
                ),
              ),
            ),
            const Divider(),
            Expanded(child: content),
          ],
        );
      },
    );
  }

  Widget _sectionContent(
    RuntimeSettings settings,
    _SettingsSectionId selectedSection,
  ) {
    final controller = widget.controller;
    return SingleChildScrollView(
      key: PageStorageKey<String>('settings-${selectedSection.slug}-scroll'),
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.lg,
        AppSpacing.lg,
        AppSpacing.lg,
        AppSpacing.huge,
      ),
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
                _SettingsSectionId.capture => _captureSection(settings),
                _SettingsSectionId.storageData => _storageSection(settings),
                _SettingsSectionId.sync => _syncSection(settings),
                _SettingsSectionId.feedback => _feedbackSection(settings),
                _SettingsSectionId.quickPaste => _QuickPasteSection(
                  key: _quickPasteSectionKey,
                  controller: widget.quickPaste!,
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
      placeholder: const Text('Search settings'),
      onChanged: _updateSearch,
      features: const [
        InputFeature.leading(Icon(LucideIcons.search)),
        InputFeature.clear(),
      ],
    );
  }

  Widget _mobileNavigationSelect(
    List<_SettingsNavigationTarget> targets,
    _SettingsSectionId selectedSection,
  ) {
    final selected = _selectedTarget(targets, selectedSection);
    return SizedBox(
      width: double.infinity,
      child: Select<_SettingsNavigationTarget>(
        key: const ValueKey<String>('settings-mobile-section-select'),
        value: selected,
        placeholder: Text('${targets.length} matching settings'),
        onChanged: (target) {
          if (target != null) _activateTarget(target);
        },
        popup: SelectPopup<_SettingsNavigationTarget>(
          items: SelectItemList(
            children: [
              for (final target in targets)
                SelectItemButton<_SettingsNavigationTarget>(
                  key: ValueKey<String>('mobile-${target.widgetKey}'),
                  value: target,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(target.section.icon, size: AppIconSize.sm),
                      const Gap(AppSpacing.sm),
                      Flexible(child: _navigationTargetLabel(target)),
                    ],
                  ),
                ),
            ],
          ),
        ).call,
        itemBuilder: (context, target) => Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(target.section.icon, size: AppIconSize.sm),
            const Gap(AppSpacing.sm),
            Flexible(child: _navigationTargetLabel(target)),
          ],
        ),
      ),
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

  _SettingsSectionId get _effectiveSelectedSection {
    if (_selectedSection == _SettingsSectionId.quickPaste &&
        widget.quickPaste == null) {
      return _SettingsSectionId.capture;
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
      for (final section in _SettingsSectionId.values)
        if (section != _SettingsSectionId.quickPaste ||
            widget.quickPaste != null)
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
      if (widget.onOpenAndroidCaptureSetup != null)
        _SettingsNavigationTarget(
          id: _SettingsTargetId.androidBackgroundCapture,
          section: _SettingsSectionId.capture,
          label: 'Android background capture',
          description: 'Full or Limited mode with Shizuku or ADB setup.',
          keywords: 'background permissions setup',
          targetKey: _androidCaptureKey,
        ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.clipboardCapture,
        section: _SettingsSectionId.capture,
        label: 'Clipboard capture',
        description: 'Pause or resume clipboard capture.',
        targetKey: _clipboardCaptureKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.excludedApplications,
        section: _SettingsSectionId.capture,
        label: 'Excluded applications',
        description: 'Applications whose clipboard changes are never captured.',
        keywords: 'privacy app identifiers',
        targetKey: _excludedApplicationsKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.retention,
        section: _SettingsSectionId.storageData,
        label: 'Retention',
        description: 'Automatically remove old unpinned clipboard items.',
        targetKey: _retentionKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.storageQuota,
        section: _SettingsSectionId.storageData,
        label: 'Storage quota',
        description: 'Maximum local storage used by unpinned history.',
        keywords: 'disk space limit',
        targetKey: _storageQuotaKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.historyFiles,
        section: _SettingsSectionId.storageData,
        label: 'History files',
        description: 'Export history, create backups, or restore a backup.',
        keywords: 'text encrypted backup data',
        targetKey: _historyFilesKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.sync,
        section: _SettingsSectionId.sync,
        label: 'Sync',
        description: 'Paired-device synchronization.',
        targetKey: _syncEnabledKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.lanVisibility,
        section: _SettingsSectionId.sync,
        label: 'LAN visibility',
        description: 'Allow nearby devices to discover this device.',
        keywords: 'local network discovery',
        targetKey: _lanVisibilityKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.notificationOnCopy,
        section: _SettingsSectionId.feedback,
        label: 'Notification on copy',
        description: 'Show a notification after a background capture.',
        targetKey: _notificationOnCopyKey,
      ),
      _SettingsNavigationTarget(
        id: _SettingsTargetId.soundOnCopy,
        section: _SettingsSectionId.feedback,
        label: 'Sound on copy',
        description: 'Play platform feedback after a successful capture.',
        targetKey: _soundOnCopyKey,
      ),
      if (widget.appUpdate != null)
        _SettingsNavigationTarget(
          id: _SettingsTargetId.applicationUpdates,
          section: _SettingsSectionId.feedback,
          label: 'Application updates',
          description: 'Check for and install CopyPaste updates.',
          keywords: 'version github release',
          targetKey: _applicationUpdatesKey,
        ),
      if (widget.quickPaste != null) ...[
        _SettingsNavigationTarget(
          id: _SettingsTargetId.quickPasteShortcut,
          section: _SettingsSectionId.quickPaste,
          label: 'Open Quick Paste',
          description: 'Configure the global Quick Paste shortcut.',
          keywords: 'keyboard hotkey',
          targetKey: _quickPasteShortcutKey,
        ),
        _SettingsNavigationTarget(
          id: _SettingsTargetId.quickPasteAutoPaste,
          section: _SettingsSectionId.quickPaste,
          label: 'Paste automatically',
          description: 'Paste the selected clip into the previous app.',
          keywords: 'accessibility automatic',
          targetKey: _quickPasteAutoPasteKey,
        ),
      ],
    ];
  }

  GlobalKey _sectionKey(_SettingsSectionId section) => switch (section) {
    _SettingsSectionId.capture => _captureSectionKey,
    _SettingsSectionId.storageData => _storageSectionKey,
    _SettingsSectionId.sync => _syncSectionKey,
    _SettingsSectionId.feedback => _feedbackSectionKey,
    _SettingsSectionId.quickPaste => _quickPasteSectionKey,
  };

  Key? _selectedNavigationKey(
    List<_SettingsNavigationTarget> targets,
    _SettingsSectionId selectedSection,
  ) {
    final selected = _selectedTarget(targets, selectedSection);
    return selected == null ? null : ValueKey<String>(selected.widgetKey);
  }

  _SettingsNavigationTarget? _selectedTarget(
    List<_SettingsNavigationTarget> targets,
    _SettingsSectionId selectedSection,
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
    final highlight = target.isSearchResult;
    _highlightTimer?.cancel();
    setState(() {
      _selectedSection = target.section;
      _selectedTargetId = highlight ? target.id : null;
      _highlightedTargetId = highlight ? target.id : null;
    });
    if (!highlight) return;
    _highlightTimer = Timer(AppMotion.settingsHighlightHold, () {
      if (!mounted || _highlightedTargetId != target.id) return;
      setState(() => _highlightedTargetId = null);
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
      _searchQuery = value.trim().toLowerCase();
      _selectedTargetId = null;
    });
  }

  Widget _captureSection(RuntimeSettings settings) {
    final capture = widget.controller.capture;
    return _SettingsSection(
      key: _captureSectionKey,
      title: 'Capture',
      description: 'Control clipboard capture and application exclusions.',
      children: [
        if (widget.onOpenAndroidCaptureSetup != null) ...[
          _SettingCard(
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
          const Gap(AppSpacing.md),
        ],
        _SettingCard(
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
          const Gap(AppSpacing.md),
          const Alert(
            leading: Icon(LucideIcons.circleAlert),
            title: Text('Capture is not running'),
            content: Text(
              'CopyPaste is not currently receiving clipboard changes on this device.',
            ),
          ),
        ],
        const Gap(AppSpacing.md),
        Card(
          key: _excludedApplicationsKey,
          theme: AppTheme.settingsSearchTargetCardTheme(
            context,
            highlighted: _isHighlighted(_SettingsTargetId.excludedApplications),
          ),
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
                        const Text(
                          'Clipboard changes from these application identifiers are never captured.',
                        ).muted().textSmall(),
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
                            style: const ButtonStyle.ghostIcon(),
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
      ],
    );
  }

  Widget _storageSection(RuntimeSettings settings) {
    return _SettingsSection(
      key: _storageSectionKey,
      title: 'Storage & Data',
      description: 'Set retention limits and manage local history files.',
      children: [
        _SettingCard(
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
        const Gap(AppSpacing.md),
        _SettingCard(
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
        const Gap(AppSpacing.md),
        Card(
          key: _historyFilesKey,
          theme: AppTheme.settingsSearchTargetCardTheme(
            context,
            highlighted: _isHighlighted(_SettingsTargetId.historyFiles),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Text('History files').medium(),
              const Gap(AppSpacing.xs),
              const Text(
                'Text export is portable. Encrypted backups preserve the complete local history for this device.',
              ).muted().textSmall(),
              const Gap(AppSpacing.lg),
              Wrap(
                spacing: AppSpacing.sm,
                runSpacing: AppSpacing.sm,
                children: [
                  Button.secondary(
                    onPressed: widget.controller.busy
                        ? null
                        : widget.controller.exportTextHistory,
                    leading: const Icon(LucideIcons.fileOutput),
                    child: const Text('Export text history'),
                  ),
                  Button.secondary(
                    onPressed: widget.controller.busy
                        ? null
                        : widget.controller.createBackup,
                    leading: const Icon(LucideIcons.archive),
                    child: const Text('Create encrypted backup'),
                  ),
                  Button.destructive(
                    onPressed: widget.controller.busy ? null : _confirmRestore,
                    leading: const Icon(LucideIcons.history),
                    child: const Text('Restore backup'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }

  Widget _syncSection(RuntimeSettings settings) {
    return _SettingsSection(
      key: _syncSectionKey,
      title: 'Sync',
      description: 'Control synchronization with paired devices.',
      children: [
        _SettingCard(
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
        const Gap(AppSpacing.md),
        _SettingCard(
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

  Widget _feedbackSection(RuntimeSettings settings) {
    return _SettingsSection(
      key: _feedbackSectionKey,
      title: 'Feedback',
      description: 'Manage application updates and clipboard feedback.',
      children: [
        _SettingCard(
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
        const Gap(AppSpacing.md),
        _SettingCard(
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
        if (widget.appUpdate case final controller?) ...[
          const Gap(AppSpacing.md),
          AnimatedBuilder(
            animation: controller,
            builder: (context, _) => _SettingCard(
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
        key: const ValueKey<String>('quit-after-app-update'),
        onPressed: widget.onQuitForUpdate,
        leading: const Icon(LucideIcons.logOut),
        child: const Text('Quit CopyPaste'),
      ),
      AppUpdatePhase.idle ||
      AppUpdatePhase.upToDate ||
      AppUpdatePhase.error => Button.secondary(
        key: const ValueKey<String>('check-app-update'),
        onPressed: controller.check,
        leading: const Icon(LucideIcons.refreshCw),
        child: const Text('Check again'),
      ),
    };
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
  });

  final String title;
  final String description;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(title, style: Theme.of(context).typography.h3),
        const Gap(AppSpacing.xs),
        Text(description).muted(),
        const Gap(AppSpacing.lg),
        ...children,
      ],
    );
  }
}

class _SettingCard extends StatelessWidget {
  const _SettingCard({
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
    final copy = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title).medium(),
        const Gap(AppSpacing.xs),
        Text(description).muted().textSmall(),
      ],
    );
    return Card(
      theme: AppTheme.settingsSearchTargetCardTheme(
        context,
        highlighted: highlighted,
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final stack =
              constraints.maxWidth < 420 ||
              MediaQuery.textScalerOf(context).scale(1) > 1.3;
          if (stack) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                copy,
                const Gap(AppSpacing.md),
                Align(alignment: Alignment.centerRight, child: trailing),
              ],
            );
          }
          return Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Expanded(child: copy),
              const Gap(AppSpacing.lg),
              trailing,
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
    required this.shortcutKey,
    required this.autoPasteKey,
    required this.shortcutHighlighted,
    required this.autoPasteHighlighted,
  });

  final QuickPasteSettingsController controller;
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
          title: 'Quick Paste',
          description:
              'Open clipboard history from anywhere without switching windows.',
          children: [
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
              _SettingCard(
                key: shortcutKey,
                highlighted: shortcutHighlighted,
                title: 'Open Quick Paste',
                description:
                    'The shortcut works while CopyPaste is running in the background.',
                trailing: _ShortcutRecorder(controller: controller),
              ),
              const Gap(AppSpacing.md),
              _SettingCard(
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
                const Gap(AppSpacing.md),
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
                const Gap(AppSpacing.md),
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
              style: const ButtonStyle.ghostIcon(),
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

enum _SettingsSectionId {
  capture(
    label: 'Capture',
    slug: 'capture',
    description: 'Clipboard capture and application exclusions.',
    icon: LucideIcons.clipboard,
  ),
  storageData(
    label: 'Storage & Data',
    slug: 'storage-data',
    description: 'Retention, storage limits, export, and backup.',
    icon: LucideIcons.database,
  ),
  sync(
    label: 'Sync',
    slug: 'sync',
    description: 'Synchronization and nearby-device discovery.',
    icon: LucideIcons.refreshCw,
  ),
  feedback(
    label: 'Feedback',
    slug: 'feedback',
    description: 'Capture feedback and application updates.',
    icon: LucideIcons.bell,
  ),
  quickPaste(
    label: 'Quick Paste',
    slug: 'quick-paste',
    description: 'Shortcut and automatic paste behavior.',
    icon: LucideIcons.keyboard,
  );

  const _SettingsSectionId({
    required this.label,
    required this.slug,
    required this.description,
    required this.icon,
  });

  final String label;
  final String slug;
  final String description;
  final IconData icon;
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
  final _SettingsSectionId section;
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
  static const String androidBackgroundCapture =
      'result-android-background-capture';
  static const String clipboardCapture = 'result-clipboard-capture';
  static const String excludedApplications = 'result-excluded-applications';
  static const String retention = 'result-retention';
  static const String storageQuota = 'result-storage-quota';
  static const String historyFiles = 'result-history-files';
  static const String sync = 'result-sync';
  static const String lanVisibility = 'result-lan-visibility';
  static const String notificationOnCopy = 'result-notification-on-copy';
  static const String soundOnCopy = 'result-sound-on-copy';
  static const String applicationUpdates = 'result-application-updates';
  static const String quickPasteShortcut = 'result-quick-paste-shortcut';
  static const String quickPasteAutoPaste = 'result-quick-paste-auto-paste';
}
