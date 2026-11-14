// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Japanese (`ja`).
class AppLocalizationsJa extends AppLocalizations {
  AppLocalizationsJa([String locale = 'ja']) : super(locale);

  @override
  String get commonCancel => 'キャンセル';

  @override
  String get commonClose => '閉じる';

  @override
  String get commonDone => '完了';

  @override
  String get commonRetry => '再試行';

  @override
  String get commonTryAgain => 'もう一度試す';

  @override
  String get commonCopy => 'コピー';

  @override
  String get commonDelete => '削除';

  @override
  String get commonSave => '保存';

  @override
  String get commonContinue => '続ける';

  @override
  String get commonBack => '戻る';

  @override
  String get commonOpenSettings => '設定を開く';

  @override
  String get commonLoading => '読み込み中';

  @override
  String get commonApply => '適用';

  @override
  String get commonClear => 'クリア';

  @override
  String get commonPin => 'ピン留め';

  @override
  String get commonEnable => '有効にする';

  @override
  String get settingsLanguageTitle => '言語';

  @override
  String get settingsLanguageDescription => 'CopyPaste のインターフェイスの言語を選択します。';

  @override
  String get settingsLanguageSystem => 'システムのデフォルト';

  @override
  String get settingsLanguageKeywords =>
      '言語 ロケール 翻訳 インターフェイス language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'ログイン時の起動を更新できませんでした。もう一度お試しください。';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed => 'システムの起動設定を開けませんでした。';

  @override
  String get settingsRuntimeRefreshFailed => '設定を再読み込みできませんでした。もう一度お試しください。';

  @override
  String get settingsRuntimeLoadFailed => '設定を読み込めませんでした。';

  @override
  String get settingsRuntimeAppAlreadyExcluded => 'そのアプリはすでに除外されています。';

  @override
  String get settingsRuntimeChangeFailed => '要求された変更を完了できませんでした。';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'クリップボードの記録を再開するには、必要な権限の設定を完了してください。';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'クリップボードの記録は一時停止中です。Android の記録サービスから停止の確認がありませんでした。';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      '記録の通知を有効にするには、通知の許可が必要です。';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste は保存された設定を読み取れませんでした。保護用の値が適用されています。設定を確認して保存し、修復してください。';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste は読み取れなかった設定を保護用の値で復旧しました。設定を確認して保存し、修復してください。';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'アプリ識別子は空にできず、256 バイト以下である必要があります。';

  @override
  String get settingsRuntimeExcludedAppsLimit => '除外できるアプリは 256 個までです。';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed => '自動ペーストを更新できませんでした。';

  @override
  String get settingsRuntimeShortcutRegisterFailed => 'そのショートカットを登録できませんでした。';

  @override
  String get settingsRuntimeShortcutPauseFailed => '現在のショートカットを一時停止できませんでした。';

  @override
  String get settingsRuntimeShortcutRestoreFailed => 'ショートカットを復元できませんでした。';

  @override
  String get settingsRuntimeQuickPasteOpenFailed => 'Quick Paste を開けませんでした。';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'アクセシビリティの許可を要求できませんでした。';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Linux 統合の状態を更新できませんでした。';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'クリップボード入力の許可が得られませんでした。';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'クリップボード入力の許可を要求できませんでした。';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'デスクトップ統合のセットアップを開けませんでした。';

  @override
  String get settingsRuntimeTextHistoryFileType => 'CopyPaste テキスト履歴';

  @override
  String get settingsRuntimeEncryptedBackupFileType => 'CopyPaste 暗号化バックアップ';

  @override
  String get settingsRuntimeNotificationImage => '画像';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return '画像 · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'ファイル';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste はアップデートのインストール状態を復元できませんでした。';

  @override
  String get updateLinuxAutomaticUnsupported => 'この Linux 環境では自動アップデートできません。';

  @override
  String get updateMacosPackageMissing =>
      'このリリースには署名済みの macOS アップデートパッケージがありません。';

  @override
  String get updateInstalledVersionInvalid =>
      'インストールされている CopyPaste のバージョンが無効です。';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste は GitHub に接続できず、アップデートを確認できませんでした。';

  @override
  String get updateCheckTimedOut => 'アップデートの確認がタイムアウトしました。';

  @override
  String get updateCheckDeviceFailed => 'CopyPaste はこのデバイスでアップデートを確認できませんでした。';

  @override
  String get updateCheckFailed => 'CopyPaste はアップデートを確認できませんでした。';

  @override
  String get updateDownloadFailed => 'CopyPaste はアップデートをダウンロードできませんでした。';

  @override
  String get updateDownloadTimedOut => 'アップデートのダウンロードがタイムアウトしました。';

  @override
  String get updateInstallFailed => 'CopyPaste はアップデートをインストールできませんでした。';

  @override
  String get updateContinueInAndroidInstaller =>
      'Android のシステムインストーラで続行してください。';

  @override
  String get updateContinueInPackageManager => 'システムのパッケージマネージャで続行してください。';

  @override
  String get updateInstallerStarting => 'インストーラを起動しています。';

  @override
  String get updatePermissionRequiredAndroid =>
      'Android の設定で CopyPaste にアプリのインストールを許可してから、続行してください。';

  @override
  String get updatePermissionRequiredLinux =>
      'このアップデートをインストールするには、システムのパッケージマネージャで認証してください。';

  @override
  String get updatePermissionRequired =>
      'CopyPaste にこのアップデートのインストールを許可してから、続行してください。';

  @override
  String get updateRestartRequired =>
      'アップデートがインストールされました。使用するには CopyPaste を再起動してください。';

  @override
  String get updateRestarting => 'CopyPaste を再起動しています。';

  @override
  String get updateRestartFailed => 'CopyPaste を再起動できませんでした。もう一度お試しください。';

  @override
  String get updateReleasePageOpenFailed => 'CopyPaste はリリースページを開けませんでした。';

  @override
  String get updatePackageInvalid =>
      'ダウンロードしたアップデートは有効な CopyPaste パッケージではありません。';

  @override
  String get updateSignatureInvalid => 'ダウンロードしたアップデートの署名が無効です。';

  @override
  String get updateDowngradeRefused => 'ダウンロードしたパッケージはこのバージョンより新しくありません。';

  @override
  String get updateHomebrewRequired =>
      'ここでアップデートするには、Homebrew で CopyPaste をインストールしてください。';

  @override
  String get updateInstallationCancelled => 'アップデートのインストールがキャンセルされました。';

  @override
  String get updateInstallationBlocked =>
      'Android によってアップデートのインストールがブロックされました。';

  @override
  String get updateInstallationConflict => 'アップデートがインストール済みのアプリと競合しています。';

  @override
  String get updateInstallationIncompatible => 'このアップデートはこのデバイスと互換性がありません。';

  @override
  String get updateInstallationStorage =>
      'アップデートをインストールするには、ストレージの空き容量を増やしてください。';

  @override
  String get updateInstallationInterrupted =>
      'アップデートのインストールが中断されました。もう一度お試しください。';

  @override
  String get updateInstallationBusy => 'すでに別のアップデートをインストール中です。';

  @override
  String get updatePackageDetailsInvalid => 'アップデートパッケージの情報が無効です。';

  @override
  String get updateIntegrityCheckFailed => 'ダウンロードしたアップデートの整合性チェックに失敗しました。';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste はシステムのインストーラを起動できませんでした。';

  @override
  String get updateInstallLocationTranslocated =>
      'アップデートするには、CopyPaste を「アプリケーション」フォルダに移動し、そこから開いてください。';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste は現在のフォルダで自身を置き換えられません。リリースページからアップデートしてください。';

  @override
  String get updateReplacementBlocked =>
      'macOS によって CopyPaste の置き換えがブロックされました。「アプリ管理」設定で許可するか、リリースページを使用してください。';

  @override
  String get updatePackageNotDownloadable =>
      'このプラットフォームでは、ダウンロード可能なアップデートパッケージを使用しません。';

  @override
  String get updatePackageSizeInvalid => 'アップデートパッケージのサイズが無効です。';

  @override
  String get updateUpdaterSignatureInvalid => 'ダウンロードしたアップデートのアップデータ署名が無効です。';

  @override
  String get updatePackageUrlUntrusted => 'アップデートパッケージの URL は信頼されていません。';

  @override
  String get updateDownloadLeftGitHub => 'アップデートのダウンロードが GitHub 外にリダイレクトされました。';

  @override
  String get updateDownloadTooManyRedirects => 'アップデートのダウンロードでリダイレクトが多すぎます。';

  @override
  String get updatePackageTooLarge => 'アップデートパッケージが大きすぎます。';

  @override
  String get updateSignatureSizeInvalid => 'アップデータ署名のサイズが無効です。';

  @override
  String get updateMetadataInvalid => 'GitHub から無効なアップデートメタデータが返されました。';

  @override
  String get updateMetadataTooLarge => 'アップデートメタデータが大きすぎます。';

  @override
  String get quickPasteActionFailedTitle => '操作に失敗しました';

  @override
  String get quickPasteOpenAppSemantics => 'CopyPaste を開く';

  @override
  String get quickPasteSearchPlaceholder => '入力して検索…';

  @override
  String get quickPasteLoadingHistory => 'クリップボード履歴を読み込んでいます。';

  @override
  String get quickPasteHistoryUnavailable => '履歴を利用できません';

  @override
  String get quickPasteNoClipsFound => 'クリップが見つかりません';

  @override
  String get quickPasteLoadMore => 'さらに読み込む';

  @override
  String get quickPasteTransferFailed => '失敗';

  @override
  String get quickPasteClipUnavailable => 'クリップを利用できません';

  @override
  String get quickPasteFileFallbackName => 'ファイル';

  @override
  String get quickPastePathUnavailable => 'パス不明';

  @override
  String get quickPastePreferences => '環境設定…';

  @override
  String get quickPasteAbout => 'CopyPaste について';

  @override
  String get quickPasteQuit => '終了';

  @override
  String get quickPasteClearHistoryTitle => '履歴を消去しますか？';

  @override
  String get quickPasteClearHistoryMessage => 'ピン留めしたクリップは履歴に残ります。';

  @override
  String get quickPasteClearHistoryFailed => '履歴を削除できませんでした。';

  @override
  String get quickPasteAboutDescription =>
      'macOS、Android、Windows、Linux で使えるプライベートなクリップボード履歴。';

  @override
  String get transfersTitle => 'ダウンロードと転送';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '転送中 $count 件',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件が進行中',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => '最新';

  @override
  String get transfersShowEarlier => '以前';

  @override
  String get transfersClearCompletedHistory => '完了した履歴を消去';

  @override
  String get transfersErrorTitle => '転送';

  @override
  String get transfersEmptyTitle => '転送はありません';

  @override
  String get transfersEmptyMessage => 'ファイルのインポート、ダウンロード、送信がここに表示されます。';

  @override
  String transfersSpeed(String size) {
    return '$size/秒';
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
  String get transfersRemoveFromHistory => '履歴から削除';

  @override
  String get transfersCancelDownload => 'ダウンロードをキャンセル';

  @override
  String get transfersShowFile => 'ファイルを表示';

  @override
  String get transfersPhaseSending => '送信中';

  @override
  String get transfersPhaseDownloading => 'ダウンロード中';

  @override
  String get transfersPhaseAdding => '追加中';

  @override
  String get transfersPhaseAdded => '追加済み';

  @override
  String get transfersPhaseSent => '送信済み';

  @override
  String get transfersPhaseCompleted => '完了';

  @override
  String get transfersPhaseCancelled => 'キャンセル済み';

  @override
  String get transfersPhaseInterrupted => '中断';

  @override
  String get transfersPhaseFailed => '失敗';

  @override
  String get transfersUpdatesUnavailable => '転送の更新を受信できません。開き直して更新してください。';

  @override
  String get transfersLoadFailed => '転送を読み込めませんでした。もう一度お試しください。';

  @override
  String get transfersUpdatesLoadFailed => '転送の最新状態を読み込めませんでした。';

  @override
  String get transfersRetryFailed => 'このファイルを再試行できませんでした。';

  @override
  String get transfersClearHistoryFailed => '転送履歴を消去できませんでした。';

  @override
  String get transfersRemoveRecordFailed => 'この履歴レコードを削除できませんでした。';

  @override
  String get transfersCancelFailed => '転送をキャンセルできませんでした。まだ実行中の可能性があります。';

  @override
  String get transfersSavedFileUnavailable =>
      '保存したファイルが見つかりません。移動または削除された可能性があります。';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '通知 · 未読 $count 件',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => '通知';

  @override
  String get notificationsClearRead => '既読の通知を消去';

  @override
  String get notificationsEmptyTitle => '通知はありません';

  @override
  String get notificationsEmptyMessage => 'デバイスの接続リクエストとその結果がここに表示されます。';

  @override
  String get notificationsSystemAlertTitle => 'システム通知';

  @override
  String get notificationsReview => '確認';

  @override
  String get notificationsConnectionRequest => '接続リクエスト';

  @override
  String get notificationsConfirmConnection => '接続を確認';

  @override
  String get notificationsDeviceConnected => 'デバイスが接続されました';

  @override
  String get notificationsConnectionRejected => '接続が拒否されました';

  @override
  String get notificationsConnectionCancelled => '接続がキャンセルされました';

  @override
  String get notificationsConnectionRequestExpired => '接続リクエストの期限が切れました';

  @override
  String get notificationsConnectionFailed => '接続に失敗しました';

  @override
  String get notificationsNearbyDevice => '近くのデバイス';

  @override
  String get notificationsPermissionDenied =>
      'バックグラウンドでリクエストを受け取るには、OS の設定でシステム通知を許可してください。';

  @override
  String get notificationsSystemUnavailable =>
      'システム通知を利用できません。接続リクエストは引き続きここに表示されます。';

  @override
  String get platformCaptureStateUnavailable => '利用不可';

  @override
  String get platformCaptureStatePaused => '一時停止中';

  @override
  String get platformCaptureStateRunning => '記録中';

  @override
  String get platformCaptureStateStopped => '停止中';

  @override
  String get platformCaptureActionUnavailable => '記録を利用できません';

  @override
  String get platformCaptureActionResume => '記録を再開';

  @override
  String get platformCaptureActionPause => '記録を一時停止';

  @override
  String get platformCaptureActionResumeStopped => '記録を再開（停止中）';

  @override
  String get platformTrayOpen => '開く';

  @override
  String get platformTraySettings => '設定';

  @override
  String get platformTrayQuit => '終了';

  @override
  String get platformWindowControlsUnavailable =>
      'デスクトップのウインドウ操作を利用できません。CopyPaste は表示されたままになります。';

  @override
  String get platformTrayUnavailable =>
      'システムトレイを利用できません。CopyPaste を開いたままにして、再試行するか終了してください。';

  @override
  String get platformQuitFailed => 'CopyPaste を終了できませんでした。もう一度お試しください。';

  @override
  String get platformVisibilityChangeFailed =>
      'アプリの表示状態を変更できませんでした。もう一度お試しください。';

  @override
  String get platformCaptureNotificationTitle => 'クリップボードを保存しました';

  @override
  String get platformCaptureNotificationBody => '新しい項目が CopyPaste に追加されました。';

  @override
  String get platformPairingNotificationTitle => '接続リクエスト';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName が接続を求めています。CopyPaste を開いてコードを照合し、確認してください。';
  }

  @override
  String get platformPairingNotificationChannelName => 'デバイスの接続リクエスト';

  @override
  String get platformPairingNotificationChannelDescription =>
      '近くのデバイスからの接続リクエスト';

  @override
  String get platformNotificationDefaultAction => '開く';

  @override
  String get platformFilesSelectionFailed => 'ファイルを選択できませんでした。';

  @override
  String get platformFilesOnlyRegular => 'インポートできるのは通常のファイルのみです。';

  @override
  String get platformModulesUnavailable => 'このデバイスではモジュールを利用できません。';

  @override
  String get platformSmsAccessConfigureFailed => 'SMS へのアクセスを設定できませんでした。';

  @override
  String get platformSmsAccessVerifyFailed => 'SMS へのアクセスを確認できませんでした。';

  @override
  String get platformUpdatesUnavailableHere => 'ここではアプリのアップデートを利用できません。';

  @override
  String get platformUpdateVersionUnavailable => 'インストールされているバージョンを取得できません。';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'このインストールではアプリのアップデートを利用できません。';

  @override
  String get platformUpdateLinuxUnsupported => 'この Linux 環境では自動アップデートできません。';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'この Linux アップデートは実行中のアーキテクチャと一致しません。';

  @override
  String get platformUpdateMacosUnsupported => 'この macOS 環境では自動アップデートできません。';

  @override
  String get platformUpdateStartFailed => 'CopyPaste はアップデートを開始できませんでした。';

  @override
  String get platformGlobalShortcutDescription => 'Quick Paste を開く';

  @override
  String get settingsSectionGeneral => '一般';

  @override
  String get settingsSectionGeneralSummary => '言語とアプリの表示。';

  @override
  String get settingsSectionClipboard => 'クリップボード';

  @override
  String get settingsSectionClipboardSummary => '記録と履歴の上限。';

  @override
  String get settingsSectionPrivacy => 'プライバシー';

  @override
  String get settingsSectionPrivacySummary => 'アプリの除外と画面の保護。';

  @override
  String get settingsSectionQuickPasteSummary => 'ショートカットと自動ペーストの動作。';

  @override
  String get settingsSectionSync => '同期';

  @override
  String get settingsSectionSyncSummary => '同期と近くのデバイスの検出。';

  @override
  String get settingsSectionNotifications => '通知';

  @override
  String get settingsSectionNotificationsSummary => '記録の通知とサウンド。';

  @override
  String get settingsSectionData => 'データ';

  @override
  String get settingsSectionDataSummary => 'エクスポート、バックアップ、復元。';

  @override
  String get settingsSectionModules => 'モジュール';

  @override
  String get settingsSectionModulesSummary => 'オプションモジュールのインストールと管理。';

  @override
  String get settingsSectionRules => 'コンテンツルール';

  @override
  String get settingsSectionRulesSummary => 'クリップを自動でタグ付け、スキップ、保護、期限切れにします。';

  @override
  String get settingsSectionQuickAccess => 'クイックアクセス';

  @override
  String get settingsSectionQuickAccessSummary => 'キーボード、タイル、ホーム画面ウィジェット。';

  @override
  String get settingsSectionSnippets => 'スニペット';

  @override
  String get settingsSectionSnippetsSummary => '再利用できるテキストとテンプレート。';

  @override
  String get settingsSectionTransforms => 'テキスト変換';

  @override
  String get settingsSectionTransformsSummary => 'お気に入り、パイプライン、貼り付け時の既定の変換。';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary => '複数のコピーを集めて順番に貼り付けます。';

  @override
  String pasteQueuePasted(int position, int total) {
    return '$total 件中 $position 件目を貼り付けました';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return '$total 件中 $position 件目をクリップボードに置きました';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件を貼り付けました',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件をクリップボードに置きました',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'キューが完了しました';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: '追加しました。キューに $total 件',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'キューが空のため、何も貼り付けていません';

  @override
  String get pasteQueueMixed => '「すべて貼り付け」はテキストのみ、またはファイルのみのキューで使えます';

  @override
  String get pasteQueueSeveralFiles => 'ファイルは 1 つずつ貼り付けてください。';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件は利用できなくなりました',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => '「次を貼り付け」はコピーのみ行います';

  @override
  String get pasteQueueAccessibilityMessage =>
      '項目をコピーしました。自動で貼り付けるにはアクセシビリティを許可してください。';

  @override
  String get pasteQueuePasteUnsupported => '自動で貼り付けできませんでした。代わりに項目をコピーしました。';

  @override
  String get pasteQueueTargetElevated =>
      '管理者として実行中のアプリには貼り付けできません。代わりに項目をコピーしました。';

  @override
  String get pasteQueueNoTarget => '貼り付け先のアプリがありません。代わりに項目をコピーしました。';

  @override
  String get pasteQueueCollectingStarted => 'コピーを収集中';

  @override
  String get pasteQueueCollectingStopped => '収集を停止しました';

  @override
  String get pasteQueueFailed => 'Paste Queue の操作を完了できませんでした';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '残り $count 件',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '残り $count 件 · 収集中',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => '収集中';

  @override
  String get pasteQueueFinishedSummary => '完了';

  @override
  String pasteQueueFull(int max) {
    return 'キューが満杯です（$max）';
  }

  @override
  String get pasteQueueFullMessage => '新しいコピーは追加されなくなりました。';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue は空です';

  @override
  String get pasteQueueEmptyMessage => 'コピーを収集するか、履歴から項目を追加します。';

  @override
  String get pasteQueueCollectingEmptyTitle => 'コピーすると追加されます';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      '収集を停止するまで、コピーした内容がキューに追加されます。';

  @override
  String get pasteQueueStartCollecting => '収集を開始';

  @override
  String get pasteQueueStopCollecting => '収集を停止';

  @override
  String get pasteQueueLoading => 'キューを読み込んでいます…';

  @override
  String get pasteQueueLoadFailed => 'キューを読み込めませんでした';

  @override
  String get pasteQueuePasteNext => '次を貼り付け';

  @override
  String get pasteQueueCopyNext => '次をコピー';

  @override
  String get pasteQueuePasteAll => 'すべて貼り付け';

  @override
  String get pasteQueueCopyAll => 'すべてコピー';

  @override
  String get pasteQueueReverse => '順序を反転';

  @override
  String get pasteQueueStepBack => '1 つ戻る';

  @override
  String get pasteQueueSkip => 'スキップ';

  @override
  String get pasteQueueRemovePasted => '貼り付け済みを削除';

  @override
  String get pasteQueueCleared => 'キューを消去しました';

  @override
  String get pasteQueueUndo => '元に戻す';

  @override
  String get pasteQueueStartFresh => '新しく始める';

  @override
  String get pasteQueueAppend => '追加する';

  @override
  String get pasteQueueRestartTitle => '新しい収集を開始しますか？';

  @override
  String get pasteQueueRestartMessage =>
      '「新しく始める」は貼り付け済みの項目を削除します。「追加する」は残します。';

  @override
  String pasteQueueStale(String time) {
    return '$time のキュー';
  }

  @override
  String get pasteQueueStatusPasted => '貼り付け済み';

  @override
  String get pasteQueueStatusSkipped => 'スキップ済み';

  @override
  String get pasteQueueStatusUnavailable => '利用できません';

  @override
  String get pasteQueueEdited => '編集済み';

  @override
  String get pasteQueueSecret => 'シークレット';

  @override
  String get pasteQueueKindImage => '画像';

  @override
  String get pasteQueueKindFiles => 'ファイル';

  @override
  String pasteQueueRenderedAt(String time) {
    return '$time に生成';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return '項目 $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return '$position 番目に移動しました';
  }

  @override
  String get pasteQueueMoveUp => '上へ移動';

  @override
  String get pasteQueueMoveDown => '下へ移動';

  @override
  String get pasteQueueMoveToTop => '先頭へ移動';

  @override
  String get pasteQueueMoveToBottom => '末尾へ移動';

  @override
  String get pasteQueueReorder => 'ドラッグして並べ替え';

  @override
  String get pasteQueueMoreActions => 'その他の操作';

  @override
  String get pasteQueuePasteNow => '今すぐ貼り付け';

  @override
  String get pasteQueueEdit => 'テキストを編集';

  @override
  String get pasteQueueUseOriginal => '元のテキストを使用';

  @override
  String get pasteQueueRemove => 'キューから削除';

  @override
  String get pasteQueueEditTitle => 'キューのテキストを編集';

  @override
  String get pasteQueueEditMessage => '変更はこのキューにのみ適用されます。';

  @override
  String get pasteQueueShortcutPromptTitle => 'ショートカットを設定';

  @override
  String get pasteQueueShortcutPromptMessage => 'CopyPaste を開かずに収集・貼り付けできます。';

  @override
  String get pasteQueueShortcutToggleCollecting => '収集の開始・停止';

  @override
  String get pasteQueueShortcutPasteNextPlain => '次をプレーンテキストで貼り付け';

  @override
  String get pasteQueueShortcutDescription => 'どのアプリでも使えるグローバルショートカットです。';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'この組み合わせは別の CopyPaste ショートカットで使われています。以前の設定を維持します。';

  @override
  String get pasteQueueShortcutInUse => '別のアプリがこの組み合わせを使用しています。';

  @override
  String get pasteQueueShortcutRejected => 'システムがこの組み合わせを拒否しました。以前の設定を維持します。';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt は多くの配列で文字入力に使われます。別の組み合わせを選んでください。';

  @override
  String get pasteQueueShortcutUnavailable => 'このセッションではグローバルショートカットを使用できません。';

  @override
  String get pasteQueueShortcutFailed => 'このショートカットを設定できませんでした。';

  @override
  String get pasteQueueEnableTitle => 'Paste Queue を有効にする';

  @override
  String get pasteQueueEnableDescription => 'オフにするとショートカットと収集が停止します。';

  @override
  String get pasteQueuePasteNextStyleTitle => '「次を貼り付け」の動作';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      '自動で貼り付けるか、クリップボードへのコピーのみ行います。';

  @override
  String get pasteQueuePasteIntoApp => '前面のアプリに貼り付け';

  @override
  String get pasteQueueClipboardOnly => 'クリップボードのみ';

  @override
  String get pasteQueueInsertTitle => '新しい項目の追加先';

  @override
  String get pasteQueueInsertDescription => '収集・追加した項目をキューのどこに入れるか。';

  @override
  String get pasteQueueInsertEnd => '末尾（先入れ先出し）';

  @override
  String get pasteQueueInsertStart => '先頭（後入れ先出し）';

  @override
  String get pasteQueueEndBehaviorTitle => '最後の項目の後';

  @override
  String get pasteQueueEndBehaviorDescription => 'すべて貼り付けた後に「次を貼り付け」が行う動作。';

  @override
  String get pasteQueueEndStop => '停止';

  @override
  String get pasteQueueEndLoop => '最初から繰り返す';

  @override
  String get pasteQueueEndKeepLast => '最後の項目を貼り付け続ける';

  @override
  String get pasteQueueSeparatorTitle => '「すべて貼り付け」の区切り';

  @override
  String get pasteQueueSeparatorDescription => 'まとめて貼り付けるときに項目の間に入れます。';

  @override
  String get pasteQueueSeparatorNewLine => '改行';

  @override
  String get pasteQueueSeparatorBlankLine => '空行';

  @override
  String get pasteQueueSeparatorTab => 'タブ';

  @override
  String get pasteQueueSeparatorSpace => 'スペース';

  @override
  String get pasteQueueSeparatorComma => 'カンマ';

  @override
  String get pasteQueueSeparatorCommaSpace => 'カンマとスペース';

  @override
  String get pasteQueueSeparatorNone => 'なし';

  @override
  String get pasteQueueSeparatorCustom => 'カスタム';

  @override
  String get pasteQueueCustomSeparatorPlaceholder => '32 文字まで。\\n と \\t を使用可';

  @override
  String get pasteQueuePlainTextTitle => '常にプレーンテキストで貼り付け';

  @override
  String get pasteQueuePlainTextDescription => 'キューの項目から書式を取り除きます。';

  @override
  String get pasteQueueIdleStopTitle => '収集を自動停止するまでの時間';

  @override
  String get pasteQueueIdleStopDescription => 'この時間コピーがないと収集を停止します。';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes 分';
  }

  @override
  String get pasteQueueIdleNever => '停止しない';

  @override
  String get pasteQueueMaxEntriesTitle => 'キューの上限';

  @override
  String get pasteQueueMaxEntriesDescription => 'いっぱいになると新しいコピーは追加されません。';

  @override
  String get pasteQueueRestoreTitle => '完了時に以前のクリップボードを復元';

  @override
  String get pasteQueueRestoreDescription => '貼り付け前にコピーしていた内容に戻します。';

  @override
  String get pasteQueueSettingsKeywords => 'paste queue キュー 収集 順番 ショートカット 区切り';

  @override
  String get settingsSectionAbout => '情報';

  @override
  String get settingsSectionAboutSummary => 'バージョンとアプリのアップデート。';

  @override
  String get settingsGeneralDescription => '言語、自動起動、アプリの表示。';

  @override
  String get settingsModulesDescription => 'CopyPaste のオプション機能。';

  @override
  String get settingsClipboardDescription => 'クリップボードの変更を記録し、履歴の上限を管理します。';

  @override
  String get settingsDataDescription => 'クリップボード履歴をエクスポート、バックアップ、復元します。';

  @override
  String get settingsSyncDescription => 'ペアリングしたデバイスとの同期を管理します。';

  @override
  String get settingsPrivacyDescription => '記録するアプリを管理し、クリップボードの内容を保護します。';

  @override
  String get settingsNotificationsDescription => '記録したクリップの通知とサウンドを選択します。';

  @override
  String get settingsAboutDescription => 'CopyPaste のバージョンとアプリのアップデート。';

  @override
  String get settingsQuickPasteDescription =>
      'ウインドウを切り替えずに、どこからでもクリップボード履歴を開けます。';

  @override
  String get settingsLoadingMessage => '設定を読み込んでいます。';

  @override
  String get settingsUnavailableTitle => '設定を利用できません';

  @override
  String get settingsNoResultsTitle => '設定が見つかりません';

  @override
  String get settingsNoResultsMessage => '別のキーワードで検索してください。';

  @override
  String get settingsAttentionTitle => '設定の確認が必要です';

  @override
  String get settingsSearchPlaceholder => '設定を検索';

  @override
  String get settingsBackToSettings => '設定に戻る';

  @override
  String get settingsStartOnBootTitle => 'デバイスの起動時に開始';

  @override
  String get settingsStartAtLoginTitle => 'ログイン時に起動';

  @override
  String get settingsStartupSearchDescription => 'CopyPaste を自動的に起動します。';

  @override
  String get settingsStartupKeywords =>
      '起動 ログイン ブート 自動起動 スタートアップ startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'タスク マネージャーで無効になっています。「設定」→「アプリ」→「スタートアップ」で CopyPaste を有効にしてください。';

  @override
  String get settingsStartupMacApproval =>
      '「システム設定」→「一般」→「ログイン項目」で CopyPaste を許可してください。';

  @override
  String get settingsStartupUnavailable => 'このインストールでは自動起動を利用できません。';

  @override
  String get settingsStartupAndroidDescription => '再起動後にバックグラウンドサービスを再開します。';

  @override
  String get settingsStartupDesktopDescription => 'ログイン時に CopyPaste を自動的に開きます。';

  @override
  String get settingsShowInDock => 'Dock に表示';

  @override
  String get settingsShowInTaskbar => 'タスクバーに表示';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'ウインドウ切り替え用のアプリアイコンを表示します。';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'Dock タスクバー アプリ アイコン 表示 dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'メニューバーに表示';

  @override
  String get settingsShowInSystemTray => 'システムトレイに表示';

  @override
  String get settingsTrayVisibilityDescription =>
      'CopyPaste と記録の操作にすばやくアクセスできるアイコンを表示します。';

  @override
  String get settingsTrayVisibilityKeywords =>
      'トレイ メニューバー ステータス アイコン 表示 tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle => '表示を変更できませんでした';

  @override
  String get settingsModulesSearchDescription =>
      'オプションのモジュールをインストール、アップデート、削除します。';

  @override
  String get settingsModulesKeywords =>
      '機能 コマンド 拡張機能 features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Android のバックグラウンド記録';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      '「フル」または「制限付き」モード（Shizuku または ADB で設定）。';

  @override
  String get settingsAndroidCaptureKeywords =>
      'バックグラウンド 権限 セットアップ background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'フル／制限付きモードと、Shizuku または ADB の初回設定。';

  @override
  String get settingsOpenSetup => 'セットアップを開く';

  @override
  String get settingsSaveScreenshotsTitle => 'スクリーンショットを保存';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      '新しい Android のスクリーンショットを自動的に履歴に保存します。';

  @override
  String get settingsSaveScreenshotsKeywords =>
      '画像 写真 記録 権限 images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription => '新しいスクリーンショットを自動的に履歴に追加します。';

  @override
  String get settingsScreenshotAccessTitle => 'スクリーンショットへのアクセス';

  @override
  String get settingsScreenshotAccessDescription =>
      'バックグラウンドでスクリーンショットを保存するために必要です。';

  @override
  String get settingsAllowAccess => 'アクセスを許可';

  @override
  String get settingsScreenshotCaptureStoppedTitle => 'スクリーンショットの記録が停止しています';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      '新しいスクリーンショットは保存されていません。記録を再開してサービスを再起動してください。';

  @override
  String get settingsResumeCapture => '記録を再開';

  @override
  String get settingsClipboardCaptureTitle => 'クリップボードの記録';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'クリップボードの記録を一時停止または再開します。';

  @override
  String get settingsCaptureStateUnavailable => '状態を取得できません';

  @override
  String get settingsCaptureStoppedTitle => '記録が停止しています';

  @override
  String get settingsCaptureStoppedMessage =>
      'CopyPaste は現在、このデバイスでクリップボードの変更を受信していません。';

  @override
  String get settingsRetentionTitle => '保持期間';

  @override
  String get settingsRetentionDescription =>
      'ピン留めされていない古いクリップボードの項目を自動的に削除します。';

  @override
  String get settingsRetentionForever => '無期限に保持';

  @override
  String get settingsStorageQuotaTitle => 'ストレージの上限';

  @override
  String get settingsStorageQuotaDescription =>
      'ピン留めされていない履歴が使用するローカルストレージの最大容量。';

  @override
  String get settingsStorageQuotaKeywords => 'ディスク 容量 上限 disk space limit';

  @override
  String get settingsMaxFileSizeTitle => '最大ファイルサイズ';

  @override
  String get settingsMaxFileSizeSearchDescription => 'ファイルの追加とダウンロードの上限。';

  @override
  String get settingsMaxFileSizeKeywords =>
      'インポート ダウンロード 転送 サイズ 上限 import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription => '追加・ダウンロードできるファイルの最大サイズです。';

  @override
  String get settingsHistoryFilesTitle => '履歴ファイル';

  @override
  String get settingsHistoryFilesSearchDescription =>
      '履歴のエクスポート、バックアップの作成、バックアップの復元。';

  @override
  String get settingsHistoryFilesKeywords =>
      'テキスト 暗号化 バックアップ データ text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'テキスト履歴をエクスポート';

  @override
  String get settingsExportTextDescription => 'テキストのクリップを汎用的なファイルに保存します。';

  @override
  String get settingsExportAction => 'エクスポート';

  @override
  String get settingsBackupTitle => '暗号化バックアップ';

  @override
  String get settingsBackupDescription => 'このデバイスのローカル履歴全体を保存します。';

  @override
  String get settingsCreateBackupAction => 'バックアップを作成';

  @override
  String get settingsRestoreTitle => 'バックアップを復元';

  @override
  String get settingsRestoreDescription => 'ローカル履歴を暗号化バックアップで置き換えます。';

  @override
  String get settingsRestoreAction => '復元';

  @override
  String get settingsRestoreDialogTitle => 'ローカル履歴を置き換えますか？';

  @override
  String get settingsRestoreDialogMessage =>
      'バックアップでこのデバイスの履歴を置き換えます。設定は保持されます。';

  @override
  String get settingsRestoreFailed => 'バックアップを復元できませんでした。';

  @override
  String get settingsSyncSwitchTitle => '同期';

  @override
  String get settingsSyncSwitchDescription => 'ペアリングしたデバイスと履歴を同期します。';

  @override
  String get settingsSyncSearchDescription => 'ペアリングしたデバイスとの同期。';

  @override
  String get settingsInstantClipboardTitle => 'インスタントクリップボード';

  @override
  String get settingsInstantClipboardSearchDescription =>
      '他のデバイスからの新しいクリップを自動的にコピーします。';

  @override
  String get settingsInstantClipboardKeywords =>
      '自動 コピー ペースト 同期 受信 automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      '他のデバイスの新しいクリップをこのクリップボードにコピーします。';

  @override
  String get settingsLanVisibilityTitle => 'LAN での可視性';

  @override
  String get settingsLanVisibilityDescription => '近くのデバイスがこのデバイスを検出できるようにします。';

  @override
  String get settingsLanVisibilityKeywords =>
      'ローカル ネットワーク 検出 local network discovery';

  @override
  String get settingsExcludedAppsTitle => '除外するアプリ';

  @override
  String get settingsExcludedAppsSearchDescription => '除外したアプリからの自動記録をスキップします。';

  @override
  String get settingsExcludedAppsKeywords =>
      'プライバシー アプリ 識別子 privacy app identifiers';

  @override
  String get settingsExcludedAppsDesktopDescription => 'これらのアプリからは自動記録しません。';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      '除外したアプリがある間は自動記録を一時停止します。';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'アプリの除外は macOS、Windows、Android、Linux で利用できます。';

  @override
  String get settingsExcludedAppsAdd => '追加';

  @override
  String get settingsExcludedAppsEmpty => '除外されているアプリはありません。';

  @override
  String settingsExcludedAppsRemove(String app) {
    return '$app を削除';
  }

  @override
  String get applicationPickerSearch => 'アプリを検索';

  @override
  String get applicationPickerEmpty => 'アプリが見つかりません';

  @override
  String get settingsSkipSecretTitle => '機密のクリップボードをスキップ';

  @override
  String get settingsSkipSecretDescription => 'アプリが機密と指定したコピーを保存しません。';

  @override
  String get settingsSkipSecretKeywords =>
      'シークレット パスワード 機密 非表示 マスク secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => '一時的なクリップボードをスキップ';

  @override
  String get settingsSkipTransientDescription => 'アプリが一時的と指定したコピーを保存しません。';

  @override
  String get settingsSkipTransientKeywords =>
      '一時的 一時 プライバシー transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'スクリーンショットをブロック';

  @override
  String get settingsBlockScreenshotsDescription =>
      'CopyPaste のスクリーンショットと画面収録を防止します。';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'このプラットフォームではスクリーンショットのブロックを利用できません。';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'プライバシー 画面 キャプチャ 保護 ペアリング QR セキュリティ コード privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'コピー時に通知';

  @override
  String get settingsNotifyOnCopyDescription => 'バックグラウンドで記録したあとに通知を表示します。';

  @override
  String get settingsNotificationPreviewTitle => 'クリップボードの内容を表示';

  @override
  String get settingsNotificationPreviewDescription =>
      'コピーの通知にクリップのプレビューを含めます。';

  @override
  String get settingsNotificationPreviewKeywords =>
      '通知 プレビュー テキスト 画像 プライバシー notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'コピー時にサウンド';

  @override
  String get settingsSoundOnCopyDescription =>
      '記録に成功したときにプラットフォームのフィードバックを再生します。';

  @override
  String get settingsVersionTitle => 'バージョン';

  @override
  String get settingsVersionDescription => 'インストールされている CopyPaste のバージョン。';

  @override
  String get settingsVersionUnavailable => '不明';

  @override
  String get settingsUpdatesTitle => 'アプリのアップデート';

  @override
  String get settingsUpdatesSearchDescription =>
      'CopyPaste のアップデートを確認してインストールします。';

  @override
  String get settingsUpdatesKeywords =>
      'バージョン GitHub リリース version github release';

  @override
  String get settingsUpdateIdle => 'CopyPaste の新しいバージョンを確認します。';

  @override
  String get settingsUpdateChecking => 'アップデートを確認しています。';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste は最新です。';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'バージョン $version は最新です。';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'バージョン $available を利用できます。このデバイスのバージョンは $current です。';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'バージョン $version をダウンロード中 · $percent%。';
  }

  @override
  String get settingsUpdatePreparingModules => 'インストール済みモジュールのアップデートを準備しています。';

  @override
  String settingsUpdateInstalling(String version) {
    return 'バージョン $version をインストールしています。';
  }

  @override
  String get settingsUpdateNeedsAttention => 'アプリのアップデートに対応が必要です。';

  @override
  String get settingsUpdateNowAction => '今すぐアップデート';

  @override
  String get settingsUpdateOpenReleaseAction => 'リリースを開く';

  @override
  String get settingsUpdateCheckingAction => '確認中';

  @override
  String get settingsUpdateDownloadingAction => 'ダウンロード中';

  @override
  String get settingsUpdateInstallingAction => 'インストール中';

  @override
  String get settingsUpdateRestartAction => 'CopyPaste を再起動';

  @override
  String get settingsUpdateCheckAgainAction => '再確認';

  @override
  String get settingsUpdateNoneTitle => 'アップデートはありません';

  @override
  String get settingsUpdateLatest => 'CopyPaste は最新バージョンです。';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version は最新バージョンです。';
  }

  @override
  String get settingsUpdateAvailableTitle => 'アップデートがあります';

  @override
  String get settingsUpdateReady => 'CopyPaste の新しいバージョンをインストールする準備ができました。';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version をインストールする準備ができました。';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'アップデートを利用できません';

  @override
  String get settingsUpdateUnavailableMessage => 'このインストールでは自動アップデートできません。';

  @override
  String get settingsUpdateCheckFailedTitle => 'アップデートの確認に失敗しました';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'CopyPaste はアップデートを確認できませんでした。';

  @override
  String get settingsQuickPasteSetupTitle => 'Quick Paste をセットアップ';

  @override
  String get settingsQuickPasteSetupMessage =>
      '署名済みの GNOME または KDE コンパニオンを有効にし、前のアプリに戻れるようキーボード操作を許可してください。';

  @override
  String get settingsQuickPasteDesktopIntegration => 'デスクトップ統合';

  @override
  String get settingsQuickPasteAllowKeyboard => 'キーボード操作を許可';

  @override
  String get settingsQuickPasteCheckAgain => '再確認';

  @override
  String get settingsQuickPasteLoading => 'Quick Paste の設定を読み込んでいます。';

  @override
  String get settingsQuickPasteUnavailableTitle => 'Quick Paste を利用できません';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'このデバイスには Quick Paste ウインドウがありません。';

  @override
  String get settingsQuickPasteShortcutTitle => 'Quick Paste を開く';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Quick Paste のグローバルショートカットを設定します。';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'キーボード ホットキー ショートカット keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'CopyPaste がバックグラウンドで実行中に使えます。';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return '登録済みのショートカット：$shortcut';
  }

  @override
  String get settingsAutoPasteTitle => '自動的にペースト';

  @override
  String get settingsAutoPasteSearchDescription => '選択したクリップを前のアプリにペーストします。';

  @override
  String get settingsAutoPasteKeywords => 'アクセシビリティ 自動 accessibility automatic';

  @override
  String get settingsAutoPasteDescription => 'クリップを選択すると、前のアプリに戻ってペーストします。';

  @override
  String get settingsAccessibilityRequiredTitle => 'アクセシビリティの許可が必要です';

  @override
  String get settingsAccessibilityRequiredMessage =>
      '許可されるまでは、クリップを選択するとペーストせずにコピーのみ行います。';

  @override
  String get settingsQuickPasteAttentionTitle => 'Quick Paste の確認が必要です';

  @override
  String get settingsShortcutRecording => 'ショートカットを押してください…';

  @override
  String get settingsShortcutNotSet => '未設定';

  @override
  String get settingsShortcutReset => 'ショートカットをリセット';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 日',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'デバイスを読み込んでいます…';

  @override
  String get devicesUnavailableTitle => 'デバイスを利用できません';

  @override
  String get devicesNearbyTitle => '近くのデバイス';

  @override
  String get devicesScan => 'スキャン';

  @override
  String get devicesScanning => 'スキャン中…';

  @override
  String get devicesNoneNearby => '近くにデバイスがありません';

  @override
  String get devicesThisDevice => 'このデバイス';

  @override
  String get devicesTrusted => '信頼済み';

  @override
  String get devicesConnect => '接続';

  @override
  String devicesCardPing(String ping) {
    return 'Ping: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Ping:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'このデバイスの名前を変更';

  @override
  String get devicesNamePlaceholder => 'デバイス名';

  @override
  String get devicesRenameFailed => 'デバイス名を保存できませんでした。';

  @override
  String devicesRevokeDialogTitle(String device) {
    return '$device のペアリングを取り消しますか？';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return '$device とのペアリングを解除しますか？';
  }

  @override
  String get devicesRevokeDialogMessage =>
      'このデバイスをグループから削除し、このペアリングを永久にブロックします。他のデバイスには再接続時に削除が反映されます。';

  @override
  String get devicesUnpairDialogMessage =>
      'このデバイスをグループから削除します。他のデバイスには再接続時に削除が反映されます。再度追加するには、新たに確認が必要です。';

  @override
  String get devicesRevoke => '取り消す';

  @override
  String get devicesUnpair => 'ペアリング解除';

  @override
  String get devicesActionFailedMessage => 'デバイスの操作を完了できませんでした。';

  @override
  String get devicesCloseDetails => 'デバイスの詳細を閉じる';

  @override
  String get devicesDetailType => 'デバイスの種類';

  @override
  String get devicesDetailOperatingSystem => 'オペレーティングシステム';

  @override
  String get devicesDetailModel => 'モデル';

  @override
  String get devicesDetailAppVersion => 'CopyPaste のバージョン';

  @override
  String get devicesDetailProtocolVersion => 'プロトコルのバージョン';

  @override
  String get devicesDetailStatus => '状態';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => '測定日時';

  @override
  String get devicesDetailLastSeen => '最終確認日時';

  @override
  String get devicesDetailLanEndpoint => 'LAN エンドポイント';

  @override
  String get devicesDetailDeviceId => 'デバイス ID';

  @override
  String get devicesDetailPairingId => 'ペアリング ID';

  @override
  String get devicesDetailProfileProvenance => 'プロファイルの出所';

  @override
  String get devicesDetailProfileTrust => 'プロファイルの信頼度';

  @override
  String get devicesDetailProfileUpdated => 'プロファイルの更新日時';

  @override
  String get devicesValueUnavailable => '不明';

  @override
  String get devicesRenameDevice => 'デバイス名を変更';

  @override
  String get devicesSyncNow => '今すぐ同期';

  @override
  String get devicesRevokePairing => 'ペアリングを取り消す';

  @override
  String get devicesLocal => 'ローカル';

  @override
  String get devicesTrustAuthenticated => '認証済み';

  @override
  String get devicesTrustUnverified => '未検証';

  @override
  String get devicesProvenanceSelfReported => '自己申告';

  @override
  String get devicesProvenanceObserved => '観測';

  @override
  String get devicesProvenanceMeasured => '測定';

  @override
  String get devicesConnectionOnline => 'オンライン';

  @override
  String get devicesConnectionOffline => 'オフライン';

  @override
  String get devicesConnectionVisible => '検出済み';

  @override
  String get devicesConnectionNotSeen => '未検出';

  @override
  String get devicesConnectionUnknown => '不明';

  @override
  String get devicesPeerAvailable => '利用可能';

  @override
  String get devicesPeerVisibleNow => '現在検出済み';

  @override
  String get devicesPeerStatusUnknown => '状態不明';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => 'デスクトップ';

  @override
  String get devicesClassLaptop => 'ノートパソコン';

  @override
  String get devicesClassPhone => 'スマートフォン';

  @override
  String get devicesClassTablet => 'タブレット';

  @override
  String get devicesClassUnknown => 'デバイス';

  @override
  String get devicesUnknownOs => '不明な OS';

  @override
  String get devicesAddDevice => 'デバイスを追加';

  @override
  String get devicesScanQr => 'QR をスキャン';

  @override
  String get devicesEnterCode => 'コードを入力';

  @override
  String get devicesScannerUnavailableTitle => 'スキャナを利用できません';

  @override
  String get devicesActionFailedTitle => '操作に失敗しました';

  @override
  String get devicesRefreshFailedTitle => 'デバイスの更新に失敗しました';

  @override
  String get devicesNearbyPairingUnavailable => '近くのデバイスとのペアリングは利用できません。';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      '近くのデバイスとのペアリングは利用できません。「デバイスを追加」でペアリングコードを表示してください。';

  @override
  String get pairingTitle => 'ペアリング';

  @override
  String get pairingPairDevice => 'デバイスをペアリング';

  @override
  String get pairingScanQrCode => 'QR コードをスキャン';

  @override
  String get pairingEnterPairingCode => 'ペアリングコードを入力';

  @override
  String get pairingConnectNearbyDevice => '近くのデバイスを接続';

  @override
  String get pairingSendingRequest => '接続リクエストを送信しています…';

  @override
  String get pairingEmptyTitle => 'ペアリング方法を選択';

  @override
  String get pairingEmptyMessage => 'ペアリングの操作がここに表示されます。';

  @override
  String get pairingCreatingInvitation => 'ペアリングの招待を作成しています。';

  @override
  String get pairingCode => 'ペアリングコード';

  @override
  String get pairingCodeHint => 'もう一方のデバイスに表示されている 8 文字のコードを入力してください。';

  @override
  String get pairingDeviceAddress => 'デバイスのアドレス';

  @override
  String get pairingDeviceAddressHint =>
      'もう一方のデバイスに表示された host:port 形式のアドレスを使用してください。';

  @override
  String get pairingCheckDetailsTitle => 'ペアリング情報を確認してください';

  @override
  String get pairingActionFailed => 'ペアリング操作に失敗しました';

  @override
  String get pairingConnectingDevice => 'デバイスを接続しています…';

  @override
  String get pairingQrScannerTitle => 'QR スキャナ';

  @override
  String get pairingCameraPreviewPlaceholder => 'カメラのプレビューがここに表示されます。';

  @override
  String get pairingScannerNeedsAttention => 'スキャナの確認が必要です';

  @override
  String get pairingCodeIncomplete => '8 文字のペアリングコードをすべて入力してください。';

  @override
  String get pairingAddressMissing => 'デバイスの host:port アドレスを入力してください。';

  @override
  String get pairingWaitingForOtherDevice => 'もう一方のデバイスを待っています';

  @override
  String get pairingInviteInstructions =>
      'もう一方のデバイスで「デバイスを追加」→「コードを入力」を選択するか、QR コードをスキャンしてください。';

  @override
  String get pairingDetailsTitle => 'ペアリング情報';

  @override
  String get pairingAddress => 'アドレス';

  @override
  String get pairingVerificationCode => '確認コード';

  @override
  String get pairingVerificationCodeHint => '両方のデバイスでこのコードが一致する場合のみ確認してください。';

  @override
  String get pairingReject => '拒否';

  @override
  String get pairingConfirm => '確認';

  @override
  String get pairingAccept => '承認';

  @override
  String get pairingJoinDevice => 'デバイスに参加';

  @override
  String get pairingStartScanner => 'スキャナを起動';

  @override
  String get pairingActionNotStarted => 'ペアリング操作を開始できませんでした。';

  @override
  String get pairingStateHandshaking => 'デバイスを検証中';

  @override
  String get pairingStateAwaitingConfirmation => '両方のデバイスで確認してください';

  @override
  String get pairingStateConfirmed => 'デバイスをペアリングしました';

  @override
  String get pairingStateRejected => 'ペアリングが拒否されました';

  @override
  String get pairingStateCancelled => 'ペアリングがキャンセルされました';

  @override
  String get pairingStateTimedOut => 'ペアリングがタイムアウトしました';

  @override
  String get pairingStateFailed => 'ペアリングに失敗しました';

  @override
  String get pairingHandshakingDescription => 'デバイス間で安全なチャネルを確立しています。';

  @override
  String get pairingAwaitingConfirmationDescription =>
      '保護された確認コードを照合してから、両方のデバイスで確認してください。';

  @override
  String get pairingConfirmedDescription =>
      'デバイスが接続されました。グループのメンバー情報は自動的に同期されます。';

  @override
  String get pairingNoDeviceAdded => 'デバイスは追加されませんでした。';

  @override
  String get pairingTimedOutDescription => '続けるには、新しくペアリングを開始してください。';

  @override
  String get pairingIdleDescription => 'デバイスを接続するには、ペアリングを開始してください。';

  @override
  String get pairingProtectedUnavailableTitle => '保護されたペアリングを利用できません';

  @override
  String get pairingProtectedUnavailableMessage =>
      'このペアリングセッションは、専用の保護されたウインドウで開く必要があります。';

  @override
  String get pairingCancel => 'ペアリングをキャンセル';

  @override
  String get pairingOrEnterCode => 'またはペアリングコードを入力';

  @override
  String get pairingJoinSecurely => '安全に参加';

  @override
  String get pairingProtectedWaitingDescription =>
      'この保護されたウインドウで、QR コードを表示するか、QR コードをスキャンするか、ペアリングコードを入力してください。';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return '残り $seconds 秒';
  }

  @override
  String get pairingInvalidQr => 'CopyPaste のペアリング用 QR コードをスキャンしてください。';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Google スキャナを利用できません。代わりにペアリングコードを入力してください。';

  @override
  String get pairingScannerCouldNotOpen =>
      'スキャナを開けませんでした。代わりにペアリングコードを入力してください。';

  @override
  String get pairingSecurePresentationUnavailable => 'ペアリング画面を安全に表示できません。';

  @override
  String get pairingVerificationCodeRequired => '先に確認コードを表示して照合してください。';

  @override
  String get syncStatusUnavailable => '同期の状態を取得できません';

  @override
  String get syncStatusOff => '同期オフ';

  @override
  String get syncStatusWaitingForSync => '同期を待機中';

  @override
  String get syncStatusSyncing => '同期中';

  @override
  String get syncStatusUpToDate => '最新';

  @override
  String get syncStatusInterrupted => '同期が中断されました';

  @override
  String get syncStatusPartiallySynchronized => '一部のみ同期済み';

  @override
  String get syncStatusRetrying => '再試行中';

  @override
  String get syncStatusWaitingToSync => '同期待ち';

  @override
  String get syncStatusWaitingForDevice => 'デバイスを待機中';

  @override
  String get syncStatusNoPairedDevices => 'ペアリングしたデバイスはありません';

  @override
  String get syncStatusDevicesOffline => 'デバイスがオフライン';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · アクティビティログを利用できません';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '同期オフ · 問題 $count 件',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · オフライン $count 台',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 台のデバイスで対応が必要です',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'デバイスがオフライン · 問題 $count 件',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => '接続を確認中';

  @override
  String get syncConnectionUnknown => '接続状態不明';

  @override
  String get syncLogDeviceOffline => 'デバイスがオフラインになりました。';

  @override
  String get syncLogConnectionRestored => '接続が回復しました。';

  @override
  String get syncLogProblemDetected => '同期の問題が検出されました。';

  @override
  String get syncLogProblemResolved => '同期の問題が解決しました。';

  @override
  String get syncLogFailed => '同期に失敗しました。';

  @override
  String get syncLogRecovered => '同期が回復しました。';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent 件のクリップ',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received 件のクリップ',
    );
    return '$_temp0を送信 · $_temp1を受信';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return '同期：$status';
  }

  @override
  String get syncDetailsTitle => '同期';

  @override
  String get syncDetailsClose => '同期の詳細を閉じる';

  @override
  String get syncNoPairedDevicesMessage => 'クリップボード履歴を同期するには、デバイスをペアリングしてください。';

  @override
  String get syncActivityLogUnavailableTitle => 'アクティビティログを利用できません';

  @override
  String get syncActivityLogUnavailableMessage =>
      '同期のアクティビティを読み取りまたは保存できませんでした。';

  @override
  String get syncDetailsDevicesHeading => 'デバイス';

  @override
  String get syncDetailsActivityHeading => 'アクティビティ';

  @override
  String get syncDetailsNoRecentProblems => '最近の同期の問題はありません';

  @override
  String get syncDetailsUnknownPeer => 'ペアリングしたデバイス';

  @override
  String get syncPeerConnection => '接続';

  @override
  String get syncPeerLastCheckStarted => '前回の確認開始日時';

  @override
  String get syncPeerItemsOverSizeLimit => 'サイズ上限を超える項目';

  @override
  String syncPeerLastSeen(String time) {
    return '最終確認：$time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return '最終同期：$time';
  }

  @override
  String get syncPeerLastSuccessfulSync => '前回の同期成功日時';

  @override
  String get syncPeerConnectionDetails => '接続の詳細';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return '$device の接続の詳細';
  }

  @override
  String get historyKindText => 'テキスト';

  @override
  String get historyKindLink => 'リンク';

  @override
  String get historyKindEmail => 'メール';

  @override
  String get historyKindColor => 'カラー';

  @override
  String get historyKindPhone => '電話番号';

  @override
  String get historyKindCode => 'コード';

  @override
  String get historyKindPath => 'パス';

  @override
  String get historyKindCredentials => '認証情報';

  @override
  String get historyKindImage => '画像';

  @override
  String get historyKindFile => 'ファイル';

  @override
  String get historyKindOther => 'その他';

  @override
  String get historySortNewest => '新しい順';

  @override
  String get historySortOldest => '古い順';

  @override
  String get historySortRelevance => '関連度順';

  @override
  String get historyErrorUpdatesUnavailable => '履歴の更新を一時的に受信できません。';

  @override
  String get historyErrorBulkDelete => '選択したクリップの一部を削除できませんでした。もう一度お試しください。';

  @override
  String get historyErrorBulkPin => '選択したクリップの一部でピン留めを更新できませんでした。もう一度お試しください。';

  @override
  String get historyErrorRefresh => '履歴を更新できませんでした。もう一度お試しください。';

  @override
  String get historyErrorLoad => '履歴を読み込めませんでした。もう一度お試しください。';

  @override
  String get historyErrorLoadMore => '続きの履歴を読み込めませんでした。';

  @override
  String get historyErrorLoadEarlier => '以前の履歴を読み込めませんでした。';

  @override
  String get historyErrorOpenClip => 'このクリップを開けませんでした。';

  @override
  String get historyErrorCopyPlainText => 'プレーンテキストをコピーできませんでした。';

  @override
  String get historyErrorCopyClip => 'このクリップをコピーできませんでした。';

  @override
  String get historyErrorCopyText => 'このテキストをコピーできませんでした。';

  @override
  String get historyErrorPrepareFile =>
      'このファイルをダウンロードできませんでした。コピー元のデバイスを確認して、もう一度お試しください。';

  @override
  String get historyErrorDownloadFile => 'このファイルをダウンロードできませんでした。';

  @override
  String get historyErrorSelectFiles => 'ファイルを選択できませんでした。もう一度お試しください。';

  @override
  String get historyErrorReadSelectedFile =>
      '選択したファイルを読み取れませんでした。アクセス権と空き容量を確認してください。';

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
      other: '$total 件中 $failed 件のファイルをインポートできませんでした。最初のファイル：$name。$reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate => 'クリップの機密設定を更新できませんでした。';

  @override
  String get historyErrorPinUpdate => 'ピン留めを更新できませんでした。';

  @override
  String get historyErrorPinnedOrderSave => 'ピン留めの順序を保存できませんでした。もう一度お試しください。';

  @override
  String get historyErrorPinnedOrderRefresh => '順序は保存されましたが、履歴を更新できませんでした。';

  @override
  String get historyErrorDeleteClip => 'このクリップを削除できませんでした。';

  @override
  String get historyErrorDeleteAll => '履歴を削除できませんでした。';

  @override
  String get historyErrorBulkDeleteFallback => '選択したクリップを削除できませんでした。';

  @override
  String get historyOcrErrorUnavailable =>
      'OCR を利用できません。設定でモジュールを確認して、もう一度お試しください。';

  @override
  String get historyOcrErrorStart => 'OCR を開始できませんでした。もう一度お試しください。';

  @override
  String get historyOcrErrorRecognize => '画像内のテキストを認識できませんでした。もう一度お試しください。';

  @override
  String get historyOcrRecognizing => '画像内のテキストを認識しています。';

  @override
  String get historyOcrFailedTitle => 'OCR に失敗しました';

  @override
  String get historyOcrNoText => 'テキストが見つかりません';

  @override
  String get historyOcrTitle => '画像内のテキスト';

  @override
  String get historyOcrTextCopied => '認識したテキストをコピーしました。';

  @override
  String get historyOcrCopyError => 'テキストをコピーできませんでした。もう一度お試しください。';

  @override
  String get historyTagsErrorLoad => 'タグを読み込めませんでした。もう一度お試しください。';

  @override
  String get historyTagsErrorSave =>
      'タグの変更を保存できませんでした。64 文字以内の重複しない名前を使用し、ピン留めしたクリップを選択してください。';

  @override
  String get historyTags => 'タグ';

  @override
  String get historyTagsUpdateErrorTitle => 'タグを更新できませんでした';

  @override
  String get historyTagsEmpty => 'タグはまだありません';

  @override
  String get historyTagRename => '名前を変更';

  @override
  String get historyTagActions => 'タグの操作';

  @override
  String get historyTagNamePlaceholder => 'タグ名';

  @override
  String get historyCreateTag => 'タグを作成';

  @override
  String get historySaveTag => 'タグを保存';

  @override
  String get historyTagsFilterAll => 'すべてのタグ';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 個のタグ',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'タグで絞り込む：$selection';
  }

  @override
  String get historyManageTags => 'タグを管理';

  @override
  String get historyTagsFilterSemantics => 'ピン留めしたクリップをタグで絞り込む';

  @override
  String get historyDownloadErrorCancelled => 'ファイルのダウンロードがキャンセルされました。';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'ダウンロードサービスを利用できません。CopyPaste を開き直して、もう一度お試しください。';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'ダウンロードサービスに接続できませんでした。もう一度お試しください。';

  @override
  String get historyFileErrorAccess => '選択したファイルにアクセスできませんでした。';

  @override
  String get historySpoilerRevealSemantics => '機密コンテンツ。マスクを外して表示';

  @override
  String get historyActionFailedTitle => '操作に失敗しました';

  @override
  String get historyLoadingMessage => 'クリップボード履歴を読み込んでいます。';

  @override
  String get historyUnavailableTitle => '履歴を利用できません';

  @override
  String get historyEmptyTitle => 'クリップが見つかりません';

  @override
  String get historyEmptyMessage => '記録したクリップや検索に一致する履歴がここに表示されます。';

  @override
  String get historyAttentionTitle => '履歴の確認が必要です';

  @override
  String get historyLoadMore => 'さらに読み込む';

  @override
  String get historySectionPinned => 'ピン留め';

  @override
  String get historySectionToday => '今日';

  @override
  String get historySectionYesterday => '昨日';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'クリップボードの行 $count 件をスキップしました',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      '保持されている一部のクリップを読み取れなかったため、この一覧には表示されません。';

  @override
  String get historySearch => '履歴を検索';

  @override
  String get historyResetFilters => 'フィルタをリセット';

  @override
  String get historyImportFiles => 'ファイルをインポート';

  @override
  String get historySelectClips => 'クリップを選択';

  @override
  String get historyCloseSearch => '検索を閉じる';

  @override
  String get historyFilterAllClips => 'すべてのクリップ';

  @override
  String get historyFilterAllDevices => 'すべてのデバイス';

  @override
  String get historyFilterAllApps => 'すべてのアプリ';

  @override
  String get historyPinClip => 'クリップをピン留め';

  @override
  String get historyUnpinClip => 'クリップのピン留めを解除';

  @override
  String get historyReorderPinnedClip => 'ピン留めしたクリップを並べ替え';

  @override
  String get historyReorderPinnedHint =>
      'ドラッグして移動します。Alt + 上矢印キーまたは下矢印キーで 1 つずつ移動します。';

  @override
  String get historyClipUnavailable => 'クリップを利用できません';

  @override
  String historyClipKindSemantics(String kind) {
    return 'クリップの種類：$kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'クリップを選択';

  @override
  String get historyInspectorEmptyMessage => 'クリップの全内容と操作がここに表示されます。';

  @override
  String get historyNoPreview => 'プレビューはありません。';

  @override
  String get historyCloseClipDetails => 'クリップの詳細を閉じる';

  @override
  String get historyCopyOptions => 'コピーのオプション';

  @override
  String get historyCopyPlainText => 'プレーンテキストとしてコピー';

  @override
  String get historyManageClipTags => 'クリップのタグを管理';

  @override
  String get historyPinnedButton => 'ピン留め済み';

  @override
  String get historyAddSpoiler => 'マスクを追加';

  @override
  String get historyRemoveSpoiler => 'マスクを解除';

  @override
  String get historyUpdateFailedTitle => '更新に失敗しました';

  @override
  String get historyRecognizeImageText => '画像内のテキストを認識';

  @override
  String get historyCopiedTitle => 'コピーしました';

  @override
  String get historyCopyFailedTitle => 'コピーに失敗しました';

  @override
  String get historyPlainTextCopied => 'プレーンテキストをコピーしました。';

  @override
  String get historyClipCopied => 'クリップをコピーしました。';

  @override
  String get historyDownloadedTitle => 'ダウンロードしました';

  @override
  String get historyFileSaved => 'ファイルを保存しました。';

  @override
  String get historyDownload => 'ダウンロード';

  @override
  String get historyDownloading => 'ダウンロード中…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'ダウンロード中 $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => 'キャンセル中…';

  @override
  String get historyDownloadCancellingTooltip => 'ダウンロードをキャンセルしています';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'ダウンロード中 $percent% · クリックしてキャンセル';
  }

  @override
  String get historyDownloadFailedRetryTooltip => 'ダウンロードに失敗しました · クリックして再試行';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'ダウンロードをキャンセル、$percent% 完了';
  }

  @override
  String get historyRetryDownload => 'ダウンロードを再試行';

  @override
  String get historyDownloadFailedTitle => 'ダウンロードに失敗しました';

  @override
  String get historyShowFile => 'ファイルを表示';

  @override
  String get historyMetadataPrivacy => 'プライバシー';

  @override
  String get historyMetadataConfidential => '機密';

  @override
  String get historyMetadataCaptured => '記録日時';

  @override
  String get historyMetadataObservedApp => 'コピー元アプリ';

  @override
  String get historyMetadataDevice => 'デバイス';

  @override
  String get historyMetadataClipType => 'クリップの種類';

  @override
  String get historyMetadataType => 'タイプ';

  @override
  String get historyMetadataPath => 'パス';

  @override
  String get historyMetadataFile => 'ファイル';

  @override
  String get historyMetadataResolution => '解像度';

  @override
  String get historyMetadataSize => 'サイズ';

  @override
  String get historyMetadataFiles => 'ファイル数';

  @override
  String get historyMetadataSync => '同期';

  @override
  String get historyMetadataTooLargeToSync => '大きすぎて同期できません';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '選択した $count 件のクリップを削除しますか？',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'このクリップを削除しますか？';

  @override
  String get historyBulkTagTooltip => '選択したピン留めクリップにタグを付ける';

  @override
  String get historyBulkExit => '選択を終了';

  @override
  String get historyBulkPin => '選択項目をピン留め';

  @override
  String get historyBulkUnpin => '選択項目のピン留めを解除';

  @override
  String get historyBulkDelete => '選択項目を削除';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件を選択中',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => '日付で絞り込む';

  @override
  String get historyDatePresetToday => '今日';

  @override
  String get historyDatePresetYesterday => '昨日';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: '過去 $days 日間',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'すべての期間';

  @override
  String get appRuntimeStartFailed => 'CopyPaste のランタイムを起動できませんでした。';

  @override
  String get appPreparing => 'CopyPaste を準備しています。';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste は保護された履歴へのアクセスを待っています。アプリの利用可能な部分は引き続き使用できます。';

  @override
  String get appHistoryUnavailableTitle => '履歴のランタイムを利用できません';

  @override
  String get appHistoryUnavailableMessage => '履歴を開く前に、アプリのランタイムを起動してください。';

  @override
  String get appDevicesUnavailableTitle => 'デバイスのランタイムを利用できません';

  @override
  String get appDevicesUnavailableMessage => 'デバイスを開く前に、アプリのランタイムを起動してください。';

  @override
  String get appSettingsUnavailableTitle => '設定のランタイムを利用できません';

  @override
  String get appSettingsUnavailableMessage => '設定を開く前に、アプリのランタイムを起動してください。';

  @override
  String get appQuit => 'CopyPaste を終了';

  @override
  String get appWindowSetupTitle => 'ウインドウの設定に確認が必要です';

  @override
  String get appOpenHistory => '履歴を開く';

  @override
  String get appDestinationHistory => '履歴';

  @override
  String get appDestinationDevices => 'デバイス';

  @override
  String get appDestinationSettings => '設定';

  @override
  String appDestinationSelected(String destination) {
    return '$destination、選択中';
  }

  @override
  String get appActiveFilter => 'フィルタ適用中';

  @override
  String get appActionFailed => '操作に失敗しました';

  @override
  String get sharedAndroidVersionRequired => 'Android 11 以降が必要です';

  @override
  String get sharedUseAdbTab => 'このデバイスでは「ADB」タブを使用してください。';

  @override
  String get sharedOneTimeAccessApplied => '1 回限りのアクセスを適用しました';

  @override
  String get sharedInstallShizuku => 'Shizuku をインストール';

  @override
  String get sharedPairAndStartShizuku => 'Shizuku をペアリングして起動';

  @override
  String get sharedAllowCopyPaste => 'CopyPaste を許可';

  @override
  String get sharedShizukuNoLongerNeeded => 'Shizuku はもう必要ありません。';

  @override
  String get sharedGetShizukuDescription =>
      '1 回限りのアクセスを適用するには、Shizuku を入手してください。';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Shizuku で「ワイヤレス デバッグ」を使用してから、ここに戻ってください。';

  @override
  String get sharedApproveInShizuku => 'Shizuku で CopyPaste を 1 回承認してください。';

  @override
  String get sharedGetShizuku => 'Shizuku を入手';

  @override
  String get sharedOpenShizuku => 'Shizuku を開く';

  @override
  String get sharedRunOnComputer => 'コンピュータで実行';

  @override
  String get sharedCopyAllCommands => 'すべてのコマンドをコピー';

  @override
  String get sharedEnableUsbDebugging => '「USB デバッグ」を有効にして、このスマートフォンを接続してください。';

  @override
  String get sharedAllow => '許可';

  @override
  String get sharedSetupCommandsCopyFailed => 'セットアップコマンドをコピーできませんでした。';

  @override
  String get sharedShizukuOpenFailed => 'Shizuku を開けませんでした。';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => 'CopyPaste のセットアップ';

  @override
  String get onboardingWelcomeTitle => 'CopyPaste へようこそ';

  @override
  String get onboardingWelcomeSubtitle => 'いつでも使える、あなたのクリップボード。';

  @override
  String get onboardingReadyTitle => 'CopyPaste の準備ができました';

  @override
  String get onboardingGetStarted => 'はじめる';

  @override
  String get onboardingOptionalSetupTitle => 'オプションのセットアップに確認が必要です';

  @override
  String get onboardingOpenSystemSettings => '設定を開く';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return 'ステップ $step/$count · $platform';
  }

  @override
  String get onboardingSetupErrorTitle => 'セットアップの確認が必要です';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste は $platform のセットアップ状態を読み取れませんでした。';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste はセットアップの進行状況を保存できませんでした。';

  @override
  String get onboardingStartAtLogin => 'ログイン時に起動';

  @override
  String get onboardingStartAtLoginDescription => 'ログイン時に CopyPaste を開く';

  @override
  String get onboardingStartAtLoginSemantics => 'ログイン時に CopyPaste を起動';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'ログイン時の起動を有効にできませんでした。このまま続行できます。';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'ログイン時の起動を更新できませんでした。このまま続行できます。';

  @override
  String get onboardingMacosTitle => 'Mac をセットアップ';

  @override
  String get onboardingAccessibility => 'アクセシビリティ';

  @override
  String get onboardingAutoPasteEnabled => '自動ペーストが有効です';

  @override
  String get onboardingAutoPasteOptional => '自動ペースト用（オプション）';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'インストールされた CopyPaste アプリで利用できます。';

  @override
  String get onboardingLoginItemApprovalTitle => 'ログイン項目の承認が必要です';

  @override
  String get onboardingLoginItemApprovalMessage =>
      '「システム設定」で CopyPaste を許可してから、ここに戻ってください。';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste は macOS のセットアップ状態を読み取れませんでした。';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'アクセシビリティの許可を要求できませんでした。';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste は macOS のセットアップ状態を更新できませんでした。';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'アクセシビリティの状態を更新できませんでした。';

  @override
  String get onboardingMacosLoginItemsOpenFailed => '「ログイン項目」設定を開けませんでした。';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'ログイン時に起動するには、「システム設定」での承認が必要です。';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'ログイン時の起動を無効にできませんでした。';

  @override
  String get onboardingWindowsTitle => 'PC をセットアップ';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'このインストールでは利用できません。このまま続行できます。';

  @override
  String get onboardingTaskManagerDisabledTitle => 'タスク マネージャーで無効になっています';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Windows のスタートアップ アプリで CopyPaste を有効にしてから、ここに戻ってください。';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'ログイン時の起動はタスク マネージャーで無効になっています。後で Windows の設定から有効にできます。';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'ログイン時の起動は利用できません。このまま続行できます。';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste は Windows のセットアップ状態を読み取れませんでした。';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Windows のスタートアップ設定を開けませんでした。';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'ログイン時の起動を更新できませんでした。後で「設定」から変更できます。';

  @override
  String get onboardingLinuxChecking => 'Linux のデスクトップセッションを確認しています。';

  @override
  String get onboardingLinuxTitle => 'Linux デスクトップをセットアップ';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste でクリップボードのコピー元を特定し、Quick Paste を使用するには、検証済みのデスクトップ統合が必要です。';

  @override
  String get onboardingLinuxSession => 'デスクトップセッション';

  @override
  String get onboardingLinuxX11Detected => 'X11 セッションを検出しました。';

  @override
  String get onboardingLinuxWaylandDetected => 'Wayland セッションを検出しました。';

  @override
  String get onboardingLinuxSessionUnsupported =>
      '対応している X11 または Wayland のデスクトップセッションが検出されませんでした。';

  @override
  String get onboardingLinuxCompanion => 'GNOME または KDE コンパニオン';

  @override
  String get onboardingLinuxCompanionReady =>
      '認証済みの Clipboard v2 ブリッジの準備ができました。';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'コピー元アプリの除外設定により、クリップボードの記録が一時停止しています。再開するには、署名済みのコンパニオンを有効にしてください。';

  @override
  String get onboardingLinuxCompanionInstall =>
      '署名済みのコンパニオンセッションパッケージをインストールして有効にしてから、ここに戻ってください。';

  @override
  String get onboardingLinuxOpenSetup => 'セットアップを開く';

  @override
  String get onboardingLinuxKeyboardControl => 'キーボード操作';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'Quick Paste でのキーボード操作が許可されています。';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'このデスクトップセッションではキーボード操作を利用できません。';

  @override
  String get onboardingLinuxAllowKeyboard => 'キーボード操作を許可';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste の準備ができました。';

  @override
  String get onboardingLinuxQuickPastePending =>
      '必要なデスクトップ統合が有効になると、Quick Paste を使用できるようになります。';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'ログイン時に CopyPaste を開きます。';

  @override
  String get onboardingLinuxLinks => 'CopyPaste リンク';

  @override
  String get onboardingLinuxLinksRegistered => 'CopyPaste リンクはこのアプリで開きます。';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'ペアリングの招待用に CopyPaste リンクを登録します。';

  @override
  String get onboardingLinuxRegisterLinks => 'リンクを登録';

  @override
  String get onboardingLinuxScreenshotBlocking => 'スクリーンショットのブロック';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Linux ではスクリーンショットのブロックを利用できません。保護された保存データは暗号化されたままです。';

  @override
  String get onboardingLinuxCheckAgain => '再確認';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste は Linux 統合の状態を読み取れませんでした。';

  @override
  String get onboardingLinuxSetupOpenFailed => 'デスクトップ統合のセットアップを開けませんでした。';

  @override
  String get onboardingLinuxKeyboardDenied => 'キーボード操作の許可が得られませんでした。';

  @override
  String get onboardingLinuxStartAtLoginFailed => 'ログイン時の起動を更新できませんでした。';

  @override
  String get onboardingLinuxLinksFailed => 'CopyPaste リンクを登録できませんでした。';

  @override
  String get onboardingLinuxStatusRefreshFailed => 'Linux 統合の状態を更新できませんでした。';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'この X11 セッションではクリップボードにアクセスできません。';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste には X11 または Wayland のデスクトップセッションが必要です。';

  @override
  String get onboardingAndroidFullCapture => 'フル記録';

  @override
  String get onboardingAndroidBackgroundCapture => 'バックグラウンド記録';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      '一度設定したら、実際のコピーで動作を確認します。';

  @override
  String get onboardingAndroidLimitedCapture => '制限付き記録';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'CopyPaste に共有するか、コピー後にこのアプリに戻ります。';

  @override
  String get onboardingAndroidNotification => '記録の通知';

  @override
  String get onboardingAndroidNotificationDescription => '記録の実行中に必要です';

  @override
  String get onboardingAndroidBackgroundActivity => 'バックグラウンドでの動作';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      '記録の取りこぼしを減らすために推奨';

  @override
  String get onboardingAndroidOpen => '開く';

  @override
  String get onboardingAndroidApplyAccess => '記録へのアクセスを適用';

  @override
  String get onboardingAndroidApplyAccessDescription => '1 回限りの記録アクセスを適用します。';

  @override
  String get onboardingAndroidFullCaptureLater => 'フル記録は後で設定できます。';

  @override
  String get onboardingAndroidCaptureWorking => 'バックグラウンド記録は動作しています';

  @override
  String get onboardingAndroidVerifyCopy => '実際のコピーで確認';

  @override
  String get onboardingAndroidCopyReached => '新しいコピーがローカルの履歴に届きました。';

  @override
  String get onboardingAndroidCopyInstruction => '別のアプリでコピーしてから、ここに戻ってください。';

  @override
  String get onboardingAndroidStartCapture => '記録を開始';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste は Android のセットアップ状態を読み取れませんでした。';

  @override
  String get onboardingAndroidStateRefreshFailed => 'Android の記録状態を更新できませんでした。';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Android のセットアップを監視できません。';

  @override
  String get onboardingAndroidIntakeFailed => 'クリップボードの取り込みを有効にできませんでした。';

  @override
  String get onboardingAndroidStopCaptureFailed => 'バックグラウンド記録を停止できませんでした。';

  @override
  String get onboardingAndroidBatterySettingsFailed => '電池の設定を開けませんでした。';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste は Shizuku で許可されていますが、Android の記録権限を適用できませんでした。';

  @override
  String get onboardingAndroidAllowInShizuku =>
      '記録権限を適用するには、Shizuku で CopyPaste を許可してください。';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      '先にバックグラウンドでのクリップボードアクセスを許可してください。';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'バックグラウンド記録を開始する前に、通知を許可してください。';

  @override
  String get onboardingAndroidStartCaptureFailed => 'バックグラウンド記録を開始できませんでした。';

  @override
  String get onboardingAndroidSetupOpenFailed => 'Android の記録セットアップを開けませんでした。';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Android のセットアップの選択を保存できませんでした。';

  @override
  String get modulesMarketplaceTab => 'マーケットプレイス';

  @override
  String get modulesInstalledTab => 'インストール済み';

  @override
  String get modulesRefresh => '更新';

  @override
  String get modulesSearchPlaceholder => 'モジュールを検索';

  @override
  String get modulesOperationFailed => 'モジュールの操作に失敗しました';

  @override
  String get modulesLoading => 'モジュールを読み込んでいます。';

  @override
  String get modulesUnavailableTitle => 'モジュールを利用できません';

  @override
  String get modulesUnavailableMessage => 'もう一度モジュールを読み込んでください。';

  @override
  String get modulesNoneInstalled => 'インストール済みのモジュールはありません';

  @override
  String get modulesNoMatches => '一致するモジュールはありません';

  @override
  String get modulesChooseFromMarketplace => 'マーケットプレイスからモジュールを選択してください。';

  @override
  String get modulesTryDifferentSearch => '別のキーワードで検索してください。';

  @override
  String get modulesMarketplaceLoading => 'マーケットプレイスを読み込んでいます。';

  @override
  String get modulesMarketplaceUnavailableTitle => 'マーケットプレイスを利用できません';

  @override
  String get modulesMarketplaceUnavailableMessage => 'もう一度マーケットプレイスを読み込んでください。';

  @override
  String get modulesNoneAvailable => '利用できるモジュールはありません';

  @override
  String get modulesNoneAvailableMessage => 'モジュールが公開されると、ここに表示されます。';

  @override
  String modulesEnableModule(String module) {
    return '$module を有効にする';
  }

  @override
  String get modulesInstall => 'インストール';

  @override
  String modulesUpdateTo(String version) {
    return '$version にアップデート';
  }

  @override
  String get modulesRemoveModule => 'モジュールを削除';

  @override
  String get modulesModuleSettings => 'モジュールの設定';

  @override
  String modulesSettingsTitle(String module) {
    return '$module の設定';
  }

  @override
  String get modulesDownloadingModel => '言語モデルをダウンロードしています…';

  @override
  String get modulesRun => '実行';

  @override
  String modulesRemoveTitle(String module) {
    return '$module を削除しますか？';
  }

  @override
  String get modulesRemoveMessage => 'モジュールとそのローカル設定が削除されます。';

  @override
  String get modulesRemove => '削除';

  @override
  String get modulesRemoveFailed => 'モジュールを削除できませんでした。';

  @override
  String get modulesCloseSettings => 'モジュールの設定を閉じる';

  @override
  String get modulesRemoved => 'モジュールを削除しました';

  @override
  String get modulesNoSettingsTitle => '追加の設定はありません';

  @override
  String get modulesNoSettingsMessage => 'このモジュールには追加の設定は必要ありません。';

  @override
  String get modulesSetUpSmsAccess => 'SMS へのアクセスを設定';

  @override
  String get modulesRestartApp => 'CopyPaste を再起動';

  @override
  String get modulesSmsAccessTitle => 'SMS へのアクセス';

  @override
  String get modulesCloseSmsAccess => 'SMS へのアクセスを閉じる';

  @override
  String get modulesSmsChecking => 'SMS へのアクセスを確認しています。';

  @override
  String get modulesSmsReady =>
      'アクセスの準備ができました。新しいコードのコピーを開始するには、SMS Codes を有効にしてください。';

  @override
  String get modulesSmsAllowAccess =>
      '新しい SMS メッセージからコードをコピーするには、アクセスを許可してください。';

  @override
  String get modulesSmsNotification => 'SMS の通知';

  @override
  String get modulesSmsNotificationDescription => 'SMS Codes の実行中に必要です';

  @override
  String get modulesSmsApplyAccess => 'SMS へのアクセスを適用';

  @override
  String get modulesSmsApplyAccessDescription => '1 回限りの SMS アクセスを適用します。';

  @override
  String get modulesSmsSetupFailed => 'SMS のセットアップに失敗しました';

  @override
  String get modulesFileSelectionFailed => 'ファイルの選択に失敗しました';

  @override
  String get modulesChooseLanguages => '言語を選択';

  @override
  String get modulesChooseFile => 'ファイルを選択';

  @override
  String modulesModelReady(String model) {
    return '$model モデルの準備ができました';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return '$model モデル · $size MiB のダウンロード';
  }

  @override
  String get modulesPlatformsUnknown => 'プラットフォーム：不明';

  @override
  String modulesPlatforms(String platforms) {
    return 'プラットフォーム：$platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid => 'モジュールカタログの署名が無効です。';

  @override
  String get modulesMarketplaceConnectFailed => 'モジュールマーケットプレイスに接続できませんでした。';

  @override
  String get modulesMarketplaceTimeout => 'モジュールマーケットプレイスから応答がありませんでした。';

  @override
  String get modulesIncompatible => 'このモジュールはこのデバイスと互換性がありません。';

  @override
  String get modulesPackageSizeInvalid => 'モジュールパッケージのサイズが無効です。';

  @override
  String get modulesDownloadInterrupted => 'モジュールのダウンロードが中断されました。';

  @override
  String get modulesDownloadTimeout => 'モジュールのダウンロードから応答がありませんでした。';

  @override
  String get modulesPackageUrlUntrusted => 'モジュールパッケージの URL は信頼されていません。';

  @override
  String get modulesDownloadLeftGithub => 'モジュールのダウンロードが GitHub 外にリダイレクトされました。';

  @override
  String get modulesDownloadTooManyRedirects => 'モジュールのダウンロードでリダイレクトが多すぎます。';

  @override
  String get modulesDownloadTooLarge => 'モジュールのダウンロードが大きすぎます。';

  @override
  String get modulesPackageIntegrityFailed => 'モジュールパッケージの整合性チェックに失敗しました。';

  @override
  String get modulesDownloadUnavailable => 'このモジュールは現在ダウンロードできません。';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'モジュールマーケットプレイスを利用できません。もう一度お試しください。';

  @override
  String get modulesCatalogInvalid => 'モジュールカタログが無効です。';

  @override
  String get modulesThisSystem => 'このシステム';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version 以降';
  }

  @override
  String get modulesNotAvailableForDevice => 'このデバイスでは利用できません。';

  @override
  String get modulesAppVersionUnverified =>
      'インストールされている CopyPaste のバージョンを確認できませんでした。';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'このモジュールは CopyPaste $version に対応していません。';
  }

  @override
  String modulesRequires(String requirement) {
    return '$requirement が必要です。';
  }

  @override
  String get modulesMarketplaceLoadFailed => 'モジュールマーケットプレイスを読み込めませんでした。';

  @override
  String get modulesSmsSetupRequired => 'このモジュールを有効にする前に、SMS へのアクセスを設定してください。';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'SMS の監視を開始できませんでした。モジュールは有効のままです。再試行するには、もう一度無効にしてください。';

  @override
  String get modulesSmsMonitoringDisabled =>
      'SMS の監視を開始できませんでした。モジュールは無効になりました。';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'SMS の監視を開始できませんでした。モジュールの有効状態を確認できませんでした。';

  @override
  String get modulesSmsMonitoringFailed => 'SMS の監視を開始できませんでした。';

  @override
  String get modulesSmsAndroidOnly => 'SMS へのアクセスは Android でのみ利用できます。';

  @override
  String get modulesSmsMonitoringNeedsSetup => 'SMS の監視にはセットアップが必要です。';

  @override
  String get modulesOperationIncomplete => 'モジュールの操作を完了できませんでした。';

  @override
  String get modulesUnsupportedEvent => 'モジュールが対応していないイベントを使用しています。';

  @override
  String get modulesUnsupportedResult => 'モジュールが対応していない結果を返しました。';

  @override
  String get modulesUnsupportedField => 'モジュールが対応していないフィールドを使用しています。';

  @override
  String get modulesResponseInvalid => 'モジュールの応答が無効です。';

  @override
  String get modulesFileSelectionUnavailable => 'ファイルの選択を利用できません。';

  @override
  String get modulesFileSelectFailed => 'ファイルを選択できませんでした。';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field は受け付けられないファイルです。';
  }

  @override
  String get modulesSmsVerifyFailed => 'SMS へのアクセスを確認できませんでした。';

  @override
  String get modulesSmsAllowNotifications =>
      'SMS Codes をバックグラウンドで動作させ続けるには、通知を許可してください。';

  @override
  String get modulesSmsNotGranted => 'SMS へのアクセスが許可されませんでした。';

  @override
  String get modulesSmsConfigureFailed => 'SMS へのアクセスを設定できませんでした。';

  @override
  String get onboardingLinuxContinueBasic => '基本の記録で続ける';

  @override
  String get onboardingLinuxCompanionOptional =>
      'オプション。Quick Paste とコピー元アプリの除外に必要です。';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'オプション。Quick Paste で前のアプリに戻るために必要です。';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'この Wayland セッションではクリップボードにアクセスできません。署名済みの GNOME または KDE コンパニオンを有効にしてから、再確認してください。';

  @override
  String get historyMoreActions => 'その他の操作';

  @override
  String get historyTagsAction => 'タグ…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '機密項目のロック解除',
      'other': '機密項目のロックを解除',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '非表示スニペットの使用',
      'other': '非表示スニペットを使用',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '機密項目のコピー',
      'other': '機密項目をコピー',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'このシステムでは利用できません';

  @override
  String get deviceAuthNotConfigured => '先にデバイスのパスワード、PIN、または生体認証を設定してください';

  @override
  String get commonOn => 'オン';

  @override
  String get commonOff => 'オフ';

  @override
  String get settingsQuickPasteStartFailed =>
      'Quick Paste を開始できなかったため、ショートカットは機能しません。';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut は別のアプリで使用されています。別のショートカットを選択してください。';
  }

  @override
  String get devicesEmptyTitle => 'ほかのデバイスはまだペアリングされていません';

  @override
  String get devicesEmptyMessage => '同じネットワーク上のデバイスとペアリングして履歴を同期します。';

  @override
  String get pairingStateNotCompleted => 'ペアリングは完了しませんでした';

  @override
  String get pairingStepInvite => '招待';

  @override
  String get pairingStepScan => 'スキャン';

  @override
  String get pairingStepCode => 'コード';

  @override
  String get pairingStepRequest => 'リクエスト';

  @override
  String get pairingStepConnect => '接続';

  @override
  String get pairingStepConfirm => '確認';

  @override
  String get pairingStepPaired => 'ペアリング済み';

  @override
  String get pairingOtherDevice => 'もう一方のデバイス';

  @override
  String get pairingDecisionNotConfirmed => '未確認';

  @override
  String get pairingDecisionSending => '決定を送信しています…';

  @override
  String get pairingDecisionConfirmed => '確認済み';

  @override
  String get pairingDecisionRejected => '拒否済み';

  @override
  String get pairingDecisionWaiting => '確認を待っています';

  @override
  String get pairingDecisionUnknown => '回答は不明';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 台のデバイス',
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
  String get appNavigationLabel => 'メインナビゲーション';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'ステップ $step/$count: $name';
  }

  @override
  String get modulesPhaseDownload => 'ダウンロード';

  @override
  String get modulesPhaseInstall => 'インストール';

  @override
  String modulesInstallingModule(String module) {
    return '$module をインストール中';
  }

  @override
  String modulesUpdatingModule(String module) {
    return '$module をアップデート中';
  }

  @override
  String get commonDismiss => '閉じる';

  @override
  String get settingsRetentionConfirmTitle => '古い履歴を削除しますか？';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return '新しい上限：$limit。これより古いピン留めされていない項目は完全に削除されます。';
  }

  @override
  String get settingsStorageQuotaConfirmTitle => 'ストレージの上限を下げますか？';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return '新しい上限：$size。超えた分は古いピン留めされていない項目から完全に削除されます。';
  }

  @override
  String get settingsLimitConfirmAction => '上限を適用';

  @override
  String get settingsHideEverywhereTitle => 'CopyPaste を完全に非表示にしますか？';

  @override
  String get settingsHideEverywhereMessage =>
      '両方オフにするとアイコンが表示されません。開くには CopyPaste を再度起動します。';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return '両方オフにするとアイコンが表示されません。開くには $shortcut を押すか、CopyPaste を再度起動します。';
  }

  @override
  String get settingsHideEverywhereAction => '非表示にする';

  @override
  String get settingsExportDoneTitle => 'テキスト履歴をエクスポートしました';

  @override
  String get settingsBackupDoneTitle => 'バックアップを作成しました';

  @override
  String get settingsRestoreDoneTitle => '履歴を復元しました';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'テキストクリップ $count 件をエクスポートしました',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件は読み取れませんでした',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return '暗号化バックアップを保存しました · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'このデバイスのクリップボード履歴がバックアップの内容に置き換わりました。';

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
    return '$quota 中 $used を使用';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '$used を使用';
  }

  @override
  String get storageUsageOverQuota => '上限超過';

  @override
  String storageUsageOldestClip(String date) {
    return '最も古いクリップ：$date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'ストレージ：$usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return '新しい上限：$size。現在 $used を使用中のため、古いピン留めされていない項目から完全に削除されます。';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'CopyPaste を開く';

  @override
  String get trayOpenQuickPaste => 'Quick Paste を開く';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Quick Paste を開く ($shortcut)';
  }

  @override
  String get trayNewSnippet => '新しいスニペット';

  @override
  String get trayRecent => '最近';

  @override
  String get trayRecentLoading => '読み込み中…';

  @override
  String get trayRecentFailed => 'クリップを読み込めませんでした';

  @override
  String get trayRecentEmpty => 'まだクリップがありません';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '最近のクリップ $count 件を非表示',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'シークレット';

  @override
  String get trayCopyLatestFromOtherDevice => '別のデバイスの最新クリップをコピー';

  @override
  String get trayCaptureNow => '今すぐ記録';

  @override
  String trayPausedUntil(String time) {
    return '$time まで一時停止中';
  }

  @override
  String get trayPauseFor => '一時停止する時間…';

  @override
  String get trayPauseFor15Minutes => '15 分';

  @override
  String get trayPauseForOneHour => '1 時間';

  @override
  String get trayPauseUntilResumed => '再開するまで';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count 件',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '収集中: $count 件',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return '最近スキップ ($count)';
  }

  @override
  String get trayOpenContentRules => 'コンテンツルールを開く';

  @override
  String get trayRecentSettingsTitle => 'メニューバーとシステムトレイ';

  @override
  String get trayRecentSettingsDescription =>
      'メニューバーまたはシステムトレイのメニューで「最近」に表示する内容です。';

  @override
  String get trayRecentCountTitle => '最近のクリップ';

  @override
  String get trayRecentCountDescription => 'メニューに表示する最新クリップの数です。';

  @override
  String get trayRecentPreviewTitle => '最近のクリップのプレビュー';

  @override
  String get trayRecentPreviewDescription => 'メニューに表示するクリップのテキスト量です。';

  @override
  String get trayRecentPreviewFull => '完全なプレビュー';

  @override
  String get trayRecentPreviewTitles => '種類と取得元のみ';

  @override
  String get trayRecentPreviewHidden => '非表示';

  @override
  String get trayRecentSettingsKeywords => 'トレイ メニューバー 最近 プレビュー プライバシー 件数';

  @override
  String get transformsPaletteTitle => 'テキストを変換';

  @override
  String get transformsFilterPlaceholder => '変換を絞り込む';

  @override
  String get transformsNoMatches => '一致する変換はありません';

  @override
  String get transformsGroupResults => '結果';

  @override
  String get transformsGroupSuggested => 'おすすめ';

  @override
  String get transformsGroupRecent => '最近';

  @override
  String get transformsGroupFavorites => 'お気に入り';

  @override
  String get transformsGroupPipelines => 'パイプライン';

  @override
  String get transformsCategoryStructure => 'JSON・YAML・CSV・XML';

  @override
  String get transformsCategoryEncode => 'エンコードとデコード';

  @override
  String get transformsCategoryCase => '大文字と小文字';

  @override
  String get transformsCategoryHash => 'ハッシュ';

  @override
  String get transformsCategoryLines => '行と空白';

  @override
  String get transformsCategoryClean => 'クリーンアップ';

  @override
  String get transformsCategoryUrl => 'リンク';

  @override
  String get transformsCategoryMarkup => 'リッチテキストと Markdown';

  @override
  String get transformsCategoryDev => '開発者';

  @override
  String get transformsCategoryOther => 'その他';

  @override
  String get transformsChainEmpty => 'ステップはまだありません';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return '次に: $name';
  }

  @override
  String get transformsStepEnabled => 'このステップを実行';

  @override
  String get transformsMoveEarlier => '上へ移動';

  @override
  String get transformsMoveLater => '下へ移動';

  @override
  String get transformsRemoveStep => 'ステップを削除';

  @override
  String get transformsSavePipeline => 'パイプラインとして保存';

  @override
  String get transformsSavePipelineTitle => 'パイプラインを保存';

  @override
  String get transformsSavePipelineFailed => 'パイプラインを保存できませんでした。';

  @override
  String get transformsPipelineNameLabel => '名前';

  @override
  String get transformsPipelineDescriptionLabel => '説明（任意）';

  @override
  String get transformsPipelineStepsLabel => 'ステップ';

  @override
  String get transformsPipelineNoSteps => 'ステップを1つ以上追加してください。';

  @override
  String get transformsPipelineNameRequired => '名前を入力してください。';

  @override
  String get transformsPipelineNameTooLong => '80文字以内にしてください。';

  @override
  String get transformsPipelineNameTaken => '同じ名前のパイプラインがすでにあります。';

  @override
  String get transformsPipelineSavedTitle => 'パイプラインを保存しました';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name はすべての変換パレットで使えます。';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'ステップ: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => '削除されたパイプライン';

  @override
  String get transformsNeedsNewerVersion => '新しいバージョンの CopyPaste が必要です';

  @override
  String get transformsLoadFailed => '変換を読み込めませんでした。';

  @override
  String get transformsNotTextTitle => '変換はテキストに対して行います';

  @override
  String get transformsNotTextMessage =>
      'この項目は画像、ファイル、または色です。テキスト項目を選ぶか、テキストをコピーしてください。';

  @override
  String get transformsNothingTitle => '変換するものがありません';

  @override
  String get transformsNothingMessage => 'テキストをコピーするか、履歴の項目を選んでください。';

  @override
  String get transformsPickTransform => '変換を選んでください';

  @override
  String get transformsPickTransformHint => 'Tab でステップとして追加、Enter で結果を適用します。';

  @override
  String get transformsPreviewing => 'プレビューを更新中…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return '行: $lines · 単語: $words · 文字: $chars';
  }

  @override
  String get transformsNoChanges => '変更なし';

  @override
  String transformsTruncated(String size) {
    return '$size のうち最初の 64 KiB を表示しています';
  }

  @override
  String get transformsRandom => 'ランダム';

  @override
  String get transformsNotRun => '未実行';

  @override
  String get transformsSensitiveHidden => '結果は機密のため非表示です。';

  @override
  String get transformsReveal => '表示';

  @override
  String get transformsPaneOutput => '出力';

  @override
  String get transformsPaneInput => '入力';

  @override
  String get transformsPaneChanges => '変更点';

  @override
  String get transformsCannotTransform => 'このテキストは変換できません';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'ステップ $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => '16進数で表示';

  @override
  String get transformsActionPaste => '貼り付け';

  @override
  String get transformsActionSave => '新しい項目として保存';

  @override
  String get transformsCopiedMessage => '結果はクリップボードと履歴にあります。';

  @override
  String get transformsSavedTitle => '保存しました';

  @override
  String get transformsSavedMessage => '新しい履歴項目として保存しました。';

  @override
  String get transformsPasteFailedTitle => '貼り付けできませんでした';

  @override
  String get transformsPasteFailedMessage => '結果はクリップボードにあります。手動で貼り付けてください。';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message（$line 行 $column 列）';
  }

  @override
  String get transformsErrorBinary => 'デコードしたデータはテキストではなくバイナリです';

  @override
  String get transformsErrorTooLarge => '大きすぎて変換できません';

  @override
  String get transformsErrorInvalidParam => 'このステップの設定が無効です';

  @override
  String get transformsErrorCancelled => 'キャンセルしました';

  @override
  String get transformsErrorTooExpensive => 'パターンの処理が重すぎます';

  @override
  String get transformsErrorPattern => 'パターンが無効です';

  @override
  String get transformsErrorTimestampUnit => '数値が秒かミリ秒かを選んでください';

  @override
  String get transformsErrorExternalEntity => '外部エンティティは使用できません';

  @override
  String get transformsErrorJwtEncrypted => '暗号化されたトークンはデコードできません';

  @override
  String get transformsErrorJson => '無効な JSON';

  @override
  String get transformsErrorYaml => '無効な YAML';

  @override
  String get transformsErrorXml => '無効な XML';

  @override
  String get transformsErrorCsv => '無効な CSV';

  @override
  String get transformsErrorBase64 => '無効な Base64';

  @override
  String get transformsErrorHex => '無効な16進数';

  @override
  String get transformsErrorEscape => '無効なエスケープシーケンス';

  @override
  String get transformsErrorUrl => 'パーセントエンコードが不正です';

  @override
  String get transformsErrorJwt => '有効な JWT ではありません';

  @override
  String get transformsErrorDate => '認識できる日付またはタイムスタンプではありません';

  @override
  String get transformsErrorGeneric => 'このステップは入力を変換できません';

  @override
  String get transformsWarningSignedLink => '署名付きリンクは変更していません';

  @override
  String get transformsWarningExpired => '期限切れ';

  @override
  String get transformsWarningNotYetValid => 'まだ有効ではありません';

  @override
  String get transformsWarningSignatureNotVerified => '署名は検証されていません';

  @override
  String get transformsWarningDuplicateKeys => '重複したキー';

  @override
  String get transformsWarningLenientJson => '標準外の JSON を受け入れました';

  @override
  String get transformsWarningDoubleEncoded => 'JSON が二重にエンコードされていました';

  @override
  String get transformsWarningEscapes => '不明なエスケープシーケンスはそのまま残しました';

  @override
  String get transformsWarningLocalZone => 'ローカルのタイムゾーンとみなしました';

  @override
  String get transformsWarningNoMatches => '一致なし';

  @override
  String get transformsWarningGeneric => '結果を確認してください';

  @override
  String get transformsRestoreTitle => '貼り付け後にクリップボードを復元';

  @override
  String get transformsRestoreDescription => '貼り付け後に以前のクリップボードに戻します。';

  @override
  String get transformsDefaultTitle => '貼り付け時の既定の変換';

  @override
  String get transformsDefaultDescription => 'Quick Paste から貼り付けるテキストに適用されます。';

  @override
  String get transformsNoDefault => 'なし';

  @override
  String get transformsAppDefaultsTitle => 'アプリごとの既定';

  @override
  String get transformsAppDefaultsDescription => '特定のアプリで別の既定を使います。';

  @override
  String transformsOtherPlatform(String platform) {
    return '$platform のみ';
  }

  @override
  String get transformsAddApp => 'アプリを追加';

  @override
  String get transformsRemove => '削除';

  @override
  String get transformsFavoritesTitle => 'お気に入り';

  @override
  String get transformsFavoritesDescription => 'すべてのパレットで先頭に表示されます。';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'パレットで Mod+$number';
  }

  @override
  String get transformsAddFavorite => 'お気に入りを追加';

  @override
  String get transformsPipelinesTitle => 'パイプライン';

  @override
  String get transformsPipelinesDescription => '保存した変換の組み合わせです。';

  @override
  String get transformsNewPipeline => '新しいパイプライン';

  @override
  String get transformsEditPipeline => 'パイプラインを編集';

  @override
  String get transformsDuplicatePipeline => '複製';

  @override
  String get transformsDeletePipeline => '削除';

  @override
  String transformsDeletePipelineTitle(String name) {
    return '$name を削除しますか？';
  }

  @override
  String get transformsDeletePipelineMessage => 'これを使うお気に入りと貼り付けの既定は解除されます。';

  @override
  String get transformsDeletePipelineFailed => 'パイプラインを削除できませんでした。';

  @override
  String get transformsAddStep => 'ステップを追加';

  @override
  String get transformsHiddenTitle => '非表示の変換';

  @override
  String get transformsHiddenDescription => 'パレットとおすすめに表示されません。';

  @override
  String get transformsHideTransform => '変換を非表示にする';

  @override
  String get transformsShow => '表示';

  @override
  String get transformsSuggestionsTitle => 'おすすめ';

  @override
  String get transformsSuggestionsDescription => 'テキストに合う変換をおすすめします。';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web トークン';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl => 'トラッキングパラメーター付きリンク';

  @override
  String get transformsDetectorTimestamp => 'タイムスタンプ';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'トラッキングパラメーター';

  @override
  String get transformsTrackingDescription => '追加で削除する、または常に残すリンクのパラメーターです。';

  @override
  String get transformsTrackingRemoveLabel => '追加で削除';

  @override
  String get transformsTrackingAllowLabel => '常に保持';

  @override
  String get transformsTrackingPlaceholder =>
      'name または name@domain を入力して Enter';

  @override
  String get quickAccessUnavailableTitle => 'クイックアクセスの状態を取得できません';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste はキーボード、タイル、ウィジェットの状態を読み取れませんでした。もう一度お試しください。';

  @override
  String get quickAccessActionFailedTitle => '操作を完了できませんでした';

  @override
  String get quickAccessActionFailedMessage =>
      'Android が要求した画面を開けなかったか、変更を保存できませんでした。もう一度お試しください。';

  @override
  String get quickAccessKeyboardTitle => 'クリップキーボード';

  @override
  String get quickAccessKeyboardDescription =>
      'キーボード設定で「CopyPaste クリップ」をオンにします。';

  @override
  String get quickAccessKeyboardOn => 'オン';

  @override
  String get quickAccessKeyboardOff => 'オフ';

  @override
  String get quickAccessOpenKeyboardSettings => 'キーボード設定を開く';

  @override
  String get quickAccessKeyboardSearchDescription =>
      '「CopyPaste クリップ」キーボードパネルをオンにします。';

  @override
  String get quickAccessKeyboardKeywords => 'キーボード 入力方法 ime パネル 有効';

  @override
  String get quickAccessSwitchKeyboardTitle => 'クリップキーボードに切り替え';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      '「CopyPaste クリップ」を現在のキーボードにします。';

  @override
  String get quickAccessSwitchKeyboardAction => 'キーボードを選択';

  @override
  String get quickAccessKeyboardCurrent => '現在のキーボード';

  @override
  String get quickAccessCaptureTierTitle => '記録モード';

  @override
  String get quickAccessCaptureTierDescription => '他のアプリからのコピーを保存する方法です。';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      '他のアプリのコピーがどのように保存されるかを確認します。';

  @override
  String get quickAccessCaptureTierKeywords => '記録 段階 フル パネル 手動 shizuku 収集';

  @override
  String get quickAccessTierFull => 'フル: すべてのコピーをバックグラウンドで保存';

  @override
  String get quickAccessTierPanelCollecting => 'パネル選択中: 収集中';

  @override
  String get quickAccessTierPanelCatchUp => 'パネルモード: 開いたときに取り込み';

  @override
  String get quickAccessTierManual => '手動: アプリ、タイル、ウィジェット、共有から保存';

  @override
  String get quickAccessTierPaused => '記録を一時停止中';

  @override
  String get quickAccessAutoReturnTitle => '挿入後に前のキーボードに戻る';

  @override
  String get quickAccessAutoReturnDescription => 'クリップを 1 つ挿入したら元に戻ります。';

  @override
  String get quickAccessAutoReturnKeywords => '戻る 前のキーボード 自動 挿入';

  @override
  String get quickAccessFocusSearchTitle => 'パネルを開いたときに検索にフォーカス';

  @override
  String get quickAccessFocusSearchDescription => '検索欄を選択した状態でパネルを開きます。';

  @override
  String get quickAccessTilesTitle => 'クイック設定タイル';

  @override
  String get quickAccessTilesSearchDescription =>
      '「クリップボードを保存」と「クリップボードの記録」タイルを追加します。';

  @override
  String get quickAccessTilesKeywords => 'クイック設定 タイル 通知パネル 保存 一時停止';

  @override
  String get quickAccessTileInstructions =>
      '2 回下にスワイプし、[編集] をタップして CopyPaste のタイルを追加します。';

  @override
  String get quickAccessSaveTileTitle => '「クリップボードを保存」タイル';

  @override
  String get quickAccessSaveTileDescription => 'クイック設定からクリップボードの内容を保存します。';

  @override
  String get quickAccessCaptureTileTitle => '「クリップボードの記録」タイル';

  @override
  String get quickAccessCaptureTileDescription => 'クイック設定から記録を一時停止・再開します。';

  @override
  String get quickAccessAddTile => 'タイルを追加';

  @override
  String get quickAccessTileAdded => 'クイック設定に追加済み';

  @override
  String get quickAccessWidgetTitle => '「最近のクリップ」ウィジェット';

  @override
  String get quickAccessWidgetDescription => 'ホーム画面の最近のクリップ。タップでコピーします。';

  @override
  String get quickAccessWidgetInstructions =>
      'ホーム画面を長押しし、[ウィジェット] から「最近のクリップ」を追加します。';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 個のウィジェットを追加済み',
      zero: '未追加',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'ウィジェットを追加';

  @override
  String get quickAccessWidgetKeywords => 'ホーム画面 ウィジェット ランチャー 最近';

  @override
  String snippetsAbbreviationBadge(String abbreviation) {
    return '略語 $abbreviation';
  }

  @override
  String get snippetsAbbreviationLabel => '略語';

  @override
  String get snippetsAbbreviationPlaceholder => '省略可、例: sig';

  @override
  String snippetsAbbreviationTaken(String title) {
    return '「$title」で使用済みです';
  }

  @override
  String snippetsAbbreviationTooLong(int max) {
    return '$max 文字以内にしてください。';
  }

  @override
  String get snippetsAbbreviationWhitespace => '略語にスペースや制御文字は使用できません。';

  @override
  String snippetsAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '同期後に確認が必要なスニペット: $count 件',
      zero: '確認が必要なスニペットはありません',
    );
    return '$_temp0';
  }

  @override
  String get snippetsBodyLabel => 'テキスト';

  @override
  String get snippetsBodyPlaceholder => 'スニペットのテキストを入力';

  @override
  String get snippetsClearSelection => '選択を解除';

  @override
  String get snippetsClipTooLarge => '大きすぎるため編集できません。このクリップは読み取り専用で開きます。';

  @override
  String snippetsCollectionAccent(String accent) {
    String _temp0 = intl.Intl.selectLogic(accent, {
      'neutral': 'ニュートラル',
      'blue': '青',
      'green': '緑',
      'amber': '琥珀',
      'red': '赤',
      'other': 'ニュートラル',
    });
    return '$_temp0';
  }

  @override
  String get snippetsCollectionAccentLabel => 'アクセントカラー';

  @override
  String get snippetsCollectionIcon => 'アイコン';

  @override
  String get snippetsCollectionLabel => 'コレクション';

  @override
  String get snippetsCollectionName => '名前';

  @override
  String get snippetsCollectionNameTaken => 'この名前のコレクションはすでにあります。';

  @override
  String snippetsCollectionNameTooLong(int max) {
    return '$max 文字以内にしてください。';
  }

  @override
  String get snippetsConcealedBody => 'このスニペットは非表示です。';

  @override
  String get snippetsConcealedConfirmAction => '続ける';

  @override
  String get snippetsConcealedConfirmMessage =>
      'このデバイスでは本人確認できません。自分のデバイスの場合のみ続行してください。';

  @override
  String get snippetsConcealedConfirmTitle => '本人確認';

  @override
  String get snippetsConcealedPreview => '非表示スニペット';

  @override
  String get snippetsConcealedSwitch => '非表示';

  @override
  String get snippetsConcealedTitle => '非表示スニペット';

  @override
  String get snippetsConflictBadge => '競合コピー';

  @override
  String get snippetsCopied => 'コピーしました';

  @override
  String get snippetsCreate => 'スニペットを作成';

  @override
  String snippetsCreateNamed(String query) {
    return 'スニペット「$query」を作成';
  }

  @override
  String get snippetsDeleteCollection => 'コレクションを削除';

  @override
  String get snippetsDeleteCollectionMove => 'スニペットを「未分類」に移動';

  @override
  String snippetsDeleteCollectionTitle(String name) {
    return '「$name」を削除しますか?';
  }

  @override
  String get snippetsDeleteCollectionWithSnippets => 'スニペットも削除';

  @override
  String get snippetsDeletedMessage => '「最近削除した項目」に移動しました。';

  @override
  String snippetsDeletedTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のスニペットを削除しました',
      one: 'スニペットを削除しました',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDestination => '保存先';

  @override
  String snippetsDestinationName(String destination) {
    String _temp0 = intl.Intl.selectLogic(destination, {
      'historyClip': '履歴のクリップ',
      'snippet': 'スニペット',
      'other': '履歴のクリップ',
    });
    return '$_temp0';
  }

  @override
  String get snippetsDiscard => '破棄';

  @override
  String snippetsDragging(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のスニペット',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDuplicateAbbreviation =>
      '別のスニペットに同じ略語があります。どちらかを変更してください。';

  @override
  String get snippetsDuplicateBadge => '略語の重複';

  @override
  String get snippetsEditClip => 'クリップを編集';

  @override
  String get snippetsEditCollection => 'コレクションを編集';

  @override
  String get snippetsEditSnippet => 'スニペットを編集';

  @override
  String get snippetsEditTab => '編集';

  @override
  String get snippetsEmptyMessage => 'スニペットに略語を付けて、Quick Paste で入力します。';

  @override
  String get snippetsEmptyTitle => 'スニペットはまだありません';

  @override
  String get snippetsEnableTemplate => 'オンにする';

  @override
  String get snippetsEnableTemplateMessage => 'プレースホルダーのように見えるテキストも埋め込まれます。';

  @override
  String get snippetsEnableTemplateTitle => 'テンプレートをオンにしますか?';

  @override
  String get snippetsExport => '書き出す';

  @override
  String get snippetsExportCollection => 'コレクションを書き出す';

  @override
  String get snippetsExportConcealedConfirm => '読み取り可能なファイルに非表示スニペットを含める';

  @override
  String snippetsExportCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のスニペットを書き出しました',
      zero: 'スニペットは書き出されませんでした',
    );
    return '$_temp0';
  }

  @override
  String get snippetsExportDone => 'スニペットを書き出しました';

  @override
  String get snippetsExportEncrypt => 'パスフレーズで暗号化';

  @override
  String get snippetsExportFailed => '書き出しに失敗しました';

  @override
  String get snippetsExportPlainConfirm => 'ファイルを持つ人なら誰でも読めることを理解しています';

  @override
  String snippetsExportScope(String scope, int count) {
    String _temp0 = intl.Intl.selectLogic(scope, {
      'all': 'すべてのスニペット',
      'collections': 'このコレクション',
      'snippets': '選択したスニペット ($count)',
      'other': 'すべてのスニペット',
    });
    return '$_temp0';
  }

  @override
  String get snippetsExportTitle => 'スニペットを書き出す';

  @override
  String get snippetsFieldBuilderTitle => 'フィールドを挿入';

  @override
  String get snippetsFieldDefault => '既定値';

  @override
  String get snippetsFieldName => '名前';

  @override
  String get snippetsFieldOptions => '選択肢 (カンマ区切り)';

  @override
  String get snippetsFieldRequired => 'この項目を入力してください。';

  @override
  String get snippetsFieldType => '種類';

  @override
  String snippetsFieldTypeName(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'text': '1 行',
      'multiline': '複数行',
      'choice': '選択',
      'other': '1 行',
    });
    return '$_temp0';
  }

  @override
  String get snippetsFormattingNotice => '保存すると書式は削除されます。';

  @override
  String get snippetsGoToSnippet => 'スニペットへ移動';

  @override
  String get snippetsImport => '読み込む';

  @override
  String get snippetsImportAbbreviationPolicy => '略語が使用済み';

  @override
  String snippetsImportAbbreviationPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'importWithout': '略語なしで読み込む',
      'renameWithSuffix': '略語に番号を付ける',
      'skip': 'スキップ',
      'other': '略語なしで読み込む',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportCollectionPolicy => '同じ名前のコレクション';

  @override
  String snippetsImportCollectionPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'merge': '既存のものに追加',
      'createNew': '新しく作成',
      'other': '既存のものに追加',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportConvert => '既知のプレースホルダーを変換';

  @override
  String snippetsImportCounts(
    int added,
    int identical,
    int updated,
    int conflicts,
    int invalid,
  ) {
    return '新規: $added。同一 (スキップ): $identical。変更: $updated。略語の競合: $conflicts。無効: $invalid。';
  }

  @override
  String get snippetsImportDone => 'スニペットを読み込みました';

  @override
  String get snippetsImportFailed => '読み込みに失敗しました';

  @override
  String get snippetsImportFileType => 'スニペットファイル';

  @override
  String get snippetsImportFormat => '形式';

  @override
  String snippetsImportFormatName(String format) {
    String _temp0 = intl.Intl.selectLogic(format, {
      'bundle': 'CopyPaste バンドル',
      'json': 'JSON リスト',
      'csv': 'CSV',
      'other': 'CopyPaste バンドル',
    });
    return '$_temp0';
  }

  @override
  String snippetsImportInvalidRow(int row, String reason) {
    return '$row 行目: $reason';
  }

  @override
  String get snippetsImportInvalidRows => '読み込まれない行';

  @override
  String get snippetsImportPassphraseHint => 'パスフレーズ (ファイルが暗号化されている場合)';

  @override
  String get snippetsImportPreview => 'プレビュー';

  @override
  String snippetsImportReport(
    int inserted,
    int updated,
    int skipped,
    int flagged,
    int unrecognized,
  ) {
    return '追加: $inserted。更新: $updated。スキップ: $skipped。確認が必要な略語: $flagged。認識できないプレースホルダー: $unrecognized。';
  }

  @override
  String get snippetsImportSameIdPolicy => '同じスニペットで内容が異なる';

  @override
  String snippetsImportSameIdPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'keepBoth': '両方を残す',
      'replace': '既存のものを置き換える',
      'skip': 'スキップ',
      'other': '両方を残す',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportTitle => 'スニペットを読み込む';

  @override
  String get snippetsInsert => '挿入';

  @override
  String get snippetsInsertPlaceholder => 'プレースホルダーを挿入';

  @override
  String get snippetsKeepReferences => 'そのままにする';

  @override
  String get snippetsLibraryDescription => 'スニペットとコレクションを閲覧、整理、編集します。';

  @override
  String get snippetsLibraryKeywords => 'スニペット テンプレート 略語 コレクション テキスト 定型文';

  @override
  String get snippetsLibraryTitle => 'スニペットライブラリ';

  @override
  String snippetsLint(String code, String detail) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'unclosed': 'このプレースホルダーが閉じられていません。',
      'malformed': 'このプレースホルダーを読み取れません。',
      'unknownPlaceholder': '不明なプレースホルダー「$detail」。入力どおりに貼り付けられます。',
      'unknownParameter': '不明なオプション「$detail」。',
      'duplicateParameter': 'オプション「$detail」が 2 回指定されています。',
      'missingParameter': 'オプション「$detail」がありません。',
      'invalidValue': 'オプション「$detail」の値が無効です。',
      'conflictingParameters': 'これらのオプションは同時に使用できません。',
      'unknownLocale': '不明な言語です。',
      'unknownTimeZone': '不明なタイムゾーンです。',
      'invalidPattern': '日付のパターンが無効です。',
      'dateOutOfRange': '日付が範囲外です。',
      'unknownModifier': '不明な修飾子「$detail」。',
      'modifierFailed': '修飾子「$detail」を適用できませんでした。',
      'emptyClipboard': 'クリップボードの項目 $detail は空です。',
      'extraCursor': '最初のカーソルプレースホルダーのみが使用されます。',
      'nestedCursor': '挿入されたスニペット内のカーソルは無視されます。',
      'tooManyFields': 'フィールド「$detail」は 10 個の上限を超えています。',
      'conflictingField': 'フィールド「$detail」が異なる設定で 2 回定義されています。',
      'choiceDefaultNotOption': '「$detail」の既定値が選択肢にありません。',
      'cycle': 'スニペットが互いを循環して挿入しています: $detail。',
      'depthExceeded': 'スニペットが 3 階層を超えて挿入されています。',
      'nestingLimit': '挿入されたスニペットが多すぎます。',
      'unknownSnippet': 'この略語のスニペットはありません。',
      'containsPlaceholders': 'テキストにプレースホルダーが含まれています。入力するにはテンプレートをオンにしてください。',
      'other': 'このプレースホルダーに問題があります。',
    });
    return '$_temp0';
  }

  @override
  String get snippetsLoadFailed => 'スニペットを読み込めませんでした';

  @override
  String get snippetsLoadMore => 'さらに読み込む';

  @override
  String get snippetsMerge => '統合';

  @override
  String get snippetsMergeCollection => '別のコレクションに統合';

  @override
  String snippetsMergeCollectionTitle(String name) {
    return '「$name」の統合先';
  }

  @override
  String get snippetsMoveTo => '移動先';

  @override
  String get snippetsNewCollection => '新規コレクション';

  @override
  String get snippetsNewSnippet => '新規スニペット';

  @override
  String get snippetsNewText => '新規テキスト';

  @override
  String snippetsNoMatch(String query) {
    return '「$query」に一致するスニペットはありません';
  }

  @override
  String get snippetsPassphraseConfirm => 'パスフレーズをもう一度入力';

  @override
  String snippetsPassphraseHint(int min) {
    return 'パスフレーズ ($min 文字以上)';
  }

  @override
  String get snippetsPassphraseLabel => 'パスフレーズ';

  @override
  String get snippetsPassphraseNoRecovery => '忘れたパスフレーズは復元できません。';

  @override
  String snippetsPassphraseStrength(String score) {
    String _temp0 = intl.Intl.selectLogic(score, {
      'tooShort': '短すぎます',
      'weak': '弱い',
      'fair': '普通',
      'good': '良い',
      'other': '強い',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPinnedSwitch => 'ピン留め';

  @override
  String get snippetsPlaceholderGroup => 'プレースホルダー';

  @override
  String snippetsPlaceholderKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'today': '日付',
      'dateShort': '日付 (短い形式)',
      'dateLong': '日付 (長い形式)',
      'dateIso': '日付 (ISO 8601)',
      'dateNextWeek': '1 週間後の日付',
      'now': '時刻',
      'dateTime': '日付と時刻',
      'clipboard': 'クリップボード',
      'clipboardPrevious': '前のクリップボード項目',
      'field': '入力フィールド…',
      'cursor': 'カーソル位置',
      'uuid': 'UUID',
      'random': '乱数',
      'upper': '修飾子: 大文字',
      'lower': '修飾子: 小文字',
      'trim': '修飾子: 空白を削除',
      'other': 'プレースホルダー',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPlaceholderNestedGroup => '別のスニペットを挿入';

  @override
  String get snippetsPlaceholderNoMatch => '一致するプレースホルダーはありません';

  @override
  String get snippetsPreviewEmpty => 'プレビューする内容がありません';

  @override
  String get snippetsPreviewTab => 'プレビュー';

  @override
  String get snippetsPurge => '完全に削除';

  @override
  String get snippetsPurgeMessage => 'すべてのデバイスから消去されます。この操作は元に戻せません。';

  @override
  String snippetsPurgeTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のスニペットを完全に削除しますか?',
      one: 'スニペットを完全に削除しますか?',
    );
    return '$_temp0';
  }

  @override
  String get snippetsRecentlyDeletedEmpty => '「最近削除した項目」は空です';

  @override
  String get snippetsRecordPastedDescription => '貼り付けたスニペットを履歴に追加します。';

  @override
  String get snippetsRecordPastedKeywords => '履歴 記録 貼り付け スニペット';

  @override
  String get snippetsRecordPastedTitle => '貼り付けたスニペットを履歴に記録';

  @override
  String get snippetsReplaceOriginal => '元のクリップを置き換える';

  @override
  String get snippetsReplaceOriginalHint => '保存後に元のクリップを削除します。';

  @override
  String get snippetsRequireAuthDescription => '先にパスワードまたは生体認証を求めます。';

  @override
  String get snippetsRequireAuthKeywords => '非表示 パスワード 生体認証 指紋 顔 ロック 本人確認';

  @override
  String get snippetsRequireAuthTitle => '非表示スニペットの使用前に本人確認';

  @override
  String get snippetsRestore => '復元';

  @override
  String get snippetsSaveBlockTooLarge => 'テキストが 64 KB を超えています。';

  @override
  String get snippetsSaveFailed => '保存できませんでした';

  @override
  String get snippetsScopeAll => 'すべて';

  @override
  String get snippetsScopeEmpty => 'スニペットはありません';

  @override
  String get snippetsScopePinned => 'ピン留め';

  @override
  String get snippetsScopeRecentlyDeleted => '最近削除した項目';

  @override
  String get snippetsSearchPlaceholder => 'スニペットを検索';

  @override
  String snippetsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件選択',
    );
    return '$_temp0';
  }

  @override
  String get snippetsShow => '表示';

  @override
  String get snippetsShowAll => 'すべて表示';

  @override
  String get snippetsShowAttention => '確認';

  @override
  String snippetsSoftLimit(int count) {
    return 'スニペットが $count 件あります。上限は 20,000 件です。';
  }

  @override
  String get snippetsTemplateBadge => 'テンプレート';

  @override
  String get snippetsTemplateSwitch => 'テンプレート';

  @override
  String get snippetsTitleLabel => 'タイトル';

  @override
  String get snippetsTitlePlaceholder => '省略可';

  @override
  String get snippetsTransferDescription => 'スニペットをバックアップしたり、アプリ間で移したりします。';

  @override
  String get snippetsTransferKeywords =>
      '読み込み 書き出し インポート エクスポート バックアップ json csv パスフレーズ';

  @override
  String get snippetsTransferTitle => '読み込みと書き出し';

  @override
  String get snippetsUndo => '元に戻す';

  @override
  String get snippetsUnfiled => '未分類';

  @override
  String snippetsUnfilledWarning(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 個のプレースホルダーが入力どおりに貼り付けられます',
    );
    return '$_temp0';
  }

  @override
  String get snippetsUnpin => 'ピン留めを解除';

  @override
  String get snippetsUnsavedMessage => '閉じる前に変更を保存しますか?';

  @override
  String get snippetsUnsavedTitle => '未保存の変更';

  @override
  String get snippetsUntitled => '無題のスニペット';

  @override
  String get snippetsUpdateReferences => '更新';

  @override
  String snippetsUpdateReferencesMessage(String from, String to) {
    return 'ほかのテンプレートが「$from」を挿入している可能性があります。「$to」を使うように更新しますか?';
  }

  @override
  String get snippetsUpdateReferencesTitle => '参照を更新しますか?';

  @override
  String get snippetsUseFailed => 'スニペットを使用できませんでした';

  @override
  String get quickPasteTabQueue => 'キュー';

  @override
  String get quickPasteMaskedMode => 'パスワード欄: プレビューは非表示です';

  @override
  String quickPasteMaskedItem(String kind) {
    return '非表示: $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'シークレット項目のロックを解除';

  @override
  String quickPastePastingWith(String name) {
    return '貼り付けに使用: $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut で元のまま貼り付け';
  }

  @override
  String get quickPasteDefaultSkipped => '既定の変換をスキップしました';

  @override
  String get quickPasteDefaultSkippedMessage => '代わりに元の内容を使用しました。';

  @override
  String get quickPastePasteManually => '内容はクリップボードにあります。手動で貼り付けてください。';

  @override
  String get quickPastePaste => '貼り付け';

  @override
  String get quickPasteSnippetsUnavailable => 'スニペットを利用できません';

  @override
  String get rulesLoadFailed => 'コンテンツルールを読み込めませんでした。';

  @override
  String get rulesNewRule => '新しいルール';

  @override
  String get rulesEditRule => 'ルールを編集';

  @override
  String get rulesUntitled => '名前のないルール';

  @override
  String get rulesPrivacyDefaultsTitle => 'プライバシーの既定値';

  @override
  String get rulesPrivacyDefaultsDescription => 'プライバシー設定はどのルールよりも先に適用されます。';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'プライバシー 非表示 一時 除外アプリ 組み込み 認証情報 パスワード カード';

  @override
  String get rulesOpenPrivacy => 'プライバシーを開く';

  @override
  String get rulesMyRulesTitle => 'マイルール';

  @override
  String get rulesMyRulesDescription => '新しいコピーに対して順に実行されます。';

  @override
  String get rulesMyRulesKeywords => 'ルール 自動化 タグ スキップ シークレット 期限 削除 パターン 正規表現';

  @override
  String get rulesEmptyTitle => 'ルールはまだありません';

  @override
  String get rulesEmptyMessage => 'ルールは新しいコピーにタグ付け、保護、期限設定、スキップを行います。';

  @override
  String get rulesDeletedTitle => '最近削除したルール';

  @override
  String get rulesDeletedDescription => '30 日以内なら復元できます。';

  @override
  String rulesDeletedAt(String time) {
    return '削除日時: $time';
  }

  @override
  String get rulesRestore => '復元';

  @override
  String get rulesDeletedToast => 'ルールを削除しました';

  @override
  String get rulesDeletedToastMessage => '30 日間復元できます。';

  @override
  String get rulesUndo => '元に戻す';

  @override
  String rulesConflictMessage(String rule, String device) {
    return '$device で同時に行われた「$rule」の編集は、新しい編集に置き換えられました。';
  }

  @override
  String get rulesAnotherDevice => '別のデバイス';

  @override
  String get rulesRestoreConflict => 'その版を使用';

  @override
  String get rulesSkipLogTitle => '最近スキップ';

  @override
  String get rulesSkipLogDescription => 'このデバイスでルールにより保存されなかったコピーです。';

  @override
  String get rulesSkipLogKeywords => 'スキップ 無視 保存されない ログ';

  @override
  String get rulesSkipLogShow => '表示';

  @override
  String get rulesSkipLogEmpty => 'スキップされたコピーはありません。';

  @override
  String get rulesUnknownApp => '不明なアプリ';

  @override
  String get rulesMatchesEverything => '任意のコピー';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary、その後停止';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': ' または ',
      'other': ' かつ ',
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
    return '$action: $value';
  }

  @override
  String rulesRunsOn(String devices) {
    return '実行先: $devices';
  }

  @override
  String rulesNewFromDevice(String device) {
    return '$device から新規';
  }

  @override
  String get rulesNeedsUpdateBadge => '要アップデート';

  @override
  String get rulesUpdateRequired => 'このルールを実行するには CopyPaste をアップデートしてください。';

  @override
  String get rulesErrorBadge => 'エラー';

  @override
  String rulesErrorHint(String problem) {
    return '$problem 修正されるまで、一致したコピーは保護されます。';
  }

  @override
  String get rulesReorder => 'ドラッグして並べ替え';

  @override
  String get rulesMoreActions => 'その他の操作';

  @override
  String get rulesEdit => '編集';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': '上へ移動',
      'down': '下へ移動',
      'top': '先頭へ移動',
      'other': '末尾へ移動',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => '既存の項目に適用…';

  @override
  String get rulesResetBuiltin => 'デフォルトに戻す';

  @override
  String get rulesNameLabel => '名前';

  @override
  String get rulesNamePlaceholder => '例: 仕事用リンク';

  @override
  String get rulesWhenTitle => '条件';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'いずれかの条件に一致',
      'other': 'すべての条件に一致',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions => '条件なし: このルールはすべてのコピーに適用されます。';

  @override
  String get rulesAddCondition => '条件を追加';

  @override
  String get rulesRemoveCondition => '条件を削除';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'テキスト',
      'content_type': 'コンテンツの種類',
      'detected_kind': '検出された種類',
      'source_app': 'コピー元アプリ',
      'size': 'サイズ',
      'privacy_marker': 'プライバシーマーカー',
      'capture_origin': '取得元',
      'other': '不明な条件',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'を含む',
      'equals': 'と等しい',
      'starts_with': 'で始まる',
      'ends_with': 'で終わる',
      'matches_regex': 'がパターンに一致',
      'other': 'に一致',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'を含まない',
      'equals': 'と等しくない',
      'starts_with': 'で始まらない',
      'ends_with': 'で終わらない',
      'matches_regex': 'がパターンに一致しない',
      'other': 'に一致しない',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'が次以外',
      'other': 'が次のいずれか',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'が次より小さい',
      'other': 'が次より大きい',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'が次以上',
      'other': 'が次以下',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': '行',
      'bytes': 'バイト',
      'other': '文字',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': '行',
      'bytes': 'バイト',
      'other': '文字',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'プレーンテキスト',
      'rich_text': 'リッチテキスト',
      'image': '画像',
      'files': 'ファイル',
      'color': 'カラー',
      'link': 'リンク',
      'other': 'その他',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'パスワードまたはシークレット',
      'url': 'Web アドレス',
      'email': 'メールアドレス',
      'phone': '電話番号',
      'code': 'コード',
      'color': 'カラー値',
      'json': 'JSON',
      'file_path': 'ファイルパス',
      'number': '数値',
      'one_time_code': 'ワンタイムコード',
      'payment_card': 'カード番号',
      'other': 'その他',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': '非表示',
      'transient': '一時的',
      'auto_generated': '自動生成',
      'excluded_from_history': '履歴から除外',
      'other': 'その他',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'このデバイスでコピー',
      'received': '別のデバイスから受信',
      'created_in_app': 'CopyPaste で作成',
      'other': 'その他',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => '検索するテキスト';

  @override
  String get rulesPatternPlaceholder => 'パターン (例: ^ticket-[0-9]+)';

  @override
  String get rulesPatternHint => '先読み・後読みと後方参照には対応していません。';

  @override
  String get rulesCaseSensitive => '大文字と小文字を区別';

  @override
  String get rulesChooseValues => '選択';

  @override
  String get rulesUnknownConditionNotice =>
      'この条件は新しいバージョンの CopyPaste で作成されました。';

  @override
  String get rulesSourceAppNotice => 'Android では、クリップキーボードでのコピーのみ一致します。';

  @override
  String get rulesPrivacyMarkerNotice => 'すべてのシステムがすべてのマーカーを報告するわけではありません。';

  @override
  String get rulesThenTitle => 'アクション';

  @override
  String get rulesNoActions => 'アクションを 1 つ以上追加してください。';

  @override
  String get rulesAddAction => 'アクションを追加';

  @override
  String get rulesRemoveAction => 'アクションを削除';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': '保存しない',
      'mark_secret': 'シークレットにする',
      'delete_after': '次の時間後に削除',
      'add_tags': 'タグを追加',
      'transform': '変換',
      'other': '不明なアクション',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone': '「保存しない」は他のアクションと組み合わせられません。',
      'present': 'このルールにはすでにこのアクションがあります。',
      'other': '1 つのルールに設定できるアクションは 8 個までです。',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'このアクションは新しいバージョンの CopyPaste で作成されました。';

  @override
  String get rulesDontSaveWarning => '一致したコピーは保存、インデックス作成、同期されません。';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 分',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 時間',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 日',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry => 'クリップボードにこのコピーが残っていれば、それも消去します。';

  @override
  String get rulesNoTags => '履歴でタグを作成すると、ここで使用できます。';

  @override
  String get rulesNoTransforms => '利用できる変換はありません。';

  @override
  String get rulesReplaceClipboard => 'クリップボードも結果に置き換える';

  @override
  String get rulesOptionsTitle => 'オプション';

  @override
  String get rulesEnabledSwitch => '有効';

  @override
  String get rulesStopProcessing => 'このルールが一致したら以降のルールを処理しない';

  @override
  String get rulesRunsOnLabel => '実行するデバイス';

  @override
  String get rulesAllDevices => 'すべてのデバイス';

  @override
  String get rulesChosenDevices => '選択したデバイス';

  @override
  String get rulesTestTitle => 'テスト';

  @override
  String get rulesTestDescription => 'サンプルテキストでルールを試します。';

  @override
  String get rulesSamplePlaceholder => 'サンプルテキストを貼り付けるか入力';

  @override
  String get rulesTestFullSet => 'すべてのルールと一緒にテスト';

  @override
  String get rulesRunTest => 'テストを実行';

  @override
  String get rulesTestFailed => 'テストを実行できませんでした。';

  @override
  String get rulesPreviewHistory => '履歴でプレビュー';

  @override
  String get rulesSaveFailed => 'ルールを保存できませんでした。';

  @override
  String get rulesReadOnlyNotice =>
      '新しいバージョンの CopyPaste で作成されました。編集・実行するにはアップデートしてください。';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'ルールを読み取れませんでした。',
      'unsupported_schema': 'このルールには新しいバージョンの CopyPaste が必要です。',
      'unknown_condition_type': 'この条件には新しいバージョンの CopyPaste が必要です。',
      'unknown_action_type': 'このアクションには新しいバージョンの CopyPaste が必要です。',
      'too_many_conditions': '1 つのルールに設定できる条件は 20 個までです。',
      'too_many_actions': '1 つのルールに設定できるアクションは 8 個までです。',
      'too_many_rules': '実行されるのは最初の 200 個のルールだけです。',
      'no_actions': 'アクションを 1 つ以上追加してください。',
      'conflicting_actions': '「保存しない」は他のアクションと組み合わせられません。',
      'duplicate_action': '各アクションは 1 回だけ使用できます。',
      'empty_value': '値を入力してください。',
      'empty_list': '値を 1 つ以上選択してください。',
      'too_many_values': '選択できる値は 64 個までです。',
      'invalid_value': 'この値は無効です。',
      'pattern_too_long': 'パターンは 2,000 文字以内にしてください。',
      'unsupported_look_around':
          '先読みと後読みには対応していません。代わりに「すべての条件に一致」で 2 つの条件を使用してください。',
      'unsupported_backreference': '後方参照には対応していません。',
      'pattern_too_large': 'このパターンは複雑すぎます。繰り返しを簡単にしてください。',
      'invalid_pattern': 'このパターンは無効です。',
      'invalid_duration': '1 分から 90 日の期間を選択してください。',
      'invalid_tags': 'タグを 1 つ以上選択してください。',
      'invalid_transform': '変換を選択してください。',
      'other': 'このルールには問題があります。',
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
      'matched': '一致',
      'not_matched': '不一致',
      'unknown': '不明',
      'skipped': '未実行',
      'other': 'エラー',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return '条件 $number';
  }

  @override
  String get rulesTraceNoRules => '評価されたルールはありません。';

  @override
  String get rulesTraceStopped => '以降のルールは実行されませんでした。';

  @override
  String get rulesTraceUnknownReason => 'このコピーでは不明なため、一致しません。';

  @override
  String get rulesTraceRuleError => 'このルールを実行できなかったため、一致したコピーは保護されます。';

  @override
  String get rulesTraceOutcomeTitle => '結果';

  @override
  String get rulesTraceNoEffect => 'このコピーを変更するルールはありません。';

  @override
  String get rulesTraceSkipped => '保存されない';

  @override
  String get rulesTraceSecret => 'シークレットとして保存';

  @override
  String rulesTraceExpires(String duration) {
    return '$duration後に削除';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'タグ: $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return '変換: $names';
  }

  @override
  String get rulesTraceOutput => '変換後のテキスト';

  @override
  String get rulesPreviewTitle => '既存の項目に適用';

  @override
  String rulesPreviewRunning(int count) {
    return '最新の $count 件のクリップを確認しています…';
  }

  @override
  String get rulesPreviewNone => 'このルールに一致する最近のクリップはありません。';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のクリップが一致',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件が削除されます',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件がシークレットになります',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件に削除時刻が設定されます',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件にタグが付きます',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return '確認したのは最新の $count 件のクリップだけです。';
  }

  @override
  String rulesPreviewMore(int count) {
    return 'ほか $count 件';
  }

  @override
  String get rulesPreviewNote => '適用するまで何も変わりません。';

  @override
  String get rulesSecretItem => 'シークレット項目';

  @override
  String rulesSecretItemFrom(String app) {
    return '$app のシークレット項目';
  }

  @override
  String get rulesPreviewCancelled => 'プレビューを停止しました。何も変更されていません。';

  @override
  String get rulesPreviewFailed => 'プレビューを実行できませんでした。';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のクリップを削除しますか？',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage => '一致するクリップがすべてのデバイスから削除されます。元に戻せません。';

  @override
  String get rulesApplyDeleteConfirm => '削除して適用';

  @override
  String rulesApplyProgress(int done, int total) {
    return '適用中: $done / $total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のクリップを変更しました',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のクリップを削除しました',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のクリップを変更できませんでした',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled => '停止しました。ここまでの変更は保持されます。';

  @override
  String get historyActionEdit => '編集';

  @override
  String get historyActionSaveAsSnippet => 'スニペットとして保存';

  @override
  String get historyActionAddToQueue => 'Paste Queue に追加';

  @override
  String get historyActionTransform => '変換…';

  @override
  String get historyActionExplain => 'これがここにある理由';

  @override
  String get historyActionKeep => '保持';

  @override
  String get historyNewClip => '新規クリップ';

  @override
  String get historySavedTitle => '保存しました';

  @override
  String get historyEditSaved => '編集したテキストは履歴の先頭に新しいクリップとして追加されました。';

  @override
  String get historyNewClipSaved => 'テキストを保存しました。';

  @override
  String get historySnippetSaved => 'クリップのテキストをスニペットにしました。';

  @override
  String get historySnippetSaveFailed => 'このクリップをスニペットとして保存できませんでした。';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のクリップをキューに追加しました。',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed =>
      'Paste Queue に追加できませんでした。オンになっているか確認してください。';

  @override
  String get historyKeepFailed => 'このクリップを保持できませんでした。';

  @override
  String get historyExplainFailed => '説明を読み込めませんでした。';

  @override
  String get historyDeletesSoon => '1 分以内に削除';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 分後に削除',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 時間後に削除',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 日後に削除',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => '手動';

  @override
  String get historyProvenanceEdited => '編集済み';

  @override
  String get historyProvenanceTransformed => '変換済み';

  @override
  String get historyProvenanceModule => 'モジュール';

  @override
  String historyRuleTag(String name) {
    return '$name（自動）';
  }

  @override
  String get historyMetadataExpiry => '削除';

  @override
  String get historyMetadataRuleTags => '自動タグ';

  @override
  String modulesEnableTitle(String module) {
    return '$module を有効にしますか？';
  }

  @override
  String get modulesEnableAccess => 'このモジュールでできること:';

  @override
  String modulesCapability(String capability) {
    String _temp0 = intl.Intl.selectLogic(capability, {
      'history_read': 'クリップボード履歴を読む',
      'clipboard_write': 'クリップボードにコピーする',
      'library_write': 'スニペットを保存し項目をピン留めする',
      'snippets': 'スニペットを読む',
      'synced_records': 'デバイス間でデータを同期する',
      'local_state': 'このデバイスにデータを保存する',
      'prompts': '操作の確認を求める',
      'endpoints': 'ローカルアプリからの接続を受け付ける',
      'other': 'CopyPaste のサービスを使う',
    });
    return '$_temp0';
  }

  @override
  String modulesPromptFrom(String module) {
    return '$module から';
  }

  @override
  String get modulesUnknownModule => '不明なモジュール';

  @override
  String get modulesPromptAnswerFailed => '回答を送信できませんでした。';

  @override
  String get modulesPromptNotificationBody => 'CopyPaste を開いて回答してください。';

  @override
  String get modulesUnknownDevice => '不明なデバイス';

  @override
  String get modulesChooseDevices => 'デバイスを選択';

  @override
  String get modulesAddApp => 'アプリを追加';

  @override
  String modulesRemoveApp(String app) {
    return '$app を削除';
  }

  @override
  String get modulesNoApps => 'アプリが選択されていません';
}
