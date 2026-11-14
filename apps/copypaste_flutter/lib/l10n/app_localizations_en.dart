// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get commonCancel => 'Cancel';

  @override
  String get commonClose => 'Close';

  @override
  String get commonDone => 'Done';

  @override
  String get commonRetry => 'Retry';

  @override
  String get commonTryAgain => 'Try again';

  @override
  String get commonCopy => 'Copy';

  @override
  String get commonDelete => 'Delete';

  @override
  String get commonSave => 'Save';

  @override
  String get commonContinue => 'Continue';

  @override
  String get commonBack => 'Back';

  @override
  String get commonOpenSettings => 'Open settings';

  @override
  String get commonLoading => 'Loading';

  @override
  String get commonApply => 'Apply';

  @override
  String get commonClear => 'Clear';

  @override
  String get commonPin => 'Pin';

  @override
  String get commonEnable => 'Enable';

  @override
  String get settingsLanguageTitle => 'Language';

  @override
  String get settingsLanguageDescription =>
      'Choose the language of the CopyPaste interface.';

  @override
  String get settingsLanguageSystem => 'System default';

  @override
  String get settingsLanguageKeywords =>
      'language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Start at login could not be updated. Try again.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'System startup settings could not be opened.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Settings could not be refreshed. Try again.';

  @override
  String get settingsRuntimeLoadFailed => 'Settings could not be loaded.';

  @override
  String get settingsRuntimeAppAlreadyExcluded =>
      'That application is already excluded.';

  @override
  String get settingsRuntimeChangeFailed =>
      'The requested change could not be completed.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Clipboard capture permissions must be completed before capture can resume.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'Clipboard capture is paused. The Android capture service did not confirm that it stopped.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'Notification permission is required to enable capture notifications.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste could not read saved settings. Protective values are active; review and save your preferences to repair them.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste recovered unreadable settings with protective values. Review and save your preferences to repair them.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Application identifiers must be non-empty and 256 bytes or fewer.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'You can exclude up to 256 applications.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Auto-paste could not be updated.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'That shortcut could not be registered.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'The current shortcut could not be paused.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'The shortcut could not be restored.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'Quick Paste could not be opened.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Accessibility permission could not be requested.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Linux integration status could not be refreshed.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'Clipboard input permission was not granted.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Clipboard input permission could not be requested.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Desktop integration setup could not be opened.';

  @override
  String get settingsRuntimeTextHistoryFileType => 'CopyPaste text history';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'CopyPaste encrypted backup';

  @override
  String get settingsRuntimeNotificationImage => 'Image';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Image · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'File';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste could not restore the update installation.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'This Linux installation cannot update automatically.';

  @override
  String get updateMacosPackageMissing =>
      'This release has no signed macOS update package.';

  @override
  String get updateInstalledVersionInvalid =>
      'The installed CopyPaste version is invalid.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste could not reach GitHub to check for updates.';

  @override
  String get updateCheckTimedOut => 'The update check timed out.';

  @override
  String get updateCheckDeviceFailed =>
      'CopyPaste could not check for updates on this device.';

  @override
  String get updateCheckFailed => 'CopyPaste could not check for updates.';

  @override
  String get updateDownloadFailed => 'CopyPaste could not download the update.';

  @override
  String get updateDownloadTimedOut => 'The update download timed out.';

  @override
  String get updateInstallFailed => 'CopyPaste could not install the update.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Continue in the Android system installer.';

  @override
  String get updateContinueInPackageManager =>
      'Continue in your system package manager.';

  @override
  String get updateInstallerStarting => 'The installer is starting.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Allow CopyPaste to install apps in Android settings, then continue.';

  @override
  String get updatePermissionRequiredLinux =>
      'Authenticate in your system package manager to install this update.';

  @override
  String get updatePermissionRequired =>
      'Allow CopyPaste to install this update, then continue.';

  @override
  String get updateRestartRequired =>
      'The update is installed. Restart CopyPaste to use it.';

  @override
  String get updateRestarting => 'Restarting CopyPaste.';

  @override
  String get updateRestartFailed => 'CopyPaste could not restart. Try again.';

  @override
  String get updateReleasePageOpenFailed =>
      'CopyPaste could not open the release page.';

  @override
  String get updatePackageInvalid =>
      'The downloaded update is not a valid CopyPaste package.';

  @override
  String get updateSignatureInvalid =>
      'The downloaded update has an invalid signature.';

  @override
  String get updateDowngradeRefused =>
      'The downloaded package is not newer than this version.';

  @override
  String get updateHomebrewRequired =>
      'Install CopyPaste with Homebrew to update it here.';

  @override
  String get updateInstallationCancelled =>
      'The update installation was cancelled.';

  @override
  String get updateInstallationBlocked =>
      'Android blocked the update installation.';

  @override
  String get updateInstallationConflict =>
      'The update conflicts with the installed app.';

  @override
  String get updateInstallationIncompatible =>
      'The update is incompatible with this device.';

  @override
  String get updateInstallationStorage =>
      'Free up storage to install the update.';

  @override
  String get updateInstallationInterrupted =>
      'The update installation was interrupted. Try again.';

  @override
  String get updateInstallationBusy =>
      'An update installation is already in progress.';

  @override
  String get updatePackageDetailsInvalid =>
      'The update package details are invalid.';

  @override
  String get updateIntegrityCheckFailed =>
      'The downloaded update failed its integrity check.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste could not start the system installer.';

  @override
  String get updateInstallLocationTranslocated =>
      'Move CopyPaste to the Applications folder and open it from there to update.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste cannot replace itself in its current folder. Use the release page to update.';

  @override
  String get updateReplacementBlocked =>
      'macOS blocked replacing CopyPaste. Allow it in App Management settings or use the release page.';

  @override
  String get updatePackageNotDownloadable =>
      'This platform does not use a downloadable update package.';

  @override
  String get updatePackageSizeInvalid => 'The update package size is invalid.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'The downloaded update has an invalid updater signature.';

  @override
  String get updatePackageUrlUntrusted =>
      'The update package URL is not trusted.';

  @override
  String get updateDownloadLeftGitHub => 'The update download left GitHub.';

  @override
  String get updateDownloadTooManyRedirects =>
      'The update download used too many redirects.';

  @override
  String get updatePackageTooLarge => 'The update package is too large.';

  @override
  String get updateSignatureSizeInvalid =>
      'The updater signature size is invalid.';

  @override
  String get updateMetadataInvalid =>
      'GitHub returned invalid update metadata.';

  @override
  String get updateMetadataTooLarge => 'The update metadata is too large.';

  @override
  String get quickPasteActionFailedTitle => 'Action failed';

  @override
  String get quickPasteOpenAppSemantics => 'Open CopyPaste';

  @override
  String get quickPasteSearchPlaceholder => 'Type to search…';

  @override
  String get quickPasteLoadingHistory => 'Loading clipboard history.';

  @override
  String get quickPasteHistoryUnavailable => 'History is unavailable';

  @override
  String get quickPasteNoClipsFound => 'No clips found';

  @override
  String get quickPasteLoadMore => 'Load more';

  @override
  String get quickPasteTransferFailed => 'Failed';

  @override
  String get quickPasteClipUnavailable => 'Clip unavailable';

  @override
  String get quickPasteFileFallbackName => 'File';

  @override
  String get quickPastePathUnavailable => 'Path unavailable';

  @override
  String get quickPastePreferences => 'Preferences…';

  @override
  String get quickPasteAbout => 'About';

  @override
  String get quickPasteQuit => 'Quit';

  @override
  String get quickPasteClearHistoryTitle => 'Clear history?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Pinned clips will stay in your history.';

  @override
  String get quickPasteClearHistoryFailed => 'History could not be deleted.';

  @override
  String get quickPasteAboutDescription =>
      'Private clipboard history across macOS, Android, Windows, and Linux.';

  @override
  String get transfersTitle => 'Downloads and transfers';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count active transfers',
      one: '1 active transfer',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count active',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Latest';

  @override
  String get transfersShowEarlier => 'Earlier';

  @override
  String get transfersClearCompletedHistory => 'Clear completed history';

  @override
  String get transfersErrorTitle => 'Transfers';

  @override
  String get transfersEmptyTitle => 'No transfers';

  @override
  String get transfersEmptyMessage =>
      'File imports, downloads and outgoing transfers appear here.';

  @override
  String transfersSpeed(String size) {
    return '$size/s';
  }

  @override
  String transfersBytesProgress(String written, String total) {
    return '$written / $total';
  }

  @override
  String transfersProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String transfersStatusSize(String status, String size) {
    return '$status · $size';
  }

  @override
  String transfersStatusProgress(String status, String written, String total) {
    return '$status · $written / $total';
  }

  @override
  String get transfersRemoveFromHistory => 'Remove from history';

  @override
  String get transfersCancelDownload => 'Cancel download';

  @override
  String get transfersShowFile => 'Show file';

  @override
  String get transfersPhaseSending => 'Sending';

  @override
  String get transfersPhaseDownloading => 'Downloading';

  @override
  String get transfersPhaseAdding => 'Adding';

  @override
  String get transfersPhaseAdded => 'Added';

  @override
  String get transfersPhaseSent => 'Sent';

  @override
  String get transfersPhaseCompleted => 'Completed';

  @override
  String get transfersPhaseCancelled => 'Cancelled';

  @override
  String get transfersPhaseInterrupted => 'Interrupted';

  @override
  String get transfersPhaseFailed => 'Failed';

  @override
  String get transfersUpdatesUnavailable =>
      'Transfer updates are unavailable. Reopen to refresh.';

  @override
  String get transfersLoadFailed => 'Transfers could not be loaded. Try again.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Transfer updates could not be loaded.';

  @override
  String get transfersRetryFailed => 'This file could not be retried.';

  @override
  String get transfersClearHistoryFailed =>
      'Transfer history could not be cleared.';

  @override
  String get transfersRemoveRecordFailed =>
      'This history record could not be removed.';

  @override
  String get transfersCancelFailed =>
      'The transfer could not be cancelled. It may still be running.';

  @override
  String get transfersSavedFileUnavailable =>
      'The saved file is unavailable. It may have been moved or deleted.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Notifications · $count unread',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Notifications';

  @override
  String get notificationsClearRead => 'Clear read notifications';

  @override
  String get notificationsEmptyTitle => 'No notifications';

  @override
  String get notificationsEmptyMessage =>
      'Device connection requests and their results appear here.';

  @override
  String get notificationsSystemAlertTitle => 'System notifications';

  @override
  String get notificationsReview => 'Review';

  @override
  String get notificationsConnectionRequest => 'Connection request';

  @override
  String get notificationsConfirmConnection => 'Confirm connection';

  @override
  String get notificationsDeviceConnected => 'Device connected';

  @override
  String get notificationsConnectionRejected => 'Connection rejected';

  @override
  String get notificationsConnectionCancelled => 'Connection cancelled';

  @override
  String get notificationsConnectionRequestExpired =>
      'Connection request expired';

  @override
  String get notificationsConnectionFailed => 'Connection failed';

  @override
  String get notificationsNearbyDevice => 'Nearby device';

  @override
  String get notificationsPermissionDenied =>
      'Allow system notifications in your OS settings to receive requests in the background.';

  @override
  String get notificationsSystemUnavailable =>
      'System notifications are unavailable. Connection requests remain here.';

  @override
  String get platformCaptureStateUnavailable => 'Unavailable';

  @override
  String get platformCaptureStatePaused => 'Paused';

  @override
  String get platformCaptureStateRunning => 'Capturing';

  @override
  String get platformCaptureStateStopped => 'Not running';

  @override
  String get platformCaptureActionUnavailable => 'Capture unavailable';

  @override
  String get platformCaptureActionResume => 'Resume capture';

  @override
  String get platformCaptureActionPause => 'Pause capture';

  @override
  String get platformCaptureActionResumeStopped =>
      'Resume capture (not running)';

  @override
  String get platformTrayOpen => 'Open';

  @override
  String get platformTraySettings => 'Settings';

  @override
  String get platformTrayQuit => 'Quit';

  @override
  String get platformWindowControlsUnavailable =>
      'Desktop window controls are unavailable. CopyPaste will stay visible.';

  @override
  String get platformTrayUnavailable =>
      'The system tray is unavailable. Keep CopyPaste open, then retry or quit.';

  @override
  String get platformQuitFailed =>
      'CopyPaste could not quit. Please try again.';

  @override
  String get platformVisibilityChangeFailed =>
      'Application visibility could not be changed. Please try again.';

  @override
  String get platformCaptureNotificationTitle => 'Clipboard saved';

  @override
  String get platformCaptureNotificationBody =>
      'A new item was added to CopyPaste.';

  @override
  String get platformPairingNotificationTitle => 'Connection request';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName wants to connect. Open CopyPaste to compare the code and confirm.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Device connection requests';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Requests to connect nearby devices';

  @override
  String get platformNotificationDefaultAction => 'Open';

  @override
  String get platformFilesSelectionFailed => 'Files could not be selected.';

  @override
  String get platformFilesOnlyRegular => 'Only regular files can be imported.';

  @override
  String get platformModulesUnavailable =>
      'Modules are unavailable on this device.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'SMS access could not be configured.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'SMS access could not be verified.';

  @override
  String get platformUpdatesUnavailableHere =>
      'Application updates are unavailable here.';

  @override
  String get platformUpdateVersionUnavailable =>
      'The installed version is unavailable.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'Application updates are unavailable on this installation.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'This Linux installation cannot update automatically.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'This Linux update does not match the running architecture.';

  @override
  String get platformUpdateMacosUnsupported =>
      'This macOS installation cannot update automatically.';

  @override
  String get platformUpdateStartFailed =>
      'CopyPaste could not start the update.';

  @override
  String get platformGlobalShortcutDescription => 'Open Quick Paste';

  @override
  String get settingsSectionGeneral => 'General';

  @override
  String get settingsSectionGeneralSummary =>
      'Language and application visibility.';

  @override
  String get settingsSectionClipboard => 'Clipboard';

  @override
  String get settingsSectionClipboardSummary => 'Capture and history limits.';

  @override
  String get settingsSectionPrivacy => 'Privacy';

  @override
  String get settingsSectionPrivacySummary =>
      'Application exclusions and screen protection.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Shortcut and automatic paste behavior.';

  @override
  String get settingsSectionSync => 'Sync';

  @override
  String get settingsSectionSyncSummary =>
      'Synchronization and nearby-device discovery.';

  @override
  String get settingsSectionNotifications => 'Notifications';

  @override
  String get settingsSectionNotificationsSummary =>
      'Capture notifications and sounds.';

  @override
  String get settingsSectionData => 'Data';

  @override
  String get settingsSectionDataSummary => 'Export, backup, and restore.';

  @override
  String get settingsSectionModules => 'Modules';

  @override
  String get settingsSectionModulesSummary =>
      'Install and manage optional modules.';

  @override
  String get settingsSectionRules => 'Content rules';

  @override
  String get settingsSectionRulesSummary =>
      'Tag, skip, protect, or expire clips automatically.';

  @override
  String get settingsSectionQuickAccess => 'Quick access';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Keyboard, tiles, and home-screen widget.';

  @override
  String get settingsSectionSnippets => 'Snippets';

  @override
  String get settingsSectionSnippetsSummary => 'Reusable text and templates.';

  @override
  String get settingsSectionTransforms => 'Text transforms';

  @override
  String get settingsSectionTransformsSummary =>
      'Favorites, pipelines, and default paste transforms.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Collect several copies and paste them in order.';

  @override
  String pasteQueuePasted(int position, int total) {
    return 'Pasted $position of $total';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'Item $position of $total is on the clipboard';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Pasted $count items',
      one: 'Pasted 1 item',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items are on the clipboard',
      one: '1 item is on the clipboard',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Queue finished';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Added, $total in queue',
      one: 'Added, 1 in queue',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'Queue empty, nothing pasted';

  @override
  String get pasteQueueMixed =>
      'Paste all works with text-only or files-only queues';

  @override
  String get pasteQueueSeveralFiles => 'Paste files one at a time.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items no longer available',
      one: '1 item no longer available',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => 'Paste next copies only';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Item copied. Allow Accessibility to paste automatically.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Couldn\'t paste automatically. Item copied instead.';

  @override
  String get pasteQueueTargetElevated =>
      'Can\'t paste into an admin app. Item copied instead.';

  @override
  String get pasteQueueNoTarget => 'No app to paste into. Item copied instead.';

  @override
  String get pasteQueueCollectingStarted => 'Collecting copies';

  @override
  String get pasteQueueCollectingStopped => 'Stopped collecting';

  @override
  String get pasteQueueFailed => 'Couldn\'t complete the Paste Queue action';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pending',
      one: '1 pending',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pending · Collecting',
      one: '1 pending · Collecting',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Collecting';

  @override
  String get pasteQueueFinishedSummary => 'Finished';

  @override
  String pasteQueueFull(int max) {
    return 'Queue full ($max)';
  }

  @override
  String get pasteQueueFullMessage => 'New copies are no longer added.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue is empty';

  @override
  String get pasteQueueEmptyMessage =>
      'Collect copies or add items from history.';

  @override
  String get pasteQueueCollectingEmptyTitle => 'Copy items to add them';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Each copy joins the queue until you stop collecting.';

  @override
  String get pasteQueueStartCollecting => 'Start collecting';

  @override
  String get pasteQueueStopCollecting => 'Stop collecting';

  @override
  String get pasteQueueLoading => 'Loading the queue…';

  @override
  String get pasteQueueLoadFailed => 'Couldn\'t load the queue';

  @override
  String get pasteQueuePasteNext => 'Paste next';

  @override
  String get pasteQueueCopyNext => 'Copy next';

  @override
  String get pasteQueuePasteAll => 'Paste all';

  @override
  String get pasteQueueCopyAll => 'Copy all';

  @override
  String get pasteQueueReverse => 'Reverse order';

  @override
  String get pasteQueueStepBack => 'Step back';

  @override
  String get pasteQueueSkip => 'Skip';

  @override
  String get pasteQueueRemovePasted => 'Remove pasted';

  @override
  String get pasteQueueCleared => 'Queue cleared';

  @override
  String get pasteQueueUndo => 'Undo';

  @override
  String get pasteQueueStartFresh => 'Start fresh';

  @override
  String get pasteQueueAppend => 'Append';

  @override
  String get pasteQueueRestartTitle => 'Start a new collection?';

  @override
  String get pasteQueueRestartMessage =>
      'Start fresh removes the pasted items. Append keeps them.';

  @override
  String pasteQueueStale(String time) {
    return 'Queue from $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Pasted';

  @override
  String get pasteQueueStatusSkipped => 'Skipped';

  @override
  String get pasteQueueStatusUnavailable => 'No longer available';

  @override
  String get pasteQueueEdited => 'Edited';

  @override
  String get pasteQueueSecret => 'Secret';

  @override
  String get pasteQueueKindImage => 'Image';

  @override
  String get pasteQueueKindFiles => 'Files';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Rendered at $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Item $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'Moved to position $position';
  }

  @override
  String get pasteQueueMoveUp => 'Move up';

  @override
  String get pasteQueueMoveDown => 'Move down';

  @override
  String get pasteQueueMoveToTop => 'Move to top';

  @override
  String get pasteQueueMoveToBottom => 'Move to bottom';

  @override
  String get pasteQueueReorder => 'Drag to reorder';

  @override
  String get pasteQueueMoreActions => 'More actions';

  @override
  String get pasteQueuePasteNow => 'Paste now';

  @override
  String get pasteQueueEdit => 'Edit text';

  @override
  String get pasteQueueUseOriginal => 'Use original text';

  @override
  String get pasteQueueRemove => 'Remove from queue';

  @override
  String get pasteQueueEditTitle => 'Edit queued text';

  @override
  String get pasteQueueEditMessage => 'Edits apply only to this queue.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Set shortcuts';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Collect and paste without opening CopyPaste.';

  @override
  String get pasteQueueShortcutToggleCollecting => 'Start or stop collecting';

  @override
  String get pasteQueueShortcutPasteNextPlain => 'Paste next as plain text';

  @override
  String get pasteQueueShortcutDescription =>
      'A global shortcut that works in any app.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Another CopyPaste shortcut uses this combination. The previous one is kept.';

  @override
  String get pasteQueueShortcutInUse => 'Another app uses this combination.';

  @override
  String get pasteQueueShortcutRejected =>
      'The system refused this combination. The previous one is kept.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt types characters on many layouts. Choose another.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Global shortcuts aren\'t available in this session.';

  @override
  String get pasteQueueShortcutFailed => 'Couldn\'t set this shortcut.';

  @override
  String get pasteQueueEnableTitle => 'Enable Paste Queue';

  @override
  String get pasteQueueEnableDescription =>
      'When off, shortcuts and collecting stop.';

  @override
  String get pasteQueuePasteNextStyleTitle => 'Paste next does';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Paste automatically or only copy to the clipboard.';

  @override
  String get pasteQueuePasteIntoApp => 'Paste into the front app';

  @override
  String get pasteQueueClipboardOnly => 'Put on the clipboard only';

  @override
  String get pasteQueueInsertTitle => 'New items go to';

  @override
  String get pasteQueueInsertDescription =>
      'Where collected and added items join the queue.';

  @override
  String get pasteQueueInsertEnd => 'End (first in, first out)';

  @override
  String get pasteQueueInsertStart => 'Start (last in, first out)';

  @override
  String get pasteQueueEndBehaviorTitle => 'After the last item';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'What Paste next does once every item was pasted.';

  @override
  String get pasteQueueEndStop => 'Stop';

  @override
  String get pasteQueueEndLoop => 'Start over';

  @override
  String get pasteQueueEndKeepLast => 'Keep pasting the last item';

  @override
  String get pasteQueueSeparatorTitle => 'Paste all separator';

  @override
  String get pasteQueueSeparatorDescription =>
      'Placed between items when pasting all at once.';

  @override
  String get pasteQueueSeparatorNewLine => 'New line';

  @override
  String get pasteQueueSeparatorBlankLine => 'Blank line';

  @override
  String get pasteQueueSeparatorTab => 'Tab';

  @override
  String get pasteQueueSeparatorSpace => 'Space';

  @override
  String get pasteQueueSeparatorComma => 'Comma';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Comma and space';

  @override
  String get pasteQueueSeparatorNone => 'None';

  @override
  String get pasteQueueSeparatorCustom => 'Custom';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'Up to 32 characters; \\n and \\t allowed';

  @override
  String get pasteQueuePlainTextTitle => 'Always paste as plain text';

  @override
  String get pasteQueuePlainTextDescription =>
      'Drops formatting from queued items.';

  @override
  String get pasteQueueIdleStopTitle => 'Stop collecting after';

  @override
  String get pasteQueueIdleStopDescription =>
      'Stop collecting after this long without copies.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes min';
  }

  @override
  String get pasteQueueIdleNever => 'Never';

  @override
  String get pasteQueueMaxEntriesTitle => 'Queue size limit';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'When full, new copies aren\'t added.';

  @override
  String get pasteQueueRestoreTitle =>
      'Restore the previous clipboard when finished';

  @override
  String get pasteQueueRestoreDescription =>
      'Bring back what you copied before pasting.';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue collect sequence order shortcut hotkey separator stack';

  @override
  String get settingsSectionAbout => 'About';

  @override
  String get settingsSectionAboutSummary => 'Version and application updates.';

  @override
  String get settingsGeneralDescription =>
      'Language, automatic startup, and application visibility.';

  @override
  String get settingsModulesDescription => 'Optional features for CopyPaste.';

  @override
  String get settingsClipboardDescription =>
      'Capture clipboard changes and manage history limits.';

  @override
  String get settingsDataDescription =>
      'Export, back up, and restore clipboard history.';

  @override
  String get settingsSyncDescription =>
      'Control synchronization with paired devices.';

  @override
  String get settingsPrivacyDescription =>
      'Control which apps are captured and protect clipboard content.';

  @override
  String get settingsNotificationsDescription =>
      'Choose notifications and sounds for captured clips.';

  @override
  String get settingsAboutDescription =>
      'CopyPaste version and application updates.';

  @override
  String get settingsQuickPasteDescription =>
      'Open clipboard history from anywhere without switching windows.';

  @override
  String get settingsLoadingMessage => 'Loading settings.';

  @override
  String get settingsUnavailableTitle => 'Settings are unavailable';

  @override
  String get settingsNoResultsTitle => 'No settings found';

  @override
  String get settingsNoResultsMessage => 'Try a different search.';

  @override
  String get settingsAttentionTitle => 'Settings need attention';

  @override
  String get settingsSearchPlaceholder => 'Search settings';

  @override
  String get settingsBackToSettings => 'Back to Settings';

  @override
  String get settingsStartOnBootTitle => 'Start on device boot';

  @override
  String get settingsStartAtLoginTitle => 'Start at login';

  @override
  String get settingsStartupSearchDescription =>
      'Start CopyPaste automatically.';

  @override
  String get settingsStartupKeywords =>
      'startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Disabled in Task Manager. Enable CopyPaste in Settings → Apps → Startup.';

  @override
  String get settingsStartupMacApproval =>
      'Allow CopyPaste in System Settings → General → Login Items.';

  @override
  String get settingsStartupUnavailable =>
      'Automatic startup is unavailable in this installation.';

  @override
  String get settingsStartupAndroidDescription =>
      'Resume background services after a restart.';

  @override
  String get settingsStartupDesktopDescription =>
      'Open CopyPaste automatically when you sign in.';

  @override
  String get settingsShowInDock => 'Show in Dock';

  @override
  String get settingsShowInTaskbar => 'Show in taskbar';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Show the application icon for switching windows.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Show in menu bar';

  @override
  String get settingsShowInSystemTray => 'Show in system tray';

  @override
  String get settingsTrayVisibilityDescription =>
      'Show quick access to CopyPaste and capture controls.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle => 'Visibility could not be changed';

  @override
  String get settingsModulesSearchDescription =>
      'Install, update, and remove optional modules.';

  @override
  String get settingsModulesKeywords => 'features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Android background capture';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Full or Limited mode with Shizuku or ADB setup.';

  @override
  String get settingsAndroidCaptureKeywords => 'background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Full or Limited mode and one-time Shizuku or ADB setup.';

  @override
  String get settingsOpenSetup => 'Open setup';

  @override
  String get settingsSaveScreenshotsTitle => 'Save screenshots';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Automatically save new Android screenshots to History.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Automatically add new screenshots to History.';

  @override
  String get settingsScreenshotAccessTitle => 'Screenshot access';

  @override
  String get settingsScreenshotAccessDescription =>
      'Needed to save screenshots in the background.';

  @override
  String get settingsAllowAccess => 'Allow access';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'Screenshot capture is not running';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'New screenshots are not being saved. Resume capture to start the service again.';

  @override
  String get settingsResumeCapture => 'Resume capture';

  @override
  String get settingsClipboardCaptureTitle => 'Clipboard capture';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Pause or resume clipboard capture.';

  @override
  String get settingsCaptureStateUnavailable => 'State unavailable';

  @override
  String get settingsCaptureStoppedTitle => 'Capture is not running';

  @override
  String get settingsCaptureStoppedMessage =>
      'CopyPaste is not currently receiving clipboard changes on this device.';

  @override
  String get settingsRetentionTitle => 'Retention';

  @override
  String get settingsRetentionDescription =>
      'Automatically remove old unpinned clipboard items.';

  @override
  String get settingsRetentionForever => 'Keep forever';

  @override
  String get settingsStorageQuotaTitle => 'Storage quota';

  @override
  String get settingsStorageQuotaDescription =>
      'Maximum local storage used by unpinned history.';

  @override
  String get settingsStorageQuotaKeywords => 'disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Maximum file size';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Limit for adding and downloading files.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Largest file you can add or download.';

  @override
  String get settingsHistoryFilesTitle => 'History files';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Export history, create backups, or restore a backup.';

  @override
  String get settingsHistoryFilesKeywords => 'text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Export text history';

  @override
  String get settingsExportTextDescription =>
      'Save text clips in a portable file.';

  @override
  String get settingsExportAction => 'Export';

  @override
  String get settingsBackupTitle => 'Encrypted backup';

  @override
  String get settingsBackupDescription =>
      'Save the complete local history for this device.';

  @override
  String get settingsCreateBackupAction => 'Create backup';

  @override
  String get settingsRestoreTitle => 'Restore backup';

  @override
  String get settingsRestoreDescription =>
      'Replace local history with an encrypted backup.';

  @override
  String get settingsRestoreAction => 'Restore';

  @override
  String get settingsRestoreDialogTitle => 'Replace local history?';

  @override
  String get settingsRestoreDialogMessage =>
      'The backup replaces this device\'s history. Settings are kept.';

  @override
  String get settingsRestoreFailed => 'The backup could not be restored.';

  @override
  String get settingsSyncSwitchTitle => 'Sync';

  @override
  String get settingsSyncSwitchDescription =>
      'Sync history with paired devices.';

  @override
  String get settingsSyncSearchDescription => 'Paired-device synchronization.';

  @override
  String get settingsInstantClipboardTitle => 'Instant clipboard';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Automatically copy new clips from other devices.';

  @override
  String get settingsInstantClipboardKeywords =>
      'automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Copy new clips from other devices to this clipboard.';

  @override
  String get settingsLanVisibilityTitle => 'LAN visibility';

  @override
  String get settingsLanVisibilityDescription =>
      'Allow nearby devices to discover this device.';

  @override
  String get settingsLanVisibilityKeywords => 'local network discovery';

  @override
  String get settingsExcludedAppsTitle => 'Excluded applications';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Skip automatic capture from excluded applications.';

  @override
  String get settingsExcludedAppsKeywords => 'privacy app identifiers';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Skip automatic capture from these apps.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Automatic capture pauses while any app is excluded.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'Application exclusions are supported on macOS, Windows, Android, and Linux.';

  @override
  String get settingsExcludedAppsAdd => 'Add';

  @override
  String get settingsExcludedAppsEmpty => 'No applications are excluded.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return 'Remove $app';
  }

  @override
  String get applicationPickerSearch => 'Search applications';

  @override
  String get applicationPickerEmpty => 'No applications found';

  @override
  String get settingsSkipSecretTitle => 'Skip confidential clipboard';

  @override
  String get settingsSkipSecretDescription =>
      'Don\'t save copies that apps mark as secret.';

  @override
  String get settingsSkipSecretKeywords =>
      'secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Skip temporary clipboard';

  @override
  String get settingsSkipTransientDescription =>
      'Don\'t save copies that apps mark as temporary.';

  @override
  String get settingsSkipTransientKeywords => 'transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Block screenshots';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Prevent screenshots and screen recording of CopyPaste.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'Screenshot blocking is unavailable on this platform.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Notification on copy';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Show a notification after a background capture.';

  @override
  String get settingsNotificationPreviewTitle => 'Show clipboard content';

  @override
  String get settingsNotificationPreviewDescription =>
      'Include a clip preview in copy notifications.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Sound on copy';

  @override
  String get settingsSoundOnCopyDescription =>
      'Play platform feedback after a successful capture.';

  @override
  String get settingsVersionTitle => 'Version';

  @override
  String get settingsVersionDescription => 'Installed CopyPaste version.';

  @override
  String get settingsVersionUnavailable => 'Unavailable';

  @override
  String get settingsUpdatesTitle => 'Application updates';

  @override
  String get settingsUpdatesSearchDescription =>
      'Check for and install CopyPaste updates.';

  @override
  String get settingsUpdatesKeywords => 'version github release';

  @override
  String get settingsUpdateIdle => 'Check for a newer CopyPaste version.';

  @override
  String get settingsUpdateChecking => 'Checking for updates.';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste is up to date.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'Version $version is up to date.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'Version $available is available. This device has version $current.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Downloading version $version · $percent%.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Preparing installed module updates.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Installing version $version.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'Application updates need attention.';

  @override
  String get settingsUpdateNowAction => 'Update now';

  @override
  String get settingsUpdateOpenReleaseAction => 'Open release';

  @override
  String get settingsUpdateCheckingAction => 'Checking';

  @override
  String get settingsUpdateDownloadingAction => 'Downloading';

  @override
  String get settingsUpdateInstallingAction => 'Installing';

  @override
  String get settingsUpdateRestartAction => 'Restart CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => 'Check again';

  @override
  String get settingsUpdateNoneTitle => 'No updates available';

  @override
  String get settingsUpdateLatest => 'CopyPaste is the latest version.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version is the latest version.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Update available';

  @override
  String get settingsUpdateReady =>
      'A newer CopyPaste version is ready to install.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version is ready to install.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Update unavailable';

  @override
  String get settingsUpdateUnavailableMessage =>
      'This installation cannot update automatically.';

  @override
  String get settingsUpdateCheckFailedTitle => 'Update check failed';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'CopyPaste could not check for updates.';

  @override
  String get settingsQuickPasteSetupTitle => 'Set up Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Enable the signed GNOME or KDE companion and allow keyboard control to return to the previous app.';

  @override
  String get settingsQuickPasteDesktopIntegration => 'Desktop integration';

  @override
  String get settingsQuickPasteAllowKeyboard => 'Allow keyboard control';

  @override
  String get settingsQuickPasteCheckAgain => 'Check again';

  @override
  String get settingsQuickPasteLoading => 'Loading Quick Paste settings.';

  @override
  String get settingsQuickPasteUnavailableTitle => 'Quick Paste is unavailable';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'This device does not provide the Quick Paste window.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Open Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Configure the global Quick Paste shortcut.';

  @override
  String get settingsQuickPasteShortcutKeywords => 'keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'Works while CopyPaste runs in the background.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Registered shortcut: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Paste automatically';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Paste the selected clip into the previous app.';

  @override
  String get settingsAutoPasteKeywords => 'accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Selecting a clip returns to the previous app and pastes it.';

  @override
  String get settingsAccessibilityRequiredTitle => 'Accessibility is required';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Until permission is granted, selecting a clip copies it without pasting.';

  @override
  String get settingsQuickPasteAttentionTitle => 'Quick Paste needs attention';

  @override
  String get settingsShortcutRecording => 'Press shortcut…';

  @override
  String get settingsShortcutNotSet => 'Not set';

  @override
  String get settingsShortcutReset => 'Reset shortcut';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count days',
      one: '1 day',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Loading devices…';

  @override
  String get devicesUnavailableTitle => 'Devices are unavailable';

  @override
  String get devicesNearbyTitle => 'Nearby devices';

  @override
  String get devicesScan => 'Scan';

  @override
  String get devicesScanning => 'Scanning…';

  @override
  String get devicesNoneNearby => 'No devices nearby';

  @override
  String get devicesThisDevice => 'This device';

  @override
  String get devicesTrusted => 'Trusted';

  @override
  String get devicesConnect => 'Connect';

  @override
  String devicesCardPing(String ping) {
    return 'Ping: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Ping:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'Rename this device';

  @override
  String get devicesNamePlaceholder => 'Device name';

  @override
  String get devicesRenameFailed => 'The device name could not be saved.';

  @override
  String devicesRevokeDialogTitle(String device) {
    return 'Revoke $device?';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return 'Unpair $device?';
  }

  @override
  String get devicesRevokeDialogMessage =>
      'This removes the device from your group and permanently blocks this pairing. Other devices apply the removal when they reconnect.';

  @override
  String get devicesUnpairDialogMessage =>
      'This removes the device from your group. Other devices apply the removal when they reconnect. Adding it again requires a new confirmation.';

  @override
  String get devicesRevoke => 'Revoke';

  @override
  String get devicesUnpair => 'Unpair';

  @override
  String get devicesActionFailedMessage =>
      'The device action could not be completed.';

  @override
  String get devicesCloseDetails => 'Close device details';

  @override
  String get devicesDetailType => 'Device type';

  @override
  String get devicesDetailOperatingSystem => 'Operating system';

  @override
  String get devicesDetailModel => 'Model';

  @override
  String get devicesDetailAppVersion => 'CopyPaste version';

  @override
  String get devicesDetailProtocolVersion => 'Protocol version';

  @override
  String get devicesDetailStatus => 'Status';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => 'Measured';

  @override
  String get devicesDetailLastSeen => 'Last seen';

  @override
  String get devicesDetailLanEndpoint => 'LAN endpoint';

  @override
  String get devicesDetailDeviceId => 'Device ID';

  @override
  String get devicesDetailPairingId => 'Pairing ID';

  @override
  String get devicesDetailProfileProvenance => 'Profile provenance';

  @override
  String get devicesDetailProfileTrust => 'Profile trust';

  @override
  String get devicesDetailProfileUpdated => 'Profile updated';

  @override
  String get devicesValueUnavailable => 'Unavailable';

  @override
  String get devicesRenameDevice => 'Rename device';

  @override
  String get devicesSyncNow => 'Sync now';

  @override
  String get devicesRevokePairing => 'Revoke pairing';

  @override
  String get devicesLocal => 'Local';

  @override
  String get devicesTrustAuthenticated => 'Authenticated';

  @override
  String get devicesTrustUnverified => 'Unverified';

  @override
  String get devicesProvenanceSelfReported => 'Self-reported';

  @override
  String get devicesProvenanceObserved => 'Observed';

  @override
  String get devicesProvenanceMeasured => 'Measured';

  @override
  String get devicesConnectionOnline => 'Online';

  @override
  String get devicesConnectionOffline => 'Offline';

  @override
  String get devicesConnectionVisible => 'Visible';

  @override
  String get devicesConnectionNotSeen => 'Not seen';

  @override
  String get devicesConnectionUnknown => 'Unknown';

  @override
  String get devicesPeerAvailable => 'Available';

  @override
  String get devicesPeerVisibleNow => 'Visible now';

  @override
  String get devicesPeerStatusUnknown => 'Status unknown';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => 'Desktop';

  @override
  String get devicesClassLaptop => 'Laptop';

  @override
  String get devicesClassPhone => 'Phone';

  @override
  String get devicesClassTablet => 'Tablet';

  @override
  String get devicesClassUnknown => 'Device';

  @override
  String get devicesUnknownOs => 'Unknown OS';

  @override
  String get devicesAddDevice => 'Add device';

  @override
  String get devicesScanQr => 'Scan QR';

  @override
  String get devicesEnterCode => 'Enter code';

  @override
  String get devicesScannerUnavailableTitle => 'Scanner unavailable';

  @override
  String get devicesActionFailedTitle => 'Action failed';

  @override
  String get devicesRefreshFailedTitle => 'Device refresh failed';

  @override
  String get devicesNearbyPairingUnavailable =>
      'Nearby pairing is unavailable.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'Nearby pairing is unavailable. Use Add device to show a pairing code.';

  @override
  String get pairingTitle => 'Pairing';

  @override
  String get pairingPairDevice => 'Pair a device';

  @override
  String get pairingScanQrCode => 'Scan QR code';

  @override
  String get pairingEnterPairingCode => 'Enter pairing code';

  @override
  String get pairingConnectNearbyDevice => 'Connect nearby device';

  @override
  String get pairingSendingRequest => 'Sending connection request…';

  @override
  String get pairingEmptyTitle => 'Choose a pairing action';

  @override
  String get pairingEmptyMessage => 'Pairing controls appear here.';

  @override
  String get pairingCreatingInvitation => 'Creating a pairing invitation.';

  @override
  String get pairingCode => 'Pairing code';

  @override
  String get pairingCodeHint =>
      'Enter the 8-character code shown on the other device.';

  @override
  String get pairingDeviceAddress => 'Device address';

  @override
  String get pairingDeviceAddressHint =>
      'Use the host:port address reported by the other device.';

  @override
  String get pairingCheckDetailsTitle => 'Check pairing details';

  @override
  String get pairingActionFailed => 'Pairing action failed';

  @override
  String get pairingConnectingDevice => 'Connecting device…';

  @override
  String get pairingQrScannerTitle => 'QR scanner';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'The camera preview appears here.';

  @override
  String get pairingScannerNeedsAttention => 'Scanner needs attention';

  @override
  String get pairingCodeIncomplete =>
      'Enter the complete 8-character pairing code.';

  @override
  String get pairingAddressMissing => 'Enter the device host:port address.';

  @override
  String get pairingWaitingForOtherDevice => 'Waiting for the other device';

  @override
  String get pairingInviteInstructions =>
      'On the other device, choose Add device → Enter code, or scan the QR code.';

  @override
  String get pairingDetailsTitle => 'Pairing details';

  @override
  String get pairingAddress => 'Address';

  @override
  String get pairingVerificationCode => 'Verification code';

  @override
  String get pairingVerificationCodeHint =>
      'Confirm only if this code matches on both devices.';

  @override
  String get pairingReject => 'Reject';

  @override
  String get pairingConfirm => 'Confirm';

  @override
  String get pairingAccept => 'Accept';

  @override
  String get pairingJoinDevice => 'Join device';

  @override
  String get pairingStartScanner => 'Start scanner';

  @override
  String get pairingActionNotStarted =>
      'The pairing action could not be started.';

  @override
  String get pairingStateHandshaking => 'Verifying device';

  @override
  String get pairingStateAwaitingConfirmation => 'Confirm on both devices';

  @override
  String get pairingStateConfirmed => 'Device paired';

  @override
  String get pairingStateRejected => 'Pairing rejected';

  @override
  String get pairingStateCancelled => 'Pairing cancelled';

  @override
  String get pairingStateTimedOut => 'Pairing timed out';

  @override
  String get pairingStateFailed => 'Pairing failed';

  @override
  String get pairingHandshakingDescription =>
      'The devices are establishing a secure channel.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Compare the protected verification code, then confirm on both devices.';

  @override
  String get pairingConfirmedDescription =>
      'The device is connected. Your group membership will sync automatically.';

  @override
  String get pairingNoDeviceAdded => 'No device was added.';

  @override
  String get pairingTimedOutDescription =>
      'Start a new pairing attempt to continue.';

  @override
  String get pairingIdleDescription =>
      'Start a pairing attempt to connect a device.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'Protected pairing is unavailable';

  @override
  String get pairingProtectedUnavailableMessage =>
      'This pairing session must open in a dedicated protected window.';

  @override
  String get pairingCancel => 'Cancel pairing';

  @override
  String get pairingOrEnterCode => 'Or enter a pairing code';

  @override
  String get pairingJoinSecurely => 'Join securely';

  @override
  String get pairingProtectedWaitingDescription =>
      'Reveal a QR code, scan one, or enter a pairing code in this protected window.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Expires in ${seconds}s';
  }

  @override
  String get pairingInvalidQr => 'Scan a CopyPaste pairing QR code.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Google scanner is unavailable. Enter the pairing code instead.';

  @override
  String get pairingScannerCouldNotOpen =>
      'The scanner could not open. Enter the pairing code instead.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'Secure pairing presentation is unavailable.';

  @override
  String get pairingVerificationCodeRequired =>
      'Reveal and compare the verification code first.';

  @override
  String get syncStatusUnavailable => 'Sync state unavailable';

  @override
  String get syncStatusOff => 'Sync off';

  @override
  String get syncStatusWaitingForSync => 'Waiting for sync';

  @override
  String get syncStatusSyncing => 'Syncing';

  @override
  String get syncStatusUpToDate => 'Up to date';

  @override
  String get syncStatusInterrupted => 'Synchronization interrupted';

  @override
  String get syncStatusPartiallySynchronized => 'Partially synchronized';

  @override
  String get syncStatusRetrying => 'Retrying';

  @override
  String get syncStatusWaitingToSync => 'Waiting to sync';

  @override
  String get syncStatusWaitingForDevice => 'Waiting for device';

  @override
  String get syncStatusNoPairedDevices => 'No paired devices';

  @override
  String get syncStatusDevicesOffline => 'Devices offline';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · activity log unavailable';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Sync off · $count issues',
      one: 'Sync off · 1 issue',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count offline',
      one: '$status · 1 offline',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count devices need attention',
      one: '1 device needs attention',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Devices offline · $count issues',
      one: 'Devices offline · 1 issue',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Checking connection';

  @override
  String get syncConnectionUnknown => 'Connection unknown';

  @override
  String get syncLogDeviceOffline => 'Device went offline.';

  @override
  String get syncLogConnectionRestored => 'Connection restored.';

  @override
  String get syncLogProblemDetected => 'Synchronization problem detected.';

  @override
  String get syncLogProblemResolved => 'Synchronization problem resolved.';

  @override
  String get syncLogFailed => 'Synchronization failed.';

  @override
  String get syncLogRecovered => 'Synchronization restored.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent clips',
      one: '1 clip',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received clips',
      one: '1 clip',
    );
    return 'Sent $_temp0 · Received $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Synchronization: $status';
  }

  @override
  String get syncDetailsTitle => 'Synchronization';

  @override
  String get syncDetailsClose => 'Close synchronization details';

  @override
  String get syncNoPairedDevicesMessage =>
      'Pair a device to synchronize clipboard history.';

  @override
  String get syncActivityLogUnavailableTitle => 'Activity log unavailable';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Sync activity could not be read or saved.';

  @override
  String get syncDetailsDevicesHeading => 'Devices';

  @override
  String get syncDetailsActivityHeading => 'Activity';

  @override
  String get syncDetailsNoRecentProblems =>
      'No recent synchronization problems';

  @override
  String get syncDetailsUnknownPeer => 'Paired device';

  @override
  String get syncPeerConnection => 'Connection';

  @override
  String get syncPeerLastCheckStarted => 'Last check started';

  @override
  String get syncPeerItemsOverSizeLimit => 'Items over the size limit';

  @override
  String syncPeerLastSeen(String time) {
    return 'Last seen $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Last synced $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Last successful sync';

  @override
  String get syncPeerConnectionDetails => 'Connection details';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Connection details for $device';
  }

  @override
  String get historyKindText => 'Text';

  @override
  String get historyKindLink => 'Link';

  @override
  String get historyKindEmail => 'Email';

  @override
  String get historyKindColor => 'Color';

  @override
  String get historyKindPhone => 'Phone';

  @override
  String get historyKindCode => 'Code';

  @override
  String get historyKindPath => 'Path';

  @override
  String get historyKindCredentials => 'Credentials';

  @override
  String get historyKindImage => 'Image';

  @override
  String get historyKindFile => 'File';

  @override
  String get historyKindOther => 'Other';

  @override
  String get historySortNewest => 'Newest first';

  @override
  String get historySortOldest => 'Oldest first';

  @override
  String get historySortRelevance => 'Best match';

  @override
  String get historyErrorUpdatesUnavailable =>
      'History updates are temporarily unavailable.';

  @override
  String get historyErrorBulkDelete =>
      'Some selected clips could not be deleted. Try again.';

  @override
  String get historyErrorBulkPin =>
      'Some selected pins could not be updated. Try again.';

  @override
  String get historyErrorRefresh =>
      'History could not be refreshed. Try again.';

  @override
  String get historyErrorLoad => 'History could not be loaded. Try again.';

  @override
  String get historyErrorLoadMore => 'More history could not be loaded.';

  @override
  String get historyErrorLoadEarlier => 'Earlier history could not be loaded.';

  @override
  String get historyErrorOpenClip => 'This clip could not be opened.';

  @override
  String get historyErrorCopyPlainText => 'Plain text could not be copied.';

  @override
  String get historyErrorCopyClip => 'This clip could not be copied.';

  @override
  String get historyErrorCopyText => 'This text could not be copied.';

  @override
  String get historyErrorPrepareFile =>
      'This file could not be downloaded. Check the source device and try again.';

  @override
  String get historyErrorDownloadFile => 'This file could not be downloaded.';

  @override
  String get historyErrorSelectFiles =>
      'Files could not be selected. Try again.';

  @override
  String get historyErrorReadSelectedFile =>
      'The selected file could not be read. Check access and available storage.';

  @override
  String historyErrorImportFiles(
    int total,
    int failed,
    String name,
    String reason,
  ) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other:
          '$failed of $total files could not be imported. First: $name. $reason',
      one: '$failed of 1 file could not be imported. First: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'The clip secrecy could not be updated.';

  @override
  String get historyErrorPinUpdate => 'The pin could not be updated.';

  @override
  String get historyErrorPinnedOrderSave =>
      'The pinned order could not be saved. Try again.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'The order was saved, but history could not be refreshed.';

  @override
  String get historyErrorDeleteClip => 'This clip could not be deleted.';

  @override
  String get historyErrorDeleteAll => 'History could not be deleted.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'The selected clips could not be deleted.';

  @override
  String get historyOcrErrorUnavailable =>
      'OCR is unavailable. Check the module in Settings and try again.';

  @override
  String get historyOcrErrorStart => 'OCR could not be started. Try again.';

  @override
  String get historyOcrErrorRecognize =>
      'Image text could not be recognized. Try again.';

  @override
  String get historyOcrRecognizing => 'Recognizing image text.';

  @override
  String get historyOcrFailedTitle => 'OCR failed';

  @override
  String get historyOcrNoText => 'No text found';

  @override
  String get historyOcrTitle => 'Image text';

  @override
  String get historyOcrTextCopied => 'Recognized text copied.';

  @override
  String get historyOcrCopyError => 'Text could not be copied. Try again.';

  @override
  String get historyTagsErrorLoad => 'Tags could not be loaded. Try again.';

  @override
  String get historyTagsErrorSave =>
      'The tag change could not be saved. Use a unique name of up to 64 characters and select pinned clips.';

  @override
  String get historyTags => 'Tags';

  @override
  String get historyTagsUpdateErrorTitle => 'Could not update tags';

  @override
  String get historyTagsEmpty => 'No tags yet';

  @override
  String get historyTagRename => 'Rename';

  @override
  String get historyTagActions => 'Tag actions';

  @override
  String get historyTagNamePlaceholder => 'Tag name';

  @override
  String get historyCreateTag => 'Create tag';

  @override
  String get historySaveTag => 'Save tag';

  @override
  String get historyTagsFilterAll => 'All tags';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count tags',
      one: '1 tag',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Filter by tags: $selection';
  }

  @override
  String get historyManageTags => 'Manage tags';

  @override
  String get historyTagsFilterSemantics => 'Filter pinned clips by tags';

  @override
  String get historyDownloadErrorCancelled =>
      'The file download was cancelled.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'The download service is unavailable. Reopen CopyPaste and try again.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'The download service did not connect. Try again.';

  @override
  String get historyFileErrorAccess =>
      'The selected file could not be accessed.';

  @override
  String get historySpoilerRevealSemantics =>
      'Confidential content. Reveal spoiler';

  @override
  String get historyActionFailedTitle => 'Action failed';

  @override
  String get historyLoadingMessage => 'Loading clipboard history.';

  @override
  String get historyUnavailableTitle => 'History is unavailable';

  @override
  String get historyEmptyTitle => 'No clips found';

  @override
  String get historyEmptyMessage =>
      'Captured clips and matching history appear here.';

  @override
  String get historyAttentionTitle => 'History needs attention';

  @override
  String get historyLoadMore => 'Load more';

  @override
  String get historySectionPinned => 'Pinned';

  @override
  String get historySectionToday => 'Today';

  @override
  String get historySectionYesterday => 'Yesterday';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clipboard rows were skipped',
      one: '1 clipboard row was skipped',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Some retained clips could not be read and are not shown in this list.';

  @override
  String get historySearch => 'Search history';

  @override
  String get historyResetFilters => 'Reset filters';

  @override
  String get historyImportFiles => 'Import files';

  @override
  String get historySelectClips => 'Select clips';

  @override
  String get historyCloseSearch => 'Close search';

  @override
  String get historyFilterAllClips => 'All clips';

  @override
  String get historyFilterAllDevices => 'All devices';

  @override
  String get historyFilterAllApps => 'All apps';

  @override
  String get historyPinClip => 'Pin clip';

  @override
  String get historyUnpinClip => 'Unpin clip';

  @override
  String get historyReorderPinnedClip => 'Reorder pinned clip';

  @override
  String get historyReorderPinnedHint =>
      'Drag to move. Alt + Up or Down moves one position.';

  @override
  String get historyClipUnavailable => 'Clip unavailable';

  @override
  String historyClipKindSemantics(String kind) {
    return '$kind clip type';
  }

  @override
  String get historyInspectorEmptyTitle => 'Select a clip';

  @override
  String get historyInspectorEmptyMessage =>
      'Its full content and actions appear here.';

  @override
  String get historyNoPreview => 'No preview is available.';

  @override
  String get historyCloseClipDetails => 'Close clip details';

  @override
  String get historyCopyOptions => 'Copy options';

  @override
  String get historyCopyPlainText => 'Copy plain text';

  @override
  String get historyManageClipTags => 'Manage clip tags';

  @override
  String get historyPinnedButton => 'Pinned';

  @override
  String get historyAddSpoiler => 'Add spoiler';

  @override
  String get historyRemoveSpoiler => 'Remove spoiler';

  @override
  String get historyUpdateFailedTitle => 'Update failed';

  @override
  String get historyRecognizeImageText => 'Recognize image text';

  @override
  String get historyCopiedTitle => 'Copied';

  @override
  String get historyCopyFailedTitle => 'Copy failed';

  @override
  String get historyPlainTextCopied => 'Plain text copied.';

  @override
  String get historyClipCopied => 'Clip copied.';

  @override
  String get historyDownloadedTitle => 'Downloaded';

  @override
  String get historyFileSaved => 'File saved.';

  @override
  String get historyDownload => 'Download';

  @override
  String get historyDownloading => 'Downloading…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Downloading $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => 'Cancelling…';

  @override
  String get historyDownloadCancellingTooltip => 'Cancelling download';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Downloading $percent% · Click to cancel';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Download failed · Click to retry';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Cancel download, $percent% complete';
  }

  @override
  String get historyRetryDownload => 'Retry download';

  @override
  String get historyDownloadFailedTitle => 'Download failed';

  @override
  String get historyShowFile => 'Show file';

  @override
  String get historyMetadataPrivacy => 'Privacy';

  @override
  String get historyMetadataConfidential => 'Confidential';

  @override
  String get historyMetadataCaptured => 'Captured';

  @override
  String get historyMetadataObservedApp => 'Observed app';

  @override
  String get historyMetadataDevice => 'Device';

  @override
  String get historyMetadataClipType => 'Clip type';

  @override
  String get historyMetadataType => 'Type';

  @override
  String get historyMetadataPath => 'Path';

  @override
  String get historyMetadataFile => 'File';

  @override
  String get historyMetadataResolution => 'Resolution';

  @override
  String get historyMetadataSize => 'Size';

  @override
  String get historyMetadataFiles => 'Files';

  @override
  String get historyMetadataSync => 'Sync';

  @override
  String get historyMetadataTooLargeToSync => 'Too large to sync';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Delete $count selected clips?',
      one: 'Delete 1 selected clip?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Delete this clip?';

  @override
  String get historyBulkTagTooltip => 'Tag selected pinned clips';

  @override
  String get historyBulkExit => 'Exit selection';

  @override
  String get historyBulkPin => 'Pin selected';

  @override
  String get historyBulkUnpin => 'Unpin selected';

  @override
  String get historyBulkDelete => 'Delete selected';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count selected',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Filter by date';

  @override
  String get historyDatePresetToday => 'Today';

  @override
  String get historyDatePresetYesterday => 'Yesterday';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Last $days days',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'All time';

  @override
  String get appRuntimeStartFailed => 'CopyPaste could not start its runtime.';

  @override
  String get appPreparing => 'Preparing CopyPaste.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste is waiting for protected history access. The available parts of the app remain usable.';

  @override
  String get appHistoryUnavailableTitle => 'History runtime is unavailable';

  @override
  String get appHistoryUnavailableMessage =>
      'Start the application runtime before opening history.';

  @override
  String get appDevicesUnavailableTitle => 'Devices runtime is unavailable';

  @override
  String get appDevicesUnavailableMessage =>
      'Start the application runtime before opening devices.';

  @override
  String get appSettingsUnavailableTitle => 'Settings runtime is unavailable';

  @override
  String get appSettingsUnavailableMessage =>
      'Start the application runtime before opening settings.';

  @override
  String get appQuit => 'Quit CopyPaste';

  @override
  String get appWindowSetupTitle => 'Window setup needs attention';

  @override
  String get appOpenHistory => 'Open History';

  @override
  String get appDestinationHistory => 'History';

  @override
  String get appDestinationDevices => 'Devices';

  @override
  String get appDestinationSettings => 'Settings';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, selected';
  }

  @override
  String get appActiveFilter => 'Active filter';

  @override
  String get appActionFailed => 'Action failed';

  @override
  String get sharedAndroidVersionRequired => 'Android 11 or newer is required';

  @override
  String get sharedUseAdbTab => 'Use the ADB tab on this device.';

  @override
  String get sharedOneTimeAccessApplied => 'One-time access applied';

  @override
  String get sharedInstallShizuku => 'Install Shizuku';

  @override
  String get sharedPairAndStartShizuku => 'Pair and start Shizuku';

  @override
  String get sharedAllowCopyPaste => 'Allow CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded => 'Shizuku is no longer needed.';

  @override
  String get sharedGetShizukuDescription =>
      'Get Shizuku to apply one-time access.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Use Wireless debugging in Shizuku, then return.';

  @override
  String get sharedApproveInShizuku => 'Approve CopyPaste once in Shizuku.';

  @override
  String get sharedGetShizuku => 'Get Shizuku';

  @override
  String get sharedOpenShizuku => 'Open Shizuku';

  @override
  String get sharedRunOnComputer => 'Run on your computer';

  @override
  String get sharedCopyAllCommands => 'Copy all commands';

  @override
  String get sharedEnableUsbDebugging =>
      'Enable USB debugging and connect this phone.';

  @override
  String get sharedAllow => 'Allow';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Setup commands could not be copied.';

  @override
  String get sharedShizukuOpenFailed => 'Shizuku could not be opened.';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => 'Set up CopyPaste';

  @override
  String get onboardingWelcomeTitle => 'Welcome to CopyPaste';

  @override
  String get onboardingWelcomeSubtitle => 'Your clipboard, ready when you are.';

  @override
  String get onboardingReadyTitle => 'CopyPaste is ready';

  @override
  String get onboardingGetStarted => 'Get started';

  @override
  String get onboardingOptionalSetupTitle => 'Optional setup needs attention';

  @override
  String get onboardingOpenSystemSettings => 'Open Settings';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return 'STEP $step OF $count · $platform';
  }

  @override
  String get onboardingSetupErrorTitle => 'Setup needs attention';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste could not read the $platform setup state.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste could not save onboarding progress.';

  @override
  String get onboardingStartAtLogin => 'Start at login';

  @override
  String get onboardingStartAtLoginDescription =>
      'Open CopyPaste when you sign in';

  @override
  String get onboardingStartAtLoginSemantics => 'Start CopyPaste at login';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Start at login could not be enabled. You can continue without it.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Start at login could not be updated. You can continue without it.';

  @override
  String get onboardingMacosTitle => 'Set up your Mac';

  @override
  String get onboardingAccessibility => 'Accessibility';

  @override
  String get onboardingAutoPasteEnabled => 'Auto-paste enabled';

  @override
  String get onboardingAutoPasteOptional => 'Optional for auto-paste';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Available in the installed CopyPaste app.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'Login Item approval is required';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Allow CopyPaste in System Settings, then return.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste could not read the macOS setup state.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Accessibility permission could not be requested.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste could not refresh the macOS setup state.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Accessibility status could not be refreshed.';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Login Items settings could not be opened.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'Start at login requires approval in System Settings.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Start at login could not be disabled.';

  @override
  String get onboardingWindowsTitle => 'Set up your PC';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'Unavailable in this installation. You can continue without it.';

  @override
  String get onboardingTaskManagerDisabledTitle => 'Disabled in Task Manager';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Enable CopyPaste in Windows startup apps, then return.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'Start at login is disabled in Task Manager. You can enable it in Windows Settings later.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'Start at login is unavailable. You can continue without it.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste could not read Windows setup state.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Windows startup settings could not be opened.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Start at login could not be updated. You can change it later in Settings.';

  @override
  String get onboardingLinuxChecking => 'Checking your Linux desktop session.';

  @override
  String get onboardingLinuxTitle => 'Set up your Linux desktop';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste needs verified desktop integration for clipboard attribution and Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Desktop session';

  @override
  String get onboardingLinuxX11Detected => 'X11 session detected.';

  @override
  String get onboardingLinuxWaylandDetected => 'Wayland session detected.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'No supported X11 or Wayland desktop session was detected.';

  @override
  String get onboardingLinuxCompanion => 'GNOME or KDE companion';

  @override
  String get onboardingLinuxCompanionReady =>
      'The authenticated Clipboard v2 bridge is ready.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'Clipboard capture is paused by source app exclusions. Enable the signed companion to resume it.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Install and enable the signed companion session package, then return here.';

  @override
  String get onboardingLinuxOpenSetup => 'Open setup';

  @override
  String get onboardingLinuxKeyboardControl => 'Keyboard control';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'Keyboard control is allowed for Quick Paste.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'Keyboard control is unavailable in this desktop session.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Allow keyboard control';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste is ready.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Quick Paste will be ready after the required desktop integration is active.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'Open CopyPaste when you sign in.';

  @override
  String get onboardingLinuxLinks => 'CopyPaste links';

  @override
  String get onboardingLinuxLinksRegistered =>
      'CopyPaste links open in this app.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Register CopyPaste links for pairing invitations.';

  @override
  String get onboardingLinuxRegisterLinks => 'Register links';

  @override
  String get onboardingLinuxScreenshotBlocking => 'Screenshot blocking';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Screenshot blocking is unavailable on Linux. Protected stored data remains encrypted.';

  @override
  String get onboardingLinuxCheckAgain => 'Check again';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste could not read Linux integration status.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Desktop integration setup could not be opened.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'Keyboard control permission was not granted.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Start at login could not be updated.';

  @override
  String get onboardingLinuxLinksFailed =>
      'CopyPaste links could not be registered.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Linux integration status could not be refreshed.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'Clipboard access is unavailable in this X11 session.';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste requires an X11 or Wayland desktop session.';

  @override
  String get onboardingAndroidFullCapture => 'Full capture';

  @override
  String get onboardingAndroidBackgroundCapture => 'Background capture';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Set up once, then verify a real copy.';

  @override
  String get onboardingAndroidLimitedCapture => 'Limited capture';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'Share to CopyPaste or return after copying.';

  @override
  String get onboardingAndroidNotification => 'Capture notification';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Required while capture runs';

  @override
  String get onboardingAndroidBackgroundActivity => 'Background activity';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Recommended to reduce capture loss';

  @override
  String get onboardingAndroidOpen => 'Open';

  @override
  String get onboardingAndroidApplyAccess => 'Apply capture access';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Apply one-time capture access.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'You can set up Full capture later.';

  @override
  String get onboardingAndroidCaptureWorking => 'Background capture is working';

  @override
  String get onboardingAndroidVerifyCopy => 'Verify a real copy';

  @override
  String get onboardingAndroidCopyReached =>
      'A new copy reached your local History.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Copy in another app, then return here.';

  @override
  String get onboardingAndroidStartCapture => 'Start capture';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste could not read Android setup state.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Android capture state could not be refreshed.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Android setup monitoring is unavailable.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'Clipboard intake could not be enabled.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Background capture could not be stopped.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Battery settings could not be opened.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste is allowed in Shizuku, but Android capture grants could not be applied.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Allow CopyPaste in Shizuku to apply capture grants.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Grant background clipboard access first.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Allow notifications before starting background capture.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Background capture could not be started.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Android capture setup could not be opened.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Android setup choice could not be saved.';

  @override
  String get modulesMarketplaceTab => 'Marketplace';

  @override
  String get modulesInstalledTab => 'Installed';

  @override
  String get modulesRefresh => 'Refresh';

  @override
  String get modulesSearchPlaceholder => 'Search modules';

  @override
  String get modulesOperationFailed => 'Module operation failed';

  @override
  String get modulesLoading => 'Loading modules.';

  @override
  String get modulesUnavailableTitle => 'Modules are unavailable';

  @override
  String get modulesUnavailableMessage => 'Try loading modules again.';

  @override
  String get modulesNoneInstalled => 'No modules installed';

  @override
  String get modulesNoMatches => 'No matching modules';

  @override
  String get modulesChooseFromMarketplace =>
      'Choose a module from the marketplace.';

  @override
  String get modulesTryDifferentSearch => 'Try a different search.';

  @override
  String get modulesMarketplaceLoading => 'Loading marketplace.';

  @override
  String get modulesMarketplaceUnavailableTitle => 'Marketplace is unavailable';

  @override
  String get modulesMarketplaceUnavailableMessage =>
      'Try loading the marketplace again.';

  @override
  String get modulesNoneAvailable => 'No modules available';

  @override
  String get modulesNoneAvailableMessage =>
      'Modules will appear here when they are published.';

  @override
  String modulesEnableModule(String module) {
    return 'Enable $module';
  }

  @override
  String get modulesInstall => 'Install';

  @override
  String modulesUpdateTo(String version) {
    return 'Update to $version';
  }

  @override
  String get modulesRemoveModule => 'Remove module';

  @override
  String get modulesModuleSettings => 'Module settings';

  @override
  String modulesSettingsTitle(String module) {
    return '$module settings';
  }

  @override
  String get modulesDownloadingModel => 'Downloading language model…';

  @override
  String get modulesRun => 'Run';

  @override
  String modulesRemoveTitle(String module) {
    return 'Remove $module?';
  }

  @override
  String get modulesRemoveMessage =>
      'The module and its local settings will be removed.';

  @override
  String get modulesRemove => 'Remove';

  @override
  String get modulesRemoveFailed => 'The module could not be removed.';

  @override
  String get modulesCloseSettings => 'Close module settings';

  @override
  String get modulesRemoved => 'Module removed';

  @override
  String get modulesNoSettingsTitle => 'No additional settings';

  @override
  String get modulesNoSettingsMessage =>
      'This module does not need extra configuration.';

  @override
  String get modulesSetUpSmsAccess => 'Set up SMS access';

  @override
  String get modulesRestartApp => 'Restart CopyPaste';

  @override
  String get modulesSmsAccessTitle => 'SMS access';

  @override
  String get modulesCloseSmsAccess => 'Close SMS access';

  @override
  String get modulesSmsChecking => 'Checking SMS access.';

  @override
  String get modulesSmsReady =>
      'Access is ready. Enable SMS Codes to start copying new codes.';

  @override
  String get modulesSmsAllowAccess =>
      'Allow access to copy codes from new SMS messages.';

  @override
  String get modulesSmsNotification => 'SMS notification';

  @override
  String get modulesSmsNotificationDescription =>
      'Required while SMS Codes runs';

  @override
  String get modulesSmsApplyAccess => 'Apply SMS access';

  @override
  String get modulesSmsApplyAccessDescription => 'Apply one-time SMS access.';

  @override
  String get modulesSmsSetupFailed => 'SMS setup failed';

  @override
  String get modulesFileSelectionFailed => 'File selection failed';

  @override
  String get modulesChooseLanguages => 'Choose languages';

  @override
  String get modulesChooseFile => 'Choose file';

  @override
  String modulesModelReady(String model) {
    return '$model model is ready';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return '$model model · $size MiB download';
  }

  @override
  String get modulesPlatformsUnknown => 'Platforms: Unknown';

  @override
  String modulesPlatforms(String platforms) {
    return 'Platforms: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'The module catalog signature is invalid.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Could not connect to the module marketplace.';

  @override
  String get modulesMarketplaceTimeout =>
      'The module marketplace did not respond.';

  @override
  String get modulesIncompatible =>
      'This module is not compatible with this device.';

  @override
  String get modulesPackageSizeInvalid => 'The module package size is invalid.';

  @override
  String get modulesDownloadInterrupted =>
      'The module download was interrupted.';

  @override
  String get modulesDownloadTimeout => 'The module download did not respond.';

  @override
  String get modulesPackageUrlUntrusted =>
      'The module package URL is not trusted.';

  @override
  String get modulesDownloadLeftGithub => 'The module download left GitHub.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'The module download used too many redirects.';

  @override
  String get modulesDownloadTooLarge => 'The module download is too large.';

  @override
  String get modulesPackageIntegrityFailed =>
      'The module package failed its integrity check.';

  @override
  String get modulesDownloadUnavailable =>
      'This module download is currently unavailable.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'The module marketplace is unavailable. Try again.';

  @override
  String get modulesCatalogInvalid => 'The module catalog is invalid.';

  @override
  String get modulesThisSystem => 'this system';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version or newer';
  }

  @override
  String get modulesNotAvailableForDevice => 'Not available for this device.';

  @override
  String get modulesAppVersionUnverified =>
      'The installed CopyPaste version could not be verified.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'This module does not support CopyPaste $version.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Requires $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed =>
      'The module marketplace could not be loaded.';

  @override
  String get modulesSmsSetupRequired =>
      'Set up SMS access before enabling this module.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'SMS monitoring could not be started. The module is still enabled; disable it again to retry.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'SMS monitoring could not be started. The module was disabled.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'SMS monitoring could not be started. The module enabled state could not be confirmed.';

  @override
  String get modulesSmsMonitoringFailed =>
      'SMS monitoring could not be started.';

  @override
  String get modulesSmsAndroidOnly =>
      'SMS access is available only on Android.';

  @override
  String get modulesSmsMonitoringNeedsSetup => 'SMS monitoring needs setup.';

  @override
  String get modulesOperationIncomplete =>
      'The module operation could not be completed.';

  @override
  String get modulesUnsupportedEvent => 'The module uses an unsupported event.';

  @override
  String get modulesUnsupportedResult =>
      'The module returned an unsupported result.';

  @override
  String get modulesUnsupportedField => 'The module uses an unsupported field.';

  @override
  String get modulesResponseInvalid => 'The module response is invalid.';

  @override
  String get modulesFileSelectionUnavailable =>
      'File selection is unavailable.';

  @override
  String get modulesFileSelectFailed => 'The file could not be selected.';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field is not an accepted file.';
  }

  @override
  String get modulesSmsVerifyFailed => 'SMS access could not be verified.';

  @override
  String get modulesSmsAllowNotifications =>
      'Allow notifications to keep SMS Codes active in the background.';

  @override
  String get modulesSmsNotGranted => 'SMS access was not granted.';

  @override
  String get modulesSmsConfigureFailed => 'SMS access could not be configured.';

  @override
  String get onboardingLinuxContinueBasic => 'Continue with basic capture';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Optional. Needed for Quick Paste and source app exclusions.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Optional. Needed for Quick Paste to return to the previous app.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'Clipboard access is unavailable in this Wayland session. Enable the signed GNOME or KDE companion, then check again.';

  @override
  String get historyMoreActions => 'More actions';

  @override
  String get historyTagsAction => 'Tags…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'unlock secret items',
      'other': 'Unlock secret items',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'use a concealed snippet',
      'other': 'Use a concealed snippet',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'copy a secret item',
      'other': 'Copy a secret item',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'Not available on this system';

  @override
  String get deviceAuthNotConfigured =>
      'Set up a device password, PIN or biometrics first';

  @override
  String get commonOn => 'On';

  @override
  String get commonOff => 'Off';

  @override
  String get settingsQuickPasteStartFailed =>
      'Quick Paste could not start, so its shortcut does not work.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut is used by another app. Choose another shortcut.';
  }

  @override
  String get devicesEmptyTitle => 'No other devices paired yet';

  @override
  String get devicesEmptyMessage =>
      'Pair a device on the same network to sync history.';

  @override
  String get pairingStateNotCompleted => 'Pairing not completed';

  @override
  String get pairingStepInvite => 'Invite';

  @override
  String get pairingStepScan => 'Scan';

  @override
  String get pairingStepCode => 'Code';

  @override
  String get pairingStepRequest => 'Request';

  @override
  String get pairingStepConnect => 'Connect';

  @override
  String get pairingStepConfirm => 'Confirm';

  @override
  String get pairingStepPaired => 'Paired';

  @override
  String get pairingOtherDevice => 'Other device';

  @override
  String get pairingDecisionNotConfirmed => 'Not confirmed yet';

  @override
  String get pairingDecisionSending => 'Sending your decision…';

  @override
  String get pairingDecisionConfirmed => 'Confirmed';

  @override
  String get pairingDecisionRejected => 'Rejected';

  @override
  String get pairingDecisionWaiting => 'Waiting for confirmation';

  @override
  String get pairingDecisionUnknown => 'Answer not known';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count devices',
      one: '1 device',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count lines',
      one: '1 line',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Main navigation';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Step $step of $count: $name';
  }

  @override
  String get modulesPhaseDownload => 'Download';

  @override
  String get modulesPhaseInstall => 'Install';

  @override
  String modulesInstallingModule(String module) {
    return 'Installing $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return 'Updating $module';
  }

  @override
  String get commonDismiss => 'Dismiss';

  @override
  String get settingsRetentionConfirmTitle => 'Remove older history?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'New limit: $limit. Older unpinned items are deleted permanently.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle => 'Reduce the storage quota?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'New quota: $size. The oldest unpinned items over it are deleted permanently.';
  }

  @override
  String get settingsLimitConfirmAction => 'Apply limit';

  @override
  String get settingsHideEverywhereTitle => 'Hide CopyPaste completely?';

  @override
  String get settingsHideEverywhereMessage =>
      'With both off, there\'s no icon. To open it, start CopyPaste again.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'With both off, there\'s no icon. To open it, press $shortcut or start CopyPaste again.';
  }

  @override
  String get settingsHideEverywhereAction => 'Hide';

  @override
  String get settingsExportDoneTitle => 'Text history exported';

  @override
  String get settingsBackupDoneTitle => 'Backup created';

  @override
  String get settingsRestoreDoneTitle => 'History restored';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Exported $count text clips',
      one: 'Exported 1 text clip',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count could not be read',
      one: '1 could not be read',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Encrypted backup saved · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'The clipboard history on this device now matches the backup.';

  @override
  String sharedSizeKibibytes(String size) {
    return '$size KiB';
  }

  @override
  String sharedSizeMebibytes(String size) {
    return '$size MiB';
  }

  @override
  String sharedSizeGibibytes(String size) {
    return '$size GiB';
  }

  @override
  String storageUsageShort(String used, String quota) {
    return '$used of $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return '$used used of $quota';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '$used used';
  }

  @override
  String get storageUsageOverQuota => 'Over quota';

  @override
  String storageUsageOldestClip(String date) {
    return 'Oldest clip $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Storage: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'New quota: $size. $used is in use, so the oldest unpinned items are deleted permanently.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips',
      one: '1 clip',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'Open CopyPaste';

  @override
  String get trayOpenQuickPaste => 'Open Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Open Quick Paste ($shortcut)';
  }

  @override
  String get trayNewSnippet => 'New snippet';

  @override
  String get trayRecent => 'Recent';

  @override
  String get trayRecentLoading => 'Loading…';

  @override
  String get trayRecentFailed => 'Couldn\'t load clips';

  @override
  String get trayRecentEmpty => 'No clips yet';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count recent clips hidden',
      one: '1 recent clip hidden',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Secret';

  @override
  String get trayCopyLatestFromOtherDevice => 'Copy latest from other device';

  @override
  String get trayCaptureNow => 'Capture now';

  @override
  String trayPausedUntil(String time) {
    return 'Paused until $time';
  }

  @override
  String get trayPauseFor => 'Pause for…';

  @override
  String get trayPauseFor15Minutes => '15 minutes';

  @override
  String get trayPauseForOneHour => '1 hour';

  @override
  String get trayPauseUntilResumed => 'Until I resume';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count items',
      one: 'Paste Queue: 1 item',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Collecting: $count items',
      one: 'Collecting: 1 item',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Recently skipped ($count)';
  }

  @override
  String get trayOpenContentRules => 'Open content rules';

  @override
  String get trayRecentSettingsTitle => 'Menu bar and tray';

  @override
  String get trayRecentSettingsDescription =>
      'What the menu bar or tray menu lists under Recent.';

  @override
  String get trayRecentCountTitle => 'Recent clips';

  @override
  String get trayRecentCountDescription =>
      'How many of the newest clips the menu lists.';

  @override
  String get trayRecentPreviewTitle => 'Recent clip previews';

  @override
  String get trayRecentPreviewDescription =>
      'How much clip text the menu shows.';

  @override
  String get trayRecentPreviewFull => 'Full preview';

  @override
  String get trayRecentPreviewTitles => 'Type and source only';

  @override
  String get trayRecentPreviewHidden => 'Hidden';

  @override
  String get trayRecentSettingsKeywords =>
      'tray menu bar recent preview privacy count';

  @override
  String get transformsPaletteTitle => 'Transform text';

  @override
  String get transformsFilterPlaceholder => 'Filter transforms';

  @override
  String get transformsNoMatches => 'No transforms match';

  @override
  String get transformsGroupResults => 'Results';

  @override
  String get transformsGroupSuggested => 'Suggested';

  @override
  String get transformsGroupRecent => 'Recent';

  @override
  String get transformsGroupFavorites => 'Favorites';

  @override
  String get transformsGroupPipelines => 'Pipelines';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV and XML';

  @override
  String get transformsCategoryEncode => 'Encode and decode';

  @override
  String get transformsCategoryCase => 'Case';

  @override
  String get transformsCategoryHash => 'Hashes';

  @override
  String get transformsCategoryLines => 'Lines and whitespace';

  @override
  String get transformsCategoryClean => 'Clean up';

  @override
  String get transformsCategoryUrl => 'Links';

  @override
  String get transformsCategoryMarkup => 'Rich text and Markdown';

  @override
  String get transformsCategoryDev => 'Developer';

  @override
  String get transformsCategoryOther => 'Other';

  @override
  String get transformsChainEmpty => 'No steps yet';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Then: $name';
  }

  @override
  String get transformsStepEnabled => 'Run this step';

  @override
  String get transformsMoveEarlier => 'Move up';

  @override
  String get transformsMoveLater => 'Move down';

  @override
  String get transformsRemoveStep => 'Remove step';

  @override
  String get transformsSavePipeline => 'Save as pipeline';

  @override
  String get transformsSavePipelineTitle => 'Save pipeline';

  @override
  String get transformsSavePipelineFailed => 'The pipeline could not be saved.';

  @override
  String get transformsPipelineNameLabel => 'Name';

  @override
  String get transformsPipelineDescriptionLabel => 'Description (optional)';

  @override
  String get transformsPipelineStepsLabel => 'Steps';

  @override
  String get transformsPipelineNoSteps => 'Add at least one step.';

  @override
  String get transformsPipelineNameRequired => 'Enter a name.';

  @override
  String get transformsPipelineNameTooLong => 'Use at most 80 characters.';

  @override
  String get transformsPipelineNameTaken =>
      'Another pipeline already has this name.';

  @override
  String get transformsPipelineSavedTitle => 'Pipeline saved';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name is available in every transform palette.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Steps: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Deleted pipeline';

  @override
  String get transformsNeedsNewerVersion => 'Needs a newer CopyPaste version';

  @override
  String get transformsLoadFailed => 'Transforms could not be loaded.';

  @override
  String get transformsNotTextTitle => 'Transforms work on text';

  @override
  String get transformsNotTextMessage =>
      'This item is an image, a file or a color. Pick a text item or copy some text.';

  @override
  String get transformsNothingTitle => 'Nothing to transform';

  @override
  String get transformsNothingMessage =>
      'Copy some text or pick a history item.';

  @override
  String get transformsPickTransform => 'Pick a transform';

  @override
  String get transformsPickTransformHint =>
      'Tab adds it as a step. Enter applies the result.';

  @override
  String get transformsPreviewing => 'Updating preview…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Lines: $lines · Words: $words · Characters: $chars';
  }

  @override
  String get transformsNoChanges => 'No changes';

  @override
  String transformsTruncated(String size) {
    return 'Showing the first 64 KiB of $size';
  }

  @override
  String get transformsRandom => 'Random';

  @override
  String get transformsNotRun => 'Not run';

  @override
  String get transformsSensitiveHidden => 'The result is sensitive and hidden.';

  @override
  String get transformsReveal => 'Reveal';

  @override
  String get transformsPaneOutput => 'Output';

  @override
  String get transformsPaneInput => 'Input';

  @override
  String get transformsPaneChanges => 'Changes';

  @override
  String get transformsCannotTransform => 'Can\'t transform this text';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Step $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Show as hex';

  @override
  String get transformsActionPaste => 'Paste';

  @override
  String get transformsActionSave => 'Save as new clip';

  @override
  String get transformsCopiedMessage =>
      'The result is on the clipboard and in history.';

  @override
  String get transformsSavedTitle => 'Saved';

  @override
  String get transformsSavedMessage => 'Saved as a new history item.';

  @override
  String get transformsPasteFailedTitle => 'Couldn\'t paste';

  @override
  String get transformsPasteFailedMessage =>
      'The result is on the clipboard. Paste it manually.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message at line $line, column $column';
  }

  @override
  String get transformsErrorBinary => 'Decoded data is binary, not text';

  @override
  String get transformsErrorTooLarge => 'Too large to transform';

  @override
  String get transformsErrorInvalidParam =>
      'A setting of this step is not valid';

  @override
  String get transformsErrorCancelled => 'Cancelled';

  @override
  String get transformsErrorTooExpensive => 'Pattern too expensive';

  @override
  String get transformsErrorPattern => 'The pattern is not valid';

  @override
  String get transformsErrorTimestampUnit =>
      'Choose whether the number is in seconds or milliseconds';

  @override
  String get transformsErrorExternalEntity =>
      'External entities are not allowed';

  @override
  String get transformsErrorJwtEncrypted =>
      'Encrypted tokens can\'t be decoded';

  @override
  String get transformsErrorJson => 'Invalid JSON';

  @override
  String get transformsErrorYaml => 'Invalid YAML';

  @override
  String get transformsErrorXml => 'Invalid XML';

  @override
  String get transformsErrorCsv => 'Invalid CSV';

  @override
  String get transformsErrorBase64 => 'Invalid Base64';

  @override
  String get transformsErrorHex => 'Invalid hex';

  @override
  String get transformsErrorEscape => 'Invalid escape sequence';

  @override
  String get transformsErrorUrl => 'Malformed percent-encoding';

  @override
  String get transformsErrorJwt => 'Not a valid JWT';

  @override
  String get transformsErrorDate => 'Not a recognized date or timestamp';

  @override
  String get transformsErrorGeneric => 'This step can\'t transform its input';

  @override
  String get transformsWarningSignedLink => 'Signed link left unchanged';

  @override
  String get transformsWarningExpired => 'Expired';

  @override
  String get transformsWarningNotYetValid => 'Not yet valid';

  @override
  String get transformsWarningSignatureNotVerified => 'Signature not verified';

  @override
  String get transformsWarningDuplicateKeys => 'Duplicate keys';

  @override
  String get transformsWarningLenientJson => 'Accepted non-standard JSON';

  @override
  String get transformsWarningDoubleEncoded => 'The JSON was encoded twice';

  @override
  String get transformsWarningEscapes =>
      'Unknown escape sequences kept as written';

  @override
  String get transformsWarningLocalZone => 'Local time zone assumed';

  @override
  String get transformsWarningNoMatches => 'No matches';

  @override
  String get transformsWarningGeneric => 'Check the result';

  @override
  String get transformsRestoreTitle => 'Restore clipboard after paste';

  @override
  String get transformsRestoreDescription =>
      'Bring back the previous clipboard after pasting.';

  @override
  String get transformsDefaultTitle => 'Default paste transform';

  @override
  String get transformsDefaultDescription =>
      'Applied to text you paste from Quick Paste.';

  @override
  String get transformsNoDefault => 'None';

  @override
  String get transformsAppDefaultsTitle => 'Defaults per app';

  @override
  String get transformsAppDefaultsDescription =>
      'Use a different default in specific apps.';

  @override
  String transformsOtherPlatform(String platform) {
    return 'only on $platform';
  }

  @override
  String get transformsAddApp => 'Add app';

  @override
  String get transformsRemove => 'Remove';

  @override
  String get transformsFavoritesTitle => 'Favorites';

  @override
  String get transformsFavoritesDescription => 'Shown first in every palette.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Mod+$number in the palette';
  }

  @override
  String get transformsAddFavorite => 'Add a favorite';

  @override
  String get transformsPipelinesTitle => 'Pipelines';

  @override
  String get transformsPipelinesDescription => 'Saved chains of transforms.';

  @override
  String get transformsNewPipeline => 'New pipeline';

  @override
  String get transformsEditPipeline => 'Edit pipeline';

  @override
  String get transformsDuplicatePipeline => 'Duplicate';

  @override
  String get transformsDeletePipeline => 'Delete';

  @override
  String transformsDeletePipelineTitle(String name) {
    return 'Delete $name?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Favorites and paste defaults using it are cleared.';

  @override
  String get transformsDeletePipelineFailed =>
      'The pipeline could not be deleted.';

  @override
  String get transformsAddStep => 'Add a step';

  @override
  String get transformsHiddenTitle => 'Hidden transforms';

  @override
  String get transformsHiddenDescription =>
      'Left out of palettes and suggestions.';

  @override
  String get transformsHideTransform => 'Hide a transform';

  @override
  String get transformsShow => 'Show';

  @override
  String get transformsSuggestionsTitle => 'Suggestions';

  @override
  String get transformsSuggestionsDescription =>
      'Suggest transforms that fit the text.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web Tokens';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl => 'Links with tracking parameters';

  @override
  String get transformsDetectorTimestamp => 'Timestamps';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Tracking parameters';

  @override
  String get transformsTrackingDescription =>
      'Link parameters to also remove or always keep.';

  @override
  String get transformsTrackingRemoveLabel => 'Also remove';

  @override
  String get transformsTrackingAllowLabel => 'Always keep';

  @override
  String get transformsTrackingPlaceholder => 'name or name@domain, then Enter';

  @override
  String get quickAccessUnavailableTitle => 'Quick access status unavailable';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste couldn\'t read the keyboard, tile, and widget status. Try again.';

  @override
  String get quickAccessActionFailedTitle => 'Couldn\'t complete that';

  @override
  String get quickAccessActionFailedMessage =>
      'Android didn\'t open the requested screen or save the change. Try again.';

  @override
  String get quickAccessKeyboardTitle => 'Clips keyboard';

  @override
  String get quickAccessKeyboardDescription =>
      'Turn on CopyPaste clips in keyboard settings.';

  @override
  String get quickAccessKeyboardOn => 'Turned on';

  @override
  String get quickAccessKeyboardOff => 'Turned off';

  @override
  String get quickAccessOpenKeyboardSettings => 'Open keyboard settings';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'Turn on the CopyPaste clips keyboard panel.';

  @override
  String get quickAccessKeyboardKeywords =>
      'keyboard input method ime panel enable';

  @override
  String get quickAccessSwitchKeyboardTitle => 'Switch to the clips keyboard';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'Make CopyPaste clips the current keyboard.';

  @override
  String get quickAccessSwitchKeyboardAction => 'Choose keyboard';

  @override
  String get quickAccessKeyboardCurrent => 'Current keyboard';

  @override
  String get quickAccessCaptureTierTitle => 'Capture mode';

  @override
  String get quickAccessCaptureTierDescription =>
      'How copies from other apps are saved.';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'See how copies from other apps are saved.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'capture tier full panel manual shizuku collect';

  @override
  String get quickAccessTierFull =>
      'Full: every copy is saved in the background';

  @override
  String get quickAccessTierPanelCollecting => 'Panel selected: collecting';

  @override
  String get quickAccessTierPanelCatchUp => 'Panel mode: catch-up on open';

  @override
  String get quickAccessTierManual =>
      'Manual: save from the app, a tile, the widget, or Share';

  @override
  String get quickAccessTierPaused => 'Capture paused';

  @override
  String get quickAccessAutoReturnTitle =>
      'Return to previous keyboard after inserting';

  @override
  String get quickAccessAutoReturnDescription =>
      'Switch back after inserting one clip.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'switch back previous keyboard auto return insert';

  @override
  String get quickAccessFocusSearchTitle => 'Focus search when the panel opens';

  @override
  String get quickAccessFocusSearchDescription =>
      'Open the panel with search ready.';

  @override
  String get quickAccessTilesTitle => 'Quick Settings tiles';

  @override
  String get quickAccessTilesSearchDescription =>
      'Add the Save clipboard and Clipboard capture tiles.';

  @override
  String get quickAccessTilesKeywords => 'quick settings tile shade save pause';

  @override
  String get quickAccessTileInstructions =>
      'Swipe down twice, tap Edit, and add a CopyPaste tile.';

  @override
  String get quickAccessSaveTileTitle => 'Save clipboard tile';

  @override
  String get quickAccessSaveTileDescription =>
      'Saves what\'s on the clipboard from Quick Settings.';

  @override
  String get quickAccessCaptureTileTitle => 'Clipboard capture tile';

  @override
  String get quickAccessCaptureTileDescription =>
      'Pauses and resumes capture from Quick Settings.';

  @override
  String get quickAccessAddTile => 'Add tile';

  @override
  String get quickAccessTileAdded => 'Added to Quick Settings';

  @override
  String get quickAccessWidgetTitle => 'Recent clips widget';

  @override
  String get quickAccessWidgetDescription =>
      'Recent clips on your home screen, one tap to copy.';

  @override
  String get quickAccessWidgetInstructions =>
      'Hold the home screen, tap Widgets, and add Recent clips.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count widgets added',
      one: '1 widget added',
      zero: 'Not added',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Add widget';

  @override
  String get quickAccessWidgetKeywords => 'home screen widget launcher recent';

  @override
  String snippetsAbbreviationBadge(String abbreviation) {
    return 'Abbreviation $abbreviation';
  }

  @override
  String get snippetsAbbreviationLabel => 'Abbreviation';

  @override
  String get snippetsAbbreviationPlaceholder => 'Optional, for example sig';

  @override
  String snippetsAbbreviationTaken(String title) {
    return 'Already used by “$title”';
  }

  @override
  String snippetsAbbreviationTooLong(int max) {
    return 'Use at most $max characters.';
  }

  @override
  String get snippetsAbbreviationWhitespace =>
      'An abbreviation can\'t contain spaces or control characters.';

  @override
  String snippetsAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count snippets need review after a sync',
      one: '1 snippet needs review after a sync',
      zero: 'No snippets need review',
    );
    return '$_temp0';
  }

  @override
  String get snippetsBodyLabel => 'Text';

  @override
  String get snippetsBodyPlaceholder => 'Type the snippet text';

  @override
  String get snippetsClearSelection => 'Clear selection';

  @override
  String get snippetsClipTooLarge =>
      'Too large to edit. This clip opens read-only.';

  @override
  String snippetsCollectionAccent(String accent) {
    String _temp0 = intl.Intl.selectLogic(accent, {
      'neutral': 'Neutral',
      'blue': 'Blue',
      'green': 'Green',
      'amber': 'Amber',
      'red': 'Red',
      'other': 'Neutral',
    });
    return '$_temp0';
  }

  @override
  String get snippetsCollectionAccentLabel => 'Accent';

  @override
  String get snippetsCollectionIcon => 'Icon';

  @override
  String get snippetsCollectionLabel => 'Collection';

  @override
  String get snippetsCollectionName => 'Name';

  @override
  String get snippetsCollectionNameTaken =>
      'A collection with this name already exists.';

  @override
  String snippetsCollectionNameTooLong(int max) {
    return 'Use at most $max characters.';
  }

  @override
  String get snippetsConcealedBody => 'This snippet is concealed.';

  @override
  String get snippetsConcealedConfirmAction => 'Continue';

  @override
  String get snippetsConcealedConfirmMessage =>
      'This device can\'t verify you. Continue only on your own device.';

  @override
  String get snippetsConcealedConfirmTitle => 'Confirm it\'s you';

  @override
  String get snippetsConcealedPreview => 'Concealed snippet';

  @override
  String get snippetsConcealedSwitch => 'Concealed';

  @override
  String get snippetsConcealedTitle => 'Concealed snippet';

  @override
  String get snippetsConflictBadge => 'Conflict copy';

  @override
  String get snippetsCopied => 'Copied';

  @override
  String get snippetsCreate => 'Create snippet';

  @override
  String snippetsCreateNamed(String query) {
    return 'Create snippet “$query”';
  }

  @override
  String get snippetsDeleteCollection => 'Delete collection';

  @override
  String get snippetsDeleteCollectionMove => 'Move its snippets to Unfiled';

  @override
  String snippetsDeleteCollectionTitle(String name) {
    return 'Delete “$name”?';
  }

  @override
  String get snippetsDeleteCollectionWithSnippets => 'Delete its snippets too';

  @override
  String get snippetsDeletedMessage => 'Moved to Recently Deleted.';

  @override
  String snippetsDeletedTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count snippets deleted',
      one: 'Snippet deleted',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDestination => 'Save to';

  @override
  String snippetsDestinationName(String destination) {
    String _temp0 = intl.Intl.selectLogic(destination, {
      'historyClip': 'History clip',
      'snippet': 'Snippet',
      'other': 'History clip',
    });
    return '$_temp0';
  }

  @override
  String get snippetsDiscard => 'Discard';

  @override
  String snippetsDragging(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count snippets',
      one: '1 snippet',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDuplicateAbbreviation =>
      'Another snippet has this abbreviation. Change one.';

  @override
  String get snippetsDuplicateBadge => 'Duplicate abbreviation';

  @override
  String get snippetsEditClip => 'Edit clip';

  @override
  String get snippetsEditCollection => 'Edit collection';

  @override
  String get snippetsEditSnippet => 'Edit snippet';

  @override
  String get snippetsEditTab => 'Edit';

  @override
  String get snippetsEmptyMessage =>
      'Give a snippet an abbreviation, then type it in Quick Paste.';

  @override
  String get snippetsEmptyTitle => 'No snippets yet';

  @override
  String get snippetsEnableTemplate => 'Turn on';

  @override
  String get snippetsEnableTemplateMessage =>
      'Text that looks like a placeholder will be filled in.';

  @override
  String get snippetsEnableTemplateTitle => 'Turn on Template?';

  @override
  String get snippetsExport => 'Export';

  @override
  String get snippetsExportCollection => 'Export collection';

  @override
  String get snippetsExportConcealedConfirm =>
      'Include concealed snippets in the readable file';

  @override
  String snippetsExportCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count snippets written',
      one: '1 snippet written',
      zero: 'No snippets written',
    );
    return '$_temp0';
  }

  @override
  String get snippetsExportDone => 'Snippets exported';

  @override
  String get snippetsExportEncrypt => 'Encrypt with passphrase';

  @override
  String get snippetsExportFailed => 'Export failed';

  @override
  String get snippetsExportPlainConfirm =>
      'I understand that anyone with the file can read it';

  @override
  String snippetsExportScope(String scope, int count) {
    String _temp0 = intl.Intl.selectLogic(scope, {
      'all': 'All snippets',
      'collections': 'This collection',
      'snippets': 'Selected snippets ($count)',
      'other': 'All snippets',
    });
    return '$_temp0';
  }

  @override
  String get snippetsExportTitle => 'Export snippets';

  @override
  String get snippetsFieldBuilderTitle => 'Insert field';

  @override
  String get snippetsFieldDefault => 'Default value';

  @override
  String get snippetsFieldName => 'Name';

  @override
  String get snippetsFieldOptions => 'Options, separated by commas';

  @override
  String get snippetsFieldRequired => 'Fill in this field.';

  @override
  String get snippetsFieldType => 'Type';

  @override
  String snippetsFieldTypeName(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'text': 'Single line',
      'multiline': 'Multiple lines',
      'choice': 'Choice',
      'other': 'Single line',
    });
    return '$_temp0';
  }

  @override
  String get snippetsFormattingNotice =>
      'Formatting will be removed when you save.';

  @override
  String get snippetsGoToSnippet => 'Go to snippet';

  @override
  String get snippetsImport => 'Import';

  @override
  String get snippetsImportAbbreviationPolicy => 'Abbreviation already used';

  @override
  String snippetsImportAbbreviationPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'importWithout': 'Import without abbreviation',
      'renameWithSuffix': 'Add a number to the abbreviation',
      'skip': 'Skip',
      'other': 'Import without abbreviation',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportCollectionPolicy => 'Collection with the same name';

  @override
  String snippetsImportCollectionPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'merge': 'Add to the existing one',
      'createNew': 'Create a new one',
      'other': 'Add to the existing one',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportConvert => 'Convert known placeholders';

  @override
  String snippetsImportCounts(
    int added,
    int identical,
    int updated,
    int conflicts,
    int invalid,
  ) {
    return 'New: $added. Identical, skipped: $identical. Changed: $updated. Abbreviation conflicts: $conflicts. Invalid: $invalid.';
  }

  @override
  String get snippetsImportDone => 'Snippets imported';

  @override
  String get snippetsImportFailed => 'Import failed';

  @override
  String get snippetsImportFileType => 'Snippet files';

  @override
  String get snippetsImportFormat => 'Format';

  @override
  String snippetsImportFormatName(String format) {
    String _temp0 = intl.Intl.selectLogic(format, {
      'bundle': 'CopyPaste bundle',
      'json': 'JSON list',
      'csv': 'CSV',
      'other': 'CopyPaste bundle',
    });
    return '$_temp0';
  }

  @override
  String snippetsImportInvalidRow(int row, String reason) {
    return 'Row $row: $reason';
  }

  @override
  String get snippetsImportInvalidRows => 'Rows that won\'t be imported';

  @override
  String get snippetsImportPassphraseHint =>
      'Passphrase, if the file is encrypted';

  @override
  String get snippetsImportPreview => 'Preview';

  @override
  String snippetsImportReport(
    int inserted,
    int updated,
    int skipped,
    int flagged,
    int unrecognized,
  ) {
    return 'Added: $inserted. Updated: $updated. Skipped: $skipped. Abbreviations to review: $flagged. Unrecognized placeholders: $unrecognized.';
  }

  @override
  String get snippetsImportSameIdPolicy => 'Same snippet, different content';

  @override
  String snippetsImportSameIdPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'keepBoth': 'Keep both',
      'replace': 'Replace existing',
      'skip': 'Skip',
      'other': 'Keep both',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportTitle => 'Import snippets';

  @override
  String get snippetsInsert => 'Insert';

  @override
  String get snippetsInsertPlaceholder => 'Insert placeholder';

  @override
  String get snippetsKeepReferences => 'Keep them';

  @override
  String get snippetsLibraryDescription =>
      'Browse, organize and edit snippets and collections.';

  @override
  String get snippetsLibraryKeywords =>
      'snippets templates abbreviations collections text';

  @override
  String get snippetsLibraryTitle => 'Snippet library';

  @override
  String snippetsLint(String code, String detail) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'unclosed': 'This placeholder is never closed.',
      'malformed': 'This placeholder can\'t be read.',
      'unknownPlaceholder':
          'Unknown placeholder “$detail”. It is pasted as typed.',
      'unknownParameter': 'Unknown option “$detail”.',
      'duplicateParameter': 'Option “$detail” is set twice.',
      'missingParameter': 'Option “$detail” is missing.',
      'invalidValue': 'Option “$detail” has an invalid value.',
      'conflictingParameters': 'These options can\'t be used together.',
      'unknownLocale': 'Unknown language.',
      'unknownTimeZone': 'Unknown time zone.',
      'invalidPattern': 'Invalid date pattern.',
      'dateOutOfRange': 'The date is out of range.',
      'unknownModifier': 'Unknown modifier “$detail”.',
      'modifierFailed': 'Modifier “$detail” couldn\'t be applied.',
      'emptyClipboard': 'Clipboard item $detail is empty.',
      'extraCursor': 'Only the first cursor placeholder is used.',
      'nestedCursor': 'A cursor inside an inserted snippet is ignored.',
      'tooManyFields': 'Field “$detail” is over the limit of 10 fields.',
      'conflictingField':
          'Field “$detail” is defined twice with different settings.',
      'choiceDefaultNotOption':
          'The default of “$detail” is not one of its options.',
      'cycle': 'Snippets insert each other in a loop: $detail.',
      'depthExceeded': 'Snippets are inserted more than 3 levels deep.',
      'nestingLimit': 'Too many inserted snippets.',
      'unknownSnippet': 'No snippet has this abbreviation.',
      'containsPlaceholders':
          'The text contains placeholders. Turn on Template to fill them in.',
      'other': 'This placeholder has a problem.',
    });
    return '$_temp0';
  }

  @override
  String get snippetsLoadFailed => 'Couldn\'t load snippets';

  @override
  String get snippetsLoadMore => 'Load more';

  @override
  String get snippetsMerge => 'Merge';

  @override
  String get snippetsMergeCollection => 'Merge into another collection';

  @override
  String snippetsMergeCollectionTitle(String name) {
    return 'Merge “$name” into';
  }

  @override
  String get snippetsMoveTo => 'Move to';

  @override
  String get snippetsNewCollection => 'New collection';

  @override
  String get snippetsNewSnippet => 'New snippet';

  @override
  String get snippetsNewText => 'New text';

  @override
  String snippetsNoMatch(String query) {
    return 'No snippets match “$query”';
  }

  @override
  String get snippetsPassphraseConfirm => 'Repeat passphrase';

  @override
  String snippetsPassphraseHint(int min) {
    return 'Passphrase, at least $min characters';
  }

  @override
  String get snippetsPassphraseLabel => 'Passphrase';

  @override
  String get snippetsPassphraseNoRecovery =>
      'A forgotten passphrase can\'t be recovered.';

  @override
  String snippetsPassphraseStrength(String score) {
    String _temp0 = intl.Intl.selectLogic(score, {
      'tooShort': 'Too short',
      'weak': 'Weak',
      'fair': 'Fair',
      'good': 'Good',
      'other': 'Strong',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPinnedSwitch => 'Pinned';

  @override
  String get snippetsPlaceholderGroup => 'Placeholders';

  @override
  String snippetsPlaceholderKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'today': 'Date',
      'dateShort': 'Date, short',
      'dateLong': 'Date, long',
      'dateIso': 'Date, ISO 8601',
      'dateNextWeek': 'Date in one week',
      'now': 'Time',
      'dateTime': 'Date and time',
      'clipboard': 'Clipboard',
      'clipboardPrevious': 'Previous clipboard item',
      'field': 'Fill-in field…',
      'cursor': 'Cursor position',
      'uuid': 'UUID',
      'random': 'Random number',
      'upper': 'Modifier: uppercase',
      'lower': 'Modifier: lowercase',
      'trim': 'Modifier: trim spaces',
      'other': 'Placeholder',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPlaceholderNestedGroup => 'Insert another snippet';

  @override
  String get snippetsPlaceholderNoMatch => 'No placeholders match';

  @override
  String get snippetsPreviewEmpty => 'Nothing to preview';

  @override
  String get snippetsPreviewTab => 'Preview';

  @override
  String get snippetsPurge => 'Delete permanently';

  @override
  String get snippetsPurgeMessage =>
      'They will be erased on all your devices. This can\'t be undone.';

  @override
  String snippetsPurgeTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Delete $count snippets permanently?',
      one: 'Delete snippet permanently?',
    );
    return '$_temp0';
  }

  @override
  String get snippetsRecentlyDeletedEmpty => 'Recently Deleted is empty';

  @override
  String get snippetsRecordPastedDescription =>
      'Add pasted snippets to History.';

  @override
  String get snippetsRecordPastedKeywords => 'history record pasted snippets';

  @override
  String get snippetsRecordPastedTitle => 'Record pasted snippets in History';

  @override
  String get snippetsReplaceOriginal => 'Replace original';

  @override
  String get snippetsReplaceOriginalHint =>
      'Deletes the original clip after saving.';

  @override
  String get snippetsRequireAuthDescription =>
      'Ask for your password or biometrics first.';

  @override
  String get snippetsRequireAuthKeywords =>
      'concealed password biometrics fingerprint face lock verify';

  @override
  String get snippetsRequireAuthTitle =>
      'Verify before using concealed snippets';

  @override
  String get snippetsRestore => 'Restore';

  @override
  String get snippetsSaveBlockTooLarge => 'The text is longer than 64 KB.';

  @override
  String get snippetsSaveFailed => 'Couldn\'t save';

  @override
  String get snippetsScopeAll => 'All';

  @override
  String get snippetsScopeEmpty => 'No snippets here';

  @override
  String get snippetsScopePinned => 'Pinned';

  @override
  String get snippetsScopeRecentlyDeleted => 'Recently Deleted';

  @override
  String get snippetsSearchPlaceholder => 'Search snippets';

  @override
  String snippetsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count selected',
      one: '1 selected',
    );
    return '$_temp0';
  }

  @override
  String get snippetsShow => 'Show';

  @override
  String get snippetsShowAll => 'Show all';

  @override
  String get snippetsShowAttention => 'Review';

  @override
  String snippetsSoftLimit(int count) {
    return 'You have $count snippets. Up to 20,000 are allowed.';
  }

  @override
  String get snippetsTemplateBadge => 'Template';

  @override
  String get snippetsTemplateSwitch => 'Template';

  @override
  String get snippetsTitleLabel => 'Title';

  @override
  String get snippetsTitlePlaceholder => 'Optional';

  @override
  String get snippetsTransferDescription =>
      'Back up snippets or move them between apps.';

  @override
  String get snippetsTransferKeywords =>
      'import export backup bundle json csv passphrase';

  @override
  String get snippetsTransferTitle => 'Import and export';

  @override
  String get snippetsUndo => 'Undo';

  @override
  String get snippetsUnfiled => 'Unfiled';

  @override
  String snippetsUnfilledWarning(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count placeholders are pasted as typed',
      one: '1 placeholder is pasted as typed',
    );
    return '$_temp0';
  }

  @override
  String get snippetsUnpin => 'Unpin';

  @override
  String get snippetsUnsavedMessage => 'Save your changes before closing?';

  @override
  String get snippetsUnsavedTitle => 'Unsaved changes';

  @override
  String get snippetsUntitled => 'Untitled snippet';

  @override
  String get snippetsUpdateReferences => 'Update';

  @override
  String snippetsUpdateReferencesMessage(String from, String to) {
    return 'Other templates may insert “$from”. Update them to use “$to”?';
  }

  @override
  String get snippetsUpdateReferencesTitle => 'Update references?';

  @override
  String get snippetsUseFailed => 'Couldn\'t use the snippet';

  @override
  String get quickPasteTabQueue => 'Queue';

  @override
  String get quickPasteMaskedMode => 'Password field: previews are hidden';

  @override
  String quickPasteMaskedItem(String kind) {
    return 'Hidden: $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'Unlock secret items';

  @override
  String quickPastePastingWith(String name) {
    return 'Pasting with: $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut pastes the original';
  }

  @override
  String get quickPasteDefaultSkipped => 'Default transform skipped';

  @override
  String get quickPasteDefaultSkippedMessage =>
      'The original was used instead.';

  @override
  String get quickPastePasteManually =>
      'The content is on the clipboard. Paste it manually.';

  @override
  String get quickPastePaste => 'Paste';

  @override
  String get quickPasteSnippetsUnavailable => 'Snippets are unavailable';

  @override
  String get rulesLoadFailed => 'Content rules could not be loaded.';

  @override
  String get rulesNewRule => 'New rule';

  @override
  String get rulesEditRule => 'Edit rule';

  @override
  String get rulesUntitled => 'Untitled rule';

  @override
  String get rulesPrivacyDefaultsTitle => 'Privacy defaults';

  @override
  String get rulesPrivacyDefaultsDescription =>
      'Privacy settings apply before any rule.';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'privacy concealed temporary excluded apps built-in credentials passwords payment cards';

  @override
  String get rulesOpenPrivacy => 'Open Privacy';

  @override
  String get rulesMyRulesTitle => 'My rules';

  @override
  String get rulesMyRulesDescription => 'Run in order on new copies.';

  @override
  String get rulesMyRulesKeywords =>
      'content rules automation tag skip secret expire delete pattern regex';

  @override
  String get rulesEmptyTitle => 'No rules yet';

  @override
  String get rulesEmptyMessage =>
      'Rules tag, protect, expire or skip new copies.';

  @override
  String get rulesDeletedTitle => 'Recently deleted';

  @override
  String get rulesDeletedDescription => 'Restore within 30 days.';

  @override
  String rulesDeletedAt(String time) {
    return 'Deleted $time';
  }

  @override
  String get rulesRestore => 'Restore';

  @override
  String get rulesDeletedToast => 'Rule deleted';

  @override
  String get rulesDeletedToastMessage => 'You can restore it for 30 days.';

  @override
  String get rulesUndo => 'Undo';

  @override
  String rulesConflictMessage(String rule, String device) {
    return 'A concurrent edit of “$rule” from $device was replaced by a newer one.';
  }

  @override
  String get rulesAnotherDevice => 'another device';

  @override
  String get rulesRestoreConflict => 'Use that version';

  @override
  String get rulesSkipLogTitle => 'Recently skipped';

  @override
  String get rulesSkipLogDescription =>
      'Copies your rules didn\'t save on this device.';

  @override
  String get rulesSkipLogKeywords => 'skipped ignored not saved log';

  @override
  String get rulesSkipLogShow => 'Show';

  @override
  String get rulesSkipLogEmpty => 'No copies were skipped.';

  @override
  String get rulesUnknownApp => 'Unknown app';

  @override
  String get rulesMatchesEverything => 'Any copy';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary, then stop';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': ' or ',
      'other': ' and ',
    });
    return '$_temp0';
  }

  @override
  String rulesConditionSummary(
    String attribute,
    String operator,
    String value,
  ) {
    return '$attribute $operator $value';
  }

  @override
  String rulesActionSummary(String action, String value) {
    return '$action $value';
  }

  @override
  String rulesRunsOn(String devices) {
    return 'Runs on $devices';
  }

  @override
  String rulesNewFromDevice(String device) {
    return 'New from $device';
  }

  @override
  String get rulesNeedsUpdateBadge => 'Needs update';

  @override
  String get rulesUpdateRequired => 'Update CopyPaste to run this rule.';

  @override
  String get rulesErrorBadge => 'Error';

  @override
  String rulesErrorHint(String problem) {
    return '$problem Until fixed, matches are protected.';
  }

  @override
  String get rulesReorder => 'Drag to reorder';

  @override
  String get rulesMoreActions => 'More actions';

  @override
  String get rulesEdit => 'Edit';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': 'Move up',
      'down': 'Move down',
      'top': 'Move to top',
      'other': 'Move to bottom',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => 'Apply to existing items…';

  @override
  String get rulesResetBuiltin => 'Reset to default';

  @override
  String get rulesNameLabel => 'Name';

  @override
  String get rulesNamePlaceholder => 'For example, Work links';

  @override
  String get rulesWhenTitle => 'When';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'Any condition matches',
      'other': 'All conditions match',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions =>
      'No conditions: this rule applies to every copy.';

  @override
  String get rulesAddCondition => 'Add condition';

  @override
  String get rulesRemoveCondition => 'Remove condition';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'Text',
      'content_type': 'Content type',
      'detected_kind': 'Detected kind',
      'source_app': 'Source app',
      'size': 'Size',
      'privacy_marker': 'Privacy marker',
      'capture_origin': 'Origin',
      'other': 'Unknown condition',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'contains',
      'equals': 'is',
      'starts_with': 'starts with',
      'ends_with': 'ends with',
      'matches_regex': 'matches the pattern',
      'other': 'matches',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'does not contain',
      'equals': 'is not',
      'starts_with': 'does not start with',
      'ends_with': 'does not end with',
      'matches_regex': 'does not match the pattern',
      'other': 'does not match',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'is not',
      'other': 'is',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'is less than',
      'other': 'is more than',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'is not less than',
      'other': 'is not more than',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'lines',
      'bytes': 'bytes',
      'other': 'characters',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'lines',
      'bytes': 'bytes',
      'other': 'characters',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'Plain text',
      'rich_text': 'Rich text',
      'image': 'Image',
      'files': 'Files',
      'color': 'Color',
      'link': 'Link',
      'other': 'Other',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'Password or secret',
      'url': 'Web address',
      'email': 'Email address',
      'phone': 'Phone number',
      'code': 'Code',
      'color': 'Color value',
      'json': 'JSON',
      'file_path': 'File path',
      'number': 'Number',
      'one_time_code': 'One-time code',
      'payment_card': 'Payment card number',
      'other': 'Other',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': 'Concealed',
      'transient': 'Temporary',
      'auto_generated': 'Auto-generated',
      'excluded_from_history': 'Excluded from history',
      'other': 'Other',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'Copied on this device',
      'received': 'Received from another device',
      'created_in_app': 'Created in CopyPaste',
      'other': 'Other',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => 'Text to look for';

  @override
  String get rulesPatternPlaceholder => 'Pattern, for example ^ticket-[0-9]+';

  @override
  String get rulesPatternHint =>
      'Look-around and backreferences aren\'t supported.';

  @override
  String get rulesCaseSensitive => 'Match case';

  @override
  String get rulesChooseValues => 'Choose';

  @override
  String get rulesUnknownConditionNotice =>
      'This condition comes from a newer version of CopyPaste.';

  @override
  String get rulesSourceAppNotice =>
      'On Android, only copies made with the clips keyboard match.';

  @override
  String get rulesPrivacyMarkerNotice =>
      'Not every system reports every marker.';

  @override
  String get rulesThenTitle => 'Then';

  @override
  String get rulesNoActions => 'Add at least one action.';

  @override
  String get rulesAddAction => 'Add action';

  @override
  String get rulesRemoveAction => 'Remove action';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': 'Don\'t save',
      'mark_secret': 'Mark as secret',
      'delete_after': 'Delete after',
      'add_tags': 'Add tags',
      'transform': 'Transform',
      'other': 'Unknown action',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone': '“Don\'t save” can\'t be combined with other actions.',
      'present': 'This rule already has this action.',
      'other': 'A rule can have up to 8 actions.',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'This action comes from a newer version of CopyPaste.';

  @override
  String get rulesDontSaveWarning =>
      'Matching copies are never stored, indexed or synced.';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count minutes',
      one: '1 minute',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count hours',
      one: '1 hour',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count days',
      one: '1 day',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry =>
      'Also clear the clipboard if it still holds this copy.';

  @override
  String get rulesNoTags => 'Create tags in History to use them here.';

  @override
  String get rulesNoTransforms => 'No transforms are available.';

  @override
  String get rulesReplaceClipboard =>
      'Also replace the clipboard with the result';

  @override
  String get rulesOptionsTitle => 'Options';

  @override
  String get rulesEnabledSwitch => 'Enabled';

  @override
  String get rulesStopProcessing =>
      'Stop processing later rules when this one matches';

  @override
  String get rulesRunsOnLabel => 'Runs on';

  @override
  String get rulesAllDevices => 'All devices';

  @override
  String get rulesChosenDevices => 'Chosen devices';

  @override
  String get rulesTestTitle => 'Test';

  @override
  String get rulesTestDescription => 'Try the rule on sample text.';

  @override
  String get rulesSamplePlaceholder => 'Paste or type sample text';

  @override
  String get rulesTestFullSet => 'Test together with all rules';

  @override
  String get rulesRunTest => 'Run test';

  @override
  String get rulesTestFailed => 'The test could not run.';

  @override
  String get rulesPreviewHistory => 'Preview against history';

  @override
  String get rulesSaveFailed => 'The rule could not be saved.';

  @override
  String get rulesReadOnlyNotice =>
      'Made by a newer CopyPaste. Update to edit or run it.';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'The rule could not be read.',
      'unsupported_schema': 'This rule needs a newer version of CopyPaste.',
      'unknown_condition_type':
          'This condition needs a newer version of CopyPaste.',
      'unknown_action_type': 'This action needs a newer version of CopyPaste.',
      'too_many_conditions': 'A rule can have up to 20 conditions.',
      'too_many_actions': 'A rule can have up to 8 actions.',
      'too_many_rules': 'Only the first 200 rules run.',
      'no_actions': 'Add at least one action.',
      'conflicting_actions':
          '“Don\'t save” can\'t be combined with other actions.',
      'duplicate_action': 'Each action can be used only once.',
      'empty_value': 'Enter a value.',
      'empty_list': 'Choose at least one value.',
      'too_many_values': 'Choose up to 64 values.',
      'invalid_value': 'This value is not valid.',
      'pattern_too_long': 'Patterns can be up to 2,000 characters.',
      'unsupported_look_around':
          'Look-ahead and look-behind are not supported. Use two conditions with “All conditions match” instead.',
      'unsupported_backreference': 'Backreferences are not supported.',
      'pattern_too_large':
          'This pattern is too complex. Simplify its repetitions.',
      'invalid_pattern': 'This pattern is not valid.',
      'invalid_duration': 'Choose a duration from 1 minute to 90 days.',
      'invalid_tags': 'Choose at least one tag.',
      'invalid_transform': 'Choose a transform.',
      'other': 'This rule has a problem.',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceLine(String label, String result) {
    return '$label: $result';
  }

  @override
  String rulesTraceResult(String result) {
    String _temp0 = intl.Intl.selectLogic(result, {
      'matched': 'matched',
      'not_matched': 'not matched',
      'unknown': 'unknown',
      'skipped': 'not run',
      'other': 'error',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return 'Condition $number';
  }

  @override
  String get rulesTraceNoRules => 'No rule was evaluated.';

  @override
  String get rulesTraceStopped => 'Later rules did not run.';

  @override
  String get rulesTraceUnknownReason =>
      'Unknown for this copy, so it doesn\'t match.';

  @override
  String get rulesTraceRuleError =>
      'This rule couldn\'t run, so matches are protected.';

  @override
  String get rulesTraceOutcomeTitle => 'Result';

  @override
  String get rulesTraceNoEffect => 'No rule changes this copy.';

  @override
  String get rulesTraceSkipped => 'Not saved';

  @override
  String get rulesTraceSecret => 'Saved as secret';

  @override
  String rulesTraceExpires(String duration) {
    return 'Deleted after $duration';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'Tagged $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return 'Transformed with $names';
  }

  @override
  String get rulesTraceOutput => 'Transformed text';

  @override
  String get rulesPreviewTitle => 'Apply to existing items';

  @override
  String rulesPreviewRunning(int count) {
    return 'Checking the latest $count clips…';
  }

  @override
  String get rulesPreviewNone => 'No recent clip matches this rule.';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips match',
      one: '1 clip matches',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count would be deleted',
      one: '1 would be deleted',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count would be marked secret',
      one: '1 would be marked secret',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count would get a deletion time',
      one: '1 would get a deletion time',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count would be tagged',
      one: '1 would be tagged',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return 'Only the latest $count clips were checked.';
  }

  @override
  String rulesPreviewMore(int count) {
    return 'and $count more';
  }

  @override
  String get rulesPreviewNote => 'Nothing changes until you apply.';

  @override
  String get rulesSecretItem => 'Secret item';

  @override
  String rulesSecretItemFrom(String app) {
    return 'Secret item from $app';
  }

  @override
  String get rulesPreviewCancelled =>
      'The preview was stopped. Nothing changed.';

  @override
  String get rulesPreviewFailed => 'The preview could not run.';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Delete $count clips?',
      one: 'Delete 1 clip?',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage =>
      'Matching clips are deleted on all your devices. This can\'t be undone.';

  @override
  String get rulesApplyDeleteConfirm => 'Delete and apply';

  @override
  String rulesApplyProgress(int done, int total) {
    return 'Applying: $done of $total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips changed',
      one: '1 clip changed',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips deleted',
      one: '1 clip deleted',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips could not be changed',
      one: '1 clip could not be changed',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled => 'Stopped. Changes made so far stay.';

  @override
  String get historyActionEdit => 'Edit';

  @override
  String get historyActionSaveAsSnippet => 'Save as snippet';

  @override
  String get historyActionAddToQueue => 'Add to Paste Queue';

  @override
  String get historyActionTransform => 'Transform…';

  @override
  String get historyActionExplain => 'Why is this here?';

  @override
  String get historyActionKeep => 'Keep';

  @override
  String get historyNewClip => 'New clip';

  @override
  String get historySavedTitle => 'Saved';

  @override
  String get historyEditSaved =>
      'The edited text is a new clip at the top of History.';

  @override
  String get historyNewClipSaved => 'Your text was saved.';

  @override
  String get historySnippetSaved => 'The clip\'s text is now a snippet.';

  @override
  String get historySnippetSaveFailed =>
      'This clip could not be saved as a snippet.';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips joined the queue.',
      one: '1 clip joined the queue.',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed =>
      'Couldn\'t add to Paste Queue. Check that it is turned on.';

  @override
  String get historyKeepFailed => 'This clip could not be kept.';

  @override
  String get historyExplainFailed => 'The explanation could not be loaded.';

  @override
  String get historyDeletesSoon => 'Deletes in less than a minute';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Deletes in $count min',
      one: 'Deletes in 1 min',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Deletes in $count h',
      one: 'Deletes in 1 h',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Deletes in $count days',
      one: 'Deletes in 1 day',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => 'Manual';

  @override
  String get historyProvenanceEdited => 'Edited';

  @override
  String get historyProvenanceTransformed => 'Transformed';

  @override
  String historyRuleTag(String name) {
    return '$name (auto)';
  }

  @override
  String get historyMetadataExpiry => 'Deletion';

  @override
  String get historyMetadataRuleTags => 'Automatic tags';
}
