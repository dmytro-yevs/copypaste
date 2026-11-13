// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get commonCancel => '取消';

  @override
  String get commonClose => '关闭';

  @override
  String get commonDone => '完成';

  @override
  String get commonRetry => '重试';

  @override
  String get commonTryAgain => '再试一次';

  @override
  String get commonCopy => '复制';

  @override
  String get commonDelete => '删除';

  @override
  String get commonSave => '保存';

  @override
  String get commonContinue => '继续';

  @override
  String get commonBack => '返回';

  @override
  String get commonOpenSettings => '打开设置';

  @override
  String get commonLoading => '正在加载';

  @override
  String get commonApply => '应用';

  @override
  String get commonClear => '清除';

  @override
  String get commonPin => '置顶';

  @override
  String get commonEnable => '启用';

  @override
  String get settingsLanguageTitle => '语言';

  @override
  String get settingsLanguageDescription => '选择 CopyPaste 界面的语言。';

  @override
  String get settingsLanguageSystem => '系统默认';

  @override
  String get settingsLanguageKeywords =>
      '语言 区域 翻译 界面 language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed => '无法更新“登录时启动”设置。请重试。';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed => '无法打开系统启动设置。';

  @override
  String get settingsRuntimeRefreshFailed => '无法刷新设置。请重试。';

  @override
  String get settingsRuntimeLoadFailed => '无法加载设置。';

  @override
  String get settingsRuntimeAppAlreadyExcluded => '该应用已被排除。';

  @override
  String get settingsRuntimeChangeFailed => '无法完成所请求的更改。';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      '需先完成剪贴板捕获权限设置，才能恢复捕获。';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      '剪贴板捕获已暂停。Android 捕获服务未确认已停止。';

  @override
  String get settingsRuntimeNotificationPermissionRequired => '需要通知权限才能启用捕获通知。';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste 无法读取已保存的设置。已启用保护性默认值；请检查并保存偏好设置以进行修复。';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste 已使用保护性默认值恢复无法读取的设置。请检查并保存偏好设置以进行修复。';

  @override
  String get settingsRuntimeExcludedAppIdInvalid => '应用标识符不能为空，且不得超过 256 字节。';

  @override
  String get settingsRuntimeExcludedAppsLimit => '最多可排除 256 个应用。';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed => '无法更新自动粘贴设置。';

  @override
  String get settingsRuntimeShortcutRegisterFailed => '无法注册该快捷键。';

  @override
  String get settingsRuntimeShortcutPauseFailed => '无法暂停当前快捷键。';

  @override
  String get settingsRuntimeShortcutRestoreFailed => '无法恢复快捷键。';

  @override
  String get settingsRuntimeQuickPasteOpenFailed => '无法打开 Quick Paste。';

  @override
  String get settingsRuntimeAccessibilityRequestFailed => '无法请求辅助功能权限。';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed => '无法刷新 Linux 集成状态。';

  @override
  String get settingsRuntimeClipboardInputNotGranted => '未授予剪贴板输入权限。';

  @override
  String get settingsRuntimeClipboardInputRequestFailed => '无法请求剪贴板输入权限。';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed => '无法打开桌面集成设置。';

  @override
  String get settingsRuntimeTextHistoryFileType => 'CopyPaste 文本历史记录';

  @override
  String get settingsRuntimeEncryptedBackupFileType => 'CopyPaste 加密备份';

  @override
  String get settingsRuntimeNotificationImage => '图片';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return '图片 · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => '文件';

  @override
  String get updateRestoreInstallationFailed => 'CopyPaste 无法恢复更新安装。';

  @override
  String get updateLinuxAutomaticUnsupported => '此 Linux 安装无法自动更新。';

  @override
  String get updateMacosPackageMissing => '此版本没有已签名的 macOS 更新包。';

  @override
  String get updateInstalledVersionInvalid => '已安装的 CopyPaste 版本无效。';

  @override
  String get updateCheckUnreachable => 'CopyPaste 无法连接 GitHub 以检查更新。';

  @override
  String get updateCheckTimedOut => '检查更新超时。';

  @override
  String get updateCheckDeviceFailed => 'CopyPaste 无法在此设备上检查更新。';

  @override
  String get updateCheckFailed => 'CopyPaste 无法检查更新。';

  @override
  String get updateDownloadFailed => 'CopyPaste 无法下载更新。';

  @override
  String get updateDownloadTimedOut => '下载更新超时。';

  @override
  String get updateInstallFailed => 'CopyPaste 无法安装更新。';

  @override
  String get updateContinueInAndroidInstaller => '请在 Android 系统安装程序中继续。';

  @override
  String get updateContinueInPackageManager => '请在系统软件包管理器中继续。';

  @override
  String get updateInstallerStarting => '正在启动安装程序。';

  @override
  String get updatePermissionRequiredAndroid =>
      '请在 Android 设置中允许 CopyPaste 安装应用，然后继续。';

  @override
  String get updatePermissionRequiredLinux => '请在系统软件包管理器中进行身份验证以安装此更新。';

  @override
  String get updatePermissionRequired => '请允许 CopyPaste 安装此更新，然后继续。';

  @override
  String get updateRestartRequired => '更新已安装。请重新启动 CopyPaste 以使用新版本。';

  @override
  String get updateRestarting => '正在重新启动 CopyPaste。';

  @override
  String get updateRestartFailed => 'CopyPaste 无法重新启动。请重试。';

  @override
  String get updateReleasePageOpenFailed => 'CopyPaste 无法打开发布页面。';

  @override
  String get updatePackageInvalid => '下载的更新不是有效的 CopyPaste 软件包。';

  @override
  String get updateSignatureInvalid => '下载的更新签名无效。';

  @override
  String get updateDowngradeRefused => '下载的软件包版本不高于当前版本。';

  @override
  String get updateHomebrewRequired => '请使用 Homebrew 安装 CopyPaste，才能在此处更新。';

  @override
  String get updateInstallationCancelled => '更新安装已取消。';

  @override
  String get updateInstallationBlocked => 'Android 阻止了更新安装。';

  @override
  String get updateInstallationConflict => '更新与已安装的应用冲突。';

  @override
  String get updateInstallationIncompatible => '更新与此设备不兼容。';

  @override
  String get updateInstallationStorage => '请释放存储空间以安装更新。';

  @override
  String get updateInstallationInterrupted => '更新安装已中断。请重试。';

  @override
  String get updateInstallationBusy => '已有更新正在安装。';

  @override
  String get updatePackageDetailsInvalid => '更新包信息无效。';

  @override
  String get updateIntegrityCheckFailed => '下载的更新未通过完整性检查。';

  @override
  String get updateInstallerLaunchFailed => 'CopyPaste 无法启动系统安装程序。';

  @override
  String get updateInstallLocationTranslocated =>
      '请将 CopyPaste 移到“应用程序”文件夹，然后从该位置打开以进行更新。';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste 无法在当前文件夹中替换自身。请通过发布页面进行更新。';

  @override
  String get updateReplacementBlocked =>
      'macOS 阻止了替换 CopyPaste。请在“App 管理”设置中允许，或使用发布页面。';

  @override
  String get updatePackageNotDownloadable => '此平台不使用可下载的更新包。';

  @override
  String get updatePackageSizeInvalid => '更新包大小无效。';

  @override
  String get updateUpdaterSignatureInvalid => '下载的更新未通过更新程序签名验证。';

  @override
  String get updatePackageUrlUntrusted => '更新包 URL 不受信任。';

  @override
  String get updateDownloadLeftGitHub => '更新下载被重定向到了 GitHub 之外。';

  @override
  String get updateDownloadTooManyRedirects => '更新下载的重定向次数过多。';

  @override
  String get updatePackageTooLarge => '更新包过大。';

  @override
  String get updateSignatureSizeInvalid => '更新程序签名大小无效。';

  @override
  String get updateMetadataInvalid => 'GitHub 返回了无效的更新元数据。';

  @override
  String get updateMetadataTooLarge => '更新元数据过大。';

  @override
  String get quickPasteActionFailedTitle => '操作失败';

  @override
  String get quickPasteOpenAppSemantics => '打开 CopyPaste';

  @override
  String get quickPasteSearchPlaceholder => '输入以搜索…';

  @override
  String get quickPasteLoadingHistory => '正在加载剪贴板历史记录。';

  @override
  String get quickPasteHistoryUnavailable => '历史记录不可用';

  @override
  String get quickPasteNoClipsFound => '未找到剪贴项';

  @override
  String get quickPasteLoadMore => '加载更多';

  @override
  String get quickPasteTransferFailed => '失败';

  @override
  String get quickPasteClipUnavailable => '剪贴项不可用';

  @override
  String get quickPasteFileFallbackName => '文件';

  @override
  String get quickPastePathUnavailable => '路径不可用';

  @override
  String get quickPastePreferences => '偏好设置…';

  @override
  String get quickPasteAbout => '关于';

  @override
  String get quickPasteQuit => '退出';

  @override
  String get quickPasteClearHistoryTitle => '清除历史记录？';

  @override
  String get quickPasteClearHistoryMessage => '已置顶的剪贴项将保留在历史记录中。';

  @override
  String get quickPasteClearHistoryFailed => '无法删除历史记录。';

  @override
  String get quickPasteAboutDescription =>
      '跨 macOS、Android、Windows 和 Linux 的私密剪贴板历史记录。';

  @override
  String get transfersTitle => '下载与传输';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个传输进行中',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个进行中',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => '最新';

  @override
  String get transfersShowEarlier => '更早';

  @override
  String get transfersClearCompletedHistory => '清除已完成的记录';

  @override
  String get transfersErrorTitle => '传输';

  @override
  String get transfersEmptyTitle => '无传输';

  @override
  String get transfersEmptyMessage => '文件导入、下载和传出传输将显示在这里。';

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
  String get transfersRemoveFromHistory => '从记录中移除';

  @override
  String get transfersCancelDownload => '取消下载';

  @override
  String get transfersShowFile => '显示文件';

  @override
  String get transfersPhaseSending => '正在发送';

  @override
  String get transfersPhaseDownloading => '正在下载';

  @override
  String get transfersPhaseAdding => '正在添加';

  @override
  String get transfersPhaseAdded => '已添加';

  @override
  String get transfersPhaseSent => '已发送';

  @override
  String get transfersPhaseCompleted => '已完成';

  @override
  String get transfersPhaseCancelled => '已取消';

  @override
  String get transfersPhaseInterrupted => '已中断';

  @override
  String get transfersPhaseFailed => '失败';

  @override
  String get transfersUpdatesUnavailable => '无法获取传输更新。请重新打开以刷新。';

  @override
  String get transfersLoadFailed => '无法加载传输。请重试。';

  @override
  String get transfersUpdatesLoadFailed => '无法加载传输更新。';

  @override
  String get transfersRetryFailed => '无法重试此文件。';

  @override
  String get transfersClearHistoryFailed => '无法清除传输记录。';

  @override
  String get transfersRemoveRecordFailed => '无法移除此记录。';

  @override
  String get transfersCancelFailed => '无法取消传输。传输可能仍在进行。';

  @override
  String get transfersSavedFileUnavailable => '已保存的文件不可用。它可能已被移动或删除。';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '通知 · $count 条未读',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => '通知';

  @override
  String get notificationsClearRead => '清除已读通知';

  @override
  String get notificationsEmptyTitle => '无通知';

  @override
  String get notificationsEmptyMessage => '设备连接请求及其结果将显示在这里。';

  @override
  String get notificationsSystemAlertTitle => '系统通知';

  @override
  String get notificationsReview => '查看';

  @override
  String get notificationsConnectionRequest => '连接请求';

  @override
  String get notificationsConfirmConnection => '确认连接';

  @override
  String get notificationsDeviceConnected => '设备已连接';

  @override
  String get notificationsConnectionRejected => '连接已拒绝';

  @override
  String get notificationsConnectionCancelled => '连接已取消';

  @override
  String get notificationsConnectionRequestExpired => '连接请求已过期';

  @override
  String get notificationsConnectionFailed => '连接失败';

  @override
  String get notificationsNearbyDevice => '附近的设备';

  @override
  String get notificationsPermissionDenied => '请在系统设置中允许系统通知，以便在后台接收请求。';

  @override
  String get notificationsSystemUnavailable => '系统通知不可用。连接请求会保留在此处。';

  @override
  String get platformCaptureStateUnavailable => '不可用';

  @override
  String get platformCaptureStatePaused => '已暂停';

  @override
  String get platformCaptureStateRunning => '正在捕获';

  @override
  String get platformCaptureStateStopped => '未运行';

  @override
  String get platformCaptureActionUnavailable => '捕获不可用';

  @override
  String get platformCaptureActionResume => '恢复捕获';

  @override
  String get platformCaptureActionPause => '暂停捕获';

  @override
  String get platformCaptureActionResumeStopped => '恢复捕获（未运行）';

  @override
  String get platformTrayOpen => '打开';

  @override
  String get platformTraySettings => '设置';

  @override
  String get platformTrayQuit => '退出';

  @override
  String get platformWindowControlsUnavailable => '桌面窗口控制不可用。CopyPaste 将保持可见。';

  @override
  String get platformTrayUnavailable => '系统托盘不可用。请保持 CopyPaste 打开，然后重试或退出。';

  @override
  String get platformQuitFailed => 'CopyPaste 无法退出。请重试。';

  @override
  String get platformVisibilityChangeFailed => '无法更改应用的可见性。请重试。';

  @override
  String get platformCaptureNotificationTitle => '剪贴板已保存';

  @override
  String get platformCaptureNotificationBody => '新的剪贴项已添加到 CopyPaste。';

  @override
  String get platformPairingNotificationTitle => '连接请求';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName 请求连接。请打开 CopyPaste 比对代码并确认。';
  }

  @override
  String get platformPairingNotificationChannelName => '设备连接请求';

  @override
  String get platformPairingNotificationChannelDescription => '连接附近设备的请求';

  @override
  String get platformNotificationDefaultAction => '打开';

  @override
  String get platformFilesSelectionFailed => '无法选择文件。';

  @override
  String get platformFilesOnlyRegular => '只能导入普通文件。';

  @override
  String get platformModulesUnavailable => '模块在此设备上不可用。';

  @override
  String get platformSmsAccessConfigureFailed => '无法配置短信访问权限。';

  @override
  String get platformSmsAccessVerifyFailed => '无法验证短信访问权限。';

  @override
  String get platformUpdatesUnavailableHere => '此处无法使用应用更新。';

  @override
  String get platformUpdateVersionUnavailable => '无法获取已安装的版本。';

  @override
  String get platformUpdatesUnavailableInstallation => '此安装无法使用应用更新。';

  @override
  String get platformUpdateLinuxUnsupported => '此 Linux 安装无法自动更新。';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      '此 Linux 更新与当前运行的架构不匹配。';

  @override
  String get platformUpdateMacosUnsupported => '此 macOS 安装无法自动更新。';

  @override
  String get platformUpdateStartFailed => 'CopyPaste 无法开始更新。';

  @override
  String get platformGlobalShortcutDescription => '打开 Quick Paste';

  @override
  String get settingsSectionGeneral => '通用';

  @override
  String get settingsSectionGeneralSummary => '语言和应用可见性。';

  @override
  String get settingsSectionClipboard => '剪贴板';

  @override
  String get settingsSectionClipboardSummary => '捕获和历史记录限制。';

  @override
  String get settingsSectionPrivacy => '隐私';

  @override
  String get settingsSectionPrivacySummary => '应用排除和屏幕保护。';

  @override
  String get settingsSectionQuickPasteSummary => '快捷键和自动粘贴行为。';

  @override
  String get settingsSectionSync => '同步';

  @override
  String get settingsSectionSyncSummary => '同步和附近设备发现。';

  @override
  String get settingsSectionNotifications => '通知';

  @override
  String get settingsSectionNotificationsSummary => '捕获通知和声音。';

  @override
  String get settingsSectionData => '数据';

  @override
  String get settingsSectionDataSummary => '导出、备份和恢复。';

  @override
  String get settingsSectionModules => '模块';

  @override
  String get settingsSectionModulesSummary => '安装和管理可选模块。';

  @override
  String get settingsSectionRules => '内容规则';

  @override
  String get settingsSectionRulesSummary => '自动为剪贴项添加标签、跳过、保护或设置过期。';

  @override
  String get settingsSectionQuickAccess => '快速访问';

  @override
  String get settingsSectionQuickAccessSummary => '键盘、快捷设置图块和主屏幕小组件。';

  @override
  String get settingsSectionSnippets => '片段';

  @override
  String get settingsSectionSnippetsSummary => '可重复使用的文本和模板。';

  @override
  String get settingsSectionTransforms => '文本转换';

  @override
  String get settingsSectionTransformsSummary => '收藏、流程和粘贴时的默认转换。';

  @override
  String get settingsSectionAiTools => 'AI 工具';

  @override
  String get settingsSectionAiToolsSummary => '连接 AI 助手并控制它们可以查看的内容。';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary => '收集多次复制的内容并按顺序粘贴。';

  @override
  String pasteQueuePasted(int position, int total) {
    return '已粘贴第 $position 项，共 $total 项';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return '第 $position 项（共 $total 项）已在剪贴板中';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已粘贴 $count 项',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 项已在剪贴板中',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => '队列已完成';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: '已添加，队列中有 $total 项',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => '队列为空，未粘贴任何内容';

  @override
  String get pasteQueueMixed => '“全部粘贴”仅适用于纯文本或纯文件的队列';

  @override
  String get pasteQueueSeveralFiles => '暂不支持一次粘贴多个文件，请逐个粘贴。';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 项已不可用',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => '“粘贴下一项”仅复制';

  @override
  String get pasteQueueAccessibilityMessage =>
      '允许“辅助功能”即可自动粘贴。在此之前，该项已在剪贴板中，请手动粘贴。';

  @override
  String get pasteQueuePasteUnsupported => '无法自动粘贴。该项已在剪贴板中，请用常用快捷键粘贴。';

  @override
  String get pasteQueueTargetElevated => '无法粘贴到以管理员身份运行的应用。该项仍在队列中，也在剪贴板中。';

  @override
  String get pasteQueueNoTarget => '没有可粘贴的目标应用。该项仍在队列中，也在剪贴板中。';

  @override
  String get pasteQueueCollectingStarted => '正在收集复制内容';

  @override
  String get pasteQueueCollectingStopped => '已停止收集';

  @override
  String get pasteQueueFailed => '无法完成 Paste Queue 操作';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 项待粘贴',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 项待粘贴 · 收集中',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => '收集中';

  @override
  String get pasteQueueFinishedSummary => '已完成';

  @override
  String pasteQueueFull(int max) {
    return '队列已满（$max）';
  }

  @override
  String get pasteQueueFullMessage => '不再添加新的复制内容。未移除任何内容。';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue 为空';

  @override
  String get pasteQueueEmptyMessage => '收集复制内容以按顺序粘贴，或从历史记录中添加项目。';

  @override
  String get pasteQueueCollectingEmptyTitle => '复制内容即可添加';

  @override
  String get pasteQueueCollectingEmptyMessage => '在停止收集之前，每次复制的内容都会加入队列。';

  @override
  String get pasteQueueStartCollecting => '开始收集';

  @override
  String get pasteQueueStopCollecting => '停止收集';

  @override
  String get pasteQueueLoading => '正在加载队列…';

  @override
  String get pasteQueueLoadFailed => '无法加载队列';

  @override
  String get pasteQueuePasteNext => '粘贴下一项';

  @override
  String get pasteQueueCopyNext => '复制下一项';

  @override
  String get pasteQueuePasteAll => '全部粘贴';

  @override
  String get pasteQueueCopyAll => '全部复制';

  @override
  String get pasteQueueReverse => '反转顺序';

  @override
  String get pasteQueueStepBack => '退回一项';

  @override
  String get pasteQueueSkip => '跳过';

  @override
  String get pasteQueueRemovePasted => '移除已粘贴项';

  @override
  String get pasteQueueCleared => '队列已清空';

  @override
  String get pasteQueueUndo => '撤销';

  @override
  String get pasteQueueStartFresh => '重新开始';

  @override
  String get pasteQueueAppend => '追加';

  @override
  String get pasteQueueRestartTitle => '开始新的收集？';

  @override
  String get pasteQueueRestartMessage => '“重新开始”会移除已粘贴项，“追加”会保留它们。';

  @override
  String pasteQueueStale(String time) {
    return '$time 的队列';
  }

  @override
  String get pasteQueueStatusPasted => '已粘贴';

  @override
  String get pasteQueueStatusSkipped => '已跳过';

  @override
  String get pasteQueueStatusUnavailable => '已不可用';

  @override
  String get pasteQueueEdited => '已编辑';

  @override
  String get pasteQueueSecret => '机密';

  @override
  String get pasteQueueKindImage => '图片';

  @override
  String get pasteQueueKindFiles => '文件';

  @override
  String pasteQueueRenderedAt(String time) {
    return '生成于 $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return '第 $position 项：$content';
  }

  @override
  String pasteQueueMoved(int position) {
    return '已移到第 $position 位';
  }

  @override
  String get pasteQueueMoveUp => '上移';

  @override
  String get pasteQueueMoveDown => '下移';

  @override
  String get pasteQueueMoveToTop => '移到顶部';

  @override
  String get pasteQueueMoveToBottom => '移到底部';

  @override
  String get pasteQueueReorder => '拖动以重新排序';

  @override
  String get pasteQueueMoreActions => '更多操作';

  @override
  String get pasteQueuePasteNow => '立即粘贴';

  @override
  String get pasteQueueEdit => '编辑文本';

  @override
  String get pasteQueueUseOriginal => '使用原始文本';

  @override
  String get pasteQueueRemove => '从队列中移除';

  @override
  String get pasteQueueEditTitle => '编辑队列中的文本';

  @override
  String get pasteQueueEditMessage => '更改仅适用于此队列，历史记录保留原始内容。';

  @override
  String get pasteQueueShortcutPromptTitle => '设置快捷键';

  @override
  String get pasteQueueShortcutPromptMessage => '选择快捷键，无需打开 CopyPaste 即可收集和粘贴。';

  @override
  String get pasteQueueShortcutToggleCollecting => '开始或停止收集';

  @override
  String get pasteQueueShortcutPasteNextPlain => '以纯文本粘贴下一项';

  @override
  String get pasteQueueShortcutDescription => '在任何应用中都可用的全局快捷键。';

  @override
  String get pasteQueueShortcutHeldByApp =>
      '另一个 CopyPaste 快捷键已使用此组合。已保留之前的快捷键。';

  @override
  String get pasteQueueShortcutInUse => '另一个应用正在使用此组合。已保留之前的快捷键，请选择其他组合。';

  @override
  String get pasteQueueShortcutRejected => '系统拒绝了此组合。已保留之前的快捷键。';

  @override
  String get pasteQueueShortcutAltGr =>
      '在许多键盘布局中 Ctrl+Alt 用于输入字符（AltGr）。请选择其他组合。';

  @override
  String get pasteQueueShortcutUnavailable =>
      '此会话不支持全局快捷键。请在窗口或 Quick Paste 中使用队列。';

  @override
  String get pasteQueueShortcutFailed => '无法设置此快捷键。';

  @override
  String get pasteQueueEnableTitle => '启用 Paste Queue';

  @override
  String get pasteQueueEnableDescription => '关闭后，队列快捷键将被释放，也不会收集任何内容。';

  @override
  String get pasteQueuePasteNextStyleTitle => '“粘贴下一项”的操作';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      '粘贴到前台应用需要发送按键的权限。“仅剪贴板”则由您自行粘贴。';

  @override
  String get pasteQueuePasteIntoApp => '粘贴到前台应用';

  @override
  String get pasteQueueClipboardOnly => '仅放入剪贴板';

  @override
  String get pasteQueueInsertTitle => '新项目添加到';

  @override
  String get pasteQueueInsertDescription => '收集和添加的项目加入队列的位置。';

  @override
  String get pasteQueueInsertEnd => '末尾（先进先出）';

  @override
  String get pasteQueueInsertStart => '开头（后进先出）';

  @override
  String get pasteQueueEndBehaviorTitle => '最后一项之后';

  @override
  String get pasteQueueEndBehaviorDescription => '所有项目粘贴完成后“粘贴下一项”的操作。';

  @override
  String get pasteQueueEndStop => '停止';

  @override
  String get pasteQueueEndLoop => '从头开始';

  @override
  String get pasteQueueEndKeepLast => '继续粘贴最后一项';

  @override
  String get pasteQueueSeparatorTitle => '“全部粘贴”分隔符';

  @override
  String get pasteQueueSeparatorDescription => '一次粘贴全部时放在各项之间。';

  @override
  String get pasteQueueSeparatorNewLine => '换行';

  @override
  String get pasteQueueSeparatorBlankLine => '空行';

  @override
  String get pasteQueueSeparatorTab => '制表符';

  @override
  String get pasteQueueSeparatorSpace => '空格';

  @override
  String get pasteQueueSeparatorComma => '逗号';

  @override
  String get pasteQueueSeparatorCommaSpace => '逗号和空格';

  @override
  String get pasteQueueSeparatorNone => '无';

  @override
  String get pasteQueueSeparatorCustom => '自定义';

  @override
  String get pasteQueueCustomSeparatorPlaceholder => '最多 32 个字符；可使用 \\n 和 \\t';

  @override
  String get pasteQueuePlainTextTitle => '始终以纯文本粘贴';

  @override
  String get pasteQueuePlainTextDescription => '去除队列项目的格式。';

  @override
  String get pasteQueueIdleStopTitle => '停止收集的时间';

  @override
  String get pasteQueueIdleStopDescription => '若在此时长内没有复制，将停止收集。项目会保留。';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes 分钟';
  }

  @override
  String get pasteQueueIdleNever => '从不';

  @override
  String get pasteQueueMaxEntriesTitle => '队列大小上限';

  @override
  String get pasteQueueMaxEntriesDescription => '队列满后不再添加项目，也不会丢弃任何内容。';

  @override
  String get pasteQueueRestoreTitle => '完成后恢复之前的剪贴板';

  @override
  String get pasteQueueRestoreDescription => '恢复队列开始粘贴之前您复制的内容。';

  @override
  String get pasteQueueSettingsKeywords => 'paste queue 队列 收集 顺序 快捷键 分隔符';

  @override
  String get settingsSectionAbout => '关于';

  @override
  String get settingsSectionAboutSummary => '版本和应用更新。';

  @override
  String get settingsGeneralDescription => '语言、自动启动和应用可见性。';

  @override
  String get settingsModulesDescription => 'CopyPaste 的可选功能。';

  @override
  String get settingsClipboardDescription => '捕获剪贴板更改并管理历史记录限制。';

  @override
  String get settingsDataDescription => '导出、备份和恢复剪贴板历史记录。';

  @override
  String get settingsSyncDescription => '控制与已配对设备的同步。';

  @override
  String get settingsPrivacyDescription => '控制捕获哪些应用的内容，并保护剪贴板内容。';

  @override
  String get settingsNotificationsDescription => '为捕获的剪贴项选择通知和声音。';

  @override
  String get settingsAboutDescription => 'CopyPaste 版本和应用更新。';

  @override
  String get settingsQuickPasteDescription => '无需切换窗口，随时打开剪贴板历史记录。';

  @override
  String get settingsLoadingMessage => '正在加载设置。';

  @override
  String get settingsUnavailableTitle => '设置不可用';

  @override
  String get settingsNoResultsTitle => '未找到设置';

  @override
  String get settingsNoResultsMessage => '请尝试其他搜索词。';

  @override
  String get settingsAttentionTitle => '设置需要处理';

  @override
  String get settingsSearchPlaceholder => '搜索设置';

  @override
  String get settingsBackToSettings => '返回设置';

  @override
  String get settingsStartOnBootTitle => '开机时启动';

  @override
  String get settingsStartAtLoginTitle => '登录时启动';

  @override
  String get settingsStartupSearchDescription => '自动启动 CopyPaste。';

  @override
  String get settingsStartupKeywords =>
      '启动 登录 开机 打开 自动 自启动 startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      '已在任务管理器中禁用。请在“设置 → 应用 → 启动”中启用 CopyPaste。';

  @override
  String get settingsStartupMacApproval => '请在“系统设置 → 通用 → 登录项”中允许 CopyPaste。';

  @override
  String get settingsStartupUnavailable => '此安装不支持自动启动。';

  @override
  String get settingsStartupAndroidDescription => '重新启动设备后恢复已启用的后台服务。';

  @override
  String get settingsStartupDesktopDescription => '登录时自动打开 CopyPaste。';

  @override
  String get settingsShowInDock => '在程序坞中显示';

  @override
  String get settingsShowInTaskbar => '在任务栏中显示';

  @override
  String get settingsTaskbarVisibilityDescription => '显示用于切换窗口的应用图标。';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      '程序坞 任务栏 应用 图标 可见性 dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => '在菜单栏中显示';

  @override
  String get settingsShowInSystemTray => '在系统托盘中显示';

  @override
  String get settingsTrayVisibilityDescription => '显示 CopyPaste 和捕获控制的快速入口。';

  @override
  String get settingsTrayVisibilityKeywords =>
      '托盘 菜单栏 状态 图标 可见性 tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle => '无法更改可见性';

  @override
  String get settingsModulesSearchDescription => '安装、更新和移除可选模块。';

  @override
  String get settingsModulesKeywords => '功能 命令 扩展 features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Android 后台捕获';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      '“完整”或“受限”模式，使用 Shizuku 或 ADB 设置。';

  @override
  String get settingsAndroidCaptureKeywords =>
      '后台 权限 设置 background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      '选择“完整”或“受限”模式，并管理一次性的 Shizuku 或 ADB 设置。';

  @override
  String get settingsOpenSetup => '打开设置向导';

  @override
  String get settingsSaveScreenshotsTitle => '保存截屏';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      '自动将新的 Android 截屏保存到历史记录。';

  @override
  String get settingsSaveScreenshotsKeywords =>
      '图片 照片 捕获 权限 images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription => '自动将新截屏添加到历史记录。';

  @override
  String get settingsScreenshotAccessTitle => '截屏访问权限';

  @override
  String get settingsScreenshotAccessDescription => '允许访问所有照片和通知，以便在后台保存新截屏。';

  @override
  String get settingsAllowAccess => '允许访问';

  @override
  String get settingsScreenshotCaptureStoppedTitle => '截屏捕获未运行';

  @override
  String get settingsScreenshotCaptureStoppedMessage => '新截屏未被保存。请恢复捕获以重新启动服务。';

  @override
  String get settingsResumeCapture => '恢复捕获';

  @override
  String get settingsClipboardCaptureTitle => '剪贴板捕获';

  @override
  String get settingsClipboardCaptureSearchDescription => '暂停或恢复剪贴板捕获。';

  @override
  String get settingsCaptureStateUnavailable => '状态不可用';

  @override
  String get settingsCaptureStoppedTitle => '捕获未运行';

  @override
  String get settingsCaptureStoppedMessage => 'CopyPaste 目前未在此设备上接收剪贴板更改。';

  @override
  String get settingsRetentionTitle => '保留期限';

  @override
  String get settingsRetentionDescription => '自动移除未置顶的旧剪贴项。';

  @override
  String get settingsRetentionForever => '永久保留';

  @override
  String get settingsStorageQuotaTitle => '存储配额';

  @override
  String get settingsStorageQuotaDescription => '未置顶历史记录可使用的最大本地存储空间。';

  @override
  String get settingsStorageQuotaKeywords => '磁盘 空间 限制 disk space limit';

  @override
  String get settingsMaxFileSizeTitle => '最大文件大小';

  @override
  String get settingsMaxFileSizeSearchDescription => '添加和下载文件的大小限制。';

  @override
  String get settingsMaxFileSizeKeywords =>
      '导入 下载 传输 大小 限制 import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      '添加和下载文件的大小限制。仅当你选择“下载”时才会传输文件内容。';

  @override
  String get settingsHistoryFilesTitle => '历史记录文件';

  @override
  String get settingsHistoryFilesSearchDescription => '导出历史记录、创建备份或恢复备份。';

  @override
  String get settingsHistoryFilesKeywords =>
      '文本 加密 备份 数据 text encrypted backup data';

  @override
  String get settingsExportTextTitle => '导出文本历史记录';

  @override
  String get settingsExportTextDescription => '将文本剪贴项保存到可移植文件中。';

  @override
  String get settingsExportAction => '导出';

  @override
  String get settingsBackupTitle => '加密备份';

  @override
  String get settingsBackupDescription => '保存此设备的完整本地历史记录。';

  @override
  String get settingsCreateBackupAction => '创建备份';

  @override
  String get settingsRestoreTitle => '恢复备份';

  @override
  String get settingsRestoreDescription => '用加密备份替换本地历史记录。';

  @override
  String get settingsRestoreAction => '恢复';

  @override
  String get settingsRestoreDialogTitle => '替换本地历史记录？';

  @override
  String get settingsRestoreDialogMessage =>
      '所选加密备份将替换此设备上的剪贴板历史记录。当前设置和设备身份将保留。';

  @override
  String get settingsRestoreFailed => '无法恢复备份。';

  @override
  String get settingsSyncSwitchTitle => '同步';

  @override
  String get settingsSyncSwitchDescription => '已配对设备同步的总开关。';

  @override
  String get settingsSyncSearchDescription => '已配对设备同步。';

  @override
  String get settingsInstantClipboardTitle => '即时剪贴板';

  @override
  String get settingsInstantClipboardSearchDescription => '自动复制来自其他设备的新剪贴项。';

  @override
  String get settingsInstantClipboardKeywords =>
      '自动 复制 粘贴 同步 接收 automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription => '自动将其他设备上较新的剪贴项复制到此设备的剪贴板。';

  @override
  String get settingsLanVisibilityTitle => '局域网可见性';

  @override
  String get settingsLanVisibilityDescription => '允许附近的设备发现此设备。';

  @override
  String get settingsLanVisibilityKeywords =>
      '本地 网络 发现 local network discovery';

  @override
  String get settingsExcludedAppsTitle => '排除的应用';

  @override
  String get settingsExcludedAppsSearchDescription => '跳过对排除应用的自动捕获。';

  @override
  String get settingsExcludedAppsKeywords =>
      '隐私 应用 标识符 privacy app identifiers';

  @override
  String get settingsExcludedAppsMacDescription =>
      '这些应用处于活动状态时跳过自动捕获。后台复制可能会绕过排除规则。';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      '根据可识别的剪贴板所有者跳过自动捕获。设置了排除规则但无法识别来源时，将暂停捕获。';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      '由于 Android 无法识别来源应用，设置排除规则后将跳过自动捕获。';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'macOS、Windows、Android 和 Linux 支持应用排除。';

  @override
  String get settingsExcludedAppsAdd => '添加';

  @override
  String get settingsExcludedAppsEmpty => '未排除任何应用。';

  @override
  String settingsExcludedAppsRemove(String app) {
    return '移除 $app';
  }

  @override
  String get settingsExcludeDialogTitle => '排除应用';

  @override
  String get settingsExcludeDialogAction => '排除';

  @override
  String get settingsExcludeDialogPlaceholder => '应用标识符';

  @override
  String get settingsExcludeFailed => '无法排除该应用。';

  @override
  String get settingsSkipSecretTitle => '跳过机密剪贴板内容';

  @override
  String get settingsSkipSecretDescription => '不将来源应用标记为机密的内容存入历史记录。';

  @override
  String get settingsSkipSecretKeywords =>
      '机密 密码 敏感 隐藏 遮盖 secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => '跳过临时剪贴板内容';

  @override
  String get settingsSkipTransientDescription => '不将来源应用标记为临时的副本存入历史记录。';

  @override
  String get settingsSkipTransientKeywords =>
      '临时 暂时 隐私 transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => '阻止截屏';

  @override
  String get settingsBlockScreenshotsDescription => '阻止对 CopyPaste 进行截屏和录屏。';

  @override
  String get settingsBlockScreenshotsUnsupported => '此平台不支持阻止截屏。';

  @override
  String get settingsBlockScreenshotsKeywords =>
      '隐私 屏幕 捕获 保护 配对 二维码 安全 代码 privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => '复制时通知';

  @override
  String get settingsNotifyOnCopyDescription => '后台捕获后显示通知。';

  @override
  String get settingsNotificationPreviewTitle => '显示剪贴板内容';

  @override
  String get settingsNotificationPreviewDescription => '在复制通知中包含剪贴项预览。';

  @override
  String get settingsNotificationPreviewKeywords =>
      '通知 预览 文本 图片 隐私 notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => '复制时播放声音';

  @override
  String get settingsSoundOnCopyDescription => '成功捕获后播放系统反馈。';

  @override
  String get settingsVersionTitle => '版本';

  @override
  String get settingsVersionDescription => '已安装的 CopyPaste 版本。';

  @override
  String get settingsVersionUnavailable => '不可用';

  @override
  String get settingsUpdatesTitle => '应用更新';

  @override
  String get settingsUpdatesSearchDescription => '检查并安装 CopyPaste 更新。';

  @override
  String get settingsUpdatesKeywords => '版本 发布 version github release';

  @override
  String get settingsUpdateIdle => '检查是否有更新的 CopyPaste 版本。';

  @override
  String get settingsUpdateChecking => '正在检查更新。';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste 已是最新版本。';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return '版本 $version 已是最新。';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return '版本 $available 现已推出。此设备的版本为 $current。';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return '正在下载版本 $version · $percent%。';
  }

  @override
  String get settingsUpdatePreparingModules => '正在准备已安装模块的更新。';

  @override
  String settingsUpdateInstalling(String version) {
    return '正在安装版本 $version。';
  }

  @override
  String get settingsUpdateNeedsAttention => '应用更新需要处理。';

  @override
  String get settingsUpdateNowAction => '立即更新';

  @override
  String get settingsUpdateOpenReleaseAction => '打开发布页面';

  @override
  String get settingsUpdateCheckingAction => '正在检查';

  @override
  String get settingsUpdateDownloadingAction => '正在下载';

  @override
  String get settingsUpdateInstallingAction => '正在安装';

  @override
  String get settingsUpdateRestartAction => '重新启动 CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => '再次检查';

  @override
  String get settingsUpdateNoneTitle => '没有可用更新';

  @override
  String get settingsUpdateLatest => 'CopyPaste 已是最新版本。';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version 已是最新版本。';
  }

  @override
  String get settingsUpdateAvailableTitle => '有可用更新';

  @override
  String get settingsUpdateReady => '新版本的 CopyPaste 已可安装。';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version 已可安装。';
  }

  @override
  String get settingsUpdateUnavailableTitle => '无法更新';

  @override
  String get settingsUpdateUnavailableMessage => '此安装无法自动更新。';

  @override
  String get settingsUpdateCheckFailedTitle => '检查更新失败';

  @override
  String get settingsUpdateCheckFailedMessage => 'CopyPaste 无法检查更新。';

  @override
  String get settingsQuickPasteSetupTitle => '设置 Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      '请启用已签名的 GNOME 或 KDE 配套组件，并允许键盘控制以返回上一个应用。';

  @override
  String get settingsQuickPasteDesktopIntegration => '桌面集成';

  @override
  String get settingsQuickPasteAllowKeyboard => '允许键盘控制';

  @override
  String get settingsQuickPasteCheckAgain => '再次检查';

  @override
  String get settingsQuickPasteLoading => '正在加载 Quick Paste 设置。';

  @override
  String get settingsQuickPasteUnavailableTitle => 'Quick Paste 不可用';

  @override
  String get settingsQuickPasteUnavailableMessage => '此设备不提供 Quick Paste 窗口。';

  @override
  String get settingsQuickPasteShortcutTitle => '打开 Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      '配置 Quick Paste 全局快捷键。';

  @override
  String get settingsQuickPasteShortcutKeywords => '键盘 快捷键 热键 keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'CopyPaste 在后台运行时，即可使用该快捷键。';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return '已注册的快捷键：$shortcut';
  }

  @override
  String get settingsAutoPasteTitle => '自动粘贴';

  @override
  String get settingsAutoPasteSearchDescription => '将所选剪贴项粘贴到上一个应用中。';

  @override
  String get settingsAutoPasteKeywords => '辅助功能 自动 accessibility automatic';

  @override
  String get settingsAutoPasteDescription => '选择剪贴项后会返回上一个应用并粘贴。';

  @override
  String get settingsAccessibilityRequiredTitle => '需要辅助功能权限';

  @override
  String get settingsAccessibilityRequiredMessage => '在授予权限之前，选择剪贴项只会复制而不会粘贴。';

  @override
  String get settingsQuickPasteAttentionTitle => 'Quick Paste 需要处理';

  @override
  String get settingsShortcutRecording => '请按快捷键…';

  @override
  String get settingsShortcutNotSet => '未设置';

  @override
  String get settingsShortcutReset => '重置快捷键';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 天',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => '正在加载设备…';

  @override
  String get devicesUnavailableTitle => '设备不可用';

  @override
  String get devicesNearbyTitle => '附近的设备';

  @override
  String get devicesScan => '扫描';

  @override
  String get devicesScanning => '正在扫描…';

  @override
  String get devicesNoneNearby => '附近没有设备';

  @override
  String get devicesThisDevice => '此设备';

  @override
  String get devicesTrusted => '受信任';

  @override
  String get devicesConnect => '连接';

  @override
  String devicesCardPing(String ping) {
    return '延迟：$ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return '延迟：\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => '重命名此设备';

  @override
  String get devicesNamePlaceholder => '设备名称';

  @override
  String get devicesRenameFailed => '无法保存设备名称。';

  @override
  String devicesRevokeDialogTitle(String device) {
    return '撤销 $device？';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return '解除与 $device 的配对？';
  }

  @override
  String get devicesRevokeDialogMessage =>
      '这会将该设备从你的设备组中移除，并永久阻止此配对。其他设备重新连接时会应用此移除操作。';

  @override
  String get devicesUnpairDialogMessage =>
      '这会将该设备从你的设备组中移除。其他设备重新连接时会应用此移除操作。再次添加该设备需要重新确认。';

  @override
  String get devicesRevoke => '撤销';

  @override
  String get devicesUnpair => '解除配对';

  @override
  String get devicesActionFailedMessage => '无法完成设备操作。';

  @override
  String get devicesCloseDetails => '关闭设备详情';

  @override
  String get devicesDetailType => '设备类型';

  @override
  String get devicesDetailOperatingSystem => '操作系统';

  @override
  String get devicesDetailModel => '型号';

  @override
  String get devicesDetailAppVersion => 'CopyPaste 版本';

  @override
  String get devicesDetailProtocolVersion => '协议版本';

  @override
  String get devicesDetailStatus => '状态';

  @override
  String get devicesDetailPing => '延迟';

  @override
  String get devicesDetailMeasured => '测量时间';

  @override
  String get devicesDetailLastSeen => '上次发现';

  @override
  String get devicesDetailLanEndpoint => '局域网端点';

  @override
  String get devicesDetailDeviceId => '设备 ID';

  @override
  String get devicesDetailPairingId => '配对 ID';

  @override
  String get devicesDetailProfileProvenance => '信息来源';

  @override
  String get devicesDetailProfileTrust => '信息可信度';

  @override
  String get devicesDetailProfileUpdated => '信息更新时间';

  @override
  String get devicesValueUnavailable => '不可用';

  @override
  String get devicesRenameDevice => '重命名设备';

  @override
  String get devicesSyncNow => '立即同步';

  @override
  String get devicesRevokePairing => '撤销配对';

  @override
  String get devicesLocal => '本地';

  @override
  String get devicesTrustAuthenticated => '已认证';

  @override
  String get devicesTrustUnverified => '未验证';

  @override
  String get devicesProvenanceSelfReported => '自行报告';

  @override
  String get devicesProvenanceObserved => '观测';

  @override
  String get devicesProvenanceMeasured => '实测';

  @override
  String get devicesConnectionOnline => '在线';

  @override
  String get devicesConnectionOffline => '离线';

  @override
  String get devicesConnectionVisible => '可见';

  @override
  String get devicesConnectionNotSeen => '未发现';

  @override
  String get devicesConnectionUnknown => '未知';

  @override
  String get devicesPeerAvailable => '可用';

  @override
  String get devicesPeerVisibleNow => '当前可见';

  @override
  String get devicesPeerStatusUnknown => '状态未知';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => '台式电脑';

  @override
  String get devicesClassLaptop => '笔记本电脑';

  @override
  String get devicesClassPhone => '手机';

  @override
  String get devicesClassTablet => '平板电脑';

  @override
  String get devicesClassUnknown => '设备';

  @override
  String get devicesUnknownOs => '未知操作系统';

  @override
  String get devicesAddDevice => '添加设备';

  @override
  String get devicesScanQr => '扫描二维码';

  @override
  String get devicesEnterCode => '输入代码';

  @override
  String get devicesScannerUnavailableTitle => '扫描器不可用';

  @override
  String get devicesActionFailedTitle => '操作失败';

  @override
  String get devicesRefreshFailedTitle => '设备刷新失败';

  @override
  String get devicesNearbyPairingUnavailable => '附近配对不可用。';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      '附近配对不可用。请使用“添加设备”显示配对代码。';

  @override
  String get pairingTitle => '配对';

  @override
  String get pairingPairDevice => '配对设备';

  @override
  String get pairingScanQrCode => '扫描二维码';

  @override
  String get pairingEnterPairingCode => '输入配对代码';

  @override
  String get pairingConnectNearbyDevice => '连接附近的设备';

  @override
  String get pairingSendingRequest => '正在发送连接请求…';

  @override
  String get pairingEmptyTitle => '选择配对方式';

  @override
  String get pairingEmptyMessage => '配对控件将显示在这里。';

  @override
  String get pairingCreatingInvitation => '正在创建配对邀请。';

  @override
  String get pairingCode => '配对代码';

  @override
  String get pairingCodeHint => '输入另一台设备上显示的 8 位代码。';

  @override
  String get pairingDeviceAddress => '设备地址';

  @override
  String get pairingDeviceAddressHint => '使用另一台设备报告的 host:port 地址。';

  @override
  String get pairingCheckDetailsTitle => '检查配对信息';

  @override
  String get pairingActionFailed => '配对操作失败';

  @override
  String get pairingConnectingDevice => '正在连接设备…';

  @override
  String get pairingQrScannerTitle => '二维码扫描器';

  @override
  String get pairingCameraPreviewPlaceholder => '相机预览将显示在这里。';

  @override
  String get pairingScannerNeedsAttention => '扫描器需要处理';

  @override
  String get pairingCodeIncomplete => '请输入完整的 8 位配对代码。';

  @override
  String get pairingAddressMissing => '请输入设备的 host:port 地址。';

  @override
  String get pairingWaitingForOtherDevice => '正在等待另一台设备';

  @override
  String get pairingInviteInstructions => '在另一台设备上，选择“添加设备 → 输入代码”，或扫描二维码。';

  @override
  String get pairingDetailsTitle => '配对信息';

  @override
  String get pairingAddress => '地址';

  @override
  String get pairingVerificationCode => '验证码';

  @override
  String get pairingVerificationCodeHint => '仅当两台设备上的验证码一致时才确认。';

  @override
  String get pairingReject => '拒绝';

  @override
  String get pairingConfirm => '确认';

  @override
  String get pairingAccept => '接受';

  @override
  String get pairingJoinDevice => '加入设备';

  @override
  String get pairingStartScanner => '启动扫描器';

  @override
  String get pairingActionNotStarted => '无法开始配对操作。';

  @override
  String get pairingStateHandshaking => '正在验证设备';

  @override
  String get pairingStateAwaitingConfirmation => '请在两台设备上确认';

  @override
  String get pairingStateConfirmed => '设备已配对';

  @override
  String get pairingStateRejected => '配对已拒绝';

  @override
  String get pairingStateCancelled => '配对已取消';

  @override
  String get pairingStateTimedOut => '配对超时';

  @override
  String get pairingStateFailed => '配对失败';

  @override
  String get pairingHandshakingDescription => '设备正在建立安全通道。';

  @override
  String get pairingAwaitingConfirmationDescription => '请比对受保护的验证码，然后在两台设备上确认。';

  @override
  String get pairingConfirmedDescription => '设备已连接。你的设备组成员信息将自动同步。';

  @override
  String get pairingNoDeviceAdded => '未添加任何设备。';

  @override
  String get pairingTimedOutDescription => '请重新开始配对以继续。';

  @override
  String get pairingIdleDescription => '开始配对以连接设备。';

  @override
  String get pairingProtectedUnavailableTitle => '受保护的配对不可用';

  @override
  String get pairingProtectedUnavailableMessage => '此配对会话必须在专用的受保护窗口中打开。';

  @override
  String get pairingCancel => '取消配对';

  @override
  String get pairingOrEnterCode => '或输入配对代码';

  @override
  String get pairingJoinSecurely => '安全加入';

  @override
  String get pairingProtectedWaitingDescription =>
      '在此受保护窗口中显示二维码、扫描二维码或输入配对代码。';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return '$seconds 秒后过期';
  }

  @override
  String get pairingInvalidQr => '请扫描 CopyPaste 配对二维码。';

  @override
  String get pairingGoogleScannerUnavailable => 'Google 扫描器不可用。请改为输入配对代码。';

  @override
  String get pairingScannerCouldNotOpen => '无法打开扫描器。请改为输入配对代码。';

  @override
  String get pairingSecurePresentationUnavailable => '无法安全显示配对信息。';

  @override
  String get pairingVerificationCodeRequired => '请先显示并比对验证码。';

  @override
  String get syncStatusUnavailable => '同步状态不可用';

  @override
  String get syncStatusOff => '同步已关闭';

  @override
  String get syncStatusWaitingForSync => '等待同步';

  @override
  String get syncStatusSyncing => '正在同步';

  @override
  String get syncStatusUpToDate => '已是最新';

  @override
  String get syncStatusInterrupted => '同步已中断';

  @override
  String get syncStatusPartiallySynchronized => '部分已同步';

  @override
  String get syncStatusRetrying => '正在重试';

  @override
  String get syncStatusWaitingToSync => '待同步';

  @override
  String get syncStatusWaitingForDevice => '等待设备';

  @override
  String get syncStatusNoPairedDevices => '没有已配对的设备';

  @override
  String get syncStatusDevicesOffline => '设备离线';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · 活动日志不可用';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '同步已关闭 · $count 个问题',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count 台离线',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 台设备需要处理',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '设备离线 · $count 个问题',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => '正在检查连接';

  @override
  String get syncConnectionUnknown => '连接状态未知';

  @override
  String get syncLogDeviceOffline => '设备已离线。';

  @override
  String get syncLogConnectionRestored => '连接已恢复。';

  @override
  String get syncLogProblemDetected => '检测到同步问题。';

  @override
  String get syncLogProblemResolved => '同步问题已解决。';

  @override
  String get syncLogFailed => '同步失败。';

  @override
  String get syncLogRecovered => '同步已恢复。';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent 个剪贴项',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received 个剪贴项',
    );
    return '已发送 $_temp0 · 已接收 $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return '同步：$status';
  }

  @override
  String get syncDetailsTitle => '同步';

  @override
  String get syncDetailsClose => '关闭同步详情';

  @override
  String get syncNoPairedDevicesMessage => '配对设备以同步剪贴板历史记录。';

  @override
  String get syncActivityLogUnavailableTitle => '活动日志不可用';

  @override
  String get syncActivityLogUnavailableMessage => '无法读取或保存同步活动。';

  @override
  String get syncDetailsDevicesHeading => '设备';

  @override
  String get syncDetailsActivityHeading => '活动';

  @override
  String get syncDetailsNoRecentProblems => '最近没有同步问题';

  @override
  String get syncDetailsUnknownPeer => '已配对的设备';

  @override
  String get syncPeerConnection => '连接';

  @override
  String get syncPeerLastCheckStarted => '上次检查开始时间';

  @override
  String get syncPeerItemsOverSizeLimit => '超出大小限制的项目';

  @override
  String syncPeerLastSeen(String time) {
    return '上次发现：$time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return '上次同步：$time';
  }

  @override
  String get syncPeerLastSuccessfulSync => '上次成功同步';

  @override
  String get syncPeerConnectionDetails => '连接详情';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return '$device 的连接详情';
  }

  @override
  String get historyKindText => '文本';

  @override
  String get historyKindLink => '链接';

  @override
  String get historyKindEmail => '电子邮件';

  @override
  String get historyKindColor => '颜色';

  @override
  String get historyKindPhone => '电话';

  @override
  String get historyKindCode => '代码';

  @override
  String get historyKindPath => '路径';

  @override
  String get historyKindCredentials => '凭据';

  @override
  String get historyKindImage => '图片';

  @override
  String get historyKindFile => '文件';

  @override
  String get historyKindOther => '其他';

  @override
  String get historySortNewest => '最新优先';

  @override
  String get historySortOldest => '最早优先';

  @override
  String get historySortRelevance => '最佳匹配';

  @override
  String get historyErrorUpdatesUnavailable => '历史记录更新暂时不可用。';

  @override
  String get historyErrorBulkDelete => '部分所选剪贴项无法删除。请重试。';

  @override
  String get historyErrorBulkPin => '部分所选剪贴项的置顶状态无法更新。请重试。';

  @override
  String get historyErrorRefresh => '无法刷新历史记录。请重试。';

  @override
  String get historyErrorLoad => '无法加载历史记录。请重试。';

  @override
  String get historyErrorLoadMore => '无法加载更多历史记录。';

  @override
  String get historyErrorLoadEarlier => '无法加载更早的历史记录。';

  @override
  String get historyErrorOpenClip => '无法打开此剪贴项。';

  @override
  String get historyErrorCopyPlainText => '无法复制纯文本。';

  @override
  String get historyErrorCopyClip => '无法复制此剪贴项。';

  @override
  String get historyErrorCopyText => '无法复制此文本。';

  @override
  String get historyErrorPrepareFile => '无法下载此文件。请检查来源设备后重试。';

  @override
  String get historyErrorDownloadFile => '无法下载此文件。';

  @override
  String get historyErrorSelectFiles => '无法选择文件。请重试。';

  @override
  String get historyErrorReadSelectedFile => '无法读取所选文件。请检查访问权限和可用存储空间。';

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
      other: '$total 个文件中有 $failed 个无法导入。第一个：$name。$reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate => '无法更新剪贴项的机密状态。';

  @override
  String get historyErrorPinUpdate => '无法更新置顶状态。';

  @override
  String get historyErrorPinnedOrderSave => '无法保存置顶顺序。请重试。';

  @override
  String get historyErrorPinnedOrderRefresh => '顺序已保存，但无法刷新历史记录。';

  @override
  String get historyErrorDeleteClip => '无法删除此剪贴项。';

  @override
  String get historyErrorDeleteAll => '无法删除历史记录。';

  @override
  String get historyErrorBulkDeleteFallback => '无法删除所选剪贴项。';

  @override
  String get historyOcrErrorUnavailable => '文字识别不可用。请在设置中检查该模块后重试。';

  @override
  String get historyOcrErrorStart => '无法启动文字识别。请重试。';

  @override
  String get historyOcrErrorRecognize => '无法识别图片中的文字。请重试。';

  @override
  String get historyOcrRecognizing => '正在识别图片中的文字。';

  @override
  String get historyOcrFailedTitle => '文字识别失败';

  @override
  String get historyOcrNoText => '未找到文字';

  @override
  String get historyOcrTitle => '图片文字';

  @override
  String get historyOcrTextCopied => '已复制识别出的文字。';

  @override
  String get historyOcrCopyError => '无法复制文字。请重试。';

  @override
  String get historyTagsErrorLoad => '无法加载标签。请重试。';

  @override
  String get historyTagsErrorSave => '无法保存标签更改。请使用不超过 64 个字符的唯一名称，并选择已置顶的剪贴项。';

  @override
  String get historyTags => '标签';

  @override
  String get historyTagsUpdateErrorTitle => '无法更新标签';

  @override
  String get historyTagsEmpty => '尚无标签';

  @override
  String get historyTagRename => '重命名';

  @override
  String get historyTagActions => '标签操作';

  @override
  String get historyTagNamePlaceholder => '标签名称';

  @override
  String get historyCreateTag => '创建标签';

  @override
  String get historySaveTag => '保存标签';

  @override
  String get historyTagsFilterAll => '所有标签';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个标签',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return '按标签筛选：$selection';
  }

  @override
  String get historyManageTags => '管理标签';

  @override
  String get historyTagsFilterSemantics => '按标签筛选已置顶的剪贴项';

  @override
  String get historyDownloadErrorCancelled => '文件下载已取消。';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      '下载服务不可用。请重新打开 CopyPaste 后重试。';

  @override
  String get historyDownloadErrorServiceTimeout => '下载服务未连接。请重试。';

  @override
  String get historyFileErrorAccess => '无法访问所选文件。';

  @override
  String get historySpoilerRevealSemantics => '机密内容。显示遮罩下的内容';

  @override
  String get historyActionFailedTitle => '操作失败';

  @override
  String get historyLoadingMessage => '正在加载剪贴板历史记录。';

  @override
  String get historyUnavailableTitle => '历史记录不可用';

  @override
  String get historyEmptyTitle => '未找到剪贴项';

  @override
  String get historyEmptyMessage => '捕获的剪贴项和匹配的历史记录将显示在这里。';

  @override
  String get historyAttentionTitle => '历史记录需要处理';

  @override
  String get historyLoadMore => '加载更多';

  @override
  String get historySectionPinned => '已置顶';

  @override
  String get historySectionToday => '今天';

  @override
  String get historySectionYesterday => '昨天';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已跳过 $count 条剪贴板记录',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage => '部分保留的剪贴项无法读取，未在此列表中显示。';

  @override
  String get historySearch => '搜索历史记录';

  @override
  String get historyResetFilters => '重置筛选条件';

  @override
  String get historyImportFiles => '导入文件';

  @override
  String get historySelectClips => '选择剪贴项';

  @override
  String get historyCloseSearch => '关闭搜索';

  @override
  String get historyFilterAllClips => '所有剪贴项';

  @override
  String get historyFilterAllDevices => '所有设备';

  @override
  String get historyFilterAllApps => '所有应用';

  @override
  String get historyPinClip => '置顶剪贴项';

  @override
  String get historyUnpinClip => '取消置顶剪贴项';

  @override
  String get historyReorderPinnedClip => '调整置顶剪贴项的顺序';

  @override
  String get historyReorderPinnedHint => '拖移以移动。按 Alt + 上箭头或下箭头可移动一个位置。';

  @override
  String get historyClipUnavailable => '剪贴项不可用';

  @override
  String historyClipKindSemantics(String kind) {
    return '剪贴项类型：$kind';
  }

  @override
  String get historyInspectorEmptyTitle => '选择一个剪贴项';

  @override
  String get historyInspectorEmptyMessage => '其完整内容和操作将显示在这里。';

  @override
  String get historyNoPreview => '无可用预览。';

  @override
  String get historyCloseClipDetails => '关闭剪贴项详情';

  @override
  String get historyCopyOptions => '复制选项';

  @override
  String get historyCopyPlainText => '复制纯文本';

  @override
  String get historyManageClipTags => '管理剪贴项标签';

  @override
  String get historyPinnedButton => '已置顶';

  @override
  String get historyAddSpoiler => '添加遮罩';

  @override
  String get historyRemoveSpoiler => '移除遮罩';

  @override
  String get historyUpdateFailedTitle => '更新失败';

  @override
  String get historyRecognizeImageText => '识别图片文字';

  @override
  String get historyCopiedTitle => '已复制';

  @override
  String get historyCopyFailedTitle => '复制失败';

  @override
  String get historyPlainTextCopied => '已复制纯文本。';

  @override
  String get historyClipCopied => '已复制剪贴项。';

  @override
  String get historyDownloadedTitle => '已下载';

  @override
  String get historyFileSaved => '文件已保存。';

  @override
  String get historyDownload => '下载';

  @override
  String get historyDownloading => '正在下载…';

  @override
  String historyDownloadingPercent(int percent) {
    return '正在下载 $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => '正在取消…';

  @override
  String get historyDownloadCancellingTooltip => '正在取消下载';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return '正在下载 $percent% · 点击以取消';
  }

  @override
  String get historyDownloadFailedRetryTooltip => '下载失败 · 点击以重试';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return '取消下载，已完成 $percent%';
  }

  @override
  String get historyRetryDownload => '重试下载';

  @override
  String get historyDownloadFailedTitle => '下载失败';

  @override
  String get historyShowFile => '显示文件';

  @override
  String get historyMetadataPrivacy => '隐私';

  @override
  String get historyMetadataConfidential => '机密';

  @override
  String get historyMetadataCaptured => '捕获时间';

  @override
  String get historyMetadataObservedApp => '来源应用';

  @override
  String get historyMetadataDevice => '设备';

  @override
  String get historyMetadataClipType => '剪贴项类型';

  @override
  String get historyMetadataType => '类型';

  @override
  String get historyMetadataPath => '路径';

  @override
  String get historyMetadataFile => '文件';

  @override
  String get historyMetadataResolution => '分辨率';

  @override
  String get historyMetadataSize => '大小';

  @override
  String get historyMetadataFiles => '文件数';

  @override
  String get historyMetadataSync => '同步';

  @override
  String get historyMetadataTooLargeToSync => '过大，无法同步';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '删除 $count 个所选剪贴项？',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => '删除此剪贴项？';

  @override
  String get historyBulkTagTooltip => '为所选的已置顶剪贴项添加标签';

  @override
  String get historyBulkExit => '退出选择';

  @override
  String get historyBulkPin => '置顶所选项';

  @override
  String get historyBulkUnpin => '取消置顶所选项';

  @override
  String get historyBulkDelete => '删除所选项';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已选择 $count 项',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => '按日期筛选';

  @override
  String get historyDatePresetToday => '今天';

  @override
  String get historyDatePresetYesterday => '昨天';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: '最近 $days 天',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => '全部时间';

  @override
  String get appRuntimeStartFailed => 'CopyPaste 无法启动其运行时。';

  @override
  String get appPreparing => '正在准备 CopyPaste。';

  @override
  String get appRuntimeWaiting => 'CopyPaste 正在等待受保护历史记录的访问权限。应用的其他可用部分仍可正常使用。';

  @override
  String get appHistoryUnavailableTitle => '历史记录运行时不可用';

  @override
  String get appHistoryUnavailableMessage => '请先启动应用运行时，再打开历史记录。';

  @override
  String get appDevicesUnavailableTitle => '设备运行时不可用';

  @override
  String get appDevicesUnavailableMessage => '请先启动应用运行时，再打开设备。';

  @override
  String get appSettingsUnavailableTitle => '设置运行时不可用';

  @override
  String get appSettingsUnavailableMessage => '请先启动应用运行时，再打开设置。';

  @override
  String get appQuit => '退出 CopyPaste';

  @override
  String get appWindowSetupTitle => '窗口设置需要处理';

  @override
  String get appOpenHistory => '打开历史记录';

  @override
  String get appDestinationHistory => '历史记录';

  @override
  String get appDestinationDevices => '设备';

  @override
  String get appDestinationSettings => '设置';

  @override
  String appDestinationSelected(String destination) {
    return '$destination，已选择';
  }

  @override
  String get appActiveFilter => '已启用筛选';

  @override
  String get appActionFailed => '操作失败';

  @override
  String get sharedAndroidVersionRequired => '需要 Android 11 或更高版本';

  @override
  String get sharedUseAdbTab => '请在此设备上使用 ADB 标签页。';

  @override
  String get sharedOneTimeAccessApplied => '已应用一次性访问权限';

  @override
  String get sharedInstallShizuku => '安装 Shizuku';

  @override
  String get sharedPairAndStartShizuku => '配对并启动 Shizuku';

  @override
  String get sharedAllowCopyPaste => '允许 CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded => '不再需要 Shizuku。';

  @override
  String get sharedGetShizukuDescription => '获取 Shizuku 以应用一次性访问权限。';

  @override
  String get sharedShizukuWirelessDebugging => '在 Shizuku 中使用“无线调试”，然后返回。';

  @override
  String get sharedApproveInShizuku => '在 Shizuku 中允许 CopyPaste 一次。';

  @override
  String get sharedGetShizuku => '获取 Shizuku';

  @override
  String get sharedOpenShizuku => '打开 Shizuku';

  @override
  String get sharedRunOnComputer => '在电脑上运行';

  @override
  String get sharedCopyAllCommands => '复制全部命令';

  @override
  String get sharedEnableUsbDebugging => '启用“USB 调试”并连接此手机。';

  @override
  String get sharedAllow => '允许';

  @override
  String get sharedSetupCommandsCopyFailed => '无法复制设置命令。';

  @override
  String get sharedShizukuOpenFailed => '无法打开 Shizuku。';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => '设置 CopyPaste';

  @override
  String get onboardingWelcomeTitle => '欢迎使用 CopyPaste';

  @override
  String get onboardingWelcomeSubtitle => '你的剪贴板，随时待命。';

  @override
  String get onboardingReadyTitle => 'CopyPaste 已准备就绪';

  @override
  String get onboardingGetStarted => '开始使用';

  @override
  String get onboardingOptionalSetupTitle => '可选设置需要处理';

  @override
  String get onboardingOpenSystemSettings => '打开系统设置';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return '第 $step 步，共 $count 步 · $platform';
  }

  @override
  String get onboardingSetupErrorTitle => '设置需要处理';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste 无法读取 $platform 设置状态。';
  }

  @override
  String get onboardingSaveProgressFailed => 'CopyPaste 无法保存设置进度。';

  @override
  String get onboardingStartAtLogin => '登录时启动';

  @override
  String get onboardingStartAtLoginDescription => '登录时打开 CopyPaste';

  @override
  String get onboardingStartAtLoginSemantics => '登录时启动 CopyPaste';

  @override
  String get onboardingStartAtLoginEnableFailed => '无法启用“登录时启动”。你可以跳过此项继续。';

  @override
  String get onboardingStartAtLoginUpdateFailed => '无法更新“登录时启动”。你可以跳过此项继续。';

  @override
  String get onboardingMacosTitle => '设置你的 Mac';

  @override
  String get onboardingAccessibility => '辅助功能';

  @override
  String get onboardingAutoPasteEnabled => '已启用自动粘贴';

  @override
  String get onboardingAutoPasteOptional => '可选，用于自动粘贴';

  @override
  String get onboardingMacosStartAtLoginUnavailable => '在已安装的 CopyPaste 应用中可用。';

  @override
  String get onboardingLoginItemApprovalTitle => '需要批准登录项';

  @override
  String get onboardingLoginItemApprovalMessage =>
      '请在“系统设置”中允许 CopyPaste，然后返回。';

  @override
  String get onboardingMacosStateReadFailed => 'CopyPaste 无法读取 macOS 设置状态。';

  @override
  String get onboardingMacosAccessibilityRequestFailed => '无法请求辅助功能权限。';

  @override
  String get onboardingMacosStateRefreshFailed => 'CopyPaste 无法刷新 macOS 设置状态。';

  @override
  String get onboardingMacosAccessibilityRefreshFailed => '无法刷新辅助功能状态。';

  @override
  String get onboardingMacosLoginItemsOpenFailed => '无法打开“登录项”设置。';

  @override
  String get onboardingMacosStartAtLoginApproval => '“登录时启动”需要在“系统设置”中批准。';

  @override
  String get onboardingMacosStartAtLoginDisableFailed => '无法停用“登录时启动”。';

  @override
  String get onboardingWindowsTitle => '设置你的电脑';

  @override
  String get onboardingWindowsStartAtLoginUnavailable => '此安装不支持此功能。你可以跳过此项继续。';

  @override
  String get onboardingTaskManagerDisabledTitle => '已在任务管理器中禁用';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      '请在 Windows 启动应用中启用 CopyPaste，然后返回。';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      '“登录时启动”已在任务管理器中禁用。你稍后可以在 Windows 设置中启用。';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      '“登录时启动”不可用。你可以跳过此项继续。';

  @override
  String get onboardingWindowsStateReadFailed => 'CopyPaste 无法读取 Windows 设置状态。';

  @override
  String get onboardingWindowsStartupSettingsFailed => '无法打开 Windows 启动设置。';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      '无法更新“登录时启动”。你稍后可以在“设置”中更改。';

  @override
  String get onboardingLinuxChecking => '正在检查你的 Linux 桌面会话。';

  @override
  String get onboardingLinuxTitle => '设置你的 Linux 桌面';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste 需要经过验证的桌面集成，才能识别剪贴板来源并使用 Quick Paste。';

  @override
  String get onboardingLinuxSession => '桌面会话';

  @override
  String get onboardingLinuxX11Detected => '检测到 X11 会话。';

  @override
  String get onboardingLinuxWaylandDetected => '检测到 Wayland 会话。';

  @override
  String get onboardingLinuxSessionUnsupported =>
      '未检测到受支持的 X11 或 Wayland 桌面会话。';

  @override
  String get onboardingLinuxCompanion => 'GNOME 或 KDE 配套组件';

  @override
  String get onboardingLinuxCompanionReady => '经过认证的 Clipboard v2 桥接已就绪。';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      '由于来源应用排除规则，剪贴板捕获已暂停。请启用已签名的配套组件以恢复捕获。';

  @override
  String get onboardingLinuxCompanionInstall => '请安装并启用已签名的配套组件会话软件包，然后返回此处。';

  @override
  String get onboardingLinuxOpenSetup => '打开设置向导';

  @override
  String get onboardingLinuxKeyboardControl => '键盘控制';

  @override
  String get onboardingLinuxKeyboardAllowed => '已允许 Quick Paste 使用键盘控制。';

  @override
  String get onboardingLinuxKeyboardUnsupported => '此桌面会话不支持键盘控制。';

  @override
  String get onboardingLinuxAllowKeyboard => '允许键盘控制';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste 已就绪。';

  @override
  String get onboardingLinuxQuickPastePending => '所需的桌面集成启用后，Quick Paste 即可就绪。';

  @override
  String get onboardingLinuxStartAtLoginDescription => '登录时打开 CopyPaste。';

  @override
  String get onboardingLinuxLinks => 'CopyPaste 链接';

  @override
  String get onboardingLinuxLinksRegistered => 'CopyPaste 链接将在此应用中打开。';

  @override
  String get onboardingLinuxLinksUnregistered => '注册 CopyPaste 链接以用于配对邀请。';

  @override
  String get onboardingLinuxRegisterLinks => '注册链接';

  @override
  String get onboardingLinuxScreenshotBlocking => '阻止截屏';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Linux 不支持阻止截屏。受保护的存储数据仍保持加密。';

  @override
  String get onboardingLinuxCheckAgain => '再次检查';

  @override
  String get onboardingLinuxStatusReadFailed => 'CopyPaste 无法读取 Linux 集成状态。';

  @override
  String get onboardingLinuxSetupOpenFailed => '无法打开桌面集成设置。';

  @override
  String get onboardingLinuxKeyboardDenied => '未授予键盘控制权限。';

  @override
  String get onboardingLinuxStartAtLoginFailed => '无法更新“登录时启动”。';

  @override
  String get onboardingLinuxLinksFailed => '无法注册 CopyPaste 链接。';

  @override
  String get onboardingLinuxStatusRefreshFailed => '无法刷新 Linux 集成状态。';

  @override
  String get onboardingLinuxX11ClipboardUnavailable => '此 X11 会话无法访问剪贴板。';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste 需要 X11 或 Wayland 桌面会话。';

  @override
  String get onboardingAndroidFullCapture => '完整捕获';

  @override
  String get onboardingAndroidBackgroundCapture => '后台捕获';

  @override
  String get onboardingAndroidFullCaptureDescription => '只需设置一次，然后用真实复制进行验证。';

  @override
  String get onboardingAndroidLimitedCapture => '受限捕获';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      '分享到 CopyPaste，或复制后返回本应用。';

  @override
  String get onboardingAndroidNotification => '捕获通知';

  @override
  String get onboardingAndroidNotificationDescription => '捕获运行期间必需';

  @override
  String get onboardingAndroidBackgroundActivity => '后台活动';

  @override
  String get onboardingAndroidBackgroundActivityDescription => '建议开启，以减少捕获遗漏';

  @override
  String get onboardingAndroidOpen => '打开';

  @override
  String get onboardingAndroidApplyAccess => '应用捕获访问权限';

  @override
  String get onboardingAndroidApplyAccessDescription => '应用一次性捕获访问权限。';

  @override
  String get onboardingAndroidFullCaptureLater => '你稍后可以设置“完整捕获”。';

  @override
  String get onboardingAndroidCaptureWorking => '后台捕获正常运行';

  @override
  String get onboardingAndroidVerifyCopy => '验证真实复制';

  @override
  String get onboardingAndroidCopyReached => '新复制的内容已进入本地历史记录。';

  @override
  String get onboardingAndroidCopyInstruction => '在其他应用中复制内容，然后返回此处。';

  @override
  String get onboardingAndroidStartCapture => '开始捕获';

  @override
  String get onboardingAndroidStateReadFailed => 'CopyPaste 无法读取 Android 设置状态。';

  @override
  String get onboardingAndroidStateRefreshFailed => '无法刷新 Android 捕获状态。';

  @override
  String get onboardingAndroidMonitoringUnavailable => '无法监控 Android 设置状态。';

  @override
  String get onboardingAndroidIntakeFailed => '无法启用剪贴板接收。';

  @override
  String get onboardingAndroidStopCaptureFailed => '无法停止后台捕获。';

  @override
  String get onboardingAndroidBatterySettingsFailed => '无法打开电池设置。';

  @override
  String get onboardingAndroidGrantsFailed =>
      '已在 Shizuku 中允许 CopyPaste，但无法应用 Android 捕获授权。';

  @override
  String get onboardingAndroidAllowInShizuku =>
      '请在 Shizuku 中允许 CopyPaste 以应用捕获授权。';

  @override
  String get onboardingAndroidGrantAccessFirst => '请先授予后台剪贴板访问权限。';

  @override
  String get onboardingAndroidAllowNotificationsFirst => '开始后台捕获前，请先允许通知。';

  @override
  String get onboardingAndroidStartCaptureFailed => '无法启动后台捕获。';

  @override
  String get onboardingAndroidSetupOpenFailed => '无法打开 Android 捕获设置。';

  @override
  String get onboardingAndroidChoiceSaveFailed => '无法保存 Android 设置选项。';

  @override
  String get modulesMarketplaceTab => '市场';

  @override
  String get modulesInstalledTab => '已安装';

  @override
  String get modulesRefresh => '刷新';

  @override
  String get modulesSearchPlaceholder => '搜索模块';

  @override
  String get modulesOperationFailed => '模块操作失败';

  @override
  String get modulesLoading => '正在加载模块。';

  @override
  String get modulesUnavailableTitle => '模块不可用';

  @override
  String get modulesUnavailableMessage => '请尝试重新加载模块。';

  @override
  String get modulesNoneInstalled => '未安装任何模块';

  @override
  String get modulesNoMatches => '没有匹配的模块';

  @override
  String get modulesChooseFromMarketplace => '从市场中选择一个模块。';

  @override
  String get modulesTryDifferentSearch => '请尝试其他搜索词。';

  @override
  String get modulesMarketplaceLoading => '正在加载市场。';

  @override
  String get modulesMarketplaceUnavailableTitle => '市场不可用';

  @override
  String get modulesMarketplaceUnavailableMessage => '请尝试重新加载市场。';

  @override
  String get modulesNoneAvailable => '没有可用模块';

  @override
  String get modulesNoneAvailableMessage => '模块发布后将显示在这里。';

  @override
  String modulesEnableModule(String module) {
    return '启用 $module';
  }

  @override
  String get modulesInstall => '安装';

  @override
  String modulesUpdateTo(String version) {
    return '更新到 $version';
  }

  @override
  String get modulesRemoveModule => '移除模块';

  @override
  String get modulesModuleSettings => '模块设置';

  @override
  String modulesSettingsTitle(String module) {
    return '$module 设置';
  }

  @override
  String get modulesDownloadingModel => '正在下载语言模型…';

  @override
  String get modulesRun => '运行';

  @override
  String modulesRemoveTitle(String module) {
    return '移除 $module？';
  }

  @override
  String get modulesRemoveMessage => '该模块及其本地设置将被移除。';

  @override
  String get modulesRemove => '移除';

  @override
  String get modulesRemoveFailed => '无法移除该模块。';

  @override
  String get modulesCloseSettings => '关闭模块设置';

  @override
  String get modulesRemoved => '模块已移除';

  @override
  String get modulesNoSettingsTitle => '无其他设置';

  @override
  String get modulesNoSettingsMessage => '此模块无需额外配置。';

  @override
  String get modulesSetUpSmsAccess => '设置短信访问权限';

  @override
  String get modulesRestartApp => '重新启动 CopyPaste';

  @override
  String get modulesSmsAccessTitle => '短信访问权限';

  @override
  String get modulesCloseSmsAccess => '关闭短信访问权限面板';

  @override
  String get modulesSmsChecking => '正在检查短信访问权限。';

  @override
  String get modulesSmsReady => '访问权限已就绪。启用 SMS Codes 即可开始复制新验证码。';

  @override
  String get modulesSmsAllowAccess => '允许访问，以便从新短信中复制验证码。';

  @override
  String get modulesSmsNotification => '短信通知';

  @override
  String get modulesSmsNotificationDescription => 'SMS Codes 运行期间必需';

  @override
  String get modulesSmsApplyAccess => '应用短信访问权限';

  @override
  String get modulesSmsApplyAccessDescription => '应用一次性短信访问权限。';

  @override
  String get modulesSmsSetupFailed => '短信设置失败';

  @override
  String get modulesFileSelectionFailed => '文件选择失败';

  @override
  String get modulesChooseLanguages => '选择语言';

  @override
  String get modulesChooseFile => '选择文件';

  @override
  String modulesModelReady(String model) {
    return '$model 模型已就绪';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return '$model 模型 · 需下载 $size MiB';
  }

  @override
  String get modulesPlatformsUnknown => '平台：未知';

  @override
  String modulesPlatforms(String platforms) {
    return '平台：$platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid => '模块目录签名无效。';

  @override
  String get modulesMarketplaceConnectFailed => '无法连接模块市场。';

  @override
  String get modulesMarketplaceTimeout => '模块市场无响应。';

  @override
  String get modulesIncompatible => '此模块与此设备不兼容。';

  @override
  String get modulesPackageSizeInvalid => '模块包大小无效。';

  @override
  String get modulesDownloadInterrupted => '模块下载已中断。';

  @override
  String get modulesDownloadTimeout => '模块下载无响应。';

  @override
  String get modulesPackageUrlUntrusted => '模块包 URL 不受信任。';

  @override
  String get modulesDownloadLeftGithub => '模块下载被重定向到了 GitHub 之外。';

  @override
  String get modulesDownloadTooManyRedirects => '模块下载的重定向次数过多。';

  @override
  String get modulesDownloadTooLarge => '模块下载过大。';

  @override
  String get modulesPackageIntegrityFailed => '模块包未通过完整性检查。';

  @override
  String get modulesDownloadUnavailable => '此模块当前无法下载。';

  @override
  String get modulesMarketplaceUnavailableRetry => '模块市场不可用。请重试。';

  @override
  String get modulesCatalogInvalid => '模块目录无效。';

  @override
  String get modulesThisSystem => '此系统';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version 或更高版本';
  }

  @override
  String get modulesNotAvailableForDevice => '不适用于此设备。';

  @override
  String get modulesAppVersionUnverified => '无法验证已安装的 CopyPaste 版本。';

  @override
  String modulesAppVersionUnsupported(String version) {
    return '此模块不支持 CopyPaste $version。';
  }

  @override
  String modulesRequires(String requirement) {
    return '需要 $requirement。';
  }

  @override
  String get modulesMarketplaceLoadFailed => '无法加载模块市场。';

  @override
  String get modulesSmsSetupRequired => '启用此模块前，请先设置短信访问权限。';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      '无法启动短信监控。该模块仍处于启用状态；请再次停用以重试。';

  @override
  String get modulesSmsMonitoringDisabled => '无法启动短信监控。该模块已被停用。';

  @override
  String get modulesSmsMonitoringUnconfirmed => '无法启动短信监控。无法确认该模块的启用状态。';

  @override
  String get modulesSmsMonitoringFailed => '无法启动短信监控。';

  @override
  String get modulesSmsAndroidOnly => '短信访问权限仅在 Android 上可用。';

  @override
  String get modulesSmsMonitoringNeedsSetup => '短信监控需要设置。';

  @override
  String get modulesOperationIncomplete => '无法完成模块操作。';

  @override
  String get modulesUnsupportedEvent => '该模块使用了不受支持的事件。';

  @override
  String get modulesUnsupportedResult => '该模块返回了不受支持的结果。';

  @override
  String get modulesUnsupportedField => '该模块使用了不受支持的字段。';

  @override
  String get modulesResponseInvalid => '模块响应无效。';

  @override
  String get modulesFileSelectionUnavailable => '文件选择不可用。';

  @override
  String get modulesFileSelectFailed => '无法选择该文件。';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field 不是可接受的文件。';
  }

  @override
  String get modulesSmsVerifyFailed => '无法验证短信访问权限。';

  @override
  String get modulesSmsAllowNotifications => '请允许通知，以使 SMS Codes 在后台保持运行。';

  @override
  String get modulesSmsNotGranted => '未授予短信访问权限。';

  @override
  String get modulesSmsConfigureFailed => '无法配置短信访问权限。';

  @override
  String get onboardingLinuxContinueBasic => '以基本捕获继续';

  @override
  String get onboardingLinuxCompanionOptional => '可选。Quick Paste 和来源应用排除需要此组件。';

  @override
  String get onboardingLinuxKeyboardOptional =>
      '可选。Quick Paste 需要此权限才能返回上一个应用。';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      '此 Wayland 会话无法访问剪贴板。请启用已签名的 GNOME 或 KDE 配套组件，然后再次检查。';

  @override
  String get historyMoreActions => '更多操作';

  @override
  String get historyTagsAction => '标签…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '解锁机密项目',
      'other': '解锁机密项目',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '使用隐藏的片段',
      'other': '使用隐藏的片段',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '复制机密项目',
      'other': '复制机密项目',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => '此系统不支持';

  @override
  String get deviceAuthNotConfigured => '请先设置设备密码、PIN 或生物识别';

  @override
  String get commonOn => '已开启';

  @override
  String get commonOff => '已关闭';

  @override
  String get settingsQuickPasteStartFailed => '无法启动 Quick Paste，因此其快捷键不起作用。';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut 已被其他应用占用。请选择其他快捷键。';
  }

  @override
  String get devicesEmptyTitle => '尚未配对其他设备';

  @override
  String get devicesEmptyMessage =>
      '配对另一台设备，即可在设备之间同步剪贴板历史记录。两台设备必须处于同一网络，且每次配对都需要在两台设备上确认。';

  @override
  String get pairingStateNotCompleted => '配对未完成';

  @override
  String get pairingStepInvite => '邀请';

  @override
  String get pairingStepScan => '扫描';

  @override
  String get pairingStepCode => '代码';

  @override
  String get pairingStepRequest => '请求';

  @override
  String get pairingStepConnect => '连接';

  @override
  String get pairingStepConfirm => '确认';

  @override
  String get pairingStepPaired => '已配对';

  @override
  String get pairingOtherDevice => '另一台设备';

  @override
  String get pairingDecisionNotConfirmed => '尚未确认';

  @override
  String get pairingDecisionSending => '正在发送您的决定…';

  @override
  String get pairingDecisionConfirmed => '已确认';

  @override
  String get pairingDecisionRejected => '已拒绝';

  @override
  String get pairingDecisionWaiting => '正在等待确认';

  @override
  String get pairingDecisionUnknown => '答复未知';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 台设备',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 行',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => '主导航';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return '第 $step 步，共 $count 步：$name';
  }

  @override
  String get modulesPhaseDownload => '下载';

  @override
  String get modulesPhaseInstall => '安装';

  @override
  String modulesInstallingModule(String module) {
    return '正在安装 $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return '正在更新 $module';
  }

  @override
  String get commonDismiss => '关闭';

  @override
  String get settingsRetentionConfirmTitle => '移除较早的历史记录？';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return '新的期限：$limit。更早的未置顶剪贴项将立即从此设备移除，且无法恢复。已置顶的剪贴项会保留。';
  }

  @override
  String get settingsStorageQuotaConfirmTitle => '减小存储配额？';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return '新的配额：$size。如果未置顶的历史记录占用更多空间，最早的剪贴项将立即被移除，且无法恢复。已置顶的剪贴项会保留。';
  }

  @override
  String get settingsLimitConfirmAction => '应用限制';

  @override
  String get settingsHideEverywhereTitle => '完全隐藏 CopyPaste？';

  @override
  String get settingsHideEverywhereMessage =>
      '两项都关闭时，CopyPaste 没有任何可见图标。要再次打开，请重新启动 CopyPaste。';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return '两项都关闭时，CopyPaste 没有任何可见图标。要再次打开，请按 $shortcut 或重新启动 CopyPaste。';
  }

  @override
  String get settingsHideEverywhereAction => '隐藏';

  @override
  String get settingsExportDoneTitle => '文本历史记录已导出';

  @override
  String get settingsBackupDoneTitle => '备份已创建';

  @override
  String get settingsRestoreDoneTitle => '历史记录已恢复';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已导出 $count 个文本剪贴项',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个无法读取',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return '加密备份已保存 · $size';
  }

  @override
  String get settingsRestoreDoneMessage => '此设备上的剪贴板历史记录现已与备份一致。';

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
    return '$used / $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return '已用 $used，共 $quota';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '已用 $used';
  }

  @override
  String get storageUsageOverQuota => '已超出配额';

  @override
  String storageUsageOldestClip(String date) {
    return '最早的剪贴项：$date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return '存储：$usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return '新的配额：$size。未置顶的历史记录占用 $used，因此最早的剪贴项将立即被移除，直到符合配额。移除后无法恢复。已置顶的剪贴项会保留。';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个剪贴项',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => '打开 CopyPaste';

  @override
  String get trayOpenQuickPaste => '打开 Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return '打开 Quick Paste ($shortcut)';
  }

  @override
  String get trayNewSnippet => '新建片段';

  @override
  String get trayRecent => '最近';

  @override
  String get trayRecentLoading => '正在加载…';

  @override
  String get trayRecentFailed => '无法加载剪贴';

  @override
  String get trayRecentEmpty => '还没有剪贴';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已隐藏 $count 条最近剪贴',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => '机密';

  @override
  String get trayCopyLatestFromOtherDevice => '复制其他设备上的最新剪贴';

  @override
  String get trayCaptureNow => '立即捕获';

  @override
  String trayPausedUntil(String time) {
    return '已暂停至 $time';
  }

  @override
  String get trayPauseFor => '暂停…';

  @override
  String get trayPauseFor15Minutes => '15 分钟';

  @override
  String get trayPauseForOneHour => '1 小时';

  @override
  String get trayPauseUntilResumed => '直到我恢复';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue：$count 项',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '正在收集：$count 项',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return '最近跳过 ($count)';
  }

  @override
  String get trayOpenContentRules => '打开内容规则';

  @override
  String get trayRecentSettingsTitle => '菜单栏和托盘';

  @override
  String get trayRecentSettingsDescription => '菜单栏或托盘菜单在“最近”下显示的内容。';

  @override
  String get trayRecentCountTitle => '最近剪贴';

  @override
  String get trayRecentCountDescription => '菜单显示的最新剪贴数量。';

  @override
  String get trayRecentPreviewTitle => '最近剪贴预览';

  @override
  String get trayRecentPreviewDescription =>
      '系统会在 CopyPaste 加密之外保存菜单文本。减少显示可避免剪贴内容出现在其中。机密内容始终会被遮盖。';

  @override
  String get trayRecentPreviewFull => '完整预览';

  @override
  String get trayRecentPreviewTitles => '仅类型和来源';

  @override
  String get trayRecentPreviewHidden => '隐藏';

  @override
  String get trayRecentSettingsKeywords => '托盘 菜单栏 最近 预览 隐私 数量';

  @override
  String get transformsPaletteTitle => '转换文本';

  @override
  String get transformsFilterPlaceholder => '筛选转换';

  @override
  String get transformsNoMatches => '没有匹配的转换';

  @override
  String get transformsGroupResults => '结果';

  @override
  String get transformsGroupSuggested => '建议';

  @override
  String get transformsGroupRecent => '最近';

  @override
  String get transformsGroupFavorites => '收藏';

  @override
  String get transformsGroupPipelines => '流水线';

  @override
  String get transformsCategoryStructure => 'JSON、YAML、CSV 和 XML';

  @override
  String get transformsCategoryEncode => '编码与解码';

  @override
  String get transformsCategoryCase => '大小写';

  @override
  String get transformsCategoryHash => '哈希';

  @override
  String get transformsCategoryLines => '行与空白';

  @override
  String get transformsCategoryClean => '清理';

  @override
  String get transformsCategoryUrl => '链接';

  @override
  String get transformsCategoryMarkup => '富文本与 Markdown';

  @override
  String get transformsCategoryDev => '开发者';

  @override
  String get transformsCategoryOther => '其他';

  @override
  String get transformsChainEmpty => '尚无步骤';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return '然后：$name';
  }

  @override
  String get transformsStepEnabled => '运行此步骤';

  @override
  String get transformsMoveEarlier => '上移';

  @override
  String get transformsMoveLater => '下移';

  @override
  String get transformsRemoveStep => '移除步骤';

  @override
  String get transformsSavePipeline => '另存为流水线';

  @override
  String get transformsSavePipelineTitle => '保存流水线';

  @override
  String get transformsSavePipelineFailed => '无法保存流水线。';

  @override
  String get transformsPipelineNameLabel => '名称';

  @override
  String get transformsPipelineDescriptionLabel => '描述（可选）';

  @override
  String get transformsPipelineStepsLabel => '步骤';

  @override
  String get transformsPipelineNoSteps => '请至少添加一个步骤。';

  @override
  String get transformsPipelineNameRequired => '请输入名称。';

  @override
  String get transformsPipelineNameTooLong => '最多 80 个字符。';

  @override
  String get transformsPipelineNameTaken => '已有同名流水线。';

  @override
  String get transformsPipelineSavedTitle => '流水线已保存';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name 已可在所有转换面板中使用。';
  }

  @override
  String transformsPipelineSteps(int count) {
    return '步骤：$count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => '已删除的流水线';

  @override
  String get transformsNeedsNewerVersion => '需要更新版本的 CopyPaste';

  @override
  String get transformsLoadFailed => '无法加载转换。';

  @override
  String get transformsNotTextTitle => '转换仅适用于文本';

  @override
  String get transformsNotTextMessage => '此项目是图片、文件或颜色。请选择文本项目或复制一些文本。';

  @override
  String get transformsNothingTitle => '没有可转换的内容';

  @override
  String get transformsNothingMessage => '复制一些文本，或选择一个历史项目。';

  @override
  String get transformsPickTransform => '选择一个转换';

  @override
  String get transformsPickTransformHint => 'Tab 将其添加为步骤，Enter 应用结果。';

  @override
  String get transformsPreviewing => '正在更新预览…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return '行：$lines · 词：$words · 字符：$chars';
  }

  @override
  String get transformsNoChanges => '无更改';

  @override
  String transformsTruncated(String size) {
    return '显示 $size 中的前 64 KiB';
  }

  @override
  String get transformsRandom => '随机';

  @override
  String get transformsNotRun => '未运行';

  @override
  String get transformsSensitiveHidden => '结果为敏感内容，已隐藏。';

  @override
  String get transformsReveal => '显示';

  @override
  String get transformsPaneOutput => '输出';

  @override
  String get transformsPaneInput => '输入';

  @override
  String get transformsPaneChanges => '更改';

  @override
  String get transformsCannotTransform => '无法转换此文本';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return '步骤 $step · $name：$message';
  }

  @override
  String get transformsUseHexOutput => '以十六进制显示';

  @override
  String get transformsActionPaste => '粘贴';

  @override
  String get transformsActionSave => '另存为新项目';

  @override
  String get transformsCopiedMessage => '结果已在剪贴板和历史记录中。';

  @override
  String get transformsSavedTitle => '已保存';

  @override
  String get transformsSavedMessage => '结果已成为新的历史项目，剪贴板未改变。';

  @override
  String get transformsPasteFailedTitle => '无法粘贴';

  @override
  String get transformsPasteFailedMessage => '结果已在剪贴板中，请手动粘贴。';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message（第 $line 行，第 $column 列）';
  }

  @override
  String get transformsErrorBinary => '解码后的数据是二进制而非文本';

  @override
  String get transformsErrorTooLarge => '内容过大，无法转换';

  @override
  String get transformsErrorInvalidParam => '此步骤的设置无效';

  @override
  String get transformsErrorCancelled => '已取消';

  @override
  String get transformsErrorTooExpensive => '模式开销过大';

  @override
  String get transformsErrorPattern => '模式无效';

  @override
  String get transformsErrorTimestampUnit => '请选择该数字的单位是秒还是毫秒';

  @override
  String get transformsErrorExternalEntity => '不允许外部实体';

  @override
  String get transformsErrorJwtEncrypted => '无法解码加密令牌';

  @override
  String get transformsErrorJson => '无效的 JSON';

  @override
  String get transformsErrorYaml => '无效的 YAML';

  @override
  String get transformsErrorXml => '无效的 XML';

  @override
  String get transformsErrorCsv => '无效的 CSV';

  @override
  String get transformsErrorBase64 => '无效的 Base64';

  @override
  String get transformsErrorHex => '无效的十六进制';

  @override
  String get transformsErrorEscape => '无效的转义序列';

  @override
  String get transformsErrorUrl => '百分号编码格式错误';

  @override
  String get transformsErrorJwt => '不是有效的 JWT';

  @override
  String get transformsErrorDate => '无法识别的日期或时间戳';

  @override
  String get transformsErrorGeneric => '此步骤无法转换其输入';

  @override
  String get transformsWarningSignedLink => '已签名链接保持不变';

  @override
  String get transformsWarningExpired => '已过期';

  @override
  String get transformsWarningNotYetValid => '尚未生效';

  @override
  String get transformsWarningSignatureNotVerified => '签名未验证';

  @override
  String get transformsWarningDuplicateKeys => '重复的键';

  @override
  String get transformsWarningLenientJson => '已接受非标准 JSON';

  @override
  String get transformsWarningDoubleEncoded => 'JSON 被编码了两次';

  @override
  String get transformsWarningEscapes => '未知转义序列已按原样保留';

  @override
  String get transformsWarningLocalZone => '已假定为本地时区';

  @override
  String get transformsWarningNoMatches => '无匹配项';

  @override
  String get transformsWarningGeneric => '请检查结果';

  @override
  String get transformsRestoreTitle => '粘贴后恢复剪贴板';

  @override
  String get transformsRestoreDescription => '转换粘贴后，如果期间未复制其他内容，将恢复之前的剪贴板。';

  @override
  String get transformsDefaultTitle => '默认粘贴转换';

  @override
  String get transformsDefaultDescription =>
      'Quick Paste 会对文本应用此流水线，并在粘贴前显示。若失败则粘贴原文。';

  @override
  String get transformsNoDefault => '无';

  @override
  String get transformsAppDefaultsTitle => '按应用设置默认值';

  @override
  String get transformsAppDefaultsDescription =>
      '通过包 ID、可执行文件或软件包名称为单个应用覆盖默认值。在其他平台创建的条目会同步，但在此处无效。';

  @override
  String get transformsAppIdentifierPlaceholder => '应用标识符';

  @override
  String transformsOtherPlatform(String platform) {
    return '仅限 $platform';
  }

  @override
  String get transformsAdd => '添加';

  @override
  String get transformsRemove => '移除';

  @override
  String get transformsFavoritesTitle => '收藏';

  @override
  String get transformsFavoritesDescription =>
      '收藏会在每个面板中优先显示。前九个可用 Mod+1 至 Mod+9 运行。';

  @override
  String transformsFavoriteShortcut(int number) {
    return '面板中按 Mod+$number';
  }

  @override
  String get transformsAddFavorite => '添加收藏';

  @override
  String get transformsPipelinesTitle => '流水线';

  @override
  String get transformsPipelinesDescription => '已保存的转换链及其设置，会同步到您的其他设备。';

  @override
  String get transformsNewPipeline => '新建流水线';

  @override
  String get transformsEditPipeline => '编辑流水线';

  @override
  String get transformsDuplicatePipeline => '复制';

  @override
  String get transformsDeletePipeline => '删除';

  @override
  String transformsDeletePipelineTitle(String name) {
    return '删除 $name？';
  }

  @override
  String get transformsDeletePipelineMessage => '使用它的收藏和粘贴默认值将在所有设备上恢复为无。';

  @override
  String get transformsDeletePipelineFailed => '无法删除流水线。';

  @override
  String get transformsAddStep => '添加步骤';

  @override
  String get transformsHiddenTitle => '隐藏的转换';

  @override
  String get transformsHiddenDescription => '隐藏的转换不会出现在面板和建议中，使用它们的流水线仍可运行。';

  @override
  String get transformsHideTransform => '隐藏转换';

  @override
  String get transformsShow => '显示';

  @override
  String get transformsSuggestionsTitle => '建议';

  @override
  String get transformsSuggestionsDescription =>
      '打开面板时，会检查其文本的前 64 KiB 是否属于以下类型，并建议最多三个转换。不会在后台运行任何内容。';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web 令牌';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl => '带跟踪参数的链接';

  @override
  String get transformsDetectorTimestamp => '时间戳';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => '跟踪参数';

  @override
  String get transformsTrackingDescription =>
      '内置列表随应用提供，仅随更新而变化。添加要移除或保留的名称；写成 名称@域名 可限定到某个域名。';

  @override
  String get transformsTrackingRemoveLabel => '额外移除';

  @override
  String get transformsTrackingAllowLabel => '始终保留';

  @override
  String get transformsTrackingPlaceholder => '名称或 名称@域名，然后按 Enter';

  @override
  String get quickAccessUnavailableTitle => '快速访问状态不可用';

  @override
  String get quickAccessUnavailableMessage => 'CopyPaste 无法读取键盘、图块和小组件的状态。请重试。';

  @override
  String get quickAccessActionFailedTitle => '无法完成操作';

  @override
  String get quickAccessActionFailedMessage => 'Android 未打开所请求的界面或未保存更改。请重试。';

  @override
  String get quickAccessKeyboardTitle => '剪贴项键盘';

  @override
  String get quickAccessKeyboardDescription =>
      '在系统键盘设置中开启“CopyPaste 剪贴项”。Android 会提示键盘可以看到你输入的内容：剪贴项面板从不记录按键，没有用于在其他应用中输入的按键，只插入你点按的项目。';

  @override
  String get quickAccessKeyboardOn => '已开启';

  @override
  String get quickAccessKeyboardOff => '已关闭';

  @override
  String get quickAccessOpenKeyboardSettings => '打开键盘设置';

  @override
  String get quickAccessKeyboardSearchDescription => '开启“CopyPaste 剪贴项”键盘面板。';

  @override
  String get quickAccessKeyboardKeywords => '键盘 输入法 ime 面板 开启';

  @override
  String get quickAccessSwitchKeyboardTitle => '切换到剪贴项键盘';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      '将“CopyPaste 剪贴项”选为当前键盘。保持选中期间，每次复制都会被保存。';

  @override
  String get quickAccessSwitchKeyboardAction => '选择键盘';

  @override
  String get quickAccessKeyboardCurrent => '当前键盘';

  @override
  String get quickAccessCaptureTierTitle => '捕获模式';

  @override
  String get quickAccessCaptureTierDescription =>
      '没有通过 Shizuku 或 ADB 授予的权限时，只有在剪贴项面板打开时、保持选中期间，或通过 CopyPaste 应用、图块、小组件或分享操作时才会保存复制内容。';

  @override
  String get quickAccessCaptureTierSearchDescription => '查看如何保存来自其他应用的复制内容。';

  @override
  String get quickAccessCaptureTierKeywords => '捕获 级别 完整 面板 手动 shizuku 收集';

  @override
  String get quickAccessTierFull => '完整：在后台保存每次复制';

  @override
  String get quickAccessTierPanelCollecting => '已选中面板：正在收集';

  @override
  String get quickAccessTierPanelCatchUp => '面板模式：打开时补存';

  @override
  String get quickAccessTierManual => '手动：通过应用、图块、小组件或分享保存';

  @override
  String get quickAccessTierPaused => '捕获已暂停';

  @override
  String get quickAccessAutoReturnTitle => '插入后返回上一个键盘';

  @override
  String get quickAccessAutoReturnDescription =>
      '单次插入后切换回去。使用“队列”标签页或项目操作时面板保持打开。关闭此项可继续收集复制内容。';

  @override
  String get quickAccessAutoReturnKeywords => '返回 上一个键盘 自动 插入';

  @override
  String get quickAccessFocusSearchTitle => '打开面板时聚焦搜索';

  @override
  String get quickAccessFocusSearchDescription =>
      '打开面板时定位到搜索框。关闭时，点按一次即可使用剪贴项。';

  @override
  String get quickAccessTilesTitle => '快捷设置图块';

  @override
  String get quickAccessTilesSearchDescription => '添加“保存剪贴板”和“剪贴板捕获”图块。';

  @override
  String get quickAccessTilesKeywords => '快捷设置 图块 通知栏 保存 暂停';

  @override
  String get quickAccessTileInstructions =>
      '从屏幕顶部向下滑动两次，点按“编辑”，然后将 CopyPaste 图块拖到快捷设置中。';

  @override
  String get quickAccessSaveTileTitle => '“保存剪贴板”图块';

  @override
  String get quickAccessSaveTileDescription => '从快捷设置保存剪贴板中的内容。';

  @override
  String get quickAccessCaptureTileTitle => '“剪贴板捕获”图块';

  @override
  String get quickAccessCaptureTileDescription => '从快捷设置暂停和恢复捕获。';

  @override
  String get quickAccessAddTile => '添加图块';

  @override
  String get quickAccessTileAdded => '已添加到快捷设置';

  @override
  String get quickAccessWidgetTitle => '“最近的剪贴项”小组件';

  @override
  String get quickAccessWidgetDescription => '在主屏幕上显示最新的剪贴项。点按即可复制。';

  @override
  String get quickAccessWidgetInstructions =>
      '长按主屏幕，点按“小组件”，然后拖动 CopyPaste 的“最近的剪贴项”。';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已添加 $count 个小组件',
      zero: '未添加',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => '添加小组件';

  @override
  String get quickAccessWidgetKeywords => '主屏幕 小组件 启动器 最近';

  @override
  String get aiToolsEnableTitle => '允许 AI 工具访问这台电脑上的 CopyPaste';

  @override
  String get aiToolsEnableDescription =>
      '你连接的 AI 工具可以读取你在下方允许的剪贴板项目。你与 AI 工具分享的内容会发送给该工具的提供方。';

  @override
  String aiToolsStatusOn(int count) {
    return '已开启 · 共享历史的设备：$count';
  }

  @override
  String get aiToolsStatusOff => '已关闭 · 已连接的工具无法访问这台电脑上的 CopyPaste';

  @override
  String get aiToolsThreatModelTitle => '这些限制能防护什么';

  @override
  String get aiToolsThreatModel =>
      '权限、批准和隐去会限制 AI 工具通过其 CopyPaste 连接看到的内容。它们无法防护以你的账户运行的其他程序，这些程序本来就能读取你的剪贴板和历史。';

  @override
  String get aiToolsGrantsTitle => '已连接的工具';

  @override
  String get aiToolsGrantsDescription =>
      '在你任意一台电脑上连接的 AI 工具。更改会在拥有该连接的电脑同步后生效。';

  @override
  String aiToolsServedThrough(String desktops) {
    return '提供服务的电脑：$desktops';
  }

  @override
  String get aiToolsConnect => '连接 AI 工具';

  @override
  String get aiToolsEmptyTitle => '尚未连接 AI 工具';

  @override
  String get aiToolsEmptyMessage => '连接 AI 工具后，它可以搜索你允许的剪贴板项目。';

  @override
  String get aiToolsEmptyServedTitle => 'AI 工具通过你的电脑连接';

  @override
  String get aiToolsEmptyServedMessage =>
      '在 Mac、Windows 或 Linux 电脑上的 CopyPaste 中开启 AI 工具，你手机的历史即可在那里使用。';

  @override
  String get aiToolsUnknownDevice => '另一台电脑';

  @override
  String aiToolsGrantOwner(String device) {
    return '位于 $device';
  }

  @override
  String get aiToolsGrantWaiting => '等待首次连接';

  @override
  String aiToolsGrantUsed(String date) {
    return '已连接 · 上次使用 $date';
  }

  @override
  String get aiToolsGrantRevoked => '已撤销';

  @override
  String aiToolsGrantRevokedRemote(String device) {
    return '已撤销 · $device 同步后生效';
  }

  @override
  String get aiToolsGrantReissue => '恢复后需要新建连接';

  @override
  String get aiToolsGrantPaused => '已暂停：这台电脑上的 AI 工具已关闭';

  @override
  String get aiToolsEdit => '编辑';

  @override
  String get aiToolsActivity => '查看活动';

  @override
  String get aiToolsRevoke => '撤销';

  @override
  String aiToolsRevokeTitle(String name) {
    return '撤销 $name？';
  }

  @override
  String get aiToolsRevokeMessage => '该工具会在下一次请求时失去访问权限。若要重新连接，请新建连接。';

  @override
  String aiToolsDeleteTitle(String name) {
    return '删除 $name？';
  }

  @override
  String get aiToolsDeleteMessage => '该连接会被撤销，并从你的所有设备中删除。';

  @override
  String get aiToolsChangeFailed => '无法保存更改。';

  @override
  String get aiToolsLoadFailed => '无法加载 AI 工具设置';

  @override
  String get aiToolsEditTitle => '编辑连接';

  @override
  String get aiToolsNameLabel => '名称';

  @override
  String get aiToolsScopesLabel => '访问权限';

  @override
  String get aiToolsScopeReadHistory => '读取历史';

  @override
  String get aiToolsScopeReadSnippets => '读取片段';

  @override
  String get aiToolsScopeWriteClipboard => '写入剪贴板';

  @override
  String get aiToolsScopeOrganize => '整理（置顶、保存片段）';

  @override
  String get aiToolsLookbackLabel => '历史范围';

  @override
  String get aiToolsLookbackDay => '过去 24 小时';

  @override
  String get aiToolsLookbackWeek => '过去 7 天';

  @override
  String get aiToolsLookbackMonth => '过去 30 天';

  @override
  String get aiToolsLookbackAll => '全部历史';

  @override
  String aiToolsActivityTitle(String name) {
    return '$name 的活动';
  }

  @override
  String get aiToolsActivityEmpty => '暂无活动';

  @override
  String get aiToolsActivityFailed => '无法加载活动';

  @override
  String aiToolsActivityEntry(String tool, int count) {
    return '$tool · 结果：$count';
  }

  @override
  String get aiToolsClientLabel => 'AI 工具';

  @override
  String get aiToolsClientOther => '其他';

  @override
  String get aiToolsCreate => '创建连接';

  @override
  String aiToolsConfigHint(String client) {
    return '将此配置添加到 $client。其访问密钥只显示一次。';
  }

  @override
  String get aiToolsConfigHintOther => '将此配置添加到你的 AI 工具的 MCP 设置中。其访问密钥只显示一次。';

  @override
  String aiToolsRestartNote(String client) {
    return '下一步：重启 $client 或打开新会话。';
  }

  @override
  String get aiToolsRestartNoteOther => '下一步：重启你的 AI 工具或打开新会话。';

  @override
  String get aiToolsCopied => '已复制';

  @override
  String get aiToolsCopiedMessage => '不会保存到历史，剪贴板将在 60 秒后清除。';

  @override
  String get aiToolsCopyFailed => '无法复制配置。';

  @override
  String get aiToolsPolicyTitle => 'AI 工具可以看到的内容';

  @override
  String get aiToolsPolicyNote => '无论你在这里允许什么，敏感项目和检测到的凭据都不会被分享。';

  @override
  String get aiToolsOtherDevicesTitle => '包含其他设备的项目';

  @override
  String get aiToolsOtherDevicesDescription => '从你的其他设备同步的历史。关闭某台设备即可排除它的项目。';

  @override
  String get aiToolsKindsTitle => '内容类型';

  @override
  String get aiToolsKindsDescription => 'AI 工具可以读取的剪贴板项目类型。';

  @override
  String get aiToolsKindText => '文本';

  @override
  String get aiToolsKindLink => '链接';

  @override
  String get aiToolsKindRichText => '富文本';

  @override
  String get aiToolsKindImage => '图片';

  @override
  String get aiToolsKindFileMetadata => '文件名和大小';

  @override
  String get aiToolsExcludedAppsTitle => '排除的应用';

  @override
  String get aiToolsExcludedAppsDescription => '从这些应用复制的项目永远不会被分享。';

  @override
  String get aiToolsAddApp => '添加应用';

  @override
  String get aiToolsRemove => '移除';

  @override
  String get aiToolsDefaultLookbackTitle => '新连接的历史范围';

  @override
  String get aiToolsDefaultLookbackDescription => '更早的未置顶项目对连接不可见。置顶项目和片段不受限制。';

  @override
  String get aiToolsKeywords => 'ai 人工智能 mcp 助手 claude codex cursor 模型 智能体';

  @override
  String aiToolsApprovalTitle(String client) {
    return '$client 请求访问 CopyPaste';
  }

  @override
  String aiToolsReportedName(String name) {
    return '应用报告的名称：$name';
  }

  @override
  String aiToolsReportedExecutable(String path) {
    return '程序：$path';
  }

  @override
  String get aiToolsApprovalNote => '只允许你自己启动的工具。如果不回应，请求将在 2 分钟后被拒绝。';

  @override
  String get aiToolsDeny => '拒绝';

  @override
  String get aiToolsAllow => '允许';

  @override
  String get aiToolsVolumeTitle => '允许继续读取？';

  @override
  String aiToolsVolumeMessage(String client) {
    return '$client 在本次会话中读取了大量剪贴板历史。允许更多吗？';
  }

  @override
  String get aiToolsNotificationBody => '打开 CopyPaste 以允许或拒绝。';

  @override
  String get aiToolsDecisionFailed => '无法发送回应。请求将再次显示。';

  @override
  String get aiToolsHide => '对 AI 工具隐藏';

  @override
  String get aiToolsShow => '对 AI 工具显示';

  @override
  String get aiToolsHiddenState => '已对 AI 工具隐藏';
}
