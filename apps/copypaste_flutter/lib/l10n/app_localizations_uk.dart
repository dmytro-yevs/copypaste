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

  @override
  String get quickAccessUnavailableTitle => 'Стан швидкого доступу недоступний';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste не вдалося прочитати стан клавіатури, плиток і віджета. Спробуйте ще раз.';

  @override
  String get quickAccessActionFailedTitle => 'Не вдалося виконати дію';

  @override
  String get quickAccessActionFailedMessage =>
      'Android не відкрив потрібний екран або не зберіг зміну. Спробуйте ще раз.';

  @override
  String get quickAccessKeyboardTitle => 'Клавіатура кліпів';

  @override
  String get quickAccessKeyboardDescription =>
      'Увімкніть «Кліпи CopyPaste» в системних налаштуваннях клавіатури. Android попередить, що клавіатура може бачити введений текст: панель кліпів ніколи не записує натискання клавіш, не має клавіш для введення в інших застосунках і вставляє лише ті елементи, яких ви торкаєтеся.';

  @override
  String get quickAccessKeyboardOn => 'Увімкнено';

  @override
  String get quickAccessKeyboardOff => 'Вимкнено';

  @override
  String get quickAccessOpenKeyboardSettings => 'Налаштування клавіатури';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'Увімкнути панель клавіатури «Кліпи CopyPaste».';

  @override
  String get quickAccessKeyboardKeywords =>
      'клавіатура метод введення ime панель увімкнути';

  @override
  String get quickAccessSwitchKeyboardTitle => 'Перейти на клавіатуру кліпів';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'Виберіть «Кліпи CopyPaste» поточною клавіатурою. Поки її вибрано, кожну копію буде збережено.';

  @override
  String get quickAccessSwitchKeyboardAction => 'Вибрати клавіатуру';

  @override
  String get quickAccessKeyboardCurrent => 'Поточна клавіатура';

  @override
  String get quickAccessCaptureTierTitle => 'Режим запису';

  @override
  String get quickAccessCaptureTierDescription =>
      'Без доступу через Shizuku або ADB копії зберігаються лише тоді, коли відкривається панель кліпів, поки її вибрано, або із застосунку CopyPaste, плитки, віджета чи меню «Поділитися».';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'Як зберігаються копії з інших застосунків.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'запис рівень повний панель вручну shizuku збирати';

  @override
  String get quickAccessTierFull =>
      'Повний: кожна копія зберігається у фоновому режимі';

  @override
  String get quickAccessTierPanelCollecting => 'Панель вибрано: збирання';

  @override
  String get quickAccessTierPanelCatchUp =>
      'Режим панелі: синхронізація під час відкриття';

  @override
  String get quickAccessTierManual =>
      'Вручну: зберігайте із застосунку, плитки, віджета або меню «Поділитися»';

  @override
  String get quickAccessTierPaused => 'Запис призупинено';

  @override
  String get quickAccessAutoReturnTitle =>
      'Повертатися до попередньої клавіатури після вставлення';

  @override
  String get quickAccessAutoReturnDescription =>
      'Після одного вставлення клавіатура перемикається назад. Панель лишається відкритою, поки ви працюєте на вкладці «Черга» або з діями елемента. Вимкніть, щоб і далі збирати копії.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'повернутися попередня клавіатура автоматично вставлення';

  @override
  String get quickAccessFocusSearchTitle =>
      'Фокус на пошуку під час відкриття панелі';

  @override
  String get quickAccessFocusSearchDescription =>
      'Панель відкривається з полем пошуку. Якщо вимкнено, кліпи доступні одним дотиком.';

  @override
  String get quickAccessTilesTitle => 'Плитки швидких налаштувань';

  @override
  String get quickAccessTilesSearchDescription =>
      'Додати плитки «Зберегти буфер обміну» та «Запис буфера обміну».';

  @override
  String get quickAccessTilesKeywords =>
      'швидкі налаштування плитка шторка зберегти пауза';

  @override
  String get quickAccessTileInstructions =>
      'Двічі проведіть униз від верхнього краю екрана, торкніться «Змінити» та перетягніть плитку CopyPaste у швидкі налаштування.';

  @override
  String get quickAccessSaveTileTitle => 'Плитка «Зберегти буфер обміну»';

  @override
  String get quickAccessSaveTileDescription =>
      'Зберігає вміст буфера обміну зі швидких налаштувань.';

  @override
  String get quickAccessCaptureTileTitle => 'Плитка «Запис буфера обміну»';

  @override
  String get quickAccessCaptureTileDescription =>
      'Призупиняє та відновлює запис зі швидких налаштувань.';

  @override
  String get quickAccessAddTile => 'Додати плитку';

  @override
  String get quickAccessTileAdded => 'Додано до швидких налаштувань';

  @override
  String get quickAccessWidgetTitle => 'Віджет «Останні кліпи»';

  @override
  String get quickAccessWidgetDescription =>
      'Показує ваші останні кліпи на головному екрані. Торкніться кліпу, щоб скопіювати його.';

  @override
  String get quickAccessWidgetInstructions =>
      'Натисніть і утримуйте головний екран, торкніться «Віджети» та перетягніть «Останні кліпи» з CopyPaste.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Додано $count віджета',
      many: 'Додано $count віджетів',
      few: 'Додано $count віджети',
      one: 'Додано 1 віджет',
      zero: 'Не додано',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Додати віджет';

  @override
  String get quickAccessWidgetKeywords =>
      'головний екран віджет лаунчер останні';

  @override
  String snippetsAbbreviationBadge(String abbreviation) {
    return 'Скорочення $abbreviation';
  }

  @override
  String get snippetsAbbreviationLabel => 'Скорочення';

  @override
  String get snippetsAbbreviationPlaceholder => 'Необов’язково, наприклад підп';

  @override
  String snippetsAbbreviationTaken(String title) {
    return 'Уже використовується у «$title»';
  }

  @override
  String snippetsAbbreviationTooLong(int max) {
    return 'Використайте не більше $max символів.';
  }

  @override
  String get snippetsAbbreviationWhitespace =>
      'Скорочення не може містити пробілів або керівних символів.';

  @override
  String snippetsAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count фрагмента потребують перевірки після синхронізації',
      many: '$count фрагментів потребують перевірки після синхронізації',
      few: '$count фрагменти потребують перевірки після синхронізації',
      one: '$count фрагмент потребує перевірки після синхронізації',
      zero: 'Немає фрагментів для перевірки',
    );
    return '$_temp0';
  }

  @override
  String get snippetsBodyLabel => 'Текст';

  @override
  String get snippetsBodyPlaceholder => 'Введіть текст фрагмента';

  @override
  String get snippetsClearSelection => 'Зняти виділення';

  @override
  String get snippetsClipTooLarge =>
      'Завеликий для редагування. Цей кліп відкривається лише для читання.';

  @override
  String snippetsCollectionAccent(String accent) {
    String _temp0 = intl.Intl.selectLogic(accent, {
      'neutral': 'Нейтральний',
      'blue': 'Синій',
      'green': 'Зелений',
      'amber': 'Бурштиновий',
      'red': 'Червоний',
      'other': 'Нейтральний',
    });
    return '$_temp0';
  }

  @override
  String get snippetsCollectionAccentLabel => 'Колір акценту';

  @override
  String get snippetsCollectionIcon => 'Значок';

  @override
  String get snippetsCollectionLabel => 'Колекція';

  @override
  String get snippetsCollectionName => 'Назва';

  @override
  String get snippetsCollectionNameTaken =>
      'Колекція з такою назвою вже існує.';

  @override
  String snippetsCollectionNameTooLong(int max) {
    return 'Використайте не більше $max символів.';
  }

  @override
  String get snippetsConcealedBody => 'Цей фрагмент прихований.';

  @override
  String get snippetsConcealedConfirmAction => 'Продовжити';

  @override
  String get snippetsConcealedConfirmMessage =>
      'На цьому пристрої немає пароля, PIN-коду чи біометрії, щоб перевірити вас. Продовжуйте, лише якщо це ваш пристрій.';

  @override
  String get snippetsConcealedConfirmTitle => 'Підтвердьте, що це ви';

  @override
  String get snippetsConcealedPreview => 'Прихований фрагмент';

  @override
  String get snippetsConcealedSwitch => 'Прихований';

  @override
  String get snippetsConcealedTitle => 'Прихований фрагмент';

  @override
  String get snippetsConflictBadge => 'Конфліктна копія';

  @override
  String get snippetsCopied => 'Скопійовано';

  @override
  String get snippetsCreate => 'Створити фрагмент';

  @override
  String snippetsCreateNamed(String query) {
    return 'Створити фрагмент «$query»';
  }

  @override
  String get snippetsDeleteCollection => 'Видалити колекцію';

  @override
  String get snippetsDeleteCollectionMove =>
      'Перемістити її фрагменти в «Без колекції»';

  @override
  String snippetsDeleteCollectionTitle(String name) {
    return 'Видалити «$name»?';
  }

  @override
  String get snippetsDeleteCollectionWithSnippets => 'Видалити і її фрагменти';

  @override
  String get snippetsDeletedMessage => 'Переміщено в «Нещодавно видалені».';

  @override
  String snippetsDeletedTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Видалено $count фрагмента',
      many: 'Видалено $count фрагментів',
      few: 'Видалено $count фрагменти',
      one: 'Видалено $count фрагмент',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDestination => 'Зберегти в';

  @override
  String snippetsDestinationName(String destination) {
    String _temp0 = intl.Intl.selectLogic(destination, {
      'historyClip': 'Кліп в історії',
      'snippet': 'Фрагмент',
      'other': 'Кліп в історії',
    });
    return '$_temp0';
  }

  @override
  String get snippetsDiscard => 'Відкинути';

  @override
  String snippetsDragging(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count фрагмента',
      many: '$count фрагментів',
      few: '$count фрагменти',
      one: '$count фрагмент',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDuplicateAbbreviation =>
      'Інший фрагмент отримав таке саме скорочення під час синхронізації. Змініть одне з них.';

  @override
  String get snippetsDuplicateBadge => 'Дубльоване скорочення';

  @override
  String get snippetsEditClip => 'Редагувати кліп';

  @override
  String get snippetsEditCollection => 'Редагувати колекцію';

  @override
  String get snippetsEditSnippet => 'Редагувати фрагмент';

  @override
  String get snippetsEditTab => 'Редагування';

  @override
  String get snippetsEmptyMessage =>
      'Дайте фрагменту скорочення, а потім введіть його в Quick Paste.';

  @override
  String get snippetsEmptyTitle => 'Ще немає фрагментів';

  @override
  String get snippetsEnableTemplate => 'Увімкнути';

  @override
  String get snippetsEnableTemplateMessage =>
      'Заповнювачі працюють лише в шаблонах. Текст, що вже схожий на заповнювач, теж буде заповнено.';

  @override
  String get snippetsEnableTemplateTitle => 'Увімкнути «Шаблон»?';

  @override
  String get snippetsExport => 'Експортувати';

  @override
  String get snippetsExportCollection => 'Експортувати колекцію';

  @override
  String get snippetsExportConcealedConfirm =>
      'Додати приховані фрагменти до читабельного файлу';

  @override
  String snippetsExportCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Записано $count фрагмента',
      many: 'Записано $count фрагментів',
      few: 'Записано $count фрагменти',
      one: 'Записано $count фрагмент',
      zero: 'Жодного фрагмента не записано',
    );
    return '$_temp0';
  }

  @override
  String get snippetsExportDone => 'Фрагменти експортовано';

  @override
  String get snippetsExportEncrypt => 'Зашифрувати парольною фразою';

  @override
  String get snippetsExportFailed => 'Не вдалося експортувати';

  @override
  String get snippetsExportPlainConfirm =>
      'Я розумію, що будь-хто з цим файлом зможе його прочитати';

  @override
  String snippetsExportScope(String scope, int count) {
    String _temp0 = intl.Intl.selectLogic(scope, {
      'all': 'Усі фрагменти',
      'collections': 'Ця колекція',
      'snippets': 'Вибрані фрагменти ($count)',
      'other': 'Усі фрагменти',
    });
    return '$_temp0';
  }

  @override
  String get snippetsExportTitle => 'Експорт фрагментів';

  @override
  String get snippetsFieldBuilderTitle => 'Вставити поле';

  @override
  String get snippetsFieldDefault => 'Типове значення';

  @override
  String get snippetsFieldName => 'Назва';

  @override
  String get snippetsFieldOptions => 'Варіанти через кому';

  @override
  String get snippetsFieldRequired => 'Заповніть це поле.';

  @override
  String get snippetsFieldType => 'Тип';

  @override
  String snippetsFieldTypeName(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'text': 'Один рядок',
      'multiline': 'Кілька рядків',
      'choice': 'Вибір',
      'other': 'Один рядок',
    });
    return '$_temp0';
  }

  @override
  String get snippetsFormattingNotice =>
      'Форматування буде вилучено під час збереження.';

  @override
  String get snippetsGoToSnippet => 'Перейти до фрагмента';

  @override
  String get snippetsImport => 'Імпортувати';

  @override
  String get snippetsImportAbbreviationPolicy =>
      'Скорочення вже використовується';

  @override
  String snippetsImportAbbreviationPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'importWithout': 'Імпортувати без скорочення',
      'renameWithSuffix': 'Додати число до скорочення',
      'skip': 'Пропустити',
      'other': 'Імпортувати без скорочення',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportCollectionPolicy => 'Колекція з такою самою назвою';

  @override
  String snippetsImportCollectionPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'merge': 'Додати до наявної',
      'createNew': 'Створити нову',
      'other': 'Додати до наявної',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportConvert => 'Перетворити відомі заповнювачі';

  @override
  String snippetsImportCounts(
    int added,
    int identical,
    int updated,
    int conflicts,
    int invalid,
  ) {
    return 'Нові: $added. Однакові, пропущено: $identical. Змінені: $updated. Конфлікти скорочень: $conflicts. Недійсні: $invalid.';
  }

  @override
  String get snippetsImportDone => 'Фрагменти імпортовано';

  @override
  String get snippetsImportFailed => 'Не вдалося імпортувати';

  @override
  String get snippetsImportFileType => 'Файли фрагментів';

  @override
  String get snippetsImportFormat => 'Формат';

  @override
  String snippetsImportFormatName(String format) {
    String _temp0 = intl.Intl.selectLogic(format, {
      'bundle': 'Пакет CopyPaste',
      'json': 'Список JSON',
      'csv': 'CSV',
      'other': 'Пакет CopyPaste',
    });
    return '$_temp0';
  }

  @override
  String snippetsImportInvalidRow(int row, String reason) {
    return 'Рядок $row: $reason';
  }

  @override
  String get snippetsImportInvalidRows => 'Рядки, які не буде імпортовано';

  @override
  String get snippetsImportPassphraseHint =>
      'Парольна фраза, якщо файл зашифровано';

  @override
  String get snippetsImportPreview => 'Переглянути';

  @override
  String snippetsImportReport(
    int inserted,
    int updated,
    int skipped,
    int flagged,
    int unrecognized,
  ) {
    return 'Додано: $inserted. Оновлено: $updated. Пропущено: $skipped. Скорочення для перевірки: $flagged. Нерозпізнані заповнювачі: $unrecognized.';
  }

  @override
  String get snippetsImportSameIdPolicy => 'Той самий фрагмент, інший вміст';

  @override
  String snippetsImportSameIdPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'keepBoth': 'Залишити обидва',
      'replace': 'Замінити наявний',
      'skip': 'Пропустити',
      'other': 'Залишити обидва',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportTitle => 'Імпорт фрагментів';

  @override
  String get snippetsInsert => 'Вставити';

  @override
  String get snippetsInsertPlaceholder => 'Вставити заповнювач';

  @override
  String get snippetsKeepReferences => 'Залишити';

  @override
  String get snippetsLibraryDescription =>
      'Переглядайте, впорядковуйте й редагуйте фрагменти та колекції.';

  @override
  String get snippetsLibraryKeywords =>
      'фрагменти шаблони скорочення колекції текст сніпети';

  @override
  String get snippetsLibraryTitle => 'Бібліотека фрагментів';

  @override
  String snippetsLint(String code, String detail) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'unclosed': 'Цей заповнювач не закрито.',
      'malformed': 'Не вдається прочитати цей заповнювач.',
      'unknownPlaceholder':
          'Невідомий заповнювач «$detail». Його буде вставлено як є.',
      'unknownParameter': 'Невідомий параметр «$detail».',
      'duplicateParameter': 'Параметр «$detail» задано двічі.',
      'missingParameter': 'Бракує параметра «$detail».',
      'invalidValue': 'Параметр «$detail» має недійсне значення.',
      'conflictingParameters': 'Ці параметри не можна використовувати разом.',
      'unknownLocale': 'Невідома мова.',
      'unknownTimeZone': 'Невідомий часовий пояс.',
      'invalidPattern': 'Недійсний шаблон дати.',
      'dateOutOfRange': 'Дата поза допустимим діапазоном.',
      'unknownModifier': 'Невідомий модифікатор «$detail».',
      'modifierFailed': 'Не вдалося застосувати модифікатор «$detail».',
      'emptyClipboard': 'Елемент буфера обміну $detail порожній.',
      'extraCursor': 'Використовується лише перший заповнювач курсора.',
      'nestedCursor': 'Курсор у вставленому фрагменті ігнорується.',
      'tooManyFields': 'Поле «$detail» перевищує ліміт у 10 полів.',
      'conflictingField':
          'Поле «$detail» визначено двічі з різними параметрами.',
      'choiceDefaultNotOption':
          'Типове значення «$detail» не входить до його варіантів.',
      'cycle': 'Фрагменти вставляють один одного по колу: $detail.',
      'depthExceeded': 'Фрагменти вкладено глибше ніж на 3 рівні.',
      'nestingLimit': 'Забагато вставлених фрагментів.',
      'unknownSnippet': 'Жоден фрагмент не має такого скорочення.',
      'containsPlaceholders':
          'Текст містить заповнювачі. Увімкніть «Шаблон», щоб заповнити їх.',
      'other': 'Із цим заповнювачем проблема.',
    });
    return '$_temp0';
  }

  @override
  String get snippetsLoadFailed => 'Не вдалося завантажити фрагменти';

  @override
  String get snippetsLoadMore => 'Завантажити ще';

  @override
  String get snippetsMerge => 'Об’єднати';

  @override
  String get snippetsMergeCollection => 'Об’єднати з іншою колекцією';

  @override
  String snippetsMergeCollectionTitle(String name) {
    return 'Об’єднати «$name» з';
  }

  @override
  String get snippetsMoveTo => 'Перемістити в';

  @override
  String get snippetsNewCollection => 'Нова колекція';

  @override
  String get snippetsNewSnippet => 'Новий фрагмент';

  @override
  String get snippetsNewText => 'Новий текст';

  @override
  String snippetsNoMatch(String query) {
    return 'Немає фрагментів для «$query»';
  }

  @override
  String get snippetsPassphraseConfirm => 'Повторіть парольну фразу';

  @override
  String snippetsPassphraseHint(int min) {
    return 'Парольна фраза, щонайменше $min символів';
  }

  @override
  String get snippetsPassphraseLabel => 'Парольна фраза';

  @override
  String get snippetsPassphraseNoRecovery =>
      'Забуту парольну фразу неможливо відновити.';

  @override
  String snippetsPassphraseStrength(String score) {
    String _temp0 = intl.Intl.selectLogic(score, {
      'tooShort': 'Закоротка',
      'weak': 'Слабка',
      'fair': 'Посередня',
      'good': 'Добра',
      'other': 'Надійна',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPinnedSwitch => 'Закріплений';

  @override
  String get snippetsPlaceholderGroup => 'Заповнювачі';

  @override
  String snippetsPlaceholderKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'today': 'Дата',
      'dateShort': 'Дата, коротка',
      'dateLong': 'Дата, повна',
      'dateIso': 'Дата, ISO 8601',
      'dateNextWeek': 'Дата через тиждень',
      'now': 'Час',
      'dateTime': 'Дата й час',
      'clipboard': 'Буфер обміну',
      'clipboardPrevious': 'Попередній елемент буфера обміну',
      'field': 'Поле для заповнення…',
      'cursor': 'Позиція курсора',
      'uuid': 'UUID',
      'random': 'Випадкове число',
      'upper': 'Модифікатор: великі літери',
      'lower': 'Модифікатор: малі літери',
      'trim': 'Модифікатор: прибрати пробіли',
      'other': 'Заповнювач',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPlaceholderNestedGroup => 'Вставити інший фрагмент';

  @override
  String get snippetsPlaceholderNoMatch => 'Немає відповідних заповнювачів';

  @override
  String get snippetsPreviewEmpty => 'Нічого переглядати';

  @override
  String get snippetsPreviewTab => 'Перегляд';

  @override
  String get snippetsPurge => 'Видалити назавжди';

  @override
  String get snippetsPurgeMessage =>
      'Їх буде стерто на всіх ваших пристроях. Цю дію неможливо скасувати.';

  @override
  String snippetsPurgeTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Видалити $count фрагмента назавжди?',
      many: 'Видалити $count фрагментів назавжди?',
      few: 'Видалити $count фрагменти назавжди?',
      one: 'Видалити $count фрагмент назавжди?',
    );
    return '$_temp0';
  }

  @override
  String get snippetsRecentlyDeletedEmpty => '«Нещодавно видалені» порожні';

  @override
  String get snippetsRecordPastedDescription =>
      'Додає текст вставлених фрагментів до історії. Приховані фрагменти ніколи не записуються.';

  @override
  String get snippetsRecordPastedKeywords =>
      'історія записувати вставлені фрагменти';

  @override
  String get snippetsRecordPastedTitle =>
      'Записувати вставлені фрагменти в історію';

  @override
  String get snippetsReplaceOriginal => 'Замінити оригінал';

  @override
  String get snippetsReplaceOriginalHint =>
      'Після збереження видаляє оригінальний кліп на всіх пристроях. Якщо вимкнено, залишаються обидва.';

  @override
  String get snippetsRequireAuthDescription =>
      'Підтверджуйте, що це ви, перед використанням або показом прихованого фрагмента. Кожен пристрій запитує окремо.';

  @override
  String get snippetsRequireAuthKeywords =>
      'прихований пароль біометрія відбиток обличчя блокування перевірка';

  @override
  String get snippetsRequireAuthTitle =>
      'Перевіряти перед використанням прихованих фрагментів';

  @override
  String get snippetsRestore => 'Відновити';

  @override
  String get snippetsSaveBlockEmpty => 'Додайте текст, щоб зберегти.';

  @override
  String snippetsSaveBlockTitle(int max) {
    return 'Назва довша за $max символів.';
  }

  @override
  String get snippetsSaveBlockTooLarge => 'Текст більший за 64 КБ.';

  @override
  String get snippetsSaveFailed => 'Не вдалося зберегти';

  @override
  String get snippetsScopeAll => 'Усі';

  @override
  String get snippetsScopeEmpty => 'Тут немає фрагментів';

  @override
  String get snippetsScopePinned => 'Закріплені';

  @override
  String get snippetsScopeRecentlyDeleted => 'Нещодавно видалені';

  @override
  String get snippetsSearchPlaceholder => 'Шукати фрагменти';

  @override
  String snippetsSelected(int count) {
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
  String get snippetsShow => 'Показати';

  @override
  String get snippetsShowAll => 'Показати всі';

  @override
  String get snippetsShowAttention => 'Перевірити';

  @override
  String snippetsSoftLimit(int count) {
    return 'У вас $count фрагментів. Дозволено до 20 000.';
  }

  @override
  String get snippetsTemplateBadge => 'Шаблон';

  @override
  String get snippetsTemplateSwitch => 'Шаблон';

  @override
  String get snippetsTitleLabel => 'Назва';

  @override
  String get snippetsTitlePlaceholder => 'Необов’язково';

  @override
  String get snippetsTransferDescription =>
      'Переносьте фрагменти між програмами або зберігайте резервну копію. Експорт типово зашифрований.';

  @override
  String get snippetsTransferKeywords =>
      'імпорт експорт резервна копія пакет json csv парольна фраза';

  @override
  String get snippetsTransferTitle => 'Імпорт і експорт';

  @override
  String get snippetsUndo => 'Скасувати';

  @override
  String get snippetsUnfiled => 'Без колекції';

  @override
  String snippetsUnfilledWarning(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count заповнювача буде вставлено як є',
      many: '$count заповнювачів буде вставлено як є',
      few: '$count заповнювачі буде вставлено як є',
      one: '$count заповнювач буде вставлено як є',
    );
    return '$_temp0';
  }

  @override
  String get snippetsUnpin => 'Відкріпити';

  @override
  String get snippetsUnsavedMessage => 'Зберегти зміни перед закриттям?';

  @override
  String get snippetsUnsavedTitle => 'Незбережені зміни';

  @override
  String get snippetsUntitled => 'Фрагмент без назви';

  @override
  String get snippetsUpdateReferences => 'Оновити';

  @override
  String snippetsUpdateReferencesMessage(String from, String to) {
    return 'Інші шаблони можуть вставляти «$from». Оновити їх, щоб вони використовували «$to»?';
  }

  @override
  String get snippetsUpdateReferencesTitle => 'Оновити посилання?';

  @override
  String get snippetsUseFailed => 'Не вдалося використати фрагмент';

  @override
  String get aiToolsEnableTitle =>
      'Дозволити інструментам ШІ доступ до CopyPaste на цьому комп’ютері';

  @override
  String get aiToolsEnableDescription =>
      'Підключені інструменти ШІ можуть читати елементи буфера обміну, які ви дозволите нижче. Усе, чим ви ділитеся з інструментом ШІ, надсилається його постачальнику.';

  @override
  String aiToolsStatusOn(int count) {
    return 'Увімкнено · пристроїв, що діляться історією: $count';
  }

  @override
  String get aiToolsStatusOff =>
      'Вимкнено · підключені інструменти не мають доступу до CopyPaste на цьому комп’ютері';

  @override
  String get aiToolsThreatModelTitle => 'Від чого захищають ці обмеження';

  @override
  String get aiToolsThreatModel =>
      'Дозволи, підтвердження та приховування обмежують те, що інструмент ШІ бачить через підключення до CopyPaste. Вони не захищають від інших програм, запущених під вашим обліковим записом, які й так можуть читати буфер обміну та історію.';

  @override
  String get aiToolsGrantsTitle => 'Підключені інструменти';

  @override
  String get aiToolsGrantsDescription =>
      'Інструменти ШІ, підключені на будь-якому з ваших комп’ютерів. Зміна набуває чинності на комп’ютері, якому належить підключення, після синхронізації.';

  @override
  String aiToolsServedThrough(String desktops) {
    return 'Обслуговується через: $desktops';
  }

  @override
  String get aiToolsConnect => 'Підключити інструмент ШІ';

  @override
  String get aiToolsEmptyTitle => 'Ще немає підключених інструментів ШІ';

  @override
  String get aiToolsEmptyMessage =>
      'Підключіть інструмент ШІ, щоб він міг шукати серед дозволених елементів буфера обміну.';

  @override
  String get aiToolsEmptyServedTitle =>
      'Інструменти ШІ підключаються через комп’ютер';

  @override
  String get aiToolsEmptyServedMessage =>
      'Увімкніть інструменти ШІ в CopyPaste на комп’ютері з Mac, Windows або Linux, і там стане доступною історія вашого телефона.';

  @override
  String get aiToolsUnknownDevice => 'інший комп’ютер';

  @override
  String aiToolsGrantOwner(String device) {
    return 'На $device';
  }

  @override
  String get aiToolsGrantWaiting => 'Очікує першого підключення';

  @override
  String aiToolsGrantUsed(String date) {
    return 'Підключено · востаннє використано $date';
  }

  @override
  String get aiToolsGrantRevoked => 'Відкликано';

  @override
  String aiToolsGrantRevokedRemote(String device) {
    return 'Відкликано · набуде чинності після синхронізації $device';
  }

  @override
  String get aiToolsGrantReissue =>
      'Після відновлення потрібне нове підключення';

  @override
  String get aiToolsGrantPaused =>
      'Призупинено: інструменти ШІ вимкнено на цьому комп’ютері';

  @override
  String get aiToolsEdit => 'Змінити';

  @override
  String get aiToolsActivity => 'Переглянути активність';

  @override
  String get aiToolsRevoke => 'Відкликати';

  @override
  String aiToolsRevokeTitle(String name) {
    return 'Відкликати $name?';
  }

  @override
  String get aiToolsRevokeMessage =>
      'Інструмент втратить доступ під час наступного запиту. Щоб підключити його знову, створіть нове підключення.';

  @override
  String aiToolsDeleteTitle(String name) {
    return 'Видалити $name?';
  }

  @override
  String get aiToolsDeleteMessage =>
      'Підключення буде відкликано й видалено з усіх ваших пристроїв.';

  @override
  String get aiToolsChangeFailed => 'Не вдалося зберегти зміну.';

  @override
  String get aiToolsLoadFailed =>
      'Не вдалося завантажити налаштування інструментів ШІ';

  @override
  String get aiToolsEditTitle => 'Змінити підключення';

  @override
  String get aiToolsNameLabel => 'Назва';

  @override
  String get aiToolsScopesLabel => 'Доступ';

  @override
  String get aiToolsScopeReadHistory => 'Читати історію';

  @override
  String get aiToolsScopeReadSnippets => 'Читати фрагменти';

  @override
  String get aiToolsScopeWriteClipboard => 'Записувати в буфер обміну';

  @override
  String get aiToolsScopeOrganize =>
      'Упорядковувати (закріплювати, зберігати фрагменти)';

  @override
  String get aiToolsLookbackLabel => 'Період історії';

  @override
  String get aiToolsLookbackDay => 'Останні 24 години';

  @override
  String get aiToolsLookbackWeek => 'Останні 7 днів';

  @override
  String get aiToolsLookbackMonth => 'Останні 30 днів';

  @override
  String get aiToolsLookbackAll => 'Уся історія';

  @override
  String aiToolsActivityTitle(String name) {
    return 'Активність: $name';
  }

  @override
  String get aiToolsActivityEmpty => 'Активності ще немає';

  @override
  String get aiToolsActivityFailed => 'Не вдалося завантажити активність';

  @override
  String aiToolsActivityEntry(String tool, int count) {
    return '$tool · результатів: $count';
  }

  @override
  String get aiToolsClientLabel => 'Інструмент ШІ';

  @override
  String get aiToolsClientOther => 'Інший';

  @override
  String get aiToolsCreate => 'Створити підключення';

  @override
  String aiToolsConfigHint(String client) {
    return 'Додайте цю конфігурацію до $client. Її ключ доступу показується лише один раз.';
  }

  @override
  String get aiToolsConfigHintOther =>
      'Додайте цю конфігурацію до налаштувань MCP вашого інструмента ШІ. Її ключ доступу показується лише один раз.';

  @override
  String aiToolsRestartNote(String client) {
    return 'Далі: перезапустіть $client або відкрийте новий сеанс.';
  }

  @override
  String get aiToolsRestartNoteOther =>
      'Далі: перезапустіть інструмент ШІ або відкрийте новий сеанс.';

  @override
  String get aiToolsCopied => 'Скопійовано';

  @override
  String get aiToolsCopiedMessage =>
      'Це не зберігається в історії, а буфер обміну очиститься через 60 секунд.';

  @override
  String get aiToolsCopyFailed => 'Не вдалося скопіювати конфігурацію.';

  @override
  String get aiToolsPolicyTitle => 'Що бачать інструменти ШІ';

  @override
  String get aiToolsPolicyNote =>
      'Конфіденційні елементи та виявлені облікові дані ніколи не передаються, хоч би що ви тут дозволили.';

  @override
  String get aiToolsOtherDevicesTitle => 'Включати елементи з інших пристроїв';

  @override
  String get aiToolsOtherDevicesDescription =>
      'Історія, синхронізована з інших ваших пристроїв. Вимкніть пристрій, щоб виключити його елементи.';

  @override
  String get aiToolsKindsTitle => 'Типи вмісту';

  @override
  String get aiToolsKindsDescription =>
      'Типи елементів буфера обміну, які можуть читати інструменти ШІ.';

  @override
  String get aiToolsKindText => 'Текст';

  @override
  String get aiToolsKindLink => 'Посилання';

  @override
  String get aiToolsKindRichText => 'Форматований текст';

  @override
  String get aiToolsKindImage => 'Зображення';

  @override
  String get aiToolsKindFileMetadata => 'Назви й розміри файлів';

  @override
  String get aiToolsExcludedAppsTitle => 'Виключені застосунки';

  @override
  String get aiToolsExcludedAppsDescription =>
      'Елементи, скопійовані з цих застосунків, ніколи не передаються.';

  @override
  String get aiToolsAddApp => 'Додати застосунок';

  @override
  String get aiToolsRemove => 'Вилучити';

  @override
  String get aiToolsDefaultLookbackTitle =>
      'Період історії для нових підключень';

  @override
  String get aiToolsDefaultLookbackDescription =>
      'Старіші незакріплені елементи залишаються невидимими для підключення. Закріплені елементи та фрагменти не обмежуються.';

  @override
  String get aiToolsKeywords =>
      'ші ai mcp асистент помічник claude codex cursor модель агент';

  @override
  String aiToolsApprovalTitle(String client) {
    return '$client хоче отримати доступ до CopyPaste';
  }

  @override
  String aiToolsReportedName(String name) {
    return 'Назва, яку повідомляє застосунок: $name';
  }

  @override
  String aiToolsReportedExecutable(String path) {
    return 'Програма: $path';
  }

  @override
  String get aiToolsApprovalNote =>
      'Дозволяйте лише інструменти, які ви запустили самі. Без відповіді запит буде відхилено через 2 хвилини.';

  @override
  String get aiToolsDeny => 'Відхилити';

  @override
  String get aiToolsAllow => 'Дозволити';

  @override
  String get aiToolsVolumeTitle => 'Дозволити читати далі?';

  @override
  String aiToolsVolumeMessage(String client) {
    return 'Інструмент $client прочитав багато історії буфера обміну в цьому сеансі. Дозволити більше?';
  }

  @override
  String get aiToolsNotificationBody =>
      'Відкрийте CopyPaste, щоб дозволити або відхилити.';

  @override
  String get aiToolsDecisionFailed =>
      'Не вдалося надіслати відповідь. Запит буде показано знову.';

  @override
  String get aiToolsHide => 'Приховати від інструментів ШІ';

  @override
  String get aiToolsShow => 'Показувати інструментам ШІ';

  @override
  String get aiToolsHiddenState => 'Приховано від інструментів ШІ';

  @override
  String get quickPasteTabQueue => 'Черга';

  @override
  String get quickPasteMaskedMode =>
      'Поле пароля: попередній перегляд приховано';

  @override
  String quickPasteMaskedItem(String kind) {
    return 'Приховано: $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'Розблокувати секретні елементи';

  @override
  String quickPastePastingWith(String name) {
    return 'Вставлення з: $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut вставляє оригінал';
  }

  @override
  String get quickPasteDefaultSkipped => 'Типове перетворення пропущено';

  @override
  String get quickPasteDefaultSkippedMessage =>
      'Натомість використано оригінал.';

  @override
  String get quickPastePasteManually =>
      'Вміст у буфері обміну. Вставте його вручну.';

  @override
  String get quickPastePaste => 'Вставити';

  @override
  String get quickPasteSnippetsUnavailable => 'Фрагменти недоступні';

  @override
  String get rulesLoadFailed => 'Не вдалося завантажити правила вмісту.';

  @override
  String get rulesNewRule => 'Нове правило';

  @override
  String get rulesEditRule => 'Редагувати правило';

  @override
  String get rulesUntitled => 'Правило без назви';

  @override
  String get rulesPrivacyDefaultsTitle => 'Типові налаштування приватності';

  @override
  String get rulesPrivacyDefaultsDescription =>
      'Ці налаштування приватності захищають кожну копію на цьому пристрої ще до запуску будь-якого правила. Вбудовані правила нижче синхронізуються між вашими пристроями.';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'приватність прихований тимчасовий виключені програми вбудовані облікові дані паролі картки';

  @override
  String rulesPrivacyState(String setting, String state) {
    return '$setting: $state';
  }

  @override
  String rulesExcludedApps(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count програми',
      many: '$count програм',
      few: '$count програми',
      one: '$count програма',
      zero: 'немає',
    );
    return '$_temp0';
  }

  @override
  String get rulesOpenPrivacy => 'Відкрити Приватність';

  @override
  String get rulesMyRulesTitle => 'Мої правила';

  @override
  String get rulesMyRulesDescription =>
      'Вони виконуються згори донизу для нових копій після вбудованих правил і синхронізуються між вашими пристроями.';

  @override
  String get rulesMyRulesKeywords =>
      'правила автоматизація тег пропустити секрет термін видалити шаблон регулярний вираз';

  @override
  String get rulesEmptyTitle => 'Ще немає правил';

  @override
  String get rulesEmptyMessage =>
      'Правило може автоматично додавати теги, захищати, обмежувати строк або пропускати нові копії.';

  @override
  String get rulesDeletedTitle => 'Нещодавно видалені';

  @override
  String get rulesDeletedDescription =>
      'Видалені правила можна відновити протягом 30 днів.';

  @override
  String rulesDeletedAt(String time) {
    return 'Видалено: $time';
  }

  @override
  String get rulesRestore => 'Відновити';

  @override
  String get rulesDeletedToast => 'Правило видалено';

  @override
  String get rulesDeletedToastMessage =>
      'Його можна відновити протягом 30 днів.';

  @override
  String get rulesUndo => 'Скасувати';

  @override
  String rulesConflictMessage(String rule, String device) {
    return 'Одночасну зміну правила «$rule» з пристрою $device замінено новішою.';
  }

  @override
  String get rulesAnotherDevice => 'іншого пристрою';

  @override
  String get rulesRestoreConflict => 'Використати ту версію';

  @override
  String get rulesSkipLogTitle => 'Нещодавно пропущені';

  @override
  String get rulesSkipLogDescription =>
      'Копії, які ваші правила не зберегли на цьому пристрої після запуску CopyPaste. Зберігаються лише час, програма та правило.';

  @override
  String get rulesSkipLogKeywords =>
      'пропущені проігноровані не збережені журнал';

  @override
  String get rulesSkipLogShow => 'Показати';

  @override
  String get rulesSkipLogEmpty => 'Жодну копію не пропущено.';

  @override
  String get rulesUnknownApp => 'Невідома програма';

  @override
  String get rulesMatchesEverything => 'будь-яка копія';

  @override
  String rulesSummary(String conditions, String actions) {
    return 'Якщо $conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary, далі зупинити';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': ' або ',
      'other': ' і ',
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
    return 'Працює на: $devices';
  }

  @override
  String get rulesRunsOnAll => 'Працює на всіх пристроях';

  @override
  String rulesMatchCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Тут спрацювало $count разу',
      many: 'Тут спрацювало $count разів',
      few: 'Тут спрацювало $count рази',
      one: 'Тут спрацювало $count раз',
    );
    return '$_temp0';
  }

  @override
  String rulesLastMatched(String time) {
    return 'востаннє $time';
  }

  @override
  String rulesNewFromDevice(String device) {
    return 'Нове з $device';
  }

  @override
  String get rulesNeedsUpdateBadge => 'Потрібне оновлення';

  @override
  String get rulesUpdateRequired =>
      'Оновіть CopyPaste, щоб виконувати це правило.';

  @override
  String get rulesErrorBadge => 'Помилка';

  @override
  String rulesErrorHint(String problem) {
    return '$problem Доки це не виправлено, копії, яких може стосуватися це правило, зберігаються як секретні та приховуються від інструментів ШІ.';
  }

  @override
  String get rulesReorder => 'Перетягніть, щоб змінити порядок';

  @override
  String get rulesMoreActions => 'Інші дії';

  @override
  String get rulesEdit => 'Редагувати';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': 'Перемістити вгору',
      'down': 'Перемістити вниз',
      'top': 'На початок',
      'other': 'У кінець',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => 'Застосувати до наявних записів…';

  @override
  String get rulesResetBuiltin => 'Скинути до типових';

  @override
  String get rulesNameLabel => 'Назва';

  @override
  String get rulesNamePlaceholder => 'Наприклад, Робочі посилання';

  @override
  String get rulesWhenTitle => 'Коли';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'Виконується будь-яка умова',
      'other': 'Виконуються всі умови',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions =>
      'Без умов: це правило застосовується до кожної копії.';

  @override
  String get rulesAddCondition => 'Додати умову';

  @override
  String get rulesRemoveCondition => 'Видалити умову';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'Текст',
      'content_type': 'Тип вмісту',
      'detected_kind': 'Виявлений тип',
      'source_app': 'Програма-джерело',
      'size': 'Розмір',
      'privacy_marker': 'Позначка приватності',
      'capture_origin': 'Походження',
      'other': 'Невідома умова',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'містить',
      'equals': 'дорівнює',
      'starts_with': 'починається з',
      'ends_with': 'закінчується на',
      'matches_regex': 'відповідає шаблону',
      'other': 'відповідає',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'не містить',
      'equals': 'не дорівнює',
      'starts_with': 'не починається з',
      'ends_with': 'не закінчується на',
      'matches_regex': 'не відповідає шаблону',
      'other': 'не відповідає',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'не є',
      'other': 'є',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'менший за',
      'other': 'більший за',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'не менший за',
      'other': 'не більший за',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'рядки',
      'bytes': 'байти',
      'other': 'символи',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'рядків',
      'bytes': 'байтів',
      'other': 'символів',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'Звичайний текст',
      'rich_text': 'Форматований текст',
      'image': 'Зображення',
      'files': 'Файли',
      'color': 'Колір',
      'link': 'Посилання',
      'other': 'Інше',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'Пароль або секрет',
      'url': 'Вебадреса',
      'email': 'Адреса електронної пошти',
      'phone': 'Номер телефону',
      'code': 'Код',
      'color': 'Значення кольору',
      'json': 'JSON',
      'file_path': 'Шлях до файлу',
      'number': 'Число',
      'one_time_code': 'Одноразовий код',
      'payment_card': 'Номер картки',
      'other': 'Інше',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': 'Прихований',
      'transient': 'Тимчасовий',
      'auto_generated': 'Створений автоматично',
      'excluded_from_history': 'Виключений з історії',
      'other': 'Інше',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'Скопійовано на цьому пристрої',
      'received': 'Отримано з іншого пристрою',
      'created_in_app': 'Створено в CopyPaste',
      'other': 'Інше',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => 'Текст для пошуку';

  @override
  String get rulesPatternPlaceholder => 'Шаблон, наприклад ^ticket-[0-9]+';

  @override
  String get rulesPatternHint =>
      'Шаблони перевіряють перший 1 МіБ тексту. Перегляд уперед, назад і зворотні посилання не підтримуються.';

  @override
  String get rulesCaseSensitive => 'Враховувати регістр';

  @override
  String get rulesChooseValues => 'Вибрати';

  @override
  String get rulesUnknownConditionNotice =>
      'Ця умова походить із новішої версії CopyPaste.';

  @override
  String get rulesSourceAppNotice =>
      'Android знає програму-джерело лише для копій, зроблених клавіатурою CopyPaste. Інші копії на Android ніколи не відповідають цій умові.';

  @override
  String get rulesPrivacyMarkerNotice =>
      'Кожна система повідомляє різні позначки. Позначка, про яку система не повідомляє, ніколи не збігається.';

  @override
  String get rulesThenTitle => 'Тоді';

  @override
  String get rulesNoActions => 'Додайте принаймні одну дію.';

  @override
  String get rulesAddAction => 'Додати дію';

  @override
  String get rulesRemoveAction => 'Видалити дію';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': 'Не зберігати',
      'mark_secret': 'Позначити як секрет',
      'hide_from_ai': 'Приховати від інструментів ШІ',
      'delete_after': 'Видалити через',
      'add_tags': 'Додати теги',
      'transform': 'Перетворити',
      'other': 'Невідома дія',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone': '«Не зберігати» не можна поєднувати з іншими діями.',
      'present': 'Це правило вже має цю дію.',
      'other': 'Правило може мати до 8 дій.',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'Ця дія походить із новішої версії CopyPaste.';

  @override
  String get rulesDontSaveWarning =>
      'Відповідні копії ніколи не зберігаються, не індексуються й не синхронізуються.';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count хвилини',
      many: '$count хвилин',
      few: '$count хвилини',
      one: '$count хвилина',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count години',
      many: '$count годин',
      few: '$count години',
      one: '$count година',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
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
  String get rulesClearClipboardAtExpiry =>
      'Також очистити буфер обміну в цей час, якщо в ньому досі ця копія. На Android це станеться під час наступного запуску CopyPaste.';

  @override
  String get rulesNoTags =>
      'Створіть теги в Історії, щоб використовувати їх тут.';

  @override
  String get rulesNoTransforms => 'Немає доступних перетворень.';

  @override
  String get rulesReplaceClipboard =>
      'Також замінити вміст буфера обміну результатом';

  @override
  String get rulesOptionsTitle => 'Параметри';

  @override
  String get rulesEnabledSwitch => 'Увімкнено';

  @override
  String get rulesStopProcessing =>
      'Не обробляти наступні правила, якщо це спрацювало';

  @override
  String get rulesRunsOnLabel => 'Працює на';

  @override
  String get rulesAllDevices => 'Усі пристрої';

  @override
  String get rulesChosenDevices => 'Вибрані пристрої';

  @override
  String get rulesTestTitle => 'Перевірка';

  @override
  String get rulesTestDescription =>
      'Спробуйте правило на прикладі тексту. Приклад ніколи не зберігається й не синхронізується.';

  @override
  String get rulesSamplePlaceholder => 'Вставте або введіть приклад тексту';

  @override
  String get rulesTestFullSet => 'Перевіряти разом з усіма правилами';

  @override
  String get rulesRunTest => 'Перевірити';

  @override
  String get rulesTestFailed => 'Не вдалося виконати перевірку.';

  @override
  String get rulesPreviewHistory => 'Переглянути на історії';

  @override
  String rulesSaveBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'read_only': 'Оновіть CopyPaste, щоб редагувати це правило.',
      'name_missing': 'Дайте правилу назву.',
      'name_too_long': 'Використайте назву до 128 символів.',
      'no_actions': 'Додайте принаймні одну дію.',
      'other': 'Виправте позначені поля.',
    });
    return '$_temp0';
  }

  @override
  String get rulesSaveFailed => 'Не вдалося зберегти правило.';

  @override
  String get rulesReadOnlyNotice =>
      'Це правило створила новіша версія CopyPaste. Оновіть CopyPaste, щоб редагувати або виконувати його.';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'Не вдалося прочитати правило.',
      'unsupported_schema': 'Це правило потребує новішої версії CopyPaste.',
      'unknown_condition_type': 'Ця умова потребує новішої версії CopyPaste.',
      'unknown_action_type': 'Ця дія потребує новішої версії CopyPaste.',
      'too_many_conditions': 'Правило може мати до 20 умов.',
      'too_many_actions': 'Правило може мати до 8 дій.',
      'too_many_rules': 'Виконуються лише перші 200 правил.',
      'no_actions': 'Додайте принаймні одну дію.',
      'conflicting_actions':
          '«Не зберігати» не можна поєднувати з іншими діями.',
      'duplicate_action': 'Кожну дію можна використати лише один раз.',
      'empty_value': 'Введіть значення.',
      'empty_list': 'Виберіть принаймні одне значення.',
      'too_many_values': 'Виберіть до 64 значень.',
      'invalid_value': 'Це значення недійсне.',
      'pattern_too_long': 'Шаблони можуть мати до 2000 символів.',
      'unsupported_look_around':
          'Перегляд уперед і назад не підтримується. Натомість використайте дві умови з «Виконуються всі умови».',
      'unsupported_backreference': 'Зворотні посилання не підтримуються.',
      'pattern_too_large':
          'Цей шаблон надто складний. Спростіть його повторення.',
      'invalid_pattern': 'Цей шаблон недійсний.',
      'invalid_duration': 'Виберіть тривалість від 1 хвилини до 90 днів.',
      'invalid_tags': 'Виберіть принаймні один тег.',
      'invalid_transform': 'Виберіть перетворення.',
      'other': 'У цьому правилі є проблема.',
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
      'matched': 'збігається',
      'not_matched': 'не збігається',
      'unknown': 'невідомо',
      'skipped': 'не виконувалося',
      'other': 'помилка',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return 'Умова $number';
  }

  @override
  String get rulesTraceNoRules => 'Жодне правило не перевірялося.';

  @override
  String get rulesTraceStopped => 'Наступні правила не виконувалися.';

  @override
  String get rulesTraceUnknownReason =>
      'Для цієї копії невідомо, тож умова не збігається в жодному разі.';

  @override
  String get rulesTraceRuleError =>
      'Це правило не вдалося виконати тут, тому відповідні копії натомість захищаються.';

  @override
  String get rulesTraceOutcomeTitle => 'Результат';

  @override
  String get rulesTraceNoEffect => 'Жодне правило не змінює цю копію.';

  @override
  String get rulesTraceSkipped => 'Не зберігається';

  @override
  String get rulesTraceSecret => 'Зберігається як секрет';

  @override
  String get rulesTraceHidden => 'Прихована від інструментів ШІ';

  @override
  String rulesTraceExpires(String duration) {
    return 'Видаляється через $duration';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'Теги: $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return 'Перетворено: $names';
  }

  @override
  String get rulesTraceOutput => 'Перетворений текст';

  @override
  String get rulesPreviewTitle => 'Застосувати до наявних записів';

  @override
  String rulesPreviewRunning(int count) {
    return 'Перевірка останніх $count записів…';
  }

  @override
  String get rulesPreviewNone =>
      'Жоден нещодавній запис не відповідає цьому правилу.';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Відповідають $count запису',
      many: 'Відповідають $count записів',
      few: 'Відповідають $count записи',
      one: 'Відповідає $count запис',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count буде видалено',
      many: '$count буде видалено',
      few: '$count буде видалено',
      one: '$count буде видалено',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count буде позначено як секрет',
      many: '$count буде позначено як секрет',
      few: '$count буде позначено як секрет',
      one: '$count буде позначено як секрет',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count буде приховано від ШІ',
      many: '$count буде приховано від ШІ',
      few: '$count буде приховано від ШІ',
      one: '$count буде приховано від ШІ',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count отримають час видалення',
      many: '$count отримають час видалення',
      few: '$count отримають час видалення',
      one: '$count отримає час видалення',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count отримають теги',
      many: '$count отримають теги',
      few: '$count отримають теги',
      one: '$count отримає теги',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return 'Перевірено лише останні $count записів.';
  }

  @override
  String rulesPreviewMore(int count) {
    return 'і ще $count';
  }

  @override
  String get rulesPreviewNote =>
      'Нічого не зміниться, доки ви не застосуєте. Зміни синхронізуються з іншими пристроями як звичайні редагування.';

  @override
  String get rulesSecretItem => 'Секретний запис';

  @override
  String rulesSecretItemFrom(String app) {
    return 'Секретний запис із $app';
  }

  @override
  String get rulesPreviewCancelled => 'Перегляд зупинено. Нічого не змінено.';

  @override
  String get rulesPreviewFailed => 'Не вдалося виконати перегляд.';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Видалити $count запису?',
      many: 'Видалити $count записів?',
      few: 'Видалити $count записи?',
      one: 'Видалити $count запис?',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage =>
      'Застосування «Не зберігати» до наявних записів видаляє відповідні записи з Історії на всіх ваших пристроях. Цю дію не можна скасувати.';

  @override
  String get rulesApplyDeleteConfirm => 'Видалити й застосувати';

  @override
  String rulesApplyProgress(int done, int total) {
    return 'Застосування: $done з $total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Змінено $count запису',
      many: 'Змінено $count записів',
      few: 'Змінено $count записи',
      one: 'Змінено $count запис',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Видалено $count запису',
      many: 'Видалено $count записів',
      few: 'Видалено $count записи',
      one: 'Видалено $count запис',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Не вдалося змінити $count запису',
      many: 'Не вдалося змінити $count записів',
      few: 'Не вдалося змінити $count записи',
      one: 'Не вдалося змінити $count запис',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled =>
      'Зупинено. Записи, змінені до зупинки, залишаються зміненими.';

  @override
  String get historyActionEdit => 'Редагувати';

  @override
  String get historyActionSaveAsSnippet => 'Зберегти як фрагмент';

  @override
  String get historyActionAddToQueue => 'Додати до Paste Queue';

  @override
  String get historyActionTransform => 'Перетворити…';

  @override
  String get historyActionExplain => 'Чому це тут?';

  @override
  String get historyActionKeep => 'Залишити';

  @override
  String get historyNewClip => 'Новий кліп';

  @override
  String get historySavedTitle => 'Збережено';

  @override
  String get historyEditSaved =>
      'Змінений текст став новим кліпом угорі історії.';

  @override
  String get historyNewClipSaved => 'Ваш текст збережено.';

  @override
  String get historySnippetSaved => 'Текст кліпу тепер є фрагментом.';

  @override
  String get historySnippetSaveFailed =>
      'Не вдалося зберегти цей кліп як фрагмент.';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count кліпу додано до черги в порядку вибору.',
      many: '$count кліпів додано до черги в порядку вибору.',
      few: '$count кліпи додано до черги в порядку вибору.',
      one: '$count кліп додано до черги.',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed =>
      'Не вдалося додати до Paste Queue. Перевірте, чи її ввімкнено.';

  @override
  String get historyHideFromAiFailed =>
      'Не вдалося змінити налаштування інструментів ШІ для цього кліпу.';

  @override
  String get historyKeepFailed => 'Не вдалося залишити цей кліп.';

  @override
  String get historyExplainFailed => 'Не вдалося завантажити пояснення.';

  @override
  String get historyDeletesSoon => 'Видалиться менш ніж за хвилину';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Видалиться через $count хв',
      many: 'Видалиться через $count хв',
      few: 'Видалиться через $count хв',
      one: 'Видалиться через $count хв',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Видалиться через $count год',
      many: 'Видалиться через $count год',
      few: 'Видалиться через $count год',
      one: 'Видалиться через $count год',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Видалиться через $count дня',
      many: 'Видалиться через $count днів',
      few: 'Видалиться через $count дні',
      one: 'Видалиться через $count день',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => 'Вручну';

  @override
  String get historyProvenanceEdited => 'Змінено';

  @override
  String get historyProvenanceTransformed => 'Перетворено';

  @override
  String get historyProvenanceAiTool => 'Інструмент ШІ';

  @override
  String historyRuleTag(String name) {
    return '$name (авто)';
  }

  @override
  String get historyMetadataExpiry => 'Видалення';

  @override
  String get historyMetadataRuleTags => 'Автоматичні теги';

  @override
  String get historyMetadataAiTools => 'Інструменти ШІ';
}
