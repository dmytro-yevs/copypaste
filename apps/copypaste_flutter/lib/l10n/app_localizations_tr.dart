// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Turkish (`tr`).
class AppLocalizationsTr extends AppLocalizations {
  AppLocalizationsTr([String locale = 'tr']) : super(locale);

  @override
  String get commonCancel => 'Vazgeç';

  @override
  String get commonClose => 'Kapat';

  @override
  String get commonDone => 'Bitti';

  @override
  String get commonRetry => 'Yeniden dene';

  @override
  String get commonTryAgain => 'Tekrar dene';

  @override
  String get commonCopy => 'Kopyala';

  @override
  String get commonDelete => 'Sil';

  @override
  String get commonSave => 'Kaydet';

  @override
  String get commonContinue => 'Devam';

  @override
  String get commonBack => 'Geri';

  @override
  String get commonOpenSettings => 'Ayarları aç';

  @override
  String get commonLoading => 'Yükleniyor';

  @override
  String get commonApply => 'Uygula';

  @override
  String get commonClear => 'Temizle';

  @override
  String get commonPin => 'Sabitle';

  @override
  String get commonEnable => 'Etkinleştir';

  @override
  String get settingsLanguageTitle => 'Dil';

  @override
  String get settingsLanguageDescription =>
      'CopyPaste arayüzünün dilini seçin.';

  @override
  String get settingsLanguageSystem => 'Sistem varsayılanı';

  @override
  String get settingsLanguageKeywords =>
      'dil yerel ayar çeviri arayüz language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Girişte başlatma ayarı güncellenemedi. Tekrar deneyin.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'Sistem başlangıç ayarları açılamadı.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Ayarlar yenilenemedi. Tekrar deneyin.';

  @override
  String get settingsRuntimeLoadFailed => 'Ayarlar yüklenemedi.';

  @override
  String get settingsRuntimeAppAlreadyExcluded =>
      'Bu uygulama zaten hariç tutuluyor.';

  @override
  String get settingsRuntimeChangeFailed => 'İstenen değişiklik tamamlanamadı.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Yakalamanın sürdürülebilmesi için pano yakalama izinlerinin tamamlanması gerekir.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'Pano yakalama duraklatıldı. Android yakalama hizmeti durduğunu onaylamadı.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'Yakalama bildirimlerini etkinleştirmek için bildirim izni gerekir.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste kaydedilmiş ayarları okuyamadı. Koruyucu değerler etkin; onarmak için tercihlerinizi gözden geçirip kaydedin.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste okunamayan ayarları koruyucu değerlerle kurtardı. Onarmak için tercihlerinizi gözden geçirip kaydedin.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Uygulama tanımlayıcıları boş olmamalı ve en fazla 256 bayt olmalıdır.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'En fazla 256 uygulamayı hariç tutabilirsiniz.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Otomatik yapıştırma güncellenemedi.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'Bu kısayol kaydedilemedi.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'Geçerli kısayol duraklatılamadı.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'Kısayol geri yüklenemedi.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed => 'Quick Paste açılamadı.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Erişilebilirlik izni istenemedi.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Linux entegrasyon durumu yenilenemedi.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'Pano girişi izni verilmedi.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Pano girişi izni istenemedi.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Masaüstü entegrasyonu kurulumu açılamadı.';

  @override
  String get settingsRuntimeTextHistoryFileType => 'CopyPaste metin geçmişi';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'CopyPaste şifreli yedeği';

  @override
  String get settingsRuntimeNotificationImage => 'Görüntü';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Görüntü · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'Dosya';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste güncelleme kurulumunu geri yükleyemedi.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Bu Linux kurulumu otomatik olarak güncellenemez.';

  @override
  String get updateMacosPackageMissing =>
      'Bu sürümde imzalı bir macOS güncelleme paketi yok.';

  @override
  String get updateInstalledVersionInvalid =>
      'Yüklü CopyPaste sürümü geçersiz.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste, güncellemeleri denetlemek için GitHub\'a ulaşamadı.';

  @override
  String get updateCheckTimedOut => 'Güncelleme denetimi zaman aşımına uğradı.';

  @override
  String get updateCheckDeviceFailed =>
      'CopyPaste bu cihazda güncellemeleri denetleyemedi.';

  @override
  String get updateCheckFailed => 'CopyPaste güncellemeleri denetleyemedi.';

  @override
  String get updateDownloadFailed => 'CopyPaste güncellemeyi indiremedi.';

  @override
  String get updateDownloadTimedOut =>
      'Güncelleme indirmesi zaman aşımına uğradı.';

  @override
  String get updateInstallFailed => 'CopyPaste güncellemeyi yükleyemedi.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Android sistem yükleyicisinde devam edin.';

  @override
  String get updateContinueInPackageManager =>
      'Sistem paket yöneticinizde devam edin.';

  @override
  String get updateInstallerStarting => 'Yükleyici başlatılıyor.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Android ayarlarında CopyPaste\'in uygulama yüklemesine izin verin, ardından devam edin.';

  @override
  String get updatePermissionRequiredLinux =>
      'Bu güncellemeyi yüklemek için sistem paket yöneticinizde kimliğinizi doğrulayın.';

  @override
  String get updatePermissionRequired =>
      'CopyPaste\'in bu güncellemeyi yüklemesine izin verin, ardından devam edin.';

  @override
  String get updateRestartRequired =>
      'Güncelleme yüklendi. Kullanmak için CopyPaste\'i yeniden başlatın.';

  @override
  String get updateRestarting => 'CopyPaste yeniden başlatılıyor.';

  @override
  String get updateRestartFailed =>
      'CopyPaste yeniden başlatılamadı. Tekrar deneyin.';

  @override
  String get updateReleasePageOpenFailed =>
      'CopyPaste sürüm sayfasını açamadı.';

  @override
  String get updatePackageInvalid =>
      'İndirilen güncelleme geçerli bir CopyPaste paketi değil.';

  @override
  String get updateSignatureInvalid =>
      'İndirilen güncellemenin imzası geçersiz.';

  @override
  String get updateDowngradeRefused =>
      'İndirilen paket bu sürümden daha yeni değil.';

  @override
  String get updateHomebrewRequired =>
      'Burada güncellemek için CopyPaste\'i Homebrew ile yükleyin.';

  @override
  String get updateInstallationCancelled => 'Güncelleme kurulumu iptal edildi.';

  @override
  String get updateInstallationBlocked =>
      'Android güncelleme kurulumunu engelledi.';

  @override
  String get updateInstallationConflict =>
      'Güncelleme, yüklü uygulamayla çakışıyor.';

  @override
  String get updateInstallationIncompatible =>
      'Güncelleme bu cihazla uyumlu değil.';

  @override
  String get updateInstallationStorage =>
      'Güncellemeyi yüklemek için depolama alanı boşaltın.';

  @override
  String get updateInstallationInterrupted =>
      'Güncelleme kurulumu yarıda kesildi. Tekrar deneyin.';

  @override
  String get updateInstallationBusy =>
      'Zaten devam eden bir güncelleme kurulumu var.';

  @override
  String get updatePackageDetailsInvalid =>
      'Güncelleme paketi ayrıntıları geçersiz.';

  @override
  String get updateIntegrityCheckFailed =>
      'İndirilen güncelleme bütünlük denetiminden geçemedi.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste sistem yükleyicisini başlatamadı.';

  @override
  String get updateInstallLocationTranslocated =>
      'Güncellemek için CopyPaste\'i Uygulamalar klasörüne taşıyın ve oradan açın.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste bulunduğu klasörde kendini değiştiremiyor. Güncellemek için sürüm sayfasını kullanın.';

  @override
  String get updateReplacementBlocked =>
      'macOS, CopyPaste\'in değiştirilmesini engelledi. Uygulama Yönetimi ayarlarında izin verin veya sürüm sayfasını kullanın.';

  @override
  String get updatePackageNotDownloadable =>
      'Bu platform indirilebilir bir güncelleme paketi kullanmıyor.';

  @override
  String get updatePackageSizeInvalid =>
      'Güncelleme paketinin boyutu geçersiz.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'İndirilen güncellemenin güncelleyici imzası geçersiz.';

  @override
  String get updatePackageUrlUntrusted =>
      'Güncelleme paketinin URL\'si güvenilir değil.';

  @override
  String get updateDownloadLeftGitHub =>
      'Güncelleme indirmesi GitHub dışına yönlendirildi.';

  @override
  String get updateDownloadTooManyRedirects =>
      'Güncelleme indirmesi çok fazla yönlendirme kullandı.';

  @override
  String get updatePackageTooLarge => 'Güncelleme paketi çok büyük.';

  @override
  String get updateSignatureSizeInvalid =>
      'Güncelleyici imzasının boyutu geçersiz.';

  @override
  String get updateMetadataInvalid =>
      'GitHub geçersiz güncelleme meta verileri döndürdü.';

  @override
  String get updateMetadataTooLarge => 'Güncelleme meta verileri çok büyük.';

  @override
  String get quickPasteActionFailedTitle => 'İşlem başarısız oldu';

  @override
  String get quickPasteOpenAppSemantics => 'CopyPaste\'i aç';

  @override
  String get quickPasteSearchPlaceholder => 'Aramak için yazın…';

  @override
  String get quickPasteLoadingHistory => 'Pano geçmişi yükleniyor.';

  @override
  String get quickPasteHistoryUnavailable => 'Geçmiş kullanılamıyor';

  @override
  String get quickPasteNoClipsFound => 'Öğe bulunamadı';

  @override
  String get quickPasteLoadMore => 'Daha fazla yükle';

  @override
  String get quickPasteTransferFailed => 'Başarısız';

  @override
  String get quickPasteClipUnavailable => 'Öğe kullanılamıyor';

  @override
  String get quickPasteFileFallbackName => 'Dosya';

  @override
  String get quickPastePathUnavailable => 'Yol kullanılamıyor';

  @override
  String get quickPastePreferences => 'Tercihler…';

  @override
  String get quickPasteAbout => 'Hakkında';

  @override
  String get quickPasteQuit => 'Çık';

  @override
  String get quickPasteClearHistoryTitle => 'Geçmiş temizlensin mi?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Sabitlenmiş öğeler geçmişinizde kalır.';

  @override
  String get quickPasteClearHistoryFailed => 'Geçmiş silinemedi.';

  @override
  String get quickPasteAboutDescription =>
      'macOS, Android, Windows ve Linux\'ta özel pano geçmişi.';

  @override
  String get transfersTitle => 'İndirmeler ve aktarımlar';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count etkin aktarım',
      one: '$count etkin aktarım',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count etkin',
      one: '$count etkin',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'En son';

  @override
  String get transfersShowEarlier => 'Daha eski';

  @override
  String get transfersClearCompletedHistory => 'Tamamlanan geçmişi temizle';

  @override
  String get transfersErrorTitle => 'Aktarımlar';

  @override
  String get transfersEmptyTitle => 'Aktarım yok';

  @override
  String get transfersEmptyMessage =>
      'Dosya içe aktarmaları, indirmeler ve giden aktarımlar burada görünür.';

  @override
  String transfersSpeed(String size) {
    return '$size/sn';
  }

  @override
  String transfersBytesProgress(String written, String total) {
    return '$written / $total';
  }

  @override
  String transfersProgressPercent(int percent) {
    return '%$percent';
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
  String get transfersRemoveFromHistory => 'Geçmişten kaldır';

  @override
  String get transfersCancelDownload => 'İndirmeyi iptal et';

  @override
  String get transfersShowFile => 'Dosyayı göster';

  @override
  String get transfersPhaseSending => 'Gönderiliyor';

  @override
  String get transfersPhaseDownloading => 'İndiriliyor';

  @override
  String get transfersPhaseAdding => 'Ekleniyor';

  @override
  String get transfersPhaseAdded => 'Eklendi';

  @override
  String get transfersPhaseSent => 'Gönderildi';

  @override
  String get transfersPhaseCompleted => 'Tamamlandı';

  @override
  String get transfersPhaseCancelled => 'İptal edildi';

  @override
  String get transfersPhaseInterrupted => 'Yarıda kesildi';

  @override
  String get transfersPhaseFailed => 'Başarısız';

  @override
  String get transfersUpdatesUnavailable =>
      'Aktarım güncellemeleri kullanılamıyor. Yenilemek için yeniden açın.';

  @override
  String get transfersLoadFailed => 'Aktarımlar yüklenemedi. Tekrar deneyin.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Aktarım güncellemeleri yüklenemedi.';

  @override
  String get transfersRetryFailed => 'Bu dosya yeniden denenemedi.';

  @override
  String get transfersClearHistoryFailed => 'Aktarım geçmişi temizlenemedi.';

  @override
  String get transfersRemoveRecordFailed => 'Bu geçmiş kaydı kaldırılamadı.';

  @override
  String get transfersCancelFailed =>
      'Aktarım iptal edilemedi. Hâlâ devam ediyor olabilir.';

  @override
  String get transfersSavedFileUnavailable =>
      'Kaydedilen dosya kullanılamıyor. Taşınmış veya silinmiş olabilir.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Bildirimler · $count okunmamış',
      one: 'Bildirimler · $count okunmamış',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Bildirimler';

  @override
  String get notificationsClearRead => 'Okunan bildirimleri temizle';

  @override
  String get notificationsEmptyTitle => 'Bildirim yok';

  @override
  String get notificationsEmptyMessage =>
      'Cihaz bağlantı istekleri ve sonuçları burada görünür.';

  @override
  String get notificationsSystemAlertTitle => 'Sistem bildirimleri';

  @override
  String get notificationsReview => 'İncele';

  @override
  String get notificationsConnectionRequest => 'Bağlantı isteği';

  @override
  String get notificationsConfirmConnection => 'Bağlantıyı onayla';

  @override
  String get notificationsDeviceConnected => 'Cihaz bağlandı';

  @override
  String get notificationsConnectionRejected => 'Bağlantı reddedildi';

  @override
  String get notificationsConnectionCancelled => 'Bağlantı iptal edildi';

  @override
  String get notificationsConnectionRequestExpired =>
      'Bağlantı isteğinin süresi doldu';

  @override
  String get notificationsConnectionFailed => 'Bağlantı başarısız oldu';

  @override
  String get notificationsNearbyDevice => 'Yakındaki cihaz';

  @override
  String get notificationsPermissionDenied =>
      'Arka planda istek almak için işletim sistemi ayarlarınızda sistem bildirimlerine izin verin.';

  @override
  String get notificationsSystemUnavailable =>
      'Sistem bildirimleri kullanılamıyor. Bağlantı istekleri burada kalır.';

  @override
  String get platformCaptureStateUnavailable => 'Kullanılamıyor';

  @override
  String get platformCaptureStatePaused => 'Duraklatıldı';

  @override
  String get platformCaptureStateRunning => 'Yakalanıyor';

  @override
  String get platformCaptureStateStopped => 'Çalışmıyor';

  @override
  String get platformCaptureActionUnavailable => 'Yakalama kullanılamıyor';

  @override
  String get platformCaptureActionResume => 'Yakalamayı sürdür';

  @override
  String get platformCaptureActionPause => 'Yakalamayı duraklat';

  @override
  String get platformCaptureActionResumeStopped =>
      'Yakalamayı sürdür (çalışmıyor)';

  @override
  String get platformTrayOpen => 'Aç';

  @override
  String get platformTraySettings => 'Ayarlar';

  @override
  String get platformTrayQuit => 'Çık';

  @override
  String get platformWindowControlsUnavailable =>
      'Masaüstü pencere denetimleri kullanılamıyor. CopyPaste görünür kalacak.';

  @override
  String get platformTrayUnavailable =>
      'Sistem tepsisi kullanılamıyor. CopyPaste\'i açık tutun, ardından yeniden deneyin veya çıkın.';

  @override
  String get platformQuitFailed =>
      'CopyPaste\'ten çıkılamadı. Lütfen tekrar deneyin.';

  @override
  String get platformVisibilityChangeFailed =>
      'Uygulamanın görünürlüğü değiştirilemedi. Lütfen tekrar deneyin.';

  @override
  String get platformCaptureNotificationTitle => 'Pano kaydedildi';

  @override
  String get platformCaptureNotificationBody =>
      'CopyPaste\'e yeni bir öğe eklendi.';

  @override
  String get platformPairingNotificationTitle => 'Bağlantı isteği';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName bağlanmak istiyor. Kodu karşılaştırıp onaylamak için CopyPaste\'i açın.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Cihaz bağlantı istekleri';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Yakındaki cihazları bağlama istekleri';

  @override
  String get platformNotificationDefaultAction => 'Aç';

  @override
  String get platformFilesSelectionFailed => 'Dosyalar seçilemedi.';

  @override
  String get platformFilesOnlyRegular =>
      'Yalnızca normal dosyalar içe aktarılabilir.';

  @override
  String get platformModulesUnavailable =>
      'Modüller bu cihazda kullanılamıyor.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'SMS erişimi yapılandırılamadı.';

  @override
  String get platformSmsAccessVerifyFailed => 'SMS erişimi doğrulanamadı.';

  @override
  String get platformUpdatesUnavailableHere =>
      'Uygulama güncellemeleri burada kullanılamıyor.';

  @override
  String get platformUpdateVersionUnavailable =>
      'Yüklü sürüm bilgisi kullanılamıyor.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'Uygulama güncellemeleri bu kurulumda kullanılamıyor.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Bu Linux kurulumu otomatik olarak güncellenemez.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Bu Linux güncellemesi çalışan mimariyle eşleşmiyor.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Bu macOS kurulumu otomatik olarak güncellenemez.';

  @override
  String get platformUpdateStartFailed => 'CopyPaste güncellemeyi başlatamadı.';

  @override
  String get platformGlobalShortcutDescription => 'Quick Paste\'i aç';

  @override
  String get settingsSectionGeneral => 'Genel';

  @override
  String get settingsSectionGeneralSummary => 'Dil ve uygulama görünürlüğü.';

  @override
  String get settingsSectionClipboard => 'Pano';

  @override
  String get settingsSectionClipboardSummary => 'Yakalama ve geçmiş sınırları.';

  @override
  String get settingsSectionPrivacy => 'Gizlilik';

  @override
  String get settingsSectionPrivacySummary =>
      'Uygulama hariç tutmaları ve ekran koruması.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Kısayol ve otomatik yapıştırma davranışı.';

  @override
  String get settingsSectionSync => 'Senkronizasyon';

  @override
  String get settingsSectionSyncSummary =>
      'Senkronizasyon ve yakındaki cihazları bulma.';

  @override
  String get settingsSectionNotifications => 'Bildirimler';

  @override
  String get settingsSectionNotificationsSummary =>
      'Yakalama bildirimleri ve sesler.';

  @override
  String get settingsSectionData => 'Veriler';

  @override
  String get settingsSectionDataSummary =>
      'Dışa aktarma, yedekleme ve geri yükleme.';

  @override
  String get settingsSectionModules => 'Modüller';

  @override
  String get settingsSectionModulesSummary =>
      'İsteğe bağlı modülleri yükleyin ve yönetin.';

  @override
  String get settingsSectionRules => 'İçerik kuralları';

  @override
  String get settingsSectionRulesSummary =>
      'Öğeleri otomatik olarak etiketleyin, atlayın, koruyun veya süresini doldurun.';

  @override
  String get settingsSectionQuickAccess => 'Hızlı erişim';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Klavye, kutucuklar ve ana ekran widget\'ı.';

  @override
  String get settingsSectionSnippets => 'Parçacıklar';

  @override
  String get settingsSectionSnippetsSummary =>
      'Yeniden kullanılabilir metinler ve şablonlar.';

  @override
  String get settingsSectionTransforms => 'Metin dönüşümleri';

  @override
  String get settingsSectionTransformsSummary =>
      'Favoriler, işlem hatları ve yapıştırmada varsayılan dönüşümler.';

  @override
  String get settingsSectionAiTools => 'Yapay zekâ araçları';

  @override
  String get settingsSectionAiToolsSummary =>
      'Yapay zekâ asistanlarını bağlayın ve neleri görebileceklerini denetleyin.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Birden çok kopyayı toplayın ve sırayla yapıştırın.';

  @override
  String pasteQueuePasted(int position, int total) {
    return '$total öğeden $position. yapıştırıldı';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return '$total öğeden $position. öğe panoda';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe yapıştırıldı',
      one: '1 öğe yapıştırıldı',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe panoda',
      one: '1 öğe panoda',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Kuyruk tamamlandı';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Eklendi, kuyrukta $total',
      one: 'Eklendi, kuyrukta 1',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste =>
      'Kuyruk boş, hiçbir şey yapıştırılmadı';

  @override
  String get pasteQueueMixed =>
      'Tümünü yapıştır yalnızca metin ya da yalnızca dosya içeren kuyruklarda çalışır';

  @override
  String get pasteQueueSeveralFiles =>
      'Birden çok dosyayı aynı anda yapıştırmak henüz kullanılamıyor. Tek tek yapıştırın.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe artık kullanılamıyor',
      one: '1 öğe artık kullanılamıyor',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle =>
      'Sonrakini yapıştır yalnızca kopyalıyor';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Otomatik yapıştırmak için Erişilebilirlik izni verin. O zamana kadar öğe yapıştırmanız için panoda.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Otomatik yapıştırılamadı. Öğe panoda; her zamanki kısayolla yapıştırın.';

  @override
  String get pasteQueueTargetElevated =>
      'Yönetici olarak çalışan bir uygulamaya yapıştırılamadı. Öğe kuyrukta kalır ve panodadır.';

  @override
  String get pasteQueueNoTarget =>
      'Yapıştırılacak uygulama yok. Öğe kuyrukta kalır ve panodadır.';

  @override
  String get pasteQueueCollectingStarted => 'Kopyalar toplanıyor';

  @override
  String get pasteQueueCollectingStopped => 'Toplama durduruldu';

  @override
  String get pasteQueueFailed => 'Paste Queue işlemi tamamlanamadı';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count bekliyor',
      one: '1 bekliyor',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count bekliyor · Toplanıyor',
      one: '1 bekliyor · Toplanıyor',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Toplanıyor';

  @override
  String get pasteQueueFinishedSummary => 'Tamamlandı';

  @override
  String pasteQueueFull(int max) {
    return 'Kuyruk dolu ($max)';
  }

  @override
  String get pasteQueueFullMessage =>
      'Yeni kopyalar artık eklenmiyor. Hiçbir şey kaldırılmadı.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue boş';

  @override
  String get pasteQueueEmptyMessage =>
      'Kopyaları sırayla yapıştırmak için toplayın ya da geçmişten öğe ekleyin.';

  @override
  String get pasteQueueCollectingEmptyTitle =>
      'Eklemek için öğeleri kopyalayın';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Toplamayı durdurana kadar her kopya kuyruğa eklenir.';

  @override
  String get pasteQueueStartCollecting => 'Toplamaya başla';

  @override
  String get pasteQueueStopCollecting => 'Toplamayı durdur';

  @override
  String get pasteQueueLoading => 'Kuyruk yükleniyor…';

  @override
  String get pasteQueueLoadFailed => 'Kuyruk yüklenemedi';

  @override
  String get pasteQueuePasteNext => 'Sonrakini yapıştır';

  @override
  String get pasteQueueCopyNext => 'Sonrakini kopyala';

  @override
  String get pasteQueuePasteAll => 'Tümünü yapıştır';

  @override
  String get pasteQueueCopyAll => 'Tümünü kopyala';

  @override
  String get pasteQueueReverse => 'Sırayı ters çevir';

  @override
  String get pasteQueueStepBack => 'Bir adım geri';

  @override
  String get pasteQueueSkip => 'Atla';

  @override
  String get pasteQueueRemovePasted => 'Yapıştırılanları kaldır';

  @override
  String get pasteQueueCleared => 'Kuyruk temizlendi';

  @override
  String get pasteQueueUndo => 'Geri al';

  @override
  String get pasteQueueStartFresh => 'Baştan başla';

  @override
  String get pasteQueueAppend => 'Sona ekle';

  @override
  String get pasteQueueRestartTitle => 'Yeni bir toplama başlatılsın mı?';

  @override
  String get pasteQueueRestartMessage =>
      'Baştan başla yapıştırılan öğeleri kaldırır. Sona ekle onları korur.';

  @override
  String pasteQueueStale(String time) {
    return '$time tarihli kuyruk';
  }

  @override
  String get pasteQueueStatusPasted => 'Yapıştırıldı';

  @override
  String get pasteQueueStatusSkipped => 'Atlandı';

  @override
  String get pasteQueueStatusUnavailable => 'Artık kullanılamıyor';

  @override
  String get pasteQueueEdited => 'Düzenlendi';

  @override
  String get pasteQueueSecret => 'Gizli';

  @override
  String get pasteQueueKindImage => 'Görsel';

  @override
  String get pasteQueueKindFiles => 'Dosyalar';

  @override
  String pasteQueueRenderedAt(String time) {
    return '$time itibarıyla oluşturuldu';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Öğe $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return '$position. sıraya taşındı';
  }

  @override
  String get pasteQueueMoveUp => 'Yukarı taşı';

  @override
  String get pasteQueueMoveDown => 'Aşağı taşı';

  @override
  String get pasteQueueMoveToTop => 'En başa taşı';

  @override
  String get pasteQueueMoveToBottom => 'En sona taşı';

  @override
  String get pasteQueueReorder => 'Sıralamak için sürükleyin';

  @override
  String get pasteQueueMoreActions => 'Diğer işlemler';

  @override
  String get pasteQueuePasteNow => 'Şimdi yapıştır';

  @override
  String get pasteQueueEdit => 'Metni düzenle';

  @override
  String get pasteQueueUseOriginal => 'Özgün metni kullan';

  @override
  String get pasteQueueRemove => 'Kuyruktan kaldır';

  @override
  String get pasteQueueEditTitle => 'Kuyruktaki metni düzenle';

  @override
  String get pasteQueueEditMessage =>
      'Değişiklikler yalnızca bu kuyruğa uygulanır. Geçmiş özgün hâli korur.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Kısayolları ayarla';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'CopyPaste\'i açmadan toplamak ve yapıştırmak için kısayollar seçin.';

  @override
  String get pasteQueueShortcutToggleCollecting =>
      'Toplamayı başlat veya durdur';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Sonrakini düz metin olarak yapıştır';

  @override
  String get pasteQueueShortcutDescription =>
      'Her uygulamada çalışan genel bir kısayol.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Başka bir CopyPaste kısayolu bu bileşimi kullanıyor. Önceki korunur.';

  @override
  String get pasteQueueShortcutInUse =>
      'Başka bir uygulama bu bileşimi kullanıyor. Önceki korunur; başka bir bileşim seçin.';

  @override
  String get pasteQueueShortcutRejected =>
      'Sistem bu bileşimi reddetti. Önceki korunur.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt birçok klavye düzeninde karakter yazar (AltGr). Başka bir bileşim seçin.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Bu oturumda genel kısayollar kullanılamıyor. Kuyruğu pencereden veya Quick Paste\'ten kullanın.';

  @override
  String get pasteQueueShortcutFailed => 'Bu kısayol ayarlanamadı.';

  @override
  String get pasteQueueEnableTitle => 'Paste Queue\'yu etkinleştir';

  @override
  String get pasteQueueEnableDescription =>
      'Kapalıyken kuyruk kısayolları serbest bırakılır ve hiçbir şey toplanmaz.';

  @override
  String get pasteQueuePasteNextStyleTitle => 'Sonrakini yapıştır şunu yapar';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Öndeki uygulamaya yapıştırmak tuş vuruşu gönderme izni gerektirir. Yalnızca pano yapıştırmayı size bırakır.';

  @override
  String get pasteQueuePasteIntoApp => 'Öndeki uygulamaya yapıştır';

  @override
  String get pasteQueueClipboardOnly => 'Yalnızca panoya koy';

  @override
  String get pasteQueueInsertTitle => 'Yeni öğeler şuraya eklenir';

  @override
  String get pasteQueueInsertDescription =>
      'Toplanan ve eklenen öğelerin kuyruğa nereden katılacağı.';

  @override
  String get pasteQueueInsertEnd => 'Sona (ilk giren ilk çıkar)';

  @override
  String get pasteQueueInsertStart => 'Başa (son giren ilk çıkar)';

  @override
  String get pasteQueueEndBehaviorTitle => 'Son öğeden sonra';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'Tüm öğeler yapıştırıldıktan sonra Sonrakini yapıştır\'ın yaptığı.';

  @override
  String get pasteQueueEndStop => 'Dur';

  @override
  String get pasteQueueEndLoop => 'Baştan başla';

  @override
  String get pasteQueueEndKeepLast => 'Son öğeyi yapıştırmaya devam et';

  @override
  String get pasteQueueSeparatorTitle => 'Tümünü yapıştır ayırıcısı';

  @override
  String get pasteQueueSeparatorDescription =>
      'Tümü bir kerede yapıştırılırken öğelerin arasına konur.';

  @override
  String get pasteQueueSeparatorNewLine => 'Yeni satır';

  @override
  String get pasteQueueSeparatorBlankLine => 'Boş satır';

  @override
  String get pasteQueueSeparatorTab => 'Sekme';

  @override
  String get pasteQueueSeparatorSpace => 'Boşluk';

  @override
  String get pasteQueueSeparatorComma => 'Virgül';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Virgül ve boşluk';

  @override
  String get pasteQueueSeparatorNone => 'Yok';

  @override
  String get pasteQueueSeparatorCustom => 'Özel';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'En fazla 32 karakter; \\n ve \\t kullanılabilir';

  @override
  String get pasteQueuePlainTextTitle => 'Her zaman düz metin olarak yapıştır';

  @override
  String get pasteQueuePlainTextDescription =>
      'Kuyruktaki öğelerin biçimlendirmesini kaldırır.';

  @override
  String get pasteQueueIdleStopTitle => 'Toplamayı şu süreden sonra durdur';

  @override
  String get pasteQueueIdleStopDescription =>
      'Bu süre boyunca hiçbir şey kopyalanmazsa toplama kapanır. Öğeler kalır.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes dk';
  }

  @override
  String get pasteQueueIdleNever => 'Asla';

  @override
  String get pasteQueueMaxEntriesTitle => 'Kuyruk boyutu sınırı';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Dolduğunda öğe eklenmez. Hiçbir şey atılmaz.';

  @override
  String get pasteQueueRestoreTitle => 'Bitince önceki panoyu geri yükle';

  @override
  String get pasteQueueRestoreDescription =>
      'Kuyruk yapıştırmaya başlamadan önce kopyaladığınızı geri koyar.';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue kuyruk toplama sıra kısayol ayırıcı';

  @override
  String get settingsSectionAbout => 'Hakkında';

  @override
  String get settingsSectionAboutSummary => 'Sürüm ve uygulama güncellemeleri.';

  @override
  String get settingsGeneralDescription =>
      'Dil, otomatik başlatma ve uygulama görünürlüğü.';

  @override
  String get settingsModulesDescription =>
      'CopyPaste için isteğe bağlı özellikler.';

  @override
  String get settingsClipboardDescription =>
      'Pano değişikliklerini yakalayın ve geçmiş sınırlarını yönetin.';

  @override
  String get settingsDataDescription =>
      'Pano geçmişini dışa aktarın, yedekleyin ve geri yükleyin.';

  @override
  String get settingsSyncDescription =>
      'Eşlenmiş cihazlarla senkronizasyonu yönetin.';

  @override
  String get settingsPrivacyDescription =>
      'Hangi uygulamaların yakalanacağını denetleyin ve pano içeriğini koruyun.';

  @override
  String get settingsNotificationsDescription =>
      'Yakalanan öğeler için bildirimleri ve sesleri seçin.';

  @override
  String get settingsAboutDescription =>
      'CopyPaste sürümü ve uygulama güncellemeleri.';

  @override
  String get settingsQuickPasteDescription =>
      'Pencere değiştirmeden pano geçmişini her yerden açın.';

  @override
  String get settingsLoadingMessage => 'Ayarlar yükleniyor.';

  @override
  String get settingsUnavailableTitle => 'Ayarlar kullanılamıyor';

  @override
  String get settingsNoResultsTitle => 'Ayar bulunamadı';

  @override
  String get settingsNoResultsMessage => 'Farklı bir arama deneyin.';

  @override
  String get settingsAttentionTitle => 'Ayarlar dikkat gerektiriyor';

  @override
  String get settingsSearchPlaceholder => 'Ayarlarda ara';

  @override
  String get settingsBackToSettings => 'Ayarlar\'a dön';

  @override
  String get settingsStartOnBootTitle => 'Cihaz açılışında başlat';

  @override
  String get settingsStartAtLoginTitle => 'Girişte başlat';

  @override
  String get settingsStartupSearchDescription =>
      'CopyPaste\'i otomatik olarak başlatın.';

  @override
  String get settingsStartupKeywords =>
      'başlangıç giriş açılış başlatma otomatik otobaşlatma startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Görev Yöneticisi\'nde devre dışı bırakıldı. CopyPaste\'i Ayarlar → Uygulamalar → Başlangıç bölümünde etkinleştirin.';

  @override
  String get settingsStartupMacApproval =>
      'Sistem Ayarları → Genel → Giriş Öğeleri bölümünde CopyPaste\'e izin verin.';

  @override
  String get settingsStartupUnavailable =>
      'Otomatik başlatma bu kurulumda kullanılamıyor.';

  @override
  String get settingsStartupAndroidDescription =>
      'Cihaz yeniden başlatıldıktan sonra etkin arka plan hizmetleri sürdürülür.';

  @override
  String get settingsStartupDesktopDescription =>
      'Oturum açtığınızda CopyPaste otomatik olarak açılır.';

  @override
  String get settingsShowInDock => 'Dock\'ta göster';

  @override
  String get settingsShowInTaskbar => 'Görev çubuğunda göster';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Pencere değiştirmek için uygulama simgesini gösterir.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'dock görev çubuğu uygulama simgesi görünürlük dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Menü çubuğunda göster';

  @override
  String get settingsShowInSystemTray => 'Sistem tepsisinde göster';

  @override
  String get settingsTrayVisibilityDescription =>
      'CopyPaste\'e ve yakalama denetimlerine hızlı erişim sağlar.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'tepsi menü çubuğu durum simge görünürlük tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle => 'Görünürlük değiştirilemedi';

  @override
  String get settingsModulesSearchDescription =>
      'İsteğe bağlı modülleri yükleyin, güncelleyin ve kaldırın.';

  @override
  String get settingsModulesKeywords =>
      'özellikler komutlar uzantılar features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Android arka plan yakalaması';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Shizuku veya ADB kurulumuyla Tam ya da Sınırlı mod.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'arka plan izinler kurulum background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Tam veya Sınırlı modu seçin ve tek seferlik Shizuku ya da ADB kurulumunu yönetin.';

  @override
  String get settingsOpenSetup => 'Kurulumu aç';

  @override
  String get settingsSaveScreenshotsTitle => 'Ekran görüntülerini kaydet';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Yeni Android ekran görüntülerini otomatik olarak Geçmiş\'e kaydedin.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'görüntüler fotoğraflar yakalama izinler images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Yeni ekran görüntülerini otomatik olarak Geçmiş\'e ekler.';

  @override
  String get settingsScreenshotAccessTitle => 'Ekran görüntüsü erişimi';

  @override
  String get settingsScreenshotAccessDescription =>
      'Yeni ekran görüntülerini arka planda kaydetmek için tüm fotoğraflara ve bildirimlere erişime izin verin.';

  @override
  String get settingsAllowAccess => 'Erişime izin ver';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'Ekran görüntüsü yakalama çalışmıyor';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'Yeni ekran görüntüleri kaydedilmiyor. Hizmeti yeniden başlatmak için yakalamayı sürdürün.';

  @override
  String get settingsResumeCapture => 'Yakalamayı sürdür';

  @override
  String get settingsClipboardCaptureTitle => 'Pano yakalama';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Pano yakalamayı duraklatın veya sürdürün.';

  @override
  String get settingsCaptureStateUnavailable => 'Durum kullanılamıyor';

  @override
  String get settingsCaptureStoppedTitle => 'Yakalama çalışmıyor';

  @override
  String get settingsCaptureStoppedMessage =>
      'CopyPaste şu anda bu cihazda pano değişikliklerini almıyor.';

  @override
  String get settingsRetentionTitle => 'Saklama süresi';

  @override
  String get settingsRetentionDescription =>
      'Sabitlenmemiş eski pano öğelerini otomatik olarak kaldırır.';

  @override
  String get settingsRetentionForever => 'Süresiz sakla';

  @override
  String get settingsStorageQuotaTitle => 'Depolama kotası';

  @override
  String get settingsStorageQuotaDescription =>
      'Sabitlenmemiş geçmişin kullanabileceği en fazla yerel depolama alanı.';

  @override
  String get settingsStorageQuotaKeywords => 'disk alan sınır disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Maksimum dosya boyutu';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Dosya ekleme ve indirme sınırı.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'içe aktarma indirme aktarım boyut sınır import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Dosya ekleme ve indirme sınırı. Dosya içerikleri yalnızca İndir\'i seçtiğinizde aktarılır.';

  @override
  String get settingsHistoryFilesTitle => 'Geçmiş dosyaları';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Geçmişi dışa aktarın, yedek oluşturun veya bir yedeği geri yükleyin.';

  @override
  String get settingsHistoryFilesKeywords =>
      'metin şifreli yedek veri text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Metin geçmişini dışa aktar';

  @override
  String get settingsExportTextDescription =>
      'Metin öğelerini taşınabilir bir dosyaya kaydeder.';

  @override
  String get settingsExportAction => 'Dışa aktar';

  @override
  String get settingsBackupTitle => 'Şifreli yedek';

  @override
  String get settingsBackupDescription =>
      'Bu cihazın tüm yerel geçmişini kaydeder.';

  @override
  String get settingsCreateBackupAction => 'Yedek oluştur';

  @override
  String get settingsRestoreTitle => 'Yedeği geri yükle';

  @override
  String get settingsRestoreDescription =>
      'Yerel geçmişi şifreli bir yedekle değiştirir.';

  @override
  String get settingsRestoreAction => 'Geri yükle';

  @override
  String get settingsRestoreDialogTitle => 'Yerel geçmiş değiştirilsin mi?';

  @override
  String get settingsRestoreDialogMessage =>
      'Seçilen şifreli yedek, bu cihazdaki pano geçmişinin yerini alacak. Geçerli ayarlar ve cihaz kimliği korunur.';

  @override
  String get settingsRestoreFailed => 'Yedek geri yüklenemedi.';

  @override
  String get settingsSyncSwitchTitle => 'Senkronizasyon';

  @override
  String get settingsSyncSwitchDescription =>
      'Eşlenmiş cihaz senkronizasyonunun ana anahtarı.';

  @override
  String get settingsSyncSearchDescription => 'Eşlenmiş cihaz senkronizasyonu.';

  @override
  String get settingsInstantClipboardTitle => 'Anında pano';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Diğer cihazlardan gelen yeni öğeleri otomatik olarak kopyalayın.';

  @override
  String get settingsInstantClipboardKeywords =>
      'otomatik kopyala yapıştır senkronizasyon alınan automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Diğer cihazlardan gelen daha yeni öğeleri otomatik olarak bu cihazın panosuna kopyalar.';

  @override
  String get settingsLanVisibilityTitle => 'LAN görünürlüğü';

  @override
  String get settingsLanVisibilityDescription =>
      'Yakındaki cihazların bu cihazı bulmasına izin verir.';

  @override
  String get settingsLanVisibilityKeywords =>
      'yerel ağ keşif bulma local network discovery';

  @override
  String get settingsExcludedAppsTitle => 'Hariç tutulan uygulamalar';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Hariç tutulan uygulamalardan otomatik yakalamayı atlayın.';

  @override
  String get settingsExcludedAppsKeywords =>
      'gizlilik uygulama tanımlayıcılar privacy app identifiers';

  @override
  String get settingsExcludedAppsMacDescription =>
      'Bu uygulamalar etkinken otomatik yakalama atlanır. Arka plandaki kopyalamalar hariç tutmaları aşabilir.';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Tanımlanan pano sahiplerinden otomatik yakalama atlanır. Hariç tutmalar ayarlıyken kaynak tanımlanamazsa yakalama duraklatılır.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Android kaynak uygulamaları tanımlayamadığından, hariç tutmalar ayarlıyken otomatik yakalamayı atlar.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'Uygulama hariç tutma macOS, Windows, Android ve Linux\'ta desteklenir.';

  @override
  String get settingsExcludedAppsAdd => 'Ekle';

  @override
  String get settingsExcludedAppsEmpty => 'Hariç tutulan uygulama yok.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return '$app uygulamasını kaldır';
  }

  @override
  String get settingsExcludeDialogTitle => 'Uygulamayı hariç tut';

  @override
  String get settingsExcludeDialogAction => 'Hariç tut';

  @override
  String get settingsExcludeDialogPlaceholder => 'Uygulama tanımlayıcısı';

  @override
  String get settingsExcludeFailed => 'Uygulama hariç tutulamadı.';

  @override
  String get settingsSkipSecretTitle => 'Gizli pano içeriğini atla';

  @override
  String get settingsSkipSecretDescription =>
      'Kaynak uygulamanın gizli olarak işaretlediği içerikleri Geçmiş\'in dışında tutar.';

  @override
  String get settingsSkipSecretKeywords =>
      'gizli parola hassas saklı sürpriz secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Geçici pano içeriğini atla';

  @override
  String get settingsSkipTransientDescription =>
      'Kaynak uygulamanın geçici olarak işaretlediği kopyaları Geçmiş\'in dışında tutar.';

  @override
  String get settingsSkipTransientKeywords =>
      'geçici kısa süreli gizlilik transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Ekran görüntülerini engelle';

  @override
  String get settingsBlockScreenshotsDescription =>
      'CopyPaste\'in ekran görüntüsünün ve ekran kaydının alınmasını engeller.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'Ekran görüntüsü engelleme bu platformda kullanılamıyor.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'gizlilik ekran yakalama koruma eşleme qr güvenlik kod privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Kopyalamada bildirim';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Arka planda yakalama sonrasında bildirim gösterir.';

  @override
  String get settingsNotificationPreviewTitle => 'Pano içeriğini göster';

  @override
  String get settingsNotificationPreviewDescription =>
      'Kopyalama bildirimlerine öğe önizlemesi ekler.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'bildirim önizleme metin görüntü gizlilik notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Kopyalamada ses';

  @override
  String get settingsSoundOnCopyDescription =>
      'Başarılı bir yakalamadan sonra platform geri bildirimini oynatır.';

  @override
  String get settingsVersionTitle => 'Sürüm';

  @override
  String get settingsVersionDescription => 'Yüklü CopyPaste sürümü.';

  @override
  String get settingsVersionUnavailable => 'Kullanılamıyor';

  @override
  String get settingsUpdatesTitle => 'Uygulama güncellemeleri';

  @override
  String get settingsUpdatesSearchDescription =>
      'CopyPaste güncellemelerini denetleyin ve yükleyin.';

  @override
  String get settingsUpdatesKeywords =>
      'sürüm github yayın version github release';

  @override
  String get settingsUpdateIdle =>
      'Daha yeni bir CopyPaste sürümü olup olmadığını denetleyin.';

  @override
  String get settingsUpdateChecking => 'Güncellemeler denetleniyor.';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste güncel.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return '$version sürümü güncel.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return '$available sürümü kullanılabilir. Bu cihazda $current sürümü var.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return '$version sürümü indiriliyor · %$percent.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Yüklü modül güncellemeleri hazırlanıyor.';

  @override
  String settingsUpdateInstalling(String version) {
    return '$version sürümü yükleniyor.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'Uygulama güncellemeleri dikkat gerektiriyor.';

  @override
  String get settingsUpdateNowAction => 'Şimdi güncelle';

  @override
  String get settingsUpdateOpenReleaseAction => 'Sürüm sayfasını aç';

  @override
  String get settingsUpdateCheckingAction => 'Denetleniyor';

  @override
  String get settingsUpdateDownloadingAction => 'İndiriliyor';

  @override
  String get settingsUpdateInstallingAction => 'Yükleniyor';

  @override
  String get settingsUpdateRestartAction => 'CopyPaste\'i yeniden başlat';

  @override
  String get settingsUpdateCheckAgainAction => 'Yeniden denetle';

  @override
  String get settingsUpdateNoneTitle => 'Güncelleme yok';

  @override
  String get settingsUpdateLatest => 'CopyPaste en son sürümde.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version en son sürüm.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Güncelleme mevcut';

  @override
  String get settingsUpdateReady =>
      'Daha yeni bir CopyPaste sürümü yüklenmeye hazır.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version yüklenmeye hazır.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Güncelleme kullanılamıyor';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Bu kurulum otomatik olarak güncellenemez.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Güncelleme denetimi başarısız oldu';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'CopyPaste güncellemeleri denetleyemedi.';

  @override
  String get settingsQuickPasteSetupTitle => 'Quick Paste\'i kurun';

  @override
  String get settingsQuickPasteSetupMessage =>
      'İmzalı GNOME veya KDE yardımcısını etkinleştirin ve önceki uygulamaya dönmek için klavye denetimine izin verin.';

  @override
  String get settingsQuickPasteDesktopIntegration => 'Masaüstü entegrasyonu';

  @override
  String get settingsQuickPasteAllowKeyboard => 'Klavye denetimine izin ver';

  @override
  String get settingsQuickPasteCheckAgain => 'Yeniden denetle';

  @override
  String get settingsQuickPasteLoading => 'Quick Paste ayarları yükleniyor.';

  @override
  String get settingsQuickPasteUnavailableTitle => 'Quick Paste kullanılamıyor';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'Bu cihaz Quick Paste penceresini sağlamıyor.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Quick Paste\'i aç';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Genel Quick Paste kısayolunu yapılandırın.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'klavye kısayol tuşu keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'Kısayol, CopyPaste arka planda çalışırken kullanılabilir.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Kayıtlı kısayol: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Otomatik yapıştır';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Seçilen öğeyi önceki uygulamaya yapıştırın.';

  @override
  String get settingsAutoPasteKeywords =>
      'erişilebilirlik otomatik accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Bir öğe seçildiğinde önceki uygulamaya dönülür ve öğe yapıştırılır.';

  @override
  String get settingsAccessibilityRequiredTitle =>
      'Erişilebilirlik izni gerekli';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'İzin verilene kadar, seçilen öğe yapıştırılmadan yalnızca kopyalanır.';

  @override
  String get settingsQuickPasteAttentionTitle =>
      'Quick Paste dikkat gerektiriyor';

  @override
  String get settingsShortcutRecording => 'Kısayola basın…';

  @override
  String get settingsShortcutNotSet => 'Ayarlanmadı';

  @override
  String get settingsShortcutReset => 'Kısayolu sıfırla';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count gün',
      one: '$count gün',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Cihazlar yükleniyor…';

  @override
  String get devicesUnavailableTitle => 'Cihazlar kullanılamıyor';

  @override
  String get devicesNearbyTitle => 'Yakındaki cihazlar';

  @override
  String get devicesScan => 'Tara';

  @override
  String get devicesScanning => 'Taranıyor…';

  @override
  String get devicesNoneNearby => 'Yakında cihaz yok';

  @override
  String get devicesThisDevice => 'Bu cihaz';

  @override
  String get devicesTrusted => 'Güvenilir';

  @override
  String get devicesConnect => 'Bağlan';

  @override
  String devicesCardPing(String ping) {
    return 'Ping: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Ping:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'Bu cihazı yeniden adlandır';

  @override
  String get devicesNamePlaceholder => 'Cihaz adı';

  @override
  String get devicesRenameFailed => 'Cihaz adı kaydedilemedi.';

  @override
  String devicesRevokeDialogTitle(String device) {
    return '$device cihazının erişimi iptal edilsin mi?';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return '$device cihazının eşlemesi kaldırılsın mı?';
  }

  @override
  String get devicesRevokeDialogMessage =>
      'Bu işlem cihazı grubunuzdan kaldırır ve bu eşlemeyi kalıcı olarak engeller. Diğer cihazlar yeniden bağlandığında kaldırma işlemini uygular.';

  @override
  String get devicesUnpairDialogMessage =>
      'Bu işlem cihazı grubunuzdan kaldırır. Diğer cihazlar yeniden bağlandığında kaldırma işlemini uygular. Cihazı yeniden eklemek için yeni bir onay gerekir.';

  @override
  String get devicesRevoke => 'Erişimi iptal et';

  @override
  String get devicesUnpair => 'Eşlemeyi kaldır';

  @override
  String get devicesActionFailedMessage => 'Cihaz işlemi tamamlanamadı.';

  @override
  String get devicesCloseDetails => 'Cihaz ayrıntılarını kapat';

  @override
  String get devicesDetailType => 'Cihaz türü';

  @override
  String get devicesDetailOperatingSystem => 'İşletim sistemi';

  @override
  String get devicesDetailModel => 'Model';

  @override
  String get devicesDetailAppVersion => 'CopyPaste sürümü';

  @override
  String get devicesDetailProtocolVersion => 'Protokol sürümü';

  @override
  String get devicesDetailStatus => 'Durum';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => 'Ölçülme zamanı';

  @override
  String get devicesDetailLastSeen => 'Son görülme';

  @override
  String get devicesDetailLanEndpoint => 'LAN uç noktası';

  @override
  String get devicesDetailDeviceId => 'Cihaz kimliği';

  @override
  String get devicesDetailPairingId => 'Eşleme kimliği';

  @override
  String get devicesDetailProfileProvenance => 'Profil kaynağı';

  @override
  String get devicesDetailProfileTrust => 'Profil güven düzeyi';

  @override
  String get devicesDetailProfileUpdated => 'Profil güncellenme zamanı';

  @override
  String get devicesValueUnavailable => 'Kullanılamıyor';

  @override
  String get devicesRenameDevice => 'Cihazı yeniden adlandır';

  @override
  String get devicesSyncNow => 'Şimdi senkronize et';

  @override
  String get devicesRevokePairing => 'Eşleme erişimini iptal et';

  @override
  String get devicesLocal => 'Yerel';

  @override
  String get devicesTrustAuthenticated => 'Kimliği doğrulandı';

  @override
  String get devicesTrustUnverified => 'Doğrulanmadı';

  @override
  String get devicesProvenanceSelfReported => 'Kendi bildirdiği';

  @override
  String get devicesProvenanceObserved => 'Gözlemlendi';

  @override
  String get devicesProvenanceMeasured => 'Ölçüldü';

  @override
  String get devicesConnectionOnline => 'Çevrimiçi';

  @override
  String get devicesConnectionOffline => 'Çevrimdışı';

  @override
  String get devicesConnectionVisible => 'Görünür';

  @override
  String get devicesConnectionNotSeen => 'Görülmedi';

  @override
  String get devicesConnectionUnknown => 'Bilinmiyor';

  @override
  String get devicesPeerAvailable => 'Kullanılabilir';

  @override
  String get devicesPeerVisibleNow => 'Şu anda görünür';

  @override
  String get devicesPeerStatusUnknown => 'Durum bilinmiyor';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => 'Masaüstü';

  @override
  String get devicesClassLaptop => 'Dizüstü';

  @override
  String get devicesClassPhone => 'Telefon';

  @override
  String get devicesClassTablet => 'Tablet';

  @override
  String get devicesClassUnknown => 'Cihaz';

  @override
  String get devicesUnknownOs => 'Bilinmeyen işletim sistemi';

  @override
  String get devicesAddDevice => 'Cihaz ekle';

  @override
  String get devicesScanQr => 'QR tara';

  @override
  String get devicesEnterCode => 'Kod gir';

  @override
  String get devicesScannerUnavailableTitle => 'Tarayıcı kullanılamıyor';

  @override
  String get devicesActionFailedTitle => 'İşlem başarısız oldu';

  @override
  String get devicesRefreshFailedTitle => 'Cihazlar yenilenemedi';

  @override
  String get devicesNearbyPairingUnavailable =>
      'Yakındaki cihazla eşleme kullanılamıyor.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'Yakındaki cihazla eşleme kullanılamıyor. Eşleme kodu göstermek için Cihaz ekle\'yi kullanın.';

  @override
  String get pairingTitle => 'Eşleme';

  @override
  String get pairingPairDevice => 'Cihaz eşle';

  @override
  String get pairingScanQrCode => 'QR kodu tara';

  @override
  String get pairingEnterPairingCode => 'Eşleme kodunu gir';

  @override
  String get pairingConnectNearbyDevice => 'Yakındaki cihazı bağla';

  @override
  String get pairingSendingRequest => 'Bağlantı isteği gönderiliyor…';

  @override
  String get pairingEmptyTitle => 'Bir eşleme yöntemi seçin';

  @override
  String get pairingEmptyMessage => 'Eşleme denetimleri burada görünür.';

  @override
  String get pairingCreatingInvitation => 'Eşleme daveti oluşturuluyor.';

  @override
  String get pairingCode => 'Eşleme kodu';

  @override
  String get pairingCodeHint =>
      'Diğer cihazda gösterilen 8 karakterlik kodu girin.';

  @override
  String get pairingDeviceAddress => 'Cihaz adresi';

  @override
  String get pairingDeviceAddressHint =>
      'Diğer cihazın bildirdiği host:port adresini kullanın.';

  @override
  String get pairingCheckDetailsTitle => 'Eşleme ayrıntılarını kontrol edin';

  @override
  String get pairingActionFailed => 'Eşleme işlemi başarısız oldu';

  @override
  String get pairingConnectingDevice => 'Cihaz bağlanıyor…';

  @override
  String get pairingQrScannerTitle => 'QR tarayıcı';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'Kamera önizlemesi burada görünür.';

  @override
  String get pairingScannerNeedsAttention => 'Tarayıcı dikkat gerektiriyor';

  @override
  String get pairingCodeIncomplete =>
      '8 karakterlik eşleme kodunun tamamını girin.';

  @override
  String get pairingAddressMissing => 'Cihazın host:port adresini girin.';

  @override
  String get pairingWaitingForOtherDevice => 'Diğer cihaz bekleniyor';

  @override
  String get pairingInviteInstructions =>
      'Diğer cihazda Cihaz ekle → Kod gir\'i seçin veya QR kodunu tarayın.';

  @override
  String get pairingDetailsTitle => 'Eşleme ayrıntıları';

  @override
  String get pairingAddress => 'Adres';

  @override
  String get pairingVerificationCode => 'Doğrulama kodu';

  @override
  String get pairingVerificationCodeHint =>
      'Yalnızca bu kod her iki cihazda da aynıysa onaylayın.';

  @override
  String get pairingReject => 'Reddet';

  @override
  String get pairingConfirm => 'Onayla';

  @override
  String get pairingAccept => 'Kabul et';

  @override
  String get pairingJoinDevice => 'Cihaza katıl';

  @override
  String get pairingStartScanner => 'Tarayıcıyı başlat';

  @override
  String get pairingActionNotStarted => 'Eşleme işlemi başlatılamadı.';

  @override
  String get pairingStateHandshaking => 'Cihaz doğrulanıyor';

  @override
  String get pairingStateAwaitingConfirmation => 'Her iki cihazda da onaylayın';

  @override
  String get pairingStateConfirmed => 'Cihaz eşlendi';

  @override
  String get pairingStateRejected => 'Eşleme reddedildi';

  @override
  String get pairingStateCancelled => 'Eşleme iptal edildi';

  @override
  String get pairingStateTimedOut => 'Eşleme zaman aşımına uğradı';

  @override
  String get pairingStateFailed => 'Eşleme başarısız oldu';

  @override
  String get pairingHandshakingDescription =>
      'Cihazlar güvenli bir kanal kuruyor.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Korumalı doğrulama kodunu karşılaştırın, ardından her iki cihazda da onaylayın.';

  @override
  String get pairingConfirmedDescription =>
      'Cihaz bağlandı. Grup üyeliğiniz otomatik olarak senkronize edilecek.';

  @override
  String get pairingNoDeviceAdded => 'Hiçbir cihaz eklenmedi.';

  @override
  String get pairingTimedOutDescription =>
      'Devam etmek için yeni bir eşleme denemesi başlatın.';

  @override
  String get pairingIdleDescription =>
      'Bir cihaz bağlamak için eşleme denemesi başlatın.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'Korumalı eşleme kullanılamıyor';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Bu eşleme oturumu ayrı, korumalı bir pencerede açılmalıdır.';

  @override
  String get pairingCancel => 'Eşlemeyi iptal et';

  @override
  String get pairingOrEnterCode => 'Veya eşleme kodu girin';

  @override
  String get pairingJoinSecurely => 'Güvenli şekilde katıl';

  @override
  String get pairingProtectedWaitingDescription =>
      'Bu korumalı pencerede bir QR kodu gösterin, bir QR kodu tarayın veya eşleme kodu girin.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return '$seconds sn içinde sona erer';
  }

  @override
  String get pairingInvalidQr => 'Bir CopyPaste eşleme QR kodu tarayın.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Google tarayıcı kullanılamıyor. Bunun yerine eşleme kodunu girin.';

  @override
  String get pairingScannerCouldNotOpen =>
      'Tarayıcı açılamadı. Bunun yerine eşleme kodunu girin.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'Güvenli eşleme görünümü kullanılamıyor.';

  @override
  String get pairingVerificationCodeRequired =>
      'Önce doğrulama kodunu gösterip karşılaştırın.';

  @override
  String get syncStatusUnavailable => 'Senkronizasyon durumu kullanılamıyor';

  @override
  String get syncStatusOff => 'Senkronizasyon kapalı';

  @override
  String get syncStatusWaitingForSync => 'Senkronizasyon bekleniyor';

  @override
  String get syncStatusSyncing => 'Senkronize ediliyor';

  @override
  String get syncStatusUpToDate => 'Güncel';

  @override
  String get syncStatusInterrupted => 'Senkronizasyon yarıda kesildi';

  @override
  String get syncStatusPartiallySynchronized => 'Kısmen senkronize edildi';

  @override
  String get syncStatusRetrying => 'Yeniden deneniyor';

  @override
  String get syncStatusWaitingToSync => 'Senkronize edilmeyi bekliyor';

  @override
  String get syncStatusWaitingForDevice => 'Cihaz bekleniyor';

  @override
  String get syncStatusNoPairedDevices => 'Eşlenmiş cihaz yok';

  @override
  String get syncStatusDevicesOffline => 'Cihazlar çevrimdışı';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · etkinlik günlüğü kullanılamıyor';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Senkronizasyon kapalı · $count sorun',
      one: 'Senkronizasyon kapalı · $count sorun',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count çevrimdışı',
      one: '$status · $count çevrimdışı',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count cihaz dikkat gerektiriyor',
      one: '$count cihaz dikkat gerektiriyor',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Cihazlar çevrimdışı · $count sorun',
      one: 'Cihazlar çevrimdışı · $count sorun',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Bağlantı denetleniyor';

  @override
  String get syncConnectionUnknown => 'Bağlantı bilinmiyor';

  @override
  String get syncLogDeviceOffline => 'Cihaz çevrimdışı oldu.';

  @override
  String get syncLogConnectionRestored => 'Bağlantı yeniden kuruldu.';

  @override
  String get syncLogProblemDetected => 'Senkronizasyon sorunu algılandı.';

  @override
  String get syncLogProblemResolved => 'Senkronizasyon sorunu çözüldü.';

  @override
  String get syncLogFailed => 'Senkronizasyon başarısız oldu.';

  @override
  String get syncLogRecovered => 'Senkronizasyon yeniden sağlandı.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent öğe',
      one: '$sent öğe',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received öğe',
      one: '$received öğe',
    );
    return 'Gönderilen: $_temp0 · Alınan: $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Senkronizasyon: $status';
  }

  @override
  String get syncDetailsTitle => 'Senkronizasyon';

  @override
  String get syncDetailsClose => 'Senkronizasyon ayrıntılarını kapat';

  @override
  String get syncNoPairedDevicesMessage =>
      'Pano geçmişini senkronize etmek için bir cihaz eşleyin.';

  @override
  String get syncActivityLogUnavailableTitle =>
      'Etkinlik günlüğü kullanılamıyor';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Senkronizasyon etkinliği okunamadı veya kaydedilemedi.';

  @override
  String get syncDetailsDevicesHeading => 'Cihazlar';

  @override
  String get syncDetailsActivityHeading => 'Etkinlik';

  @override
  String get syncDetailsNoRecentProblems =>
      'Yakın zamanda senkronizasyon sorunu yok';

  @override
  String get syncDetailsUnknownPeer => 'Eşlenmiş cihaz';

  @override
  String get syncPeerConnection => 'Bağlantı';

  @override
  String get syncPeerLastCheckStarted => 'Son denetimin başlangıcı';

  @override
  String get syncPeerItemsOverSizeLimit => 'Boyut sınırını aşan öğeler';

  @override
  String syncPeerLastSeen(String time) {
    return 'Son görülme: $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Son senkronizasyon: $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Son başarılı senkronizasyon';

  @override
  String get syncPeerConnectionDetails => 'Bağlantı ayrıntıları';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return '$device için bağlantı ayrıntıları';
  }

  @override
  String get historyKindText => 'Metin';

  @override
  String get historyKindLink => 'Bağlantı';

  @override
  String get historyKindEmail => 'E-posta';

  @override
  String get historyKindColor => 'Renk';

  @override
  String get historyKindPhone => 'Telefon';

  @override
  String get historyKindCode => 'Kod';

  @override
  String get historyKindPath => 'Yol';

  @override
  String get historyKindCredentials => 'Kimlik bilgileri';

  @override
  String get historyKindImage => 'Görüntü';

  @override
  String get historyKindFile => 'Dosya';

  @override
  String get historyKindOther => 'Diğer';

  @override
  String get historySortNewest => 'Önce en yeni';

  @override
  String get historySortOldest => 'Önce en eski';

  @override
  String get historySortRelevance => 'En iyi eşleşme';

  @override
  String get historyErrorUpdatesUnavailable =>
      'Geçmiş güncellemeleri geçici olarak kullanılamıyor.';

  @override
  String get historyErrorBulkDelete =>
      'Seçilen bazı öğeler silinemedi. Tekrar deneyin.';

  @override
  String get historyErrorBulkPin =>
      'Seçilen bazı öğelerin sabitlemesi güncellenemedi. Tekrar deneyin.';

  @override
  String get historyErrorRefresh => 'Geçmiş yenilenemedi. Tekrar deneyin.';

  @override
  String get historyErrorLoad => 'Geçmiş yüklenemedi. Tekrar deneyin.';

  @override
  String get historyErrorLoadMore => 'Daha fazla geçmiş yüklenemedi.';

  @override
  String get historyErrorLoadEarlier => 'Önceki geçmiş yüklenemedi.';

  @override
  String get historyErrorOpenClip => 'Bu öğe açılamadı.';

  @override
  String get historyErrorCopyPlainText => 'Düz metin kopyalanamadı.';

  @override
  String get historyErrorCopyClip => 'Bu öğe kopyalanamadı.';

  @override
  String get historyErrorCopyText => 'Bu metin kopyalanamadı.';

  @override
  String get historyErrorPrepareFile =>
      'Bu dosya indirilemedi. Kaynak cihazı kontrol edip tekrar deneyin.';

  @override
  String get historyErrorDownloadFile => 'Bu dosya indirilemedi.';

  @override
  String get historyErrorSelectFiles => 'Dosyalar seçilemedi. Tekrar deneyin.';

  @override
  String get historyErrorReadSelectedFile =>
      'Seçilen dosya okunamadı. Erişimi ve kullanılabilir depolama alanını kontrol edin.';

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
          '$total dosyadan $failed tanesi içe aktarılamadı. İlki: $name. $reason',
      one:
          '$total dosyadan $failed tanesi içe aktarılamadı. İlki: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'Öğenin gizlilik durumu güncellenemedi.';

  @override
  String get historyErrorPinUpdate => 'Sabitleme güncellenemedi.';

  @override
  String get historyErrorPinnedOrderSave =>
      'Sabitlenmiş öğelerin sırası kaydedilemedi. Tekrar deneyin.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'Sıra kaydedildi ancak geçmiş yenilenemedi.';

  @override
  String get historyErrorDeleteClip => 'Bu öğe silinemedi.';

  @override
  String get historyErrorDeleteAll => 'Geçmiş silinemedi.';

  @override
  String get historyErrorBulkDeleteFallback => 'Seçilen öğeler silinemedi.';

  @override
  String get historyOcrErrorUnavailable =>
      'OCR kullanılamıyor. Ayarlar\'da modülü kontrol edip tekrar deneyin.';

  @override
  String get historyOcrErrorStart => 'OCR başlatılamadı. Tekrar deneyin.';

  @override
  String get historyOcrErrorRecognize =>
      'Görüntüdeki metin tanınamadı. Tekrar deneyin.';

  @override
  String get historyOcrRecognizing => 'Görüntüdeki metin tanınıyor.';

  @override
  String get historyOcrFailedTitle => 'OCR başarısız oldu';

  @override
  String get historyOcrNoText => 'Metin bulunamadı';

  @override
  String get historyOcrTitle => 'Görüntüdeki metin';

  @override
  String get historyOcrTextCopied => 'Tanınan metin kopyalandı.';

  @override
  String get historyOcrCopyError => 'Metin kopyalanamadı. Tekrar deneyin.';

  @override
  String get historyTagsErrorLoad => 'Etiketler yüklenemedi. Tekrar deneyin.';

  @override
  String get historyTagsErrorSave =>
      'Etiket değişikliği kaydedilemedi. En fazla 64 karakterlik benzersiz bir ad kullanın ve sabitlenmiş öğeleri seçin.';

  @override
  String get historyTags => 'Etiketler';

  @override
  String get historyTagsUpdateErrorTitle => 'Etiketler güncellenemedi';

  @override
  String get historyTagsEmpty => 'Henüz etiket yok';

  @override
  String get historyTagRename => 'Yeniden adlandır';

  @override
  String get historyTagActions => 'Etiket işlemleri';

  @override
  String get historyTagNamePlaceholder => 'Etiket adı';

  @override
  String get historyCreateTag => 'Etiket oluştur';

  @override
  String get historySaveTag => 'Etiketi kaydet';

  @override
  String get historyTagsFilterAll => 'Tüm etiketler';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count etiket',
      one: '$count etiket',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Etiketlere göre filtrele: $selection';
  }

  @override
  String get historyManageTags => 'Etiketleri yönet';

  @override
  String get historyTagsFilterSemantics =>
      'Sabitlenmiş öğeleri etiketlere göre filtrele';

  @override
  String get historyDownloadErrorCancelled => 'Dosya indirmesi iptal edildi.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'İndirme hizmeti kullanılamıyor. CopyPaste\'i yeniden açıp tekrar deneyin.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'İndirme hizmeti bağlanmadı. Tekrar deneyin.';

  @override
  String get historyFileErrorAccess => 'Seçilen dosyaya erişilemedi.';

  @override
  String get historySpoilerRevealSemantics => 'Gizli içerik. İçeriği göster';

  @override
  String get historyActionFailedTitle => 'İşlem başarısız oldu';

  @override
  String get historyLoadingMessage => 'Pano geçmişi yükleniyor.';

  @override
  String get historyUnavailableTitle => 'Geçmiş kullanılamıyor';

  @override
  String get historyEmptyTitle => 'Öğe bulunamadı';

  @override
  String get historyEmptyMessage =>
      'Yakalanan öğeler ve eşleşen geçmiş burada görünür.';

  @override
  String get historyAttentionTitle => 'Geçmiş dikkat gerektiriyor';

  @override
  String get historyLoadMore => 'Daha fazla yükle';

  @override
  String get historySectionPinned => 'Sabitlenenler';

  @override
  String get historySectionToday => 'Bugün';

  @override
  String get historySectionYesterday => 'Dün';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pano kaydı atlandı',
      one: '$count pano kaydı atlandı',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Saklanan bazı öğeler okunamadı ve bu listede gösterilmiyor.';

  @override
  String get historySearch => 'Geçmişte ara';

  @override
  String get historyResetFilters => 'Filtreleri sıfırla';

  @override
  String get historyImportFiles => 'Dosyaları içe aktar';

  @override
  String get historySelectClips => 'Öğeleri seç';

  @override
  String get historyCloseSearch => 'Aramayı kapat';

  @override
  String get historyFilterAllClips => 'Tüm öğeler';

  @override
  String get historyFilterAllDevices => 'Tüm cihazlar';

  @override
  String get historyFilterAllApps => 'Tüm uygulamalar';

  @override
  String get historyPinClip => 'Öğeyi sabitle';

  @override
  String get historyUnpinClip => 'Öğenin sabitlemesini kaldır';

  @override
  String get historyReorderPinnedClip => 'Sabitlenmiş öğeyi yeniden sırala';

  @override
  String get historyReorderPinnedHint =>
      'Taşımak için sürükleyin. Alt + Yukarı veya Aşağı ok tuşu bir konum taşır.';

  @override
  String get historyClipUnavailable => 'Öğe kullanılamıyor';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Öğe türü: $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Bir öğe seçin';

  @override
  String get historyInspectorEmptyMessage =>
      'Tam içeriği ve işlemleri burada görünür.';

  @override
  String get historyNoPreview => 'Önizleme yok.';

  @override
  String get historyCloseClipDetails => 'Öğe ayrıntılarını kapat';

  @override
  String get historyCopyOptions => 'Kopyalama seçenekleri';

  @override
  String get historyCopyPlainText => 'Düz metin olarak kopyala';

  @override
  String get historyManageClipTags => 'Öğe etiketlerini yönet';

  @override
  String get historyPinnedButton => 'Sabitlendi';

  @override
  String get historyAddSpoiler => 'İçeriği gizle';

  @override
  String get historyRemoveSpoiler => 'Gizlemeyi kaldır';

  @override
  String get historyUpdateFailedTitle => 'Güncelleme başarısız oldu';

  @override
  String get historyRecognizeImageText => 'Görüntüdeki metni tanı';

  @override
  String get historyCopiedTitle => 'Kopyalandı';

  @override
  String get historyCopyFailedTitle => 'Kopyalama başarısız oldu';

  @override
  String get historyPlainTextCopied => 'Düz metin kopyalandı.';

  @override
  String get historyClipCopied => 'Öğe kopyalandı.';

  @override
  String get historyDownloadedTitle => 'İndirildi';

  @override
  String get historyFileSaved => 'Dosya kaydedildi.';

  @override
  String get historyDownload => 'İndir';

  @override
  String get historyDownloading => 'İndiriliyor…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'İndiriliyor: %$percent';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '%$percent';
  }

  @override
  String get historyDownloadCancelling => 'İptal ediliyor…';

  @override
  String get historyDownloadCancellingTooltip => 'İndirme iptal ediliyor';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'İndiriliyor: %$percent · İptal etmek için tıklayın';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'İndirme başarısız oldu · Yeniden denemek için tıklayın';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'İndirmeyi iptal et, %$percent tamamlandı';
  }

  @override
  String get historyRetryDownload => 'İndirmeyi yeniden dene';

  @override
  String get historyDownloadFailedTitle => 'İndirme başarısız oldu';

  @override
  String get historyShowFile => 'Dosyayı göster';

  @override
  String get historyMetadataPrivacy => 'Gizlilik';

  @override
  String get historyMetadataConfidential => 'Gizli';

  @override
  String get historyMetadataCaptured => 'Yakalanma zamanı';

  @override
  String get historyMetadataObservedApp => 'Algılanan uygulama';

  @override
  String get historyMetadataDevice => 'Cihaz';

  @override
  String get historyMetadataClipType => 'Öğe türü';

  @override
  String get historyMetadataType => 'Tür';

  @override
  String get historyMetadataPath => 'Yol';

  @override
  String get historyMetadataFile => 'Dosya';

  @override
  String get historyMetadataResolution => 'Çözünürlük';

  @override
  String get historyMetadataSize => 'Boyut';

  @override
  String get historyMetadataFiles => 'Dosyalar';

  @override
  String get historyMetadataSync => 'Senkronizasyon';

  @override
  String get historyMetadataTooLargeToSync => 'Senkronizasyon için çok büyük';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Seçilen $count öğe silinsin mi?',
      one: 'Seçilen $count öğe silinsin mi?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Bu öğe silinsin mi?';

  @override
  String get historyBulkTagTooltip => 'Seçilen sabitlenmiş öğeleri etiketle';

  @override
  String get historyBulkExit => 'Seçimden çık';

  @override
  String get historyBulkPin => 'Seçilenleri sabitle';

  @override
  String get historyBulkUnpin => 'Seçilenlerin sabitlemesini kaldır';

  @override
  String get historyBulkDelete => 'Seçilenleri sil';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count seçildi',
      one: '$count seçildi',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Tarihe göre filtrele';

  @override
  String get historyDatePresetToday => 'Bugün';

  @override
  String get historyDatePresetYesterday => 'Dün';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Son $days gün',
      one: 'Son $days gün',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'Tüm zamanlar';

  @override
  String get appRuntimeStartFailed => 'CopyPaste çalışma ortamını başlatamadı.';

  @override
  String get appPreparing => 'CopyPaste hazırlanıyor.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste korumalı geçmiş erişimini bekliyor. Uygulamanın kullanılabilir bölümleri kullanılmaya devam edebilir.';

  @override
  String get appHistoryUnavailableTitle =>
      'Geçmiş çalışma ortamı kullanılamıyor';

  @override
  String get appHistoryUnavailableMessage =>
      'Geçmişi açmadan önce uygulama çalışma ortamını başlatın.';

  @override
  String get appDevicesUnavailableTitle =>
      'Cihazlar çalışma ortamı kullanılamıyor';

  @override
  String get appDevicesUnavailableMessage =>
      'Cihazları açmadan önce uygulama çalışma ortamını başlatın.';

  @override
  String get appSettingsUnavailableTitle =>
      'Ayarlar çalışma ortamı kullanılamıyor';

  @override
  String get appSettingsUnavailableMessage =>
      'Ayarları açmadan önce uygulama çalışma ortamını başlatın.';

  @override
  String get appQuit => 'CopyPaste\'ten çık';

  @override
  String get appWindowSetupTitle => 'Pencere kurulumu dikkat gerektiriyor';

  @override
  String get appOpenHistory => 'Geçmiş\'i aç';

  @override
  String get appDestinationHistory => 'Geçmiş';

  @override
  String get appDestinationDevices => 'Cihazlar';

  @override
  String get appDestinationSettings => 'Ayarlar';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, seçili';
  }

  @override
  String get appActiveFilter => 'Etkin filtre';

  @override
  String get appActionFailed => 'İşlem başarısız oldu';

  @override
  String get sharedAndroidVersionRequired =>
      'Android 11 veya daha yeni bir sürüm gerekli';

  @override
  String get sharedUseAdbTab => 'Bu cihazda ADB sekmesini kullanın.';

  @override
  String get sharedOneTimeAccessApplied => 'Tek seferlik erişim uygulandı';

  @override
  String get sharedInstallShizuku => 'Shizuku\'yu yükle';

  @override
  String get sharedPairAndStartShizuku => 'Shizuku\'yu eşle ve başlat';

  @override
  String get sharedAllowCopyPaste => 'CopyPaste\'e izin ver';

  @override
  String get sharedShizukuNoLongerNeeded => 'Shizuku\'ya artık gerek yok.';

  @override
  String get sharedGetShizukuDescription =>
      'Tek seferlik erişimi uygulamak için Shizuku\'yu edinin.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Shizuku\'da Kablosuz hata ayıklamayı kullanın, ardından buraya dönün.';

  @override
  String get sharedApproveInShizuku =>
      'Shizuku\'da CopyPaste\'i bir kez onaylayın.';

  @override
  String get sharedGetShizuku => 'Shizuku\'yu edin';

  @override
  String get sharedOpenShizuku => 'Shizuku\'yu aç';

  @override
  String get sharedRunOnComputer => 'Bilgisayarınızda çalıştırın';

  @override
  String get sharedCopyAllCommands => 'Tüm komutları kopyala';

  @override
  String get sharedEnableUsbDebugging =>
      'USB hata ayıklamayı etkinleştirin ve bu telefonu bağlayın.';

  @override
  String get sharedAllow => 'İzin ver';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Kurulum komutları kopyalanamadı.';

  @override
  String get sharedShizukuOpenFailed => 'Shizuku açılamadı.';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => 'CopyPaste kurulumu';

  @override
  String get onboardingWelcomeTitle => 'CopyPaste\'e hoş geldiniz';

  @override
  String get onboardingWelcomeSubtitle =>
      'Panonuz, ihtiyacınız olduğunda hazır.';

  @override
  String get onboardingReadyTitle => 'CopyPaste hazır';

  @override
  String get onboardingGetStarted => 'Başlayın';

  @override
  String get onboardingOptionalSetupTitle =>
      'İsteğe bağlı kurulum dikkat gerektiriyor';

  @override
  String get onboardingOpenSystemSettings => 'Ayarlar\'ı aç';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return 'ADIM $step/$count · $platform';
  }

  @override
  String get onboardingSetupErrorTitle => 'Kurulum dikkat gerektiriyor';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste, $platform kurulum durumunu okuyamadı.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste kurulum ilerlemesini kaydedemedi.';

  @override
  String get onboardingStartAtLogin => 'Girişte başlat';

  @override
  String get onboardingStartAtLoginDescription =>
      'Oturum açtığınızda CopyPaste açılır';

  @override
  String get onboardingStartAtLoginSemantics => 'CopyPaste\'i girişte başlat';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Girişte başlatma etkinleştirilemedi. Bu özellik olmadan devam edebilirsiniz.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Girişte başlatma güncellenemedi. Bu özellik olmadan devam edebilirsiniz.';

  @override
  String get onboardingMacosTitle => 'Mac\'inizi ayarlayın';

  @override
  String get onboardingAccessibility => 'Erişilebilirlik';

  @override
  String get onboardingAutoPasteEnabled => 'Otomatik yapıştırma etkin';

  @override
  String get onboardingAutoPasteOptional =>
      'Otomatik yapıştırma için isteğe bağlı';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Yüklü CopyPaste uygulamasında kullanılabilir.';

  @override
  String get onboardingLoginItemApprovalTitle => 'Giriş Öğesi onayı gerekli';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Sistem Ayarları\'nda CopyPaste\'e izin verin, ardından geri dönün.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste macOS kurulum durumunu okuyamadı.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Erişilebilirlik izni istenemedi.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste macOS kurulum durumunu yenileyemedi.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Erişilebilirlik durumu yenilenemedi.';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Giriş Öğeleri ayarları açılamadı.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'Girişte başlatma için Sistem Ayarları\'nda onay gerekir.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Girişte başlatma devre dışı bırakılamadı.';

  @override
  String get onboardingWindowsTitle => 'Bilgisayarınızı ayarlayın';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'Bu kurulumda kullanılamıyor. Bu özellik olmadan devam edebilirsiniz.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Görev Yöneticisi\'nde devre dışı';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Windows başlangıç uygulamalarında CopyPaste\'i etkinleştirin, ardından geri dönün.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'Girişte başlatma Görev Yöneticisi\'nde devre dışı bırakılmış. Daha sonra Windows Ayarları\'ndan etkinleştirebilirsiniz.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'Girişte başlatma kullanılamıyor. Bu özellik olmadan devam edebilirsiniz.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste Windows kurulum durumunu okuyamadı.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Windows başlangıç ayarları açılamadı.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Girişte başlatma güncellenemedi. Daha sonra Ayarlar\'dan değiştirebilirsiniz.';

  @override
  String get onboardingLinuxChecking =>
      'Linux masaüstü oturumunuz denetleniyor.';

  @override
  String get onboardingLinuxTitle => 'Linux masaüstünüzü ayarlayın';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste, pano kaynağını belirlemek ve Quick Paste için doğrulanmış masaüstü entegrasyonuna ihtiyaç duyar.';

  @override
  String get onboardingLinuxSession => 'Masaüstü oturumu';

  @override
  String get onboardingLinuxX11Detected => 'X11 oturumu algılandı.';

  @override
  String get onboardingLinuxWaylandDetected => 'Wayland oturumu algılandı.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'Desteklenen bir X11 veya Wayland masaüstü oturumu algılanmadı.';

  @override
  String get onboardingLinuxCompanion => 'GNOME veya KDE yardımcısı';

  @override
  String get onboardingLinuxCompanionReady =>
      'Kimliği doğrulanmış Clipboard v2 köprüsü hazır.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'Pano yakalama, kaynak uygulama hariç tutmaları nedeniyle duraklatıldı. Sürdürmek için imzalı yardımcıyı etkinleştirin.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'İmzalı yardımcı oturum paketini yükleyip etkinleştirin, ardından buraya dönün.';

  @override
  String get onboardingLinuxOpenSetup => 'Kurulumu aç';

  @override
  String get onboardingLinuxKeyboardControl => 'Klavye denetimi';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'Quick Paste için klavye denetimine izin verildi.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'Klavye denetimi bu masaüstü oturumunda kullanılamıyor.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Klavye denetimine izin ver';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste hazır.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Quick Paste, gerekli masaüstü entegrasyonu etkinleştikten sonra hazır olacak.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'Oturum açtığınızda CopyPaste açılır.';

  @override
  String get onboardingLinuxLinks => 'CopyPaste bağlantıları';

  @override
  String get onboardingLinuxLinksRegistered =>
      'CopyPaste bağlantıları bu uygulamada açılır.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Eşleme davetleri için CopyPaste bağlantılarını kaydedin.';

  @override
  String get onboardingLinuxRegisterLinks => 'Bağlantıları kaydet';

  @override
  String get onboardingLinuxScreenshotBlocking => 'Ekran görüntüsü engelleme';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Ekran görüntüsü engelleme Linux\'ta kullanılamıyor. Korunan depolanmış veriler şifreli kalır.';

  @override
  String get onboardingLinuxCheckAgain => 'Yeniden denetle';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste Linux entegrasyon durumunu okuyamadı.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Masaüstü entegrasyonu kurulumu açılamadı.';

  @override
  String get onboardingLinuxKeyboardDenied => 'Klavye denetimi izni verilmedi.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Girişte başlatma güncellenemedi.';

  @override
  String get onboardingLinuxLinksFailed =>
      'CopyPaste bağlantıları kaydedilemedi.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Linux entegrasyon durumu yenilenemedi.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'Bu X11 oturumunda pano erişimi kullanılamıyor.';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste için X11 veya Wayland masaüstü oturumu gerekir.';

  @override
  String get onboardingAndroidFullCapture => 'Tam yakalama';

  @override
  String get onboardingAndroidBackgroundCapture => 'Arka plan yakalaması';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Bir kez ayarlayın, ardından gerçek bir kopyalamayla doğrulayın.';

  @override
  String get onboardingAndroidLimitedCapture => 'Sınırlı yakalama';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'CopyPaste ile paylaşın veya kopyaladıktan sonra geri dönün.';

  @override
  String get onboardingAndroidNotification => 'Yakalama bildirimi';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Yakalama çalışırken gerekli';

  @override
  String get onboardingAndroidBackgroundActivity => 'Arka plan etkinliği';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Yakalama kaybını azaltmak için önerilir';

  @override
  String get onboardingAndroidOpen => 'Aç';

  @override
  String get onboardingAndroidApplyAccess => 'Yakalama erişimini uygula';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Tek seferlik yakalama erişimini uygulayın.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Tam yakalamayı daha sonra ayarlayabilirsiniz.';

  @override
  String get onboardingAndroidCaptureWorking =>
      'Arka plan yakalaması çalışıyor';

  @override
  String get onboardingAndroidVerifyCopy => 'Gerçek bir kopyalamayı doğrulayın';

  @override
  String get onboardingAndroidCopyReached =>
      'Yeni bir kopya yerel Geçmiş\'inize ulaştı.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Başka bir uygulamada kopyalayın, ardından buraya dönün.';

  @override
  String get onboardingAndroidStartCapture => 'Yakalamayı başlat';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste Android kurulum durumunu okuyamadı.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Android yakalama durumu yenilenemedi.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Android kurulum izleme kullanılamıyor.';

  @override
  String get onboardingAndroidIntakeFailed => 'Pano alımı etkinleştirilemedi.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Arka plan yakalaması durdurulamadı.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Pil ayarları açılamadı.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste\'e Shizuku\'da izin verildi ancak Android yakalama izinleri uygulanamadı.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Yakalama izinlerini uygulamak için Shizuku\'da CopyPaste\'e izin verin.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Önce arka plan pano erişimi verin.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Arka plan yakalamasını başlatmadan önce bildirimlere izin verin.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Arka plan yakalaması başlatılamadı.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Android yakalama kurulumu açılamadı.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Android kurulum seçimi kaydedilemedi.';

  @override
  String get modulesMarketplaceTab => 'Mağaza';

  @override
  String get modulesInstalledTab => 'Yüklü';

  @override
  String get modulesRefresh => 'Yenile';

  @override
  String get modulesSearchPlaceholder => 'Modüllerde ara';

  @override
  String get modulesOperationFailed => 'Modül işlemi başarısız oldu';

  @override
  String get modulesLoading => 'Modüller yükleniyor.';

  @override
  String get modulesUnavailableTitle => 'Modüller kullanılamıyor';

  @override
  String get modulesUnavailableMessage =>
      'Modülleri yeniden yüklemeyi deneyin.';

  @override
  String get modulesNoneInstalled => 'Yüklü modül yok';

  @override
  String get modulesNoMatches => 'Eşleşen modül yok';

  @override
  String get modulesChooseFromMarketplace => 'Mağazadan bir modül seçin.';

  @override
  String get modulesTryDifferentSearch => 'Farklı bir arama deneyin.';

  @override
  String get modulesMarketplaceLoading => 'Mağaza yükleniyor.';

  @override
  String get modulesMarketplaceUnavailableTitle => 'Mağaza kullanılamıyor';

  @override
  String get modulesMarketplaceUnavailableMessage =>
      'Mağazayı yeniden yüklemeyi deneyin.';

  @override
  String get modulesNoneAvailable => 'Kullanılabilir modül yok';

  @override
  String get modulesNoneAvailableMessage =>
      'Modüller yayımlandığında burada görünür.';

  @override
  String modulesEnableModule(String module) {
    return '$module modülünü etkinleştir';
  }

  @override
  String get modulesInstall => 'Yükle';

  @override
  String modulesUpdateTo(String version) {
    return '$version sürümüne güncelle';
  }

  @override
  String get modulesRemoveModule => 'Modülü kaldır';

  @override
  String get modulesModuleSettings => 'Modül ayarları';

  @override
  String modulesSettingsTitle(String module) {
    return '$module ayarları';
  }

  @override
  String get modulesDownloadingModel => 'Dil modeli indiriliyor…';

  @override
  String get modulesRun => 'Çalıştır';

  @override
  String modulesRemoveTitle(String module) {
    return '$module kaldırılsın mı?';
  }

  @override
  String get modulesRemoveMessage => 'Modül ve yerel ayarları kaldırılacak.';

  @override
  String get modulesRemove => 'Kaldır';

  @override
  String get modulesRemoveFailed => 'Modül kaldırılamadı.';

  @override
  String get modulesCloseSettings => 'Modül ayarlarını kapat';

  @override
  String get modulesRemoved => 'Modül kaldırıldı';

  @override
  String get modulesNoSettingsTitle => 'Ek ayar yok';

  @override
  String get modulesNoSettingsMessage =>
      'Bu modül ek yapılandırma gerektirmiyor.';

  @override
  String get modulesSetUpSmsAccess => 'SMS erişimini ayarla';

  @override
  String get modulesRestartApp => 'CopyPaste\'i yeniden başlat';

  @override
  String get modulesSmsAccessTitle => 'SMS erişimi';

  @override
  String get modulesCloseSmsAccess => 'SMS erişimini kapat';

  @override
  String get modulesSmsChecking => 'SMS erişimi denetleniyor.';

  @override
  String get modulesSmsReady =>
      'Erişim hazır. Yeni kodları kopyalamaya başlamak için SMS Codes\'u etkinleştirin.';

  @override
  String get modulesSmsAllowAccess =>
      'Yeni SMS mesajlarındaki kodları kopyalamak için erişime izin verin.';

  @override
  String get modulesSmsNotification => 'SMS bildirimi';

  @override
  String get modulesSmsNotificationDescription =>
      'SMS Codes çalışırken gerekli';

  @override
  String get modulesSmsApplyAccess => 'SMS erişimini uygula';

  @override
  String get modulesSmsApplyAccessDescription =>
      'Tek seferlik SMS erişimini uygulayın.';

  @override
  String get modulesSmsSetupFailed => 'SMS kurulumu başarısız oldu';

  @override
  String get modulesFileSelectionFailed => 'Dosya seçimi başarısız oldu';

  @override
  String get modulesChooseLanguages => 'Dilleri seçin';

  @override
  String get modulesChooseFile => 'Dosya seç';

  @override
  String modulesModelReady(String model) {
    return '$model modeli hazır';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return '$model modeli · $size MiB indirme';
  }

  @override
  String get modulesPlatformsUnknown => 'Platformlar: Bilinmiyor';

  @override
  String modulesPlatforms(String platforms) {
    return 'Platformlar: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'Modül kataloğunun imzası geçersiz.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Modül mağazasına bağlanılamadı.';

  @override
  String get modulesMarketplaceTimeout => 'Modül mağazası yanıt vermedi.';

  @override
  String get modulesIncompatible => 'Bu modül bu cihazla uyumlu değil.';

  @override
  String get modulesPackageSizeInvalid => 'Modül paketinin boyutu geçersiz.';

  @override
  String get modulesDownloadInterrupted => 'Modül indirmesi yarıda kesildi.';

  @override
  String get modulesDownloadTimeout => 'Modül indirmesi yanıt vermedi.';

  @override
  String get modulesPackageUrlUntrusted =>
      'Modül paketinin URL\'si güvenilir değil.';

  @override
  String get modulesDownloadLeftGithub =>
      'Modül indirmesi GitHub dışına yönlendirildi.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'Modül indirmesi çok fazla yönlendirme kullandı.';

  @override
  String get modulesDownloadTooLarge => 'Modül indirmesi çok büyük.';

  @override
  String get modulesPackageIntegrityFailed =>
      'Modül paketi bütünlük denetiminden geçemedi.';

  @override
  String get modulesDownloadUnavailable =>
      'Bu modül indirmesi şu anda kullanılamıyor.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'Modül mağazası kullanılamıyor. Tekrar deneyin.';

  @override
  String get modulesCatalogInvalid => 'Modül kataloğu geçersiz.';

  @override
  String get modulesThisSystem => 'bu sistem';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version veya üstü';
  }

  @override
  String get modulesNotAvailableForDevice => 'Bu cihaz için kullanılamıyor.';

  @override
  String get modulesAppVersionUnverified =>
      'Yüklü CopyPaste sürümü doğrulanamadı.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Bu modül CopyPaste $version sürümünü desteklemiyor.';
  }

  @override
  String modulesRequires(String requirement) {
    return '$requirement gerekir.';
  }

  @override
  String get modulesMarketplaceLoadFailed => 'Modül mağazası yüklenemedi.';

  @override
  String get modulesSmsSetupRequired =>
      'Bu modülü etkinleştirmeden önce SMS erişimini ayarlayın.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'SMS izleme başlatılamadı. Modül hâlâ etkin; yeniden denemek için modülü tekrar devre dışı bırakın.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'SMS izleme başlatılamadı. Modül devre dışı bırakıldı.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'SMS izleme başlatılamadı. Modülün etkin olup olmadığı doğrulanamadı.';

  @override
  String get modulesSmsMonitoringFailed => 'SMS izleme başlatılamadı.';

  @override
  String get modulesSmsAndroidOnly =>
      'SMS erişimi yalnızca Android\'de kullanılabilir.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'SMS izlemenin ayarlanması gerekiyor.';

  @override
  String get modulesOperationIncomplete => 'Modül işlemi tamamlanamadı.';

  @override
  String get modulesUnsupportedEvent =>
      'Modül desteklenmeyen bir olay kullanıyor.';

  @override
  String get modulesUnsupportedResult =>
      'Modül desteklenmeyen bir sonuç döndürdü.';

  @override
  String get modulesUnsupportedField =>
      'Modül desteklenmeyen bir alan kullanıyor.';

  @override
  String get modulesResponseInvalid => 'Modül yanıtı geçersiz.';

  @override
  String get modulesFileSelectionUnavailable => 'Dosya seçimi kullanılamıyor.';

  @override
  String get modulesFileSelectFailed => 'Dosya seçilemedi.';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field alanı için bu dosya kabul edilmiyor.';
  }

  @override
  String get modulesSmsVerifyFailed => 'SMS erişimi doğrulanamadı.';

  @override
  String get modulesSmsAllowNotifications =>
      'SMS Codes\'un arka planda etkin kalması için bildirimlere izin verin.';

  @override
  String get modulesSmsNotGranted => 'SMS erişimi verilmedi.';

  @override
  String get modulesSmsConfigureFailed => 'SMS erişimi yapılandırılamadı.';

  @override
  String get onboardingLinuxContinueBasic => 'Temel yakalamayla devam et';

  @override
  String get onboardingLinuxCompanionOptional =>
      'İsteğe bağlı. Quick Paste ve kaynak uygulama hariç tutmaları için gerekir.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'İsteğe bağlı. Quick Paste\'in önceki uygulamaya dönebilmesi için gerekir.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'Bu Wayland oturumunda pano erişimi kullanılamıyor. İmzalı GNOME veya KDE yardımcısını etkinleştirin, ardından yeniden denetleyin.';

  @override
  String get historyMoreActions => 'Diğer işlemler';

  @override
  String get historyTagsAction => 'Etiketler…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'gizli öğelerin kilidini açmaya',
      'other': 'Gizli öğelerin kilidini aç',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'gizli bir kod parçacığını kullanmaya',
      'other': 'Gizli kod parçacığını kullan',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'gizli bir öğeyi kopyalamaya',
      'other': 'Gizli öğeyi kopyala',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'Bu sistemde kullanılamaz';

  @override
  String get deviceAuthNotConfigured =>
      'Önce bir cihaz parolası, PIN veya biyometri ayarlayın';

  @override
  String get commonOn => 'Açık';

  @override
  String get commonOff => 'Kapalı';

  @override
  String get settingsQuickPasteStartFailed =>
      'Quick Paste başlatılamadı, bu yüzden kısayolu çalışmıyor.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut başka bir uygulama tarafından kullanılıyor. Başka bir kısayol seçin.';
  }

  @override
  String get devicesEmptyTitle => 'Henüz eşlenmiş başka cihaz yok';

  @override
  String get devicesEmptyMessage =>
      'Pano geçmişini cihazlar arasında senkronize tutmak için başka bir cihaz eşleyin. Her iki cihaz da aynı ağda olmalıdır ve her eşleme iki cihazda da onaylanır.';

  @override
  String get pairingStateNotCompleted => 'Eşleme tamamlanmadı';

  @override
  String get pairingStepInvite => 'Davet';

  @override
  String get pairingStepScan => 'Tarama';

  @override
  String get pairingStepCode => 'Kod';

  @override
  String get pairingStepRequest => 'İstek';

  @override
  String get pairingStepConnect => 'Bağlantı';

  @override
  String get pairingStepConfirm => 'Onay';

  @override
  String get pairingStepPaired => 'Eşlendi';

  @override
  String get pairingOtherDevice => 'Diğer cihaz';

  @override
  String get pairingDecisionNotConfirmed => 'Henüz onaylanmadı';

  @override
  String get pairingDecisionSending => 'Kararınız gönderiliyor…';

  @override
  String get pairingDecisionConfirmed => 'Onaylandı';

  @override
  String get pairingDecisionRejected => 'Reddedildi';

  @override
  String get pairingDecisionWaiting => 'Onay bekleniyor';

  @override
  String get pairingDecisionUnknown => 'Yanıt bilinmiyor';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count cihaz',
      one: '$count cihaz',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count satır',
      one: '$count satır',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Ana gezinme';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Adım $step/$count: $name';
  }

  @override
  String get modulesPhaseDownload => 'İndirme';

  @override
  String get modulesPhaseInstall => 'Yükleme';

  @override
  String modulesInstallingModule(String module) {
    return '$module yükleniyor';
  }

  @override
  String modulesUpdatingModule(String module) {
    return '$module güncelleniyor';
  }

  @override
  String get commonDismiss => 'Kapat';

  @override
  String get settingsRetentionConfirmTitle => 'Eski geçmiş kaldırılsın mı?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Yeni sınır: $limit. Daha eski sabitlenmemiş öğeler bu cihazdan şimdi kaldırılır ve geri getirilemez. Sabitlenmiş öğeler korunur.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle =>
      'Depolama kotası azaltılsın mı?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Yeni kota: $size. Sabitlenmemiş geçmiş daha fazla yer kaplıyorsa en eski öğeler şimdi kaldırılır ve geri getirilemez. Sabitlenmiş öğeler korunur.';
  }

  @override
  String get settingsLimitConfirmAction => 'Sınırı uygula';

  @override
  String get settingsHideEverywhereTitle => 'CopyPaste tamamen gizlensin mi?';

  @override
  String get settingsHideEverywhereMessage =>
      'İkisi de kapalıyken CopyPaste\'in görünür bir simgesi olmaz. Yeniden açmak için CopyPaste\'i tekrar başlatın.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'İkisi de kapalıyken CopyPaste\'in görünür bir simgesi olmaz. Yeniden açmak için $shortcut tuşlarına basın veya CopyPaste\'i tekrar başlatın.';
  }

  @override
  String get settingsHideEverywhereAction => 'Gizle';

  @override
  String get settingsExportDoneTitle => 'Metin geçmişi dışa aktarıldı';

  @override
  String get settingsBackupDoneTitle => 'Yedek oluşturuldu';

  @override
  String get settingsRestoreDoneTitle => 'Geçmiş geri yüklendi';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count metin öğesi dışa aktarıldı',
      one: '$count metin öğesi dışa aktarıldı',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe okunamadı',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Şifreli yedek kaydedildi · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'Bu cihazdaki pano geçmişi artık yedekle aynı.';

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
    return '$quota alanın $used kadarı kullanılıyor';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '$used kullanılıyor';
  }

  @override
  String get storageUsageOverQuota => 'Kota aşıldı';

  @override
  String storageUsageOldestClip(String date) {
    return 'En eski öğe: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Depolama: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Yeni kota: $size. Sabitlenmemiş geçmiş $used yer kaplıyor; bu yüzden sığana kadar en eski öğeleri şimdi kaldırılır. Bunlar geri getirilemez. Sabitlenmiş öğeler korunur.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe',
      one: '$count öğe',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'CopyPaste\'i aç';

  @override
  String get trayOpenQuickPaste => 'Quick Paste\'i aç';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Quick Paste\'i aç ($shortcut)';
  }

  @override
  String get trayNewSnippet => 'Yeni parçacık';

  @override
  String get trayRecent => 'Son kullanılanlar';

  @override
  String get trayRecentLoading => 'Yükleniyor…';

  @override
  String get trayRecentFailed => 'Kliplar yüklenemedi';

  @override
  String get trayRecentEmpty => 'Henüz klip yok';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count son klip gizlendi',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Gizli';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Diğer cihazdaki en son klibi kopyala';

  @override
  String get trayCaptureNow => 'Şimdi yakala';

  @override
  String trayPausedUntil(String time) {
    return '$time saatine kadar duraklatıldı';
  }

  @override
  String get trayPauseFor => 'Şu süreyle duraklat…';

  @override
  String get trayPauseFor15Minutes => '15 dakika';

  @override
  String get trayPauseForOneHour => '1 saat';

  @override
  String get trayPauseUntilResumed => 'Ben sürdürene kadar';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count öğe',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Toplanıyor: $count öğe',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Son atlananlar ($count)';
  }

  @override
  String get trayOpenContentRules => 'İçerik kurallarını aç';

  @override
  String get trayRecentSettingsTitle => 'Menü çubuğu ve tepsi';

  @override
  String get trayRecentSettingsDescription =>
      'Menü çubuğu veya tepsi menüsünün Son kullanılanlar altında gösterdikleri.';

  @override
  String get trayRecentCountTitle => 'Son kliplar';

  @override
  String get trayRecentCountDescription =>
      'Menünün gösterdiği en yeni klip sayısı.';

  @override
  String get trayRecentPreviewTitle => 'Son klip önizlemeleri';

  @override
  String get trayRecentPreviewDescription =>
      'Sistem, menü metnini CopyPaste şifrelemesinin dışında tutar. Klip içeriğini oradan uzak tutmak için daha az gösterin. Gizli öğeler her zaman maskelenir.';

  @override
  String get trayRecentPreviewFull => 'Tam önizleme';

  @override
  String get trayRecentPreviewTitles => 'Yalnızca tür ve kaynak';

  @override
  String get trayRecentPreviewHidden => 'Gizli';

  @override
  String get trayRecentSettingsKeywords =>
      'tepsi menü çubuğu son önizleme gizlilik sayı';

  @override
  String get transformsPaletteTitle => 'Metni dönüştür';

  @override
  String get transformsFilterPlaceholder => 'Dönüşümleri filtrele';

  @override
  String get transformsNoMatches => 'Eşleşen dönüşüm yok';

  @override
  String get transformsGroupResults => 'Sonuçlar';

  @override
  String get transformsGroupSuggested => 'Önerilen';

  @override
  String get transformsGroupRecent => 'Son kullanılan';

  @override
  String get transformsGroupFavorites => 'Favoriler';

  @override
  String get transformsGroupPipelines => 'İşlem hatları';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV ve XML';

  @override
  String get transformsCategoryEncode => 'Kodla ve çöz';

  @override
  String get transformsCategoryCase => 'Büyük/küçük harf';

  @override
  String get transformsCategoryHash => 'Özetler';

  @override
  String get transformsCategoryLines => 'Satırlar ve boşluklar';

  @override
  String get transformsCategoryClean => 'Temizle';

  @override
  String get transformsCategoryUrl => 'Bağlantılar';

  @override
  String get transformsCategoryMarkup => 'Zengin metin ve Markdown';

  @override
  String get transformsCategoryDev => 'Geliştirici';

  @override
  String get transformsCategoryOther => 'Diğer';

  @override
  String get transformsChainEmpty => 'Henüz adım yok';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Sonra: $name';
  }

  @override
  String get transformsStepEnabled => 'Bu adımı çalıştır';

  @override
  String get transformsMoveEarlier => 'Yukarı taşı';

  @override
  String get transformsMoveLater => 'Aşağı taşı';

  @override
  String get transformsRemoveStep => 'Adımı kaldır';

  @override
  String get transformsSavePipeline => 'İşlem hattı olarak kaydet';

  @override
  String get transformsSavePipelineTitle => 'İşlem hattını kaydet';

  @override
  String get transformsSavePipelineFailed => 'İşlem hattı kaydedilemedi.';

  @override
  String get transformsPipelineNameLabel => 'Ad';

  @override
  String get transformsPipelineDescriptionLabel => 'Açıklama (isteğe bağlı)';

  @override
  String get transformsPipelineStepsLabel => 'Adımlar';

  @override
  String get transformsPipelineNoSteps => 'En az bir adım ekleyin.';

  @override
  String get transformsPipelineNameRequired => 'Bir ad girin.';

  @override
  String get transformsPipelineNameTooLong => 'En fazla 80 karakter kullanın.';

  @override
  String get transformsPipelineNameTaken =>
      'Bu ada sahip başka bir işlem hattı var.';

  @override
  String get transformsPipelineSavedTitle => 'İşlem hattı kaydedildi';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name her dönüşüm paletinde kullanılabilir.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Adımlar: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Silinmiş işlem hattı';

  @override
  String get transformsNeedsNewerVersion =>
      'Daha yeni bir CopyPaste sürümü gerekir';

  @override
  String get transformsLoadFailed => 'Dönüşümler yüklenemedi.';

  @override
  String get transformsNotTextTitle => 'Dönüşümler metin üzerinde çalışır';

  @override
  String get transformsNotTextMessage =>
      'Bu öğe bir görsel, dosya veya renk. Bir metin öğesi seçin ya da metin kopyalayın.';

  @override
  String get transformsNothingTitle => 'Dönüştürülecek bir şey yok';

  @override
  String get transformsNothingMessage =>
      'Metin kopyalayın veya geçmişten bir öğe seçin.';

  @override
  String get transformsPickTransform => 'Bir dönüşüm seçin';

  @override
  String get transformsPickTransformHint =>
      'Tab adım olarak ekler. Enter sonucu uygular.';

  @override
  String get transformsPreviewing => 'Önizleme güncelleniyor…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Satır: $lines · Sözcük: $words · Karakter: $chars';
  }

  @override
  String get transformsNoChanges => 'Değişiklik yok';

  @override
  String transformsTruncated(String size) {
    return '$size içinden ilk 64 KiB gösteriliyor';
  }

  @override
  String get transformsRandom => 'Rastgele';

  @override
  String get transformsNotRun => 'Çalıştırılmadı';

  @override
  String get transformsSensitiveHidden => 'Sonuç hassas olduğundan gizlendi.';

  @override
  String get transformsReveal => 'Göster';

  @override
  String get transformsPaneOutput => 'Çıktı';

  @override
  String get transformsPaneInput => 'Girdi';

  @override
  String get transformsPaneChanges => 'Değişiklikler';

  @override
  String get transformsCannotTransform => 'Bu metin dönüştürülemiyor';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Adım $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Onaltılık göster';

  @override
  String get transformsActionPaste => 'Yapıştır';

  @override
  String get transformsActionSave => 'Yeni öğe olarak kaydet';

  @override
  String get transformsCopiedMessage => 'Sonuç panoda ve geçmişte.';

  @override
  String get transformsSavedTitle => 'Kaydedildi';

  @override
  String get transformsSavedMessage =>
      'Sonuç yeni bir geçmiş öğesi oldu. Pano değişmedi.';

  @override
  String get transformsPasteFailedTitle => 'Yapıştırılamadı';

  @override
  String get transformsPasteFailedMessage => 'Sonuç panoda. Elle yapıştırın.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message ($line. satır, $column. sütun)';
  }

  @override
  String get transformsErrorBinary => 'Çözülen veri metin değil, ikili';

  @override
  String get transformsErrorTooLarge => 'Dönüştürmek için çok büyük';

  @override
  String get transformsErrorInvalidParam => 'Bu adımın bir ayarı geçersiz';

  @override
  String get transformsErrorCancelled => 'İptal edildi';

  @override
  String get transformsErrorTooExpensive => 'Desen çok maliyetli';

  @override
  String get transformsErrorPattern => 'Desen geçersiz';

  @override
  String get transformsErrorTimestampUnit =>
      'Sayının saniye mi milisaniye mi olduğunu seçin';

  @override
  String get transformsErrorExternalEntity => 'Harici varlıklara izin verilmez';

  @override
  String get transformsErrorJwtEncrypted => 'Şifreli belirteçler çözülemez';

  @override
  String get transformsErrorJson => 'Geçersiz JSON';

  @override
  String get transformsErrorYaml => 'Geçersiz YAML';

  @override
  String get transformsErrorXml => 'Geçersiz XML';

  @override
  String get transformsErrorCsv => 'Geçersiz CSV';

  @override
  String get transformsErrorBase64 => 'Geçersiz Base64';

  @override
  String get transformsErrorHex => 'Geçersiz onaltılık';

  @override
  String get transformsErrorEscape => 'Geçersiz kaçış dizisi';

  @override
  String get transformsErrorUrl => 'Hatalı yüzde kodlaması';

  @override
  String get transformsErrorJwt => 'Geçerli bir JWT değil';

  @override
  String get transformsErrorDate =>
      'Tanınan bir tarih veya zaman damgası değil';

  @override
  String get transformsErrorGeneric => 'Bu adım girdisini dönüştüremiyor';

  @override
  String get transformsWarningSignedLink => 'İmzalı bağlantı değiştirilmedi';

  @override
  String get transformsWarningExpired => 'Süresi dolmuş';

  @override
  String get transformsWarningNotYetValid => 'Henüz geçerli değil';

  @override
  String get transformsWarningSignatureNotVerified => 'İmza doğrulanmadı';

  @override
  String get transformsWarningDuplicateKeys => 'Yinelenen anahtarlar';

  @override
  String get transformsWarningLenientJson => 'Standart dışı JSON kabul edildi';

  @override
  String get transformsWarningDoubleEncoded => 'JSON iki kez kodlanmıştı';

  @override
  String get transformsWarningEscapes =>
      'Bilinmeyen kaçış dizileri olduğu gibi bırakıldı';

  @override
  String get transformsWarningLocalZone => 'Yerel saat dilimi varsayıldı';

  @override
  String get transformsWarningNoMatches => 'Eşleşme yok';

  @override
  String get transformsWarningGeneric => 'Sonucu kontrol edin';

  @override
  String get transformsRestoreTitle => 'Yapıştırdıktan sonra panoyu geri yükle';

  @override
  String get transformsRestoreDescription =>
      'Dönüştürülmüş yapıştırmadan sonra, arada başka bir şey kopyalanmadıysa önceki pano geri gelir.';

  @override
  String get transformsDefaultTitle => 'Varsayılan yapıştırma dönüşümü';

  @override
  String get transformsDefaultDescription =>
      'Quick Paste bu işlem hattını metne uygular ve yapıştırmadan önce gösterir. Başarısız olursa orijinali yapıştırır.';

  @override
  String get transformsNoDefault => 'Yok';

  @override
  String get transformsAppDefaultsTitle => 'Uygulamaya göre varsayılanlar';

  @override
  String get transformsAppDefaultsDescription =>
      'Bir uygulamanın varsayılanını paket kimliği, yürütülebilir dosya veya paket adıyla geçersiz kılın. Başka platformda yapılan girişler eşitlenir ama burada etkisizdir.';

  @override
  String get transformsAppIdentifierPlaceholder => 'Uygulama tanımlayıcısı';

  @override
  String transformsOtherPlatform(String platform) {
    return 'yalnızca $platform';
  }

  @override
  String get transformsAdd => 'Ekle';

  @override
  String get transformsRemove => 'Kaldır';

  @override
  String get transformsFavoritesTitle => 'Favoriler';

  @override
  String get transformsFavoritesDescription =>
      'Favoriler her palette önce görünür. İlk dokuzu Mod+1–Mod+9 ile çalışır.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Palette Mod+$number';
  }

  @override
  String get transformsAddFavorite => 'Favori ekle';

  @override
  String get transformsPipelinesTitle => 'İşlem hatları';

  @override
  String get transformsPipelinesDescription =>
      'Ayarlarıyla kaydedilmiş dönüşüm zincirleri. Diğer cihazlarınızla eşitlenir.';

  @override
  String get transformsNewPipeline => 'Yeni işlem hattı';

  @override
  String get transformsEditPipeline => 'İşlem hattını düzenle';

  @override
  String get transformsDuplicatePipeline => 'Çoğalt';

  @override
  String get transformsDeletePipeline => 'Sil';

  @override
  String transformsDeletePipelineTitle(String name) {
    return '$name silinsin mi?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Bunu kullanan favoriler ve yapıştırma varsayılanları tüm cihazlarda yok olarak döner.';

  @override
  String get transformsDeletePipelineFailed => 'İşlem hattı silinemedi.';

  @override
  String get transformsAddStep => 'Adım ekle';

  @override
  String get transformsHiddenTitle => 'Gizli dönüşümler';

  @override
  String get transformsHiddenDescription =>
      'Gizli dönüşümler paletlerde ve önerilerde görünmez. Onları kullanan işlem hatları çalışmaya devam eder.';

  @override
  String get transformsHideTransform => 'Bir dönüşümü gizle';

  @override
  String get transformsShow => 'Göster';

  @override
  String get transformsSuggestionsTitle => 'Öneriler';

  @override
  String get transformsSuggestionsDescription =>
      'Bir palet açıldığında, metnin ilk 64 KiB\'ı aşağıdaki türler için denetlenir ve en fazla üç dönüşüm önerilir. Arka planda hiçbir şey çalışmaz.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web Token\'ları';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl => 'İzleme parametreli bağlantılar';

  @override
  String get transformsDetectorTimestamp => 'Zaman damgaları';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'İzleme parametreleri';

  @override
  String get transformsTrackingDescription =>
      'Yerleşik liste uygulamayla gelir ve yalnızca güncellemelerle değişir. Kaldırılacak veya tutulacak adlar ekleyin; bir alan adıyla sınırlamak için ad@alanadı yazın.';

  @override
  String get transformsTrackingRemoveLabel => 'Ayrıca kaldır';

  @override
  String get transformsTrackingAllowLabel => 'Her zaman tut';

  @override
  String get transformsTrackingPlaceholder =>
      'ad veya ad@alanadı, ardından Enter';

  @override
  String get quickAccessUnavailableTitle =>
      'Hızlı erişim durumu kullanılamıyor';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste klavye, kutucuk ve widget durumunu okuyamadı. Tekrar deneyin.';

  @override
  String get quickAccessActionFailedTitle => 'İşlem tamamlanamadı';

  @override
  String get quickAccessActionFailedMessage =>
      'Android istenen ekranı açmadı veya değişikliği kaydetmedi. Tekrar deneyin.';

  @override
  String get quickAccessKeyboardTitle => 'Öğe klavyesi';

  @override
  String get quickAccessKeyboardDescription =>
      'Sistem klavye ayarlarında CopyPaste öğeleri\'ni açın. Android, bir klavyenin yazdıklarınızı görebileceği konusunda uyarır: öğe paneli tuş vuruşlarını asla kaydetmez, diğer uygulamalara yazma tuşu yoktur ve yalnızca dokunduğunuz öğeleri ekler.';

  @override
  String get quickAccessKeyboardOn => 'Açık';

  @override
  String get quickAccessKeyboardOff => 'Kapalı';

  @override
  String get quickAccessOpenKeyboardSettings => 'Klavye ayarlarını aç';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'CopyPaste öğeleri klavye panelini açın.';

  @override
  String get quickAccessKeyboardKeywords =>
      'klavye giriş yöntemi ime panel etkinleştir';

  @override
  String get quickAccessSwitchKeyboardTitle => 'Öğe klavyesine geç';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'CopyPaste öğeleri\'ni geçerli klavye olarak seçin. Seçili kaldığı sürece her kopya kaydedilir.';

  @override
  String get quickAccessSwitchKeyboardAction => 'Klavye seç';

  @override
  String get quickAccessKeyboardCurrent => 'Geçerli klavye';

  @override
  String get quickAccessCaptureTierTitle => 'Yakalama modu';

  @override
  String get quickAccessCaptureTierDescription =>
      'Shizuku veya ADB erişimi olmadan kopyalar yalnızca öğe paneli açıldığında, panel seçili kaldığı sürece veya CopyPaste uygulaması, bir kutucuk, widget ya da Paylaş üzerinden kaydedilir.';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'Diğer uygulamalardaki kopyaların nasıl kaydedildiğini görün.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'yakalama düzey tam panel manuel shizuku toplama';

  @override
  String get quickAccessTierFull => 'Tam: her kopya arka planda kaydedilir';

  @override
  String get quickAccessTierPanelCollecting => 'Panel seçili: toplanıyor';

  @override
  String get quickAccessTierPanelCatchUp => 'Panel modu: açılışta yakalama';

  @override
  String get quickAccessTierManual =>
      'Manuel: uygulama, kutucuk, widget veya Paylaş üzerinden kaydedin';

  @override
  String get quickAccessTierPaused => 'Yakalama duraklatıldı';

  @override
  String get quickAccessAutoReturnTitle =>
      'Ekledikten sonra önceki klavyeye dön';

  @override
  String get quickAccessAutoReturnDescription =>
      'Tek bir eklemeden sonra geri döner. Kuyruk sekmesini veya öğe işlemlerini kullanırken panel açık kalır. Kopya toplamaya devam etmek için kapatın.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'geri dön önceki klavye otomatik ekle';

  @override
  String get quickAccessFocusSearchTitle => 'Panel açılınca aramaya odaklan';

  @override
  String get quickAccessFocusSearchDescription =>
      'Paneli arama alanında açar. Kapalıyken öğeleriniz tek dokunuş uzaklıktadır.';

  @override
  String get quickAccessTilesTitle => 'Hızlı Ayarlar kutucukları';

  @override
  String get quickAccessTilesSearchDescription =>
      'Panoyu kaydet ve Pano yakalama kutucuklarını ekleyin.';

  @override
  String get quickAccessTilesKeywords =>
      'hızlı ayarlar kutucuk bildirim paneli kaydet duraklat';

  @override
  String get quickAccessTileInstructions =>
      'Ekranın üstünden iki kez aşağı kaydırın, Düzenle\'ye dokunun ve bir CopyPaste kutucuğunu Hızlı Ayarlar\'a sürükleyin.';

  @override
  String get quickAccessSaveTileTitle => 'Panoyu kaydet kutucuğu';

  @override
  String get quickAccessSaveTileDescription =>
      'Hızlı Ayarlar\'dan panodakini kaydeder.';

  @override
  String get quickAccessCaptureTileTitle => 'Pano yakalama kutucuğu';

  @override
  String get quickAccessCaptureTileDescription =>
      'Hızlı Ayarlar\'dan yakalamayı duraklatır ve sürdürür.';

  @override
  String get quickAccessAddTile => 'Kutucuk ekle';

  @override
  String get quickAccessTileAdded => 'Hızlı Ayarlar\'a eklendi';

  @override
  String get quickAccessWidgetTitle => 'Son öğeler widget\'ı';

  @override
  String get quickAccessWidgetDescription =>
      'En son öğelerinizi ana ekranda gösterir. Kopyalamak için birine dokunun.';

  @override
  String get quickAccessWidgetInstructions =>
      'Ana ekrana dokunup basılı tutun, Widget\'lar\'a dokunun ve CopyPaste\'teki Son öğeler\'i sürükleyin.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count widget eklendi',
      zero: 'Eklenmedi',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Widget ekle';

  @override
  String get quickAccessWidgetKeywords => 'ana ekran widget başlatıcı son';

  @override
  String snippetsAbbreviationBadge(String abbreviation) {
    return 'Kısaltma $abbreviation';
  }

  @override
  String get snippetsAbbreviationLabel => 'Kısaltma';

  @override
  String get snippetsAbbreviationPlaceholder => 'İsteğe bağlı, örneğin imza';

  @override
  String snippetsAbbreviationTaken(String title) {
    return 'Zaten “$title” tarafından kullanılıyor';
  }

  @override
  String snippetsAbbreviationTooLong(int max) {
    return 'En fazla $max karakter kullanın.';
  }

  @override
  String get snippetsAbbreviationWhitespace =>
      'Kısaltma boşluk veya denetim karakteri içeremez.';

  @override
  String snippetsAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Eşitlemeden sonra $count parçacığın incelenmesi gerekiyor',
      zero: 'İncelenecek parçacık yok',
    );
    return '$_temp0';
  }

  @override
  String get snippetsBodyLabel => 'Metin';

  @override
  String get snippetsBodyPlaceholder => 'Parçacık metnini yazın';

  @override
  String get snippetsClearSelection => 'Seçimi temizle';

  @override
  String get snippetsClipTooLarge =>
      'Düzenlemek için çok büyük. Bu öğe salt okunur açılır.';

  @override
  String snippetsCollectionAccent(String accent) {
    String _temp0 = intl.Intl.selectLogic(accent, {
      'neutral': 'Nötr',
      'blue': 'Mavi',
      'green': 'Yeşil',
      'amber': 'Kehribar',
      'red': 'Kırmızı',
      'other': 'Nötr',
    });
    return '$_temp0';
  }

  @override
  String get snippetsCollectionAccentLabel => 'Vurgu rengi';

  @override
  String get snippetsCollectionIcon => 'Simge';

  @override
  String get snippetsCollectionLabel => 'Koleksiyon';

  @override
  String get snippetsCollectionName => 'Ad';

  @override
  String get snippetsCollectionNameTaken => 'Bu adda bir koleksiyon zaten var.';

  @override
  String snippetsCollectionNameTooLong(int max) {
    return 'En fazla $max karakter kullanın.';
  }

  @override
  String get snippetsConcealedBody => 'Bu parçacık gizli.';

  @override
  String get snippetsConcealedConfirmAction => 'Devam et';

  @override
  String get snippetsConcealedConfirmMessage =>
      'Bu cihazda sizi doğrulayacak parola, PIN veya biyometri yok. Yalnızca cihaz sizinse devam edin.';

  @override
  String get snippetsConcealedConfirmTitle => 'Siz olduğunuzu onaylayın';

  @override
  String get snippetsConcealedPreview => 'Gizli parçacık';

  @override
  String get snippetsConcealedSwitch => 'Gizli';

  @override
  String get snippetsConcealedTitle => 'Gizli parçacık';

  @override
  String get snippetsConflictBadge => 'Çakışma kopyası';

  @override
  String get snippetsCopied => 'Kopyalandı';

  @override
  String get snippetsCreate => 'Parçacık oluştur';

  @override
  String snippetsCreateNamed(String query) {
    return '“$query” parçacığını oluştur';
  }

  @override
  String get snippetsDeleteCollection => 'Koleksiyonu sil';

  @override
  String get snippetsDeleteCollectionMove =>
      'Parçacıklarını Koleksiyonsuz bölümüne taşı';

  @override
  String snippetsDeleteCollectionTitle(String name) {
    return '“$name” silinsin mi?';
  }

  @override
  String get snippetsDeleteCollectionWithSnippets => 'Parçacıklarını da sil';

  @override
  String get snippetsDeletedMessage => 'Son silinenler bölümüne taşındı.';

  @override
  String snippetsDeletedTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count parçacık silindi',
      one: 'Parçacık silindi',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDestination => 'Kaydedilecek yer';

  @override
  String snippetsDestinationName(String destination) {
    String _temp0 = intl.Intl.selectLogic(destination, {
      'historyClip': 'Geçmiş öğesi',
      'snippet': 'Parçacık',
      'other': 'Geçmiş öğesi',
    });
    return '$_temp0';
  }

  @override
  String get snippetsDiscard => 'Vazgeç';

  @override
  String snippetsDragging(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count parçacık',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDuplicateAbbreviation =>
      'Başka bir parçacık eşitleme sırasında aynı kısaltmayı aldı. Birini değiştirin.';

  @override
  String get snippetsDuplicateBadge => 'Yinelenen kısaltma';

  @override
  String get snippetsEditClip => 'Öğeyi düzenle';

  @override
  String get snippetsEditCollection => 'Koleksiyonu düzenle';

  @override
  String get snippetsEditSnippet => 'Parçacığı düzenle';

  @override
  String get snippetsEditTab => 'Düzenle';

  @override
  String get snippetsEmptyMessage =>
      'Bir parçacığa kısaltma verin, sonra bunu Quick Paste\'te yazın.';

  @override
  String get snippetsEmptyTitle => 'Henüz parçacık yok';

  @override
  String get snippetsEnableTemplate => 'Aç';

  @override
  String get snippetsEnableTemplateMessage =>
      'Yer tutucular yalnızca şablonlarda çalışır. Zaten yer tutucuya benzeyen metin de doldurulur.';

  @override
  String get snippetsEnableTemplateTitle => 'Şablon açılsın mı?';

  @override
  String get snippetsExport => 'Dışa aktar';

  @override
  String get snippetsExportCollection => 'Koleksiyonu dışa aktar';

  @override
  String get snippetsExportConcealedConfirm =>
      'Gizli parçacıkları okunabilir dosyaya ekle';

  @override
  String snippetsExportCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count parçacık yazıldı',
      zero: 'Hiç parçacık yazılmadı',
    );
    return '$_temp0';
  }

  @override
  String get snippetsExportDone => 'Parçacıklar dışa aktarıldı';

  @override
  String get snippetsExportEncrypt => 'Parola ifadesiyle şifrele';

  @override
  String get snippetsExportFailed => 'Dışa aktarma başarısız';

  @override
  String get snippetsExportPlainConfirm =>
      'Dosyaya sahip olan herkesin onu okuyabileceğini anlıyorum';

  @override
  String snippetsExportScope(String scope, int count) {
    String _temp0 = intl.Intl.selectLogic(scope, {
      'all': 'Tüm parçacıklar',
      'collections': 'Bu koleksiyon',
      'snippets': 'Seçili parçacıklar ($count)',
      'other': 'Tüm parçacıklar',
    });
    return '$_temp0';
  }

  @override
  String get snippetsExportTitle => 'Parçacıkları dışa aktar';

  @override
  String get snippetsFieldBuilderTitle => 'Alan ekle';

  @override
  String get snippetsFieldDefault => 'Varsayılan değer';

  @override
  String get snippetsFieldName => 'Ad';

  @override
  String get snippetsFieldOptions => 'Virgülle ayrılmış seçenekler';

  @override
  String get snippetsFieldRequired => 'Bu alanı doldurun.';

  @override
  String get snippetsFieldType => 'Tür';

  @override
  String snippetsFieldTypeName(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'text': 'Tek satır',
      'multiline': 'Birden çok satır',
      'choice': 'Seçim',
      'other': 'Tek satır',
    });
    return '$_temp0';
  }

  @override
  String get snippetsFormattingNotice =>
      'Kaydettiğinizde biçimlendirme kaldırılır.';

  @override
  String get snippetsGoToSnippet => 'Parçacığa git';

  @override
  String get snippetsImport => 'İçe aktar';

  @override
  String get snippetsImportAbbreviationPolicy => 'Kısaltma zaten kullanılıyor';

  @override
  String snippetsImportAbbreviationPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'importWithout': 'Kısaltmasız içe aktar',
      'renameWithSuffix': 'Kısaltmaya sayı ekle',
      'skip': 'Atla',
      'other': 'Kısaltmasız içe aktar',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportCollectionPolicy => 'Aynı adlı koleksiyon';

  @override
  String snippetsImportCollectionPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'merge': 'Mevcut olana ekle',
      'createNew': 'Yeni bir tane oluştur',
      'other': 'Mevcut olana ekle',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportConvert => 'Bilinen yer tutucuları dönüştür';

  @override
  String snippetsImportCounts(
    int added,
    int identical,
    int updated,
    int conflicts,
    int invalid,
  ) {
    return 'Yeni: $added. Aynı, atlandı: $identical. Değişmiş: $updated. Kısaltma çakışmaları: $conflicts. Geçersiz: $invalid.';
  }

  @override
  String get snippetsImportDone => 'Parçacıklar içe aktarıldı';

  @override
  String get snippetsImportFailed => 'İçe aktarma başarısız';

  @override
  String get snippetsImportFileType => 'Parçacık dosyaları';

  @override
  String get snippetsImportFormat => 'Biçim';

  @override
  String snippetsImportFormatName(String format) {
    String _temp0 = intl.Intl.selectLogic(format, {
      'bundle': 'CopyPaste paketi',
      'json': 'JSON listesi',
      'csv': 'CSV',
      'other': 'CopyPaste paketi',
    });
    return '$_temp0';
  }

  @override
  String snippetsImportInvalidRow(int row, String reason) {
    return 'Satır $row: $reason';
  }

  @override
  String get snippetsImportInvalidRows => 'İçe aktarılmayacak satırlar';

  @override
  String get snippetsImportPassphraseHint => 'Dosya şifreliyse parola ifadesi';

  @override
  String get snippetsImportPreview => 'Önizle';

  @override
  String snippetsImportReport(
    int inserted,
    int updated,
    int skipped,
    int flagged,
    int unrecognized,
  ) {
    return 'Eklenen: $inserted. Güncellenen: $updated. Atlanan: $skipped. İncelenecek kısaltmalar: $flagged. Tanınmayan yer tutucular: $unrecognized.';
  }

  @override
  String get snippetsImportSameIdPolicy => 'Aynı parçacık, farklı içerik';

  @override
  String snippetsImportSameIdPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'keepBoth': 'İkisini de tut',
      'replace': 'Mevcut olanı değiştir',
      'skip': 'Atla',
      'other': 'İkisini de tut',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportTitle => 'Parçacıkları içe aktar';

  @override
  String get snippetsInsert => 'Ekle';

  @override
  String get snippetsInsertPlaceholder => 'Yer tutucu ekle';

  @override
  String get snippetsKeepReferences => 'Olduğu gibi bırak';

  @override
  String get snippetsLibraryDescription =>
      'Parçacıklara ve koleksiyonlara göz atın, bunları düzenleyin.';

  @override
  String get snippetsLibraryKeywords =>
      'parçacıklar şablonlar kısaltmalar koleksiyonlar metin snippet';

  @override
  String get snippetsLibraryTitle => 'Parçacık kitaplığı';

  @override
  String snippetsLint(String code, String detail) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'unclosed': 'Bu yer tutucu hiç kapatılmıyor.',
      'malformed': 'Bu yer tutucu okunamıyor.',
      'unknownPlaceholder':
          'Bilinmeyen yer tutucu “$detail”. Yazıldığı gibi yapıştırılır.',
      'unknownParameter': 'Bilinmeyen seçenek “$detail”.',
      'duplicateParameter': '“$detail” seçeneği iki kez ayarlanmış.',
      'missingParameter': '“$detail” seçeneği eksik.',
      'invalidValue': '“$detail” seçeneğinin değeri geçersiz.',
      'conflictingParameters': 'Bu seçenekler birlikte kullanılamaz.',
      'unknownLocale': 'Bilinmeyen dil.',
      'unknownTimeZone': 'Bilinmeyen saat dilimi.',
      'invalidPattern': 'Geçersiz tarih kalıbı.',
      'dateOutOfRange': 'Tarih aralık dışında.',
      'unknownModifier': 'Bilinmeyen değiştirici “$detail”.',
      'modifierFailed': '“$detail” değiştiricisi uygulanamadı.',
      'emptyClipboard': '$detail. pano öğesi boş.',
      'extraCursor': 'Yalnızca ilk imleç yer tutucusu kullanılır.',
      'nestedCursor': 'Eklenen bir parçacıktaki imleç yok sayılır.',
      'tooManyFields': '“$detail” alanı 10 alan sınırını aşıyor.',
      'conflictingField':
          '“$detail” alanı farklı ayarlarla iki kez tanımlanmış.',
      'choiceDefaultNotOption':
          '“$detail” alanının varsayılan değeri seçeneklerinden biri değil.',
      'cycle': 'Parçacıklar birbirini döngü içinde ekliyor: $detail.',
      'depthExceeded': 'Parçacıklar 3 düzeyden daha derine ekleniyor.',
      'nestingLimit': 'Çok fazla eklenmiş parçacık var.',
      'unknownSnippet': 'Hiçbir parçacıkta bu kısaltma yok.',
      'containsPlaceholders':
          'Metin yer tutucular içeriyor. Doldurmak için Şablon\'u açın.',
      'other': 'Bu yer tutucuda bir sorun var.',
    });
    return '$_temp0';
  }

  @override
  String get snippetsLoadFailed => 'Parçacıklar yüklenemedi';

  @override
  String get snippetsLoadMore => 'Daha fazla yükle';

  @override
  String get snippetsMerge => 'Birleştir';

  @override
  String get snippetsMergeCollection => 'Başka bir koleksiyonla birleştir';

  @override
  String snippetsMergeCollectionTitle(String name) {
    return '“$name” koleksiyonunu şununla birleştir';
  }

  @override
  String get snippetsMoveTo => 'Şuraya taşı';

  @override
  String get snippetsNewCollection => 'Yeni koleksiyon';

  @override
  String get snippetsNewSnippet => 'Yeni parçacık';

  @override
  String get snippetsNewText => 'Yeni metin';

  @override
  String snippetsNoMatch(String query) {
    return '“$query” ile eşleşen parçacık yok';
  }

  @override
  String get snippetsPassphraseConfirm => 'Parola ifadesini tekrarlayın';

  @override
  String snippetsPassphraseHint(int min) {
    return 'Parola ifadesi, en az $min karakter';
  }

  @override
  String get snippetsPassphraseLabel => 'Parola ifadesi';

  @override
  String get snippetsPassphraseNoRecovery =>
      'Unutulan parola ifadesi kurtarılamaz.';

  @override
  String snippetsPassphraseStrength(String score) {
    String _temp0 = intl.Intl.selectLogic(score, {
      'tooShort': 'Çok kısa',
      'weak': 'Zayıf',
      'fair': 'Orta',
      'good': 'İyi',
      'other': 'Güçlü',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPinnedSwitch => 'Sabitlendi';

  @override
  String get snippetsPlaceholderGroup => 'Yer tutucular';

  @override
  String snippetsPlaceholderKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'today': 'Tarih',
      'dateShort': 'Tarih, kısa',
      'dateLong': 'Tarih, uzun',
      'dateIso': 'Tarih, ISO 8601',
      'dateNextWeek': 'Bir hafta sonraki tarih',
      'now': 'Saat',
      'dateTime': 'Tarih ve saat',
      'clipboard': 'Pano',
      'clipboardPrevious': 'Önceki pano öğesi',
      'field': 'Doldurulacak alan…',
      'cursor': 'İmleç konumu',
      'uuid': 'UUID',
      'random': 'Rastgele sayı',
      'upper': 'Değiştirici: büyük harf',
      'lower': 'Değiştirici: küçük harf',
      'trim': 'Değiştirici: boşlukları kırp',
      'other': 'Yer tutucu',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPlaceholderNestedGroup => 'Başka bir parçacık ekle';

  @override
  String get snippetsPlaceholderNoMatch => 'Eşleşen yer tutucu yok';

  @override
  String get snippetsPreviewEmpty => 'Önizlenecek bir şey yok';

  @override
  String get snippetsPreviewTab => 'Önizleme';

  @override
  String get snippetsPurge => 'Kalıcı olarak sil';

  @override
  String get snippetsPurgeMessage =>
      'Tüm cihazlarınızdan silinecekler. Bu işlem geri alınamaz.';

  @override
  String snippetsPurgeTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count parçacık kalıcı olarak silinsin mi?',
      one: 'Parçacık kalıcı olarak silinsin mi?',
    );
    return '$_temp0';
  }

  @override
  String get snippetsRecentlyDeletedEmpty => 'Son silinenler boş';

  @override
  String get snippetsRecordPastedDescription =>
      'Yapıştırılan parçacık metnini Geçmiş\'e ekler. Gizli parçacıklar asla kaydedilmez.';

  @override
  String get snippetsRecordPastedKeywords =>
      'geçmiş kaydet yapıştırılan parçacıklar';

  @override
  String get snippetsRecordPastedTitle =>
      'Yapıştırılan parçacıkları Geçmiş\'e kaydet';

  @override
  String get snippetsReplaceOriginal => 'Orijinali değiştir';

  @override
  String get snippetsReplaceOriginalHint =>
      'Kaydettikten sonra orijinal öğeyi tüm cihazlardan siler. Kapalıyken ikisi de kalır.';

  @override
  String get snippetsRequireAuthDescription =>
      'Gizli bir parçacık kullanılmadan veya gösterilmeden önce siz olduğunuzu doğrulayın. Her cihaz ayrı sorar.';

  @override
  String get snippetsRequireAuthKeywords =>
      'gizli parola biyometri parmak izi yüz kilit doğrula';

  @override
  String get snippetsRequireAuthTitle =>
      'Gizli parçacıkları kullanmadan önce doğrula';

  @override
  String get snippetsRestore => 'Geri yükle';

  @override
  String get snippetsSaveBlockEmpty => 'Kaydetmek için metin ekleyin.';

  @override
  String snippetsSaveBlockTitle(int max) {
    return 'Başlık $max karakterden uzun.';
  }

  @override
  String get snippetsSaveBlockTooLarge => 'Metin 64 KB\'tan büyük.';

  @override
  String get snippetsSaveFailed => 'Kaydedilemedi';

  @override
  String get snippetsScopeAll => 'Tümü';

  @override
  String get snippetsScopeEmpty => 'Burada parçacık yok';

  @override
  String get snippetsScopePinned => 'Sabitlenenler';

  @override
  String get snippetsScopeRecentlyDeleted => 'Son silinenler';

  @override
  String get snippetsSearchPlaceholder => 'Parçacıklarda ara';

  @override
  String snippetsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count seçildi',
    );
    return '$_temp0';
  }

  @override
  String get snippetsShow => 'Göster';

  @override
  String get snippetsShowAll => 'Tümünü göster';

  @override
  String get snippetsShowAttention => 'İncele';

  @override
  String snippetsSoftLimit(int count) {
    return '$count parçacığınız var. En fazla 20.000 parçacığa izin verilir.';
  }

  @override
  String get snippetsTemplateBadge => 'Şablon';

  @override
  String get snippetsTemplateSwitch => 'Şablon';

  @override
  String get snippetsTitleLabel => 'Başlık';

  @override
  String get snippetsTitlePlaceholder => 'İsteğe bağlı';

  @override
  String get snippetsTransferDescription =>
      'Parçacıkları uygulamalar arasında taşıyın veya yedek tutun. Dışa aktarmalar varsayılan olarak şifrelenir.';

  @override
  String get snippetsTransferKeywords =>
      'içe aktar dışa aktar yedek paket json csv parola';

  @override
  String get snippetsTransferTitle => 'İçe ve dışa aktar';

  @override
  String get snippetsUndo => 'Geri al';

  @override
  String get snippetsUnfiled => 'Koleksiyonsuz';

  @override
  String snippetsUnfilledWarning(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count yer tutucu yazıldığı gibi yapıştırılır',
    );
    return '$_temp0';
  }

  @override
  String get snippetsUnpin => 'Sabitlemeyi kaldır';

  @override
  String get snippetsUnsavedMessage =>
      'Kapatmadan önce değişiklikler kaydedilsin mi?';

  @override
  String get snippetsUnsavedTitle => 'Kaydedilmemiş değişiklikler';

  @override
  String get snippetsUntitled => 'Başlıksız parçacık';

  @override
  String get snippetsUpdateReferences => 'Güncelle';

  @override
  String snippetsUpdateReferencesMessage(String from, String to) {
    return 'Diğer şablonlar “$from” ekliyor olabilir. “$to” kullanacak şekilde güncellensinler mi?';
  }

  @override
  String get snippetsUpdateReferencesTitle => 'Başvurular güncellensin mi?';

  @override
  String get snippetsUseFailed => 'Parçacık kullanılamadı';

  @override
  String get rulesLoadFailed => 'İçerik kuralları yüklenemedi.';

  @override
  String get rulesNewRule => 'Yeni kural';

  @override
  String get rulesEditRule => 'Kuralı düzenle';

  @override
  String get rulesUntitled => 'Adsız kural';

  @override
  String get rulesPrivacyDefaultsTitle => 'Gizlilik varsayılanları';

  @override
  String get rulesPrivacyDefaultsDescription =>
      'Bu gizlilik ayarları, herhangi bir kural çalışmadan önce bu cihazdaki her kopyayı korur. Aşağıdaki yerleşik kurallar cihazlarınızla eşitlenir.';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'gizlilik gizlenmiş geçici hariç uygulamalar yerleşik kimlik bilgileri parolalar kartlar';

  @override
  String rulesPrivacyState(String setting, String state) {
    return '$setting: $state';
  }

  @override
  String rulesExcludedApps(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count uygulama',
      zero: 'yok',
    );
    return '$_temp0';
  }

  @override
  String get rulesOpenPrivacy => 'Gizliliği aç';

  @override
  String get rulesMyRulesTitle => 'Kurallarım';

  @override
  String get rulesMyRulesDescription =>
      'Yeni kopyalarda, yerleşik kurallardan sonra yukarıdan aşağıya çalışır ve cihazlarınızla eşitlenir.';

  @override
  String get rulesMyRulesKeywords =>
      'kurallar otomasyon etiket atla gizli süre sil desen regex';

  @override
  String get rulesEmptyTitle => 'Henüz kural yok';

  @override
  String get rulesEmptyMessage =>
      'Bir kural yeni kopyaları otomatik olarak etiketleyebilir, koruyabilir, süresini doldurabilir veya atlayabilir.';

  @override
  String get rulesDeletedTitle => 'Son silinenler';

  @override
  String get rulesDeletedDescription =>
      'Silinen kurallar 30 gün boyunca geri yüklenebilir.';

  @override
  String rulesDeletedAt(String time) {
    return 'Silinme: $time';
  }

  @override
  String get rulesRestore => 'Geri yükle';

  @override
  String get rulesDeletedToast => 'Kural silindi';

  @override
  String get rulesDeletedToastMessage =>
      '30 gün boyunca geri yükleyebilirsiniz.';

  @override
  String get rulesUndo => 'Geri al';

  @override
  String rulesConflictMessage(String rule, String device) {
    return '$device cihazında “$rule” için yapılan eşzamanlı düzenleme daha yeni bir düzenlemeyle değiştirildi.';
  }

  @override
  String get rulesAnotherDevice => 'başka bir cihaz';

  @override
  String get rulesRestoreConflict => 'O sürümü kullan';

  @override
  String get rulesSkipLogTitle => 'Son atlananlar';

  @override
  String get rulesSkipLogDescription =>
      'CopyPaste başladığından beri kurallarınızın bu cihazda kaydetmediği kopyalar. Yalnızca zaman, uygulama ve kural saklanır.';

  @override
  String get rulesSkipLogKeywords => 'atlanan yok sayılan kaydedilmeyen günlük';

  @override
  String get rulesSkipLogShow => 'Göster';

  @override
  String get rulesSkipLogEmpty => 'Atlanan kopya yok.';

  @override
  String get rulesUnknownApp => 'Bilinmeyen uygulama';

  @override
  String get rulesMatchesEverything => 'herhangi bir kopya';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions ise → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary, sonra dur';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': ' veya ',
      'other': ' ve ',
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
    return 'Çalıştığı cihazlar: $devices';
  }

  @override
  String get rulesRunsOnAll => 'Tüm cihazlarda çalışır';

  @override
  String rulesMatchCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Burada $count kez eşleşti',
    );
    return '$_temp0';
  }

  @override
  String rulesLastMatched(String time) {
    return 'son $time';
  }

  @override
  String rulesNewFromDevice(String device) {
    return '$device cihazından yeni';
  }

  @override
  String get rulesNeedsUpdateBadge => 'Güncelleme gerekli';

  @override
  String get rulesUpdateRequired =>
      'Bu kuralı çalıştırmak için CopyPaste’i güncelleyin.';

  @override
  String get rulesErrorBadge => 'Hata';

  @override
  String rulesErrorHint(String problem) {
    return '$problem Düzeltilene kadar bu kuralın eşleşebileceği kopyalar gizli olarak kaydedilir ve yapay zekâ araçlarından gizlenir.';
  }

  @override
  String get rulesReorder => 'Yeniden sıralamak için sürükleyin';

  @override
  String get rulesMoreActions => 'Diğer işlemler';

  @override
  String get rulesEdit => 'Düzenle';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': 'Yukarı taşı',
      'down': 'Aşağı taşı',
      'top': 'En üste taşı',
      'other': 'En alta taşı',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => 'Mevcut öğelere uygula…';

  @override
  String get rulesResetBuiltin => 'Varsayılana sıfırla';

  @override
  String get rulesNameLabel => 'Ad';

  @override
  String get rulesNamePlaceholder => 'Örneğin, İş bağlantıları';

  @override
  String get rulesWhenTitle => 'Ne zaman';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'Herhangi bir koşul eşleşir',
      'other': 'Tüm koşullar eşleşir',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions => 'Koşul yok: Bu kural her kopyaya uygulanır.';

  @override
  String get rulesAddCondition => 'Koşul ekle';

  @override
  String get rulesRemoveCondition => 'Koşulu kaldır';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'Metin',
      'content_type': 'İçerik türü',
      'detected_kind': 'Algılanan tür',
      'source_app': 'Kaynak uygulama',
      'size': 'Boyut',
      'privacy_marker': 'Gizlilik işareti',
      'capture_origin': 'Kaynak',
      'other': 'Bilinmeyen koşul',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'içerir',
      'equals': 'eşittir',
      'starts_with': 'ile başlar',
      'ends_with': 'ile biter',
      'matches_regex': 'desenle eşleşir',
      'other': 'eşleşir',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'içermez',
      'equals': 'eşit değildir',
      'starts_with': 'ile başlamaz',
      'ends_with': 'ile bitmez',
      'matches_regex': 'desenle eşleşmez',
      'other': 'eşleşmez',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'şu değil',
      'other': 'şu',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'şundan küçük',
      'other': 'şundan büyük',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'şundan küçük değil',
      'other': 'şundan büyük değil',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'satır',
      'bytes': 'bayt',
      'other': 'karakter',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'satır',
      'bytes': 'bayt',
      'other': 'karakter',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'Düz metin',
      'rich_text': 'Zengin metin',
      'image': 'Görsel',
      'files': 'Dosyalar',
      'color': 'Renk',
      'link': 'Bağlantı',
      'other': 'Diğer',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'Parola veya gizli bilgi',
      'url': 'Web adresi',
      'email': 'E-posta adresi',
      'phone': 'Telefon numarası',
      'code': 'Kod',
      'color': 'Renk değeri',
      'json': 'JSON',
      'file_path': 'Dosya yolu',
      'number': 'Sayı',
      'one_time_code': 'Tek kullanımlık kod',
      'payment_card': 'Kart numarası',
      'other': 'Diğer',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': 'Gizlenmiş',
      'transient': 'Geçici',
      'auto_generated': 'Otomatik oluşturulmuş',
      'excluded_from_history': 'Geçmişten hariç',
      'other': 'Diğer',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'Bu cihazda kopyalandı',
      'received': 'Başka bir cihazdan alındı',
      'created_in_app': 'CopyPaste’te oluşturuldu',
      'other': 'Diğer',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => 'Aranacak metin';

  @override
  String get rulesPatternPlaceholder => 'Desen, örneğin ^ticket-[0-9]+';

  @override
  String get rulesPatternHint =>
      'Desenler metnin ilk 1 MiB’ını denetler. İleriye bakma, geriye bakma ve geri başvurular desteklenmez.';

  @override
  String get rulesCaseSensitive => 'Büyük/küçük harf duyarlı';

  @override
  String get rulesChooseValues => 'Seç';

  @override
  String get rulesUnknownConditionNotice =>
      'Bu koşul CopyPaste’in daha yeni bir sürümünden geliyor.';

  @override
  String get rulesSourceAppNotice =>
      'Android kaynak uygulamayı yalnızca CopyPaste klavyesiyle yapılan kopyalarda bilir. Android’deki diğer kopyalar bu koşulla hiçbir zaman eşleşmez.';

  @override
  String get rulesPrivacyMarkerNotice =>
      'Her sistem farklı işaretler bildirir. Sistemin bildirmediği bir işaret hiçbir zaman eşleşmez.';

  @override
  String get rulesThenTitle => 'O zaman';

  @override
  String get rulesNoActions => 'En az bir işlem ekleyin.';

  @override
  String get rulesAddAction => 'İşlem ekle';

  @override
  String get rulesRemoveAction => 'İşlemi kaldır';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': 'Kaydetme',
      'mark_secret': 'Gizli olarak işaretle',
      'hide_from_ai': 'Yapay zekâ araçlarından gizle',
      'delete_after': 'Şu süre sonra sil',
      'add_tags': 'Etiket ekle',
      'transform': 'Dönüştür',
      'other': 'Bilinmeyen işlem',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone': '“Kaydetme” diğer işlemlerle birleştirilemez.',
      'present': 'Bu kuralda bu işlem zaten var.',
      'other': 'Bir kuralda en fazla 8 işlem olabilir.',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'Bu işlem CopyPaste’in daha yeni bir sürümünden geliyor.';

  @override
  String get rulesDontSaveWarning =>
      'Eşleşen kopyalar hiçbir zaman kaydedilmez, dizine eklenmez veya eşitlenmez.';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dakika',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count saat',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count gün',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry =>
      'O sırada pano hâlâ bu kopyayı içeriyorsa panoyu da temizle. Android’de bu, CopyPaste bir sonraki çalıştığında olur.';

  @override
  String get rulesNoTags => 'Burada kullanmak için Geçmiş’te etiket oluşturun.';

  @override
  String get rulesNoTransforms => 'Kullanılabilir dönüşüm yok.';

  @override
  String get rulesReplaceClipboard => 'Panoyu da sonuçla değiştir';

  @override
  String get rulesOptionsTitle => 'Seçenekler';

  @override
  String get rulesEnabledSwitch => 'Etkin';

  @override
  String get rulesStopProcessing =>
      'Bu kural eşleşirse sonraki kuralları işleme';

  @override
  String get rulesRunsOnLabel => 'Çalıştığı yer';

  @override
  String get rulesAllDevices => 'Tüm cihazlar';

  @override
  String get rulesChosenDevices => 'Seçilen cihazlar';

  @override
  String get rulesTestTitle => 'Test';

  @override
  String get rulesTestDescription =>
      'Kuralı örnek metinle deneyin. Örnek hiçbir zaman kaydedilmez veya eşitlenmez.';

  @override
  String get rulesSamplePlaceholder => 'Örnek metni yapıştırın veya yazın';

  @override
  String get rulesTestFullSet => 'Tüm kurallarla birlikte test et';

  @override
  String get rulesRunTest => 'Testi çalıştır';

  @override
  String get rulesTestFailed => 'Test çalıştırılamadı.';

  @override
  String get rulesPreviewHistory => 'Geçmişte önizle';

  @override
  String rulesSaveBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'read_only': 'Bu kuralı düzenlemek için CopyPaste’i güncelleyin.',
      'name_missing': 'Kurala bir ad verin.',
      'name_too_long': 'En fazla 128 karakterlik bir ad kullanın.',
      'no_actions': 'En az bir işlem ekleyin.',
      'other': 'İşaretli alanları düzeltin.',
    });
    return '$_temp0';
  }

  @override
  String get rulesSaveFailed => 'Kural kaydedilemedi.';

  @override
  String get rulesReadOnlyNotice =>
      'Bu kuralı CopyPaste’in daha yeni bir sürümü yazdı. Düzenlemek veya çalıştırmak için CopyPaste’i güncelleyin.';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'Kural okunamadı.',
      'unsupported_schema':
          'Bu kural CopyPaste’in daha yeni bir sürümünü gerektiriyor.',
      'unknown_condition_type':
          'Bu koşul CopyPaste’in daha yeni bir sürümünü gerektiriyor.',
      'unknown_action_type':
          'Bu işlem CopyPaste’in daha yeni bir sürümünü gerektiriyor.',
      'too_many_conditions': 'Bir kuralda en fazla 20 koşul olabilir.',
      'too_many_actions': 'Bir kuralda en fazla 8 işlem olabilir.',
      'too_many_rules': 'Yalnızca ilk 200 kural çalışır.',
      'no_actions': 'En az bir işlem ekleyin.',
      'conflicting_actions': '“Kaydetme” diğer işlemlerle birleştirilemez.',
      'duplicate_action': 'Her işlem yalnızca bir kez kullanılabilir.',
      'empty_value': 'Bir değer girin.',
      'empty_list': 'En az bir değer seçin.',
      'too_many_values': 'En fazla 64 değer seçin.',
      'invalid_value': 'Bu değer geçerli değil.',
      'pattern_too_long': 'Desenler en fazla 2.000 karakter olabilir.',
      'unsupported_look_around':
          'İleriye ve geriye bakma desteklenmez. Bunun yerine “Tüm koşullar eşleşir” ile iki koşul kullanın.',
      'unsupported_backreference': 'Geri başvurular desteklenmez.',
      'pattern_too_large': 'Bu desen çok karmaşık. Tekrarlarını basitleştirin.',
      'invalid_pattern': 'Bu desen geçerli değil.',
      'invalid_duration': '1 dakika ile 90 gün arasında bir süre seçin.',
      'invalid_tags': 'En az bir etiket seçin.',
      'invalid_transform': 'Bir dönüşüm seçin.',
      'other': 'Bu kuralda bir sorun var.',
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
      'matched': 'eşleşti',
      'not_matched': 'eşleşmedi',
      'unknown': 'bilinmiyor',
      'skipped': 'çalıştırılmadı',
      'other': 'hata',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return 'Koşul $number';
  }

  @override
  String get rulesTraceNoRules => 'Hiçbir kural değerlendirilmedi.';

  @override
  String get rulesTraceStopped => 'Sonraki kurallar çalışmadı.';

  @override
  String get rulesTraceUnknownReason =>
      'Bu kopya için bilinmiyor; bu yüzden koşul hiçbir durumda eşleşmez.';

  @override
  String get rulesTraceRuleError =>
      'Bu kural burada çalışamadı; bu yüzden eşleşen kopyalar bunun yerine korunur.';

  @override
  String get rulesTraceOutcomeTitle => 'Sonuç';

  @override
  String get rulesTraceNoEffect => 'Hiçbir kural bu kopyayı değiştirmez.';

  @override
  String get rulesTraceSkipped => 'Kaydedilmez';

  @override
  String get rulesTraceSecret => 'Gizli olarak kaydedilir';

  @override
  String get rulesTraceHidden => 'Yapay zekâ araçlarından gizlenir';

  @override
  String rulesTraceExpires(String duration) {
    return '$duration sonra silinir';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'Etiketler: $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return 'Dönüştürme: $names';
  }

  @override
  String get rulesTraceOutput => 'Dönüştürülmüş metin';

  @override
  String get rulesPreviewTitle => 'Mevcut öğelere uygula';

  @override
  String rulesPreviewRunning(int count) {
    return 'Son $count öğe denetleniyor…';
  }

  @override
  String get rulesPreviewNone => 'Bu kuralla eşleşen yeni öğe yok.';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe eşleşiyor',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe silinir',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe gizli olarak işaretlenir',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe yapay zekâ araçlarından gizlenir',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğeye silinme zamanı atanır',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe etiketlenir',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return 'Yalnızca son $count öğe denetlendi.';
  }

  @override
  String rulesPreviewMore(int count) {
    return 've $count tane daha';
  }

  @override
  String get rulesPreviewNote =>
      'Uygulayana kadar hiçbir şey değişmez. Değişiklikler diğer cihazlarınızla normal düzenlemeler gibi eşitlenir.';

  @override
  String get rulesSecretItem => 'Gizli öğe';

  @override
  String rulesSecretItemFrom(String app) {
    return '$app uygulamasından gizli öğe';
  }

  @override
  String get rulesPreviewCancelled =>
      'Önizleme durduruldu. Hiçbir şey değişmedi.';

  @override
  String get rulesPreviewFailed => 'Önizleme çalıştırılamadı.';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe silinsin mi?',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage =>
      '“Kaydetme” işlemini mevcut öğelere uygulamak, eşleşen öğeleri tüm cihazlarınızdaki Geçmiş’ten siler. Bu işlem geri alınamaz.';

  @override
  String get rulesApplyDeleteConfirm => 'Sil ve uygula';

  @override
  String rulesApplyProgress(int done, int total) {
    return 'Uygulanıyor: $done/$total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe değiştirildi',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe silindi',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count öğe değiştirilemedi',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled =>
      'Durduruldu. Durdurmadan önce değiştirilen öğeler değişmiş olarak kalır.';
}
