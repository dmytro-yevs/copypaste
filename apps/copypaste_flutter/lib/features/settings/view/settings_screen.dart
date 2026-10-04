import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../platform/desktop/global_shortcut.dart';
import '../../../shared/state_view.dart';
import '../../../shared/system_date_time.dart';
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

  final _captureKey = GlobalKey();
  final _storageKey = GlobalKey();
  final _syncKey = GlobalKey();
  final _feedbackKey = GlobalKey();
  final _quickPasteKey = GlobalKey();
  final _cloudEmail = TextEditingController();
  final _cloudPassword = TextEditingController();
  final _cloudPassphrase = TextEditingController();

  @override
  void dispose() {
    _cloudEmail.dispose();
    _cloudPassword.dispose();
    _cloudPassphrase.dispose();
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
    return SingleChildScrollView(
      key: const PageStorageKey<String>('settings-scroll'),
      padding: const EdgeInsets.fromLTRB(
        AppSpacing.lg,
        AppSpacing.lg,
        AppSpacing.lg,
        AppSpacing.huge,
      ),
      child: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 900),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _anchors(),
              if (controller.noticeMessage case final message?) ...[
                const Gap(AppSpacing.lg),
                Alert(
                  leading: const Icon(LucideIcons.circleCheck),
                  title: const Text('Done'),
                  content: Text(message),
                ),
              ],
              if (controller.errorMessage case final message?) ...[
                const Gap(AppSpacing.lg),
                Alert.destructive(
                  leading: const Icon(LucideIcons.circleAlert),
                  title: const Text('Settings need attention'),
                  content: Text(message),
                ),
              ],
              const Gap(AppSpacing.xxxl),
              _captureSection(settings),
              const Gap(AppSpacing.huge),
              _storageSection(settings),
              const Gap(AppSpacing.huge),
              _syncSection(settings),
              const Gap(AppSpacing.huge),
              _feedbackSection(settings),
              if (widget.quickPaste != null) ...[
                const Gap(AppSpacing.huge),
                _QuickPasteSection(
                  key: _quickPasteKey,
                  controller: widget.quickPaste!,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _anchors() {
    final anchors = <(String, GlobalKey)>[
      ('Capture', _captureKey),
      ('Storage & Data', _storageKey),
      ('Sync', _syncKey),
      ('Feedback', _feedbackKey),
      if (widget.quickPaste != null) ('Quick Paste', _quickPasteKey),
    ];
    return Semantics(
      label: 'Settings sections',
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: Row(
          children: [
            for (var index = 0; index < anchors.length; index++) ...[
              Button.link(
                key: ValueKey<String>(
                  'settings-anchor-${anchors[index].$1.toLowerCase().replaceAll(RegExp(r'[^a-z]+'), '-')}',
                ),
                style: const ButtonStyle.link(
                  size: ButtonSize.small,
                  density: ButtonDensity.dense,
                ),
                onPressed: () => _scrollTo(anchors[index].$2),
                child: Text(anchors[index].$1),
              ),
              if (index < anchors.length - 1)
                const Text('·').muted().textSmall(),
            ],
          ],
        ),
      ),
    );
  }

  Widget _captureSection(RuntimeSettings settings) {
    final capture = widget.controller.capture;
    return _SettingsSection(
      key: _captureKey,
      title: 'Capture',
      description: 'Control clipboard capture and application exclusions.',
      children: [
        if (widget.onOpenAndroidCaptureSetup != null) ...[
          _SettingCard(
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
      key: _storageKey,
      title: 'Storage & Data',
      description: 'Set retention limits and manage local history files.',
      children: [
        _SettingCard(
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
    final cloud = widget.controller.cloud;
    return _SettingsSection(
      key: _syncKey,
      title: 'Sync',
      description: 'Control local-network and encrypted cloud synchronization.',
      children: [
        _SettingCard(
          title: 'Sync',
          description: 'Master switch for paired-device and cloud sync.',
          trailing: Switch(
            value: settings.syncEnabled,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setSyncEnabled,
          ),
        ),
        const Gap(AppSpacing.md),
        _SettingCard(
          title: 'LAN visibility',
          description: 'Allow nearby devices to discover this device.',
          trailing: Switch(
            value: settings.lanVisibility,
            onChanged: widget.controller.busy
                ? null
                : widget.controller.setLanVisibility,
          ),
        ),
        const Gap(AppSpacing.md),
        Card(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Text('Cloud sync').medium(),
              const Gap(AppSpacing.xs),
              const Text(
                'Your sync passphrase encrypts clipboard data before it reaches the cloud.',
              ).muted().textSmall(),
              const Gap(AppSpacing.lg),
              if (widget.controller.cloudErrorMessage case final message?)
                Alert.destructive(
                  leading: const Icon(LucideIcons.cloudOff),
                  title: const Text('Cloud sync is unavailable'),
                  content: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(message),
                      const Gap(AppSpacing.sm),
                      Align(
                        alignment: Alignment.centerRight,
                        child: Button.secondary(
                          onPressed: widget.controller.busy
                              ? null
                              : widget.controller.refreshCloud,
                          child: const Text('Retry'),
                        ),
                      ),
                    ],
                  ),
                )
              else if (cloud == null)
                const StateView.loading(message: 'Loading cloud status.')
              else if (!cloud.configured)
                const Alert(
                  leading: Icon(LucideIcons.cloudOff),
                  title: Text('Cloud sync is not configured'),
                  content: Text(
                    'This build does not have a cloud deployment configured.',
                  ),
                )
              else if (cloud.signedIn)
                _signedInCloud(cloud)
              else
                _signedOutCloud(),
            ],
          ),
        ),
      ],
    );
  }

  Widget _signedInCloud(CloudSettingsState cloud) {
    final sync = widget.controller.lastCloudSync;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _detailRow('Account', cloud.email ?? 'Signed in'),
        _detailRow(
          'Last sync',
          cloud.lastSync == null
              ? 'Never'
              : formatSystemDateTime(context, cloud.lastSync!),
        ),
        if (cloud.lastError case final error?) _detailRow('Last error', error),
        if (cloud.unreadableUploads > 0)
          _detailRow('Unreadable uploads', '${cloud.unreadableUploads}'),
        if (sync != null) ...[
          const Gap(AppSpacing.md),
          Text(
            '${sync.uploaded} uploaded · ${sync.applied} applied · ${sync.skippedTooLarge} kept locally',
          ).muted().textSmall(),
        ],
        const Gap(AppSpacing.lg),
        Wrap(
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          children: [
            Button.primary(
              onPressed: widget.controller.busy
                  ? null
                  : widget.controller.cloudSyncNow,
              leading: const Icon(LucideIcons.refreshCw),
              child: const Text('Sync now'),
            ),
            Button.secondary(
              onPressed: widget.controller.busy
                  ? null
                  : widget.controller.cloudSignOut,
              child: const Text('Sign out'),
            ),
          ],
        ),
      ],
    );
  }

  Widget _signedOutCloud() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TextField(
          controller: _cloudEmail,
          placeholder: const Text('Email'),
          keyboardType: TextInputType.emailAddress,
          features: const [InputFeature.leading(Icon(LucideIcons.mail))],
        ),
        const Gap(AppSpacing.sm),
        TextField(
          controller: _cloudPassword,
          placeholder: const Text('Account password'),
          obscureText: true,
          features: const [InputFeature.leading(Icon(LucideIcons.lockKeyhole))],
        ),
        const Gap(AppSpacing.sm),
        TextField(
          controller: _cloudPassphrase,
          placeholder: const Text('Sync passphrase'),
          obscureText: true,
          features: const [InputFeature.leading(Icon(LucideIcons.keyRound))],
        ),
        const Gap(AppSpacing.md),
        Wrap(
          spacing: AppSpacing.sm,
          runSpacing: AppSpacing.sm,
          children: [
            Button.primary(
              onPressed: widget.controller.busy
                  ? null
                  : () => _submitCloud(create: false),
              child: const Text('Sign in'),
            ),
            Button.secondary(
              onPressed: widget.controller.busy
                  ? null
                  : () => _submitCloud(create: true),
              child: const Text('Create account'),
            ),
          ],
        ),
      ],
    );
  }

  Widget _feedbackSection(RuntimeSettings settings) {
    return _SettingsSection(
      key: _feedbackKey,
      title: 'Feedback',
      description: 'Manage application updates and clipboard feedback.',
      children: [
        _SettingCard(
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

  Widget _detailRow(String label, String value) => Padding(
    padding: const EdgeInsets.only(top: AppSpacing.sm),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(child: Text(label).muted()),
        const Gap(AppSpacing.lg),
        Flexible(child: Text(value, textAlign: TextAlign.end)),
      ],
    ),
  );

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

  Future<void> _submitCloud({required bool create}) async {
    final email = _cloudEmail.text.trim();
    final password = _cloudPassword.text;
    final passphrase = _cloudPassphrase.text;
    if (email.isEmpty || password.isEmpty || passphrase.isEmpty) return;
    final succeeded = create
        ? await widget.controller.cloudSignUp(
            email: email,
            password: password,
            passphrase: passphrase,
          )
        : await widget.controller.cloudSignIn(
            email: email,
            password: password,
            passphrase: passphrase,
          );
    if (succeeded) {
      _cloudPassword.clear();
      _cloudPassphrase.clear();
    }
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
    required this.title,
    required this.description,
    required this.trailing,
  });

  final String title;
  final String description;
  final Widget trailing;

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
  const _QuickPasteSection({super.key, required this.controller});

  final QuickPasteSettingsController controller;

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
                title: 'Open Quick Paste',
                description:
                    'The shortcut works while CopyPaste is running in the background.',
                trailing: _ShortcutRecorder(controller: controller),
              ),
              const Gap(AppSpacing.md),
              _SettingCard(
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
