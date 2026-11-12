// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Ukrainian (`uk`).
class AppLocalizationsUk extends AppLocalizations {
  AppLocalizationsUk([String locale = 'uk']) : super(locale);

  @override
  String get commonCancel => 'Скасувати';

  @override
  String get commonClose => 'Закрити';

  @override
  String get commonDone => 'Готово';

  @override
  String get commonRetry => 'Повторити';

  @override
  String get commonTryAgain => 'Спробувати ще раз';

  @override
  String get commonCopy => 'Скопіювати';

  @override
  String get commonDelete => 'Видалити';

  @override
  String get commonSave => 'Зберегти';

  @override
  String get commonContinue => 'Продовжити';

  @override
  String get commonBack => 'Назад';

  @override
  String get commonOpenSettings => 'Відкрити налаштування';

  @override
  String get commonLoading => 'Завантаження';

  @override
  String get commonApply => 'Застосувати';

  @override
  String get commonClear => 'Очистити';

  @override
  String get commonPin => 'Закріпити';

  @override
  String get commonEnable => 'Увімкнути';

  @override
  String get settingsLanguageTitle => 'Мова';

  @override
  String get settingsLanguageDescription =>
      'Виберіть мову інтерфейсу CopyPaste.';

  @override
  String get settingsLanguageSystem => 'Як у системі';

  @override
  String get settingsLanguageKeywords =>
      'мова локаль переклад інтерфейс language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Не вдалося змінити запуск під час входу. Спробуйте ще раз.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'Не вдалося відкрити системні налаштування автозапуску.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Не вдалося оновити налаштування. Спробуйте ще раз.';

  @override
  String get settingsRuntimeLoadFailed =>
      'Не вдалося завантажити налаштування.';

  @override
  String get settingsRuntimeAppAlreadyExcluded => 'Цю програму вже виключено.';

  @override
  String get settingsRuntimeChangeFailed =>
      'Не вдалося виконати запитану зміну.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Щоб відновити запис буфера обміну, надайте всі потрібні дозволи.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'Запис буфера обміну призупинено. Служба запису Android не підтвердила зупинку.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'Щоб увімкнути сповіщення про запис, потрібен дозвіл на сповіщення.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste не вдалося прочитати збережені налаштування. Застосовано захисні значення; перегляньте й збережіть свої параметри, щоб відновити їх.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'Нечитабельні налаштування CopyPaste замінено захисними значеннями. Перегляньте й збережіть свої параметри, щоб відновити їх.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Ідентифікатор програми не може бути порожнім і має займати не більше 256 байтів.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'Можна виключити до 256 програм.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Не вдалося змінити автовставлення.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'Не вдалося зареєструвати це скорочення.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'Не вдалося тимчасово вимкнути поточне скорочення.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'Не вдалося відновити скорочення.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'Не вдалося відкрити Quick Paste.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Не вдалося запитати дозвіл «Універсальний доступ».';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Не вдалося оновити стан інтеграції з Linux.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'Дозвіл на введення з буфера обміну не надано.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Не вдалося запитати дозвіл на введення з буфера обміну.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Не вдалося відкрити налаштування інтеграції з робочим середовищем.';

  @override
  String get settingsRuntimeTextHistoryFileType => 'Текстова історія CopyPaste';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'Зашифрована резервна копія CopyPaste';

  @override
  String get settingsRuntimeNotificationImage => 'Зображення';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Зображення · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'Файл';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste не вдалося відновити встановлення оновлення.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Це встановлення для Linux не оновлюється автоматично.';

  @override
  String get updateMacosPackageMissing =>
      'Цей випуск не містить підписаного пакета оновлення для macOS.';

  @override
  String get updateInstalledVersionInvalid =>
      'Встановлена версія CopyPaste недійсна.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste не вдалося з’єднатися з GitHub, щоб перевірити оновлення.';

  @override
  String get updateCheckTimedOut => 'Час очікування перевірки оновлень минув.';

  @override
  String get updateCheckDeviceFailed =>
      'CopyPaste не вдалося перевірити оновлення на цьому пристрої.';

  @override
  String get updateCheckFailed => 'CopyPaste не вдалося перевірити оновлення.';

  @override
  String get updateDownloadFailed =>
      'CopyPaste не вдалося завантажити оновлення.';

  @override
  String get updateDownloadTimedOut =>
      'Час очікування завантаження оновлення минув.';

  @override
  String get updateInstallFailed =>
      'CopyPaste не вдалося встановити оновлення.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Продовжте в системному інсталяторі Android.';

  @override
  String get updateContinueInPackageManager =>
      'Продовжте в системному менеджері пакетів.';

  @override
  String get updateInstallerStarting => 'Інсталятор запускається.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Дозвольте CopyPaste встановлювати програми в налаштуваннях Android, а потім продовжте.';

  @override
  String get updatePermissionRequiredLinux =>
      'Пройдіть автентифікацію в системному менеджері пакетів, щоб встановити це оновлення.';

  @override
  String get updatePermissionRequired =>
      'Дозвольте CopyPaste встановити це оновлення, а потім продовжте.';

  @override
  String get updateRestartRequired =>
      'Оновлення встановлено. Перезапустіть CopyPaste, щоб почати ним користуватися.';

  @override
  String get updateRestarting => 'Перезапуск CopyPaste.';

  @override
  String get updateRestartFailed =>
      'Не вдалося перезапустити CopyPaste. Спробуйте ще раз.';

  @override
  String get updateReleasePageOpenFailed =>
      'CopyPaste не вдалося відкрити сторінку випуску.';

  @override
  String get updatePackageInvalid =>
      'Завантажене оновлення не є дійсним пакетом CopyPaste.';

  @override
  String get updateSignatureInvalid =>
      'Завантажене оновлення має недійсний підпис.';

  @override
  String get updateDowngradeRefused =>
      'Завантажений пакет не новіший за цю версію.';

  @override
  String get updateHomebrewRequired =>
      'Встановіть CopyPaste через Homebrew, щоб оновлювати програму тут.';

  @override
  String get updateInstallationCancelled => 'Встановлення оновлення скасовано.';

  @override
  String get updateInstallationBlocked =>
      'Система Android заблокувала встановлення оновлення.';

  @override
  String get updateInstallationConflict =>
      'Оновлення конфліктує зі встановленою програмою.';

  @override
  String get updateInstallationIncompatible =>
      'Оновлення несумісне з цим пристроєм.';

  @override
  String get updateInstallationStorage =>
      'Звільніть місце в пам’яті, щоб встановити оновлення.';

  @override
  String get updateInstallationInterrupted =>
      'Встановлення оновлення перервано. Спробуйте ще раз.';

  @override
  String get updateInstallationBusy => 'Встановлення оновлення вже триває.';

  @override
  String get updatePackageDetailsInvalid =>
      'Відомості про пакет оновлення недійсні.';

  @override
  String get updateIntegrityCheckFailed =>
      'Завантажене оновлення не пройшло перевірку цілісності.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste не вдалося запустити системний інсталятор.';

  @override
  String get updateInstallLocationTranslocated =>
      'Перемістіть CopyPaste до папки «Програми» й відкрийте звідти, щоб оновити.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste не може замінити себе в поточній папці. Скористайтеся сторінкою випуску, щоб оновити.';

  @override
  String get updateReplacementBlocked =>
      'Система macOS заблокувала заміну CopyPaste. Дозвольте її в налаштуваннях «Керування програмами» або скористайтеся сторінкою випуску.';

  @override
  String get updatePackageNotDownloadable =>
      'Ця платформа не використовує пакет оновлення для завантаження.';

  @override
  String get updatePackageSizeInvalid => 'Недійсний розмір пакета оновлення.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'Завантажене оновлення має недійсний підпис засобу оновлення.';

  @override
  String get updatePackageUrlUntrusted => 'Адреса пакета оновлення ненадійна.';

  @override
  String get updateDownloadLeftGitHub =>
      'Завантаження оновлення переспрямовано за межі GitHub.';

  @override
  String get updateDownloadTooManyRedirects =>
      'Завантаження оновлення має забагато переспрямувань.';

  @override
  String get updatePackageTooLarge => 'Пакет оновлення завеликий.';

  @override
  String get updateSignatureSizeInvalid =>
      'Недійсний розмір підпису засобу оновлення.';

  @override
  String get updateMetadataInvalid =>
      'GitHub повернув недійсні метадані оновлення.';

  @override
  String get updateMetadataTooLarge => 'Метадані оновлення завеликі.';

  @override
  String get quickPasteActionFailedTitle => 'Дію не виконано';

  @override
  String get quickPasteOpenAppSemantics => 'Відкрити CopyPaste';

  @override
  String get quickPasteSearchPlaceholder => 'Введіть текст для пошуку…';

  @override
  String get quickPasteLoadingHistory => 'Завантаження історії буфера обміну.';

  @override
  String get quickPasteHistoryUnavailable => 'Історія недоступна';

  @override
  String get quickPasteNoClipsFound => 'Кліпів не знайдено';

  @override
  String get quickPasteLoadMore => 'Завантажити ще';

  @override
  String get quickPasteTransferFailed => 'Помилка';

  @override
  String get quickPasteClipUnavailable => 'Кліп недоступний';

  @override
  String get quickPasteFileFallbackName => 'Файл';

  @override
  String get quickPastePathUnavailable => 'Шлях недоступний';

  @override
  String get quickPastePreferences => 'Параметри…';

  @override
  String get quickPasteAbout => 'Про програму';

  @override
  String get quickPasteQuit => 'Вийти';

  @override
  String get quickPasteClearHistoryTitle => 'Очистити історію?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Закріплені кліпи залишаться в історії.';

  @override
  String get quickPasteClearHistoryFailed => 'Не вдалося видалити історію.';

  @override
  String get quickPasteAboutDescription =>
      'Приватна історія буфера обміну для macOS, Android, Windows і Linux.';

  @override
  String get transfersTitle => 'Завантаження й передавання';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count активного передавання',
      many: '$count активних передавань',
      few: '$count активні передавання',
      one: '$count активне передавання',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count активного',
      many: '$count активних',
      few: '$count активні',
      one: '$count активне',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Останні';

  @override
  String get transfersShowEarlier => 'Раніші';

  @override
  String get transfersClearCompletedHistory => 'Очистити історію завершених';

  @override
  String get transfersErrorTitle => 'Передавання';

  @override
  String get transfersEmptyTitle => 'Немає передавань';

  @override
  String get transfersEmptyMessage =>
      'Тут з’являтимуться імпортовані файли, завантаження й вихідні передавання.';

  @override
  String transfersSpeed(String size) {
    return '$size/с';
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
  String get transfersRemoveFromHistory => 'Вилучити з історії';

  @override
  String get transfersCancelDownload => 'Скасувати завантаження';

  @override
  String get transfersShowFile => 'Показати файл';

  @override
  String get transfersPhaseSending => 'Надсилання';

  @override
  String get transfersPhaseDownloading => 'Завантаження';

  @override
  String get transfersPhaseAdding => 'Додавання';

  @override
  String get transfersPhaseAdded => 'Додано';

  @override
  String get transfersPhaseSent => 'Надіслано';

  @override
  String get transfersPhaseCompleted => 'Завершено';

  @override
  String get transfersPhaseCancelled => 'Скасовано';

  @override
  String get transfersPhaseInterrupted => 'Перервано';

  @override
  String get transfersPhaseFailed => 'Помилка';

  @override
  String get transfersUpdatesUnavailable =>
      'Оновлення стану передавань недоступні. Відкрийте знову, щоб оновити.';

  @override
  String get transfersLoadFailed =>
      'Не вдалося завантажити передавання. Спробуйте ще раз.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Не вдалося завантажити оновлення стану передавань.';

  @override
  String get transfersRetryFailed =>
      'Не вдалося повторити передавання цього файлу.';

  @override
  String get transfersClearHistoryFailed =>
      'Не вдалося очистити історію передавань.';

  @override
  String get transfersRemoveRecordFailed =>
      'Не вдалося вилучити цей запис історії.';

  @override
  String get transfersCancelFailed =>
      'Не вдалося скасувати передавання. Можливо, воно ще триває.';

  @override
  String get transfersSavedFileUnavailable =>
      'Збережений файл недоступний. Можливо, його переміщено або видалено.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Сповіщення · $count непрочитаного',
      many: 'Сповіщення · $count непрочитаних',
      few: 'Сповіщення · $count непрочитані',
      one: 'Сповіщення · $count непрочитане',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Сповіщення';

  @override
  String get notificationsClearRead => 'Очистити прочитані сповіщення';

  @override
  String get notificationsEmptyTitle => 'Немає сповіщень';

  @override
  String get notificationsEmptyMessage =>
      'Тут з’являтимуться запити на з’єднання пристроїв та їхні результати.';

  @override
  String get notificationsSystemAlertTitle => 'Системні сповіщення';

  @override
  String get notificationsReview => 'Переглянути';

  @override
  String get notificationsConnectionRequest => 'Запит на з’єднання';

  @override
  String get notificationsConfirmConnection => 'Підтвердьте з’єднання';

  @override
  String get notificationsDeviceConnected => 'Пристрій під’єднано';

  @override
  String get notificationsConnectionRejected => 'З’єднання відхилено';

  @override
  String get notificationsConnectionCancelled => 'З’єднання скасовано';

  @override
  String get notificationsConnectionRequestExpired =>
      'Термін дії запиту на з’єднання минув';

  @override
  String get notificationsConnectionFailed => 'Не вдалося з’єднатися';

  @override
  String get notificationsNearbyDevice => 'Пристрій поблизу';

  @override
  String get notificationsPermissionDenied =>
      'Дозвольте системні сповіщення в налаштуваннях ОС, щоб отримувати запити у фоновому режимі.';

  @override
  String get notificationsSystemUnavailable =>
      'Системні сповіщення недоступні. Запити на з’єднання залишаються тут.';

  @override
  String get platformCaptureStateUnavailable => 'Недоступно';

  @override
  String get platformCaptureStatePaused => 'Призупинено';

  @override
  String get platformCaptureStateRunning => 'Запис триває';

  @override
  String get platformCaptureStateStopped => 'Не запущено';

  @override
  String get platformCaptureActionUnavailable => 'Запис недоступний';

  @override
  String get platformCaptureActionResume => 'Відновити запис';

  @override
  String get platformCaptureActionPause => 'Призупинити запис';

  @override
  String get platformCaptureActionResumeStopped =>
      'Відновити запис (не запущено)';

  @override
  String get platformTrayOpen => 'Відкрити';

  @override
  String get platformTraySettings => 'Налаштування';

  @override
  String get platformTrayQuit => 'Вийти';

  @override
  String get platformWindowControlsUnavailable =>
      'Керування вікнами недоступне. Вікно CopyPaste залишатиметься видимим.';

  @override
  String get platformTrayUnavailable =>
      'Системний трей недоступний. Не закривайте CopyPaste, а потім повторіть спробу або вийдіть.';

  @override
  String get platformQuitFailed =>
      'Не вдалося вийти з CopyPaste. Спробуйте ще раз.';

  @override
  String get platformVisibilityChangeFailed =>
      'Не вдалося змінити видимість програми. Спробуйте ще раз.';

  @override
  String get platformCaptureNotificationTitle => 'Буфер обміну збережено';

  @override
  String get platformCaptureNotificationBody =>
      'До CopyPaste додано новий елемент.';

  @override
  String get platformPairingNotificationTitle => 'Запит на з’єднання';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName хоче з’єднатися. Відкрийте CopyPaste, щоб порівняти код і підтвердити.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Запити на з’єднання пристроїв';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Запити на з’єднання з пристроями поблизу';

  @override
  String get platformNotificationDefaultAction => 'Відкрити';

  @override
  String get platformFilesSelectionFailed => 'Не вдалося вибрати файли.';

  @override
  String get platformFilesOnlyRegular =>
      'Можна імпортувати лише звичайні файли.';

  @override
  String get platformModulesUnavailable =>
      'Модулі недоступні на цьому пристрої.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'Не вдалося налаштувати доступ до SMS.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'Не вдалося перевірити доступ до SMS.';

  @override
  String get platformUpdatesUnavailableHere =>
      'Оновлення програми тут недоступні.';

  @override
  String get platformUpdateVersionUnavailable =>
      'Встановлена версія недоступна.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'Оновлення програми недоступні для цього встановлення.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Це встановлення для Linux не оновлюється автоматично.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Це оновлення для Linux не відповідає архітектурі системи.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Це встановлення для macOS не оновлюється автоматично.';

  @override
  String get platformUpdateStartFailed =>
      'CopyPaste не вдалося розпочати оновлення.';

  @override
  String get platformGlobalShortcutDescription => 'Відкрити Quick Paste';

  @override
  String get settingsSectionGeneral => 'Загальні';

  @override
  String get settingsSectionGeneralSummary => 'Мова й видимість програми.';

  @override
  String get settingsSectionClipboard => 'Буфер обміну';

  @override
  String get settingsSectionClipboardSummary => 'Запис та обмеження історії.';

  @override
  String get settingsSectionPrivacy => 'Приватність';

  @override
  String get settingsSectionPrivacySummary =>
      'Виключення програм і захист екрана.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Скорочення й автоматичне вставлення.';

  @override
  String get settingsSectionSync => 'Синхронізація';

  @override
  String get settingsSectionSyncSummary =>
      'Синхронізація й виявлення пристроїв поблизу.';

  @override
  String get settingsSectionNotifications => 'Сповіщення';

  @override
  String get settingsSectionNotificationsSummary =>
      'Сповіщення й звуки запису.';

  @override
  String get settingsSectionData => 'Дані';

  @override
  String get settingsSectionDataSummary =>
      'Експорт, резервне копіювання й відновлення.';

  @override
  String get settingsSectionModules => 'Модулі';

  @override
  String get settingsSectionModulesSummary =>
      'Встановлення й керування додатковими модулями.';

  @override
  String get settingsSectionRules => 'Правила вмісту';

  @override
  String get settingsSectionRulesSummary =>
      'Автоматично додавайте теги, пропускайте, захищайте або видаляйте записи за часом.';

  @override
  String get settingsSectionQuickAccess => 'Швидкий доступ';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Клавіатура, плитки й віджет головного екрана.';

  @override
  String get settingsSectionSnippets => 'Фрагменти';

  @override
  String get settingsSectionSnippetsSummary =>
      'Повторно використовуваний текст і шаблони.';

  @override
  String get settingsSectionTransforms => 'Перетворення тексту';

  @override
  String get settingsSectionTransformsSummary =>
      'Улюблені, конвеєри й типові перетворення під час вставлення.';

  @override
  String get settingsSectionAiTools => 'Інструменти ШІ';

  @override
  String get settingsSectionAiToolsSummary =>
      'Підключайте ШІ-асистентів і керуйте тим, що вони бачать.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Збирайте кілька копій і вставляйте їх по черзі.';

  @override
  String pasteQueuePasted(int position, int total) {
    return 'Вставлено $position з $total';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'Елемент $position з $total у буфері обміну';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Вставлено $count елемента',
      many: 'Вставлено $count елементів',
      few: 'Вставлено $count елементи',
      one: 'Вставлено $count елемент',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count елемента в буфері обміну',
      many: '$count елементів у буфері обміну',
      few: '$count елементи в буфері обміну',
      one: '$count елемент у буфері обміну',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Чергу завершено';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Додано, у черзі $total',
      many: 'Додано, у черзі $total',
      few: 'Додано, у черзі $total',
      one: 'Додано, у черзі $total',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'Черга порожня, нічого не вставлено';

  @override
  String get pasteQueueMixed =>
      '«Вставити все» працює лише з чергою тільки з тексту або тільки з файлів';

  @override
  String get pasteQueueSeveralFiles =>
      'Вставляти кілька файлів одночасно поки не можна. Вставляйте їх по одному.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count елемента більше недоступні',
      many: '$count елементів більше недоступні',
      few: '$count елементи більше недоступні',
      one: '$count елемент більше недоступний',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => '«Вставити наступний» лише копіює';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Дозвольте «Універсальний доступ», щоб вставляти автоматично. Доти елемент у буфері обміну — вставте його самостійно.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Не вдалося вставити автоматично. Елемент у буфері обміну — вставте його звичним поєднанням клавіш.';

  @override
  String get pasteQueueTargetElevated =>
      'Не вдалося вставити в застосунок, запущений від імені адміністратора. Елемент лишився в черзі й у буфері обміну.';

  @override
  String get pasteQueueNoTarget =>
      'Немає застосунку, куди вставити. Елемент лишився в черзі й у буфері обміну.';

  @override
  String get pasteQueueCollectingStarted => 'Збирання копій';

  @override
  String get pasteQueueCollectingStopped => 'Збирання зупинено';

  @override
  String get pasteQueueFailed => 'Не вдалося виконати дію Paste Queue';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count в очікуванні',
      many: '$count в очікуванні',
      few: '$count в очікуванні',
      one: '$count в очікуванні',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count в очікуванні · Збирання',
      many: '$count в очікуванні · Збирання',
      few: '$count в очікуванні · Збирання',
      one: '$count в очікуванні · Збирання',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Збирання';

  @override
  String get pasteQueueFinishedSummary => 'Завершено';

  @override
  String pasteQueueFull(int max) {
    return 'Черга заповнена ($max)';
  }

  @override
  String get pasteQueueFullMessage =>
      'Нові копії більше не додаються. Нічого не видалено.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue порожня';

  @override
  String get pasteQueueEmptyMessage =>
      'Збирайте копії, щоб вставляти їх по черзі, або додайте елементи з історії.';

  @override
  String get pasteQueueCollectingEmptyTitle =>
      'Копіюйте елементи, щоб додати їх';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Кожна копія потрапляє в чергу, доки ви не зупините збирання.';

  @override
  String get pasteQueueStartCollecting => 'Почати збирання';

  @override
  String get pasteQueueStopCollecting => 'Зупинити збирання';

  @override
  String get pasteQueueLoading => 'Завантаження черги…';

  @override
  String get pasteQueueLoadFailed => 'Не вдалося завантажити чергу';

  @override
  String get pasteQueuePasteNext => 'Вставити наступний';

  @override
  String get pasteQueueCopyNext => 'Копіювати наступний';

  @override
  String get pasteQueuePasteAll => 'Вставити все';

  @override
  String get pasteQueueCopyAll => 'Копіювати все';

  @override
  String get pasteQueueReverse => 'Обернути порядок';

  @override
  String get pasteQueueStepBack => 'Крок назад';

  @override
  String get pasteQueueSkip => 'Пропустити';

  @override
  String get pasteQueueRemovePasted => 'Прибрати вставлені';

  @override
  String get pasteQueueCleared => 'Чергу очищено';

  @override
  String get pasteQueueUndo => 'Скасувати';

  @override
  String get pasteQueueStartFresh => 'Почати заново';

  @override
  String get pasteQueueAppend => 'Додати в кінець';

  @override
  String get pasteQueueRestartTitle => 'Почати нове збирання?';

  @override
  String get pasteQueueRestartMessage =>
      '«Почати заново» прибирає вставлені елементи. «Додати в кінець» їх зберігає.';

  @override
  String pasteQueueStale(String time) {
    return 'Черга від $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Вставлено';

  @override
  String get pasteQueueStatusSkipped => 'Пропущено';

  @override
  String get pasteQueueStatusUnavailable => 'Більше недоступний';

  @override
  String get pasteQueueEdited => 'Змінено';

  @override
  String get pasteQueueSecret => 'Секрет';

  @override
  String get pasteQueueKindImage => 'Зображення';

  @override
  String get pasteQueueKindFiles => 'Файли';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Сформовано о $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Елемент $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'Переміщено на позицію $position';
  }

  @override
  String get pasteQueueMoveUp => 'Перемістити вгору';

  @override
  String get pasteQueueMoveDown => 'Перемістити вниз';

  @override
  String get pasteQueueMoveToTop => 'На початок';

  @override
  String get pasteQueueMoveToBottom => 'У кінець';

  @override
  String get pasteQueueReorder => 'Перетягніть, щоб змінити порядок';

  @override
  String get pasteQueueMoreActions => 'Інші дії';

  @override
  String get pasteQueuePasteNow => 'Вставити зараз';

  @override
  String get pasteQueueEdit => 'Редагувати текст';

  @override
  String get pasteQueueUseOriginal => 'Використати оригінальний текст';

  @override
  String get pasteQueueRemove => 'Прибрати з черги';

  @override
  String get pasteQueueEditTitle => 'Редагувати текст у черзі';

  @override
  String get pasteQueueEditMessage =>
      'Зміни стосуються лише цієї черги. Історія зберігає оригінал.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Налаштувати поєднання клавіш';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Виберіть поєднання клавіш, щоб збирати й вставляти, не відкриваючи CopyPaste.';

  @override
  String get pasteQueueShortcutToggleCollecting =>
      'Почати або зупинити збирання';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Вставити наступний як звичайний текст';

  @override
  String get pasteQueueShortcutDescription =>
      'Глобальне поєднання клавіш, що працює в будь-якому застосунку.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Це поєднання вже використовує інше поєднання клавіш CopyPaste. Попереднє збережено.';

  @override
  String get pasteQueueShortcutInUse =>
      'Це поєднання використовує інший застосунок. Попереднє збережено; виберіть інше.';

  @override
  String get pasteQueueShortcutRejected =>
      'Система відхилила це поєднання. Попереднє збережено.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt вводить символи в багатьох розкладках (AltGr). Виберіть інше поєднання.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'У цьому сеансі глобальні поєднання клавіш недоступні. Користуйтеся чергою у вікні або в Quick Paste.';

  @override
  String get pasteQueueShortcutFailed =>
      'Не вдалося налаштувати це поєднання клавіш.';

  @override
  String get pasteQueueEnableTitle => 'Увімкнути Paste Queue';

  @override
  String get pasteQueueEnableDescription =>
      'Якщо вимкнено, поєднання клавіш черги звільняються й нічого не збирається.';

  @override
  String get pasteQueuePasteNextStyleTitle => '«Вставити наступний» виконує';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Вставлення в активний застосунок потребує дозволу надсилати натискання клавіш. «Лише буфер обміну» залишає вставлення вам.';

  @override
  String get pasteQueuePasteIntoApp => 'Вставити в активний застосунок';

  @override
  String get pasteQueueClipboardOnly => 'Лише в буфер обміну';

  @override
  String get pasteQueueInsertTitle => 'Нові елементи додаються';

  @override
  String get pasteQueueInsertDescription =>
      'Куди в черзі потрапляють зібрані й додані елементи.';

  @override
  String get pasteQueueInsertEnd => 'У кінець (першим прийшов — першим вийшов)';

  @override
  String get pasteQueueInsertStart =>
      'На початок (останнім прийшов — першим вийшов)';

  @override
  String get pasteQueueEndBehaviorTitle => 'Після останнього елемента';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'Що робить «Вставити наступний», коли всі елементи вже вставлено.';

  @override
  String get pasteQueueEndStop => 'Зупинити';

  @override
  String get pasteQueueEndLoop => 'Почати спочатку';

  @override
  String get pasteQueueEndKeepLast => 'Вставляти останній далі';

  @override
  String get pasteQueueSeparatorTitle => 'Роздільник для «Вставити все»';

  @override
  String get pasteQueueSeparatorDescription =>
      'Ставиться між елементами, коли вставляються всі разом.';

  @override
  String get pasteQueueSeparatorNewLine => 'Новий рядок';

  @override
  String get pasteQueueSeparatorBlankLine => 'Порожній рядок';

  @override
  String get pasteQueueSeparatorTab => 'Табуляція';

  @override
  String get pasteQueueSeparatorSpace => 'Пробіл';

  @override
  String get pasteQueueSeparatorComma => 'Кома';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Кома й пробіл';

  @override
  String get pasteQueueSeparatorNone => 'Немає';

  @override
  String get pasteQueueSeparatorCustom => 'Власний';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'До 32 символів; можна \\n і \\t';

  @override
  String get pasteQueuePlainTextTitle => 'Завжди вставляти як звичайний текст';

  @override
  String get pasteQueuePlainTextDescription =>
      'Прибирає форматування з елементів черги.';

  @override
  String get pasteQueueIdleStopTitle => 'Зупиняти збирання через';

  @override
  String get pasteQueueIdleStopDescription =>
      'Збирання вимикається, якщо стільки часу нічого не копіювали. Елементи лишаються.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes хв';
  }

  @override
  String get pasteQueueIdleNever => 'Ніколи';

  @override
  String get pasteQueueMaxEntriesTitle => 'Ліміт розміру черги';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Коли черга заповнена, елементи не додаються. Нічого не відкидається.';

  @override
  String get pasteQueueRestoreTitle =>
      'Відновлювати попередній буфер обміну після завершення';

  @override
  String get pasteQueueRestoreDescription =>
      'Повертає те, що ви скопіювали до того, як черга почала вставляти.';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue черга збирання порядок поєднання клавіш роздільник';

  @override
  String get settingsSectionAbout => 'Про програму';

  @override
  String get settingsSectionAboutSummary => 'Версія й оновлення програми.';

  @override
  String get settingsGeneralDescription =>
      'Мова, автозапуск і видимість програми.';

  @override
  String get settingsModulesDescription => 'Додаткові функції для CopyPaste.';

  @override
  String get settingsClipboardDescription =>
      'Записуйте зміни буфера обміну й керуйте обмеженнями історії.';

  @override
  String get settingsDataDescription =>
      'Експортуйте, створюйте резервні копії й відновлюйте історію буфера обміну.';

  @override
  String get settingsSyncDescription =>
      'Керуйте синхронізацією зі сполученими пристроями.';

  @override
  String get settingsPrivacyDescription =>
      'Визначайте, з яких програм записувати дані, і захищайте вміст буфера обміну.';

  @override
  String get settingsNotificationsDescription =>
      'Виберіть сповіщення й звуки для записаних кліпів.';

  @override
  String get settingsAboutDescription =>
      'Версія CopyPaste та оновлення програми.';

  @override
  String get settingsQuickPasteDescription =>
      'Відкривайте історію буфера обміну звідусіль, не перемикаючи вікна.';

  @override
  String get settingsLoadingMessage => 'Завантаження налаштувань.';

  @override
  String get settingsUnavailableTitle => 'Налаштування недоступні';

  @override
  String get settingsNoResultsTitle => 'Налаштувань не знайдено';

  @override
  String get settingsNoResultsMessage => 'Спробуйте інший запит.';

  @override
  String get settingsAttentionTitle => 'Налаштування потребують уваги';

  @override
  String get settingsSearchPlaceholder => 'Пошук у налаштуваннях';

  @override
  String get settingsBackToSettings => 'Назад до налаштувань';

  @override
  String get settingsStartOnBootTitle => 'Запуск після ввімкнення пристрою';

  @override
  String get settingsStartAtLoginTitle => 'Запуск під час входу';

  @override
  String get settingsStartupSearchDescription =>
      'Автоматично запускати CopyPaste.';

  @override
  String get settingsStartupKeywords =>
      'автозапуск запуск вхід завантаження автоматично startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Вимкнено в Диспетчері завдань. Увімкніть CopyPaste у розділі «Настройки → Програми → Автозавантаження».';

  @override
  String get settingsStartupMacApproval =>
      'Дозвольте CopyPaste у розділі «Системні параметри → Загальні → Елементи входу».';

  @override
  String get settingsStartupUnavailable =>
      'Автозапуск недоступний для цього встановлення.';

  @override
  String get settingsStartupAndroidDescription =>
      'Відновлювати ввімкнені фонові служби після перезапуску пристрою.';

  @override
  String get settingsStartupDesktopDescription =>
      'Автоматично відкривати CopyPaste під час входу в систему.';

  @override
  String get settingsShowInDock => 'Показувати в Dock';

  @override
  String get settingsShowInTaskbar => 'Показувати на панелі завдань';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Показувати значок програми для перемикання між вікнами.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'док панель завдань значок програми видимість dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Показувати в смузі меню';

  @override
  String get settingsShowInSystemTray => 'Показувати в системному треї';

  @override
  String get settingsTrayVisibilityDescription =>
      'Показувати швидкий доступ до CopyPaste і керування записом.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'трей смуга меню стан значок видимість tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle => 'Не вдалося змінити видимість';

  @override
  String get settingsModulesSearchDescription =>
      'Встановлюйте, оновлюйте й вилучайте додаткові модулі.';

  @override
  String get settingsModulesKeywords =>
      'функції команди розширення features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Фоновий запис на Android';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Повний або обмежений запис із налаштуванням через Shizuku чи ADB.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'фон дозволи налаштування background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Виберіть повний або обмежений запис і керуйте одноразовим налаштуванням Shizuku чи ADB.';

  @override
  String get settingsOpenSetup => 'Налаштувати';

  @override
  String get settingsSaveScreenshotsTitle => 'Зберігати знімки екрана';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Автоматично зберігати нові знімки екрана Android в Історії.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'зображення фото запис дозволи images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Автоматично додавати нові знімки екрана до Історії.';

  @override
  String get settingsScreenshotAccessTitle => 'Доступ до знімків екрана';

  @override
  String get settingsScreenshotAccessDescription =>
      'Надайте доступ до всіх фото й сповіщень, щоб зберігати нові знімки екрана у фоновому режимі.';

  @override
  String get settingsAllowAccess => 'Дозволити доступ';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'Запис знімків екрана не запущено';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'Нові знімки екрана не зберігаються. Відновіть запис, щоб знову запустити службу.';

  @override
  String get settingsResumeCapture => 'Відновити запис';

  @override
  String get settingsClipboardCaptureTitle => 'Запис буфера обміну';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Призупинити або відновити запис буфера обміну.';

  @override
  String get settingsCaptureStateUnavailable => 'Стан недоступний';

  @override
  String get settingsCaptureStoppedTitle => 'Запис не запущено';

  @override
  String get settingsCaptureStoppedMessage =>
      'Зараз CopyPaste не отримує змін буфера обміну на цьому пристрої.';

  @override
  String get settingsRetentionTitle => 'Зберігання';

  @override
  String get settingsRetentionDescription =>
      'Автоматично вилучати старі незакріплені елементи буфера обміну.';

  @override
  String get settingsRetentionForever => 'Зберігати завжди';

  @override
  String get settingsStorageQuotaTitle => 'Квота сховища';

  @override
  String get settingsStorageQuotaDescription =>
      'Максимальний обсяг локального сховища для незакріпленої історії.';

  @override
  String get settingsStorageQuotaKeywords =>
      'диск місце обмеження disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Максимальний розмір файлу';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Обмеження для додавання й завантаження файлів.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'імпорт завантаження передавання розмір обмеження import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Обмеження для додавання й завантаження файлів. Вміст файлу передається, лише коли ви вибираєте «Завантажити».';

  @override
  String get settingsHistoryFilesTitle => 'Файли історії';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Експортуйте історію, створюйте резервні копії або відновлюйте з резервної копії.';

  @override
  String get settingsHistoryFilesKeywords =>
      'текст зашифрована резервна копія дані text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Експорт текстової історії';

  @override
  String get settingsExportTextDescription =>
      'Зберегти текстові кліпи в переносному файлі.';

  @override
  String get settingsExportAction => 'Експортувати';

  @override
  String get settingsBackupTitle => 'Зашифрована резервна копія';

  @override
  String get settingsBackupDescription =>
      'Зберегти всю локальну історію цього пристрою.';

  @override
  String get settingsCreateBackupAction => 'Створити резервну копію';

  @override
  String get settingsRestoreTitle => 'Відновити з резервної копії';

  @override
  String get settingsRestoreDescription =>
      'Замінити локальну історію зашифрованою резервною копією.';

  @override
  String get settingsRestoreAction => 'Відновити';

  @override
  String get settingsRestoreDialogTitle => 'Замінити локальну історію?';

  @override
  String get settingsRestoreDialogMessage =>
      'Вибрана зашифрована резервна копія замінить історію буфера обміну на цьому пристрої. Поточні налаштування та ідентичність пристрою збережуться.';

  @override
  String get settingsRestoreFailed => 'Не вдалося відновити резервну копію.';

  @override
  String get settingsSyncSwitchTitle => 'Синхронізація';

  @override
  String get settingsSyncSwitchDescription =>
      'Головний перемикач синхронізації зі сполученими пристроями.';

  @override
  String get settingsSyncSearchDescription =>
      'Синхронізація зі сполученими пристроями.';

  @override
  String get settingsInstantClipboardTitle => 'Миттєвий буфер обміну';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Автоматично копіювати нові кліпи з інших пристроїв.';

  @override
  String get settingsInstantClipboardKeywords =>
      'автоматично копіювати вставити синхронізація отримані automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Автоматично копіювати новіші кліпи з інших пристроїв у буфер обміну цього пристрою.';

  @override
  String get settingsLanVisibilityTitle => 'Видимість у локальній мережі';

  @override
  String get settingsLanVisibilityDescription =>
      'Дозволити пристроям поблизу виявляти цей пристрій.';

  @override
  String get settingsLanVisibilityKeywords =>
      'локальна мережа виявлення local network discovery';

  @override
  String get settingsExcludedAppsTitle => 'Виключені програми';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Пропускати автоматичний запис із виключених програм.';

  @override
  String get settingsExcludedAppsKeywords =>
      'приватність програма ідентифікатори privacy app identifiers';

  @override
  String get settingsExcludedAppsMacDescription =>
      'Пропускати автоматичний запис під час роботи з цими програмами. Копіювання у фоновому режимі може оминати виключення.';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Пропускати автоматичний запис від визначених власників буфера обміну. Якщо виключення задано, а джерело не вдається визначити, запис призупиняється.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Поки задано виключення, Android пропускає автоматичний запис, оскільки не може визначати програми-джерела.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'Виключення програм підтримуються в macOS, Windows, Android і Linux.';

  @override
  String get settingsExcludedAppsAdd => 'Додати';

  @override
  String get settingsExcludedAppsEmpty => 'Немає виключених програм.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return 'Вилучити $app';
  }

  @override
  String get settingsExcludeDialogTitle => 'Виключити програму';

  @override
  String get settingsExcludeDialogAction => 'Виключити';

  @override
  String get settingsExcludeDialogPlaceholder => 'Ідентифікатор програми';

  @override
  String get settingsExcludeFailed => 'Не вдалося виключити програму.';

  @override
  String get settingsSkipSecretTitle => 'Пропускати конфіденційні дані';

  @override
  String get settingsSkipSecretDescription =>
      'Не додавати до Історії дані, позначені програмою як секретні.';

  @override
  String get settingsSkipSecretKeywords =>
      'секрет пароль чутливі приховані спойлер secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Пропускати тимчасові дані';

  @override
  String get settingsSkipTransientDescription =>
      'Не додавати до Історії копії, позначені програмою як тимчасові.';

  @override
  String get settingsSkipTransientKeywords =>
      'короткочасні тимчасові приватність transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Блокувати знімки екрана';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Заборонити знімки й запис екрана для CopyPaste.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'Блокування знімків екрана недоступне на цій платформі.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'приватність екран знімок захист сполучення qr безпека код privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Сповіщення під час копіювання';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Показувати сповіщення після запису у фоновому режимі.';

  @override
  String get settingsNotificationPreviewTitle =>
      'Показувати вміст буфера обміну';

  @override
  String get settingsNotificationPreviewDescription =>
      'Додавати попередній перегляд кліпу до сповіщень про копіювання.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'сповіщення перегляд текст зображення приватність notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Звук під час копіювання';

  @override
  String get settingsSoundOnCopyDescription =>
      'Відтворювати системний звук після успішного запису.';

  @override
  String get settingsVersionTitle => 'Версія';

  @override
  String get settingsVersionDescription => 'Встановлена версія CopyPaste.';

  @override
  String get settingsVersionUnavailable => 'Недоступно';

  @override
  String get settingsUpdatesTitle => 'Оновлення програми';

  @override
  String get settingsUpdatesSearchDescription =>
      'Перевіряйте й встановлюйте оновлення CopyPaste.';

  @override
  String get settingsUpdatesKeywords =>
      'версія github випуск version github release';

  @override
  String get settingsUpdateIdle => 'Перевірте, чи є новіша версія CopyPaste.';

  @override
  String get settingsUpdateChecking => 'Перевірка оновлень.';

  @override
  String get settingsUpdateUpToDate => 'У вас найновіша версія CopyPaste.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'Версія $version найновіша.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'Доступна версія $available. На цьому пристрої встановлено версію $current.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Завантаження версії $version · $percent%.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Підготовка оновлень встановлених модулів.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Встановлення версії $version.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'Оновлення програми потребують уваги.';

  @override
  String get settingsUpdateNowAction => 'Оновити зараз';

  @override
  String get settingsUpdateOpenReleaseAction => 'Відкрити випуск';

  @override
  String get settingsUpdateCheckingAction => 'Перевірка';

  @override
  String get settingsUpdateDownloadingAction => 'Завантаження';

  @override
  String get settingsUpdateInstallingAction => 'Встановлення';

  @override
  String get settingsUpdateRestartAction => 'Перезапустити CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => 'Перевірити знову';

  @override
  String get settingsUpdateNoneTitle => 'Немає доступних оновлень';

  @override
  String get settingsUpdateLatest => 'У вас найновіша версія CopyPaste.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version — найновіша версія.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Доступне оновлення';

  @override
  String get settingsUpdateReady =>
      'Новіша версія CopyPaste готова до встановлення.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'Версія CopyPaste $version готова до встановлення.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Оновлення недоступне';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Це встановлення не оновлюється автоматично.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Не вдалося перевірити оновлення';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'CopyPaste не вдалося перевірити оновлення.';

  @override
  String get settingsQuickPasteSetupTitle => 'Налаштуйте Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Увімкніть підписаний супровідний компонент для GNOME або KDE і дозвольте керування клавіатурою, щоб повертатися до попередньої програми.';

  @override
  String get settingsQuickPasteDesktopIntegration =>
      'Інтеграція з робочим середовищем';

  @override
  String get settingsQuickPasteAllowKeyboard =>
      'Дозволити керування клавіатурою';

  @override
  String get settingsQuickPasteCheckAgain => 'Перевірити знову';

  @override
  String get settingsQuickPasteLoading =>
      'Завантаження налаштувань Quick Paste.';

  @override
  String get settingsQuickPasteUnavailableTitle =>
      'Функція Quick Paste недоступна';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'Цей пристрій не підтримує вікно Quick Paste.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Відкрити Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Налаштуйте глобальне скорочення для Quick Paste.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'клавіатура гаряча клавіша скорочення keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'Скорочення працює, поки CopyPaste запущено у фоновому режимі.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Зареєстроване скорочення: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Вставляти автоматично';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Вставляти вибраний кліп у попередню програму.';

  @override
  String get settingsAutoPasteKeywords =>
      'універсальний доступ автоматично accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Вибір кліпу повертає до попередньої програми й вставляє його.';

  @override
  String get settingsAccessibilityRequiredTitle =>
      'Потрібен дозвіл «Універсальний доступ»';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Поки дозвіл не надано, вибраний кліп копіюється без вставлення.';

  @override
  String get settingsQuickPasteAttentionTitle => 'Quick Paste потребує уваги';

  @override
  String get settingsShortcutRecording => 'Натисніть скорочення…';

  @override
  String get settingsShortcutNotSet => 'Не задано';

  @override
  String get settingsShortcutReset => 'Скинути скорочення';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count дня',
      many: '$count днів',
      few: '$count дні',
      one: '$count день',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Завантаження пристроїв…';

  @override
  String get devicesUnavailableTitle => 'Пристрої недоступні';

  @override
  String get devicesNearbyTitle => 'Пристрої поблизу';

  @override
  String get devicesScan => 'Сканувати';

  @override
  String get devicesScanning => 'Сканування…';

  @override
  String get devicesNoneNearby => 'Поблизу немає пристроїв';

  @override
  String get devicesThisDevice => 'Цей пристрій';

  @override
  String get devicesTrusted => 'Довірений';

  @override
  String get devicesConnect => 'Під’єднати';

  @override
  String devicesCardPing(String ping) {
    return 'Пінг: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Пінг:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'Перейменувати цей пристрій';

  @override
  String get devicesNamePlaceholder => 'Назва пристрою';

  @override
  String get devicesRenameFailed => 'Не вдалося зберегти назву пристрою.';

  @override
  String devicesRevokeDialogTitle(String device) {
    return 'Відкликати пристрій $device?';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return 'Розірвати сполучення з $device?';
  }

  @override
  String get devicesRevokeDialogMessage =>
      'Пристрій буде вилучено з вашої групи, а це сполучення — назавжди заблоковано. Інші пристрої застосують вилучення після повторного з’єднання.';

  @override
  String get devicesUnpairDialogMessage =>
      'Пристрій буде вилучено з вашої групи. Інші пристрої застосують вилучення після повторного з’єднання. Щоб додати його знову, знадобиться нове підтвердження.';

  @override
  String get devicesRevoke => 'Відкликати';

  @override
  String get devicesUnpair => 'Розірвати сполучення';

  @override
  String get devicesActionFailedMessage =>
      'Не вдалося виконати дію з пристроєм.';

  @override
  String get devicesCloseDetails => 'Закрити відомості про пристрій';

  @override
  String get devicesDetailType => 'Тип пристрою';

  @override
  String get devicesDetailOperatingSystem => 'Операційна система';

  @override
  String get devicesDetailModel => 'Модель';

  @override
  String get devicesDetailAppVersion => 'Версія CopyPaste';

  @override
  String get devicesDetailProtocolVersion => 'Версія протоколу';

  @override
  String get devicesDetailStatus => 'Стан';

  @override
  String get devicesDetailPing => 'Пінг';

  @override
  String get devicesDetailMeasured => 'Виміряно';

  @override
  String get devicesDetailLastSeen => 'Востаннє в мережі';

  @override
  String get devicesDetailLanEndpoint => 'Адреса в локальній мережі';

  @override
  String get devicesDetailDeviceId => 'ID пристрою';

  @override
  String get devicesDetailPairingId => 'ID сполучення';

  @override
  String get devicesDetailProfileProvenance => 'Походження профілю';

  @override
  String get devicesDetailProfileTrust => 'Довіра до профілю';

  @override
  String get devicesDetailProfileUpdated => 'Профіль оновлено';

  @override
  String get devicesValueUnavailable => 'Недоступно';

  @override
  String get devicesRenameDevice => 'Перейменувати пристрій';

  @override
  String get devicesSyncNow => 'Синхронізувати зараз';

  @override
  String get devicesRevokePairing => 'Відкликати сполучення';

  @override
  String get devicesLocal => 'Локальний';

  @override
  String get devicesTrustAuthenticated => 'Автентифіковано';

  @override
  String get devicesTrustUnverified => 'Не перевірено';

  @override
  String get devicesProvenanceSelfReported => 'Повідомлено пристроєм';

  @override
  String get devicesProvenanceObserved => 'Спостережено';

  @override
  String get devicesProvenanceMeasured => 'Виміряно';

  @override
  String get devicesConnectionOnline => 'У мережі';

  @override
  String get devicesConnectionOffline => 'Не в мережі';

  @override
  String get devicesConnectionVisible => 'Видимий';

  @override
  String get devicesConnectionNotSeen => 'Не виявлено';

  @override
  String get devicesConnectionUnknown => 'Невідомо';

  @override
  String get devicesPeerAvailable => 'Доступний';

  @override
  String get devicesPeerVisibleNow => 'Видимий зараз';

  @override
  String get devicesPeerStatusUnknown => 'Стан невідомий';

  @override
  String get devicesPingUnavailable => '— мс';

  @override
  String get devicesPingUnderOneMillisecond => '<1 мс';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds мс';
  }

  @override
  String get devicesClassDesktop => 'Настільний комп’ютер';

  @override
  String get devicesClassLaptop => 'Ноутбук';

  @override
  String get devicesClassPhone => 'Телефон';

  @override
  String get devicesClassTablet => 'Планшет';

  @override
  String get devicesClassUnknown => 'Пристрій';

  @override
  String get devicesUnknownOs => 'Невідома ОС';

  @override
  String get devicesAddDevice => 'Додати пристрій';

  @override
  String get devicesScanQr => 'Сканувати QR';

  @override
  String get devicesEnterCode => 'Ввести код';

  @override
  String get devicesScannerUnavailableTitle => 'Сканер недоступний';

  @override
  String get devicesActionFailedTitle => 'Дію не виконано';

  @override
  String get devicesRefreshFailedTitle => 'Не вдалося оновити пристрої';

  @override
  String get devicesNearbyPairingUnavailable =>
      'Сполучення з пристроями поблизу недоступне.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'Сполучення з пристроями поблизу недоступне. Скористайтеся кнопкою «Додати пристрій», щоб показати код сполучення.';

  @override
  String get pairingTitle => 'Сполучення';

  @override
  String get pairingPairDevice => 'Сполучити пристрій';

  @override
  String get pairingScanQrCode => 'Сканувати QR-код';

  @override
  String get pairingEnterPairingCode => 'Ввести код сполучення';

  @override
  String get pairingConnectNearbyDevice => 'Під’єднати пристрій поблизу';

  @override
  String get pairingSendingRequest => 'Надсилання запиту на з’єднання…';

  @override
  String get pairingEmptyTitle => 'Виберіть дію сполучення';

  @override
  String get pairingEmptyMessage =>
      'Тут з’являться елементи керування сполученням.';

  @override
  String get pairingCreatingInvitation => 'Створення запрошення до сполучення.';

  @override
  String get pairingCode => 'Код сполучення';

  @override
  String get pairingCodeHint =>
      'Введіть 8-символьний код, показаний на іншому пристрої.';

  @override
  String get pairingDeviceAddress => 'Адреса пристрою';

  @override
  String get pairingDeviceAddressHint =>
      'Використайте адресу host:port, яку показує інший пристрій.';

  @override
  String get pairingCheckDetailsTitle => 'Перевірте дані сполучення';

  @override
  String get pairingActionFailed => 'Не вдалося виконати дію сполучення';

  @override
  String get pairingConnectingDevice => 'Під’єднання пристрою…';

  @override
  String get pairingQrScannerTitle => 'Сканер QR-кодів';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'Тут з’явиться зображення з камери.';

  @override
  String get pairingScannerNeedsAttention => 'Сканер потребує уваги';

  @override
  String get pairingCodeIncomplete =>
      'Введіть повний 8-символьний код сполучення.';

  @override
  String get pairingAddressMissing =>
      'Введіть адресу пристрою у форматі host:port.';

  @override
  String get pairingWaitingForOtherDevice => 'Очікування іншого пристрою';

  @override
  String get pairingInviteInstructions =>
      'На іншому пристрої виберіть «Додати пристрій → Ввести код» або відскануйте QR-код.';

  @override
  String get pairingDetailsTitle => 'Дані сполучення';

  @override
  String get pairingAddress => 'Адреса';

  @override
  String get pairingVerificationCode => 'Код перевірки';

  @override
  String get pairingVerificationCodeHint =>
      'Підтверджуйте, лише якщо цей код збігається на обох пристроях.';

  @override
  String get pairingReject => 'Відхилити';

  @override
  String get pairingConfirm => 'Підтвердити';

  @override
  String get pairingAccept => 'Прийняти';

  @override
  String get pairingJoinDevice => 'Приєднатися';

  @override
  String get pairingStartScanner => 'Запустити сканер';

  @override
  String get pairingActionNotStarted => 'Не вдалося розпочати сполучення.';

  @override
  String get pairingStateHandshaking => 'Перевірка пристрою';

  @override
  String get pairingStateAwaitingConfirmation =>
      'Підтвердьте на обох пристроях';

  @override
  String get pairingStateConfirmed => 'Пристрій сполучено';

  @override
  String get pairingStateRejected => 'Сполучення відхилено';

  @override
  String get pairingStateCancelled => 'Сполучення скасовано';

  @override
  String get pairingStateTimedOut => 'Час сполучення минув';

  @override
  String get pairingStateFailed => 'Не вдалося сполучити';

  @override
  String get pairingHandshakingDescription =>
      'Пристрої встановлюють захищений канал.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Порівняйте захищений код перевірки, а потім підтвердьте на обох пристроях.';

  @override
  String get pairingConfirmedDescription =>
      'Пристрій під’єднано. Членство у вашій групі синхронізуватиметься автоматично.';

  @override
  String get pairingNoDeviceAdded => 'Пристрій не додано.';

  @override
  String get pairingTimedOutDescription =>
      'Почніть нову спробу сполучення, щоб продовжити.';

  @override
  String get pairingIdleDescription =>
      'Почніть спробу сполучення, щоб під’єднати пристрій.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'Захищене сполучення недоступне';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Цей сеанс сполучення має відкриватися в окремому захищеному вікні.';

  @override
  String get pairingCancel => 'Скасувати сполучення';

  @override
  String get pairingOrEnterCode => 'Або введіть код сполучення';

  @override
  String get pairingJoinSecurely => 'Безпечно приєднатися';

  @override
  String get pairingProtectedWaitingDescription =>
      'Покажіть QR-код, відскануйте QR-код іншого пристрою або введіть код сполучення в цьому захищеному вікні.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Діє ще $seconds с';
  }

  @override
  String get pairingInvalidQr => 'Відскануйте QR-код сполучення CopyPaste.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Сканер Google недоступний. Натомість введіть код сполучення.';

  @override
  String get pairingScannerCouldNotOpen =>
      'Не вдалося відкрити сканер. Натомість введіть код сполучення.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'Захищене відображення сполучення недоступне.';

  @override
  String get pairingVerificationCodeRequired =>
      'Спочатку покажіть і порівняйте код перевірки.';

  @override
  String get syncStatusUnavailable => 'Стан синхронізації недоступний';

  @override
  String get syncStatusOff => 'Синхронізацію вимкнено';

  @override
  String get syncStatusWaitingForSync => 'Очікування синхронізації';

  @override
  String get syncStatusSyncing => 'Синхронізація';

  @override
  String get syncStatusUpToDate => 'Синхронізовано';

  @override
  String get syncStatusInterrupted => 'Синхронізацію перервано';

  @override
  String get syncStatusPartiallySynchronized => 'Частково синхронізовано';

  @override
  String get syncStatusRetrying => 'Повторна спроба';

  @override
  String get syncStatusWaitingToSync => 'Очікує синхронізації';

  @override
  String get syncStatusWaitingForDevice => 'Очікування пристрою';

  @override
  String get syncStatusNoPairedDevices => 'Немає сполучених пристроїв';

  @override
  String get syncStatusDevicesOffline => 'Пристрої не в мережі';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · журнал активності недоступний';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Синхронізацію вимкнено · $count проблеми',
      many: 'Синхронізацію вимкнено · $count проблем',
      few: 'Синхронізацію вимкнено · $count проблеми',
      one: 'Синхронізацію вимкнено · $count проблема',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count не в мережі',
      many: '$status · $count не в мережі',
      few: '$status · $count не в мережі',
      one: '$status · $count не в мережі',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count пристрою потребують уваги',
      many: '$count пристроїв потребують уваги',
      few: '$count пристрої потребують уваги',
      one: '$count пристрій потребує уваги',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Пристрої не в мережі · $count проблеми',
      many: 'Пристрої не в мережі · $count проблем',
      few: 'Пристрої не в мережі · $count проблеми',
      one: 'Пристрої не в мережі · $count проблема',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Перевірка з’єднання';

  @override
  String get syncConnectionUnknown => 'Стан з’єднання невідомий';

  @override
  String get syncLogDeviceOffline => 'Пристрій вийшов з мережі.';

  @override
  String get syncLogConnectionRestored => 'З’єднання відновлено.';

  @override
  String get syncLogProblemDetected => 'Виявлено проблему синхронізації.';

  @override
  String get syncLogProblemResolved => 'Проблему синхронізації усунено.';

  @override
  String get syncLogFailed => 'Помилка синхронізації.';

  @override
  String get syncLogRecovered => 'Синхронізацію відновлено.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent кліпу',
      many: '$sent кліпів',
      few: '$sent кліпи',
      one: '$sent кліп',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received кліпу',
      many: '$received кліпів',
      few: '$received кліпи',
      one: '$received кліп',
    );
    return 'Надіслано $_temp0 · Отримано $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Синхронізація: $status';
  }

  @override
  String get syncDetailsTitle => 'Синхронізація';

  @override
  String get syncDetailsClose => 'Закрити відомості про синхронізацію';

  @override
  String get syncNoPairedDevicesMessage =>
      'Сполучіть пристрій, щоб синхронізувати історію буфера обміну.';

  @override
  String get syncActivityLogUnavailableTitle => 'Журнал активності недоступний';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Не вдалося прочитати або зберегти активність синхронізації.';

  @override
  String get syncDetailsDevicesHeading => 'Пристрої';

  @override
  String get syncDetailsActivityHeading => 'Активність';

  @override
  String get syncDetailsNoRecentProblems =>
      'Останнім часом проблем із синхронізацією не було';

  @override
  String get syncDetailsUnknownPeer => 'Сполучений пристрій';

  @override
  String get syncPeerConnection => 'З’єднання';

  @override
  String get syncPeerLastCheckStarted => 'Початок останньої перевірки';

  @override
  String get syncPeerItemsOverSizeLimit => 'Елементи понад обмеження розміру';

  @override
  String syncPeerLastSeen(String time) {
    return 'Востаннє в мережі $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Остання синхронізація $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Остання успішна синхронізація';

  @override
  String get syncPeerConnectionDetails => 'Відомості про з’єднання';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Відомості про з’єднання з $device';
  }

  @override
  String get historyKindText => 'Текст';

  @override
  String get historyKindLink => 'Посилання';

  @override
  String get historyKindEmail => 'Ел. пошта';

  @override
  String get historyKindColor => 'Колір';

  @override
  String get historyKindPhone => 'Телефон';

  @override
  String get historyKindCode => 'Код';

  @override
  String get historyKindPath => 'Шлях';

  @override
  String get historyKindCredentials => 'Облікові дані';

  @override
  String get historyKindImage => 'Зображення';

  @override
  String get historyKindFile => 'Файл';

  @override
  String get historyKindOther => 'Інше';

  @override
  String get historySortNewest => 'Спочатку нові';

  @override
  String get historySortOldest => 'Спочатку старі';

  @override
  String get historySortRelevance => 'За релевантністю';

  @override
  String get historyErrorUpdatesUnavailable =>
      'Оновлення історії тимчасово недоступні.';

  @override
  String get historyErrorBulkDelete =>
      'Не вдалося видалити деякі вибрані кліпи. Спробуйте ще раз.';

  @override
  String get historyErrorBulkPin =>
      'Не вдалося змінити закріплення деяких вибраних кліпів. Спробуйте ще раз.';

  @override
  String get historyErrorRefresh =>
      'Не вдалося оновити історію. Спробуйте ще раз.';

  @override
  String get historyErrorLoad =>
      'Не вдалося завантажити історію. Спробуйте ще раз.';

  @override
  String get historyErrorLoadMore => 'Не вдалося завантажити більше історії.';

  @override
  String get historyErrorLoadEarlier =>
      'Не вдалося завантажити попередню історію.';

  @override
  String get historyErrorOpenClip => 'Не вдалося відкрити цей кліп.';

  @override
  String get historyErrorCopyPlainText =>
      'Не вдалося скопіювати звичайний текст.';

  @override
  String get historyErrorCopyClip => 'Не вдалося скопіювати цей кліп.';

  @override
  String get historyErrorCopyText => 'Не вдалося скопіювати цей текст.';

  @override
  String get historyErrorPrepareFile =>
      'Не вдалося завантажити цей файл. Перевірте пристрій-джерело й спробуйте ще раз.';

  @override
  String get historyErrorDownloadFile => 'Не вдалося завантажити цей файл.';

  @override
  String get historyErrorSelectFiles =>
      'Не вдалося вибрати файли. Спробуйте ще раз.';

  @override
  String get historyErrorReadSelectedFile =>
      'Не вдалося прочитати вибраний файл. Перевірте доступ і вільне місце.';

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
          'Не вдалося імпортувати $failed з $total файлу. Перший: $name. $reason',
      many:
          'Не вдалося імпортувати $failed з $total файлів. Перший: $name. $reason',
      few:
          'Не вдалося імпортувати $failed з $total файлів. Перший: $name. $reason',
      one:
          'Не вдалося імпортувати $failed з $total файлу. Перший: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'Не вдалося змінити конфіденційність кліпу.';

  @override
  String get historyErrorPinUpdate => 'Не вдалося змінити закріплення.';

  @override
  String get historyErrorPinnedOrderSave =>
      'Не вдалося зберегти порядок закріплених кліпів. Спробуйте ще раз.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'Порядок збережено, але не вдалося оновити історію.';

  @override
  String get historyErrorDeleteClip => 'Не вдалося видалити цей кліп.';

  @override
  String get historyErrorDeleteAll => 'Не вдалося видалити історію.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'Не вдалося видалити вибрані кліпи.';

  @override
  String get historyOcrErrorUnavailable =>
      'Розпізнавання тексту недоступне. Перевірте модуль у Налаштуваннях і спробуйте ще раз.';

  @override
  String get historyOcrErrorStart =>
      'Не вдалося запустити розпізнавання тексту. Спробуйте ще раз.';

  @override
  String get historyOcrErrorRecognize =>
      'Не вдалося розпізнати текст на зображенні. Спробуйте ще раз.';

  @override
  String get historyOcrRecognizing => 'Розпізнавання тексту на зображенні.';

  @override
  String get historyOcrFailedTitle => 'Помилка розпізнавання тексту';

  @override
  String get historyOcrNoText => 'Текст не знайдено';

  @override
  String get historyOcrTitle => 'Текст на зображенні';

  @override
  String get historyOcrTextCopied => 'Розпізнаний текст скопійовано.';

  @override
  String get historyOcrCopyError =>
      'Не вдалося скопіювати текст. Спробуйте ще раз.';

  @override
  String get historyTagsErrorLoad =>
      'Не вдалося завантажити теги. Спробуйте ще раз.';

  @override
  String get historyTagsErrorSave =>
      'Не вдалося зберегти зміну тегу. Використайте унікальну назву до 64 символів і виберіть закріплені кліпи.';

  @override
  String get historyTags => 'Теги';

  @override
  String get historyTagsUpdateErrorTitle => 'Не вдалося оновити теги';

  @override
  String get historyTagsEmpty => 'Тегів ще немає';

  @override
  String get historyTagRename => 'Перейменувати';

  @override
  String get historyTagActions => 'Дії з тегом';

  @override
  String get historyTagNamePlaceholder => 'Назва тегу';

  @override
  String get historyCreateTag => 'Створити тег';

  @override
  String get historySaveTag => 'Зберегти тег';

  @override
  String get historyTagsFilterAll => 'Усі теги';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count тегу',
      many: '$count тегів',
      few: '$count теги',
      one: '$count тег',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Фільтр за тегами: $selection';
  }

  @override
  String get historyManageTags => 'Керувати тегами';

  @override
  String get historyTagsFilterSemantics =>
      'Фільтрувати закріплені кліпи за тегами';

  @override
  String get historyDownloadErrorCancelled => 'Завантаження файлу скасовано.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'Служба завантаження недоступна. Відкрийте CopyPaste знову й спробуйте ще раз.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'Не вдалося під’єднатися до служби завантаження. Спробуйте ще раз.';

  @override
  String get historyFileErrorAccess => 'Немає доступу до вибраного файлу.';

  @override
  String get historySpoilerRevealSemantics =>
      'Конфіденційний вміст. Розкрити спойлер';

  @override
  String get historyActionFailedTitle => 'Дію не виконано';

  @override
  String get historyLoadingMessage => 'Завантаження історії буфера обміну.';

  @override
  String get historyUnavailableTitle => 'Історія недоступна';

  @override
  String get historyEmptyTitle => 'Кліпів не знайдено';

  @override
  String get historyEmptyMessage =>
      'Тут з’являтимуться записані кліпи й відповідні результати з історії.';

  @override
  String get historyAttentionTitle => 'Історія потребує уваги';

  @override
  String get historyLoadMore => 'Завантажити ще';

  @override
  String get historySectionPinned => 'Закріплені';

  @override
  String get historySectionToday => 'Сьогодні';

  @override
  String get historySectionYesterday => 'Учора';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Пропущено $count елемента буфера обміну',
      many: 'Пропущено $count елементів буфера обміну',
      few: 'Пропущено $count елементи буфера обміну',
      one: 'Пропущено $count елемент буфера обміну',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Деякі збережені кліпи не вдалося прочитати, тому їх не показано в цьому списку.';

  @override
  String get historySearch => 'Пошук в історії';

  @override
  String get historyResetFilters => 'Скинути фільтри';

  @override
  String get historyImportFiles => 'Імпортувати файли';

  @override
  String get historySelectClips => 'Вибрати кліпи';

  @override
  String get historyCloseSearch => 'Закрити пошук';

  @override
  String get historyFilterAllClips => 'Усі кліпи';

  @override
  String get historyFilterAllDevices => 'Усі пристрої';

  @override
  String get historyFilterAllApps => 'Усі програми';

  @override
  String get historyPinClip => 'Закріпити кліп';

  @override
  String get historyUnpinClip => 'Відкріпити кліп';

  @override
  String get historyReorderPinnedClip => 'Змінити порядок закріпленого кліпу';

  @override
  String get historyReorderPinnedHint =>
      'Перетягніть, щоб перемістити. Alt + стрілка вгору або вниз переміщує на одну позицію.';

  @override
  String get historyClipUnavailable => 'Кліп недоступний';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Тип кліпу: $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Виберіть кліп';

  @override
  String get historyInspectorEmptyMessage =>
      'Тут з’являться його повний вміст і дії.';

  @override
  String get historyNoPreview => 'Попередній перегляд недоступний.';

  @override
  String get historyCloseClipDetails => 'Закрити відомості про кліп';

  @override
  String get historyCopyOptions => 'Параметри копіювання';

  @override
  String get historyCopyPlainText => 'Скопіювати як звичайний текст';

  @override
  String get historyManageClipTags => 'Керувати тегами кліпу';

  @override
  String get historyPinnedButton => 'Закріплено';

  @override
  String get historyAddSpoiler => 'Додати спойлер';

  @override
  String get historyRemoveSpoiler => 'Прибрати спойлер';

  @override
  String get historyUpdateFailedTitle => 'Не вдалося оновити';

  @override
  String get historyRecognizeImageText => 'Розпізнати текст на зображенні';

  @override
  String get historyCopiedTitle => 'Скопійовано';

  @override
  String get historyCopyFailedTitle => 'Не вдалося скопіювати';

  @override
  String get historyPlainTextCopied => 'Звичайний текст скопійовано.';

  @override
  String get historyClipCopied => 'Кліп скопійовано.';

  @override
  String get historyDownloadedTitle => 'Завантажено';

  @override
  String get historyFileSaved => 'Файл збережено.';

  @override
  String get historyDownload => 'Завантажити';

  @override
  String get historyDownloading => 'Завантаження…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Завантаження $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => 'Скасування…';

  @override
  String get historyDownloadCancellingTooltip => 'Скасування завантаження';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Завантаження $percent% · Натисніть, щоб скасувати';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Не вдалося завантажити · Натисніть, щоб повторити';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Скасувати завантаження, виконано $percent%';
  }

  @override
  String get historyRetryDownload => 'Повторити завантаження';

  @override
  String get historyDownloadFailedTitle => 'Не вдалося завантажити';

  @override
  String get historyShowFile => 'Показати файл';

  @override
  String get historyMetadataPrivacy => 'Приватність';

  @override
  String get historyMetadataConfidential => 'Конфіденційно';

  @override
  String get historyMetadataCaptured => 'Записано';

  @override
  String get historyMetadataObservedApp => 'Програма-джерело';

  @override
  String get historyMetadataDevice => 'Пристрій';

  @override
  String get historyMetadataClipType => 'Тип кліпу';

  @override
  String get historyMetadataType => 'Тип';

  @override
  String get historyMetadataPath => 'Шлях';

  @override
  String get historyMetadataFile => 'Файл';

  @override
  String get historyMetadataResolution => 'Роздільна здатність';

  @override
  String get historyMetadataSize => 'Розмір';

  @override
  String get historyMetadataFiles => 'Файли';

  @override
  String get historyMetadataSync => 'Синхронізація';

  @override
  String get historyMetadataTooLargeToSync => 'Завеликий для синхронізації';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Видалити $count вибраного кліпу?',
      many: 'Видалити $count вибраних кліпів?',
      few: 'Видалити $count вибрані кліпи?',
      one: 'Видалити $count вибраний кліп?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Видалити цей кліп?';

  @override
  String get historyBulkTagTooltip =>
      'Додати теги до вибраних закріплених кліпів';

  @override
  String get historyBulkExit => 'Вийти з режиму вибору';

  @override
  String get historyBulkPin => 'Закріпити вибрані';

  @override
  String get historyBulkUnpin => 'Відкріпити вибрані';

  @override
  String get historyBulkDelete => 'Видалити вибрані';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Вибрано $count',
      many: 'Вибрано $count',
      few: 'Вибрано $count',
      one: 'Вибрано $count',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Фільтр за датою';

  @override
  String get historyDatePresetToday => 'Сьогодні';

  @override
  String get historyDatePresetYesterday => 'Учора';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Останні $days дня',
      many: 'Останні $days днів',
      few: 'Останні $days дні',
      one: 'Останній $days день',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'За весь час';

  @override
  String get appRuntimeStartFailed =>
      'CopyPaste не вдалося запустити середовище виконання.';

  @override
  String get appPreparing => 'Підготовка CopyPaste.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste очікує доступу до захищеної історії. Доступними частинами програми можна користуватися.';

  @override
  String get appHistoryUnavailableTitle => 'Середовище історії недоступне';

  @override
  String get appHistoryUnavailableMessage =>
      'Запустіть середовище виконання програми, перш ніж відкривати історію.';

  @override
  String get appDevicesUnavailableTitle => 'Середовище пристроїв недоступне';

  @override
  String get appDevicesUnavailableMessage =>
      'Запустіть середовище виконання програми, перш ніж відкривати пристрої.';

  @override
  String get appSettingsUnavailableTitle => 'Середовище налаштувань недоступне';

  @override
  String get appSettingsUnavailableMessage =>
      'Запустіть середовище виконання програми, перш ніж відкривати налаштування.';

  @override
  String get appQuit => 'Вийти з CopyPaste';

  @override
  String get appWindowSetupTitle => 'Налаштування вікна потребує уваги';

  @override
  String get appOpenHistory => 'Відкрити Історію';

  @override
  String get appDestinationHistory => 'Історія';

  @override
  String get appDestinationDevices => 'Пристрої';

  @override
  String get appDestinationSettings => 'Налаштування';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, вибрано';
  }

  @override
  String get appActiveFilter => 'Активний фільтр';

  @override
  String get appActionFailed => 'Дію не виконано';

  @override
  String get sharedAndroidVersionRequired =>
      'Потрібна версія Android 11 або новіша';

  @override
  String get sharedUseAdbTab => 'Скористайтеся вкладкою ADB на цьому пристрої.';

  @override
  String get sharedOneTimeAccessApplied => 'Одноразовий доступ надано';

  @override
  String get sharedInstallShizuku => 'Встановити Shizuku';

  @override
  String get sharedPairAndStartShizuku => 'Сполучити й запустити Shizuku';

  @override
  String get sharedAllowCopyPaste => 'Дозволити CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded => 'Більше Shizuku не знадобиться.';

  @override
  String get sharedGetShizukuDescription =>
      'Завантажте Shizuku, щоб надати одноразовий доступ.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Скористайтеся «Налагодженням через Wi-Fi» у Shizuku, а потім поверніться.';

  @override
  String get sharedApproveInShizuku =>
      'Один раз дозвольте CopyPaste у Shizuku.';

  @override
  String get sharedGetShizuku => 'Завантажити Shizuku';

  @override
  String get sharedOpenShizuku => 'Відкрити Shizuku';

  @override
  String get sharedRunOnComputer => 'Виконайте на комп’ютері';

  @override
  String get sharedCopyAllCommands => 'Скопіювати всі команди';

  @override
  String get sharedEnableUsbDebugging =>
      'Увімкніть «Налагодження USB» і під’єднайте цей телефон.';

  @override
  String get sharedAllow => 'Дозволити';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Не вдалося скопіювати команди налаштування.';

  @override
  String get sharedShizukuOpenFailed => 'Не вдалося відкрити Shizuku.';

  @override
  String sharedSizeBytes(String size) {
    return '$size Б';
  }

  @override
  String get onboardingTitle => 'Налаштування CopyPaste';

  @override
  String get onboardingWelcomeTitle => 'Вітаємо в CopyPaste';

  @override
  String get onboardingWelcomeSubtitle => 'Ваш буфер обміну завжди напоготові.';

  @override
  String get onboardingReadyTitle => 'CopyPaste налаштовано';

  @override
  String get onboardingGetStarted => 'Почати';

  @override
  String get onboardingOptionalSetupTitle =>
      'Додаткове налаштування потребує уваги';

  @override
  String get onboardingOpenSystemSettings => 'Відкрити налаштування';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return 'КРОК $step З $count · $platform';
  }

  @override
  String get onboardingSetupErrorTitle => 'Налаштування потребує уваги';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste не вдалося прочитати стан налаштування $platform.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste не вдалося зберегти поступ початкового налаштування.';

  @override
  String get onboardingStartAtLogin => 'Запуск під час входу';

  @override
  String get onboardingStartAtLoginDescription =>
      'Відкривати CopyPaste під час входу в систему';

  @override
  String get onboardingStartAtLoginSemantics =>
      'Запускати CopyPaste під час входу';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Не вдалося ввімкнути запуск під час входу. Можна продовжити без нього.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Не вдалося змінити запуск під час входу. Можна продовжити без нього.';

  @override
  String get onboardingMacosTitle => 'Налаштуйте Mac';

  @override
  String get onboardingAccessibility => 'Універсальний доступ';

  @override
  String get onboardingAutoPasteEnabled => 'Автовставлення ввімкнено';

  @override
  String get onboardingAutoPasteOptional => 'Необов’язково, для автовставлення';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Доступно у встановленій програмі CopyPaste.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'Потрібно схвалити елемент входу';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Дозвольте CopyPaste у Системних параметрах, а потім поверніться.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste не вдалося прочитати стан налаштування macOS.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Не вдалося запитати дозвіл «Універсальний доступ».';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste не вдалося оновити стан налаштування macOS.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Не вдалося оновити стан дозволу «Універсальний доступ».';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Не вдалося відкрити налаштування «Елементи входу».';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'Запуск під час входу потребує схвалення в Системних параметрах.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Не вдалося вимкнути запуск під час входу.';

  @override
  String get onboardingWindowsTitle => 'Налаштуйте ПК';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'Недоступно для цього встановлення. Можна продовжити без цього.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Вимкнено в Диспетчері завдань';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Увімкніть CopyPaste у списку програм автозавантаження Windows, а потім поверніться.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'Запуск під час входу вимкнено в Диспетчері завдань. Його можна ввімкнути пізніше в Настройках Windows.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'Запуск під час входу недоступний. Можна продовжити без нього.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste не вдалося прочитати стан налаштування Windows.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Не вдалося відкрити налаштування автозавантаження Windows.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Не вдалося змінити запуск під час входу. Це можна змінити пізніше в Налаштуваннях.';

  @override
  String get onboardingLinuxChecking =>
      'Перевірка сеансу робочого середовища Linux.';

  @override
  String get onboardingLinuxTitle => 'Налаштуйте робоче середовище Linux';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste потребує перевіреної інтеграції з робочим середовищем, щоб визначати джерело кліпів і підтримувати Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Сеанс робочого середовища';

  @override
  String get onboardingLinuxX11Detected => 'Виявлено сеанс X11.';

  @override
  String get onboardingLinuxWaylandDetected => 'Виявлено сеанс Wayland.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'Не виявлено підтримуваного сеансу X11 або Wayland.';

  @override
  String get onboardingLinuxCompanion => 'Супровідний компонент GNOME або KDE';

  @override
  String get onboardingLinuxCompanionReady =>
      'Автентифікований міст Clipboard v2 готовий.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'Запис буфера обміну призупинено через виключення програм-джерел. Увімкніть підписаний супровідний компонент, щоб відновити його.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Встановіть і ввімкніть підписаний пакет сеансу супровідного компонента, а потім поверніться сюди.';

  @override
  String get onboardingLinuxOpenSetup => 'Налаштувати';

  @override
  String get onboardingLinuxKeyboardControl => 'Керування клавіатурою';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'Керування клавіатурою дозволено для Quick Paste.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'Керування клавіатурою недоступне в цьому сеансі робочого середовища.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Дозволити керування клавіатурою';

  @override
  String get onboardingLinuxQuickPasteReady => 'Функція Quick Paste готова.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Функція Quick Paste запрацює, щойно буде активовано потрібну інтеграцію з робочим середовищем.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'Відкривати CopyPaste під час входу в систему.';

  @override
  String get onboardingLinuxLinks => 'Посилання CopyPaste';

  @override
  String get onboardingLinuxLinksRegistered =>
      'Посилання CopyPaste відкриваються в цій програмі.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Зареєструйте посилання CopyPaste для запрошень до сполучення.';

  @override
  String get onboardingLinuxRegisterLinks => 'Зареєструвати посилання';

  @override
  String get onboardingLinuxScreenshotBlocking => 'Блокування знімків екрана';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Блокування знімків екрана недоступне в Linux. Захищені збережені дані залишаються зашифрованими.';

  @override
  String get onboardingLinuxCheckAgain => 'Перевірити знову';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste не вдалося прочитати стан інтеграції з Linux.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Не вдалося відкрити налаштування інтеграції з робочим середовищем.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'Дозвіл на керування клавіатурою не надано.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Не вдалося змінити запуск під час входу.';

  @override
  String get onboardingLinuxLinksFailed =>
      'Не вдалося зареєструвати посилання CopyPaste.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Не вдалося оновити стан інтеграції з Linux.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'У цьому сеансі X11 немає доступу до буфера обміну.';

  @override
  String get onboardingLinuxSessionRequired =>
      'Для CopyPaste потрібен сеанс робочого середовища X11 або Wayland.';

  @override
  String get onboardingAndroidFullCapture => 'Повний запис';

  @override
  String get onboardingAndroidBackgroundCapture => 'Фоновий запис';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Налаштуйте один раз, а потім перевірте справжнім копіюванням.';

  @override
  String get onboardingAndroidLimitedCapture => 'Обмежений запис';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'Поділіться в CopyPaste або поверніться після копіювання.';

  @override
  String get onboardingAndroidNotification => 'Сповіщення про запис';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Потрібне під час запису';

  @override
  String get onboardingAndroidBackgroundActivity => 'Фонова активність';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Рекомендовано, щоб зменшити втрати під час запису';

  @override
  String get onboardingAndroidOpen => 'Відкрити';

  @override
  String get onboardingAndroidApplyAccess => 'Надати доступ до запису';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Надати одноразовий доступ до запису.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Повний запис можна налаштувати пізніше.';

  @override
  String get onboardingAndroidCaptureWorking => 'Фоновий запис працює';

  @override
  String get onboardingAndroidVerifyCopy => 'Перевірте справжнім копіюванням';

  @override
  String get onboardingAndroidCopyReached =>
      'Нова копія з’явилася в локальній Історії.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Скопіюйте щось в іншій програмі, а потім поверніться сюди.';

  @override
  String get onboardingAndroidStartCapture => 'Почати запис';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste не вдалося прочитати стан налаштування Android.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Не вдалося оновити стан запису Android.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Відстеження налаштування Android недоступне.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'Не вдалося ввімкнути приймання буфера обміну.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Не вдалося зупинити фоновий запис.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Не вдалося відкрити налаштування акумулятора.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste дозволено в Shizuku, але не вдалося застосувати дозволи Android для запису.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Дозвольте CopyPaste у Shizuku, щоб застосувати дозволи для запису.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Спочатку надайте фоновий доступ до буфера обміну.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Дозвольте сповіщення, перш ніж починати фоновий запис.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Не вдалося почати фоновий запис.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Не вдалося відкрити налаштування запису Android.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Не вдалося зберегти вибір налаштування Android.';

  @override
  String get modulesMarketplaceTab => 'Каталог';

  @override
  String get modulesInstalledTab => 'Встановлені';

  @override
  String get modulesRefresh => 'Оновити';

  @override
  String get modulesSearchPlaceholder => 'Пошук модулів';

  @override
  String get modulesOperationFailed => 'Не вдалося виконати операцію з модулем';

  @override
  String get modulesLoading => 'Завантаження модулів.';

  @override
  String get modulesUnavailableTitle => 'Модулі недоступні';

  @override
  String get modulesUnavailableMessage =>
      'Спробуйте завантажити модулі ще раз.';

  @override
  String get modulesNoneInstalled => 'Немає встановлених модулів';

  @override
  String get modulesNoMatches => 'Немає відповідних модулів';

  @override
  String get modulesChooseFromMarketplace => 'Виберіть модуль у каталозі.';

  @override
  String get modulesTryDifferentSearch => 'Спробуйте інший запит.';

  @override
  String get modulesMarketplaceLoading => 'Завантаження каталогу.';

  @override
  String get modulesMarketplaceUnavailableTitle => 'Каталог недоступний';

  @override
  String get modulesMarketplaceUnavailableMessage =>
      'Спробуйте завантажити каталог ще раз.';

  @override
  String get modulesNoneAvailable => 'Немає доступних модулів';

  @override
  String get modulesNoneAvailableMessage =>
      'Модулі з’являться тут після публікації.';

  @override
  String modulesEnableModule(String module) {
    return 'Увімкнути $module';
  }

  @override
  String get modulesInstall => 'Встановити';

  @override
  String modulesUpdateTo(String version) {
    return 'Оновити до $version';
  }

  @override
  String get modulesRemoveModule => 'Вилучити модуль';

  @override
  String get modulesModuleSettings => 'Налаштування модуля';

  @override
  String modulesSettingsTitle(String module) {
    return 'Налаштування модуля $module';
  }

  @override
  String get modulesDownloadingModel => 'Завантаження мовної моделі…';

  @override
  String get modulesRun => 'Виконати';

  @override
  String modulesRemoveTitle(String module) {
    return 'Вилучити $module?';
  }

  @override
  String get modulesRemoveMessage =>
      'Модуль і його локальні налаштування буде вилучено.';

  @override
  String get modulesRemove => 'Вилучити';

  @override
  String get modulesRemoveFailed => 'Не вдалося вилучити модуль.';

  @override
  String get modulesCloseSettings => 'Закрити налаштування модуля';

  @override
  String get modulesRemoved => 'Модуль вилучено';

  @override
  String get modulesNoSettingsTitle => 'Немає додаткових налаштувань';

  @override
  String get modulesNoSettingsMessage =>
      'Цей модуль не потребує додаткового налаштування.';

  @override
  String get modulesSetUpSmsAccess => 'Налаштувати доступ до SMS';

  @override
  String get modulesRestartApp => 'Перезапустити CopyPaste';

  @override
  String get modulesSmsAccessTitle => 'Доступ до SMS';

  @override
  String get modulesCloseSmsAccess => 'Закрити панель доступу до SMS';

  @override
  String get modulesSmsChecking => 'Перевірка доступу до SMS.';

  @override
  String get modulesSmsReady =>
      'Доступ готовий. Увімкніть модуль SMS Codes, щоб почати копіювати нові коди.';

  @override
  String get modulesSmsAllowAccess =>
      'Надайте доступ, щоб копіювати коди з нових SMS.';

  @override
  String get modulesSmsNotification => 'Сповіщення SMS';

  @override
  String get modulesSmsNotificationDescription =>
      'Потрібне під час роботи SMS Codes';

  @override
  String get modulesSmsApplyAccess => 'Надати доступ до SMS';

  @override
  String get modulesSmsApplyAccessDescription =>
      'Надати одноразовий доступ до SMS.';

  @override
  String get modulesSmsSetupFailed => 'Не вдалося налаштувати SMS';

  @override
  String get modulesFileSelectionFailed => 'Не вдалося вибрати файл';

  @override
  String get modulesChooseLanguages => 'Виберіть мови';

  @override
  String get modulesChooseFile => 'Вибрати файл';

  @override
  String modulesModelReady(String model) {
    return 'Модель $model готова';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return 'Модель $model · завантаження $size МіБ';
  }

  @override
  String get modulesPlatformsUnknown => 'Платформи: невідомо';

  @override
  String modulesPlatforms(String platforms) {
    return 'Платформи: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'Підпис каталогу модулів недійсний.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Не вдалося під’єднатися до каталогу модулів.';

  @override
  String get modulesMarketplaceTimeout => 'Каталог модулів не відповів.';

  @override
  String get modulesIncompatible => 'Цей модуль несумісний із цим пристроєм.';

  @override
  String get modulesPackageSizeInvalid => 'Недійсний розмір пакета модуля.';

  @override
  String get modulesDownloadInterrupted => 'Завантаження модуля перервано.';

  @override
  String get modulesDownloadTimeout =>
      'Час очікування завантаження модуля минув.';

  @override
  String get modulesPackageUrlUntrusted => 'Адреса пакета модуля ненадійна.';

  @override
  String get modulesDownloadLeftGithub =>
      'Завантаження модуля переспрямовано за межі GitHub.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'Завантаження модуля має забагато переспрямувань.';

  @override
  String get modulesDownloadTooLarge => 'Розмір завантаження модуля завеликий.';

  @override
  String get modulesPackageIntegrityFailed =>
      'Пакет модуля не пройшов перевірку цілісності.';

  @override
  String get modulesDownloadUnavailable =>
      'Завантаження цього модуля зараз недоступне.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'Каталог модулів недоступний. Спробуйте ще раз.';

  @override
  String get modulesCatalogInvalid => 'Каталог модулів недійсний.';

  @override
  String get modulesThisSystem => 'ця система';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version або новіша';
  }

  @override
  String get modulesNotAvailableForDevice => 'Недоступно для цього пристрою.';

  @override
  String get modulesAppVersionUnverified =>
      'Не вдалося перевірити встановлену версію CopyPaste.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Цей модуль не підтримує CopyPaste $version.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Потрібно: $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed =>
      'Не вдалося завантажити каталог модулів.';

  @override
  String get modulesSmsSetupRequired =>
      'Налаштуйте доступ до SMS, перш ніж вмикати цей модуль.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'Не вдалося запустити відстеження SMS. Модуль досі ввімкнено; вимкніть його знову, щоб повторити спробу.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'Не вдалося запустити відстеження SMS. Модуль вимкнено.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'Не вдалося запустити відстеження SMS. Не вдалося підтвердити, чи ввімкнено модуль.';

  @override
  String get modulesSmsMonitoringFailed =>
      'Не вдалося запустити відстеження SMS.';

  @override
  String get modulesSmsAndroidOnly => 'Доступ до SMS можливий лише на Android.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'Відстеження SMS потребує налаштування.';

  @override
  String get modulesOperationIncomplete =>
      'Не вдалося завершити операцію з модулем.';

  @override
  String get modulesUnsupportedEvent =>
      'Модуль використовує непідтримувану подію.';

  @override
  String get modulesUnsupportedResult =>
      'Модуль повернув непідтримуваний результат.';

  @override
  String get modulesUnsupportedField =>
      'Модуль використовує непідтримуване поле.';

  @override
  String get modulesResponseInvalid => 'Недійсна відповідь модуля.';

  @override
  String get modulesFileSelectionUnavailable => 'Вибір файлів недоступний.';

  @override
  String get modulesFileSelectFailed => 'Не вдалося вибрати файл.';

  @override
  String modulesFileNotAccepted(String field) {
    return 'Файл у полі «$field» не підходить.';
  }

  @override
  String get modulesSmsVerifyFailed => 'Не вдалося перевірити доступ до SMS.';

  @override
  String get modulesSmsAllowNotifications =>
      'Дозвольте сповіщення, щоб модуль SMS Codes працював у фоновому режимі.';

  @override
  String get modulesSmsNotGranted => 'Доступ до SMS не надано.';

  @override
  String get modulesSmsConfigureFailed =>
      'Не вдалося налаштувати доступ до SMS.';

  @override
  String get onboardingLinuxContinueBasic => 'Продовжити з базовим записом';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Необов’язково. Потрібно для Quick Paste і виключення програм-джерел.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Необов’язково. Потрібно, щоб через Quick Paste можна було повертатися до попередньої програми.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'У цьому сеансі Wayland немає доступу до буфера обміну. Увімкніть підписаний супровідний компонент для GNOME або KDE, а потім перевірте знову.';

  @override
  String get historyMoreActions => 'Інші дії';

  @override
  String get historyTagsAction => 'Теги…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'розблокувати секретні елементи',
      'other': 'Розблокувати секретні елементи',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'використати прихований фрагмент',
      'other': 'Використати прихований фрагмент',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'скопіювати секретний елемент',
      'other': 'Скопіювати секретний елемент',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'Недоступно в цій системі';

  @override
  String get deviceAuthNotConfigured =>
      'Спершу налаштуйте пароль, PIN-код або біометрію пристрою';

  @override
  String get commonOn => 'Увімкнено';

  @override
  String get commonOff => 'Вимкнено';

  @override
  String get settingsQuickPasteStartFailed =>
      'Не вдалося запустити Quick Paste, тому його скорочення не працює.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut використовує інша програма. Виберіть інше скорочення.';
  }

  @override
  String get devicesEmptyTitle => 'Інших пристроїв ще не сполучено';

  @override
  String get devicesEmptyMessage =>
      'Сполучіть інший пристрій, щоб синхронізувати між ними історію буфера обміну. Обидва пристрої мають бути в одній мережі, а кожне сполучення підтверджується на обох пристроях.';

  @override
  String get pairingStateNotCompleted => 'Сполучення не завершено';

  @override
  String get pairingStepInvite => 'Запрошення';

  @override
  String get pairingStepScan => 'Сканування';

  @override
  String get pairingStepCode => 'Код';

  @override
  String get pairingStepRequest => 'Запит';

  @override
  String get pairingStepConnect => 'З’єднання';

  @override
  String get pairingStepConfirm => 'Підтвердження';

  @override
  String get pairingStepPaired => 'Сполучено';

  @override
  String get pairingOtherDevice => 'Інший пристрій';

  @override
  String get pairingDecisionNotConfirmed => 'Ще не підтверджено';

  @override
  String get pairingDecisionSending => 'Надсилання рішення…';

  @override
  String get pairingDecisionConfirmed => 'Підтверджено';

  @override
  String get pairingDecisionRejected => 'Відхилено';

  @override
  String get pairingDecisionWaiting => 'Очікування підтвердження';

  @override
  String get pairingDecisionUnknown => 'Відповідь невідома';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count пристрою',
      many: '$count пристроїв',
      few: '$count пристрої',
      one: '$count пристрій',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count рядка',
      many: '$count рядків',
      few: '$count рядки',
      one: '$count рядок',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Головна навігація';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Крок $step з $count: $name';
  }

  @override
  String get modulesPhaseDownload => 'Завантаження';

  @override
  String get modulesPhaseInstall => 'Встановлення';

  @override
  String modulesInstallingModule(String module) {
    return 'Встановлення модуля $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return 'Оновлення модуля $module';
  }

  @override
  String get commonDismiss => 'Закрити';

  @override
  String get settingsRetentionConfirmTitle => 'Вилучити старішу історію?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Нове обмеження: $limit. Старіші незакріплені елементи буде зараз вилучено з цього пристрою, і відновити їх неможливо. Закріплені елементи збережуться.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle => 'Зменшити квоту сховища?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Нова квота: $size. Якщо незакріплена історія займає більше, найстаріші елементи буде зараз вилучено, і відновити їх неможливо. Закріплені елементи збережуться.';
  }

  @override
  String get settingsLimitConfirmAction => 'Застосувати обмеження';

  @override
  String get settingsHideEverywhereTitle => 'Повністю приховати CopyPaste?';

  @override
  String get settingsHideEverywhereMessage =>
      'Коли обидва параметри вимкнено, CopyPaste не має видимого значка. Щоб відкрити його знову, запустіть CopyPaste ще раз.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Коли обидва параметри вимкнено, CopyPaste не має видимого значка. Щоб відкрити його знову, натисніть $shortcut або запустіть CopyPaste ще раз.';
  }

  @override
  String get settingsHideEverywhereAction => 'Приховати';

  @override
  String get settingsExportDoneTitle => 'Текстову історію експортовано';

  @override
  String get settingsBackupDoneTitle => 'Резервну копію створено';

  @override
  String get settingsRestoreDoneTitle => 'Історію відновлено';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Експортовано $count текстового кліпу',
      many: 'Експортовано $count текстових кліпів',
      few: 'Експортовано $count текстові кліпи',
      one: 'Експортовано $count текстовий кліп',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count не вдалося прочитати',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Зашифровану резервну копію збережено · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'Історія буфера обміну на цьому пристрої тепер відповідає резервній копії.';

  @override
  String sharedSizeKibibytes(String size) {
    return '$size КіБ';
  }

  @override
  String sharedSizeMebibytes(String size) {
    return '$size МіБ';
  }

  @override
  String sharedSizeGibibytes(String size) {
    return '$size ГіБ';
  }

  @override
  String storageUsageShort(String used, String quota) {
    return '$used з $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return 'Використано $used з $quota';
  }

  @override
  String storageUsageNoQuota(String used) {
    return 'Використано $used';
  }

  @override
  String get storageUsageOverQuota => 'Квоту перевищено';

  @override
  String storageUsageOldestClip(String date) {
    return 'Найстаріший кліп: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Сховище: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Нова квота: $size. Незакріплена історія займає $used, тож її найстаріші кліпи буде зараз вилучено, доки вона не вміститься. Відновити їх неможливо. Закріплені елементи збережуться.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count кліпу',
      many: '$count кліпів',
      few: '$count кліпи',
      one: '$count кліп',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'Відкрити CopyPaste';

  @override
  String get trayOpenQuickPaste => 'Відкрити Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Відкрити Quick Paste ($shortcut)';
  }

  @override
  String get trayNewSnippet => 'Новий фрагмент';

  @override
  String get trayRecent => 'Нещодавні';

  @override
  String get trayRecentLoading => 'Завантаження…';

  @override
  String get trayRecentFailed => 'Не вдалося завантажити кліпи';

  @override
  String get trayRecentEmpty => 'Кліпів ще немає';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count нещодавнього кліпу приховано',
      many: '$count нещодавніх кліпів приховано',
      few: '$count нещодавні кліпи приховано',
      one: '$count нещодавній кліп приховано',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Секрет';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Копіювати останній з іншого пристрою';

  @override
  String get trayCaptureNow => 'Записати зараз';

  @override
  String trayPausedUntil(String time) {
    return 'Призупинено до $time';
  }

  @override
  String get trayPauseFor => 'Призупинити на…';

  @override
  String get trayPauseFor15Minutes => '15 хвилин';

  @override
  String get trayPauseForOneHour => '1 годину';

  @override
  String get trayPauseUntilResumed => 'Доки не відновлю';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count елемента',
      many: 'Paste Queue: $count елементів',
      few: 'Paste Queue: $count елементи',
      one: 'Paste Queue: $count елемент',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Збирання: $count елемента',
      many: 'Збирання: $count елементів',
      few: 'Збирання: $count елементи',
      one: 'Збирання: $count елемент',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Нещодавно пропущені ($count)';
  }

  @override
  String get trayOpenContentRules => 'Відкрити правила вмісту';

  @override
  String get trayRecentSettingsTitle => 'Смуга меню і трей';

  @override
  String get trayRecentSettingsDescription =>
      'Що меню в смузі меню або треї показує в розділі «Нещодавні».';

  @override
  String get trayRecentCountTitle => 'Нещодавні кліпи';

  @override
  String get trayRecentCountDescription =>
      'Скільки найновіших кліпів показує меню.';

  @override
  String get trayRecentPreviewTitle => 'Попередній перегляд нещодавніх кліпів';

  @override
  String get trayRecentPreviewDescription =>
      'Система зберігає текст меню поза шифруванням CopyPaste. Показуйте менше, щоб вміст кліпів туди не потрапляв. Секрети завжди приховано.';

  @override
  String get trayRecentPreviewFull => 'Повний перегляд';

  @override
  String get trayRecentPreviewTitles => 'Лише тип і джерело';

  @override
  String get trayRecentPreviewHidden => 'Приховано';

  @override
  String get trayRecentSettingsKeywords =>
      'трей смуга меню нещодавні перегляд приватність кількість';

  @override
  String get transformsPaletteTitle => 'Перетворити текст';

  @override
  String get transformsFilterPlaceholder => 'Фільтрувати перетворення';

  @override
  String get transformsNoMatches => 'Немає відповідних перетворень';

  @override
  String get transformsGroupResults => 'Результати';

  @override
  String get transformsGroupSuggested => 'Рекомендовані';

  @override
  String get transformsGroupRecent => 'Нещодавні';

  @override
  String get transformsGroupFavorites => 'Улюблені';

  @override
  String get transformsGroupPipelines => 'Конвеєри';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV і XML';

  @override
  String get transformsCategoryEncode => 'Кодування й декодування';

  @override
  String get transformsCategoryCase => 'Регістр';

  @override
  String get transformsCategoryHash => 'Хеші';

  @override
  String get transformsCategoryLines => 'Рядки й пробіли';

  @override
  String get transformsCategoryClean => 'Очищення';

  @override
  String get transformsCategoryUrl => 'Посилання';

  @override
  String get transformsCategoryMarkup => 'Форматований текст і Markdown';

  @override
  String get transformsCategoryDev => 'Для розробників';

  @override
  String get transformsCategoryOther => 'Інше';

  @override
  String get transformsChainEmpty => 'Кроків ще немає';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Далі: $name';
  }

  @override
  String get transformsStepEnabled => 'Виконувати цей крок';

  @override
  String get transformsMoveEarlier => 'Перемістити вище';

  @override
  String get transformsMoveLater => 'Перемістити нижче';

  @override
  String get transformsRemoveStep => 'Видалити крок';

  @override
  String get transformsSavePipeline => 'Зберегти як конвеєр';

  @override
  String get transformsSavePipelineTitle => 'Зберегти конвеєр';

  @override
  String get transformsSavePipelineFailed => 'Не вдалося зберегти конвеєр.';

  @override
  String get transformsPipelineNameLabel => 'Назва';

  @override
  String get transformsPipelineDescriptionLabel => 'Опис (необов\'язково)';

  @override
  String get transformsPipelineStepsLabel => 'Кроки';

  @override
  String get transformsPipelineNoSteps => 'Додайте принаймні один крок.';

  @override
  String get transformsPipelineNameRequired => 'Введіть назву.';

  @override
  String get transformsPipelineNameTooLong => 'Не більше 80 символів.';

  @override
  String get transformsPipelineNameTaken => 'Інший конвеєр уже має цю назву.';

  @override
  String get transformsPipelineSavedTitle => 'Конвеєр збережено';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name доступний у кожній палітрі перетворень.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Кроків: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Видалений конвеєр';

  @override
  String get transformsNeedsNewerVersion => 'Потрібна новіша версія CopyPaste';

  @override
  String get transformsLoadFailed => 'Не вдалося завантажити перетворення.';

  @override
  String get transformsNotTextTitle => 'Перетворення працюють із текстом';

  @override
  String get transformsNotTextMessage =>
      'Цей елемент — зображення, файл або колір. Виберіть текстовий елемент або скопіюйте текст.';

  @override
  String get transformsNothingTitle => 'Немає що перетворювати';

  @override
  String get transformsNothingMessage =>
      'Скопіюйте текст або виберіть елемент з історії.';

  @override
  String get transformsPickTransform => 'Виберіть перетворення';

  @override
  String get transformsPickTransformHint =>
      'Tab додає його як крок. Enter застосовує результат.';

  @override
  String get transformsPreviewing => 'Оновлення попереднього перегляду…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Рядків: $lines · Слів: $words · Символів: $chars';
  }

  @override
  String get transformsNoChanges => 'Без змін';

  @override
  String transformsTruncated(String size) {
    return 'Показано перші 64 КіБ із $size';
  }

  @override
  String get transformsRandom => 'Випадково';

  @override
  String get transformsNotRun => 'Не виконано';

  @override
  String get transformsSensitiveHidden =>
      'Результат конфіденційний і прихований.';

  @override
  String get transformsReveal => 'Показати';

  @override
  String get transformsPaneOutput => 'Результат';

  @override
  String get transformsPaneInput => 'Вхідні дані';

  @override
  String get transformsPaneChanges => 'Зміни';

  @override
  String get transformsCannotTransform => 'Не вдається перетворити цей текст';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Крок $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Показати в hex';

  @override
  String get transformsActionPaste => 'Вставити';

  @override
  String get transformsActionSave => 'Зберегти як новий елемент';

  @override
  String get transformsCopiedMessage =>
      'Результат у буфері обміну та в історії.';

  @override
  String get transformsSavedTitle => 'Збережено';

  @override
  String get transformsSavedMessage =>
      'Результат — новий елемент історії. Буфер обміну не змінено.';

  @override
  String get transformsPasteFailedTitle => 'Не вдалося вставити';

  @override
  String get transformsPasteFailedMessage =>
      'Результат у буфері обміну. Вставте його вручну.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message у рядку $line, стовпці $column';
  }

  @override
  String get transformsErrorBinary => 'Декодовані дані двійкові, а не текст';

  @override
  String get transformsErrorTooLarge => 'Завеликий для перетворення';

  @override
  String get transformsErrorInvalidParam => 'Налаштування цього кроку недійсне';

  @override
  String get transformsErrorCancelled => 'Скасовано';

  @override
  String get transformsErrorTooExpensive => 'Шаблон надто складний';

  @override
  String get transformsErrorPattern => 'Шаблон недійсний';

  @override
  String get transformsErrorTimestampUnit =>
      'Виберіть, чи число в секундах, чи в мілісекундах';

  @override
  String get transformsErrorExternalEntity => 'Зовнішні сутності заборонені';

  @override
  String get transformsErrorJwtEncrypted =>
      'Зашифровані токени не можна декодувати';

  @override
  String get transformsErrorJson => 'Недійсний JSON';

  @override
  String get transformsErrorYaml => 'Недійсний YAML';

  @override
  String get transformsErrorXml => 'Недійсний XML';

  @override
  String get transformsErrorCsv => 'Недійсний CSV';

  @override
  String get transformsErrorBase64 => 'Недійсний Base64';

  @override
  String get transformsErrorHex => 'Недійсний hex';

  @override
  String get transformsErrorEscape => 'Недійсна escape-послідовність';

  @override
  String get transformsErrorUrl => 'Неправильне відсоткове кодування';

  @override
  String get transformsErrorJwt => 'Недійсний JWT';

  @override
  String get transformsErrorDate => 'Нерозпізнана дата або позначка часу';

  @override
  String get transformsErrorGeneric =>
      'Цей крок не може перетворити вхідні дані';

  @override
  String get transformsWarningSignedLink =>
      'Підписане посилання залишено без змін';

  @override
  String get transformsWarningExpired => 'Термін дії минув';

  @override
  String get transformsWarningNotYetValid => 'Ще не дійсний';

  @override
  String get transformsWarningSignatureNotVerified => 'Підпис не перевірено';

  @override
  String get transformsWarningDuplicateKeys => 'Дубльовані ключі';

  @override
  String get transformsWarningLenientJson => 'Прийнято нестандартний JSON';

  @override
  String get transformsWarningDoubleEncoded => 'JSON було закодовано двічі';

  @override
  String get transformsWarningEscapes =>
      'Невідомі escape-послідовності залишено як є';

  @override
  String get transformsWarningLocalZone => 'Припущено локальний часовий пояс';

  @override
  String get transformsWarningNoMatches => 'Збігів немає';

  @override
  String get transformsWarningGeneric => 'Перевірте результат';

  @override
  String get transformsRestoreTitle =>
      'Відновлювати буфер обміну після вставлення';

  @override
  String get transformsRestoreDescription =>
      'Після вставлення перетвореного тексту повертається попередній вміст буфера, якщо тим часом нічого не скопійовано.';

  @override
  String get transformsDefaultTitle => 'Типове перетворення під час вставлення';

  @override
  String get transformsDefaultDescription =>
      'Quick Paste застосовує цей конвеєр до тексту й показує його перед вставленням. Якщо він не спрацює, вставляється оригінал.';

  @override
  String get transformsNoDefault => 'Немає';

  @override
  String get transformsAppDefaultsTitle => 'Типові для програм';

  @override
  String get transformsAppDefaultsDescription =>
      'Перевизначте типове для програми за ідентифікатором пакета, виконуваним файлом або назвою пакета. Записи з іншої платформи синхронізуються, але тут не діють.';

  @override
  String get transformsAppIdentifierPlaceholder => 'Ідентифікатор програми';

  @override
  String transformsOtherPlatform(String platform) {
    return 'лише на $platform';
  }

  @override
  String get transformsAdd => 'Додати';

  @override
  String get transformsRemove => 'Видалити';

  @override
  String get transformsFavoritesTitle => 'Улюблені';

  @override
  String get transformsFavoritesDescription =>
      'Улюблені показуються першими в кожній палітрі. Перші дев\'ять запускаються клавішами Mod+1–Mod+9.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Mod+$number у палітрі';
  }

  @override
  String get transformsAddFavorite => 'Додати улюблене';

  @override
  String get transformsPipelinesTitle => 'Конвеєри';

  @override
  String get transformsPipelinesDescription =>
      'Збережені ланцюжки перетворень із налаштуваннями. Вони синхронізуються з іншими пристроями.';

  @override
  String get transformsNewPipeline => 'Новий конвеєр';

  @override
  String get transformsEditPipeline => 'Редагувати конвеєр';

  @override
  String get transformsDuplicatePipeline => 'Дублювати';

  @override
  String get transformsDeletePipeline => 'Видалити';

  @override
  String transformsDeletePipelineTitle(String name) {
    return 'Видалити $name?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Улюблені й типові налаштування вставлення, що його використовують, на всіх пристроях повернуться до «Немає».';

  @override
  String get transformsDeletePipelineFailed => 'Не вдалося видалити конвеєр.';

  @override
  String get transformsAddStep => 'Додати крок';

  @override
  String get transformsHiddenTitle => 'Приховані перетворення';

  @override
  String get transformsHiddenDescription =>
      'Приховані перетворення не показуються в палітрах і рекомендаціях. Конвеєри, що їх використовують, і далі працюють.';

  @override
  String get transformsHideTransform => 'Приховати перетворення';

  @override
  String get transformsShow => 'Показати';

  @override
  String get transformsSuggestionsTitle => 'Рекомендації';

  @override
  String get transformsSuggestionsDescription =>
      'Під час відкриття палітри перші 64 КіБ тексту перевіряються на наведені нижче типи, щоб запропонувати до трьох перетворень. У фоні нічого не працює.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web Tokens';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl =>
      'Посилання з параметрами відстеження';

  @override
  String get transformsDetectorTimestamp => 'Позначки часу';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Параметри відстеження';

  @override
  String get transformsTrackingDescription =>
      'Вбудований список постачається з програмою й змінюється лише з оновленнями. Додайте назви, які треба видаляти або зберігати; формат назва@домен обмежує запис доменом.';

  @override
  String get transformsTrackingRemoveLabel => 'Також видаляти';

  @override
  String get transformsTrackingAllowLabel => 'Завжди зберігати';

  @override
  String get transformsTrackingPlaceholder =>
      'назва або назва@домен, потім Enter';
}
