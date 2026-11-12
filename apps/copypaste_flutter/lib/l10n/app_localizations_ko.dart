// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Korean (`ko`).
class AppLocalizationsKo extends AppLocalizations {
  AppLocalizationsKo([String locale = 'ko']) : super(locale);

  @override
  String get commonCancel => '취소';

  @override
  String get commonClose => '닫기';

  @override
  String get commonDone => '완료';

  @override
  String get commonRetry => '다시 시도';

  @override
  String get commonTryAgain => '다시 시도';

  @override
  String get commonCopy => '복사';

  @override
  String get commonDelete => '삭제';

  @override
  String get commonSave => '저장';

  @override
  String get commonContinue => '계속';

  @override
  String get commonBack => '뒤로';

  @override
  String get commonOpenSettings => '설정 열기';

  @override
  String get commonLoading => '불러오는 중';

  @override
  String get commonApply => '적용';

  @override
  String get commonClear => '지우기';

  @override
  String get commonPin => '고정';

  @override
  String get commonEnable => '켜기';

  @override
  String get settingsLanguageTitle => '언어';

  @override
  String get settingsLanguageDescription => 'CopyPaste 인터페이스의 언어를 선택합니다.';

  @override
  String get settingsLanguageSystem => '시스템 기본값';

  @override
  String get settingsLanguageKeywords =>
      '언어 로케일 번역 인터페이스 language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      '로그인 시 시작 설정을 업데이트할 수 없습니다. 다시 시도하십시오.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed => '시스템 시작 설정을 열 수 없습니다.';

  @override
  String get settingsRuntimeRefreshFailed => '설정을 새로 고칠 수 없습니다. 다시 시도하십시오.';

  @override
  String get settingsRuntimeLoadFailed => '설정을 불러올 수 없습니다.';

  @override
  String get settingsRuntimeAppAlreadyExcluded => '이미 제외된 앱입니다.';

  @override
  String get settingsRuntimeChangeFailed => '요청한 변경 사항을 완료할 수 없습니다.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      '수집을 재개하려면 먼저 클립보드 수집 권한 설정을 완료해야 합니다.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      '클립보드 수집이 일시 정지되었습니다. Android 수집 서비스가 중지되었는지 확인되지 않았습니다.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      '수집 알림을 켜려면 알림 권한이 필요합니다.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste가 저장된 설정을 읽을 수 없습니다. 보호 값이 적용되어 있습니다. 환경설정을 검토하고 저장하여 복구하십시오.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste가 읽을 수 없는 설정을 보호 값으로 대체했습니다. 환경설정을 검토하고 저장하여 복구하십시오.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      '앱 식별자는 비어 있지 않아야 하며 256바이트 이하여야 합니다.';

  @override
  String get settingsRuntimeExcludedAppsLimit => '최대 256개의 앱을 제외할 수 있습니다.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      '자동 붙여넣기 설정을 업데이트할 수 없습니다.';

  @override
  String get settingsRuntimeShortcutRegisterFailed => '해당 단축키를 등록할 수 없습니다.';

  @override
  String get settingsRuntimeShortcutPauseFailed => '현재 단축키를 일시 정지할 수 없습니다.';

  @override
  String get settingsRuntimeShortcutRestoreFailed => '단축키를 복원할 수 없습니다.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed => 'Quick Paste를 열 수 없습니다.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      '손쉬운 사용 권한을 요청할 수 없습니다.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Linux 통합 상태를 새로 고칠 수 없습니다.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      '클립보드 입력 권한이 허용되지 않았습니다.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      '클립보드 입력 권한을 요청할 수 없습니다.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      '데스크톱 통합 설정을 열 수 없습니다.';

  @override
  String get settingsRuntimeTextHistoryFileType => 'CopyPaste 텍스트 기록';

  @override
  String get settingsRuntimeEncryptedBackupFileType => 'CopyPaste 암호화된 백업';

  @override
  String get settingsRuntimeNotificationImage => '이미지';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return '이미지 · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => '파일';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste가 업데이트 설치 상태를 복원할 수 없습니다.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      '이 Linux 설치본은 자동으로 업데이트할 수 없습니다.';

  @override
  String get updateMacosPackageMissing => '이 릴리스에는 서명된 macOS 업데이트 패키지가 없습니다.';

  @override
  String get updateInstalledVersionInvalid => '설치된 CopyPaste 버전이 유효하지 않습니다.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste가 업데이트를 확인하기 위해 GitHub에 연결할 수 없습니다.';

  @override
  String get updateCheckTimedOut => '업데이트 확인 시간이 초과되었습니다.';

  @override
  String get updateCheckDeviceFailed => 'CopyPaste가 이 기기에서 업데이트를 확인할 수 없습니다.';

  @override
  String get updateCheckFailed => 'CopyPaste가 업데이트를 확인할 수 없습니다.';

  @override
  String get updateDownloadFailed => 'CopyPaste가 업데이트를 다운로드할 수 없습니다.';

  @override
  String get updateDownloadTimedOut => '업데이트 다운로드 시간이 초과되었습니다.';

  @override
  String get updateInstallFailed => 'CopyPaste가 업데이트를 설치할 수 없습니다.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Android 시스템 설치 프로그램에서 계속하십시오.';

  @override
  String get updateContinueInPackageManager => '시스템 패키지 관리자에서 계속하십시오.';

  @override
  String get updateInstallerStarting => '설치 프로그램을 시작하는 중입니다.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Android 설정에서 CopyPaste의 앱 설치를 허용한 다음 계속하십시오.';

  @override
  String get updatePermissionRequiredLinux =>
      '이 업데이트를 설치하려면 시스템 패키지 관리자에서 인증하십시오.';

  @override
  String get updatePermissionRequired =>
      'CopyPaste가 이 업데이트를 설치하도록 허용한 다음 계속하십시오.';

  @override
  String get updateRestartRequired =>
      '업데이트가 설치되었습니다. 업데이트를 사용하려면 CopyPaste를 다시 시작하십시오.';

  @override
  String get updateRestarting => 'CopyPaste를 다시 시작하는 중입니다.';

  @override
  String get updateRestartFailed => 'CopyPaste를 다시 시작할 수 없습니다. 다시 시도하십시오.';

  @override
  String get updateReleasePageOpenFailed => 'CopyPaste가 릴리스 페이지를 열 수 없습니다.';

  @override
  String get updatePackageInvalid => '다운로드한 업데이트가 유효한 CopyPaste 패키지가 아닙니다.';

  @override
  String get updateSignatureInvalid => '다운로드한 업데이트의 서명이 유효하지 않습니다.';

  @override
  String get updateDowngradeRefused => '다운로드한 패키지가 현재 버전보다 최신이 아닙니다.';

  @override
  String get updateHomebrewRequired =>
      '여기에서 업데이트하려면 Homebrew로 CopyPaste를 설치하십시오.';

  @override
  String get updateInstallationCancelled => '업데이트 설치가 취소되었습니다.';

  @override
  String get updateInstallationBlocked => 'Android가 업데이트 설치를 차단했습니다.';

  @override
  String get updateInstallationConflict => '업데이트가 설치된 앱과 충돌합니다.';

  @override
  String get updateInstallationIncompatible => '업데이트가 이 기기와 호환되지 않습니다.';

  @override
  String get updateInstallationStorage => '업데이트를 설치하려면 저장 공간을 확보하십시오.';

  @override
  String get updateInstallationInterrupted => '업데이트 설치가 중단되었습니다. 다시 시도하십시오.';

  @override
  String get updateInstallationBusy => '업데이트 설치가 이미 진행 중입니다.';

  @override
  String get updatePackageDetailsInvalid => '업데이트 패키지 정보가 유효하지 않습니다.';

  @override
  String get updateIntegrityCheckFailed => '다운로드한 업데이트가 무결성 검사를 통과하지 못했습니다.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste가 시스템 설치 프로그램을 시작할 수 없습니다.';

  @override
  String get updateInstallLocationTranslocated =>
      '업데이트하려면 CopyPaste를 응용 프로그램 폴더로 이동한 다음 그곳에서 여십시오.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste가 현재 폴더에서 자신을 교체할 수 없습니다. 릴리스 페이지를 사용하여 업데이트하십시오.';

  @override
  String get updateReplacementBlocked =>
      'macOS가 CopyPaste 교체를 차단했습니다. 앱 관리 설정에서 허용하거나 릴리스 페이지를 사용하십시오.';

  @override
  String get updatePackageNotDownloadable =>
      '이 플랫폼은 다운로드 가능한 업데이트 패키지를 사용하지 않습니다.';

  @override
  String get updatePackageSizeInvalid => '업데이트 패키지 크기가 유효하지 않습니다.';

  @override
  String get updateUpdaterSignatureInvalid => '다운로드한 업데이트의 업데이터 서명이 유효하지 않습니다.';

  @override
  String get updatePackageUrlUntrusted => '업데이트 패키지 URL을 신뢰할 수 없습니다.';

  @override
  String get updateDownloadLeftGitHub => '업데이트 다운로드가 GitHub 외부로 이동했습니다.';

  @override
  String get updateDownloadTooManyRedirects =>
      '업데이트 다운로드에서 리디렉션이 너무 많이 발생했습니다.';

  @override
  String get updatePackageTooLarge => '업데이트 패키지가 너무 큽니다.';

  @override
  String get updateSignatureSizeInvalid => '업데이터 서명 크기가 유효하지 않습니다.';

  @override
  String get updateMetadataInvalid => 'GitHub에서 유효하지 않은 업데이트 메타데이터를 반환했습니다.';

  @override
  String get updateMetadataTooLarge => '업데이트 메타데이터가 너무 큽니다.';

  @override
  String get quickPasteActionFailedTitle => '작업 실패';

  @override
  String get quickPasteOpenAppSemantics => 'CopyPaste 열기';

  @override
  String get quickPasteSearchPlaceholder => '입력하여 검색…';

  @override
  String get quickPasteLoadingHistory => '클립보드 기록을 불러오는 중입니다.';

  @override
  String get quickPasteHistoryUnavailable => '기록을 사용할 수 없음';

  @override
  String get quickPasteNoClipsFound => '클립을 찾을 수 없음';

  @override
  String get quickPasteLoadMore => '더 불러오기';

  @override
  String get quickPasteTransferFailed => '실패';

  @override
  String get quickPasteClipUnavailable => '클립을 사용할 수 없음';

  @override
  String get quickPasteFileFallbackName => '파일';

  @override
  String get quickPastePathUnavailable => '경로를 알 수 없음';

  @override
  String get quickPastePreferences => '환경설정…';

  @override
  String get quickPasteAbout => '정보';

  @override
  String get quickPasteQuit => '종료';

  @override
  String get quickPasteClearHistoryTitle => '기록을 지우시겠습니까?';

  @override
  String get quickPasteClearHistoryMessage => '고정된 클립은 기록에 유지됩니다.';

  @override
  String get quickPasteClearHistoryFailed => '기록을 삭제할 수 없습니다.';

  @override
  String get quickPasteAboutDescription =>
      'macOS, Android, Windows, Linux에서 사용하는 비공개 클립보드 기록.';

  @override
  String get transfersTitle => '다운로드 및 전송';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '진행 중인 전송 $count개',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count개 진행 중',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => '최신';

  @override
  String get transfersShowEarlier => '이전';

  @override
  String get transfersClearCompletedHistory => '완료된 기록 지우기';

  @override
  String get transfersErrorTitle => '전송';

  @override
  String get transfersEmptyTitle => '전송 없음';

  @override
  String get transfersEmptyMessage => '파일 가져오기, 다운로드 및 보내는 전송이 여기에 표시됩니다.';

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
  String get transfersRemoveFromHistory => '기록에서 제거';

  @override
  String get transfersCancelDownload => '다운로드 취소';

  @override
  String get transfersShowFile => '파일 보기';

  @override
  String get transfersPhaseSending => '보내는 중';

  @override
  String get transfersPhaseDownloading => '다운로드 중';

  @override
  String get transfersPhaseAdding => '추가 중';

  @override
  String get transfersPhaseAdded => '추가됨';

  @override
  String get transfersPhaseSent => '보냄';

  @override
  String get transfersPhaseCompleted => '완료됨';

  @override
  String get transfersPhaseCancelled => '취소됨';

  @override
  String get transfersPhaseInterrupted => '중단됨';

  @override
  String get transfersPhaseFailed => '실패';

  @override
  String get transfersUpdatesUnavailable =>
      '전송 업데이트를 사용할 수 없습니다. 새로 고치려면 다시 여십시오.';

  @override
  String get transfersLoadFailed => '전송 목록을 불러올 수 없습니다. 다시 시도하십시오.';

  @override
  String get transfersUpdatesLoadFailed => '전송 업데이트를 불러올 수 없습니다.';

  @override
  String get transfersRetryFailed => '이 파일을 다시 시도할 수 없습니다.';

  @override
  String get transfersClearHistoryFailed => '전송 기록을 지울 수 없습니다.';

  @override
  String get transfersRemoveRecordFailed => '이 기록 항목을 제거할 수 없습니다.';

  @override
  String get transfersCancelFailed => '전송을 취소할 수 없습니다. 아직 진행 중일 수 있습니다.';

  @override
  String get transfersSavedFileUnavailable =>
      '저장된 파일을 사용할 수 없습니다. 이동되었거나 삭제되었을 수 있습니다.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '알림 · 읽지 않음 $count개',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => '알림';

  @override
  String get notificationsClearRead => '읽은 알림 지우기';

  @override
  String get notificationsEmptyTitle => '알림 없음';

  @override
  String get notificationsEmptyMessage => '기기 연결 요청과 그 결과가 여기에 표시됩니다.';

  @override
  String get notificationsSystemAlertTitle => '시스템 알림';

  @override
  String get notificationsReview => '검토';

  @override
  String get notificationsConnectionRequest => '연결 요청';

  @override
  String get notificationsConfirmConnection => '연결 확인';

  @override
  String get notificationsDeviceConnected => '기기 연결됨';

  @override
  String get notificationsConnectionRejected => '연결 거부됨';

  @override
  String get notificationsConnectionCancelled => '연결 취소됨';

  @override
  String get notificationsConnectionRequestExpired => '연결 요청 만료됨';

  @override
  String get notificationsConnectionFailed => '연결 실패';

  @override
  String get notificationsNearbyDevice => '근처 기기';

  @override
  String get notificationsPermissionDenied =>
      '백그라운드에서 요청을 받으려면 OS 설정에서 시스템 알림을 허용하십시오.';

  @override
  String get notificationsSystemUnavailable =>
      '시스템 알림을 사용할 수 없습니다. 연결 요청은 여기에 계속 표시됩니다.';

  @override
  String get platformCaptureStateUnavailable => '사용할 수 없음';

  @override
  String get platformCaptureStatePaused => '일시 정지됨';

  @override
  String get platformCaptureStateRunning => '수집 중';

  @override
  String get platformCaptureStateStopped => '실행 중이 아님';

  @override
  String get platformCaptureActionUnavailable => '수집을 사용할 수 없음';

  @override
  String get platformCaptureActionResume => '수집 재개';

  @override
  String get platformCaptureActionPause => '수집 일시 정지';

  @override
  String get platformCaptureActionResumeStopped => '수집 재개(실행 중이 아님)';

  @override
  String get platformTrayOpen => '열기';

  @override
  String get platformTraySettings => '설정';

  @override
  String get platformTrayQuit => '종료';

  @override
  String get platformWindowControlsUnavailable =>
      '데스크톱 창 제어를 사용할 수 없습니다. CopyPaste가 계속 표시됩니다.';

  @override
  String get platformTrayUnavailable =>
      '시스템 트레이를 사용할 수 없습니다. CopyPaste를 열어 둔 채로 다시 시도하거나 종료하십시오.';

  @override
  String get platformQuitFailed => 'CopyPaste를 종료할 수 없습니다. 다시 시도하십시오.';

  @override
  String get platformVisibilityChangeFailed =>
      '앱 표시 여부를 변경할 수 없습니다. 다시 시도하십시오.';

  @override
  String get platformCaptureNotificationTitle => '클립보드 저장됨';

  @override
  String get platformCaptureNotificationBody => 'CopyPaste에 새 항목이 추가되었습니다.';

  @override
  String get platformPairingNotificationTitle => '연결 요청';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName에서 연결을 요청합니다. CopyPaste를 열어 코드를 비교하고 확인하십시오.';
  }

  @override
  String get platformPairingNotificationChannelName => '기기 연결 요청';

  @override
  String get platformPairingNotificationChannelDescription => '근처 기기의 연결 요청';

  @override
  String get platformNotificationDefaultAction => '열기';

  @override
  String get platformFilesSelectionFailed => '파일을 선택할 수 없습니다.';

  @override
  String get platformFilesOnlyRegular => '일반 파일만 가져올 수 있습니다.';

  @override
  String get platformModulesUnavailable => '이 기기에서는 모듈을 사용할 수 없습니다.';

  @override
  String get platformSmsAccessConfigureFailed => 'SMS 접근 권한을 구성할 수 없습니다.';

  @override
  String get platformSmsAccessVerifyFailed => 'SMS 접근 권한을 확인할 수 없습니다.';

  @override
  String get platformUpdatesUnavailableHere => '여기에서는 앱 업데이트를 사용할 수 없습니다.';

  @override
  String get platformUpdateVersionUnavailable => '설치된 버전을 확인할 수 없습니다.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      '이 설치본에서는 앱 업데이트를 사용할 수 없습니다.';

  @override
  String get platformUpdateLinuxUnsupported =>
      '이 Linux 설치본은 자동으로 업데이트할 수 없습니다.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      '이 Linux 업데이트가 실행 중인 아키텍처와 일치하지 않습니다.';

  @override
  String get platformUpdateMacosUnsupported =>
      '이 macOS 설치본은 자동으로 업데이트할 수 없습니다.';

  @override
  String get platformUpdateStartFailed => 'CopyPaste가 업데이트를 시작할 수 없습니다.';

  @override
  String get platformGlobalShortcutDescription => 'Quick Paste 열기';

  @override
  String get settingsSectionGeneral => '일반';

  @override
  String get settingsSectionGeneralSummary => '언어 및 앱 표시 여부.';

  @override
  String get settingsSectionClipboard => '클립보드';

  @override
  String get settingsSectionClipboardSummary => '수집 및 기록 한도.';

  @override
  String get settingsSectionPrivacy => '개인정보 보호';

  @override
  String get settingsSectionPrivacySummary => '앱 제외 및 화면 보호.';

  @override
  String get settingsSectionQuickPasteSummary => '단축키 및 자동 붙여넣기 동작.';

  @override
  String get settingsSectionSync => '동기화';

  @override
  String get settingsSectionSyncSummary => '동기화 및 근처 기기 검색.';

  @override
  String get settingsSectionNotifications => '알림';

  @override
  String get settingsSectionNotificationsSummary => '수집 알림 및 사운드.';

  @override
  String get settingsSectionData => '데이터';

  @override
  String get settingsSectionDataSummary => '내보내기, 백업 및 복원.';

  @override
  String get settingsSectionModules => '모듈';

  @override
  String get settingsSectionModulesSummary => '선택적 모듈 설치 및 관리.';

  @override
  String get settingsSectionRules => '콘텐츠 규칙';

  @override
  String get settingsSectionRulesSummary =>
      '클립에 자동으로 태그를 지정하거나 건너뛰거나 보호하거나 만료합니다.';

  @override
  String get settingsSectionQuickAccess => '빠른 액세스';

  @override
  String get settingsSectionQuickAccessSummary => '키보드, 타일, 홈 화면 위젯.';

  @override
  String get settingsSectionSnippets => '스니펫';

  @override
  String get settingsSectionSnippetsSummary => '재사용 가능한 텍스트와 템플릿.';

  @override
  String get settingsSectionTransforms => '텍스트 변환';

  @override
  String get settingsSectionTransformsSummary => '즐겨찾기, 파이프라인, 붙여넣기 기본 변환.';

  @override
  String get settingsSectionAiTools => 'AI 도구';

  @override
  String get settingsSectionAiToolsSummary =>
      'AI 어시스턴트를 연결하고 볼 수 있는 내용을 관리합니다.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary => '여러 복사 항목을 모아 순서대로 붙여넣습니다.';

  @override
  String get settingsSectionAbout => '정보';

  @override
  String get settingsSectionAboutSummary => '버전 및 앱 업데이트.';

  @override
  String get settingsGeneralDescription => '언어, 자동 시작 및 앱 표시 여부.';

  @override
  String get settingsModulesDescription => 'CopyPaste의 선택적 기능.';

  @override
  String get settingsClipboardDescription => '클립보드 변경 사항을 수집하고 기록 한도를 관리합니다.';

  @override
  String get settingsDataDescription => '클립보드 기록을 내보내고, 백업하고, 복원합니다.';

  @override
  String get settingsSyncDescription => '페어링된 기기와의 동기화를 제어합니다.';

  @override
  String get settingsPrivacyDescription => '수집할 앱을 제어하고 클립보드 내용을 보호합니다.';

  @override
  String get settingsNotificationsDescription => '수집된 클립에 대한 알림과 사운드를 선택합니다.';

  @override
  String get settingsAboutDescription => 'CopyPaste 버전 및 앱 업데이트.';

  @override
  String get settingsQuickPasteDescription => '창을 전환하지 않고 어디서든 클립보드 기록을 엽니다.';

  @override
  String get settingsLoadingMessage => '설정을 불러오는 중입니다.';

  @override
  String get settingsUnavailableTitle => '설정을 사용할 수 없음';

  @override
  String get settingsNoResultsTitle => '설정을 찾을 수 없음';

  @override
  String get settingsNoResultsMessage => '다른 검색어로 시도하십시오.';

  @override
  String get settingsAttentionTitle => '설정 확인 필요';

  @override
  String get settingsSearchPlaceholder => '설정 검색';

  @override
  String get settingsBackToSettings => '설정으로 돌아가기';

  @override
  String get settingsStartOnBootTitle => '기기 부팅 시 시작';

  @override
  String get settingsStartAtLoginTitle => '로그인 시 시작';

  @override
  String get settingsStartupSearchDescription => 'CopyPaste를 자동으로 시작합니다.';

  @override
  String get settingsStartupKeywords =>
      '시작 로그인 부팅 실행 자동 자동시작 startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      '작업 관리자에서 비활성화되어 있습니다. 설정 → 앱 → 시작 프로그램에서 CopyPaste를 켜십시오.';

  @override
  String get settingsStartupMacApproval =>
      '시스템 설정 → 일반 → 로그인 항목에서 CopyPaste를 허용하십시오.';

  @override
  String get settingsStartupUnavailable => '이 설치본에서는 자동 시작을 사용할 수 없습니다.';

  @override
  String get settingsStartupAndroidDescription =>
      '기기를 다시 시작한 후 켜져 있던 백그라운드 서비스를 재개합니다.';

  @override
  String get settingsStartupDesktopDescription => '로그인할 때 CopyPaste를 자동으로 엽니다.';

  @override
  String get settingsShowInDock => 'Dock에 표시';

  @override
  String get settingsShowInTaskbar => '작업 표시줄에 표시';

  @override
  String get settingsTaskbarVisibilityDescription => '창 전환을 위한 앱 아이콘을 표시합니다.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      '독 작업표시줄 앱 아이콘 표시 dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => '메뉴 막대에 표시';

  @override
  String get settingsShowInSystemTray => '시스템 트레이에 표시';

  @override
  String get settingsTrayVisibilityDescription =>
      'CopyPaste와 수집 제어에 빠르게 접근할 수 있도록 표시합니다.';

  @override
  String get settingsTrayVisibilityKeywords =>
      '트레이 메뉴 막대 메뉴막대 상태 아이콘 표시 tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle => '표시 여부를 변경할 수 없음';

  @override
  String get settingsModulesSearchDescription => '선택적 모듈을 설치, 업데이트 및 제거합니다.';

  @override
  String get settingsModulesKeywords => '기능 명령 확장 features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Android 백그라운드 수집';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Shizuku 또는 ADB로 설정하는 전체 또는 제한 모드.';

  @override
  String get settingsAndroidCaptureKeywords =>
      '백그라운드 권한 설정 background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      '전체 또는 제한 모드를 선택하고 일회성 Shizuku 또는 ADB 설정을 관리합니다.';

  @override
  String get settingsOpenSetup => '설정 열기';

  @override
  String get settingsSaveScreenshotsTitle => '스크린샷 저장';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      '새 Android 스크린샷을 기록에 자동으로 저장합니다.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      '이미지 사진 수집 권한 images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription => '새 스크린샷을 기록에 자동으로 추가합니다.';

  @override
  String get settingsScreenshotAccessTitle => '스크린샷 접근 권한';

  @override
  String get settingsScreenshotAccessDescription =>
      '백그라운드에서 새 스크린샷을 저장하려면 모든 사진 및 알림에 대한 접근을 허용하십시오.';

  @override
  String get settingsAllowAccess => '접근 허용';

  @override
  String get settingsScreenshotCaptureStoppedTitle => '스크린샷 수집이 실행 중이 아님';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      '새 스크린샷이 저장되지 않고 있습니다. 서비스를 다시 시작하려면 수집을 재개하십시오.';

  @override
  String get settingsResumeCapture => '수집 재개';

  @override
  String get settingsClipboardCaptureTitle => '클립보드 수집';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      '클립보드 수집을 일시 정지하거나 재개합니다.';

  @override
  String get settingsCaptureStateUnavailable => '상태를 알 수 없음';

  @override
  String get settingsCaptureStoppedTitle => '수집이 실행 중이 아님';

  @override
  String get settingsCaptureStoppedMessage =>
      '현재 CopyPaste가 이 기기에서 클립보드 변경 사항을 받고 있지 않습니다.';

  @override
  String get settingsRetentionTitle => '보관 기간';

  @override
  String get settingsRetentionDescription => '고정되지 않은 오래된 클립보드 항목을 자동으로 제거합니다.';

  @override
  String get settingsRetentionForever => '영구 보관';

  @override
  String get settingsStorageQuotaTitle => '저장 공간 할당량';

  @override
  String get settingsStorageQuotaDescription => '고정되지 않은 기록이 사용하는 최대 로컬 저장 공간.';

  @override
  String get settingsStorageQuotaKeywords => '디스크 공간 한도 disk space limit';

  @override
  String get settingsMaxFileSizeTitle => '최대 파일 크기';

  @override
  String get settingsMaxFileSizeSearchDescription => '파일 추가 및 다운로드 한도.';

  @override
  String get settingsMaxFileSizeKeywords =>
      '가져오기 다운로드 전송 크기 한도 import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      '파일 추가 및 다운로드 한도입니다. 파일 내용은 ‘다운로드’를 선택할 때만 전송됩니다.';

  @override
  String get settingsHistoryFilesTitle => '기록 파일';

  @override
  String get settingsHistoryFilesSearchDescription =>
      '기록을 내보내거나, 백업을 만들거나, 백업을 복원합니다.';

  @override
  String get settingsHistoryFilesKeywords =>
      '텍스트 암호화 백업 데이터 text encrypted backup data';

  @override
  String get settingsExportTextTitle => '텍스트 기록 내보내기';

  @override
  String get settingsExportTextDescription => '텍스트 클립을 이동 가능한 파일로 저장합니다.';

  @override
  String get settingsExportAction => '내보내기';

  @override
  String get settingsBackupTitle => '암호화된 백업';

  @override
  String get settingsBackupDescription => '이 기기의 전체 로컬 기록을 저장합니다.';

  @override
  String get settingsCreateBackupAction => '백업 만들기';

  @override
  String get settingsRestoreTitle => '백업 복원';

  @override
  String get settingsRestoreDescription => '로컬 기록을 암호화된 백업으로 대체합니다.';

  @override
  String get settingsRestoreAction => '복원';

  @override
  String get settingsRestoreDialogTitle => '로컬 기록을 대체하시겠습니까?';

  @override
  String get settingsRestoreDialogMessage =>
      '선택한 암호화된 백업이 이 기기의 클립보드 기록을 대체합니다. 현재 설정과 기기 ID는 유지됩니다.';

  @override
  String get settingsRestoreFailed => '백업을 복원할 수 없습니다.';

  @override
  String get settingsSyncSwitchTitle => '동기화';

  @override
  String get settingsSyncSwitchDescription => '페어링된 기기 동기화를 한 번에 켜거나 끕니다.';

  @override
  String get settingsSyncSearchDescription => '페어링된 기기 동기화.';

  @override
  String get settingsInstantClipboardTitle => '즉시 클립보드';

  @override
  String get settingsInstantClipboardSearchDescription =>
      '다른 기기의 새 클립을 자동으로 복사합니다.';

  @override
  String get settingsInstantClipboardKeywords =>
      '자동 복사 붙여넣기 동기화 수신 automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      '다른 기기의 최신 클립을 이 기기의 클립보드에 자동으로 복사합니다.';

  @override
  String get settingsLanVisibilityTitle => 'LAN 표시';

  @override
  String get settingsLanVisibilityDescription =>
      '근처 기기가 이 기기를 검색할 수 있도록 허용합니다.';

  @override
  String get settingsLanVisibilityKeywords =>
      '로컬 네트워크 검색 local network discovery';

  @override
  String get settingsExcludedAppsTitle => '제외된 앱';

  @override
  String get settingsExcludedAppsSearchDescription => '제외된 앱에서는 자동 수집을 건너뜁니다.';

  @override
  String get settingsExcludedAppsKeywords =>
      '개인정보 앱 식별자 privacy app identifiers';

  @override
  String get settingsExcludedAppsMacDescription =>
      '이 앱이 활성 상태인 동안 자동 수집을 건너뜁니다. 백그라운드 복사는 제외 설정을 우회할 수 있습니다.';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      '식별된 클립보드 소유자로부터의 자동 수집을 건너뜁니다. 제외 항목이 설정되어 있고 출처를 식별할 수 없으면 수집이 일시 정지됩니다.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Android는 출처 앱을 식별할 수 없으므로 제외 항목이 설정되어 있는 동안 자동 수집을 건너뜁니다.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      '앱 제외는 macOS, Windows, Android 및 Linux에서 지원됩니다.';

  @override
  String get settingsExcludedAppsAdd => '추가';

  @override
  String get settingsExcludedAppsEmpty => '제외된 앱이 없습니다.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return '$app 제거';
  }

  @override
  String get settingsExcludeDialogTitle => '앱 제외';

  @override
  String get settingsExcludeDialogAction => '제외';

  @override
  String get settingsExcludeDialogPlaceholder => '앱 식별자';

  @override
  String get settingsExcludeFailed => '앱을 제외할 수 없습니다.';

  @override
  String get settingsSkipSecretTitle => '기밀 클립보드 건너뛰기';

  @override
  String get settingsSkipSecretDescription => '출처 앱이 기밀로 표시한 내용을 기록에서 제외합니다.';

  @override
  String get settingsSkipSecretKeywords =>
      '기밀 비밀번호 민감 숨김 스포일러 secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => '임시 클립보드 건너뛰기';

  @override
  String get settingsSkipTransientDescription =>
      '출처 앱이 임시로 표시한 복사 항목을 기록에서 제외합니다.';

  @override
  String get settingsSkipTransientKeywords =>
      '일시적 임시 개인정보 transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => '스크린샷 차단';

  @override
  String get settingsBlockScreenshotsDescription =>
      'CopyPaste의 스크린샷 및 화면 녹화를 차단합니다.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      '이 플랫폼에서는 스크린샷 차단을 사용할 수 없습니다.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      '개인정보 화면 캡처 보호 페어링 QR 보안 코드 privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => '복사 시 알림';

  @override
  String get settingsNotifyOnCopyDescription => '백그라운드에서 수집한 후 알림을 표시합니다.';

  @override
  String get settingsNotificationPreviewTitle => '클립보드 내용 표시';

  @override
  String get settingsNotificationPreviewDescription => '복사 알림에 클립 미리보기를 포함합니다.';

  @override
  String get settingsNotificationPreviewKeywords =>
      '알림 미리보기 텍스트 이미지 개인정보 notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => '복사 시 사운드';

  @override
  String get settingsSoundOnCopyDescription => '수집에 성공하면 플랫폼 피드백을 재생합니다.';

  @override
  String get settingsVersionTitle => '버전';

  @override
  String get settingsVersionDescription => '설치된 CopyPaste 버전.';

  @override
  String get settingsVersionUnavailable => '알 수 없음';

  @override
  String get settingsUpdatesTitle => '앱 업데이트';

  @override
  String get settingsUpdatesSearchDescription => 'CopyPaste 업데이트를 확인하고 설치합니다.';

  @override
  String get settingsUpdatesKeywords => '버전 깃허브 릴리스 version github release';

  @override
  String get settingsUpdateIdle => '최신 CopyPaste 버전이 있는지 확인합니다.';

  @override
  String get settingsUpdateChecking => '업데이트를 확인하는 중입니다.';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste가 최신 상태입니다.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return '$version 버전이 최신 상태입니다.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return '$available 버전을 사용할 수 있습니다. 이 기기의 버전은 $current입니다.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return '$version 버전 다운로드 중 · $percent%.';
  }

  @override
  String get settingsUpdatePreparingModules => '설치된 모듈 업데이트를 준비하는 중입니다.';

  @override
  String settingsUpdateInstalling(String version) {
    return '$version 버전을 설치하는 중입니다.';
  }

  @override
  String get settingsUpdateNeedsAttention => '앱 업데이트에 확인이 필요합니다.';

  @override
  String get settingsUpdateNowAction => '지금 업데이트';

  @override
  String get settingsUpdateOpenReleaseAction => '릴리스 열기';

  @override
  String get settingsUpdateCheckingAction => '확인 중';

  @override
  String get settingsUpdateDownloadingAction => '다운로드 중';

  @override
  String get settingsUpdateInstallingAction => '설치 중';

  @override
  String get settingsUpdateRestartAction => 'CopyPaste 다시 시작';

  @override
  String get settingsUpdateCheckAgainAction => '다시 확인';

  @override
  String get settingsUpdateNoneTitle => '사용 가능한 업데이트 없음';

  @override
  String get settingsUpdateLatest => 'CopyPaste가 최신 버전입니다.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return '최신 버전인 CopyPaste $version 버전을 사용 중입니다.';
  }

  @override
  String get settingsUpdateAvailableTitle => '업데이트 사용 가능';

  @override
  String get settingsUpdateReady => '최신 CopyPaste 버전을 설치할 준비가 되었습니다.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version 버전을 설치할 준비가 되었습니다.';
  }

  @override
  String get settingsUpdateUnavailableTitle => '업데이트를 사용할 수 없음';

  @override
  String get settingsUpdateUnavailableMessage => '이 설치본은 자동으로 업데이트할 수 없습니다.';

  @override
  String get settingsUpdateCheckFailedTitle => '업데이트 확인 실패';

  @override
  String get settingsUpdateCheckFailedMessage => 'CopyPaste가 업데이트를 확인할 수 없습니다.';

  @override
  String get settingsQuickPasteSetupTitle => 'Quick Paste 설정';

  @override
  String get settingsQuickPasteSetupMessage =>
      '서명된 GNOME 또는 KDE 컴패니언을 켜고, 이전 앱으로 돌아갈 수 있도록 키보드 제어를 허용하십시오.';

  @override
  String get settingsQuickPasteDesktopIntegration => '데스크톱 통합';

  @override
  String get settingsQuickPasteAllowKeyboard => '키보드 제어 허용';

  @override
  String get settingsQuickPasteCheckAgain => '다시 확인';

  @override
  String get settingsQuickPasteLoading => 'Quick Paste 설정을 불러오는 중입니다.';

  @override
  String get settingsQuickPasteUnavailableTitle => 'Quick Paste를 사용할 수 없음';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      '이 기기는 Quick Paste 창을 제공하지 않습니다.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Quick Paste 열기';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      '전역 Quick Paste 단축키를 구성합니다.';

  @override
  String get settingsQuickPasteShortcutKeywords => '키보드 단축키 핫키 keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      '단축키는 CopyPaste가 백그라운드에서 실행 중일 때 작동합니다.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return '등록된 단축키: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => '자동으로 붙여넣기';

  @override
  String get settingsAutoPasteSearchDescription => '선택한 클립을 이전 앱에 붙여넣습니다.';

  @override
  String get settingsAutoPasteKeywords => '손쉬운 사용 자동 accessibility automatic';

  @override
  String get settingsAutoPasteDescription => '클립을 선택하면 이전 앱으로 돌아가서 붙여넣습니다.';

  @override
  String get settingsAccessibilityRequiredTitle => '손쉬운 사용 권한 필요';

  @override
  String get settingsAccessibilityRequiredMessage =>
      '권한이 허용될 때까지는 클립을 선택하면 붙여넣지 않고 복사만 합니다.';

  @override
  String get settingsQuickPasteAttentionTitle => 'Quick Paste 확인 필요';

  @override
  String get settingsShortcutRecording => '단축키 입력…';

  @override
  String get settingsShortcutNotSet => '설정 안 됨';

  @override
  String get settingsShortcutReset => '단축키 재설정';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count일',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => '기기를 불러오는 중…';

  @override
  String get devicesUnavailableTitle => '기기 목록을 사용할 수 없음';

  @override
  String get devicesNearbyTitle => '근처 기기';

  @override
  String get devicesScan => '검색';

  @override
  String get devicesScanning => '검색 중…';

  @override
  String get devicesNoneNearby => '근처에 기기 없음';

  @override
  String get devicesThisDevice => '이 기기';

  @override
  String get devicesTrusted => '신뢰함';

  @override
  String get devicesConnect => '연결';

  @override
  String devicesCardPing(String ping) {
    return '핑: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return '핑:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => '이 기기 이름 변경';

  @override
  String get devicesNamePlaceholder => '기기 이름';

  @override
  String get devicesRenameFailed => '기기 이름을 저장할 수 없습니다.';

  @override
  String devicesRevokeDialogTitle(String device) {
    return '$device의 페어링을 철회하시겠습니까?';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return '$device의 페어링을 해제하시겠습니까?';
  }

  @override
  String get devicesRevokeDialogMessage =>
      '이 기기를 그룹에서 제거하고 이 페어링을 영구적으로 차단합니다. 다른 기기는 다시 연결될 때 제거 사항을 적용합니다.';

  @override
  String get devicesUnpairDialogMessage =>
      '이 기기를 그룹에서 제거합니다. 다른 기기는 다시 연결될 때 제거 사항을 적용합니다. 다시 추가하려면 새로 확인해야 합니다.';

  @override
  String get devicesRevoke => '철회';

  @override
  String get devicesUnpair => '페어링 해제';

  @override
  String get devicesActionFailedMessage => '기기 작업을 완료할 수 없습니다.';

  @override
  String get devicesCloseDetails => '기기 세부 정보 닫기';

  @override
  String get devicesDetailType => '기기 유형';

  @override
  String get devicesDetailOperatingSystem => '운영 체제';

  @override
  String get devicesDetailModel => '모델';

  @override
  String get devicesDetailAppVersion => 'CopyPaste 버전';

  @override
  String get devicesDetailProtocolVersion => '프로토콜 버전';

  @override
  String get devicesDetailStatus => '상태';

  @override
  String get devicesDetailPing => '핑';

  @override
  String get devicesDetailMeasured => '측정 시각';

  @override
  String get devicesDetailLastSeen => '마지막 확인';

  @override
  String get devicesDetailLanEndpoint => 'LAN 엔드포인트';

  @override
  String get devicesDetailDeviceId => '기기 ID';

  @override
  String get devicesDetailPairingId => '페어링 ID';

  @override
  String get devicesDetailProfileProvenance => '프로필 출처';

  @override
  String get devicesDetailProfileTrust => '프로필 신뢰도';

  @override
  String get devicesDetailProfileUpdated => '프로필 업데이트 시각';

  @override
  String get devicesValueUnavailable => '사용할 수 없음';

  @override
  String get devicesRenameDevice => '기기 이름 변경';

  @override
  String get devicesSyncNow => '지금 동기화';

  @override
  String get devicesRevokePairing => '페어링 철회';

  @override
  String get devicesLocal => '로컬';

  @override
  String get devicesTrustAuthenticated => '인증됨';

  @override
  String get devicesTrustUnverified => '확인되지 않음';

  @override
  String get devicesProvenanceSelfReported => '자체 보고';

  @override
  String get devicesProvenanceObserved => '관찰됨';

  @override
  String get devicesProvenanceMeasured => '측정됨';

  @override
  String get devicesConnectionOnline => '온라인';

  @override
  String get devicesConnectionOffline => '오프라인';

  @override
  String get devicesConnectionVisible => '감지됨';

  @override
  String get devicesConnectionNotSeen => '감지되지 않음';

  @override
  String get devicesConnectionUnknown => '알 수 없음';

  @override
  String get devicesPeerAvailable => '사용 가능';

  @override
  String get devicesPeerVisibleNow => '지금 감지됨';

  @override
  String get devicesPeerStatusUnknown => '상태 알 수 없음';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => '데스크톱';

  @override
  String get devicesClassLaptop => '노트북';

  @override
  String get devicesClassPhone => '휴대폰';

  @override
  String get devicesClassTablet => '태블릿';

  @override
  String get devicesClassUnknown => '기기';

  @override
  String get devicesUnknownOs => '알 수 없는 OS';

  @override
  String get devicesAddDevice => '기기 추가';

  @override
  String get devicesScanQr => 'QR 스캔';

  @override
  String get devicesEnterCode => '코드 입력';

  @override
  String get devicesScannerUnavailableTitle => '스캐너를 사용할 수 없음';

  @override
  String get devicesActionFailedTitle => '작업 실패';

  @override
  String get devicesRefreshFailedTitle => '기기 새로 고침 실패';

  @override
  String get devicesNearbyPairingUnavailable => '근처 기기 페어링을 사용할 수 없습니다.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      '근처 기기 페어링을 사용할 수 없습니다. ‘기기 추가’를 사용하여 페어링 코드를 표시하십시오.';

  @override
  String get pairingTitle => '페어링';

  @override
  String get pairingPairDevice => '기기 페어링';

  @override
  String get pairingScanQrCode => 'QR 코드 스캔';

  @override
  String get pairingEnterPairingCode => '페어링 코드 입력';

  @override
  String get pairingConnectNearbyDevice => '근처 기기 연결';

  @override
  String get pairingSendingRequest => '연결 요청을 보내는 중…';

  @override
  String get pairingEmptyTitle => '페어링 방법 선택';

  @override
  String get pairingEmptyMessage => '페어링 제어 항목이 여기에 표시됩니다.';

  @override
  String get pairingCreatingInvitation => '페어링 초대를 만드는 중입니다.';

  @override
  String get pairingCode => '페어링 코드';

  @override
  String get pairingCodeHint => '다른 기기에 표시된 8자리 코드를 입력하십시오.';

  @override
  String get pairingDeviceAddress => '기기 주소';

  @override
  String get pairingDeviceAddressHint => '다른 기기에 표시된 host:port 주소를 사용하십시오.';

  @override
  String get pairingCheckDetailsTitle => '페어링 정보 확인';

  @override
  String get pairingActionFailed => '페어링 작업 실패';

  @override
  String get pairingConnectingDevice => '기기 연결 중…';

  @override
  String get pairingQrScannerTitle => 'QR 스캐너';

  @override
  String get pairingCameraPreviewPlaceholder => '카메라 미리보기가 여기에 표시됩니다.';

  @override
  String get pairingScannerNeedsAttention => '스캐너 확인 필요';

  @override
  String get pairingCodeIncomplete => '8자리 페어링 코드를 모두 입력하십시오.';

  @override
  String get pairingAddressMissing => '기기의 host:port 주소를 입력하십시오.';

  @override
  String get pairingWaitingForOtherDevice => '다른 기기를 기다리는 중';

  @override
  String get pairingInviteInstructions =>
      '다른 기기에서 ‘기기 추가’ → ‘코드 입력’을 선택하거나 QR 코드를 스캔하십시오.';

  @override
  String get pairingDetailsTitle => '페어링 정보';

  @override
  String get pairingAddress => '주소';

  @override
  String get pairingVerificationCode => '확인 코드';

  @override
  String get pairingVerificationCodeHint => '두 기기의 코드가 일치하는 경우에만 확인하십시오.';

  @override
  String get pairingReject => '거부';

  @override
  String get pairingConfirm => '확인';

  @override
  String get pairingAccept => '수락';

  @override
  String get pairingJoinDevice => '기기 연결';

  @override
  String get pairingStartScanner => '스캐너 시작';

  @override
  String get pairingActionNotStarted => '페어링 작업을 시작할 수 없습니다.';

  @override
  String get pairingStateHandshaking => '기기 확인 중';

  @override
  String get pairingStateAwaitingConfirmation => '두 기기에서 확인';

  @override
  String get pairingStateConfirmed => '기기 페어링됨';

  @override
  String get pairingStateRejected => '페어링 거부됨';

  @override
  String get pairingStateCancelled => '페어링 취소됨';

  @override
  String get pairingStateTimedOut => '페어링 시간 초과';

  @override
  String get pairingStateFailed => '페어링 실패';

  @override
  String get pairingHandshakingDescription => '기기 간에 보안 채널을 설정하는 중입니다.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      '보호된 확인 코드를 비교한 다음 두 기기에서 확인하십시오.';

  @override
  String get pairingConfirmedDescription =>
      '기기가 연결되었습니다. 그룹 구성원 정보가 자동으로 동기화됩니다.';

  @override
  String get pairingNoDeviceAdded => '추가된 기기가 없습니다.';

  @override
  String get pairingTimedOutDescription => '계속하려면 새 페어링을 시작하십시오.';

  @override
  String get pairingIdleDescription => '기기를 연결하려면 페어링을 시작하십시오.';

  @override
  String get pairingProtectedUnavailableTitle => '보호된 페어링을 사용할 수 없음';

  @override
  String get pairingProtectedUnavailableMessage =>
      '이 페어링 세션은 전용 보호 창에서 열어야 합니다.';

  @override
  String get pairingCancel => '페어링 취소';

  @override
  String get pairingOrEnterCode => '또는 페어링 코드 입력';

  @override
  String get pairingJoinSecurely => '안전하게 연결';

  @override
  String get pairingProtectedWaitingDescription =>
      '이 보호된 창에서 QR 코드를 표시하거나, 스캔하거나, 페어링 코드를 입력하십시오.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return '$seconds초 후 만료';
  }

  @override
  String get pairingInvalidQr => 'CopyPaste 페어링 QR 코드를 스캔하십시오.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Google 스캐너를 사용할 수 없습니다. 대신 페어링 코드를 입력하십시오.';

  @override
  String get pairingScannerCouldNotOpen => '스캐너를 열 수 없습니다. 대신 페어링 코드를 입력하십시오.';

  @override
  String get pairingSecurePresentationUnavailable => '보안 페어링 화면을 사용할 수 없습니다.';

  @override
  String get pairingVerificationCodeRequired => '먼저 확인 코드를 표시하고 비교하십시오.';

  @override
  String get syncStatusUnavailable => '동기화 상태를 알 수 없음';

  @override
  String get syncStatusOff => '동기화 꺼짐';

  @override
  String get syncStatusWaitingForSync => '다음 동기화 대기 중';

  @override
  String get syncStatusSyncing => '동기화 중';

  @override
  String get syncStatusUpToDate => '최신 상태';

  @override
  String get syncStatusInterrupted => '동기화 중단됨';

  @override
  String get syncStatusPartiallySynchronized => '일부 동기화됨';

  @override
  String get syncStatusRetrying => '다시 시도 중';

  @override
  String get syncStatusWaitingToSync => '동기화 대기 중';

  @override
  String get syncStatusWaitingForDevice => '기기 대기 중';

  @override
  String get syncStatusNoPairedDevices => '페어링된 기기 없음';

  @override
  String get syncStatusDevicesOffline => '기기 오프라인';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · 활동 로그를 사용할 수 없음';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '동기화 꺼짐 · 문제 $count개',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count대 오프라인',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '기기 $count대 확인 필요',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '기기 오프라인 · 문제 $count개',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => '연결 확인 중';

  @override
  String get syncConnectionUnknown => '연결 상태 알 수 없음';

  @override
  String get syncLogDeviceOffline => '기기가 오프라인 상태가 되었습니다.';

  @override
  String get syncLogConnectionRestored => '연결이 복원되었습니다.';

  @override
  String get syncLogProblemDetected => '동기화 문제가 감지되었습니다.';

  @override
  String get syncLogProblemResolved => '동기화 문제가 해결되었습니다.';

  @override
  String get syncLogFailed => '동기화에 실패했습니다.';

  @override
  String get syncLogRecovered => '동기화가 복원되었습니다.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent개',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received개',
    );
    return '클립 $_temp0 보냄 · 클립 $_temp1 받음';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return '동기화: $status';
  }

  @override
  String get syncDetailsTitle => '동기화';

  @override
  String get syncDetailsClose => '동기화 세부 정보 닫기';

  @override
  String get syncNoPairedDevicesMessage => '클립보드 기록을 동기화하려면 기기를 페어링하십시오.';

  @override
  String get syncActivityLogUnavailableTitle => '활동 로그를 사용할 수 없음';

  @override
  String get syncActivityLogUnavailableMessage => '동기화 활동을 읽거나 저장할 수 없습니다.';

  @override
  String get syncDetailsDevicesHeading => '기기';

  @override
  String get syncDetailsActivityHeading => '활동';

  @override
  String get syncDetailsNoRecentProblems => '최근 동기화 문제 없음';

  @override
  String get syncDetailsUnknownPeer => '페어링된 기기';

  @override
  String get syncPeerConnection => '연결';

  @override
  String get syncPeerLastCheckStarted => '마지막 확인 시작';

  @override
  String get syncPeerItemsOverSizeLimit => '크기 한도 초과 항목';

  @override
  String syncPeerLastSeen(String time) {
    return '마지막 확인 $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return '마지막 동기화 $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => '마지막 동기화 성공';

  @override
  String get syncPeerConnectionDetails => '연결 세부 정보';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return '$device 연결 세부 정보';
  }

  @override
  String get historyKindText => '텍스트';

  @override
  String get historyKindLink => '링크';

  @override
  String get historyKindEmail => '이메일';

  @override
  String get historyKindColor => '색상';

  @override
  String get historyKindPhone => '전화번호';

  @override
  String get historyKindCode => '코드';

  @override
  String get historyKindPath => '경로';

  @override
  String get historyKindCredentials => '자격 증명';

  @override
  String get historyKindImage => '이미지';

  @override
  String get historyKindFile => '파일';

  @override
  String get historyKindOther => '기타';

  @override
  String get historySortNewest => '최신순';

  @override
  String get historySortOldest => '오래된순';

  @override
  String get historySortRelevance => '관련도순';

  @override
  String get historyErrorUpdatesUnavailable => '기록 업데이트를 일시적으로 사용할 수 없습니다.';

  @override
  String get historyErrorBulkDelete => '선택한 클립 중 일부를 삭제할 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyErrorBulkPin =>
      '선택한 클립 중 일부의 고정 상태를 업데이트할 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyErrorRefresh => '기록을 새로 고칠 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyErrorLoad => '기록을 불러올 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyErrorLoadMore => '추가 기록을 불러올 수 없습니다.';

  @override
  String get historyErrorLoadEarlier => '이전 기록을 불러올 수 없습니다.';

  @override
  String get historyErrorOpenClip => '이 클립을 열 수 없습니다.';

  @override
  String get historyErrorCopyPlainText => '일반 텍스트를 복사할 수 없습니다.';

  @override
  String get historyErrorCopyClip => '이 클립을 복사할 수 없습니다.';

  @override
  String get historyErrorCopyText => '이 텍스트를 복사할 수 없습니다.';

  @override
  String get historyErrorPrepareFile =>
      '이 파일을 다운로드할 수 없습니다. 원본 기기를 확인하고 다시 시도하십시오.';

  @override
  String get historyErrorDownloadFile => '이 파일을 다운로드할 수 없습니다.';

  @override
  String get historyErrorSelectFiles => '파일을 선택할 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyErrorReadSelectedFile =>
      '선택한 파일을 읽을 수 없습니다. 접근 권한과 사용 가능한 저장 공간을 확인하십시오.';

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
      other: '파일 $total개 중 $failed개를 가져올 수 없습니다. 첫 번째 파일: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate => '클립의 기밀 상태를 업데이트할 수 없습니다.';

  @override
  String get historyErrorPinUpdate => '고정 상태를 업데이트할 수 없습니다.';

  @override
  String get historyErrorPinnedOrderSave => '고정된 클립 순서를 저장할 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyErrorPinnedOrderRefresh => '순서는 저장되었지만 기록을 새로 고칠 수 없습니다.';

  @override
  String get historyErrorDeleteClip => '이 클립을 삭제할 수 없습니다.';

  @override
  String get historyErrorDeleteAll => '기록을 삭제할 수 없습니다.';

  @override
  String get historyErrorBulkDeleteFallback => '선택한 클립을 삭제할 수 없습니다.';

  @override
  String get historyOcrErrorUnavailable =>
      'OCR을 사용할 수 없습니다. 설정에서 모듈을 확인하고 다시 시도하십시오.';

  @override
  String get historyOcrErrorStart => 'OCR을 시작할 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyOcrErrorRecognize => '이미지 텍스트를 인식할 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyOcrRecognizing => '이미지 텍스트를 인식하는 중입니다.';

  @override
  String get historyOcrFailedTitle => 'OCR 실패';

  @override
  String get historyOcrNoText => '텍스트를 찾을 수 없음';

  @override
  String get historyOcrTitle => '이미지 텍스트';

  @override
  String get historyOcrTextCopied => '인식된 텍스트를 복사했습니다.';

  @override
  String get historyOcrCopyError => '텍스트를 복사할 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyTagsErrorLoad => '태그를 불러올 수 없습니다. 다시 시도하십시오.';

  @override
  String get historyTagsErrorSave =>
      '태그 변경 사항을 저장할 수 없습니다. 64자 이하의 고유한 이름을 사용하고 고정된 클립을 선택하십시오.';

  @override
  String get historyTags => '태그';

  @override
  String get historyTagsUpdateErrorTitle => '태그를 업데이트할 수 없음';

  @override
  String get historyTagsEmpty => '아직 태그 없음';

  @override
  String get historyTagRename => '이름 변경';

  @override
  String get historyTagActions => '태그 작업';

  @override
  String get historyTagNamePlaceholder => '태그 이름';

  @override
  String get historyCreateTag => '태그 만들기';

  @override
  String get historySaveTag => '태그 저장';

  @override
  String get historyTagsFilterAll => '모든 태그';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '태그 $count개',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return '태그로 필터링: $selection';
  }

  @override
  String get historyManageTags => '태그 관리';

  @override
  String get historyTagsFilterSemantics => '태그로 고정된 클립 필터링';

  @override
  String get historyDownloadErrorCancelled => '파일 다운로드가 취소되었습니다.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      '다운로드 서비스를 사용할 수 없습니다. CopyPaste를 다시 열고 다시 시도하십시오.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      '다운로드 서비스가 연결되지 않았습니다. 다시 시도하십시오.';

  @override
  String get historyFileErrorAccess => '선택한 파일에 접근할 수 없습니다.';

  @override
  String get historySpoilerRevealSemantics => '기밀 내용. 스포일러 표시';

  @override
  String get historyActionFailedTitle => '작업 실패';

  @override
  String get historyLoadingMessage => '클립보드 기록을 불러오는 중입니다.';

  @override
  String get historyUnavailableTitle => '기록을 사용할 수 없음';

  @override
  String get historyEmptyTitle => '클립을 찾을 수 없음';

  @override
  String get historyEmptyMessage => '수집된 클립과 일치하는 기록이 여기에 표시됩니다.';

  @override
  String get historyAttentionTitle => '기록 확인 필요';

  @override
  String get historyLoadMore => '더 불러오기';

  @override
  String get historySectionPinned => '고정됨';

  @override
  String get historySectionToday => '오늘';

  @override
  String get historySectionYesterday => '어제';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '클립보드 항목 $count개를 건너뛰었습니다',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage => '보관된 일부 클립을 읽을 수 없어 이 목록에 표시되지 않습니다.';

  @override
  String get historySearch => '기록 검색';

  @override
  String get historyResetFilters => '필터 재설정';

  @override
  String get historyImportFiles => '파일 가져오기';

  @override
  String get historySelectClips => '클립 선택';

  @override
  String get historyCloseSearch => '검색 닫기';

  @override
  String get historyFilterAllClips => '모든 클립';

  @override
  String get historyFilterAllDevices => '모든 기기';

  @override
  String get historyFilterAllApps => '모든 앱';

  @override
  String get historyPinClip => '클립 고정';

  @override
  String get historyUnpinClip => '클립 고정 해제';

  @override
  String get historyReorderPinnedClip => '고정된 클립 순서 변경';

  @override
  String get historyReorderPinnedHint =>
      '드래그하여 이동합니다. Alt + 위쪽 또는 아래쪽 화살표를 누르면 한 칸씩 이동합니다.';

  @override
  String get historyClipUnavailable => '클립을 사용할 수 없음';

  @override
  String historyClipKindSemantics(String kind) {
    return '$kind 클립 유형';
  }

  @override
  String get historyInspectorEmptyTitle => '클립 선택';

  @override
  String get historyInspectorEmptyMessage => '클립의 전체 내용과 작업이 여기에 표시됩니다.';

  @override
  String get historyNoPreview => '미리보기를 사용할 수 없습니다.';

  @override
  String get historyCloseClipDetails => '클립 세부 정보 닫기';

  @override
  String get historyCopyOptions => '복사 옵션';

  @override
  String get historyCopyPlainText => '일반 텍스트로 복사';

  @override
  String get historyManageClipTags => '클립 태그 관리';

  @override
  String get historyPinnedButton => '고정됨';

  @override
  String get historyAddSpoiler => '스포일러 추가';

  @override
  String get historyRemoveSpoiler => '스포일러 제거';

  @override
  String get historyUpdateFailedTitle => '업데이트 실패';

  @override
  String get historyRecognizeImageText => '이미지 텍스트 인식';

  @override
  String get historyCopiedTitle => '복사됨';

  @override
  String get historyCopyFailedTitle => '복사 실패';

  @override
  String get historyPlainTextCopied => '일반 텍스트를 복사했습니다.';

  @override
  String get historyClipCopied => '클립을 복사했습니다.';

  @override
  String get historyDownloadedTitle => '다운로드됨';

  @override
  String get historyFileSaved => '파일을 저장했습니다.';

  @override
  String get historyDownload => '다운로드';

  @override
  String get historyDownloading => '다운로드 중…';

  @override
  String historyDownloadingPercent(int percent) {
    return '다운로드 중 $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => '취소 중…';

  @override
  String get historyDownloadCancellingTooltip => '다운로드 취소 중';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return '다운로드 중 $percent% · 클릭하여 취소';
  }

  @override
  String get historyDownloadFailedRetryTooltip => '다운로드 실패 · 클릭하여 다시 시도';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return '다운로드 취소, $percent% 완료';
  }

  @override
  String get historyRetryDownload => '다운로드 다시 시도';

  @override
  String get historyDownloadFailedTitle => '다운로드 실패';

  @override
  String get historyShowFile => '파일 보기';

  @override
  String get historyMetadataPrivacy => '개인정보 보호';

  @override
  String get historyMetadataConfidential => '기밀';

  @override
  String get historyMetadataCaptured => '수집 시각';

  @override
  String get historyMetadataObservedApp => '감지된 앱';

  @override
  String get historyMetadataDevice => '기기';

  @override
  String get historyMetadataClipType => '클립 유형';

  @override
  String get historyMetadataType => '유형';

  @override
  String get historyMetadataPath => '경로';

  @override
  String get historyMetadataFile => '파일';

  @override
  String get historyMetadataResolution => '해상도';

  @override
  String get historyMetadataSize => '크기';

  @override
  String get historyMetadataFiles => '파일 수';

  @override
  String get historyMetadataSync => '동기화';

  @override
  String get historyMetadataTooLargeToSync => '너무 커서 동기화할 수 없음';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '선택한 클립 $count개를 삭제하시겠습니까?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => '이 클립을 삭제하시겠습니까?';

  @override
  String get historyBulkTagTooltip => '선택한 고정 클립에 태그 지정';

  @override
  String get historyBulkExit => '선택 종료';

  @override
  String get historyBulkPin => '선택 항목 고정';

  @override
  String get historyBulkUnpin => '선택 항목 고정 해제';

  @override
  String get historyBulkDelete => '선택 항목 삭제';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count개 선택됨',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => '날짜로 필터링';

  @override
  String get historyDatePresetToday => '오늘';

  @override
  String get historyDatePresetYesterday => '어제';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: '최근 $days일',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => '전체 기간';

  @override
  String get appRuntimeStartFailed => 'CopyPaste가 런타임을 시작할 수 없습니다.';

  @override
  String get appPreparing => 'CopyPaste를 준비하는 중입니다.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste가 보호된 기록에 대한 접근을 기다리는 중입니다. 앱에서 사용 가능한 부분은 계속 사용할 수 있습니다.';

  @override
  String get appHistoryUnavailableTitle => '기록 런타임을 사용할 수 없음';

  @override
  String get appHistoryUnavailableMessage => '기록을 열기 전에 앱 런타임을 시작하십시오.';

  @override
  String get appDevicesUnavailableTitle => '기기 런타임을 사용할 수 없음';

  @override
  String get appDevicesUnavailableMessage => '기기를 열기 전에 앱 런타임을 시작하십시오.';

  @override
  String get appSettingsUnavailableTitle => '설정 런타임을 사용할 수 없음';

  @override
  String get appSettingsUnavailableMessage => '설정을 열기 전에 앱 런타임을 시작하십시오.';

  @override
  String get appQuit => 'CopyPaste 종료';

  @override
  String get appWindowSetupTitle => '창 설정 확인 필요';

  @override
  String get appOpenHistory => '기록 열기';

  @override
  String get appDestinationHistory => '기록';

  @override
  String get appDestinationDevices => '기기';

  @override
  String get appDestinationSettings => '설정';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, 선택됨';
  }

  @override
  String get appActiveFilter => '활성 필터';

  @override
  String get appActionFailed => '작업 실패';

  @override
  String get sharedAndroidVersionRequired => 'Android 11 이상 필요';

  @override
  String get sharedUseAdbTab => '이 기기에서는 ADB 탭을 사용하십시오.';

  @override
  String get sharedOneTimeAccessApplied => '일회성 접근 권한 적용됨';

  @override
  String get sharedInstallShizuku => 'Shizuku 설치';

  @override
  String get sharedPairAndStartShizuku => 'Shizuku 페어링 및 시작';

  @override
  String get sharedAllowCopyPaste => 'CopyPaste 허용';

  @override
  String get sharedShizukuNoLongerNeeded => '이제 Shizuku가 필요하지 않습니다.';

  @override
  String get sharedGetShizukuDescription => '일회성 접근 권한을 적용하려면 Shizuku를 설치하십시오.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Shizuku에서 무선 디버깅을 사용한 다음 돌아오십시오.';

  @override
  String get sharedApproveInShizuku => 'Shizuku에서 CopyPaste를 한 번 승인하십시오.';

  @override
  String get sharedGetShizuku => 'Shizuku 받기';

  @override
  String get sharedOpenShizuku => 'Shizuku 열기';

  @override
  String get sharedRunOnComputer => '컴퓨터에서 실행';

  @override
  String get sharedCopyAllCommands => '모든 명령어 복사';

  @override
  String get sharedEnableUsbDebugging => 'USB 디버깅을 켜고 이 휴대폰을 연결하십시오.';

  @override
  String get sharedAllow => '허용';

  @override
  String get sharedSetupCommandsCopyFailed => '설정 명령어를 복사할 수 없습니다.';

  @override
  String get sharedShizukuOpenFailed => 'Shizuku를 열 수 없습니다.';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => 'CopyPaste 설정';

  @override
  String get onboardingWelcomeTitle => 'CopyPaste에 오신 것을 환영합니다';

  @override
  String get onboardingWelcomeSubtitle => '필요할 때 언제든 준비된 클립보드.';

  @override
  String get onboardingReadyTitle => 'CopyPaste 준비 완료';

  @override
  String get onboardingGetStarted => '시작하기';

  @override
  String get onboardingOptionalSetupTitle => '선택 설정 확인 필요';

  @override
  String get onboardingOpenSystemSettings => '설정 열기';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return '$count단계 중 $step단계 · $platform';
  }

  @override
  String get onboardingSetupErrorTitle => '설정 확인 필요';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste가 $platform 설정 상태를 읽을 수 없습니다.';
  }

  @override
  String get onboardingSaveProgressFailed => 'CopyPaste가 설정 진행 상황을 저장할 수 없습니다.';

  @override
  String get onboardingStartAtLogin => '로그인 시 시작';

  @override
  String get onboardingStartAtLoginDescription => '로그인할 때 CopyPaste 열기';

  @override
  String get onboardingStartAtLoginSemantics => '로그인 시 CopyPaste 시작';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      '로그인 시 시작을 켤 수 없습니다. 이 기능 없이 계속할 수 있습니다.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      '로그인 시 시작 설정을 업데이트할 수 없습니다. 이 기능 없이 계속할 수 있습니다.';

  @override
  String get onboardingMacosTitle => 'Mac 설정';

  @override
  String get onboardingAccessibility => '손쉬운 사용';

  @override
  String get onboardingAutoPasteEnabled => '자동 붙여넣기 켜짐';

  @override
  String get onboardingAutoPasteOptional => '자동 붙여넣기용 선택 사항';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      '설치된 CopyPaste 앱에서 사용할 수 있습니다.';

  @override
  String get onboardingLoginItemApprovalTitle => '로그인 항목 승인 필요';

  @override
  String get onboardingLoginItemApprovalMessage =>
      '시스템 설정에서 CopyPaste를 허용한 다음 돌아오십시오.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste가 macOS 설정 상태를 읽을 수 없습니다.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      '손쉬운 사용 권한을 요청할 수 없습니다.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste가 macOS 설정 상태를 새로 고칠 수 없습니다.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      '손쉬운 사용 상태를 새로 고칠 수 없습니다.';

  @override
  String get onboardingMacosLoginItemsOpenFailed => '로그인 항목 설정을 열 수 없습니다.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      '로그인 시 시작하려면 시스템 설정에서 승인해야 합니다.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed => '로그인 시 시작을 끌 수 없습니다.';

  @override
  String get onboardingWindowsTitle => 'PC 설정';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      '이 설치본에서는 사용할 수 없습니다. 이 기능 없이 계속할 수 있습니다.';

  @override
  String get onboardingTaskManagerDisabledTitle => '작업 관리자에서 비활성화됨';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Windows 시작 프로그램에서 CopyPaste를 켠 다음 돌아오십시오.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      '작업 관리자에서 로그인 시 시작이 비활성화되어 있습니다. 나중에 Windows 설정에서 켤 수 있습니다.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      '로그인 시 시작을 사용할 수 없습니다. 이 기능 없이 계속할 수 있습니다.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste가 Windows 설정 상태를 읽을 수 없습니다.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Windows 시작 프로그램 설정을 열 수 없습니다.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      '로그인 시 시작 설정을 업데이트할 수 없습니다. 나중에 설정에서 변경할 수 있습니다.';

  @override
  String get onboardingLinuxChecking => 'Linux 데스크톱 세션을 확인하는 중입니다.';

  @override
  String get onboardingLinuxTitle => 'Linux 데스크톱 설정';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste에서 클립보드 출처 확인과 Quick Paste를 사용하려면 검증된 데스크톱 통합이 필요합니다.';

  @override
  String get onboardingLinuxSession => '데스크톱 세션';

  @override
  String get onboardingLinuxX11Detected => 'X11 세션이 감지되었습니다.';

  @override
  String get onboardingLinuxWaylandDetected => 'Wayland 세션이 감지되었습니다.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      '지원되는 X11 또는 Wayland 데스크톱 세션이 감지되지 않았습니다.';

  @override
  String get onboardingLinuxCompanion => 'GNOME 또는 KDE 컴패니언';

  @override
  String get onboardingLinuxCompanionReady => '인증된 Clipboard v2 브리지가 준비되었습니다.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      '출처 앱 제외 설정으로 인해 클립보드 수집이 일시 정지되었습니다. 수집을 재개하려면 서명된 컴패니언을 켜십시오.';

  @override
  String get onboardingLinuxCompanionInstall =>
      '서명된 컴패니언 세션 패키지를 설치하고 켠 다음 여기로 돌아오십시오.';

  @override
  String get onboardingLinuxOpenSetup => '설정 안내 열기';

  @override
  String get onboardingLinuxKeyboardControl => '키보드 제어';

  @override
  String get onboardingLinuxKeyboardAllowed => 'Quick Paste에 키보드 제어가 허용되었습니다.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      '이 데스크톱 세션에서는 키보드 제어를 사용할 수 없습니다.';

  @override
  String get onboardingLinuxAllowKeyboard => '키보드 제어 허용';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste가 준비되었습니다.';

  @override
  String get onboardingLinuxQuickPastePending =>
      '필요한 데스크톱 통합이 활성화되면 Quick Paste가 준비됩니다.';

  @override
  String get onboardingLinuxStartAtLoginDescription => '로그인할 때 CopyPaste를 엽니다.';

  @override
  String get onboardingLinuxLinks => 'CopyPaste 링크';

  @override
  String get onboardingLinuxLinksRegistered => 'CopyPaste 링크가 이 앱에서 열립니다.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      '페어링 초대를 위해 CopyPaste 링크를 등록하십시오.';

  @override
  String get onboardingLinuxRegisterLinks => '링크 등록';

  @override
  String get onboardingLinuxScreenshotBlocking => '스크린샷 차단';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Linux에서는 스크린샷 차단을 사용할 수 없습니다. 보호된 저장 데이터는 계속 암호화됩니다.';

  @override
  String get onboardingLinuxCheckAgain => '다시 확인';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste가 Linux 통합 상태를 읽을 수 없습니다.';

  @override
  String get onboardingLinuxSetupOpenFailed => '데스크톱 통합 설정을 열 수 없습니다.';

  @override
  String get onboardingLinuxKeyboardDenied => '키보드 제어 권한이 허용되지 않았습니다.';

  @override
  String get onboardingLinuxStartAtLoginFailed => '로그인 시 시작 설정을 업데이트할 수 없습니다.';

  @override
  String get onboardingLinuxLinksFailed => 'CopyPaste 링크를 등록할 수 없습니다.';

  @override
  String get onboardingLinuxStatusRefreshFailed => 'Linux 통합 상태를 새로 고칠 수 없습니다.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      '이 X11 세션에서는 클립보드 접근을 사용할 수 없습니다.';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste를 사용하려면 X11 또는 Wayland 데스크톱 세션이 필요합니다.';

  @override
  String get onboardingAndroidFullCapture => '전체 수집';

  @override
  String get onboardingAndroidBackgroundCapture => '백그라운드 수집';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      '한 번 설정한 다음 실제 복사로 확인합니다.';

  @override
  String get onboardingAndroidLimitedCapture => '제한 수집';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'CopyPaste로 공유하거나 복사한 후 앱으로 돌아옵니다.';

  @override
  String get onboardingAndroidNotification => '수집 알림';

  @override
  String get onboardingAndroidNotificationDescription => '수집이 실행되는 동안 필요';

  @override
  String get onboardingAndroidBackgroundActivity => '백그라운드 활동';

  @override
  String get onboardingAndroidBackgroundActivityDescription => '수집 누락을 줄이려면 권장';

  @override
  String get onboardingAndroidOpen => '열기';

  @override
  String get onboardingAndroidApplyAccess => '수집 접근 권한 적용';

  @override
  String get onboardingAndroidApplyAccessDescription => '일회성 수집 접근 권한을 적용합니다.';

  @override
  String get onboardingAndroidFullCaptureLater => '전체 수집은 나중에 설정할 수 있습니다.';

  @override
  String get onboardingAndroidCaptureWorking => '백그라운드 수집 작동 중';

  @override
  String get onboardingAndroidVerifyCopy => '실제 복사로 확인';

  @override
  String get onboardingAndroidCopyReached => '새 복사 항목이 로컬 기록에 도착했습니다.';

  @override
  String get onboardingAndroidCopyInstruction => '다른 앱에서 복사한 다음 여기로 돌아오십시오.';

  @override
  String get onboardingAndroidStartCapture => '수집 시작';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste가 Android 설정 상태를 읽을 수 없습니다.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Android 수집 상태를 새로 고칠 수 없습니다.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Android 설정 모니터링을 사용할 수 없습니다.';

  @override
  String get onboardingAndroidIntakeFailed => '클립보드 수신을 켤 수 없습니다.';

  @override
  String get onboardingAndroidStopCaptureFailed => '백그라운드 수집을 중지할 수 없습니다.';

  @override
  String get onboardingAndroidBatterySettingsFailed => '배터리 설정을 열 수 없습니다.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'Shizuku에서 CopyPaste가 허용되었지만 Android 수집 권한을 적용할 수 없습니다.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      '수집 권한을 적용하려면 Shizuku에서 CopyPaste를 허용하십시오.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      '먼저 백그라운드 클립보드 접근 권한을 부여하십시오.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      '백그라운드 수집을 시작하기 전에 알림을 허용하십시오.';

  @override
  String get onboardingAndroidStartCaptureFailed => '백그라운드 수집을 시작할 수 없습니다.';

  @override
  String get onboardingAndroidSetupOpenFailed => 'Android 수집 설정을 열 수 없습니다.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Android 설정 선택 사항을 저장할 수 없습니다.';

  @override
  String get modulesMarketplaceTab => '마켓플레이스';

  @override
  String get modulesInstalledTab => '설치됨';

  @override
  String get modulesRefresh => '새로 고침';

  @override
  String get modulesSearchPlaceholder => '모듈 검색';

  @override
  String get modulesOperationFailed => '모듈 작업 실패';

  @override
  String get modulesLoading => '모듈을 불러오는 중입니다.';

  @override
  String get modulesUnavailableTitle => '모듈을 사용할 수 없음';

  @override
  String get modulesUnavailableMessage => '모듈을 다시 불러오십시오.';

  @override
  String get modulesNoneInstalled => '설치된 모듈 없음';

  @override
  String get modulesNoMatches => '일치하는 모듈 없음';

  @override
  String get modulesChooseFromMarketplace => '마켓플레이스에서 모듈을 선택하십시오.';

  @override
  String get modulesTryDifferentSearch => '다른 검색어로 시도하십시오.';

  @override
  String get modulesMarketplaceLoading => '마켓플레이스를 불러오는 중입니다.';

  @override
  String get modulesMarketplaceUnavailableTitle => '마켓플레이스를 사용할 수 없음';

  @override
  String get modulesMarketplaceUnavailableMessage => '마켓플레이스를 다시 불러오십시오.';

  @override
  String get modulesNoneAvailable => '사용 가능한 모듈 없음';

  @override
  String get modulesNoneAvailableMessage => '모듈이 게시되면 여기에 표시됩니다.';

  @override
  String modulesEnableModule(String module) {
    return '$module 켜기';
  }

  @override
  String get modulesInstall => '설치';

  @override
  String modulesUpdateTo(String version) {
    return '$version 버전으로 업데이트';
  }

  @override
  String get modulesRemoveModule => '모듈 제거';

  @override
  String get modulesModuleSettings => '모듈 설정';

  @override
  String modulesSettingsTitle(String module) {
    return '$module 설정';
  }

  @override
  String get modulesDownloadingModel => '언어 모델 다운로드 중…';

  @override
  String get modulesRun => '실행';

  @override
  String modulesRemoveTitle(String module) {
    return '$module 모듈을 제거하시겠습니까?';
  }

  @override
  String get modulesRemoveMessage => '모듈과 해당 로컬 설정이 제거됩니다.';

  @override
  String get modulesRemove => '제거';

  @override
  String get modulesRemoveFailed => '모듈을 제거할 수 없습니다.';

  @override
  String get modulesCloseSettings => '모듈 설정 닫기';

  @override
  String get modulesRemoved => '모듈 제거됨';

  @override
  String get modulesNoSettingsTitle => '추가 설정 없음';

  @override
  String get modulesNoSettingsMessage => '이 모듈은 추가 구성이 필요하지 않습니다.';

  @override
  String get modulesSetUpSmsAccess => 'SMS 접근 권한 설정';

  @override
  String get modulesRestartApp => 'CopyPaste 다시 시작';

  @override
  String get modulesSmsAccessTitle => 'SMS 접근 권한';

  @override
  String get modulesCloseSmsAccess => 'SMS 접근 권한 닫기';

  @override
  String get modulesSmsChecking => 'SMS 접근 권한을 확인하는 중입니다.';

  @override
  String get modulesSmsReady =>
      '접근 권한이 준비되었습니다. 새 코드 복사를 시작하려면 SMS Codes를 켜십시오.';

  @override
  String get modulesSmsAllowAccess => '새 SMS 메시지의 코드를 복사하려면 접근을 허용하십시오.';

  @override
  String get modulesSmsNotification => 'SMS 알림';

  @override
  String get modulesSmsNotificationDescription => 'SMS Codes 실행 중 필요';

  @override
  String get modulesSmsApplyAccess => 'SMS 접근 권한 적용';

  @override
  String get modulesSmsApplyAccessDescription => '일회성 SMS 접근 권한을 적용합니다.';

  @override
  String get modulesSmsSetupFailed => 'SMS 설정 실패';

  @override
  String get modulesFileSelectionFailed => '파일 선택 실패';

  @override
  String get modulesChooseLanguages => '언어 선택';

  @override
  String get modulesChooseFile => '파일 선택';

  @override
  String modulesModelReady(String model) {
    return '$model 모델 준비됨';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return '$model 모델 · $size MiB 다운로드';
  }

  @override
  String get modulesPlatformsUnknown => '플랫폼: 알 수 없음';

  @override
  String modulesPlatforms(String platforms) {
    return '플랫폼: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid => '모듈 카탈로그 서명이 유효하지 않습니다.';

  @override
  String get modulesMarketplaceConnectFailed => '모듈 마켓플레이스에 연결할 수 없습니다.';

  @override
  String get modulesMarketplaceTimeout => '모듈 마켓플레이스가 응답하지 않았습니다.';

  @override
  String get modulesIncompatible => '이 모듈은 이 기기와 호환되지 않습니다.';

  @override
  String get modulesPackageSizeInvalid => '모듈 패키지 크기가 유효하지 않습니다.';

  @override
  String get modulesDownloadInterrupted => '모듈 다운로드가 중단되었습니다.';

  @override
  String get modulesDownloadTimeout => '모듈 다운로드가 응답하지 않았습니다.';

  @override
  String get modulesPackageUrlUntrusted => '모듈 패키지 URL을 신뢰할 수 없습니다.';

  @override
  String get modulesDownloadLeftGithub => '모듈 다운로드가 GitHub 외부로 이동했습니다.';

  @override
  String get modulesDownloadTooManyRedirects => '모듈 다운로드에서 리디렉션이 너무 많이 발생했습니다.';

  @override
  String get modulesDownloadTooLarge => '모듈 다운로드가 너무 큽니다.';

  @override
  String get modulesPackageIntegrityFailed => '모듈 패키지가 무결성 검사를 통과하지 못했습니다.';

  @override
  String get modulesDownloadUnavailable => '현재 이 모듈을 다운로드할 수 없습니다.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      '모듈 마켓플레이스를 사용할 수 없습니다. 다시 시도하십시오.';

  @override
  String get modulesCatalogInvalid => '모듈 카탈로그가 유효하지 않습니다.';

  @override
  String get modulesThisSystem => '이 시스템';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version 이상';
  }

  @override
  String get modulesNotAvailableForDevice => '이 기기에서는 사용할 수 없습니다.';

  @override
  String get modulesAppVersionUnverified => '설치된 CopyPaste 버전을 확인할 수 없습니다.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return '이 모듈은 CopyPaste $version 버전을 지원하지 않습니다.';
  }

  @override
  String modulesRequires(String requirement) {
    return '필요 조건: $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed => '모듈 마켓플레이스를 불러올 수 없습니다.';

  @override
  String get modulesSmsSetupRequired => '이 모듈을 켜기 전에 SMS 접근 권한을 설정하십시오.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'SMS 모니터링을 시작할 수 없습니다. 모듈이 아직 켜져 있습니다. 다시 시도하려면 모듈을 다시 끄십시오.';

  @override
  String get modulesSmsMonitoringDisabled => 'SMS 모니터링을 시작할 수 없습니다. 모듈이 꺼졌습니다.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'SMS 모니터링을 시작할 수 없습니다. 모듈의 켜짐 상태를 확인할 수 없습니다.';

  @override
  String get modulesSmsMonitoringFailed => 'SMS 모니터링을 시작할 수 없습니다.';

  @override
  String get modulesSmsAndroidOnly => 'SMS 접근 권한은 Android에서만 사용할 수 있습니다.';

  @override
  String get modulesSmsMonitoringNeedsSetup => 'SMS 모니터링을 설정해야 합니다.';

  @override
  String get modulesOperationIncomplete => '모듈 작업을 완료할 수 없습니다.';

  @override
  String get modulesUnsupportedEvent => '모듈이 지원되지 않는 이벤트를 사용합니다.';

  @override
  String get modulesUnsupportedResult => '모듈이 지원되지 않는 결과를 반환했습니다.';

  @override
  String get modulesUnsupportedField => '모듈이 지원되지 않는 필드를 사용합니다.';

  @override
  String get modulesResponseInvalid => '모듈 응답이 유효하지 않습니다.';

  @override
  String get modulesFileSelectionUnavailable => '파일 선택을 사용할 수 없습니다.';

  @override
  String get modulesFileSelectFailed => '파일을 선택할 수 없습니다.';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field: 허용되지 않는 파일입니다.';
  }

  @override
  String get modulesSmsVerifyFailed => 'SMS 접근 권한을 확인할 수 없습니다.';

  @override
  String get modulesSmsAllowNotifications =>
      'SMS Codes가 백그라운드에서 계속 작동하도록 알림을 허용하십시오.';

  @override
  String get modulesSmsNotGranted => 'SMS 접근 권한이 부여되지 않았습니다.';

  @override
  String get modulesSmsConfigureFailed => 'SMS 접근 권한을 구성할 수 없습니다.';

  @override
  String get onboardingLinuxContinueBasic => '기본 수집으로 계속';

  @override
  String get onboardingLinuxCompanionOptional =>
      '선택 사항입니다. Quick Paste와 출처 앱 제외에 필요합니다.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      '선택 사항입니다. Quick Paste가 이전 앱으로 돌아가는 데 필요합니다.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      '이 Wayland 세션에서는 클립보드 접근을 사용할 수 없습니다. 서명된 GNOME 또는 KDE 컴패니언을 켠 다음 다시 확인하십시오.';

  @override
  String get historyMoreActions => '추가 작업';

  @override
  String get historyTagsAction => '태그…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '기밀 항목 잠금 해제',
      'other': '기밀 항목 잠금 해제',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '숨김 스니펫 사용',
      'other': '숨김 스니펫 사용',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': '기밀 항목 복사',
      'other': '기밀 항목 복사',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => '이 시스템에서는 사용할 수 없음';

  @override
  String get deviceAuthNotConfigured => '먼저 기기 암호, PIN 또는 생체 인증을 설정하세요';

  @override
  String get commonOn => '켜짐';

  @override
  String get commonOff => '꺼짐';

  @override
  String get settingsQuickPasteStartFailed =>
      'Quick Paste를 시작할 수 없어 단축키가 작동하지 않습니다.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut은(는) 다른 앱에서 사용 중입니다. 다른 단축키를 선택하세요.';
  }

  @override
  String get devicesEmptyTitle => '아직 페어링된 다른 기기가 없습니다';

  @override
  String get devicesEmptyMessage =>
      '다른 기기를 페어링하면 기기 간에 클립보드 기록이 동기화됩니다. 두 기기가 같은 네트워크에 있어야 하며, 모든 페어링은 두 기기에서 확인합니다.';

  @override
  String get pairingStateNotCompleted => '페어링이 완료되지 않음';

  @override
  String get pairingStepInvite => '초대';

  @override
  String get pairingStepScan => '스캔';

  @override
  String get pairingStepCode => '코드';

  @override
  String get pairingStepRequest => '요청';

  @override
  String get pairingStepConnect => '연결';

  @override
  String get pairingStepConfirm => '확인';

  @override
  String get pairingStepPaired => '페어링됨';

  @override
  String get pairingOtherDevice => '다른 기기';

  @override
  String get pairingDecisionNotConfirmed => '아직 확인되지 않음';

  @override
  String get pairingDecisionSending => '결정을 보내는 중…';

  @override
  String get pairingDecisionConfirmed => '확인됨';

  @override
  String get pairingDecisionRejected => '거부됨';

  @override
  String get pairingDecisionWaiting => '확인을 기다리는 중';

  @override
  String get pairingDecisionUnknown => '응답을 알 수 없음';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '기기 $count대',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count줄',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => '기본 탐색';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return '$count단계 중 $step단계: $name';
  }

  @override
  String get modulesPhaseDownload => '다운로드';

  @override
  String get modulesPhaseInstall => '설치';

  @override
  String modulesInstallingModule(String module) {
    return '$module 설치 중';
  }

  @override
  String modulesUpdatingModule(String module) {
    return '$module 업데이트 중';
  }

  @override
  String get commonDismiss => '닫기';

  @override
  String get settingsRetentionConfirmTitle => '오래된 기록을 제거하시겠습니까?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return '새 한도: $limit. 이보다 오래된 고정되지 않은 항목은 지금 이 기기에서 제거되며 복구할 수 없습니다. 고정된 항목은 유지됩니다.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle => '저장 공간 할당량을 줄이시겠습니까?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return '새 할당량: $size. 고정되지 않은 기록이 이보다 많은 공간을 사용하면 가장 오래된 항목부터 지금 제거되며 복구할 수 없습니다. 고정된 항목은 유지됩니다.';
  }

  @override
  String get settingsLimitConfirmAction => '한도 적용';

  @override
  String get settingsHideEverywhereTitle => 'CopyPaste를 완전히 숨기시겠습니까?';

  @override
  String get settingsHideEverywhereMessage =>
      '둘 다 끄면 CopyPaste 아이콘이 어디에도 표시되지 않습니다. 다시 열려면 CopyPaste를 다시 실행하십시오.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return '둘 다 끄면 CopyPaste 아이콘이 어디에도 표시되지 않습니다. 다시 열려면 $shortcut 키를 누르거나 CopyPaste를 다시 실행하십시오.';
  }

  @override
  String get settingsHideEverywhereAction => '숨기기';

  @override
  String get settingsExportDoneTitle => '텍스트 기록을 내보냈습니다';

  @override
  String get settingsBackupDoneTitle => '백업을 생성했습니다';

  @override
  String get settingsRestoreDoneTitle => '기록을 복원했습니다';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '텍스트 클립 $count개를 내보냈습니다',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count개는 읽을 수 없었습니다',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return '암호화된 백업을 저장했습니다 · $size';
  }

  @override
  String get settingsRestoreDoneMessage => '이 기기의 클립보드 기록이 백업 내용으로 바뀌었습니다.';

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
    return '$quota 중 $used 사용';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '$used 사용';
  }

  @override
  String get storageUsageOverQuota => '할당량 초과';

  @override
  String storageUsageOldestClip(String date) {
    return '가장 오래된 클립: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return '저장 공간: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return '새 할당량: $size. 고정되지 않은 기록이 $used를 사용하므로 할당량에 맞을 때까지 가장 오래된 클립부터 지금 제거됩니다. 제거된 클립은 복구할 수 없습니다. 고정된 항목은 유지됩니다.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count개',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'CopyPaste 열기';

  @override
  String get trayOpenQuickPaste => 'Quick Paste 열기';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Quick Paste 열기 ($shortcut)';
  }

  @override
  String get trayNewSnippet => '새 스니펫';

  @override
  String get trayRecent => '최근';

  @override
  String get trayRecentLoading => '불러오는 중…';

  @override
  String get trayRecentFailed => '클립을 불러올 수 없음';

  @override
  String get trayRecentEmpty => '아직 클립 없음';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '최근 클립 $count개 숨김',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => '비밀';

  @override
  String get trayCopyLatestFromOtherDevice => '다른 기기의 최신 클립 복사';

  @override
  String get trayCaptureNow => '지금 수집';

  @override
  String trayPausedUntil(String time) {
    return '$time까지 일시 정지됨';
  }

  @override
  String get trayPauseFor => '일시 정지 시간…';

  @override
  String get trayPauseFor15Minutes => '15분';

  @override
  String get trayPauseForOneHour => '1시간';

  @override
  String get trayPauseUntilResumed => '재개할 때까지';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count개 항목',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '수집 중: $count개 항목',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return '최근 건너뜀 ($count)';
  }

  @override
  String get trayOpenContentRules => '콘텐츠 규칙 열기';

  @override
  String get trayRecentSettingsTitle => '메뉴 막대 및 트레이';

  @override
  String get trayRecentSettingsDescription =>
      '메뉴 막대 또는 트레이 메뉴의 최근 항목에 표시할 내용입니다.';

  @override
  String get trayRecentCountTitle => '최근 클립';

  @override
  String get trayRecentCountDescription => '메뉴에 표시할 최신 클립 수입니다.';

  @override
  String get trayRecentPreviewTitle => '최근 클립 미리보기';

  @override
  String get trayRecentPreviewDescription =>
      '메뉴 텍스트는 CopyPaste 암호화 밖에서 시스템이 보관합니다. 클립 내용이 남지 않도록 덜 표시하세요. 비밀은 항상 가려집니다.';

  @override
  String get trayRecentPreviewFull => '전체 미리보기';

  @override
  String get trayRecentPreviewTitles => '유형 및 출처만';

  @override
  String get trayRecentPreviewHidden => '숨김';

  @override
  String get trayRecentSettingsKeywords => '트레이 메뉴 막대 최근 미리보기 개인정보 개수';

  @override
  String get transformsPaletteTitle => '텍스트 변환';

  @override
  String get transformsFilterPlaceholder => '변환 필터';

  @override
  String get transformsNoMatches => '일치하는 변환 없음';

  @override
  String get transformsGroupResults => '결과';

  @override
  String get transformsGroupSuggested => '추천';

  @override
  String get transformsGroupRecent => '최근';

  @override
  String get transformsGroupFavorites => '즐겨찾기';

  @override
  String get transformsGroupPipelines => '파이프라인';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV 및 XML';

  @override
  String get transformsCategoryEncode => '인코딩 및 디코딩';

  @override
  String get transformsCategoryCase => '대소문자';

  @override
  String get transformsCategoryHash => '해시';

  @override
  String get transformsCategoryLines => '줄 및 공백';

  @override
  String get transformsCategoryClean => '정리';

  @override
  String get transformsCategoryUrl => '링크';

  @override
  String get transformsCategoryMarkup => '서식 있는 텍스트 및 Markdown';

  @override
  String get transformsCategoryDev => '개발자';

  @override
  String get transformsCategoryOther => '기타';

  @override
  String get transformsChainEmpty => '아직 단계 없음';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return '다음: $name';
  }

  @override
  String get transformsStepEnabled => '이 단계 실행';

  @override
  String get transformsMoveEarlier => '위로 이동';

  @override
  String get transformsMoveLater => '아래로 이동';

  @override
  String get transformsRemoveStep => '단계 제거';

  @override
  String get transformsSavePipeline => '파이프라인으로 저장';

  @override
  String get transformsSavePipelineTitle => '파이프라인 저장';

  @override
  String get transformsSavePipelineFailed => '파이프라인을 저장할 수 없습니다.';

  @override
  String get transformsPipelineNameLabel => '이름';

  @override
  String get transformsPipelineDescriptionLabel => '설명(선택 사항)';

  @override
  String get transformsPipelineStepsLabel => '단계';

  @override
  String get transformsPipelineNoSteps => '단계를 하나 이상 추가하세요.';

  @override
  String get transformsPipelineNameRequired => '이름을 입력하세요.';

  @override
  String get transformsPipelineNameTooLong => '80자 이하로 입력하세요.';

  @override
  String get transformsPipelineNameTaken => '같은 이름의 파이프라인이 이미 있습니다.';

  @override
  String get transformsPipelineSavedTitle => '파이프라인 저장됨';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name을(를) 모든 변환 팔레트에서 사용할 수 있습니다.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return '단계: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => '삭제된 파이프라인';

  @override
  String get transformsNeedsNewerVersion => '최신 버전의 CopyPaste가 필요합니다';

  @override
  String get transformsLoadFailed => '변환을 불러올 수 없습니다.';

  @override
  String get transformsNotTextTitle => '변환은 텍스트에서 작동합니다';

  @override
  String get transformsNotTextMessage =>
      '이 항목은 이미지, 파일 또는 색상입니다. 텍스트 항목을 선택하거나 텍스트를 복사하세요.';

  @override
  String get transformsNothingTitle => '변환할 내용 없음';

  @override
  String get transformsNothingMessage => '텍스트를 복사하거나 기록 항목을 선택하세요.';

  @override
  String get transformsPickTransform => '변환을 선택하세요';

  @override
  String get transformsPickTransformHint => 'Tab으로 단계를 추가하고 Enter로 결과를 적용합니다.';

  @override
  String get transformsPreviewing => '미리보기 업데이트 중…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return '줄: $lines · 단어: $words · 문자: $chars';
  }

  @override
  String get transformsNoChanges => '변경 사항 없음';

  @override
  String transformsTruncated(String size) {
    return '$size 중 처음 64KiB 표시 중';
  }

  @override
  String get transformsRandom => '무작위';

  @override
  String get transformsNotRun => '실행 안 됨';

  @override
  String get transformsSensitiveHidden => '결과가 민감하여 숨겨져 있습니다.';

  @override
  String get transformsReveal => '표시';

  @override
  String get transformsPaneOutput => '출력';

  @override
  String get transformsPaneInput => '입력';

  @override
  String get transformsPaneChanges => '변경 사항';

  @override
  String get transformsCannotTransform => '이 텍스트를 변환할 수 없습니다';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return '$step단계 · $name: $message';
  }

  @override
  String get transformsUseHexOutput => '16진수로 표시';

  @override
  String get transformsActionPaste => '붙여넣기';

  @override
  String get transformsActionSave => '새 항목으로 저장';

  @override
  String get transformsCopiedMessage => '결과가 클립보드와 기록에 있습니다.';

  @override
  String get transformsSavedTitle => '저장됨';

  @override
  String get transformsSavedMessage => '결과가 새 기록 항목이 되었습니다. 클립보드는 그대로입니다.';

  @override
  String get transformsPasteFailedTitle => '붙여넣을 수 없음';

  @override
  String get transformsPasteFailedMessage => '결과가 클립보드에 있습니다. 직접 붙여넣으세요.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message($line행 $column열)';
  }

  @override
  String get transformsErrorBinary => '디코딩된 데이터가 텍스트가 아닌 바이너리입니다';

  @override
  String get transformsErrorTooLarge => '너무 커서 변환할 수 없습니다';

  @override
  String get transformsErrorInvalidParam => '이 단계의 설정이 올바르지 않습니다';

  @override
  String get transformsErrorCancelled => '취소됨';

  @override
  String get transformsErrorTooExpensive => '패턴 처리 비용이 너무 큽니다';

  @override
  String get transformsErrorPattern => '패턴이 올바르지 않습니다';

  @override
  String get transformsErrorTimestampUnit => '숫자가 초인지 밀리초인지 선택하세요';

  @override
  String get transformsErrorExternalEntity => '외부 엔터티는 허용되지 않습니다';

  @override
  String get transformsErrorJwtEncrypted => '암호화된 토큰은 디코딩할 수 없습니다';

  @override
  String get transformsErrorJson => '잘못된 JSON';

  @override
  String get transformsErrorYaml => '잘못된 YAML';

  @override
  String get transformsErrorXml => '잘못된 XML';

  @override
  String get transformsErrorCsv => '잘못된 CSV';

  @override
  String get transformsErrorBase64 => '잘못된 Base64';

  @override
  String get transformsErrorHex => '잘못된 16진수';

  @override
  String get transformsErrorEscape => '잘못된 이스케이프 시퀀스';

  @override
  String get transformsErrorUrl => '잘못된 퍼센트 인코딩';

  @override
  String get transformsErrorJwt => '유효한 JWT가 아닙니다';

  @override
  String get transformsErrorDate => '인식할 수 있는 날짜나 타임스탬프가 아닙니다';

  @override
  String get transformsErrorGeneric => '이 단계는 입력을 변환할 수 없습니다';

  @override
  String get transformsWarningSignedLink => '서명된 링크는 변경하지 않았습니다';

  @override
  String get transformsWarningExpired => '만료됨';

  @override
  String get transformsWarningNotYetValid => '아직 유효하지 않음';

  @override
  String get transformsWarningSignatureNotVerified => '서명이 확인되지 않음';

  @override
  String get transformsWarningDuplicateKeys => '중복된 키';

  @override
  String get transformsWarningLenientJson => '비표준 JSON을 허용했습니다';

  @override
  String get transformsWarningDoubleEncoded => 'JSON이 두 번 인코딩되어 있었습니다';

  @override
  String get transformsWarningEscapes => '알 수 없는 이스케이프 시퀀스를 그대로 유지했습니다';

  @override
  String get transformsWarningLocalZone => '현지 시간대로 가정했습니다';

  @override
  String get transformsWarningNoMatches => '일치 항목 없음';

  @override
  String get transformsWarningGeneric => '결과를 확인하세요';

  @override
  String get transformsRestoreTitle => '붙여넣은 후 클립보드 복원';

  @override
  String get transformsRestoreDescription =>
      '변환 결과를 붙여넣은 후, 그사이에 다른 것을 복사하지 않았다면 이전 클립보드로 되돌립니다.';

  @override
  String get transformsDefaultTitle => '붙여넣기 기본 변환';

  @override
  String get transformsDefaultDescription =>
      'Quick Paste가 텍스트에 이 파이프라인을 적용하고 붙여넣기 전에 표시합니다. 실패하면 원본을 붙여넣습니다.';

  @override
  String get transformsNoDefault => '없음';

  @override
  String get transformsAppDefaultsTitle => '앱별 기본값';

  @override
  String get transformsAppDefaultsDescription =>
      '번들 ID, 실행 파일 또는 패키지 이름으로 앱별 기본값을 재정의합니다. 다른 플랫폼에서 만든 항목은 동기화되지만 여기서는 적용되지 않습니다.';

  @override
  String get transformsAppIdentifierPlaceholder => '앱 식별자';

  @override
  String transformsOtherPlatform(String platform) {
    return '$platform에서만';
  }

  @override
  String get transformsAdd => '추가';

  @override
  String get transformsRemove => '제거';

  @override
  String get transformsFavoritesTitle => '즐겨찾기';

  @override
  String get transformsFavoritesDescription =>
      '즐겨찾기는 모든 팔레트에서 먼저 표시됩니다. 처음 9개는 Mod+1~Mod+9로 실행합니다.';

  @override
  String transformsFavoriteShortcut(int number) {
    return '팔레트에서 Mod+$number';
  }

  @override
  String get transformsAddFavorite => '즐겨찾기 추가';

  @override
  String get transformsPipelinesTitle => '파이프라인';

  @override
  String get transformsPipelinesDescription =>
      '설정과 함께 저장한 변환 체인입니다. 다른 기기와 동기화됩니다.';

  @override
  String get transformsNewPipeline => '새 파이프라인';

  @override
  String get transformsEditPipeline => '파이프라인 편집';

  @override
  String get transformsDuplicatePipeline => '복제';

  @override
  String get transformsDeletePipeline => '삭제';

  @override
  String transformsDeletePipelineTitle(String name) {
    return '$name을(를) 삭제할까요?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      '이를 사용하는 즐겨찾기와 붙여넣기 기본값은 모든 기기에서 없음으로 돌아갑니다.';

  @override
  String get transformsDeletePipelineFailed => '파이프라인을 삭제할 수 없습니다.';

  @override
  String get transformsAddStep => '단계 추가';

  @override
  String get transformsHiddenTitle => '숨긴 변환';

  @override
  String get transformsHiddenDescription =>
      '숨긴 변환은 팔레트와 추천에 표시되지 않습니다. 이를 사용하는 파이프라인은 계속 실행됩니다.';

  @override
  String get transformsHideTransform => '변환 숨기기';

  @override
  String get transformsShow => '표시';

  @override
  String get transformsSuggestionsTitle => '추천';

  @override
  String get transformsSuggestionsDescription =>
      '팔레트를 열면 텍스트의 처음 64KiB를 아래 유형으로 확인해 최대 3개의 변환을 추천합니다. 백그라운드에서는 아무것도 실행하지 않습니다.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON 웹 토큰';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl => '추적 매개변수가 있는 링크';

  @override
  String get transformsDetectorTimestamp => '타임스탬프';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => '추적 매개변수';

  @override
  String get transformsTrackingDescription =>
      '기본 제공 목록은 앱에 포함되며 업데이트할 때만 바뀝니다. 제거하거나 유지할 이름을 추가하세요. name@domain 형식으로 도메인을 한정할 수 있습니다.';

  @override
  String get transformsTrackingRemoveLabel => '추가로 제거';

  @override
  String get transformsTrackingAllowLabel => '항상 유지';

  @override
  String get transformsTrackingPlaceholder => 'name 또는 name@domain 입력 후 Enter';

  @override
  String get quickAccessUnavailableTitle => '빠른 액세스 상태를 사용할 수 없음';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste가 키보드, 타일, 위젯 상태를 읽지 못했습니다. 다시 시도하세요.';

  @override
  String get quickAccessActionFailedTitle => '작업을 완료할 수 없음';

  @override
  String get quickAccessActionFailedMessage =>
      'Android가 요청한 화면을 열지 못했거나 변경 사항을 저장하지 못했습니다. 다시 시도하세요.';

  @override
  String get quickAccessKeyboardTitle => '클립 키보드';

  @override
  String get quickAccessKeyboardDescription =>
      '시스템 키보드 설정에서 CopyPaste 클립을 켜세요. Android는 키보드가 입력 내용을 볼 수 있다고 경고하지만, 클립 패널은 키 입력을 기록하지 않고, 다른 앱에 입력하는 키가 없으며, 탭한 항목만 삽입합니다.';

  @override
  String get quickAccessKeyboardOn => '켜짐';

  @override
  String get quickAccessKeyboardOff => '꺼짐';

  @override
  String get quickAccessOpenKeyboardSettings => '키보드 설정 열기';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'CopyPaste 클립 키보드 패널을 켭니다.';

  @override
  String get quickAccessKeyboardKeywords => '키보드 입력 방법 ime 패널 사용';

  @override
  String get quickAccessSwitchKeyboardTitle => '클립 키보드로 전환';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'CopyPaste 클립을 현재 키보드로 선택하세요. 선택된 동안 모든 복사 내용이 저장됩니다.';

  @override
  String get quickAccessSwitchKeyboardAction => '키보드 선택';

  @override
  String get quickAccessKeyboardCurrent => '현재 키보드';

  @override
  String get quickAccessCaptureTierTitle => '수집 모드';

  @override
  String get quickAccessCaptureTierDescription =>
      'Shizuku 또는 ADB 접근 권한이 없으면 클립 패널이 열릴 때, 패널이 선택된 동안, 또는 CopyPaste 앱, 타일, 위젯, 공유를 통해서만 복사 내용이 저장됩니다.';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      '다른 앱의 복사 내용이 저장되는 방식을 확인합니다.';

  @override
  String get quickAccessCaptureTierKeywords => '수집 단계 전체 패널 수동 shizuku 모으기';

  @override
  String get quickAccessTierFull => '전체: 모든 복사 내용을 백그라운드에서 저장';

  @override
  String get quickAccessTierPanelCollecting => '패널 선택됨: 수집 중';

  @override
  String get quickAccessTierPanelCatchUp => '패널 모드: 열 때 가져오기';

  @override
  String get quickAccessTierManual => '수동: 앱, 타일, 위젯 또는 공유에서 저장';

  @override
  String get quickAccessTierPaused => '수집 일시중지됨';

  @override
  String get quickAccessAutoReturnTitle => '삽입 후 이전 키보드로 돌아가기';

  @override
  String get quickAccessAutoReturnDescription =>
      '한 번 삽입하면 이전 키보드로 돌아갑니다. 대기열 탭이나 항목 작업을 사용하는 동안에는 패널이 열려 있습니다. 계속 복사 내용을 모으려면 끄세요.';

  @override
  String get quickAccessAutoReturnKeywords => '돌아가기 이전 키보드 자동 삽입';

  @override
  String get quickAccessFocusSearchTitle => '패널을 열 때 검색에 포커스';

  @override
  String get quickAccessFocusSearchDescription =>
      '검색창이 선택된 상태로 패널을 엽니다. 끄면 한 번의 탭으로 클립을 사용할 수 있습니다.';

  @override
  String get quickAccessTilesTitle => '빠른 설정 타일';

  @override
  String get quickAccessTilesSearchDescription =>
      '클립보드 저장 및 클립보드 수집 타일을 추가합니다.';

  @override
  String get quickAccessTilesKeywords => '빠른 설정 타일 알림창 저장 일시중지';

  @override
  String get quickAccessTileInstructions =>
      '화면 상단에서 아래로 두 번 스와이프하고 수정을 탭한 다음 CopyPaste 타일을 빠른 설정으로 드래그하세요.';

  @override
  String get quickAccessSaveTileTitle => '클립보드 저장 타일';

  @override
  String get quickAccessSaveTileDescription => '빠른 설정에서 클립보드 내용을 저장합니다.';

  @override
  String get quickAccessCaptureTileTitle => '클립보드 수집 타일';

  @override
  String get quickAccessCaptureTileDescription =>
      '빠른 설정에서 수집을 일시중지하고 다시 시작합니다.';

  @override
  String get quickAccessAddTile => '타일 추가';

  @override
  String get quickAccessTileAdded => '빠른 설정에 추가됨';

  @override
  String get quickAccessWidgetTitle => '최근 클립 위젯';

  @override
  String get quickAccessWidgetDescription =>
      '홈 화면에 최근 클립을 표시합니다. 클립을 탭하면 복사됩니다.';

  @override
  String get quickAccessWidgetInstructions =>
      '홈 화면을 길게 누르고 위젯을 탭한 다음 CopyPaste의 최근 클립을 드래그하세요.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '위젯 $count개 추가됨',
      zero: '추가되지 않음',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => '위젯 추가';

  @override
  String get quickAccessWidgetKeywords => '홈 화면 위젯 런처 최근';
}
