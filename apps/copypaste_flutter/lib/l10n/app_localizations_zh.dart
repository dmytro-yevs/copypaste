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
  String get settingsSectionAppearance => '外观';

  @override
  String get settingsSectionAppearanceSummary => '主题和窗口材质。';

  @override
  String get settingsAppearanceDescription => '主题和窗口外观。';

  @override
  String get settingsThemeTitle => '主题';

  @override
  String get settingsThemeDescription => '浅色、深色或跟随系统。';

  @override
  String get settingsThemeKeywords => '深色模式 浅色模式 配色';

  @override
  String get settingsThemeSystem => '系统';

  @override
  String get settingsThemeLight => '浅色';

  @override
  String get settingsThemeDark => '深色';

  @override
  String get settingsAccentTitle => '强调色';

  @override
  String get settingsAccentDescription => '突出显示所选项和当前位置。';

  @override
  String get settingsAccentKeywords => '高亮颜色 色调';

  @override
  String get settingsAccentSystem => '系统';

  @override
  String get settingsAccentStandard => '默认';

  @override
  String get settingsAccentPurple => '紫色';

  @override
  String get settingsAccentPink => '粉色';

  @override
  String get settingsAccentRed => '红色';

  @override
  String get settingsAccentOrange => '橙色';

  @override
  String get settingsAccentYellow => '黄色';

  @override
  String get settingsAccentGreen => '绿色';

  @override
  String get settingsAccentTeal => '青色';

  @override
  String get settingsAccentGraphite => '石墨色';

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
  String get settingsSectionTransforms => '文本转换';

  @override
  String get settingsSectionTransformsSummary => '收藏、流程和粘贴时的默认转换。';

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
  String get pasteQueueSeveralFiles => '请逐个粘贴文件。';

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
  String get pasteQueueAccessibilityMessage => '已复制项目。允许“辅助功能”即可自动粘贴。';

  @override
  String get pasteQueuePasteUnsupported => '无法自动粘贴，已改为复制项目。';

  @override
  String get pasteQueueTargetElevated => '无法粘贴到管理员应用，已改为复制项目。';

  @override
  String get pasteQueueNoTarget => '没有可粘贴的应用，已改为复制项目。';

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
  String get pasteQueueFullMessage => '不再添加新的复制内容。';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue 为空';

  @override
  String get pasteQueueEmptyMessage => '收集复制内容，或从历史记录添加项目。';

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
  String get pasteQueueEditMessage => '更改仅应用于此队列。';

  @override
  String get pasteQueueShortcutPromptTitle => '设置快捷键';

  @override
  String get pasteQueueShortcutPromptMessage => '无需打开 CopyPaste 即可收集和粘贴。';

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
  String get pasteQueueShortcutInUse => '其他应用正在使用此组合键。';

  @override
  String get pasteQueueShortcutRejected => '系统拒绝了此组合。已保留之前的快捷键。';

  @override
  String get pasteQueueShortcutAltGr => 'Ctrl+Alt 在许多键盘布局中用于输入字符。请另选组合。';

  @override
  String get pasteQueueShortcutUnavailable => '此会话中无法使用全局快捷键。';

  @override
  String get pasteQueueShortcutFailed => '无法设置此快捷键。';

  @override
  String get pasteQueueEnableTitle => '启用 Paste Queue';

  @override
  String get pasteQueueEnableDescription => '关闭后，快捷键和收集都会停止。';

  @override
  String get pasteQueuePasteNextStyleTitle => '“粘贴下一项”的操作';

  @override
  String get pasteQueuePasteNextStyleDescription => '自动粘贴，或仅复制到剪贴板。';

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
  String get pasteQueueIdleStopDescription => '在此时间内没有复制时停止收集。';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes 分钟';
  }

  @override
  String get pasteQueueIdleNever => '从不';

  @override
  String get pasteQueueMaxEntriesTitle => '队列大小上限';

  @override
  String get pasteQueueMaxEntriesDescription => '队列已满时不再添加新的复制内容。';

  @override
  String get pasteQueueRestoreTitle => '完成后恢复之前的剪贴板';

  @override
  String get pasteQueueRestoreDescription => '恢复粘贴前复制的内容。';

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
  String get settingsStartupAndroidDescription => '重启后恢复后台服务。';

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
  String get settingsLightweightModeTitle => '轻量模式';

  @override
  String get settingsLightweightModeDescription =>
      '关闭窗口会退出应用，捕获和同步继续运行。托盘、Quick Paste 和快捷键会在重新打开应用后恢复。';

  @override
  String get settingsLightweightModeKeywords =>
      '轻量 关闭 退出 后台 内存 lightweight close quit background memory';

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
      '完整或受限模式，以及 Shizuku 或 ADB 一次性设置。';

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
  String get settingsScreenshotAccessDescription => '用于在后台保存屏幕截图。';

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
  String get settingsMaxFileSizeDescription => '可添加或下载的最大文件。';

  @override
  String get settingsFileStorageTitle => '文件存储';

  @override
  String get settingsFileStorageDescription => '链接仅在原文件存在时有效；副本会保留文件。';

  @override
  String get settingsFileStorageKeywords =>
      '文件 副本 链接 原文件 保留 存储 离线 files copy link original keep store offline';

  @override
  String get settingsFileStorageLink => '链接到原文件';

  @override
  String get settingsFileStorageCopy => '保留副本';

  @override
  String get settingsSaveFolderTitle => '文件保存位置';

  @override
  String get settingsSaveFolderAsk => '每次询问';

  @override
  String get settingsSaveFolderAskAction => '每次询问';

  @override
  String get settingsSaveFolderChooseAction => '选择文件夹';

  @override
  String get settingsSaveFolderSearchDescription => '下载文件的保存文件夹。';

  @override
  String get settingsSaveFolderKeywords =>
      '下载 文件夹 目标 目录 保存 位置 download folder destination directory save location';

  @override
  String get settingsAutoDownloadTitle => '下载收到的文件';

  @override
  String get settingsAutoDownloadDescription => '其他设备在线时，将其文件保存到该文件夹。';

  @override
  String get settingsAutoDownloadNeedsFolder => '请先选择保存文件的文件夹。';

  @override
  String get settingsAutoDownloadKeywords =>
      '自动 下载 接收 同步 文件 automatic download receive sync files';

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
  String get settingsCreateBackupAction => '备份';

  @override
  String get settingsRestoreTitle => '恢复备份';

  @override
  String get settingsRestoreDescription => '用加密备份替换本地历史记录。';

  @override
  String get settingsRestoreAction => '恢复';

  @override
  String get settingsRestoreDialogTitle => '替换本地历史记录？';

  @override
  String get settingsRestoreDialogMessage => '备份将替换此设备上的历史记录。设置会保留。';

  @override
  String get settingsRestoreFailed => '无法恢复备份。';

  @override
  String get settingsSyncSwitchTitle => '同步';

  @override
  String get settingsSyncSwitchDescription => '与已配对设备同步历史记录。';

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
  String get settingsInstantClipboardDescription => '将其他设备的新剪贴项复制到此剪贴板。';

  @override
  String get settingsLanVisibilityTitle => '局域网可见性';

  @override
  String get settingsLanVisibilityDescription => '允许附近的设备发现此设备。';

  @override
  String get settingsLanVisibilityKeywords =>
      '本地 网络 发现 local network discovery';

  @override
  String get settingsRelayTitle => '中继服务器';

  @override
  String get settingsRelayDescription => '通过你自己的中继在不同网络间同步已配对的设备。留空即可关闭。';

  @override
  String get settingsRelayKeywords =>
      '中继 服务器 远程 互联网 网络 relay server remote internet network websocket';

  @override
  String get settingsRelayPlaceholder => 'wss://relay.example.com';

  @override
  String get settingsRelayConnecting => '正在连接中继…';

  @override
  String get settingsRelayConnected => '已连接到中继。';

  @override
  String get settingsRelayUnauthorized => '中继拒绝了访问令牌。';

  @override
  String get settingsRelayUnreachable => '无法连接到中继。';

  @override
  String get settingsPortMappingTitle => '路由器端口映射';

  @override
  String get settingsPortMappingDescription =>
      '允许直连同步请求路由器（UPnP）开放端口几分钟。当两个网络都阻止直接连接时会有帮助。';

  @override
  String get settingsPortMappingKeywords =>
      '路由器 端口 映射 直连 router port mapping upnp nat forwarding direct';

  @override
  String get settingsExcludedAppsTitle => '排除的应用';

  @override
  String get settingsExcludedAppsSearchDescription => '跳过对排除应用的自动捕获。';

  @override
  String get settingsExcludedAppsKeywords =>
      '隐私 应用 标识符 privacy app identifiers';

  @override
  String get settingsExcludedAppsDesktopDescription => '不自动捕获来自这些应用的内容。';

  @override
  String get settingsExcludedAppsAndroidDescription => '存在排除的应用时，自动捕获会暂停。';

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
  String get applicationPickerSearch => '搜索应用';

  @override
  String get applicationPickerEmpty => '未找到应用';

  @override
  String get settingsSkipSecretTitle => '跳过机密剪贴板内容';

  @override
  String get settingsSkipSecretDescription => '不保存应用标记为机密的复制内容。';

  @override
  String get settingsSkipSecretKeywords =>
      '机密 密码 敏感 隐藏 遮盖 secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => '跳过临时剪贴板内容';

  @override
  String get settingsSkipTransientDescription => '不保存应用标记为临时的复制内容。';

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
  String get settingsQuickPasteShortcutDescription => 'CopyPaste 在后台运行时可用。';

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
  String get devicesSyncTitle => '同步';

  @override
  String get devicesSyncDescription => '关闭后已保存的剪贴项会保留。';

  @override
  String get devicesSyncThisDevice => '同步此设备';

  @override
  String get devicesSyncPeer => '与此设备同步';

  @override
  String get devicesSyncPause => '暂停自动同步';

  @override
  String get devicesSyncText => '文本';

  @override
  String get devicesSyncImages => '图片';

  @override
  String get devicesSyncFiles => '文件';

  @override
  String get devicesSyncAfterFirstSync => '与此设备首次同步后将显示设置。';

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
  String get pairingAddressMissing => '请输入设备的 host:port 地址，或在设置中配置中继服务器。';

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
  String get historySearchSuggestions => '搜索建议';

  @override
  String get historySearchSuggestionRecent => '最近';

  @override
  String get historySearchSuggestionTag => '标签';

  @override
  String get historySearchSuggestionApp => '应用';

  @override
  String get historySearchSuggestionKind => '类型';

  @override
  String get historySearchSuggestionDevice => '设备';

  @override
  String get historySearchClearRecent => '清除最近搜索';

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
  String get historyShare => '分享';

  @override
  String get historyErrorShareClip => '无法分享此剪贴项。';

  @override
  String get historyErrorShareClips => '无法分享所选剪贴项。';

  @override
  String get historyBulkShare => '分享所选项';

  @override
  String get historyBulkShareFilesOnlyTitle => '仅分享了文件';

  @override
  String get historyBulkShareFilesOnly => '此系统无法将文本与文件一起分享，因此已略过文本剪贴项。请单独分享它们。';

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
  String get modulesInstallFromFile => '从文件安装';

  @override
  String get modulesPackageFileType => 'CopyPaste 模块';

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
  String get modulesSmsApp => 'CopyPaste SMS';

  @override
  String get modulesSmsAppDescription => '唯一读取短信的独立应用';

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
  String get devicesEmptyMessage => '与同一网络中的设备配对以同步历史记录。';

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
  String sharedStepCounter(int step, int count) {
    return '第 $step 步，共 $count 步';
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
    return '新上限：$limit。较早的未置顶项目将被永久删除。';
  }

  @override
  String get settingsStorageQuotaConfirmTitle => '减小存储配额？';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return '新的配额：$size。超出部分将从最早的未置顶项目开始永久删除。';
  }

  @override
  String get settingsLimitConfirmAction => '应用限制';

  @override
  String get settingsHideEverywhereTitle => '完全隐藏 CopyPaste？';

  @override
  String get settingsHideEverywhereMessage =>
      '两项都关闭后将没有图标。要打开，请再次启动 CopyPaste。';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return '两项都关闭后将没有图标。要打开，请按 $shortcut 或再次启动 CopyPaste。';
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
    return '新的配额：$size。当前已用 $used，将从最早的未置顶项目开始永久删除。';
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
  String get trayRecentPreviewDescription => '菜单显示的剪贴文本量。';

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
  String get transformsSavedMessage => '已保存为新的历史记录项目。';

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
  String get transformsRestoreDescription => '粘贴后恢复之前的剪贴板内容。';

  @override
  String get transformsDefaultTitle => '默认粘贴转换';

  @override
  String get transformsDefaultDescription => '应用于从 Quick Paste 粘贴的文本。';

  @override
  String get transformsNoDefault => '无';

  @override
  String get transformsAppDefaultsTitle => '按应用设置默认值';

  @override
  String get transformsAppDefaultsDescription => '为特定应用使用不同的默认设置。';

  @override
  String transformsOtherPlatform(String platform) {
    return '仅限 $platform';
  }

  @override
  String get transformsAddApp => '添加应用';

  @override
  String get transformsRemove => '移除';

  @override
  String get transformsFavoritesTitle => '收藏';

  @override
  String get transformsFavoritesDescription => '在每个面板中优先显示。';

  @override
  String transformsFavoriteShortcut(int number) {
    return '面板中按 Mod+$number';
  }

  @override
  String get transformsAddFavorite => '添加收藏';

  @override
  String get transformsPipelinesTitle => '流水线';

  @override
  String get transformsPipelinesDescription => '已保存的转换组合。';

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
  String get transformsDeletePipelineMessage => '使用它的收藏和粘贴默认设置将被清除。';

  @override
  String get transformsDeletePipelineFailed => '无法删除流水线。';

  @override
  String get transformsAddStep => '添加步骤';

  @override
  String get transformsHiddenTitle => '隐藏的转换';

  @override
  String get transformsHiddenDescription => '不在面板和建议中显示。';

  @override
  String get transformsHideTransform => '隐藏转换';

  @override
  String get transformsShow => '显示';

  @override
  String get transformsSuggestionsTitle => '建议';

  @override
  String get transformsSuggestionsDescription => '推荐适合文本的转换。';

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
  String get transformsTrackingDescription => '额外移除或始终保留的链接参数。';

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
  String get quickAccessKeyboardDescription => '在键盘设置中开启“CopyPaste 剪贴项”。';

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
  String get quickAccessSwitchKeyboardDescription => '将“CopyPaste 剪贴项”设为当前键盘。';

  @override
  String get quickAccessSwitchKeyboardAction => '选择键盘';

  @override
  String get quickAccessKeyboardCurrent => '当前键盘';

  @override
  String get quickAccessCaptureTierTitle => '捕获模式';

  @override
  String get quickAccessCaptureTierDescription => '其他应用中的复制内容如何保存。';

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
  String get quickAccessAutoReturnDescription => '插入一个剪贴项后切换回来。';

  @override
  String get quickAccessAutoReturnKeywords => '返回 上一个键盘 自动 插入';

  @override
  String get quickAccessFocusSearchTitle => '打开面板时聚焦搜索';

  @override
  String get quickAccessFocusSearchDescription => '打开面板时聚焦搜索。';

  @override
  String get quickAccessTilesTitle => '快捷设置图块';

  @override
  String get quickAccessTilesSearchDescription => '添加“保存剪贴板”和“剪贴板捕获”图块。';

  @override
  String get quickAccessTilesKeywords => '快捷设置 图块 通知栏 保存 暂停';

  @override
  String get quickAccessTileInstructions => '向下滑动两次，点按“编辑”，然后添加 CopyPaste 图块。';

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
  String get quickAccessWidgetDescription => '主屏幕上的最近剪贴项，轻点即可复制。';

  @override
  String get quickAccessWidgetInstructions => '长按主屏幕，点按“小组件”，然后添加“最近的剪贴项”。';

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
  String get snippetsClipTooLarge => '内容过大，无法编辑。此剪贴项以只读方式打开。';

  @override
  String get snippetsConcealedConfirmAction => '继续';

  @override
  String get snippetsConcealedConfirmMessage => '此设备无法验证你的身份。仅在你自己的设备上继续。';

  @override
  String get snippetsConcealedConfirmTitle => '确认是你本人';

  @override
  String get snippetsDiscard => '放弃';

  @override
  String get snippetsEditClip => '编辑剪贴项';

  @override
  String get snippetsFormattingNotice => '保存时将移除格式。';

  @override
  String get snippetsNewText => '新建文本';

  @override
  String get snippetsReplaceOriginal => '替换原剪贴项';

  @override
  String get snippetsReplaceOriginalHint => '保存后删除原始剪贴项。';

  @override
  String get snippetsSaveFailed => '无法保存';

  @override
  String get snippetsUnsavedMessage => '关闭前保存更改吗？';

  @override
  String get snippetsUnsavedTitle => '未保存的更改';

  @override
  String get quickPasteTabQueue => '队列';

  @override
  String get quickPasteMaskedMode => '密码字段：预览已隐藏';

  @override
  String quickPasteMaskedItem(String kind) {
    return '已隐藏：$kind';
  }

  @override
  String get quickPasteUnlockSecrets => '解锁机密项目';

  @override
  String quickPastePastingWith(String name) {
    return '粘贴时使用：$name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut 粘贴原始内容';
  }

  @override
  String get quickPasteDefaultSkipped => '已跳过默认转换';

  @override
  String get quickPasteDefaultSkippedMessage => '已改用原始内容。';

  @override
  String get quickPastePasteManually => '内容已在剪贴板中，请手动粘贴。';

  @override
  String get quickPastePaste => '粘贴';

  @override
  String get rulesLoadFailed => '无法加载内容规则。';

  @override
  String get rulesNewRule => '新建规则';

  @override
  String get rulesEditRule => '编辑规则';

  @override
  String get rulesUntitled => '未命名规则';

  @override
  String get rulesPrivacyDefaultsTitle => '隐私默认设置';

  @override
  String get rulesPrivacyDefaultsDescription => '隐私设置先于所有规则生效。';

  @override
  String get rulesPrivacyDefaultsKeywords => '隐私 隐藏 临时 排除的应用 内置 凭据 密码 银行卡';

  @override
  String get rulesOpenPrivacy => '打开隐私';

  @override
  String get rulesMyRulesTitle => '我的规则';

  @override
  String get rulesMyRulesDescription => '按顺序作用于新的复制内容。';

  @override
  String get rulesMyRulesKeywords => '规则 自动化 标签 跳过 机密 过期 删除 模式 正则';

  @override
  String get rulesEmptyTitle => '还没有规则';

  @override
  String get rulesEmptyMessage => '规则可为新复制内容加标签、保护、设置过期或跳过。';

  @override
  String get rulesDeletedTitle => '最近删除';

  @override
  String get rulesDeletedDescription => '可在 30 天内恢复。';

  @override
  String rulesDeletedAt(String time) {
    return '删除于 $time';
  }

  @override
  String get rulesRestore => '恢复';

  @override
  String get rulesDeletedToast => '规则已删除';

  @override
  String get rulesDeletedToastMessage => '你可以在 30 天内恢复它。';

  @override
  String get rulesUndo => '撤销';

  @override
  String rulesConflictMessage(String rule, String device) {
    return '来自 $device 的“$rule”的同时编辑已被更新的编辑替换。';
  }

  @override
  String get rulesAnotherDevice => '另一台设备';

  @override
  String get rulesRestoreConflict => '使用该版本';

  @override
  String get rulesSkipLogTitle => '最近跳过';

  @override
  String get rulesSkipLogDescription => '你的规则在此设备上未保存的复制内容。';

  @override
  String get rulesSkipLogKeywords => '跳过 忽略 未保存 日志';

  @override
  String get rulesSkipLogShow => '显示';

  @override
  String get rulesSkipLogEmpty => '没有跳过任何复制内容。';

  @override
  String get rulesUnknownApp => '未知应用';

  @override
  String get rulesMatchesEverything => '任何复制内容';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary，然后停止';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {'any': ' 或 ', 'other': ' 且 '});
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
    return '$action：$value';
  }

  @override
  String rulesRunsOn(String devices) {
    return '运行于 $devices';
  }

  @override
  String rulesNewFromDevice(String device) {
    return '来自 $device 的新规则';
  }

  @override
  String get rulesNeedsUpdateBadge => '需要更新';

  @override
  String get rulesUpdateRequired => '更新 CopyPaste 以运行此规则。';

  @override
  String get rulesErrorBadge => '错误';

  @override
  String rulesErrorHint(String problem) {
    return '$problem 修复前，匹配的内容会受到保护。';
  }

  @override
  String get rulesReorder => '拖动以重新排序';

  @override
  String get rulesMoreActions => '更多操作';

  @override
  String get rulesEdit => '编辑';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': '上移',
      'down': '下移',
      'top': '移到顶部',
      'other': '移到底部',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => '应用到现有项目…';

  @override
  String get rulesResetBuiltin => '恢复默认';

  @override
  String get rulesNameLabel => '名称';

  @override
  String get rulesNamePlaceholder => '例如：工作链接';

  @override
  String get rulesWhenTitle => '条件';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': '满足任一条件',
      'other': '满足所有条件',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions => '无条件：此规则适用于所有复制内容。';

  @override
  String get rulesAddCondition => '添加条件';

  @override
  String get rulesRemoveCondition => '移除条件';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': '文本',
      'content_type': '内容类型',
      'detected_kind': '检测到的类型',
      'source_app': '来源应用',
      'size': '大小',
      'privacy_marker': '隐私标记',
      'capture_origin': '来源',
      'other': '未知条件',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': '包含',
      'equals': '等于',
      'starts_with': '开头是',
      'ends_with': '结尾是',
      'matches_regex': '匹配模式',
      'other': '匹配',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': '不包含',
      'equals': '不等于',
      'starts_with': '开头不是',
      'ends_with': '结尾不是',
      'matches_regex': '不匹配模式',
      'other': '不匹配',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {'true': '不是', 'other': '是'});
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': '小于',
      'other': '大于',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': '不小于',
      'other': '不大于',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': '行',
      'bytes': '字节',
      'other': '字符',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': '行',
      'bytes': '字节',
      'other': '个字符',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': '纯文本',
      'rich_text': '富文本',
      'image': '图片',
      'files': '文件',
      'color': '颜色',
      'link': '链接',
      'other': '其他',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': '密码或机密',
      'url': '网址',
      'email': '电子邮件地址',
      'phone': '电话号码',
      'code': '代码',
      'color': '颜色值',
      'json': 'JSON',
      'file_path': '文件路径',
      'number': '数字',
      'one_time_code': '一次性验证码',
      'payment_card': '银行卡号',
      'other': '其他',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': '隐藏',
      'transient': '临时',
      'auto_generated': '自动生成',
      'excluded_from_history': '不计入历史记录',
      'other': '其他',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': '在此设备上复制',
      'received': '从其他设备接收',
      'created_in_app': '在 CopyPaste 中创建',
      'other': '其他',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => '要查找的文本';

  @override
  String get rulesPatternPlaceholder => '模式，例如 ^ticket-[0-9]+';

  @override
  String get rulesPatternHint => '不支持环视和反向引用。';

  @override
  String get rulesCaseSensitive => '区分大小写';

  @override
  String get rulesChooseValues => '选择';

  @override
  String get rulesUnknownConditionNotice => '此条件来自较新版本的 CopyPaste。';

  @override
  String get rulesSourceAppNotice => '在 Android 上，仅匹配通过剪贴项键盘复制的内容。';

  @override
  String get rulesPrivacyMarkerNotice => '并非所有系统都会报告所有标记。';

  @override
  String get rulesThenTitle => '操作';

  @override
  String get rulesNoActions => '请至少添加一个操作。';

  @override
  String get rulesAddAction => '添加操作';

  @override
  String get rulesRemoveAction => '移除操作';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': '不保存',
      'mark_secret': '标记为机密',
      'delete_after': '在此时间后删除',
      'add_tags': '添加标签',
      'transform': '转换',
      'other': '未知操作',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone': '“不保存”不能与其他操作组合。',
      'present': '此规则已包含此操作。',
      'other': '一条规则最多可有 8 个操作。',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice => '此操作来自较新版本的 CopyPaste。';

  @override
  String get rulesDontSaveWarning => '匹配的复制内容永远不会被存储、索引或同步。';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 分钟',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 小时',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 天',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry => '如果剪贴板仍是此内容，也一并清除。';

  @override
  String get rulesNoTags => '在历史记录中创建标签后即可在此使用。';

  @override
  String get rulesNoTransforms => '没有可用的转换。';

  @override
  String get rulesReplaceClipboard => '同时用结果替换剪贴板';

  @override
  String get rulesOptionsTitle => '选项';

  @override
  String get rulesEnabledSwitch => '已启用';

  @override
  String get rulesStopProcessing => '此规则匹配时停止处理后续规则';

  @override
  String get rulesRunsOnLabel => '运行于';

  @override
  String get rulesAllDevices => '所有设备';

  @override
  String get rulesChosenDevices => '所选设备';

  @override
  String get rulesTestTitle => '测试';

  @override
  String get rulesTestDescription => '用示例文本试用规则。';

  @override
  String get rulesSamplePlaceholder => '粘贴或输入示例文本';

  @override
  String get rulesTestFullSet => '与所有规则一起测试';

  @override
  String get rulesRunTest => '运行测试';

  @override
  String get rulesTestFailed => '无法运行测试。';

  @override
  String get rulesPreviewHistory => '在历史记录上预览';

  @override
  String get rulesSaveFailed => '无法保存规则。';

  @override
  String get rulesReadOnlyNotice => '由较新版本的 CopyPaste 创建。请更新后再编辑或运行。';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': '无法读取此规则。',
      'unsupported_schema': '此规则需要较新版本的 CopyPaste。',
      'unknown_condition_type': '此条件需要较新版本的 CopyPaste。',
      'unknown_action_type': '此操作需要较新版本的 CopyPaste。',
      'too_many_conditions': '一条规则最多可有 20 个条件。',
      'too_many_actions': '一条规则最多可有 8 个操作。',
      'too_many_rules': '只会运行前 200 条规则。',
      'no_actions': '请至少添加一个操作。',
      'conflicting_actions': '“不保存”不能与其他操作组合。',
      'duplicate_action': '每个操作只能使用一次。',
      'empty_value': '请输入一个值。',
      'empty_list': '请至少选择一个值。',
      'too_many_values': '最多选择 64 个值。',
      'invalid_value': '此值无效。',
      'pattern_too_long': '模式最多 2,000 个字符。',
      'unsupported_look_around': '不支持前瞻和后顾。请改用“满足所有条件”搭配两个条件。',
      'unsupported_backreference': '不支持反向引用。',
      'pattern_too_large': '此模式过于复杂。请简化其重复部分。',
      'invalid_pattern': '此模式无效。',
      'invalid_duration': '请选择 1 分钟到 90 天之间的时长。',
      'invalid_tags': '请至少选择一个标签。',
      'invalid_transform': '请选择一个转换。',
      'other': '此规则有问题。',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceLine(String label, String result) {
    return '$label：$result';
  }

  @override
  String rulesTraceResult(String result) {
    String _temp0 = intl.Intl.selectLogic(result, {
      'matched': '匹配',
      'not_matched': '不匹配',
      'unknown': '未知',
      'skipped': '未运行',
      'other': '错误',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return '条件 $number';
  }

  @override
  String get rulesTraceNoRules => '未评估任何规则。';

  @override
  String get rulesTraceStopped => '后续规则未运行。';

  @override
  String get rulesTraceUnknownReason => '此复制内容的该项未知，因此不匹配。';

  @override
  String get rulesTraceRuleError => '此规则无法运行，因此匹配内容会受到保护。';

  @override
  String get rulesTraceOutcomeTitle => '结果';

  @override
  String get rulesTraceNoEffect => '没有规则会更改此复制内容。';

  @override
  String get rulesTraceSkipped => '不保存';

  @override
  String get rulesTraceSecret => '保存为机密';

  @override
  String rulesTraceExpires(String duration) {
    return '$duration后删除';
  }

  @override
  String rulesTraceTags(String tags) {
    return '标签：$tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return '转换：$names';
  }

  @override
  String get rulesTraceOutput => '转换后的文本';

  @override
  String get rulesPreviewTitle => '应用到现有项目';

  @override
  String rulesPreviewRunning(int count) {
    return '正在检查最近的 $count 个剪贴项…';
  }

  @override
  String get rulesPreviewNone => '没有最近的剪贴项匹配此规则。';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个剪贴项匹配',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '将删除 $count 个',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '将有 $count 个标记为机密',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '将有 $count 个设置删除时间',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '将有 $count 个添加标签',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return '仅检查了最近的 $count 个剪贴项。';
  }

  @override
  String rulesPreviewMore(int count) {
    return '以及另外 $count 个';
  }

  @override
  String get rulesPreviewNote => '应用之前不会有任何更改。';

  @override
  String get rulesSecretItem => '机密项目';

  @override
  String rulesSecretItemFrom(String app) {
    return '来自 $app 的机密项目';
  }

  @override
  String get rulesPreviewCancelled => '预览已停止。未做任何更改。';

  @override
  String get rulesPreviewFailed => '无法运行预览。';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '删除 $count 个剪贴项？',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage => '匹配的剪贴项将从你的所有设备中删除。此操作无法撤销。';

  @override
  String get rulesApplyDeleteConfirm => '删除并应用';

  @override
  String rulesApplyProgress(int done, int total) {
    return '正在应用：$done/$total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已更改 $count 个剪贴项',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已删除 $count 个剪贴项',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '有 $count 个剪贴项无法更改',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled => '已停止。已完成的更改会保留。';

  @override
  String get historyActionEdit => '编辑';

  @override
  String get historyActionAddToQueue => '添加到 Paste Queue';

  @override
  String get historyActionSendTo => '发送到…';

  @override
  String get historySendDialogTitle => '发送到设备';

  @override
  String get historySendDescription => '即使自动同步已暂停，也会立即发送。';

  @override
  String get historySendNoDevices => '还没有已同步的配对设备。';

  @override
  String get historySendAction => '发送';

  @override
  String get historySentTitle => '已发送';

  @override
  String historySentTo(String devices) {
    return '已发送到 $devices。';
  }

  @override
  String get historySendFailedTitle => '未发送';

  @override
  String historySendBlocked(String device) {
    return '$device 的同步设置不允许此剪贴项。';
  }

  @override
  String historySendFailed(String device) {
    return '$device 未能接收剪贴项。请确认设备在线后重试。';
  }

  @override
  String get historySendUnavailable => '当前无法发送。';

  @override
  String get historyActionTransform => '转换…';

  @override
  String get historyActionExplain => '为什么会在这里？';

  @override
  String get historyActionKeep => '保留';

  @override
  String get historyNewClip => '新建剪贴项';

  @override
  String get historySavedTitle => '已保存';

  @override
  String get historyEditSaved => '编辑后的文本已作为新剪贴项放在历史记录顶部。';

  @override
  String get historyNewClipSaved => '你的文本已保存。';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已将 $count 个剪贴项加入队列。',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed => '无法添加到 Paste Queue。请确认它已开启。';

  @override
  String get historyKeepFailed => '无法保留此剪贴项。';

  @override
  String get historyExplainFailed => '无法加载说明。';

  @override
  String get historyDeletesSoon => '不到一分钟后删除';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 分钟后删除',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 小时后删除',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 天后删除',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => '手动';

  @override
  String get historyProvenanceEdited => '已编辑';

  @override
  String get historyProvenanceTransformed => '已转换';

  @override
  String get historyProvenanceModule => '模块';

  @override
  String historyRuleTag(String name) {
    return '$name（自动）';
  }

  @override
  String get historyMetadataExpiry => '删除';

  @override
  String get historyMetadataRuleTags => '自动标签';

  @override
  String modulesEnableTitle(String module) {
    return '启用 $module？';
  }

  @override
  String get modulesEnableAccess => '它将能够：';

  @override
  String modulesCapability(String capability) {
    String _temp0 = intl.Intl.selectLogic(capability, {
      'history_read': '读取剪贴板历史',
      'clipboard_write': '复制到剪贴板',
      'library_write': '保存片段并置顶条目',
      'snippets': '读取你的片段',
      'synced_records': '在你的设备间同步其数据',
      'local_state': '在此设备上保存其数据',
      'prompts': '请你确认操作',
      'endpoints': '接受本地应用的连接',
      'other': '使用 CopyPaste 服务',
    });
    return '$_temp0';
  }

  @override
  String modulesPromptFrom(String module) {
    return '来自 $module';
  }

  @override
  String get modulesUnknownModule => '未知模块';

  @override
  String get modulesPromptAnswerFailed => '你的回答未发送。';

  @override
  String get modulesPromptNotificationBody => '打开 CopyPaste 以回复。';

  @override
  String get modulesUnknownDevice => '未知设备';

  @override
  String get modulesChooseDevices => '选择设备';

  @override
  String get modulesAddApp => '添加应用';

  @override
  String modulesRemoveApp(String app) {
    return '移除 $app';
  }

  @override
  String get modulesNoApps => '未选择应用';

  @override
  String get settingsSectionDiagnostics => '诊断';

  @override
  String get settingsSectionDiagnosticsSummary => '用于排查问题的运行日志。剪贴板内容从不记录。';

  @override
  String get diagnosticsSearchPlaceholder => '搜索日志';

  @override
  String get diagnosticsAllLevels => '所有级别';

  @override
  String get diagnosticsAllProcesses => '所有进程';

  @override
  String get diagnosticsLevelError => '错误';

  @override
  String get diagnosticsLevelWarning => '警告';

  @override
  String get diagnosticsLevelInfo => '信息';

  @override
  String get diagnosticsLevelDebug => '调试';

  @override
  String get diagnosticsLevelTrace => '跟踪';

  @override
  String get diagnosticsProcessApp => '应用';

  @override
  String get diagnosticsProcessDaemon => '后台服务';

  @override
  String get diagnosticsRefresh => '刷新';

  @override
  String get diagnosticsExport => '导出';

  @override
  String get diagnosticsExportFileType => '文本文件';

  @override
  String get diagnosticsExported => '诊断已导出';

  @override
  String get diagnosticsExportedMessage => '日志文件已可发送给支持团队。';

  @override
  String get diagnosticsExportFailed => '导出失败';

  @override
  String get diagnosticsExportFailedMessage => '无法保存日志文件。';

  @override
  String get diagnosticsLoadFailed => '无法加载日志';

  @override
  String get diagnosticsEmpty => '没有日志条目';

  @override
  String get diagnosticsLogLevelTitle => '日志级别';

  @override
  String get diagnosticsLogLevelDescription => '此设备记录的详细程度。日志从不包含剪贴板内容。';

  @override
  String get onboardingAndroidCaptureMethod => '检测方式';

  @override
  String get onboardingAndroidLogTrigger => '系统日志（推荐）';

  @override
  String get onboardingAndroidLogTriggerDescription =>
      '即时，无额外耗电。需要通过 Shizuku 或 ADB 授权一次。';

  @override
  String get onboardingAndroidPolling => '轮询';

  @override
  String get onboardingAndroidPollingDescription => '屏幕亮着时每秒检查一次。只需悬浮窗权限，但更耗电。';

  @override
  String get onboardingAndroidOverlay => '显示在其他应用上层';

  @override
  String get onboardingAndroidOverlayDescription => '让一个不可见的 1 像素窗口读取剪贴板。';

  @override
  String get onboardingAndroidOverlaySettingsFailed => '无法打开悬浮窗设置。';

  @override
  String get settingsBackgroundSyncTitle => '后台同步';

  @override
  String get settingsBackgroundSyncDescription => '应用关闭时继续同步，并显示一条通知。';

  @override
  String get settingsBackgroundSyncKeywords =>
      '后台 常驻 服务 通知 电池 keep alive persistent service notification battery';

  @override
  String get settingsBackgroundSyncStoppedTitle => '后台同步未运行';

  @override
  String get settingsBackgroundSyncStoppedMessage => '请允许通知，以便 Android 保持同步运行。';

  @override
  String get settingsBackgroundSyncResume => '恢复同步';

  @override
  String get quickAccessSendShortcutTitle => '“发送剪贴板”快捷方式';

  @override
  String get quickAccessSendShortcutDescription => '保存当前剪贴板并发送到你的设备。';

  @override
  String get quickAccessTransformShortcutTitle => '“转换剪贴板”快捷方式';

  @override
  String get quickAccessTransformShortcutDescription => '打开当前剪贴板的转换。';

  @override
  String get quickAccessAddToHomeScreen => '添加到主屏幕';

  @override
  String get quickAccessShortcutKeywords =>
      '快捷方式 主屏幕 启动器 图标 shortcut home screen launcher icon';

  @override
  String fileSetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个文件',
    );
    return '$_temp0';
  }

  @override
  String get fileSetShowFolder => '显示文件夹';

  @override
  String get historyFilesSaved => '文件已保存。';

  @override
  String get transfersResume => '继续下载';

  @override
  String get settingsStartInBackgroundTitle => '在后台启动';

  @override
  String get settingsStartInBackgroundDescription => 'CopyPaste 随登录启动时不打开窗口。';

  @override
  String get settingsWindowGlassTitle => '玻璃效果';

  @override
  String get settingsWindowGlassDescription => '模糊窗口后方的桌面。系统降低透明度时自动关闭。';

  @override
  String get settingsRestoreClipboardTitle => '启动时恢复剪贴板';

  @override
  String get settingsRestoreClipboardDescription =>
      '启动时将最新的剪贴内容放回空剪贴板。机密内容会被跳过。';

  @override
  String get settingsStorageUsageTitle => '存储空间占用';

  @override
  String get settingsStorageUsageDescription => 'CopyPaste 在此设备上占用的空间。';

  @override
  String get settingsStorageUsageUnavailable => '无法读取存储空间占用。';

  @override
  String get settingsStorageHistory => '历史记录';

  @override
  String get settingsStorageFiles => '文件';

  @override
  String get settingsStorageCaches => '缓存';

  @override
  String get settingsStorageUpgradeBackups => '升级备份';

  @override
  String get settingsStorageOther => '日志、模块和设置';

  @override
  String get settingsStorageTotal => '总计';

  @override
  String get settingsClearCacheTitle => '清除缓存';

  @override
  String get settingsClearCacheDescription => '删除预览和临时文件，历史记录保留。';

  @override
  String get settingsClearCacheAction => '清除';

  @override
  String get settingsClearCacheDoneTitle => '缓存已清除';

  @override
  String get settingsClearCacheDoneMessage => '需要时会重新生成预览。';

  @override
  String get settingsClearCacheFailed => '无法清除缓存。';

  @override
  String get settingsDataFolderTitle => '数据文件夹';

  @override
  String get settingsDataFolderDescription => '显示历史记录、备份和模块的存放位置。';

  @override
  String get settingsLogsFolderTitle => '日志文件夹';

  @override
  String get settingsLogsFolderDescription => '显示此设备的诊断日志。';

  @override
  String get settingsOpenFolderAction => '打开';

  @override
  String get settingsOpenFolderFailed => '无法打开文件夹。';

  @override
  String get settingsConfirmDeleteTitle => '删除前确认';

  @override
  String get settingsConfirmDeleteDescription => '删除单条内容前先确认。删除多条时始终确认。';

  @override
  String get settingsUpgradeBackupsTitle => '升级备份';

  @override
  String get settingsUpgradeBackupsDescription => '数据升级前生成的加密副本，保留最近 5 份。';

  @override
  String get settingsUpgradeBackupsEmpty => '暂无升级备份。';

  @override
  String settingsUpgradeBackupEntry(String date, String size) {
    return '$date · $size';
  }

  @override
  String get settingsUpgradeBackupRestoreAction => '恢复';

  @override
  String get settingsUpgradeBackupRestoreTitle => '恢复此备份？';

  @override
  String settingsUpgradeBackupRestoreMessage(String date) {
    return '历史记录将替换为 $date 的副本，之后添加的内容会被删除。';
  }

  @override
  String get settingsDownloadUpdatesTitle => '在后台下载更新';

  @override
  String get settingsDownloadUpdatesDescription =>
      '预先下载并验证找到的更新，以便立即安装。在按流量计费的网络和低电量模式下等待。';

  @override
  String settingsUpdateDownloadedReady(String available) {
    return 'CopyPaste $available 已下载，可以安装。';
  }

  @override
  String get devicesSpaceTitle => '你的空间';

  @override
  String devicesSpaceSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 台设备',
    );
    return '$_temp0';
  }

  @override
  String get devicesSpaceKeyPending => '离线设备上线后会收到新密钥。';

  @override
  String get devicesSpaceWaitingForKey => '需要新密钥';

  @override
  String get devicesSpaceRemovedTitle => '未加入空间';

  @override
  String get devicesSpaceRemovedMessage =>
      '此设备已被移除或已退出。它的历史记录保留在本设备上。加入空间即可重新同步。';

  @override
  String get devicesSpaceJoin => '加入空间';

  @override
  String get devicesRemoveFromSpace => '从空间中移除';

  @override
  String devicesRemoveDialogTitle(String device) {
    return '移除 $device?';
  }

  @override
  String get devicesRemoveDialogMessage => '它将在你的所有设备上被移除，并且无法读取此后同步的任何内容。';

  @override
  String get devicesRemoveNewPassphrase => '新的同步口令（可选）';

  @override
  String get devicesRemoveNewPassphraseHint =>
      '被移除的设备知道当前口令。如果不设置新口令，只能通过邀请添加设备。';

  @override
  String get devicesRemoveSignOutOthers => '让其他设备退出云账户';

  @override
  String get devicesPassphraseTooShort => '至少使用 12 个字符。';

  @override
  String get devicesLeaveSpace => '退出空间';

  @override
  String get devicesLeaveDialogTitle => '退出此空间？';

  @override
  String get devicesLeaveDialogMessage => '你的其他设备会移除此设备。它的历史记录保留在本设备上。';

  @override
  String get devicesLeave => '退出';

  @override
  String get onboardingSpaceTitle => '你的设备';

  @override
  String get onboardingSpaceSubtitle => '在你自己的设备之间同步剪贴板。所有内容始终加密。';

  @override
  String get onboardingSpaceCreateTitle => '创建空间';

  @override
  String get onboardingSpaceCreateDescription => '这是我第一台使用 CopyPaste 的设备。';

  @override
  String get onboardingSpaceJoinTitle => '加入空间';

  @override
  String get onboardingSpaceJoinDescription => '我已在另一台设备上使用 CopyPaste。';

  @override
  String get onboardingSpaceLater => '稍后';

  @override
  String get onboardingReadySpaceCreated => '你的空间已就绪。现在或稍后在“设备”中添加其他设备。';

  @override
  String get onboardingReadyJoin => '接下来，确认另一台设备上显示的代码。';

  @override
  String get onboardingAddDeviceNow => '添加设备';

  @override
  String get onboardingJoinNow => '立即加入';

  @override
  String get onboardingBadgeWelcome => '欢迎';

  @override
  String get onboardingBadgeSetup => '设置';

  @override
  String get onboardingBadgeDevices => '设备';

  @override
  String get onboardingBadgeReady => '就绪';

  @override
  String get onboardingPairJoinTitle => '加入你的空间';

  @override
  String get onboardingPairJoinSubtitle =>
      '输入另一台设备上的代码或扫描其二维码。然后确认两台设备显示相同的验证码。';

  @override
  String get onboardingPairAddDeviceSubtitle =>
      '在另一台设备上打开 CopyPaste，并使用此二维码或代码加入。然后确认两台设备显示相同的验证码。';

  @override
  String get onboardingPairSkip => '暂时跳过';

  @override
  String get onboardingPairInterruptedTitle => '配对未完成';

  @override
  String get onboardingPairInterruptedMessage => '请重试，或继续并稍后在“设备”中配对。';

  @override
  String onboardingReadyPaired(String device) {
    return '已与 $device 配对。你的剪贴板现在会在它们之间同步。';
  }

  @override
  String get onboardingReadyPairedUnnamed => '你的设备已配对。你的剪贴板现在会在设备之间同步。';

  @override
  String get onboardingReadyPairLater => '你可以随时在“设备”中配对设备。';

  @override
  String get settingsSectionShortcuts => '快捷键';

  @override
  String get settingsSectionShortcutsSummary => '全局键盘快捷键。可以关闭不用的快捷键。';

  @override
  String get settingsShortcutEnabledLabel => '关闭此快捷键';

  @override
  String get settingsWelcomeTourTitle => '欢迎导览';

  @override
  String get settingsWelcomeTourDescription => '重新走一遍首次设置。';

  @override
  String get settingsWelcomeTourAction => '再次显示';

  @override
  String get modulesDetails => '详情';

  @override
  String get modulesDetailsFeatures => '新增功能';

  @override
  String get modulesDetailsSearch => '使用本地模型按含义查找剪贴。';

  @override
  String get modulesDetailsPermissions => '权限';

  @override
  String get modulesDetailsPermissionsPending => '安装后列出。开启模块时由你确认。';

  @override
  String get modulesDetailsNoPermissions => '不使用 CopyPaste 服务。';

  @override
  String get modulesDetailsInformation => '信息';

  @override
  String get modulesDetailsVersion => '版本';

  @override
  String get modulesDetailsSize => '大小';

  @override
  String get modulesDetailsRequirements => '要求';

  @override
  String get modulesDetailsStatus => '状态';

  @override
  String get errorUnexpected => '出现问题，请重试。';

  @override
  String get errorRuntimeUnavailable => 'CopyPaste 未在后台运行。请重新启动应用。';

  @override
  String get errorRuntimeStartFailed => 'CopyPaste 无法启动后台服务。请重新启动应用。';

  @override
  String get errorRuntimeStarting => 'CopyPaste 仍在启动。请稍后再试。';

  @override
  String get errorRuntimeTimeout => 'CopyPaste 未及时响应。请重试。';

  @override
  String get errorRuntimeMismatch => 'CopyPaste 已更新。请重新启动应用以完成。';

  @override
  String get errorPeerUnreachable => '无法连接到另一台设备。请确认它在线。';

  @override
  String get errorPeerVersion => '另一台设备上的 CopyPaste 版本不兼容。请更新两台设备。';

  @override
  String get errorRateLimited => '尝试次数过多。请稍候再试。';

  @override
  String devicesSyncFailedWith(String devices) {
    return '无法与 $devices 同步。';
  }

  @override
  String get transfersUnavailableTitle => '无法加载传输';

  @override
  String get historyTagsLoadFailedTitle => '无法加载标签';
}
