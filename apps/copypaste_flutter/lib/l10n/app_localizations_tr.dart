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
  String get settingsRuntimeQuickPastePrepareFailed =>
      'Quick Paste hazırlanamadı.';

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
  String get settingsExportIncompleteTitle => 'Metin dışa aktarımı eksik';

  @override
  String settingsExportIncompleteMessage(String saved, String omissions) {
    return '$saved; $omissions.';
  }

  @override
  String settingsExportOmissionsJoin(String first, String second) {
    return '$first ve $second';
  }

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
  String settingsStorageQuotaGigabytes(String size) {
    return '$size GB';
  }

  @override
  String settingsFileSizeMebibytes(int size) {
    return '$size MiB';
  }

  @override
  String settingsFileSizeGibibytes(int size) {
    return '$size GiB';
  }

  @override
  String settingsExportSavedClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count metin öğesi kaydedildi',
      one: '$count metin öğesi kaydedildi',
    );
    return '$_temp0';
  }

  @override
  String settingsExportSkippedNonText(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'metin olmayan $count öğe hariç tutuldu',
      one: 'metin olmayan $count öğe hariç tutuldu',
    );
    return '$_temp0';
  }

  @override
  String settingsExportSkippedUnreadable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count metin öğesi okunamadı',
      one: '$count metin öğesi okunamadı',
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
  String get appCollapseNavigation => 'Gezinmeyi daralt';

  @override
  String get appExpandNavigation => 'Gezinmeyi genişlet';

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
  String sharedSizeKilobytes(String size) {
    return '$size KB';
  }

  @override
  String sharedSizeMegabytes(String size) {
    return '$size MB';
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
  String get modulesInstalling => 'Yükleniyor…';

  @override
  String modulesDownloadingProgress(int percent) {
    return 'İndiriliyor: %$percent';
  }

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
}
