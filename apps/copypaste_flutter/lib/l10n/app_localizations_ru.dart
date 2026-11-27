// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Russian (`ru`).
class AppLocalizationsRu extends AppLocalizations {
  AppLocalizationsRu([String locale = 'ru']) : super(locale);

  @override
  String get commonCancel => 'Отмена';

  @override
  String get commonClose => 'Закрыть';

  @override
  String get commonDone => 'Готово';

  @override
  String get commonRetry => 'Повторить';

  @override
  String get commonTryAgain => 'Попробовать ещё раз';

  @override
  String get commonCopy => 'Копировать';

  @override
  String get commonDelete => 'Удалить';

  @override
  String get commonSave => 'Сохранить';

  @override
  String get commonContinue => 'Продолжить';

  @override
  String get commonBack => 'Назад';

  @override
  String get commonOpenSettings => 'Открыть настройки';

  @override
  String get commonLoading => 'Загрузка';

  @override
  String get commonApply => 'Применить';

  @override
  String get commonClear => 'Очистить';

  @override
  String get commonPin => 'Закрепить';

  @override
  String get commonEnable => 'Включить';

  @override
  String get settingsLanguageTitle => 'Язык';

  @override
  String get settingsLanguageDescription =>
      'Выберите язык интерфейса CopyPaste.';

  @override
  String get settingsLanguageSystem => 'Как в системе';

  @override
  String get settingsLanguageKeywords =>
      'язык локаль перевод интерфейс language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Не удалось изменить автозапуск при входе. Попробуйте ещё раз.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'Не удалось открыть системные настройки автозапуска.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Не удалось обновить настройки. Попробуйте ещё раз.';

  @override
  String get settingsRuntimeLoadFailed => 'Не удалось загрузить настройки.';

  @override
  String get settingsRuntimeAppAlreadyExcluded =>
      'Это приложение уже исключено.';

  @override
  String get settingsRuntimeChangeFailed =>
      'Не удалось выполнить запрошенное изменение.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Чтобы возобновить запись буфера обмена, предоставьте все необходимые разрешения.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'Запись буфера обмена приостановлена. Служба записи Android не подтвердила остановку.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'Чтобы включить уведомления о копировании, нужно разрешение на уведомления.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'Не удалось прочитать сохранённые настройки CopyPaste. Действуют защитные значения; проверьте и сохраните параметры, чтобы восстановить их.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'Нечитаемые настройки CopyPaste заменены защитными значениями. Проверьте и сохраните параметры, чтобы восстановить их.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Идентификатор приложения не может быть пустым и должен занимать не более 256 байт.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'Можно исключить до 256 приложений.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Не удалось изменить автовставку.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'Не удалось зарегистрировать это сочетание клавиш.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'Не удалось временно отключить текущее сочетание клавиш.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'Не удалось восстановить сочетание клавиш.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'Не удалось открыть Quick Paste.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Не удалось запросить разрешение «Универсальный доступ».';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Не удалось обновить состояние интеграции с Linux.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'Разрешение на ввод из буфера обмена не предоставлено.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Не удалось запросить разрешение на ввод из буфера обмена.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Не удалось открыть настройку интеграции с рабочим окружением.';

  @override
  String get settingsRuntimeTextHistoryFileType =>
      'Текстовая история CopyPaste';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'Зашифрованная резервная копия CopyPaste';

  @override
  String get settingsRuntimeNotificationImage => 'Изображение';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Изображение · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'Файл';

  @override
  String get updateRestoreInstallationFailed =>
      'Не удалось восстановить установку обновления CopyPaste.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Это установка для Linux не обновляется автоматически.';

  @override
  String get updateMacosPackageMissing =>
      'В этом выпуске нет подписанного пакета обновления для macOS.';

  @override
  String get updateInstalledVersionInvalid =>
      'Установленная версия CopyPaste недействительна.';

  @override
  String get updateCheckUnreachable =>
      'Не удалось связаться с GitHub, чтобы проверить обновления.';

  @override
  String get updateCheckTimedOut =>
      'Время ожидания проверки обновлений истекло.';

  @override
  String get updateCheckDeviceFailed =>
      'Не удалось проверить обновления на этом устройстве.';

  @override
  String get updateCheckFailed => 'Не удалось проверить обновления.';

  @override
  String get updateDownloadFailed => 'Не удалось загрузить обновление.';

  @override
  String get updateDownloadTimedOut =>
      'Время ожидания загрузки обновления истекло.';

  @override
  String get updateInstallFailed => 'Не удалось установить обновление.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Продолжите в системном установщике Android.';

  @override
  String get updateContinueInPackageManager =>
      'Продолжите в системном менеджере пакетов.';

  @override
  String get updateInstallerStarting => 'Установщик запускается.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Разрешите CopyPaste устанавливать приложения в настройках Android и продолжите.';

  @override
  String get updatePermissionRequiredLinux =>
      'Пройдите аутентификацию в системном менеджере пакетов, чтобы установить это обновление.';

  @override
  String get updatePermissionRequired =>
      'Разрешите CopyPaste установить это обновление и продолжите.';

  @override
  String get updateRestartRequired =>
      'Обновление установлено. Перезапустите CopyPaste, чтобы начать им пользоваться.';

  @override
  String get updateRestarting => 'Перезапуск CopyPaste.';

  @override
  String get updateRestartFailed =>
      'Не удалось перезапустить CopyPaste. Попробуйте ещё раз.';

  @override
  String get updateReleasePageOpenFailed =>
      'Не удалось открыть страницу выпуска.';

  @override
  String get updatePackageInvalid =>
      'Загруженное обновление не является действительным пакетом CopyPaste.';

  @override
  String get updateSignatureInvalid =>
      'Загруженное обновление имеет недействительную подпись.';

  @override
  String get updateDowngradeRefused =>
      'Загруженный пакет не новее этой версии.';

  @override
  String get updateHomebrewRequired =>
      'Установите CopyPaste через Homebrew, чтобы обновлять его здесь.';

  @override
  String get updateInstallationCancelled => 'Установка обновления отменена.';

  @override
  String get updateInstallationBlocked =>
      'Android заблокировал установку обновления.';

  @override
  String get updateInstallationConflict =>
      'Обновление конфликтует с установленным приложением.';

  @override
  String get updateInstallationIncompatible =>
      'Обновление несовместимо с этим устройством.';

  @override
  String get updateInstallationStorage =>
      'Освободите место в памяти, чтобы установить обновление.';

  @override
  String get updateInstallationInterrupted =>
      'Установка обновления прервана. Попробуйте ещё раз.';

  @override
  String get updateInstallationBusy => 'Установка обновления уже выполняется.';

  @override
  String get updatePackageDetailsInvalid =>
      'Сведения о пакете обновления недействительны.';

  @override
  String get updateIntegrityCheckFailed =>
      'Загруженное обновление не прошло проверку целостности.';

  @override
  String get updateInstallerLaunchFailed =>
      'Не удалось запустить системный установщик.';

  @override
  String get updateInstallLocationTranslocated =>
      'Переместите CopyPaste в папку «Программы» и откройте оттуда, чтобы обновить.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste не может заменить себя в текущей папке. Воспользуйтесь страницей выпуска, чтобы обновить.';

  @override
  String get updateReplacementBlocked =>
      'macOS заблокировала замену CopyPaste. Разрешите её в настройках «Управление приложениями» или воспользуйтесь страницей выпуска.';

  @override
  String get updatePackageNotDownloadable =>
      'Эта платформа не использует загружаемый пакет обновления.';

  @override
  String get updatePackageSizeInvalid =>
      'Недействительный размер пакета обновления.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'Загруженное обновление имеет недействительную подпись средства обновления.';

  @override
  String get updatePackageUrlUntrusted =>
      'Адрес пакета обновления не является доверенным.';

  @override
  String get updateDownloadLeftGitHub =>
      'Загрузка обновления перенаправлена за пределы GitHub.';

  @override
  String get updateDownloadTooManyRedirects =>
      'При загрузке обновления произошло слишком много перенаправлений.';

  @override
  String get updatePackageTooLarge => 'Пакет обновления слишком большой.';

  @override
  String get updateSignatureSizeInvalid =>
      'Недействительный размер подписи средства обновления.';

  @override
  String get updateMetadataInvalid =>
      'GitHub вернул недействительные метаданные обновления.';

  @override
  String get updateMetadataTooLarge => 'Метаданные обновления слишком большие.';

  @override
  String get quickPasteActionFailedTitle => 'Действие не выполнено';

  @override
  String get quickPasteOpenAppSemantics => 'Открыть CopyPaste';

  @override
  String get quickPasteSearchPlaceholder => 'Введите текст для поиска…';

  @override
  String get quickPasteLoadingHistory => 'Загрузка истории буфера обмена.';

  @override
  String get quickPasteHistoryUnavailable => 'История недоступна';

  @override
  String get quickPasteNoClipsFound => 'Ничего не найдено';

  @override
  String get quickPasteLoadMore => 'Загрузить ещё';

  @override
  String get quickPasteTransferFailed => 'Ошибка';

  @override
  String get quickPasteClipUnavailable => 'Элемент недоступен';

  @override
  String get quickPasteFileFallbackName => 'Файл';

  @override
  String get quickPastePathUnavailable => 'Путь недоступен';

  @override
  String get quickPastePreferences => 'Настройки…';

  @override
  String get quickPasteAbout => 'О программе';

  @override
  String get quickPasteQuit => 'Выйти';

  @override
  String get quickPasteClearHistoryTitle => 'Очистить историю?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Закреплённые элементы останутся в истории.';

  @override
  String get quickPasteClearHistoryFailed => 'Не удалось удалить историю.';

  @override
  String get quickPasteAboutDescription =>
      'Приватная история буфера обмена для macOS, Android, Windows и Linux.';

  @override
  String get transfersTitle => 'Загрузки и передачи';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count активной передачи',
      many: '$count активных передач',
      few: '$count активные передачи',
      one: '$count активная передача',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count активной',
      many: '$count активных',
      few: '$count активные',
      one: '$count активная',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Последние';

  @override
  String get transfersShowEarlier => 'Ранние';

  @override
  String get transfersClearCompletedHistory => 'Очистить историю завершённых';

  @override
  String get transfersErrorTitle => 'Передачи';

  @override
  String get transfersEmptyTitle => 'Нет передач';

  @override
  String get transfersEmptyMessage =>
      'Здесь появятся импортированные файлы, загрузки и исходящие передачи.';

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
  String get transfersRemoveFromHistory => 'Удалить из истории';

  @override
  String get transfersCancelDownload => 'Отменить загрузку';

  @override
  String get transfersShowFile => 'Показать файл';

  @override
  String get transfersPhaseSending => 'Отправка';

  @override
  String get transfersPhaseDownloading => 'Загрузка';

  @override
  String get transfersPhaseAdding => 'Добавление';

  @override
  String get transfersPhaseAdded => 'Добавлено';

  @override
  String get transfersPhaseSent => 'Отправлено';

  @override
  String get transfersPhaseCompleted => 'Завершено';

  @override
  String get transfersPhaseCancelled => 'Отменено';

  @override
  String get transfersPhaseInterrupted => 'Прервано';

  @override
  String get transfersPhaseFailed => 'Ошибка';

  @override
  String get transfersUpdatesUnavailable =>
      'Обновления состояния передач недоступны. Откройте снова, чтобы обновить.';

  @override
  String get transfersLoadFailed =>
      'Не удалось загрузить передачи. Попробуйте ещё раз.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Не удалось загрузить обновления состояния передач.';

  @override
  String get transfersRetryFailed =>
      'Не удалось повторить передачу этого файла.';

  @override
  String get transfersClearHistoryFailed =>
      'Не удалось очистить историю передач.';

  @override
  String get transfersRemoveRecordFailed =>
      'Не удалось удалить эту запись истории.';

  @override
  String get transfersCancelFailed =>
      'Не удалось отменить передачу. Возможно, она всё ещё выполняется.';

  @override
  String get transfersSavedFileUnavailable =>
      'Сохранённый файл недоступен. Возможно, его переместили или удалили.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Уведомления · $count непрочитанного',
      many: 'Уведомления · $count непрочитанных',
      few: 'Уведомления · $count непрочитанные',
      one: 'Уведомления · $count непрочитанное',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Уведомления';

  @override
  String get notificationsClearRead => 'Очистить прочитанные уведомления';

  @override
  String get notificationsEmptyTitle => 'Нет уведомлений';

  @override
  String get notificationsEmptyMessage =>
      'Здесь появятся запросы на подключение устройств и их результаты.';

  @override
  String get notificationsSystemAlertTitle => 'Системные уведомления';

  @override
  String get notificationsReview => 'Просмотреть';

  @override
  String get notificationsConnectionRequest => 'Запрос на подключение';

  @override
  String get notificationsConfirmConnection => 'Подтвердите подключение';

  @override
  String get notificationsDeviceConnected => 'Устройство подключено';

  @override
  String get notificationsConnectionRejected => 'Подключение отклонено';

  @override
  String get notificationsConnectionCancelled => 'Подключение отменено';

  @override
  String get notificationsConnectionRequestExpired =>
      'Срок действия запроса на подключение истёк';

  @override
  String get notificationsConnectionFailed => 'Не удалось подключиться';

  @override
  String get notificationsNearbyDevice => 'Устройство поблизости';

  @override
  String get notificationsPermissionDenied =>
      'Разрешите системные уведомления в настройках ОС, чтобы получать запросы в фоновом режиме.';

  @override
  String get notificationsSystemUnavailable =>
      'Системные уведомления недоступны. Запросы на подключение остаются здесь.';

  @override
  String get platformCaptureStateUnavailable => 'Недоступно';

  @override
  String get platformCaptureStatePaused => 'Приостановлено';

  @override
  String get platformCaptureStateRunning => 'Запись идёт';

  @override
  String get platformCaptureStateStopped => 'Не запущено';

  @override
  String get platformCaptureActionUnavailable => 'Запись недоступна';

  @override
  String get platformCaptureActionResume => 'Возобновить запись';

  @override
  String get platformCaptureActionPause => 'Приостановить запись';

  @override
  String get platformCaptureActionResumeStopped =>
      'Возобновить запись (не запущено)';

  @override
  String get platformTrayOpen => 'Открыть';

  @override
  String get platformTraySettings => 'Настройки';

  @override
  String get platformTrayQuit => 'Выйти';

  @override
  String get platformWindowControlsUnavailable =>
      'Управление окнами недоступно. Окно CopyPaste останется видимым.';

  @override
  String get platformTrayUnavailable =>
      'Системный трей недоступен. Оставьте CopyPaste открытым, затем повторите попытку или выйдите из приложения.';

  @override
  String get platformQuitFailed =>
      'Не удалось выйти из CopyPaste. Попробуйте ещё раз.';

  @override
  String get platformVisibilityChangeFailed =>
      'Не удалось изменить видимость приложения. Попробуйте ещё раз.';

  @override
  String get platformCaptureNotificationTitle => 'Буфер обмена сохранён';

  @override
  String get platformCaptureNotificationBody =>
      'В CopyPaste добавлен новый элемент.';

  @override
  String get platformPairingNotificationTitle => 'Запрос на подключение';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName хочет подключиться. Откройте CopyPaste, чтобы сравнить код и подтвердить подключение.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Запросы на подключение устройств';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Запросы на подключение к устройствам поблизости';

  @override
  String get platformNotificationDefaultAction => 'Открыть';

  @override
  String get platformFilesSelectionFailed => 'Не удалось выбрать файлы.';

  @override
  String get platformFilesOnlyRegular =>
      'Можно импортировать только обычные файлы.';

  @override
  String get platformModulesUnavailable =>
      'Модули недоступны на этом устройстве.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'Не удалось настроить доступ к SMS.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'Не удалось проверить доступ к SMS.';

  @override
  String get platformUpdatesUnavailableHere =>
      'Обновления приложения здесь недоступны.';

  @override
  String get platformUpdateVersionUnavailable =>
      'Установленная версия недоступна.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'Обновления приложения недоступны для этой установки.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Эта установка для Linux не обновляется автоматически.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Это обновление для Linux не соответствует архитектуре системы.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Эта установка для macOS не обновляется автоматически.';

  @override
  String get platformUpdateStartFailed =>
      'Не удалось запустить обновление CopyPaste.';

  @override
  String get platformGlobalShortcutDescription => 'Открыть Quick Paste';

  @override
  String get settingsSectionGeneral => 'Общие';

  @override
  String get settingsSectionGeneralSummary => 'Язык и видимость приложения.';

  @override
  String get settingsSectionAppearance => 'Оформление';

  @override
  String get settingsSectionAppearanceSummary => 'Тема и материал окон.';

  @override
  String get settingsAppearanceDescription => 'Тема и внешний вид окон.';

  @override
  String get settingsThemeTitle => 'Тема';

  @override
  String get settingsThemeDescription => 'Светлая, тёмная или как в системе.';

  @override
  String get settingsThemeKeywords => 'тёмная тема светлая тема цветовая схема';

  @override
  String get settingsThemeSystem => 'Системная';

  @override
  String get settingsThemeLight => 'Светлая';

  @override
  String get settingsThemeDark => 'Тёмная';

  @override
  String get settingsAccentTitle => 'Цвет акцента';

  @override
  String get settingsAccentDescription =>
      'Подсвечивает выделение и текущий раздел.';

  @override
  String get settingsAccentKeywords => 'цвет подсветки оттенок';

  @override
  String get settingsAccentSystem => 'Системный';

  @override
  String get settingsAccentStandard => 'По умолчанию';

  @override
  String get settingsAccentPurple => 'Фиолетовый';

  @override
  String get settingsAccentPink => 'Розовый';

  @override
  String get settingsAccentRed => 'Красный';

  @override
  String get settingsAccentOrange => 'Оранжевый';

  @override
  String get settingsAccentYellow => 'Жёлтый';

  @override
  String get settingsAccentGreen => 'Зелёный';

  @override
  String get settingsAccentTeal => 'Бирюзовый';

  @override
  String get settingsAccentGraphite => 'Графитовый';

  @override
  String get settingsSectionClipboard => 'Буфер обмена';

  @override
  String get settingsSectionClipboardSummary => 'Запись и ограничения истории.';

  @override
  String get settingsSectionPrivacy => 'Конфиденциальность';

  @override
  String get settingsSectionPrivacySummary =>
      'Исключённые приложения и защита экрана.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Сочетание клавиш и автовставка.';

  @override
  String get settingsSectionSync => 'Синхронизация';

  @override
  String get settingsSectionSyncSummary =>
      'Синхронизация и поиск устройств поблизости.';

  @override
  String get settingsSectionNotifications => 'Уведомления';

  @override
  String get settingsSectionNotificationsSummary =>
      'Уведомления и звуки записи.';

  @override
  String get settingsSectionData => 'Данные';

  @override
  String get settingsSectionDataSummary =>
      'Экспорт, резервное копирование и восстановление.';

  @override
  String get settingsSectionModules => 'Модули';

  @override
  String get settingsSectionModulesSummary =>
      'Установка и управление дополнительными модулями.';

  @override
  String get settingsSectionRules => 'Правила содержимого';

  @override
  String get settingsSectionRulesSummary =>
      'Автоматически назначайте теги, пропускайте, защищайте элементы и задавайте срок их хранения.';

  @override
  String get settingsSectionQuickAccess => 'Быстрый доступ';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Клавиатура, плитки и виджет главного экрана.';

  @override
  String get settingsSectionTransforms => 'Преобразования текста';

  @override
  String get settingsSectionTransformsSummary =>
      'Избранное, цепочки и преобразования при вставке по умолчанию.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Собирайте несколько копий и вставляйте их по порядку.';

  @override
  String pasteQueuePasted(int position, int total) {
    return 'Вставлено $position из $total';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'Элемент $position из $total в буфере обмена';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Вставлено $count элемента',
      many: 'Вставлено $count элементов',
      few: 'Вставлено $count элемента',
      one: 'Вставлен $count элемент',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count элемента в буфере обмена',
      many: '$count элементов в буфере обмена',
      few: '$count элемента в буфере обмена',
      one: '$count элемент в буфере обмена',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Очередь завершена';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Добавлено, в очереди $total',
      many: 'Добавлено, в очереди $total',
      few: 'Добавлено, в очереди $total',
      one: 'Добавлено, в очереди $total',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'Очередь пуста, ничего не вставлено';

  @override
  String get pasteQueueMixed =>
      '«Вставить все» работает только с очередью из текста или только из файлов';

  @override
  String get pasteQueueSeveralFiles => 'Вставляйте файлы по одному.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count элемента больше недоступны',
      many: '$count элементов больше недоступны',
      few: '$count элемента больше недоступны',
      one: '$count элемент больше недоступен',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => '«Вставить следующий» только копирует';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Элемент скопирован. Разрешите «Универсальный доступ», чтобы вставлять автоматически.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Не удалось вставить автоматически. Элемент скопирован.';

  @override
  String get pasteQueueTargetElevated =>
      'Нельзя вставить в приложение, запущенное от имени администратора. Элемент скопирован.';

  @override
  String get pasteQueueNoTarget =>
      'Нет приложения для вставки. Элемент скопирован.';

  @override
  String get pasteQueueCollectingStarted => 'Сбор копий';

  @override
  String get pasteQueueCollectingStopped => 'Сбор остановлен';

  @override
  String get pasteQueueFailed => 'Не удалось выполнить действие Paste Queue';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count в ожидании',
      many: '$count в ожидании',
      few: '$count в ожидании',
      one: '$count в ожидании',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count в ожидании · Сбор',
      many: '$count в ожидании · Сбор',
      few: '$count в ожидании · Сбор',
      one: '$count в ожидании · Сбор',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Сбор';

  @override
  String get pasteQueueFinishedSummary => 'Завершено';

  @override
  String pasteQueueFull(int max) {
    return 'Очередь заполнена ($max)';
  }

  @override
  String get pasteQueueFullMessage => 'Новые копии больше не добавляются.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue пуста';

  @override
  String get pasteQueueEmptyMessage =>
      'Собирайте копии или добавляйте элементы из истории.';

  @override
  String get pasteQueueCollectingEmptyTitle =>
      'Копируйте элементы, чтобы добавить их';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Каждая копия попадает в очередь, пока вы не остановите сбор.';

  @override
  String get pasteQueueStartCollecting => 'Начать сбор';

  @override
  String get pasteQueueStopCollecting => 'Остановить сбор';

  @override
  String get pasteQueueLoading => 'Загрузка очереди…';

  @override
  String get pasteQueueLoadFailed => 'Не удалось загрузить очередь';

  @override
  String get pasteQueuePasteNext => 'Вставить следующий';

  @override
  String get pasteQueueCopyNext => 'Копировать следующий';

  @override
  String get pasteQueuePasteAll => 'Вставить все';

  @override
  String get pasteQueueCopyAll => 'Копировать все';

  @override
  String get pasteQueueReverse => 'Обратный порядок';

  @override
  String get pasteQueueStepBack => 'Шаг назад';

  @override
  String get pasteQueueSkip => 'Пропустить';

  @override
  String get pasteQueueRemovePasted => 'Удалить вставленные';

  @override
  String get pasteQueueCleared => 'Очередь очищена';

  @override
  String get pasteQueueUndo => 'Отменить';

  @override
  String get pasteQueueStartFresh => 'Начать заново';

  @override
  String get pasteQueueAppend => 'Добавить в конец';

  @override
  String get pasteQueueRestartTitle => 'Начать новый сбор?';

  @override
  String get pasteQueueRestartMessage =>
      '«Начать заново» удаляет вставленные элементы. «Добавить в конец» сохраняет их.';

  @override
  String pasteQueueStale(String time) {
    return 'Очередь от $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Вставлено';

  @override
  String get pasteQueueStatusSkipped => 'Пропущено';

  @override
  String get pasteQueueStatusUnavailable => 'Больше недоступен';

  @override
  String get pasteQueueEdited => 'Изменено';

  @override
  String get pasteQueueSecret => 'Секрет';

  @override
  String get pasteQueueKindImage => 'Изображение';

  @override
  String get pasteQueueKindFiles => 'Файлы';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Сформировано в $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Элемент $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'Перемещено на позицию $position';
  }

  @override
  String get pasteQueueMoveUp => 'Переместить вверх';

  @override
  String get pasteQueueMoveDown => 'Переместить вниз';

  @override
  String get pasteQueueMoveToTop => 'В начало';

  @override
  String get pasteQueueMoveToBottom => 'В конец';

  @override
  String get pasteQueueReorder => 'Перетащите, чтобы изменить порядок';

  @override
  String get pasteQueueMoreActions => 'Другие действия';

  @override
  String get pasteQueuePasteNow => 'Вставить сейчас';

  @override
  String get pasteQueueEdit => 'Изменить текст';

  @override
  String get pasteQueueUseOriginal => 'Использовать исходный текст';

  @override
  String get pasteQueueRemove => 'Убрать из очереди';

  @override
  String get pasteQueueEditTitle => 'Редактировать текст в очереди';

  @override
  String get pasteQueueEditMessage =>
      'Изменения относятся только к этой очереди.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Настроить сочетания клавиш';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Собирайте и вставляйте, не открывая CopyPaste.';

  @override
  String get pasteQueueShortcutToggleCollecting => 'Начать или остановить сбор';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Вставить следующий как обычный текст';

  @override
  String get pasteQueueShortcutDescription =>
      'Глобальное сочетание клавиш, работающее в любом приложении.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Это сочетание уже использует другое сочетание клавиш CopyPaste. Предыдущее сохранено.';

  @override
  String get pasteQueueShortcutInUse =>
      'Это сочетание использует другое приложение.';

  @override
  String get pasteQueueShortcutRejected =>
      'Система отклонила это сочетание. Предыдущее сохранено.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt вводит символы во многих раскладках. Выберите другое сочетание.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Глобальные сочетания клавиш недоступны в этом сеансе.';

  @override
  String get pasteQueueShortcutFailed =>
      'Не удалось задать это сочетание клавиш.';

  @override
  String get pasteQueueEnableTitle => 'Включить Paste Queue';

  @override
  String get pasteQueueEnableDescription =>
      'Если выключено, сочетания клавиш и сбор не работают.';

  @override
  String get pasteQueuePasteNextStyleTitle => '«Вставить следующий» выполняет';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Вставлять автоматически или только копировать в буфер обмена.';

  @override
  String get pasteQueuePasteIntoApp => 'Вставить в активное приложение';

  @override
  String get pasteQueueClipboardOnly => 'Только в буфер обмена';

  @override
  String get pasteQueueInsertTitle => 'Новые элементы добавляются';

  @override
  String get pasteQueueInsertDescription =>
      'Куда в очереди попадают собранные и добавленные элементы.';

  @override
  String get pasteQueueInsertEnd => 'В конец (первым пришёл — первым вышел)';

  @override
  String get pasteQueueInsertStart =>
      'В начало (последним пришёл — первым вышел)';

  @override
  String get pasteQueueEndBehaviorTitle => 'После последнего элемента';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'Что делает «Вставить следующий», когда все элементы вставлены.';

  @override
  String get pasteQueueEndStop => 'Остановить';

  @override
  String get pasteQueueEndLoop => 'Начать сначала';

  @override
  String get pasteQueueEndKeepLast => 'Продолжать вставлять последний элемент';

  @override
  String get pasteQueueSeparatorTitle => 'Разделитель для «Вставить все»';

  @override
  String get pasteQueueSeparatorDescription =>
      'Ставится между элементами при вставке всех сразу.';

  @override
  String get pasteQueueSeparatorNewLine => 'Новая строка';

  @override
  String get pasteQueueSeparatorBlankLine => 'Пустая строка';

  @override
  String get pasteQueueSeparatorTab => 'Табуляция';

  @override
  String get pasteQueueSeparatorSpace => 'Пробел';

  @override
  String get pasteQueueSeparatorComma => 'Запятая';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Запятая и пробел';

  @override
  String get pasteQueueSeparatorNone => 'Нет';

  @override
  String get pasteQueueSeparatorCustom => 'Свой';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'До 32 символов; можно \\n и \\t';

  @override
  String get pasteQueuePlainTextTitle => 'Всегда вставлять как обычный текст';

  @override
  String get pasteQueuePlainTextDescription =>
      'Убирает форматирование у элементов очереди.';

  @override
  String get pasteQueueIdleStopTitle => 'Останавливать сбор через';

  @override
  String get pasteQueueIdleStopDescription =>
      'Останавливать сбор, если столько времени не было копий.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes мин';
  }

  @override
  String get pasteQueueIdleNever => 'Никогда';

  @override
  String get pasteQueueMaxEntriesTitle => 'Лимит размера очереди';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Когда очередь заполнена, новые копии не добавляются.';

  @override
  String get pasteQueueRestoreTitle =>
      'Восстанавливать предыдущий буфер обмена после завершения';

  @override
  String get pasteQueueRestoreDescription =>
      'Возвращает то, что было скопировано до вставки.';

  @override
  String get pasteQueueSettingsKeywords =>
      'очередь вставки сбор порядок сочетание клавиш горячая клавиша разделитель стек paste queue collect sequence order shortcut hotkey separator stack';

  @override
  String get settingsSectionAbout => 'О программе';

  @override
  String get settingsSectionAboutSummary => 'Версия и обновления приложения.';

  @override
  String get settingsGeneralDescription =>
      'Язык, автозапуск и видимость приложения.';

  @override
  String get settingsModulesDescription => 'Дополнительные функции CopyPaste.';

  @override
  String get settingsClipboardDescription =>
      'Записывайте изменения буфера обмена и управляйте ограничениями истории.';

  @override
  String get settingsDataDescription =>
      'Экспортируйте, создавайте резервные копии и восстанавливайте историю буфера обмена.';

  @override
  String get settingsSyncDescription =>
      'Управляйте синхронизацией с сопряжёнными устройствами.';

  @override
  String get settingsPrivacyDescription =>
      'Выбирайте, из каких приложений вести запись, и защищайте содержимое буфера обмена.';

  @override
  String get settingsNotificationsDescription =>
      'Выберите уведомления и звуки для записанных элементов.';

  @override
  String get settingsAboutDescription =>
      'Версия CopyPaste и обновления приложения.';

  @override
  String get settingsQuickPasteDescription =>
      'Открывайте историю буфера обмена откуда угодно, не переключая окна.';

  @override
  String get settingsLoadingMessage => 'Загрузка настроек.';

  @override
  String get settingsUnavailableTitle => 'Настройки недоступны';

  @override
  String get settingsNoResultsTitle => 'Настройки не найдены';

  @override
  String get settingsNoResultsMessage => 'Попробуйте другой запрос.';

  @override
  String get settingsAttentionTitle => 'Требуется внимание к настройкам';

  @override
  String get settingsSearchPlaceholder => 'Поиск в настройках';

  @override
  String get settingsBackToSettings => 'Назад к настройкам';

  @override
  String get settingsStartOnBootTitle => 'Запуск при включении устройства';

  @override
  String get settingsStartAtLoginTitle => 'Запуск при входе';

  @override
  String get settingsStartupSearchDescription =>
      'Автоматически запускать CopyPaste.';

  @override
  String get settingsStartupKeywords =>
      'автозапуск запуск вход загрузка автоматически startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Отключено в Диспетчере задач. Включите CopyPaste в разделе «Параметры → Приложения → Автозагрузка».';

  @override
  String get settingsStartupMacApproval =>
      'Разрешите CopyPaste в разделе «Системные настройки → Основные → Элементы входа».';

  @override
  String get settingsStartupUnavailable =>
      'Автозапуск недоступен в этой установке.';

  @override
  String get settingsStartupAndroidDescription =>
      'Возобновлять фоновые службы после перезапуска.';

  @override
  String get settingsStartupDesktopDescription =>
      'Автоматически открывать CopyPaste при входе в систему.';

  @override
  String get settingsShowInDock => 'Показывать в Dock';

  @override
  String get settingsShowInTaskbar => 'Показывать на панели задач';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Показывать значок приложения для переключения между окнами.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'док панель задач значок приложения видимость dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Показывать в строке меню';

  @override
  String get settingsShowInSystemTray => 'Показывать в системном трее';

  @override
  String get settingsTrayVisibilityDescription =>
      'Показывать быстрый доступ к CopyPaste и управление записью.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'трей строка меню статус значок видимость tray menu bar menubar status icon visibility';

  @override
  String get settingsLightweightModeTitle => 'Лёгкий режим';

  @override
  String get settingsLightweightModeDescription =>
      'Закрытие окна завершает приложение; запись и синхронизация продолжаются. Трей, Quick Paste и сочетания клавиш вернутся, когда вы откроете его снова.';

  @override
  String get settingsLightweightModeKeywords =>
      'лёгкий закрыть выйти фон память lightweight close quit background memory';

  @override
  String get settingsVisibilityErrorTitle => 'Не удалось изменить видимость';

  @override
  String get settingsModulesSearchDescription =>
      'Устанавливайте, обновляйте и удаляйте дополнительные модули.';

  @override
  String get settingsModulesKeywords =>
      'функции команды расширения features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Фоновая запись на Android';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Полный или ограниченный режим с настройкой через Shizuku или ADB.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'фон разрешения настройка background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Полный или ограниченный режим и однократная настройка через Shizuku или ADB.';

  @override
  String get settingsOpenSetup => 'Настроить';

  @override
  String get settingsSaveScreenshotsTitle => 'Сохранять снимки экрана';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Автоматически сохранять новые снимки экрана Android в истории.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'изображения фото запись разрешения images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Автоматически добавлять новые снимки экрана в историю.';

  @override
  String get settingsScreenshotAccessTitle => 'Доступ к снимкам экрана';

  @override
  String get settingsScreenshotAccessDescription =>
      'Нужен для сохранения снимков экрана в фоне.';

  @override
  String get settingsAllowAccess => 'Разрешить доступ';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'Запись снимков экрана не запущена';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'Новые снимки экрана не сохраняются. Возобновите запись, чтобы снова запустить службу.';

  @override
  String get settingsResumeCapture => 'Возобновить запись';

  @override
  String get settingsClipboardCaptureTitle => 'Запись буфера обмена';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Приостановить или возобновить запись буфера обмена.';

  @override
  String get settingsCaptureStateUnavailable => 'Состояние недоступно';

  @override
  String get settingsCaptureStoppedTitle => 'Запись не запущена';

  @override
  String get settingsCaptureStoppedMessage =>
      'Сейчас CopyPaste не получает изменения буфера обмена на этом устройстве.';

  @override
  String get settingsRetentionTitle => 'Хранение';

  @override
  String get settingsRetentionDescription =>
      'Автоматически удалять старые незакреплённые элементы буфера обмена.';

  @override
  String get settingsRetentionForever => 'Хранить всегда';

  @override
  String get settingsStorageQuotaTitle => 'Квота хранилища';

  @override
  String get settingsStorageQuotaDescription =>
      'Максимальный объём локального хранилища для незакреплённой истории.';

  @override
  String get settingsStorageQuotaKeywords =>
      'диск место ограничение disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Максимальный размер файла';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Ограничение для добавления и загрузки файлов.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'импорт загрузка передача размер ограничение import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Наибольший файл, который можно добавить или загрузить.';

  @override
  String get settingsFileStorageTitle => 'Хранение файлов';

  @override
  String get settingsFileStorageDescription =>
      'Ссылка работает, только пока существует оригинал; копия сохраняет файл.';

  @override
  String get settingsFileStorageKeywords =>
      'файлы копия ссылка оригинал хранить офлайн files copy link original keep store offline';

  @override
  String get settingsFileStorageLink => 'Ссылка на оригинал';

  @override
  String get settingsFileStorageCopy => 'Хранить копию';

  @override
  String get settingsSaveFolderTitle => 'Сохранять файлы в';

  @override
  String get settingsSaveFolderAsk => 'Спрашивать каждый раз';

  @override
  String get settingsSaveFolderAskAction => 'Спрашивать каждый раз';

  @override
  String get settingsSaveFolderChooseAction => 'Выбрать папку';

  @override
  String get settingsSaveFolderSearchDescription =>
      'Папка для загруженных файлов.';

  @override
  String get settingsSaveFolderKeywords =>
      'загрузка папка назначение каталог место сохранения download folder destination directory save location';

  @override
  String get settingsAutoDownloadTitle => 'Загружать полученные файлы';

  @override
  String get settingsAutoDownloadDescription =>
      'Сохраняет файлы с других устройств в папку, пока устройства в сети.';

  @override
  String get settingsAutoDownloadNeedsFolder =>
      'Сначала выберите папку для сохранения файлов.';

  @override
  String get settingsAutoDownloadKeywords =>
      'автоматически загружать получать синхронизация файлы automatic download receive sync files';

  @override
  String get settingsHistoryFilesTitle => 'Файлы истории';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Экспортируйте историю, создавайте резервные копии или восстанавливайте из копии.';

  @override
  String get settingsHistoryFilesKeywords =>
      'текст зашифрованная резервная копия данные text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Экспорт текстовой истории';

  @override
  String get settingsExportTextDescription =>
      'Сохранить текстовые элементы в переносимый файл.';

  @override
  String get settingsExportAction => 'Экспортировать';

  @override
  String get settingsBackupTitle => 'Зашифрованная резервная копия';

  @override
  String get settingsBackupDescription =>
      'Сохранить всю локальную историю этого устройства.';

  @override
  String get settingsCreateBackupAction => 'Создать копию';

  @override
  String get settingsRestoreTitle => 'Восстановить из резервной копии';

  @override
  String get settingsRestoreDescription =>
      'Заменить локальную историю зашифрованной резервной копией.';

  @override
  String get settingsRestoreAction => 'Восстановить';

  @override
  String get settingsRestoreDialogTitle => 'Заменить локальную историю?';

  @override
  String get settingsRestoreDialogMessage =>
      'Резервная копия заменит историю на этом устройстве. Настройки сохранятся.';

  @override
  String get settingsRestoreFailed =>
      'Не удалось восстановить резервную копию.';

  @override
  String get settingsSyncSwitchTitle => 'Синхронизация';

  @override
  String get settingsSyncSwitchDescription =>
      'Синхронизировать историю с сопряжёнными устройствами.';

  @override
  String get settingsSyncSearchDescription =>
      'Синхронизация с сопряжёнными устройствами.';

  @override
  String get settingsInstantClipboardTitle => 'Мгновенный буфер обмена';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Автоматически копировать новые элементы с других устройств.';

  @override
  String get settingsInstantClipboardKeywords =>
      'автоматически копировать вставить синхронизация полученные automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Копировать новые элементы с других устройств в этот буфер обмена.';

  @override
  String get settingsLanVisibilityTitle => 'Видимость в локальной сети';

  @override
  String get settingsLanVisibilityDescription =>
      'Разрешить устройствам поблизости обнаруживать это устройство.';

  @override
  String get settingsLanVisibilityKeywords =>
      'локальная сеть обнаружение local network discovery';

  @override
  String get settingsRelayTitle => 'Сервер-ретранслятор';

  @override
  String get settingsRelayDescription =>
      'Синхронизируйте сопряжённые устройства из разных сетей через собственный ретранслятор. Оставьте поле пустым, чтобы отключить.';

  @override
  String get settingsRelayKeywords =>
      'ретранслятор сервер удалённый интернет сеть relay server remote internet network websocket';

  @override
  String get settingsRelayPlaceholder => 'wss://relay.example.com';

  @override
  String get settingsRelayConnecting => 'Подключение к ретранслятору…';

  @override
  String get settingsRelayConnected => 'Подключено к ретранслятору.';

  @override
  String get settingsRelayUnauthorized =>
      'Ретранслятор отклонил токен доступа.';

  @override
  String get settingsRelayUnreachable =>
      'Не удаётся подключиться к ретранслятору.';

  @override
  String get settingsPortMappingTitle => 'Проброс портов на роутере';

  @override
  String get settingsPortMappingDescription =>
      'Разрешить прямой синхронизации просить роутер (UPnP) открыть порт на несколько минут. Помогает, когда обе сети блокируют прямые подключения.';

  @override
  String get settingsPortMappingKeywords =>
      'роутер порт проброс прямое router port mapping upnp nat forwarding direct';

  @override
  String get settingsExcludedAppsTitle => 'Исключённые приложения';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Пропускать автоматическую запись из исключённых приложений.';

  @override
  String get settingsExcludedAppsKeywords =>
      'конфиденциальность приложение идентификаторы privacy app identifiers';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Не записывать автоматически из этих приложений.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Пока есть исключённые приложения, автоматическая запись приостановлена.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'Исключения приложений поддерживаются в macOS, Windows, Android и Linux.';

  @override
  String get settingsExcludedAppsAdd => 'Добавить';

  @override
  String get settingsExcludedAppsEmpty => 'Нет исключённых приложений.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return 'Удалить $app';
  }

  @override
  String get applicationPickerSearch => 'Поиск приложений';

  @override
  String get applicationPickerEmpty => 'Приложения не найдены';

  @override
  String get settingsSkipSecretTitle => 'Пропускать конфиденциальные данные';

  @override
  String get settingsSkipSecretDescription =>
      'Не сохранять копии, которые приложения помечают как секретные.';

  @override
  String get settingsSkipSecretKeywords =>
      'секрет пароль чувствительные скрытые спойлер secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Пропускать временные данные';

  @override
  String get settingsSkipTransientDescription =>
      'Не сохранять копии, которые приложения помечают как временные.';

  @override
  String get settingsSkipTransientKeywords =>
      'кратковременные временные конфиденциальность transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Блокировать снимки экрана';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Запретить снимки и запись экрана CopyPaste.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'Блокировка снимков экрана недоступна на этой платформе.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'конфиденциальность экран снимок защита сопряжение qr безопасность код privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Уведомление при копировании';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Показывать уведомление после фоновой записи.';

  @override
  String get settingsNotificationPreviewTitle =>
      'Показывать содержимое буфера обмена';

  @override
  String get settingsNotificationPreviewDescription =>
      'Добавлять предпросмотр элемента в уведомления о копировании.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'уведомление предпросмотр текст изображение конфиденциальность notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Звук при копировании';

  @override
  String get settingsSoundOnCopyDescription =>
      'Воспроизводить системный звук после успешной записи.';

  @override
  String get settingsVersionTitle => 'Версия';

  @override
  String get settingsVersionDescription => 'Установленная версия CopyPaste.';

  @override
  String get settingsVersionUnavailable => 'Недоступно';

  @override
  String get settingsUpdatesTitle => 'Обновления приложения';

  @override
  String get settingsUpdatesSearchDescription =>
      'Проверяйте и устанавливайте обновления CopyPaste.';

  @override
  String get settingsUpdatesKeywords =>
      'версия github выпуск version github release';

  @override
  String get settingsUpdateIdle =>
      'Проверьте, есть ли более новая версия CopyPaste.';

  @override
  String get settingsUpdateChecking => 'Проверка обновлений.';

  @override
  String get settingsUpdateUpToDate => 'У вас последняя версия CopyPaste.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'Версия $version — последняя.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'Доступна версия $available. На этом устройстве установлена версия $current.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Загрузка версии $version · $percent%.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Подготовка обновлений установленных модулей.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Установка версии $version.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'Обновления приложения требуют внимания.';

  @override
  String get settingsUpdateNowAction => 'Обновить сейчас';

  @override
  String get settingsUpdateOpenReleaseAction => 'Открыть выпуск';

  @override
  String get settingsUpdateCheckingAction => 'Проверка';

  @override
  String get settingsUpdateDownloadingAction => 'Загрузка';

  @override
  String get settingsUpdateInstallingAction => 'Установка';

  @override
  String get settingsUpdateRestartAction => 'Перезапустить CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => 'Проверить снова';

  @override
  String get settingsUpdateNoneTitle => 'Обновлений нет';

  @override
  String get settingsUpdateLatest => 'У вас последняя версия CopyPaste.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version — последняя версия.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Доступно обновление';

  @override
  String get settingsUpdateReady =>
      'Более новая версия CopyPaste готова к установке.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'Версия CopyPaste $version готова к установке.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Обновление недоступно';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Эта установка не обновляется автоматически.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Не удалось проверить обновления';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'Не удалось проверить обновления CopyPaste.';

  @override
  String get settingsQuickPasteSetupTitle => 'Настройте Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Включите подписанный компонент для GNOME или KDE и разрешите управление клавиатурой, чтобы возвращаться в предыдущее приложение.';

  @override
  String get settingsQuickPasteDesktopIntegration =>
      'Интеграция с рабочим окружением';

  @override
  String get settingsQuickPasteAllowKeyboard =>
      'Разрешить управление клавиатурой';

  @override
  String get settingsQuickPasteCheckAgain => 'Проверить снова';

  @override
  String get settingsQuickPasteLoading => 'Загрузка настроек Quick Paste.';

  @override
  String get settingsQuickPasteUnavailableTitle => 'Quick Paste недоступен';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'Это устройство не поддерживает окно Quick Paste.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Открыть Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Настройте глобальное сочетание клавиш для Quick Paste.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'клавиатура горячая клавиша сочетание keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'Работает, пока CopyPaste запущен в фоне.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Зарегистрированное сочетание: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Вставлять автоматически';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Вставлять выбранный элемент в предыдущее приложение.';

  @override
  String get settingsAutoPasteKeywords =>
      'универсальный доступ автоматически accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Выбор элемента возвращает в предыдущее приложение и вставляет его.';

  @override
  String get settingsAccessibilityRequiredTitle =>
      'Требуется «Универсальный доступ»';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Пока разрешение не предоставлено, выбранный элемент только копируется без вставки.';

  @override
  String get settingsQuickPasteAttentionTitle => 'Quick Paste требует внимания';

  @override
  String get settingsShortcutRecording => 'Нажмите сочетание…';

  @override
  String get settingsShortcutNotSet => 'Не задано';

  @override
  String get settingsShortcutReset => 'Сбросить сочетание';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count дня',
      many: '$count дней',
      few: '$count дня',
      one: '$count день',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Загрузка устройств…';

  @override
  String get devicesUnavailableTitle => 'Устройства недоступны';

  @override
  String get devicesNearbyTitle => 'Устройства поблизости';

  @override
  String get devicesScan => 'Сканировать';

  @override
  String get devicesScanning => 'Сканирование…';

  @override
  String get devicesNoneNearby => 'Поблизости нет устройств';

  @override
  String get devicesThisDevice => 'Это устройство';

  @override
  String get devicesTrusted => 'Доверенное';

  @override
  String get devicesConnect => 'Подключить';

  @override
  String devicesCardPing(String ping) {
    return 'Пинг: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Пинг:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'Переименовать это устройство';

  @override
  String get devicesNamePlaceholder => 'Название устройства';

  @override
  String get devicesRenameFailed => 'Не удалось сохранить название устройства.';

  @override
  String get devicesActionFailedMessage =>
      'Не удалось выполнить действие с устройством.';

  @override
  String get devicesCloseDetails => 'Закрыть сведения об устройстве';

  @override
  String get devicesDetailType => 'Тип устройства';

  @override
  String get devicesDetailOperatingSystem => 'Операционная система';

  @override
  String get devicesDetailModel => 'Модель';

  @override
  String get devicesDetailAppVersion => 'Версия CopyPaste';

  @override
  String get devicesDetailProtocolVersion => 'Версия протокола';

  @override
  String get devicesDetailStatus => 'Состояние';

  @override
  String get devicesDetailPing => 'Пинг';

  @override
  String get devicesDetailMeasured => 'Измерено';

  @override
  String get devicesDetailLastSeen => 'Последний раз в сети';

  @override
  String get devicesDetailLanEndpoint => 'Адрес в локальной сети';

  @override
  String get devicesDetailDeviceId => 'ID устройства';

  @override
  String get devicesDetailPairingId => 'ID сопряжения';

  @override
  String get devicesDetailProfileProvenance => 'Источник профиля';

  @override
  String get devicesDetailProfileTrust => 'Доверие к профилю';

  @override
  String get devicesDetailProfileUpdated => 'Профиль обновлён';

  @override
  String get devicesValueUnavailable => 'Недоступно';

  @override
  String get devicesRenameDevice => 'Переименовать устройство';

  @override
  String get devicesSyncNow => 'Синхронизировать сейчас';

  @override
  String get devicesSyncTitle => 'Синхронизация';

  @override
  String get devicesSyncDescription =>
      'Отключение не удаляет уже сохранённые элементы.';

  @override
  String get devicesSyncThisDevice => 'Синхронизировать это устройство';

  @override
  String get devicesSyncPeer => 'Синхронизировать с этим устройством';

  @override
  String get devicesSyncPause => 'Приостановить автосинхронизацию';

  @override
  String get devicesSyncText => 'Текст';

  @override
  String get devicesSyncImages => 'Изображения';

  @override
  String get devicesSyncFiles => 'Файлы';

  @override
  String get devicesSyncAfterFirstSync =>
      'Настройки синхронизации появятся после первой синхронизации с этим устройством.';

  @override
  String get devicesLocal => 'Локальный';

  @override
  String get devicesTrustAuthenticated => 'Аутентифицировано';

  @override
  String get devicesTrustUnverified => 'Не проверено';

  @override
  String get devicesProvenanceSelfReported => 'Сообщено устройством';

  @override
  String get devicesProvenanceObserved => 'Наблюдалось';

  @override
  String get devicesProvenanceMeasured => 'Измерено';

  @override
  String get devicesConnectionOnline => 'В сети';

  @override
  String get devicesConnectionOffline => 'Не в сети';

  @override
  String get devicesConnectionVisible => 'Видимое';

  @override
  String get devicesConnectionNotSeen => 'Не обнаружено';

  @override
  String get devicesConnectionUnknown => 'Неизвестно';

  @override
  String get devicesPeerAvailable => 'Доступно';

  @override
  String get devicesPeerVisibleNow => 'Видимо сейчас';

  @override
  String get devicesPeerStatusUnknown => 'Состояние неизвестно';

  @override
  String get devicesPingUnavailable => '— мс';

  @override
  String get devicesPingUnderOneMillisecond => '<1 мс';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds мс';
  }

  @override
  String get devicesClassDesktop => 'Настольный компьютер';

  @override
  String get devicesClassLaptop => 'Ноутбук';

  @override
  String get devicesClassPhone => 'Телефон';

  @override
  String get devicesClassTablet => 'Планшет';

  @override
  String get devicesClassUnknown => 'Устройство';

  @override
  String get devicesUnknownOs => 'Неизвестная ОС';

  @override
  String get devicesAddDevice => 'Добавить устройство';

  @override
  String get devicesScanQr => 'Сканировать QR-код';

  @override
  String get devicesEnterCode => 'Ввести код';

  @override
  String get devicesScannerUnavailableTitle => 'Сканер недоступен';

  @override
  String get devicesActionFailedTitle => 'Действие не выполнено';

  @override
  String get devicesRefreshFailedTitle =>
      'Не удалось обновить список устройств';

  @override
  String get devicesNearbyPairingUnavailable =>
      'Сопряжение с устройствами поблизости недоступно.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'Сопряжение с устройствами поблизости недоступно. Используйте «Добавить устройство», чтобы показать код сопряжения.';

  @override
  String get pairingTitle => 'Сопряжение';

  @override
  String get pairingPairDevice => 'Сопрячьте устройство';

  @override
  String get pairingScanQrCode => 'Сканировать QR-код';

  @override
  String get pairingEnterPairingCode => 'Ввести код сопряжения';

  @override
  String get pairingConnectNearbyDevice => 'Подключить устройство поблизости';

  @override
  String get pairingSendingRequest => 'Отправка запроса на подключение…';

  @override
  String get pairingEmptyTitle => 'Выберите способ сопряжения';

  @override
  String get pairingEmptyMessage =>
      'Здесь появятся элементы управления сопряжением.';

  @override
  String get pairingCreatingInvitation => 'Создание приглашения к сопряжению.';

  @override
  String get pairingCode => 'Код сопряжения';

  @override
  String get pairingCodeHint =>
      'Введите 8-символьный код, показанный на другом устройстве.';

  @override
  String get pairingDeviceAddress => 'Адрес устройства';

  @override
  String get pairingDeviceAddressHint =>
      'Используйте адрес host:port, который показывает другое устройство.';

  @override
  String get pairingCheckDetailsTitle => 'Проверьте данные сопряжения';

  @override
  String get pairingActionFailed => 'Не удалось выполнить действие сопряжения';

  @override
  String get pairingConnectingDevice => 'Подключение устройства…';

  @override
  String get pairingQrScannerTitle => 'Сканер QR-кодов';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'Здесь появится изображение с камеры.';

  @override
  String get pairingScannerNeedsAttention => 'Сканер требует внимания';

  @override
  String get pairingCodeIncomplete =>
      'Введите полный 8-символьный код сопряжения.';

  @override
  String get pairingAddressMissing =>
      'Введите адрес устройства в формате host:port или настройте сервер-ретранслятор в настройках.';

  @override
  String get pairingWaitingForOtherDevice => 'Ожидание другого устройства';

  @override
  String get pairingInviteInstructions =>
      'На другом устройстве выберите «Добавить устройство → Ввести код» или отсканируйте QR-код.';

  @override
  String get pairingDetailsTitle => 'Данные сопряжения';

  @override
  String get pairingAddress => 'Адрес';

  @override
  String get pairingVerificationCode => 'Код проверки';

  @override
  String get pairingVerificationCodeHint =>
      'Подтверждайте, только если этот код совпадает на обоих устройствах.';

  @override
  String get pairingReject => 'Отклонить';

  @override
  String get pairingConfirm => 'Подтвердить';

  @override
  String get pairingAccept => 'Принять';

  @override
  String get pairingJoinDevice => 'Присоединиться';

  @override
  String get pairingStartScanner => 'Запустить сканер';

  @override
  String get pairingActionNotStarted => 'Не удалось начать сопряжение.';

  @override
  String get pairingStateHandshaking => 'Проверка устройства';

  @override
  String get pairingStateAwaitingConfirmation =>
      'Подтвердите на обоих устройствах';

  @override
  String get pairingStateConfirmed => 'Устройство сопряжено';

  @override
  String get pairingStateRejected => 'Сопряжение отклонено';

  @override
  String get pairingStateCancelled => 'Сопряжение отменено';

  @override
  String get pairingStateTimedOut => 'Время сопряжения истекло';

  @override
  String get pairingStateFailed => 'Не удалось выполнить сопряжение';

  @override
  String get pairingHandshakingDescription =>
      'Устройства устанавливают защищённый канал.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Сравните защищённый код проверки, затем подтвердите на обоих устройствах.';

  @override
  String get pairingConfirmedDescription =>
      'Устройство подключено. Ваше членство в группе будет синхронизироваться автоматически.';

  @override
  String get pairingNoDeviceAdded => 'Устройство не добавлено.';

  @override
  String get pairingTimedOutDescription =>
      'Начните новую попытку сопряжения, чтобы продолжить.';

  @override
  String get pairingIdleDescription =>
      'Начните сопряжение, чтобы подключить устройство.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'Защищённое сопряжение недоступно';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Этот сеанс сопряжения должен открываться в отдельном защищённом окне.';

  @override
  String get pairingCancel => 'Отменить сопряжение';

  @override
  String get pairingOrEnterCode => 'Или введите код сопряжения';

  @override
  String get pairingJoinSecurely => 'Безопасно присоединиться';

  @override
  String get pairingProtectedWaitingDescription =>
      'Покажите QR-код, отсканируйте QR-код другого устройства или введите код сопряжения в этом защищённом окне.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Действует ещё $seconds с';
  }

  @override
  String get pairingInvalidQr => 'Отсканируйте QR-код сопряжения CopyPaste.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Сканер Google недоступен. Вместо него введите код сопряжения.';

  @override
  String get pairingScannerCouldNotOpen =>
      'Не удалось открыть сканер. Вместо него введите код сопряжения.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'Защищённое отображение сопряжения недоступно.';

  @override
  String get pairingVerificationCodeRequired =>
      'Сначала покажите и сравните код проверки.';

  @override
  String get syncStatusUnavailable => 'Состояние синхронизации недоступно';

  @override
  String get syncStatusOff => 'Синхронизация выключена';

  @override
  String get syncStatusWaitingForSync => 'Ожидание синхронизации';

  @override
  String get syncStatusSyncing => 'Синхронизация';

  @override
  String get syncStatusUpToDate => 'Синхронизировано';

  @override
  String get syncStatusInterrupted => 'Синхронизация прервана';

  @override
  String get syncStatusPartiallySynchronized => 'Частично синхронизировано';

  @override
  String get syncStatusRetrying => 'Повторная попытка';

  @override
  String get syncStatusWaitingToSync => 'Ожидает синхронизации';

  @override
  String get syncStatusWaitingForDevice => 'Ожидание устройства';

  @override
  String get syncStatusNoPairedDevices => 'Нет сопряжённых устройств';

  @override
  String get syncStatusDevicesOffline => 'Устройства не в сети';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · журнал активности недоступен';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Синхронизация выключена · $count проблемы',
      many: 'Синхронизация выключена · $count проблем',
      few: 'Синхронизация выключена · $count проблемы',
      one: 'Синхронизация выключена · $count проблема',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count не в сети',
      many: '$status · $count не в сети',
      few: '$status · $count не в сети',
      one: '$status · $count не в сети',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count устройства требуют внимания',
      many: '$count устройств требуют внимания',
      few: '$count устройства требуют внимания',
      one: '$count устройство требует внимания',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Устройства не в сети · $count проблемы',
      many: 'Устройства не в сети · $count проблем',
      few: 'Устройства не в сети · $count проблемы',
      one: 'Устройства не в сети · $count проблема',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Проверка подключения';

  @override
  String get syncConnectionUnknown => 'Состояние подключения неизвестно';

  @override
  String get syncLogDeviceOffline => 'Устройство отключилось от сети.';

  @override
  String get syncLogConnectionRestored => 'Подключение восстановлено.';

  @override
  String get syncLogProblemDetected => 'Обнаружена проблема синхронизации.';

  @override
  String get syncLogProblemResolved => 'Проблема синхронизации устранена.';

  @override
  String get syncLogFailed => 'Ошибка синхронизации.';

  @override
  String get syncLogRecovered => 'Синхронизация восстановлена.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent элемента',
      many: '$sent элементов',
      few: '$sent элемента',
      one: '$sent элемент',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received элемента',
      many: '$received элементов',
      few: '$received элемента',
      one: '$received элемент',
    );
    return 'Отправлено $_temp0 · Получено $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Синхронизация: $status';
  }

  @override
  String get syncDetailsTitle => 'Синхронизация';

  @override
  String get syncDetailsClose => 'Закрыть сведения о синхронизации';

  @override
  String get syncNoPairedDevicesMessage =>
      'Сопрягите устройство, чтобы синхронизировать историю буфера обмена.';

  @override
  String get syncActivityLogUnavailableTitle => 'Журнал активности недоступен';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Не удалось прочитать или сохранить данные об активности синхронизации.';

  @override
  String get syncDetailsDevicesHeading => 'Устройства';

  @override
  String get syncDetailsActivityHeading => 'Активность';

  @override
  String get syncDetailsNoRecentProblems =>
      'За последнее время проблем с синхронизацией не было';

  @override
  String get syncDetailsUnknownPeer => 'Сопряжённое устройство';

  @override
  String get syncPeerConnection => 'Подключение';

  @override
  String get syncPeerLastCheckStarted => 'Начало последней проверки';

  @override
  String get syncPeerItemsOverSizeLimit => 'Элементы сверх лимита размера';

  @override
  String syncPeerLastSeen(String time) {
    return 'Последний раз в сети $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Последняя синхронизация $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Последняя успешная синхронизация';

  @override
  String get syncPeerConnectionDetails => 'Сведения о подключении';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Сведения о подключении с $device';
  }

  @override
  String get historyKindText => 'Текст';

  @override
  String get historyKindLink => 'Ссылка';

  @override
  String get historyKindEmail => 'Эл. почта';

  @override
  String get historyKindColor => 'Цвет';

  @override
  String get historyKindPhone => 'Телефон';

  @override
  String get historyKindCode => 'Код';

  @override
  String get historyKindPath => 'Путь';

  @override
  String get historyKindCredentials => 'Учётные данные';

  @override
  String get historyKindImage => 'Изображение';

  @override
  String get historyKindFile => 'Файл';

  @override
  String get historyKindOther => 'Другое';

  @override
  String get historySortNewest => 'Сначала новые';

  @override
  String get historySortOldest => 'Сначала старые';

  @override
  String get historySortRelevance => 'По релевантности';

  @override
  String get historyErrorUpdatesUnavailable =>
      'Обновления истории временно недоступны.';

  @override
  String get historyErrorBulkDelete =>
      'Не удалось удалить некоторые выбранные элементы. Попробуйте ещё раз.';

  @override
  String get historyErrorBulkPin =>
      'Не удалось изменить закрепление некоторых выбранных элементов. Попробуйте ещё раз.';

  @override
  String get historyErrorRefresh =>
      'Не удалось обновить историю. Попробуйте ещё раз.';

  @override
  String get historyErrorLoad =>
      'Не удалось загрузить историю. Попробуйте ещё раз.';

  @override
  String get historyErrorLoadMore => 'Не удалось загрузить больше истории.';

  @override
  String get historyErrorLoadEarlier =>
      'Не удалось загрузить предыдущую историю.';

  @override
  String get historyErrorOpenClip => 'Не удалось открыть этот элемент.';

  @override
  String get historyErrorCopyPlainText =>
      'Не удалось скопировать обычный текст.';

  @override
  String get historyErrorCopyClip => 'Не удалось скопировать этот элемент.';

  @override
  String get historyErrorCopyText => 'Не удалось скопировать этот текст.';

  @override
  String get historyErrorPrepareFile =>
      'Не удалось загрузить этот файл. Проверьте устройство-источник и попробуйте ещё раз.';

  @override
  String get historyErrorDownloadFile => 'Не удалось загрузить этот файл.';

  @override
  String get historyErrorSelectFiles =>
      'Не удалось выбрать файлы. Попробуйте ещё раз.';

  @override
  String get historyErrorReadSelectedFile =>
      'Не удалось прочитать выбранный файл. Проверьте доступ и свободное место.';

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
          'Не удалось импортировать $failed из $total файла. Первый: $name. $reason',
      many:
          'Не удалось импортировать $failed из $total файлов. Первый: $name. $reason',
      few:
          'Не удалось импортировать $failed из $total файлов. Первый: $name. $reason',
      one:
          'Не удалось импортировать $failed из $total файла. Первый: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'Не удалось изменить конфиденциальность элемента.';

  @override
  String get historyErrorPinUpdate => 'Не удалось изменить закрепление.';

  @override
  String get historyErrorPinnedOrderSave =>
      'Не удалось сохранить порядок закреплённых элементов. Попробуйте ещё раз.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'Порядок сохранён, но не удалось обновить историю.';

  @override
  String get historyErrorDeleteClip => 'Не удалось удалить этот элемент.';

  @override
  String get historyErrorDeleteAll => 'Не удалось удалить историю.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'Не удалось удалить выбранные элементы.';

  @override
  String get historyOcrErrorUnavailable =>
      'Распознавание текста недоступно. Проверьте модуль в настройках и попробуйте ещё раз.';

  @override
  String get historyOcrErrorStart =>
      'Не удалось запустить распознавание текста. Попробуйте ещё раз.';

  @override
  String get historyOcrErrorRecognize =>
      'Не удалось распознать текст на изображении. Попробуйте ещё раз.';

  @override
  String get historyOcrRecognizing => 'Распознавание текста на изображении.';

  @override
  String get historyOcrFailedTitle => 'Ошибка распознавания текста';

  @override
  String get historyOcrNoText => 'Текст не найден';

  @override
  String get historyOcrTitle => 'Текст на изображении';

  @override
  String get historyOcrTextCopied => 'Распознанный текст скопирован.';

  @override
  String get historyOcrCopyError =>
      'Не удалось скопировать текст. Попробуйте ещё раз.';

  @override
  String get historyTagsErrorLoad =>
      'Не удалось загрузить теги. Попробуйте ещё раз.';

  @override
  String get historyTagsErrorSave =>
      'Не удалось сохранить изменение тега. Используйте уникальное название до 64 символов и выберите закреплённые элементы.';

  @override
  String get historyTags => 'Теги';

  @override
  String get historyTagsUpdateErrorTitle => 'Не удалось обновить теги';

  @override
  String get historyTagsEmpty => 'Тегов пока нет';

  @override
  String get historyTagRename => 'Переименовать';

  @override
  String get historyTagActions => 'Действия с тегом';

  @override
  String get historyTagNamePlaceholder => 'Название тега';

  @override
  String get historyCreateTag => 'Создать тег';

  @override
  String get historySaveTag => 'Сохранить тег';

  @override
  String get historyTagsFilterAll => 'Все теги';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count тега',
      many: '$count тегов',
      few: '$count тега',
      one: '$count тег',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Фильтр по тегам: $selection';
  }

  @override
  String get historyManageTags => 'Управлять тегами';

  @override
  String get historyTagsFilterSemantics =>
      'Фильтровать закреплённые элементы по тегам';

  @override
  String get historyDownloadErrorCancelled => 'Загрузка файла отменена.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'Служба загрузки недоступна. Откройте CopyPaste снова и попробуйте ещё раз.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'Не удалось подключиться к службе загрузки. Попробуйте ещё раз.';

  @override
  String get historyFileErrorAccess => 'Нет доступа к выбранному файлу.';

  @override
  String get historySpoilerRevealSemantics =>
      'Конфиденциальное содержимое. Раскрыть спойлер';

  @override
  String get historyActionFailedTitle => 'Действие не выполнено';

  @override
  String get historyLoadingMessage => 'Загрузка истории буфера обмена.';

  @override
  String get historyUnavailableTitle => 'История недоступна';

  @override
  String get historyEmptyTitle => 'Ничего не найдено';

  @override
  String get historyEmptyMessage =>
      'Здесь появятся записанные элементы и подходящие результаты из истории.';

  @override
  String get historyAttentionTitle => 'История требует внимания';

  @override
  String get historyLoadMore => 'Загрузить ещё';

  @override
  String get historySectionPinned => 'Закреплённые';

  @override
  String get historySectionToday => 'Сегодня';

  @override
  String get historySectionYesterday => 'Вчера';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Пропущено $count элемента буфера обмена',
      many: 'Пропущено $count элементов буфера обмена',
      few: 'Пропущено $count элемента буфера обмена',
      one: 'Пропущен $count элемент буфера обмена',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Некоторые сохранённые элементы не удалось прочитать, поэтому они не показаны в этом списке.';

  @override
  String get historySearch => 'Поиск в истории';

  @override
  String get historySearchSuggestions => 'Подсказки поиска';

  @override
  String get historySearchSuggestionRecent => 'Недавнее';

  @override
  String get historySearchSuggestionTag => 'Тег';

  @override
  String get historySearchSuggestionApp => 'Приложение';

  @override
  String get historySearchSuggestionKind => 'Тип';

  @override
  String get historySearchSuggestionDevice => 'Устройство';

  @override
  String get historySearchClearRecent => 'Очистить недавние поиски';

  @override
  String get historyResetFilters => 'Сбросить фильтры';

  @override
  String get historyImportFiles => 'Импортировать файлы';

  @override
  String get historySelectClips => 'Выбрать элементы';

  @override
  String get historyCloseSearch => 'Закрыть поиск';

  @override
  String get historyFilterAllClips => 'Все элементы';

  @override
  String get historyFilterAllDevices => 'Все устройства';

  @override
  String get historyFilterAllApps => 'Все приложения';

  @override
  String get historyPinClip => 'Закрепить элемент';

  @override
  String get historyUnpinClip => 'Открепить элемент';

  @override
  String get historyReorderPinnedClip =>
      'Изменить порядок закреплённого элемента';

  @override
  String get historyReorderPinnedHint =>
      'Перетащите, чтобы переместить. Alt + стрелка вверх или вниз перемещает на одну позицию.';

  @override
  String get historyClipUnavailable => 'Элемент недоступен';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Тип элемента: $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Выберите элемент';

  @override
  String get historyInspectorEmptyMessage =>
      'Здесь появятся его полное содержимое и действия.';

  @override
  String get historyNoPreview => 'Предпросмотр недоступен.';

  @override
  String get historyCloseClipDetails => 'Закрыть сведения об элементе';

  @override
  String get historyCopyOptions => 'Параметры копирования';

  @override
  String get historyCopyPlainText => 'Копировать как обычный текст';

  @override
  String get historyManageClipTags => 'Управлять тегами элемента';

  @override
  String get historyPinnedButton => 'Закреплено';

  @override
  String get historyAddSpoiler => 'Добавить спойлер';

  @override
  String get historyRemoveSpoiler => 'Убрать спойлер';

  @override
  String get historyUpdateFailedTitle => 'Не удалось обновить';

  @override
  String get historyRecognizeImageText => 'Распознать текст на изображении';

  @override
  String get historyCopiedTitle => 'Скопировано';

  @override
  String get historyCopyFailedTitle => 'Не удалось скопировать';

  @override
  String get historyPlainTextCopied => 'Обычный текст скопирован.';

  @override
  String get historyClipCopied => 'Элемент скопирован.';

  @override
  String get historyDownloadedTitle => 'Загружено';

  @override
  String get historyFileSaved => 'Файл сохранён.';

  @override
  String get historyShare => 'Поделиться';

  @override
  String get historyErrorShareClip => 'Не удалось поделиться этим элементом.';

  @override
  String get historyErrorShareClips =>
      'Не удалось поделиться выбранными элементами.';

  @override
  String get historyBulkShare => 'Поделиться выбранными';

  @override
  String get historyBulkShareFilesOnlyTitle => 'Переданы только файлы';

  @override
  String get historyBulkShareFilesOnly =>
      'Текстовые элементы пропущены, так как эта система не может делиться текстом вместе с файлами. Поделитесь ими отдельно.';

  @override
  String get historyDownload => 'Загрузить';

  @override
  String get historyDownloading => 'Загрузка…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Загрузка $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => 'Отмена…';

  @override
  String get historyDownloadCancellingTooltip => 'Отмена загрузки';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Загрузка $percent% · Нажмите, чтобы отменить';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Не удалось загрузить · Нажмите, чтобы повторить';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Отменить загрузку, выполнено $percent%';
  }

  @override
  String get historyRetryDownload => 'Повторить загрузку';

  @override
  String get historyDownloadFailedTitle => 'Не удалось загрузить';

  @override
  String get historyShowFile => 'Показать файл';

  @override
  String get historyMetadataPrivacy => 'Конфиденциальность';

  @override
  String get historyMetadataConfidential => 'Конфиденциально';

  @override
  String get historyMetadataCaptured => 'Записано';

  @override
  String get historyMetadataObservedApp => 'Приложение-источник';

  @override
  String get historyMetadataDevice => 'Устройство';

  @override
  String get historyMetadataClipType => 'Тип элемента';

  @override
  String get historyMetadataType => 'Тип';

  @override
  String get historyMetadataPath => 'Путь';

  @override
  String get historyMetadataFile => 'Файл';

  @override
  String get historyMetadataResolution => 'Разрешение';

  @override
  String get historyMetadataSize => 'Размер';

  @override
  String get historyMetadataFiles => 'Файлы';

  @override
  String get historyMetadataSync => 'Синхронизация';

  @override
  String get historyMetadataTooLargeToSync =>
      'Слишком большой для синхронизации';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Удалить $count выбранного элемента?',
      many: 'Удалить $count выбранных элементов?',
      few: 'Удалить $count выбранных элемента?',
      one: 'Удалить $count выбранный элемент?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Удалить этот элемент?';

  @override
  String get historyBulkTagTooltip =>
      'Добавить теги к выбранным закреплённым элементам';

  @override
  String get historyBulkExit => 'Выйти из режима выбора';

  @override
  String get historyBulkPin => 'Закрепить выбранные';

  @override
  String get historyBulkUnpin => 'Открепить выбранные';

  @override
  String get historyBulkDelete => 'Удалить выбранные';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Выбрано $count',
      many: 'Выбрано $count',
      few: 'Выбрано $count',
      one: 'Выбрано $count',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Фильтр по дате';

  @override
  String get historyDatePresetToday => 'Сегодня';

  @override
  String get historyDatePresetYesterday => 'Вчера';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Последние $days дня',
      many: 'Последние $days дней',
      few: 'Последние $days дня',
      one: 'Последний $days день',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'За всё время';

  @override
  String get appRuntimeStartFailed =>
      'Не удалось запустить среду выполнения CopyPaste.';

  @override
  String get appPreparing => 'Подготовка CopyPaste.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste ожидает доступа к защищённой истории. Доступные части приложения остаются рабочими.';

  @override
  String get appHistoryUnavailableTitle =>
      'Среда выполнения истории недоступна';

  @override
  String get appHistoryUnavailableMessage =>
      'Запустите среду выполнения приложения, прежде чем открывать историю.';

  @override
  String get appDevicesUnavailableTitle =>
      'Среда выполнения устройств недоступна';

  @override
  String get appDevicesUnavailableMessage =>
      'Запустите среду выполнения приложения, прежде чем открывать устройства.';

  @override
  String get appSettingsUnavailableTitle =>
      'Среда выполнения настроек недоступна';

  @override
  String get appSettingsUnavailableMessage =>
      'Запустите среду выполнения приложения, прежде чем открывать настройки.';

  @override
  String get appQuit => 'Выйти из CopyPaste';

  @override
  String get appWindowSetupTitle => 'Настройка окна требует внимания';

  @override
  String get appOpenHistory => 'Открыть историю';

  @override
  String get appDestinationHistory => 'История';

  @override
  String get appDestinationDevices => 'Устройства';

  @override
  String get appDestinationSettings => 'Настройки';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, выбрано';
  }

  @override
  String get appActiveFilter => 'Активный фильтр';

  @override
  String get appActionFailed => 'Действие не выполнено';

  @override
  String get sharedAndroidVersionRequired => 'Требуется Android 11 или новее';

  @override
  String get sharedUseAdbTab => 'Используйте вкладку ADB на этом устройстве.';

  @override
  String get sharedOneTimeAccessApplied => 'Одноразовый доступ предоставлен';

  @override
  String get sharedInstallShizuku => 'Установить Shizuku';

  @override
  String get sharedPairAndStartShizuku => 'Сопрягите и запустите Shizuku';

  @override
  String get sharedAllowCopyPaste => 'Разрешить CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded => 'Shizuku больше не понадобится.';

  @override
  String get sharedGetShizukuDescription =>
      'Загрузите Shizuku, чтобы предоставить одноразовый доступ.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Включите «Отладку по Wi-Fi» в Shizuku, затем вернитесь.';

  @override
  String get sharedApproveInShizuku =>
      'Один раз разрешите CopyPaste в Shizuku.';

  @override
  String get sharedGetShizuku => 'Загрузить Shizuku';

  @override
  String get sharedOpenShizuku => 'Открыть Shizuku';

  @override
  String get sharedRunOnComputer => 'Выполните на компьютере';

  @override
  String get sharedCopyAllCommands => 'Копировать все команды';

  @override
  String get sharedEnableUsbDebugging =>
      'Включите «Отладку по USB» и подключите этот телефон.';

  @override
  String get sharedAllow => 'Разрешить';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Не удалось скопировать команды настройки.';

  @override
  String get sharedShizukuOpenFailed => 'Не удалось открыть Shizuku.';

  @override
  String sharedSizeBytes(String size) {
    return '$size Б';
  }

  @override
  String get onboardingTitle => 'Настройка CopyPaste';

  @override
  String get onboardingWelcomeTitle => 'Добро пожаловать в CopyPaste';

  @override
  String get onboardingWelcomeSubtitle =>
      'Ваш буфер обмена всегда готов к работе.';

  @override
  String get onboardingReadyTitle => 'CopyPaste настроен';

  @override
  String get onboardingGetStarted => 'Начать';

  @override
  String get onboardingOptionalSetupTitle =>
      'Дополнительная настройка требует внимания';

  @override
  String get onboardingOpenSystemSettings => 'Открыть настройки';

  @override
  String get onboardingSetupErrorTitle => 'Настройка требует внимания';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'Не удалось прочитать состояние настроек $platform.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'Не удалось сохранить прогресс начальной настройки.';

  @override
  String get onboardingStartAtLogin => 'Запуск при входе';

  @override
  String get onboardingStartAtLoginDescription =>
      'Открывать CopyPaste при входе в систему';

  @override
  String get onboardingStartAtLoginSemantics => 'Запускать CopyPaste при входе';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Не удалось включить запуск при входе. Можно продолжить без него.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Не удалось изменить запуск при входе. Можно продолжить без него.';

  @override
  String get onboardingMacosTitle => 'Настройте Mac';

  @override
  String get onboardingAccessibility => 'Универсальный доступ';

  @override
  String get onboardingAutoPasteEnabled => 'Автовставка включена';

  @override
  String get onboardingAutoPasteOptional => 'Необязательно, для автовставки';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Доступно в установленном приложении CopyPaste.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'Требуется одобрение элемента входа';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Разрешите CopyPaste в «Системных настройках», затем вернитесь.';

  @override
  String get onboardingMacosStateReadFailed =>
      'Не удалось прочитать состояние настройки macOS.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Не удалось запросить разрешение «Универсальный доступ».';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'Не удалось обновить состояние настройки macOS.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Не удалось обновить состояние разрешения «Универсальный доступ».';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Не удалось открыть настройки «Элементы входа».';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'Запуск при входе требует одобрения в «Системных настройках».';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Не удалось отключить запуск при входе.';

  @override
  String get onboardingWindowsTitle => 'Настройте компьютер';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'Недоступно в этой установке. Можно продолжить без этого.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Отключено в Диспетчере задач';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Включите CopyPaste в списке приложений автозагрузки Windows, затем вернитесь.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'Запуск при входе отключён в Диспетчере задач. Его можно включить позже в «Параметрах Windows».';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'Запуск при входе недоступен. Можно продолжить без него.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'Не удалось прочитать состояние настройки Windows.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Не удалось открыть настройки автозагрузки Windows.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Не удалось изменить запуск при входе. Это можно сделать позже в настройках.';

  @override
  String get onboardingLinuxChecking =>
      'Проверка сеанса рабочего окружения Linux.';

  @override
  String get onboardingLinuxTitle => 'Настройте рабочее окружение Linux';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste требуется проверенная интеграция с рабочим окружением для определения источника элементов и работы Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Сеанс рабочего окружения';

  @override
  String get onboardingLinuxX11Detected => 'Обнаружен сеанс X11.';

  @override
  String get onboardingLinuxWaylandDetected => 'Обнаружен сеанс Wayland.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'Поддерживаемый сеанс X11 или Wayland не обнаружен.';

  @override
  String get onboardingLinuxCompanion => 'Компонент для GNOME или KDE';

  @override
  String get onboardingLinuxCompanionReady =>
      'Аутентифицированный мост Clipboard v2 готов.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'Запись буфера обмена приостановлена из-за исключений приложений-источников. Включите подписанный компонент, чтобы возобновить её.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Установите и включите подписанный пакет сеанса компонента, затем вернитесь сюда.';

  @override
  String get onboardingLinuxOpenSetup => 'Открыть настройку';

  @override
  String get onboardingLinuxKeyboardControl => 'Управление клавиатурой';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'Управление клавиатурой разрешено для Quick Paste.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'Управление клавиатурой недоступно в этом сеансе рабочего окружения.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Разрешить управление клавиатурой';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste готов к работе.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Quick Paste станет доступен после активации необходимой интеграции с рабочим окружением.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'Открывать CopyPaste при входе в систему.';

  @override
  String get onboardingLinuxLinks => 'Ссылки CopyPaste';

  @override
  String get onboardingLinuxLinksRegistered =>
      'Ссылки CopyPaste открываются в этом приложении.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Зарегистрируйте ссылки CopyPaste для приглашений к сопряжению.';

  @override
  String get onboardingLinuxRegisterLinks => 'Зарегистрировать ссылки';

  @override
  String get onboardingLinuxScreenshotBlocking => 'Блокировка снимков экрана';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Блокировка снимков экрана недоступна в Linux. Защищённые сохранённые данные остаются зашифрованными.';

  @override
  String get onboardingLinuxCheckAgain => 'Проверить снова';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'Не удалось прочитать состояние интеграции с Linux.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Не удалось открыть настройку интеграции с рабочим окружением.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'Разрешение на управление клавиатурой не предоставлено.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Не удалось изменить запуск при входе.';

  @override
  String get onboardingLinuxLinksFailed =>
      'Не удалось зарегистрировать ссылки CopyPaste.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Не удалось обновить состояние интеграции с Linux.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'В этом сеансе X11 нет доступа к буферу обмена.';

  @override
  String get onboardingLinuxSessionRequired =>
      'Для CopyPaste требуется сеанс рабочего окружения X11 или Wayland.';

  @override
  String get onboardingAndroidFullCapture => 'Полная запись';

  @override
  String get onboardingAndroidBackgroundCapture => 'Фоновая запись';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Настройте один раз, затем проверьте настоящим копированием.';

  @override
  String get onboardingAndroidLimitedCapture => 'Ограниченная запись';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'Поделитесь в CopyPaste или вернитесь после копирования.';

  @override
  String get onboardingAndroidNotification => 'Уведомление о записи';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Требуется во время записи';

  @override
  String get onboardingAndroidBackgroundActivity => 'Фоновая активность';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Рекомендуется, чтобы уменьшить потери при записи';

  @override
  String get onboardingAndroidOpen => 'Открыть';

  @override
  String get onboardingAndroidApplyAccess => 'Применить доступ к записи';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Предоставить одноразовый доступ к записи.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Полную запись можно настроить позже.';

  @override
  String get onboardingAndroidCaptureWorking => 'Фоновая запись работает';

  @override
  String get onboardingAndroidVerifyCopy => 'Проверьте настоящим копированием';

  @override
  String get onboardingAndroidCopyReached =>
      'Новая копия появилась в локальной истории.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Скопируйте что-нибудь в другом приложении, затем вернитесь сюда.';

  @override
  String get onboardingAndroidStartCapture => 'Начать запись';

  @override
  String get onboardingAndroidStateReadFailed =>
      'Не удалось прочитать состояние настройки Android.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Не удалось обновить состояние записи Android.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Отслеживание настройки Android недоступно.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'Не удалось включить приём буфера обмена.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Не удалось остановить фоновую запись.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Не удалось открыть настройки аккумулятора.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste разрешён в Shizuku, но не удалось применить разрешения Android для записи.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Разрешите CopyPaste в Shizuku, чтобы применить разрешения для записи.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Сначала предоставьте фоновый доступ к буферу обмена.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Разрешите уведомления, прежде чем начинать фоновую запись.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Не удалось начать фоновую запись.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Не удалось открыть настройку записи Android.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Не удалось сохранить выбор настройки Android.';

  @override
  String get modulesMarketplaceTab => 'Каталог';

  @override
  String get modulesInstalledTab => 'Установленные';

  @override
  String get modulesRefresh => 'Обновить';

  @override
  String get modulesInstallFromFile => 'Установить из файла';

  @override
  String get modulesPackageFileType => 'Модуль CopyPaste';

  @override
  String get modulesSearchPlaceholder => 'Поиск модулей';

  @override
  String get modulesOperationFailed =>
      'Не удалось выполнить операцию с модулем';

  @override
  String get modulesLoading => 'Загрузка модулей.';

  @override
  String get modulesUnavailableTitle => 'Модули недоступны';

  @override
  String get modulesUnavailableMessage =>
      'Попробуйте загрузить модули ещё раз.';

  @override
  String get modulesNoneInstalled => 'Нет установленных модулей';

  @override
  String get modulesNoMatches => 'Нет подходящих модулей';

  @override
  String get modulesChooseFromMarketplace => 'Выберите модуль в каталоге.';

  @override
  String get modulesTryDifferentSearch => 'Попробуйте другой запрос.';

  @override
  String get modulesMarketplaceLoading => 'Загрузка каталога.';

  @override
  String get modulesMarketplaceUnavailableTitle => 'Каталог недоступен';

  @override
  String get modulesMarketplaceUnavailableMessage =>
      'Попробуйте загрузить каталог ещё раз.';

  @override
  String get modulesNoneAvailable => 'Нет доступных модулей';

  @override
  String get modulesNoneAvailableMessage =>
      'Модули появятся здесь после публикации.';

  @override
  String modulesEnableModule(String module) {
    return 'Включить $module';
  }

  @override
  String get modulesInstall => 'Установить';

  @override
  String modulesUpdateTo(String version) {
    return 'Обновить до $version';
  }

  @override
  String get modulesRemoveModule => 'Удалить модуль';

  @override
  String get modulesModuleSettings => 'Настройки модуля';

  @override
  String modulesSettingsTitle(String module) {
    return 'Настройки модуля $module';
  }

  @override
  String get modulesDownloadingModel => 'Загрузка языковой модели…';

  @override
  String get modulesRun => 'Выполнить';

  @override
  String modulesRemoveTitle(String module) {
    return 'Удалить $module?';
  }

  @override
  String get modulesRemoveMessage =>
      'Модуль и его локальные настройки будут удалены.';

  @override
  String get modulesRemove => 'Удалить';

  @override
  String get modulesRemoveFailed => 'Не удалось удалить модуль.';

  @override
  String get modulesCloseSettings => 'Закрыть настройки модуля';

  @override
  String get modulesRemoved => 'Модуль удалён';

  @override
  String get modulesNoSettingsTitle => 'Нет дополнительных настроек';

  @override
  String get modulesNoSettingsMessage =>
      'Этому модулю не требуется дополнительная настройка.';

  @override
  String get modulesSetUpSmsAccess => 'Настроить доступ к SMS';

  @override
  String get modulesRestartApp => 'Перезапустить CopyPaste';

  @override
  String get modulesSmsAccessTitle => 'Доступ к SMS';

  @override
  String get modulesCloseSmsAccess => 'Закрыть панель доступа к SMS';

  @override
  String get modulesSmsChecking => 'Проверка доступа к SMS.';

  @override
  String get modulesSmsReady =>
      'Доступ готов. Включите SMS Codes, чтобы начать копировать новые коды.';

  @override
  String get modulesSmsAllowAccess =>
      'Разрешите доступ, чтобы копировать коды из новых SMS.';

  @override
  String get modulesSmsNotification => 'Уведомление SMS';

  @override
  String get modulesSmsNotificationDescription =>
      'Требуется во время работы SMS Codes';

  @override
  String get modulesSmsApp => 'CopyPaste SMS';

  @override
  String get modulesSmsAppDescription =>
      'Отдельное приложение, которое единственное читает SMS';

  @override
  String get modulesSmsApplyAccess => 'Применить доступ к SMS';

  @override
  String get modulesSmsApplyAccessDescription =>
      'Предоставить одноразовый доступ к SMS.';

  @override
  String get modulesSmsSetupFailed => 'Не удалось настроить SMS';

  @override
  String get modulesFileSelectionFailed => 'Не удалось выбрать файл';

  @override
  String get modulesChooseLanguages => 'Выберите языки';

  @override
  String get modulesChooseFile => 'Выбрать файл';

  @override
  String modulesModelReady(String model) {
    return 'Модель $model готова';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return 'Модель $model · загрузка $size МиБ';
  }

  @override
  String get modulesPlatformsUnknown => 'Платформы: неизвестно';

  @override
  String modulesPlatforms(String platforms) {
    return 'Платформы: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'Подпись каталога модулей недействительна.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Не удалось подключиться к каталогу модулей.';

  @override
  String get modulesMarketplaceTimeout => 'Каталог модулей не ответил.';

  @override
  String get modulesIncompatible =>
      'Этот модуль несовместим с этим устройством.';

  @override
  String get modulesPackageSizeInvalid =>
      'Недействительный размер пакета модуля.';

  @override
  String get modulesDownloadInterrupted => 'Загрузка модуля прервана.';

  @override
  String get modulesDownloadTimeout =>
      'Время ожидания загрузки модуля истекло.';

  @override
  String get modulesPackageUrlUntrusted =>
      'Адрес пакета модуля не является доверенным.';

  @override
  String get modulesDownloadLeftGithub =>
      'Загрузка модуля перенаправлена за пределы GitHub.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'При загрузке модуля произошло слишком много перенаправлений.';

  @override
  String get modulesDownloadTooLarge => 'Загрузка модуля слишком большая.';

  @override
  String get modulesPackageIntegrityFailed =>
      'Пакет модуля не прошёл проверку целостности.';

  @override
  String get modulesDownloadUnavailable =>
      'Загрузка этого модуля сейчас недоступна.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'Каталог модулей недоступен. Попробуйте ещё раз.';

  @override
  String get modulesCatalogInvalid => 'Каталог модулей недействителен.';

  @override
  String get modulesThisSystem => 'эта система';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version или новее';
  }

  @override
  String get modulesNotAvailableForDevice => 'Недоступно для этого устройства.';

  @override
  String get modulesAppVersionUnverified =>
      'Не удалось проверить установленную версию CopyPaste.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Этот модуль не поддерживает CopyPaste $version.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Требуется: $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed =>
      'Не удалось загрузить каталог модулей.';

  @override
  String get modulesSmsSetupRequired =>
      'Настройте доступ к SMS, прежде чем включать этот модуль.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'Не удалось запустить отслеживание SMS. Модуль всё ещё включён; отключите его снова, чтобы повторить попытку.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'Не удалось запустить отслеживание SMS. Модуль отключён.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'Не удалось запустить отслеживание SMS. Не удалось подтвердить, включён ли модуль.';

  @override
  String get modulesSmsMonitoringFailed =>
      'Не удалось запустить отслеживание SMS.';

  @override
  String get modulesSmsAndroidOnly =>
      'Доступ к SMS возможен только на Android.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'Отслеживание SMS требует настройки.';

  @override
  String get modulesSmsMonitoringUnavailable =>
      'Не удалось проверить отслеживание SMS.';

  @override
  String get modulesOperationIncomplete =>
      'Не удалось завершить операцию с модулем.';

  @override
  String get modulesUnsupportedEvent =>
      'Модуль использует неподдерживаемое событие.';

  @override
  String get modulesUnsupportedResult =>
      'Модуль вернул неподдерживаемый результат.';

  @override
  String get modulesUnsupportedField =>
      'Модуль использует неподдерживаемое поле.';

  @override
  String get modulesResponseInvalid => 'Недействительный ответ модуля.';

  @override
  String get modulesFileSelectionUnavailable => 'Выбор файлов недоступен.';

  @override
  String get modulesFileSelectFailed => 'Не удалось выбрать файл.';

  @override
  String modulesFileNotAccepted(String field) {
    return 'Файл в поле «$field» не подходит.';
  }

  @override
  String get modulesSmsVerifyFailed => 'Не удалось проверить доступ к SMS.';

  @override
  String get modulesSmsAllowNotifications =>
      'Разрешите уведомления, чтобы модуль SMS Codes работал в фоновом режиме.';

  @override
  String get modulesSmsNotGranted => 'Доступ к SMS не предоставлен.';

  @override
  String get modulesSmsConfigureFailed => 'Не удалось настроить доступ к SMS.';

  @override
  String get onboardingLinuxContinueBasic => 'Продолжить с базовой записью';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Необязательно. Требуется для Quick Paste и исключения приложений-источников.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Необязательно. Требуется, чтобы Quick Paste возвращал в предыдущее приложение.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'В этом сеансе Wayland нет доступа к буферу обмена. Включите подписанный компонент для GNOME или KDE, затем проверьте снова.';

  @override
  String get historyMoreActions => 'Другие действия';

  @override
  String get historyTagsAction => 'Теги…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'разблокировать секретные элементы',
      'other': 'Разблокировать секретные элементы',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'скопировать секретный элемент',
      'other': 'Скопировать секретный элемент',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'Недоступно в этой системе';

  @override
  String get deviceAuthNotConfigured =>
      'Сначала настройте пароль, PIN-код или биометрию устройства';

  @override
  String get commonOn => 'Вкл.';

  @override
  String get commonOff => 'Выкл.';

  @override
  String get settingsQuickPasteStartFailed =>
      'Не удалось запустить Quick Paste, поэтому его сочетание клавиш не работает.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut использует другое приложение. Выберите другое сочетание.';
  }

  @override
  String get devicesEmptyTitle => 'Другие устройства пока не сопряжены';

  @override
  String get devicesEmptyMessage =>
      'Сопрягите устройство в той же сети, чтобы синхронизировать историю.';

  @override
  String get pairingStateNotCompleted => 'Сопряжение не завершено';

  @override
  String get pairingStepInvite => 'Приглашение';

  @override
  String get pairingStepScan => 'Сканирование';

  @override
  String get pairingStepCode => 'Код';

  @override
  String get pairingStepRequest => 'Запрос';

  @override
  String get pairingStepConnect => 'Подключение';

  @override
  String get pairingStepConfirm => 'Подтверждение';

  @override
  String get pairingStepPaired => 'Сопряжено';

  @override
  String get pairingOtherDevice => 'Другое устройство';

  @override
  String get pairingDecisionNotConfirmed => 'Ещё не подтверждено';

  @override
  String get pairingDecisionSending => 'Отправка решения…';

  @override
  String get pairingDecisionConfirmed => 'Подтверждено';

  @override
  String get pairingDecisionRejected => 'Отклонено';

  @override
  String get pairingDecisionWaiting => 'Ожидание подтверждения';

  @override
  String get pairingDecisionUnknown => 'Ответ неизвестен';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count устройства',
      many: '$count устройств',
      few: '$count устройства',
      one: '$count устройство',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count строки',
      many: '$count строк',
      few: '$count строки',
      one: '$count строка',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Главная навигация';

  @override
  String sharedStepCounter(int step, int count) {
    return 'Шаг $step из $count';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Шаг $step из $count: $name';
  }

  @override
  String get modulesPhaseDownload => 'Загрузка';

  @override
  String get modulesPhaseInstall => 'Установка';

  @override
  String modulesInstallingModule(String module) {
    return 'Установка модуля $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return 'Обновление модуля $module';
  }

  @override
  String get commonDismiss => 'Закрыть';

  @override
  String get settingsRetentionConfirmTitle => 'Удалить более старую историю?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Новый лимит: $limit. Более старые незакреплённые элементы будут удалены безвозвратно.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle => 'Уменьшить квоту хранилища?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Новая квота: $size. Незакреплённые элементы сверх неё будут удалены безвозвратно.';
  }

  @override
  String get settingsLimitConfirmAction => 'Применить лимит';

  @override
  String get settingsHideEverywhereTitle => 'Полностью скрыть CopyPaste?';

  @override
  String get settingsHideEverywhereMessage =>
      'Если выключить оба параметра, значка не будет. Чтобы открыть CopyPaste, запустите его снова.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Если выключить оба параметра, значка не будет. Чтобы открыть CopyPaste, нажмите $shortcut или запустите его снова.';
  }

  @override
  String get settingsHideEverywhereAction => 'Скрыть';

  @override
  String get settingsExportDoneTitle => 'Текстовая история экспортирована';

  @override
  String get settingsBackupDoneTitle => 'Резервная копия создана';

  @override
  String get settingsRestoreDoneTitle => 'История восстановлена';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Экспортировано $count текстового элемента',
      many: 'Экспортировано $count текстовых элементов',
      few: 'Экспортировано $count текстовых элемента',
      one: 'Экспортирован $count текстовый элемент',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count не удалось прочитать',
      many: '$count не удалось прочитать',
      few: '$count не удалось прочитать',
      one: '$count не удалось прочитать',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Зашифрованная резервная копия сохранена · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'История буфера обмена на этом устройстве теперь соответствует резервной копии.';

  @override
  String sharedSizeKibibytes(String size) {
    return '$size КиБ';
  }

  @override
  String sharedSizeMebibytes(String size) {
    return '$size МиБ';
  }

  @override
  String sharedSizeGibibytes(String size) {
    return '$size ГиБ';
  }

  @override
  String storageUsageShort(String used, String quota) {
    return '$used из $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return 'Использовано $used из $quota';
  }

  @override
  String storageUsageNoQuota(String used) {
    return 'Использовано $used';
  }

  @override
  String get storageUsageOverQuota => 'Квота превышена';

  @override
  String storageUsageOldestClip(String date) {
    return 'Самый старый элемент: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Хранилище: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Новая квота: $size. Занято $used, поэтому самые старые незакреплённые элементы будут удалены безвозвратно.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count элемента',
      many: '$count элементов',
      few: '$count элемента',
      one: '$count элемент',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'Открыть CopyPaste';

  @override
  String get trayOpenQuickPaste => 'Открыть Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Открыть Quick Paste ($shortcut)';
  }

  @override
  String get trayRecent => 'Недавние';

  @override
  String get trayRecentLoading => 'Загрузка…';

  @override
  String get trayRecentFailed => 'Не удалось загрузить элементы';

  @override
  String get trayRecentEmpty => 'Элементов пока нет';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count недавнего элемента скрыто',
      many: '$count недавних элементов скрыто',
      few: '$count недавних элемента скрыты',
      one: '$count недавний элемент скрыт',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Секрет';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Копировать последний элемент с другого устройства';

  @override
  String get trayCaptureNow => 'Записать сейчас';

  @override
  String trayPausedUntil(String time) {
    return 'Приостановлено до $time';
  }

  @override
  String get trayPauseFor => 'Приостановить на…';

  @override
  String get trayPauseFor15Minutes => '15 минут';

  @override
  String get trayPauseForOneHour => '1 час';

  @override
  String get trayPauseUntilResumed => 'Пока я не возобновлю';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count элемента',
      many: 'Paste Queue: $count элементов',
      few: 'Paste Queue: $count элемента',
      one: 'Paste Queue: $count элемент',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Сбор: $count элемента',
      many: 'Сбор: $count элементов',
      few: 'Сбор: $count элемента',
      one: 'Сбор: $count элемент',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Недавно пропущенные ($count)';
  }

  @override
  String get trayOpenContentRules => 'Открыть правила содержимого';

  @override
  String get trayRecentSettingsTitle => 'Строка меню и трей';

  @override
  String get trayRecentSettingsDescription =>
      'Что меню в строке меню или трее показывает в разделе «Недавние».';

  @override
  String get trayRecentCountTitle => 'Недавние элементы';

  @override
  String get trayRecentCountDescription =>
      'Сколько последних элементов показывает меню.';

  @override
  String get trayRecentPreviewTitle => 'Предпросмотр недавних элементов';

  @override
  String get trayRecentPreviewDescription =>
      'Сколько текста элементов показывает меню.';

  @override
  String get trayRecentPreviewFull => 'Полный предпросмотр';

  @override
  String get trayRecentPreviewTitles => 'Только тип и источник';

  @override
  String get trayRecentPreviewHidden => 'Скрыто';

  @override
  String get trayRecentSettingsKeywords =>
      'трей строка меню недавние предпросмотр конфиденциальность количество';

  @override
  String get transformsPaletteTitle => 'Преобразовать текст';

  @override
  String get transformsFilterPlaceholder => 'Фильтровать преобразования';

  @override
  String get transformsNoMatches => 'Нет подходящих преобразований';

  @override
  String get transformsGroupResults => 'Результаты';

  @override
  String get transformsGroupSuggested => 'Рекомендуемые';

  @override
  String get transformsGroupRecent => 'Недавние';

  @override
  String get transformsGroupFavorites => 'Избранное';

  @override
  String get transformsGroupPipelines => 'Цепочки';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV и XML';

  @override
  String get transformsCategoryEncode => 'Кодирование и декодирование';

  @override
  String get transformsCategoryCase => 'Регистр';

  @override
  String get transformsCategoryHash => 'Хеши';

  @override
  String get transformsCategoryLines => 'Строки и пробелы';

  @override
  String get transformsCategoryClean => 'Очистка';

  @override
  String get transformsCategoryUrl => 'Ссылки';

  @override
  String get transformsCategoryMarkup => 'Форматированный текст и Markdown';

  @override
  String get transformsCategoryDev => 'Для разработчиков';

  @override
  String get transformsCategoryOther => 'Другое';

  @override
  String get transformsChainEmpty => 'Шагов пока нет';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Затем: $name';
  }

  @override
  String get transformsStepEnabled => 'Выполнять этот шаг';

  @override
  String get transformsMoveEarlier => 'Переместить выше';

  @override
  String get transformsMoveLater => 'Переместить ниже';

  @override
  String get transformsRemoveStep => 'Удалить шаг';

  @override
  String get transformsSavePipeline => 'Сохранить как цепочку';

  @override
  String get transformsSavePipelineTitle => 'Сохранить цепочку';

  @override
  String get transformsSavePipelineFailed => 'Не удалось сохранить цепочку.';

  @override
  String get transformsPipelineNameLabel => 'Название';

  @override
  String get transformsPipelineDescriptionLabel => 'Описание (необязательно)';

  @override
  String get transformsPipelineStepsLabel => 'Шаги';

  @override
  String get transformsPipelineNoSteps => 'Добавьте хотя бы один шаг.';

  @override
  String get transformsPipelineNameRequired => 'Введите название.';

  @override
  String get transformsPipelineNameTooLong => 'Не более 80 символов.';

  @override
  String get transformsPipelineNameTaken =>
      'Другая цепочка уже имеет это название.';

  @override
  String get transformsPipelineSavedTitle => 'Цепочка сохранена';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name доступна во всех палитрах преобразований.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Шагов: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Удалённая цепочка';

  @override
  String get transformsNeedsNewerVersion =>
      'Требуется более новая версия CopyPaste';

  @override
  String get transformsLoadFailed => 'Не удалось загрузить преобразования.';

  @override
  String get transformsNotTextTitle => 'Преобразования работают с текстом';

  @override
  String get transformsNotTextMessage =>
      'Этот элемент — изображение, файл или цвет. Выберите текстовый элемент или скопируйте текст.';

  @override
  String get transformsNothingTitle => 'Нечего преобразовывать';

  @override
  String get transformsNothingMessage =>
      'Скопируйте текст или выберите элемент из истории.';

  @override
  String get transformsPickTransform => 'Выберите преобразование';

  @override
  String get transformsPickTransformHint =>
      'Tab добавляет его как шаг. Enter применяет результат.';

  @override
  String get transformsPreviewing => 'Обновление предпросмотра…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Строк: $lines · Слов: $words · Символов: $chars';
  }

  @override
  String get transformsNoChanges => 'Без изменений';

  @override
  String transformsTruncated(String size) {
    return 'Показаны первые 64 КиБ из $size';
  }

  @override
  String get transformsRandom => 'Случайно';

  @override
  String get transformsNotRun => 'Не выполнено';

  @override
  String get transformsSensitiveHidden => 'Результат конфиденциален и скрыт.';

  @override
  String get transformsReveal => 'Показать';

  @override
  String get transformsPaneOutput => 'Результат';

  @override
  String get transformsPaneInput => 'Входные данные';

  @override
  String get transformsPaneChanges => 'Изменения';

  @override
  String get transformsCannotTransform => 'Не удаётся преобразовать этот текст';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Шаг $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Показать в hex';

  @override
  String get transformsActionPaste => 'Вставить';

  @override
  String get transformsActionSave => 'Сохранить как новый элемент';

  @override
  String get transformsCopiedMessage =>
      'Результат в буфере обмена и в истории.';

  @override
  String get transformsSavedTitle => 'Сохранено';

  @override
  String get transformsSavedMessage => 'Сохранено как новый элемент истории.';

  @override
  String get transformsPasteFailedTitle => 'Не удалось вставить';

  @override
  String get transformsPasteFailedMessage =>
      'Результат в буфере обмена. Вставьте его вручную.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message в строке $line, столбце $column';
  }

  @override
  String get transformsErrorBinary =>
      'Декодированные данные двоичные, а не текст';

  @override
  String get transformsErrorTooLarge => 'Слишком большой для преобразования';

  @override
  String get transformsErrorInvalidParam =>
      'Настройка этого шага недействительна';

  @override
  String get transformsErrorCancelled => 'Отменено';

  @override
  String get transformsErrorTooExpensive => 'Шаблон слишком сложный';

  @override
  String get transformsErrorPattern => 'Шаблон недействителен';

  @override
  String get transformsErrorTimestampUnit =>
      'Выберите, число указано в секундах или миллисекундах';

  @override
  String get transformsErrorExternalEntity => 'Внешние сущности запрещены';

  @override
  String get transformsErrorJwtEncrypted =>
      'Зашифрованные токены нельзя декодировать';

  @override
  String get transformsErrorJson => 'Недействительный JSON';

  @override
  String get transformsErrorYaml => 'Недействительный YAML';

  @override
  String get transformsErrorXml => 'Недействительный XML';

  @override
  String get transformsErrorCsv => 'Недействительный CSV';

  @override
  String get transformsErrorBase64 => 'Недействительный Base64';

  @override
  String get transformsErrorHex => 'Недействительный hex';

  @override
  String get transformsErrorEscape =>
      'Недействительная escape-последовательность';

  @override
  String get transformsErrorUrl => 'Неправильное процентное кодирование';

  @override
  String get transformsErrorJwt => 'Недействительный JWT';

  @override
  String get transformsErrorDate => 'Нераспознанная дата или отметка времени';

  @override
  String get transformsErrorGeneric =>
      'Этот шаг не может преобразовать входные данные';

  @override
  String get transformsWarningSignedLink =>
      'Подписанная ссылка оставлена без изменений';

  @override
  String get transformsWarningExpired => 'Срок действия истёк';

  @override
  String get transformsWarningNotYetValid => 'Ещё недействителен';

  @override
  String get transformsWarningSignatureNotVerified => 'Подпись не проверена';

  @override
  String get transformsWarningDuplicateKeys => 'Дублирующиеся ключи';

  @override
  String get transformsWarningLenientJson => 'Принят нестандартный JSON';

  @override
  String get transformsWarningDoubleEncoded => 'JSON был закодирован дважды';

  @override
  String get transformsWarningEscapes =>
      'Неизвестные escape-последовательности оставлены как есть';

  @override
  String get transformsWarningLocalZone => 'Предположен локальный часовой пояс';

  @override
  String get transformsWarningNoMatches => 'Совпадений нет';

  @override
  String get transformsWarningGeneric => 'Проверьте результат';

  @override
  String get transformsRestoreTitle =>
      'Восстанавливать буфер обмена после вставки';

  @override
  String get transformsRestoreDescription =>
      'Возвращать предыдущий буфер обмена после вставки.';

  @override
  String get transformsDefaultTitle =>
      'Преобразование при вставке по умолчанию';

  @override
  String get transformsDefaultDescription =>
      'Применяется к тексту, вставляемому из Quick Paste.';

  @override
  String get transformsNoDefault => 'Нет';

  @override
  String get transformsAppDefaultsTitle =>
      'Значения по умолчанию для приложений';

  @override
  String get transformsAppDefaultsDescription =>
      'Использовать другое значение по умолчанию в отдельных приложениях.';

  @override
  String transformsOtherPlatform(String platform) {
    return 'только на $platform';
  }

  @override
  String get transformsAddApp => 'Добавить приложение';

  @override
  String get transformsRemove => 'Удалить';

  @override
  String get transformsFavoritesTitle => 'Избранное';

  @override
  String get transformsFavoritesDescription =>
      'Показывается первым в каждой палитре.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Mod+$number в палитре';
  }

  @override
  String get transformsAddFavorite => 'Добавить в избранное';

  @override
  String get transformsPipelinesTitle => 'Цепочки';

  @override
  String get transformsPipelinesDescription =>
      'Сохранённые цепочки преобразований.';

  @override
  String get transformsNewPipeline => 'Новая цепочка';

  @override
  String get transformsEditPipeline => 'Редактировать цепочку';

  @override
  String get transformsDuplicatePipeline => 'Дублировать';

  @override
  String get transformsDeletePipeline => 'Удалить';

  @override
  String transformsDeletePipelineTitle(String name) {
    return 'Удалить $name?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Избранное и значения по умолчанию для вставки, использующие её, будут очищены.';

  @override
  String get transformsDeletePipelineFailed => 'Не удалось удалить цепочку.';

  @override
  String get transformsAddStep => 'Добавить шаг';

  @override
  String get transformsHiddenTitle => 'Скрытые преобразования';

  @override
  String get transformsHiddenDescription =>
      'Не показываются в палитрах и рекомендациях.';

  @override
  String get transformsHideTransform => 'Скрыть преобразование';

  @override
  String get transformsShow => 'Показать';

  @override
  String get transformsSuggestionsTitle => 'Рекомендации';

  @override
  String get transformsSuggestionsDescription =>
      'Предлагать преобразования, подходящие к тексту.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web Tokens';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl =>
      'Ссылки с параметрами отслеживания';

  @override
  String get transformsDetectorTimestamp => 'Отметки времени';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Параметры отслеживания';

  @override
  String get transformsTrackingDescription =>
      'Параметры ссылок, которые также удалять или всегда сохранять.';

  @override
  String get transformsTrackingRemoveLabel => 'Также удалять';

  @override
  String get transformsTrackingAllowLabel => 'Всегда сохранять';

  @override
  String get transformsTrackingPlaceholder => 'имя или имя@домен, затем Enter';

  @override
  String get quickAccessUnavailableTitle =>
      'Состояние быстрого доступа недоступно';

  @override
  String get quickAccessUnavailableMessage =>
      'Не удалось прочитать состояние клавиатуры, плиток и виджета. Попробуйте ещё раз.';

  @override
  String get quickAccessActionFailedTitle => 'Не удалось выполнить действие';

  @override
  String get quickAccessActionFailedMessage =>
      'Android не открыл нужный экран или не сохранил изменение. Попробуйте ещё раз.';

  @override
  String get quickAccessKeyboardTitle => 'Клавиатура элементов';

  @override
  String get quickAccessKeyboardDescription =>
      'Включите «Элементы CopyPaste» в настройках клавиатуры.';

  @override
  String get quickAccessKeyboardOn => 'Включено';

  @override
  String get quickAccessKeyboardOff => 'Выключено';

  @override
  String get quickAccessOpenKeyboardSettings => 'Настройки клавиатуры';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'Включить панель клавиатуры «Элементы CopyPaste».';

  @override
  String get quickAccessKeyboardKeywords =>
      'клавиатура метод ввода ime панель включить';

  @override
  String get quickAccessSwitchKeyboardTitle =>
      'Перейти на клавиатуру элементов';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'Сделать «Элементы CopyPaste» текущей клавиатурой.';

  @override
  String get quickAccessSwitchKeyboardAction => 'Выбрать клавиатуру';

  @override
  String get quickAccessKeyboardCurrent => 'Текущая клавиатура';

  @override
  String get quickAccessCaptureTierTitle => 'Режим записи';

  @override
  String get quickAccessCaptureTierDescription =>
      'Как сохраняются копии из других приложений.';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'Как сохраняются копии из других приложений.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'запись режим полный панель вручную shizuku собирать';

  @override
  String get quickAccessTierFull => 'Полный: каждая копия сохраняется в фоне';

  @override
  String get quickAccessTierPanelCollecting => 'Выбрана панель: сбор';

  @override
  String get quickAccessTierPanelCatchUp =>
      'Режим панели: синхронизация при открытии';

  @override
  String get quickAccessTierManual =>
      'Вручную: сохраняйте из приложения, плитки, виджета или через «Поделиться»';

  @override
  String get quickAccessTierPaused => 'Запись приостановлена';

  @override
  String get quickAccessAutoReturnTitle =>
      'Возвращаться к предыдущей клавиатуре после вставки';

  @override
  String get quickAccessAutoReturnDescription =>
      'Переключаться назад после вставки одного элемента.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'вернуться предыдущая клавиатура автоматически вставка';

  @override
  String get quickAccessFocusSearchTitle =>
      'Фокус на поиске при открытии панели';

  @override
  String get quickAccessFocusSearchDescription =>
      'Открывать панель с готовым поиском.';

  @override
  String get quickAccessTilesTitle => 'Плитки быстрых настроек';

  @override
  String get quickAccessTilesSearchDescription =>
      'Добавить плитки «Сохранить буфер обмена» и «Запись буфера обмена».';

  @override
  String get quickAccessTilesKeywords =>
      'быстрые настройки плитка шторка сохранить пауза';

  @override
  String get quickAccessTileInstructions =>
      'Смахните вниз дважды, нажмите «Изменить» и добавьте плитку CopyPaste.';

  @override
  String get quickAccessSaveTileTitle => 'Плитка «Сохранить буфер обмена»';

  @override
  String get quickAccessSaveTileDescription =>
      'Сохраняет содержимое буфера обмена из быстрых настроек.';

  @override
  String get quickAccessCaptureTileTitle => 'Плитка «Запись буфера обмена»';

  @override
  String get quickAccessCaptureTileDescription =>
      'Приостанавливает и возобновляет запись из быстрых настроек.';

  @override
  String get quickAccessAddTile => 'Добавить плитку';

  @override
  String get quickAccessTileAdded => 'Добавлено в быстрые настройки';

  @override
  String get quickAccessWidgetTitle => 'Виджет «Недавние элементы»';

  @override
  String get quickAccessWidgetDescription =>
      'Недавние элементы на главном экране, копирование одним касанием.';

  @override
  String get quickAccessWidgetInstructions =>
      'Удерживайте главный экран, нажмите «Виджеты» и добавьте «Недавние элементы».';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Добавлено $count виджета',
      many: 'Добавлено $count виджетов',
      few: 'Добавлено $count виджета',
      one: 'Добавлен 1 виджет',
      zero: 'Не добавлено',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Добавить виджет';

  @override
  String get quickAccessWidgetKeywords =>
      'главный экран виджет лаунчер недавние';

  @override
  String get snippetsClipTooLarge =>
      'Слишком большой для редактирования. Этот элемент открывается только для чтения.';

  @override
  String get snippetsConcealedConfirmAction => 'Продолжить';

  @override
  String get snippetsConcealedConfirmMessage =>
      'Это устройство не может вас проверить. Продолжайте только на собственном устройстве.';

  @override
  String get snippetsConcealedConfirmTitle => 'Подтвердите, что это вы';

  @override
  String get snippetsDiscard => 'Не сохранять';

  @override
  String get snippetsEditClip => 'Редактировать элемент';

  @override
  String get snippetsFormattingNotice =>
      'Форматирование будет удалено при сохранении.';

  @override
  String get snippetsNewText => 'Новый текст';

  @override
  String get snippetsReplaceOriginal => 'Заменить оригинал';

  @override
  String get snippetsReplaceOriginalHint =>
      'Удаляет исходный элемент после сохранения.';

  @override
  String get snippetsSaveFailed => 'Не удалось сохранить';

  @override
  String get snippetsUnsavedMessage => 'Сохранить изменения перед закрытием?';

  @override
  String get snippetsUnsavedTitle => 'Несохранённые изменения';

  @override
  String get quickPasteTabQueue => 'Очередь';

  @override
  String get quickPasteMaskedMode => 'Поле пароля: предпросмотры скрыты';

  @override
  String quickPasteMaskedItem(String kind) {
    return 'Скрыто: $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'Разблокировать секретные элементы';

  @override
  String quickPastePastingWith(String name) {
    return 'Вставка с: $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut вставляет оригинал';
  }

  @override
  String get quickPasteDefaultSkipped =>
      'Преобразование по умолчанию пропущено';

  @override
  String get quickPasteDefaultSkippedMessage =>
      'Вместо него использован оригинал.';

  @override
  String get quickPastePasteManually =>
      'Содержимое в буфере обмена. Вставьте его вручную.';

  @override
  String get quickPastePaste => 'Вставить';

  @override
  String get rulesLoadFailed => 'Не удалось загрузить правила содержимого.';

  @override
  String get rulesNewRule => 'Новое правило';

  @override
  String get rulesEditRule => 'Редактировать правило';

  @override
  String get rulesUntitled => 'Правило без названия';

  @override
  String get rulesPrivacyDefaultsTitle =>
      'Настройки конфиденциальности по умолчанию';

  @override
  String get rulesPrivacyDefaultsDescription =>
      'Настройки конфиденциальности применяются раньше любых правил.';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'конфиденциальность скрытый временный исключённые приложения встроенные учётные данные пароли платёжные карты';

  @override
  String get rulesOpenPrivacy => 'Открыть «Конфиденциальность»';

  @override
  String get rulesMyRulesTitle => 'Мои правила';

  @override
  String get rulesMyRulesDescription =>
      'Выполняются по порядку для новых копий.';

  @override
  String get rulesMyRulesKeywords =>
      'правила автоматизация тег пропустить секрет срок удалить шаблон регулярное выражение';

  @override
  String get rulesEmptyTitle => 'Правил пока нет';

  @override
  String get rulesEmptyMessage =>
      'Правила назначают теги, защищают, ограничивают срок или пропускают новые копии.';

  @override
  String get rulesDeletedTitle => 'Недавно удалённые';

  @override
  String get rulesDeletedDescription => 'Можно восстановить в течение 30 дней.';

  @override
  String rulesDeletedAt(String time) {
    return 'Удалено: $time';
  }

  @override
  String get rulesRestore => 'Восстановить';

  @override
  String get rulesDeletedToast => 'Правило удалено';

  @override
  String get rulesDeletedToastMessage =>
      'Его можно восстановить в течение 30 дней.';

  @override
  String get rulesUndo => 'Отменить';

  @override
  String rulesConflictMessage(String rule, String device) {
    return 'Одновременное изменение правила «$rule» от $device заменено более новым.';
  }

  @override
  String get rulesAnotherDevice => 'другого устройства';

  @override
  String get rulesRestoreConflict => 'Использовать ту версию';

  @override
  String get rulesSkipLogTitle => 'Недавно пропущенные';

  @override
  String get rulesSkipLogDescription =>
      'Копии, которые ваши правила не сохранили на этом устройстве.';

  @override
  String get rulesSkipLogKeywords =>
      'пропущенные проигнорированные не сохранённые журнал';

  @override
  String get rulesSkipLogShow => 'Показать';

  @override
  String get rulesSkipLogEmpty => 'Ни одна копия не была пропущена.';

  @override
  String get rulesUnknownApp => 'Неизвестное приложение';

  @override
  String get rulesMatchesEverything => 'Любая копия';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary, затем остановить';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': ' или ',
      'other': ' и ',
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
    return 'Работает на: $devices';
  }

  @override
  String rulesNewFromDevice(String device) {
    return 'Новое с $device';
  }

  @override
  String get rulesNeedsUpdateBadge => 'Требуется обновление';

  @override
  String get rulesUpdateRequired =>
      'Обновите CopyPaste, чтобы выполнять это правило.';

  @override
  String get rulesErrorBadge => 'Ошибка';

  @override
  String rulesErrorHint(String problem) {
    return '$problem Пока не исправлено, совпадения защищены.';
  }

  @override
  String get rulesReorder => 'Перетащите, чтобы изменить порядок';

  @override
  String get rulesMoreActions => 'Другие действия';

  @override
  String get rulesEdit => 'Редактировать';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': 'Переместить вверх',
      'down': 'Переместить вниз',
      'top': 'В начало',
      'other': 'В конец',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => 'Применить к существующим элементам…';

  @override
  String get rulesResetBuiltin => 'Сбросить к значению по умолчанию';

  @override
  String get rulesNameLabel => 'Название';

  @override
  String get rulesNamePlaceholder => 'Например, рабочие ссылки';

  @override
  String get rulesWhenTitle => 'Когда';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'Соответствует любое условие',
      'other': 'Соответствуют все условия',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions =>
      'Без условий: правило применяется к каждой копии.';

  @override
  String get rulesAddCondition => 'Добавить условие';

  @override
  String get rulesRemoveCondition => 'Удалить условие';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'Текст',
      'content_type': 'Тип содержимого',
      'detected_kind': 'Обнаруженный тип',
      'source_app': 'Приложение-источник',
      'size': 'Размер',
      'privacy_marker': 'Метка конфиденциальности',
      'capture_origin': 'Источник',
      'other': 'Неизвестное условие',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'содержит',
      'equals': 'равен',
      'starts_with': 'начинается с',
      'ends_with': 'заканчивается на',
      'matches_regex': 'соответствует шаблону',
      'other': 'соответствует',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'не содержит',
      'equals': 'не равен',
      'starts_with': 'не начинается с',
      'ends_with': 'не заканчивается на',
      'matches_regex': 'не соответствует шаблону',
      'other': 'не соответствует',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'не входит в',
      'other': 'входит в',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'меньше',
      'other': 'больше',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'не меньше',
      'other': 'не больше',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'строки',
      'bytes': 'байты',
      'other': 'символы',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'строки',
      'bytes': 'байта',
      'other': 'символа',
    });
    String _temp1 = intl.Intl.selectLogic(unit, {
      'lines': 'строк',
      'bytes': 'байт',
      'other': 'символов',
    });
    String _temp2 = intl.Intl.selectLogic(unit, {
      'lines': 'строки',
      'bytes': 'байта',
      'other': 'символа',
    });
    String _temp3 = intl.Intl.selectLogic(unit, {
      'lines': 'строка',
      'bytes': 'байт',
      'other': 'символ',
    });
    String _temp4 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count $_temp0',
      many: '$count $_temp1',
      few: '$count $_temp2',
      one: '$count $_temp3',
    );
    return '$_temp4';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'Обычный текст',
      'rich_text': 'Форматированный текст',
      'image': 'Изображение',
      'files': 'Файлы',
      'color': 'Цвет',
      'link': 'Ссылка',
      'other': 'Другое',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'Пароль или секрет',
      'url': 'Веб-адрес',
      'email': 'Адрес электронной почты',
      'phone': 'Номер телефона',
      'code': 'Код',
      'color': 'Значение цвета',
      'json': 'JSON',
      'file_path': 'Путь к файлу',
      'number': 'Число',
      'one_time_code': 'Одноразовый код',
      'payment_card': 'Номер платёжной карты',
      'other': 'Другое',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': 'Скрытый',
      'transient': 'Временный',
      'auto_generated': 'Создан автоматически',
      'excluded_from_history': 'Исключён из истории',
      'other': 'Другое',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'Скопировано на этом устройстве',
      'received': 'Получено с другого устройства',
      'created_in_app': 'Создано в CopyPaste',
      'other': 'Другое',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => 'Текст для поиска';

  @override
  String get rulesPatternPlaceholder => 'Шаблон, например ^ticket-[0-9]+';

  @override
  String get rulesPatternHint =>
      'Просмотр вперёд и назад (look-around) и обратные ссылки не поддерживаются.';

  @override
  String get rulesCaseSensitive => 'Учитывать регистр';

  @override
  String get rulesChooseValues => 'Выбрать';

  @override
  String get rulesUnknownConditionNotice =>
      'Это условие из более новой версии CopyPaste.';

  @override
  String get rulesSourceAppNotice =>
      'На Android совпадают только копии, сделанные с клавиатуры элементов.';

  @override
  String get rulesPrivacyMarkerNotice =>
      'Не каждая система сообщает каждую метку.';

  @override
  String get rulesThenTitle => 'Тогда';

  @override
  String get rulesNoActions => 'Добавьте хотя бы одно действие.';

  @override
  String get rulesAddAction => 'Добавить действие';

  @override
  String get rulesRemoveAction => 'Удалить действие';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': 'Не сохранять',
      'mark_secret': 'Пометить как секрет',
      'delete_after': 'Удаление по таймеру',
      'add_tags': 'Добавить теги',
      'transform': 'Преобразовать',
      'other': 'Неизвестное действие',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone': '«Не сохранять» нельзя сочетать с другими действиями.',
      'present': 'В этом правиле уже есть это действие.',
      'other': 'Правило может содержать до 8 действий.',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'Это действие из более новой версии CopyPaste.';

  @override
  String get rulesDontSaveWarning =>
      'Совпадающие копии никогда не сохраняются, не индексируются и не синхронизируются.';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count минуты',
      many: '$count минут',
      few: '$count минуты',
      one: '$count минута',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count часа',
      many: '$count часов',
      few: '$count часа',
      one: '$count час',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count дня',
      many: '$count дней',
      few: '$count дня',
      one: '$count день',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry =>
      'Также очищать буфер обмена, если в нём всё ещё эта копия.';

  @override
  String get rulesNoTags =>
      'Создайте теги в истории, чтобы использовать их здесь.';

  @override
  String get rulesNoTransforms => 'Нет доступных преобразований.';

  @override
  String get rulesReplaceClipboard => 'Также заменить буфер обмена результатом';

  @override
  String get rulesOptionsTitle => 'Параметры';

  @override
  String get rulesEnabledSwitch => 'Включено';

  @override
  String get rulesStopProcessing =>
      'Не обрабатывать следующие правила, если сработало это';

  @override
  String get rulesRunsOnLabel => 'Работает на';

  @override
  String get rulesAllDevices => 'Все устройства';

  @override
  String get rulesChosenDevices => 'Выбранные устройства';

  @override
  String get rulesTestTitle => 'Проверка';

  @override
  String get rulesTestDescription => 'Проверьте правило на образце текста.';

  @override
  String get rulesSamplePlaceholder => 'Вставьте или введите образец текста';

  @override
  String get rulesTestFullSet => 'Проверять вместе со всеми правилами';

  @override
  String get rulesRunTest => 'Проверить';

  @override
  String get rulesTestFailed => 'Не удалось выполнить проверку.';

  @override
  String get rulesPreviewHistory => 'Просмотреть на истории';

  @override
  String get rulesSaveFailed => 'Не удалось сохранить правило.';

  @override
  String get rulesReadOnlyNotice =>
      'Создано более новой версией CopyPaste. Обновите, чтобы изменить или запустить.';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'Не удалось прочитать правило.',
      'unsupported_schema': 'Это правило требует более новой версии CopyPaste.',
      'unknown_condition_type':
          'Это условие требует более новой версии CopyPaste.',
      'unknown_action_type':
          'Это действие требует более новой версии CopyPaste.',
      'too_many_conditions': 'Правило может содержать до 20 условий.',
      'too_many_actions': 'Правило может содержать до 8 действий.',
      'too_many_rules': 'Выполняются только первые 200 правил.',
      'no_actions': 'Добавьте хотя бы одно действие.',
      'conflicting_actions':
          '«Не сохранять» нельзя сочетать с другими действиями.',
      'duplicate_action': 'Каждое действие можно использовать только один раз.',
      'empty_value': 'Введите значение.',
      'empty_list': 'Выберите хотя бы одно значение.',
      'too_many_values': 'Выберите до 64 значений.',
      'invalid_value': 'Это значение недействительно.',
      'pattern_too_long': 'Шаблоны могут содержать до 2000 символов.',
      'unsupported_look_around':
          'Просмотр вперёд и назад не поддерживается. Вместо этого используйте два условия с «Соответствуют все условия».',
      'unsupported_backreference': 'Обратные ссылки не поддерживаются.',
      'pattern_too_large':
          'Этот шаблон слишком сложный. Упростите его повторения.',
      'invalid_pattern': 'Этот шаблон недействителен.',
      'invalid_duration': 'Выберите длительность от 1 минуты до 90 дней.',
      'invalid_tags': 'Выберите хотя бы один тег.',
      'invalid_transform': 'Выберите преобразование.',
      'other': 'В этом правиле есть проблема.',
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
      'matched': 'совпадает',
      'not_matched': 'не совпадает',
      'unknown': 'неизвестно',
      'skipped': 'не выполнялось',
      'other': 'ошибка',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return 'Условие $number';
  }

  @override
  String get rulesTraceNoRules => 'Ни одно правило не проверялось.';

  @override
  String get rulesTraceStopped => 'Следующие правила не выполнялись.';

  @override
  String get rulesTraceUnknownReason =>
      'Неизвестно для этой копии, поэтому совпадения нет.';

  @override
  String get rulesTraceRuleError =>
      'Это правило не удалось выполнить, поэтому совпадения защищены.';

  @override
  String get rulesTraceOutcomeTitle => 'Результат';

  @override
  String get rulesTraceNoEffect => 'Ни одно правило не изменяет эту копию.';

  @override
  String get rulesTraceSkipped => 'Не сохраняется';

  @override
  String get rulesTraceSecret => 'Сохраняется как секрет';

  @override
  String rulesTraceExpires(String duration) {
    return 'Удаляется через $duration';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'Теги: $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return 'Преобразовано: $names';
  }

  @override
  String get rulesTraceOutput => 'Преобразованный текст';

  @override
  String get rulesPreviewTitle => 'Применить к существующим элементам';

  @override
  String rulesPreviewRunning(int count) {
    return 'Проверка последних $count элементов…';
  }

  @override
  String get rulesPreviewNone =>
      'Ни один недавний элемент не соответствует этому правилу.';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Совпадают $count элемента',
      many: 'Совпадают $count элементов',
      few: 'Совпадают $count элемента',
      one: 'Совпадает $count элемент',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Будет удалено $count элемента',
      many: 'Будет удалено $count элементов',
      few: 'Будет удалено $count элемента',
      one: 'Будет удалён $count элемент',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count элемента будут помечены как секрет',
      many: '$count элементов будут помечены как секрет',
      few: '$count элемента будут помечены как секрет',
      one: '$count элемент будет помечен как секрет',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count элемента получат время удаления',
      many: '$count элементов получат время удаления',
      few: '$count элемента получат время удаления',
      one: '$count элемент получит время удаления',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count элемента получат теги',
      many: '$count элементов получат теги',
      few: '$count элемента получат теги',
      one: '$count элемент получит теги',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return 'Проверены только последние $count элементов.';
  }

  @override
  String rulesPreviewMore(int count) {
    return 'и ещё $count';
  }

  @override
  String get rulesPreviewNote => 'Ничего не изменится, пока вы не примените.';

  @override
  String get rulesSecretItem => 'Секретный элемент';

  @override
  String rulesSecretItemFrom(String app) {
    return 'Секретный элемент из $app';
  }

  @override
  String get rulesPreviewCancelled =>
      'Просмотр остановлен. Ничего не изменено.';

  @override
  String get rulesPreviewFailed => 'Не удалось выполнить просмотр.';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Удалить $count элемента?',
      many: 'Удалить $count элементов?',
      few: 'Удалить $count элемента?',
      one: 'Удалить $count элемент?',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage =>
      'Совпадающие элементы будут удалены на всех ваших устройствах. Это нельзя отменить.';

  @override
  String get rulesApplyDeleteConfirm => 'Удалить и применить';

  @override
  String rulesApplyProgress(int done, int total) {
    return 'Применение: $done из $total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Изменено $count элемента',
      many: 'Изменено $count элементов',
      few: 'Изменено $count элемента',
      one: 'Изменён $count элемент',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Удалено $count элемента',
      many: 'Удалено $count элементов',
      few: 'Удалено $count элемента',
      one: 'Удалён $count элемент',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Не удалось изменить $count элемента',
      many: 'Не удалось изменить $count элементов',
      few: 'Не удалось изменить $count элемента',
      one: 'Не удалось изменить $count элемент',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled =>
      'Остановлено. Уже внесённые изменения сохранятся.';

  @override
  String get historyActionEdit => 'Редактировать';

  @override
  String get historyActionAddToQueue => 'Добавить в Paste Queue';

  @override
  String get historyActionSendTo => 'Отправить на…';

  @override
  String get historySendDialogTitle => 'Отправить на устройство';

  @override
  String get historySendDescription =>
      'Отправляет сейчас, даже если автосинхронизация приостановлена.';

  @override
  String get historySendNoDevices =>
      'Ни одно сопряжённое устройство ещё не синхронизировалось.';

  @override
  String get historySendAction => 'Отправить';

  @override
  String get historySentTitle => 'Отправлено';

  @override
  String historySentTo(String devices) {
    return 'Отправлено на $devices.';
  }

  @override
  String get historySendFailedTitle => 'Не отправлено';

  @override
  String historySendBlocked(String device) {
    return 'Настройки синхронизации $device не разрешают передачу этого элемента.';
  }

  @override
  String historySendFailed(String device) {
    return '$device не принял элемент. Убедитесь, что устройство в сети, и попробуйте ещё раз.';
  }

  @override
  String get historySendUnavailable => 'Отправка сейчас недоступна.';

  @override
  String get historyActionTransform => 'Преобразовать…';

  @override
  String get historyActionExplain => 'Почему это здесь?';

  @override
  String get historyActionKeep => 'Оставить';

  @override
  String get historyNewClip => 'Новый элемент';

  @override
  String get historySavedTitle => 'Сохранено';

  @override
  String get historyEditSaved =>
      'Изменённый текст стал новым элементом в начале истории.';

  @override
  String get historyNewClipSaved => 'Ваш текст сохранён.';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count элемента добавлены в очередь.',
      many: '$count элементов добавлены в очередь.',
      few: '$count элемента добавлены в очередь.',
      one: '$count элемент добавлен в очередь.',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed =>
      'Не удалось добавить в Paste Queue. Проверьте, включена ли она.';

  @override
  String get historyKeepFailed => 'Не удалось оставить этот элемент.';

  @override
  String get historyExplainFailed => 'Не удалось загрузить объяснение.';

  @override
  String get historyDeletesSoon => 'Удалится меньше чем через минуту';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Удалится через $count мин',
      many: 'Удалится через $count мин',
      few: 'Удалится через $count мин',
      one: 'Удалится через $count мин',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Удалится через $count ч',
      many: 'Удалится через $count ч',
      few: 'Удалится через $count ч',
      one: 'Удалится через $count ч',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Удалится через $count дня',
      many: 'Удалится через $count дней',
      few: 'Удалится через $count дня',
      one: 'Удалится через $count день',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => 'Вручную';

  @override
  String get historyProvenanceEdited => 'Изменено';

  @override
  String get historyProvenanceTransformed => 'Преобразовано';

  @override
  String get historyProvenanceModule => 'Модуль';

  @override
  String historyRuleTag(String name) {
    return '$name (авто)';
  }

  @override
  String get historyMetadataExpiry => 'Удаление';

  @override
  String get historyMetadataRuleTags => 'Автоматические теги';

  @override
  String modulesEnableTitle(String module) {
    return 'Включить $module?';
  }

  @override
  String get modulesEnableAccess => 'Модуль сможет:';

  @override
  String modulesCapability(String capability) {
    String _temp0 = intl.Intl.selectLogic(capability, {
      'history_read': 'Читать историю буфера обмена',
      'clipboard_write': 'Копировать в буфер обмена',
      'library_write': 'Сохранять фрагменты и закреплять элементы',
      'snippets': 'Читать ваши фрагменты',
      'synced_records': 'Синхронизировать свои данные между устройствами',
      'local_state': 'Хранить свои данные на этом устройстве',
      'prompts': 'Запрашивать у вас подтверждение действий',
      'endpoints': 'Принимать подключения от локальных приложений',
      'other': 'Использовать сервис CopyPaste',
    });
    return '$_temp0';
  }

  @override
  String modulesPromptFrom(String module) {
    return 'От $module';
  }

  @override
  String get modulesUnknownModule => 'Неизвестный модуль';

  @override
  String get modulesPromptAnswerFailed => 'Ваш ответ не отправлен.';

  @override
  String get modulesPromptNotificationBody =>
      'Откройте CopyPaste, чтобы ответить.';

  @override
  String get modulesUnknownDevice => 'Неизвестное устройство';

  @override
  String get modulesChooseDevices => 'Выберите устройства';

  @override
  String get modulesAddApp => 'Добавить приложение';

  @override
  String modulesRemoveApp(String app) {
    return 'Удалить $app';
  }

  @override
  String get modulesNoApps => 'Приложения не выбраны';

  @override
  String get settingsSectionDiagnostics => 'Диагностика';

  @override
  String get settingsSectionDiagnosticsSummary =>
      'Журналы работы для устранения неполадок. Содержимое буфера обмена никогда не записывается.';

  @override
  String get diagnosticsSearchPlaceholder => 'Поиск в журналах';

  @override
  String get diagnosticsAllLevels => 'Все уровни';

  @override
  String get diagnosticsAllProcesses => 'Все процессы';

  @override
  String get diagnosticsLevelError => 'Ошибка';

  @override
  String get diagnosticsLevelWarning => 'Предупреждение';

  @override
  String get diagnosticsLevelInfo => 'Информация';

  @override
  String get diagnosticsLevelDebug => 'Отладка';

  @override
  String get diagnosticsLevelTrace => 'Трассировка';

  @override
  String get diagnosticsProcessApp => 'Приложение';

  @override
  String get diagnosticsProcessDaemon => 'Фоновая служба';

  @override
  String get diagnosticsRefresh => 'Обновить';

  @override
  String get diagnosticsExport => 'Экспортировать';

  @override
  String get diagnosticsExportFileType => 'Текстовый файл';

  @override
  String get diagnosticsExported => 'Диагностика экспортирована';

  @override
  String get diagnosticsExportedMessage =>
      'Файл журнала готов к отправке в поддержку.';

  @override
  String get diagnosticsExportFailed => 'Не удалось экспортировать';

  @override
  String get diagnosticsExportFailedMessage =>
      'Не удалось сохранить файл журнала.';

  @override
  String get diagnosticsLoadFailed => 'Не удалось загрузить журналы';

  @override
  String get diagnosticsEmpty => 'Нет записей в журнале';

  @override
  String get diagnosticsLogLevelTitle => 'Уровень журналирования';

  @override
  String get diagnosticsLogLevelDescription =>
      'Сколько подробностей записывается на этом устройстве. Содержимое буфера обмена никогда не попадает в журнал.';

  @override
  String get onboardingAndroidCaptureMethod => 'Способ обнаружения';

  @override
  String get onboardingAndroidLogTrigger => 'Системный журнал (рекомендуется)';

  @override
  String get onboardingAndroidLogTriggerDescription =>
      'Мгновенно, без дополнительного расхода заряда. Требуется однократный доступ через Shizuku или ADB.';

  @override
  String get onboardingAndroidPolling => 'Опрос';

  @override
  String get onboardingAndroidPollingDescription =>
      'Проверяет каждую секунду, пока экран включён. Требуется только разрешение на наложение; расходует больше заряда.';

  @override
  String get onboardingAndroidOverlay => 'Поверх других приложений';

  @override
  String get onboardingAndroidOverlayDescription =>
      'Позволяет невидимому окну размером 1 пиксель читать буфер обмена.';

  @override
  String get onboardingAndroidOverlaySettingsFailed =>
      'Не удалось открыть настройки наложения.';

  @override
  String get settingsBackgroundSyncTitle => 'Фоновая синхронизация';

  @override
  String get settingsBackgroundSyncDescription =>
      'Синхронизировать, когда приложение закрыто. Показывает уведомление.';

  @override
  String get settingsBackgroundSyncKeywords =>
      'фон постоянно служба уведомление аккумулятор keep alive persistent service notification battery';

  @override
  String get settingsBackgroundSyncStoppedTitle =>
      'Фоновая синхронизация не работает';

  @override
  String get settingsBackgroundSyncStoppedMessage =>
      'Разрешите уведомления, чтобы Android не останавливал синхронизацию.';

  @override
  String get settingsBackgroundSyncResume => 'Возобновить синхронизацию';

  @override
  String get quickAccessSendShortcutTitle => 'Ярлык «Отправить буфер»';

  @override
  String get quickAccessSendShortcutDescription =>
      'Сохраняет текущий буфер обмена и отправляет его на ваши устройства.';

  @override
  String get quickAccessTransformShortcutTitle => 'Ярлык «Преобразовать буфер»';

  @override
  String get quickAccessTransformShortcutDescription =>
      'Открывает преобразования для текущего буфера обмена.';

  @override
  String get quickAccessAddToHomeScreen => 'Добавить на главный экран';

  @override
  String get quickAccessShortcutKeywords =>
      'ярлык главный экран лаунчер значок shortcut home screen launcher icon';

  @override
  String fileSetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count файла',
      many: '$count файлов',
      few: '$count файла',
      one: '$count файл',
    );
    return '$_temp0';
  }

  @override
  String get fileSetShowFolder => 'Показать папку';

  @override
  String get historyFilesSaved => 'Файлы сохранены.';

  @override
  String get transfersResume => 'Продолжить';

  @override
  String get settingsStartInBackgroundTitle => 'Запускать в фоне';

  @override
  String get settingsStartInBackgroundDescription =>
      'Когда CopyPaste запускается при входе, окно не открывается.';

  @override
  String get settingsWindowGlassTitle => 'Эффект стекла';

  @override
  String get settingsWindowGlassDescription =>
      'Размывает рабочий стол за окном. Отключается, когда система уменьшает прозрачность.';

  @override
  String get settingsRestoreClipboardTitle =>
      'Восстанавливать буфер обмена при запуске';

  @override
  String get settingsRestoreClipboardDescription =>
      'При запуске возвращает последний элемент в пустой буфер обмена. Секретные элементы пропускаются.';

  @override
  String get settingsStorageUsageTitle => 'Использование хранилища';

  @override
  String get settingsStorageUsageDescription =>
      'Место, которое CopyPaste занимает на этом устройстве.';

  @override
  String get settingsStorageUsageUnavailable =>
      'Не удалось прочитать использование хранилища.';

  @override
  String get settingsStorageHistory => 'История';

  @override
  String get settingsStorageFiles => 'Файлы';

  @override
  String get settingsStorageCaches => 'Кеши';

  @override
  String get settingsStorageUpgradeBackups =>
      'Резервные копии перед обновлением';

  @override
  String get settingsStorageOther => 'Журналы, модули и настройки';

  @override
  String get settingsStorageTotal => 'Всего';

  @override
  String get settingsClearCacheTitle => 'Очистить кеш';

  @override
  String get settingsClearCacheDescription =>
      'Удаляет предпросмотры и временные файлы. История остаётся.';

  @override
  String get settingsClearCacheAction => 'Очистить';

  @override
  String get settingsClearCacheDoneTitle => 'Кеш очищен';

  @override
  String get settingsClearCacheDoneMessage =>
      'Предпросмотры создаются заново, когда понадобятся.';

  @override
  String get settingsClearCacheFailed => 'Не удалось очистить кеш.';

  @override
  String get settingsDataFolderTitle => 'Папка данных';

  @override
  String get settingsDataFolderDescription =>
      'Показывает, где хранятся история, резервные копии и модули.';

  @override
  String get settingsLogsFolderTitle => 'Папка журналов';

  @override
  String get settingsLogsFolderDescription =>
      'Показывает диагностические журналы этого устройства.';

  @override
  String get settingsOpenFolderAction => 'Открыть';

  @override
  String get settingsOpenFolderFailed => 'Не удалось открыть папку.';

  @override
  String get settingsConfirmDeleteTitle => 'Подтверждать удаление';

  @override
  String get settingsConfirmDeleteDescription =>
      'Спрашивать перед удалением одного элемента. При удалении нескольких подтверждение требуется всегда.';

  @override
  String get settingsUpgradeBackupsTitle => 'Резервные копии перед обновлением';

  @override
  String get settingsUpgradeBackupsDescription =>
      'Зашифрованные копии, созданные перед обновлением формата данных. Сохраняются 5 последних.';

  @override
  String get settingsUpgradeBackupsEmpty =>
      'Резервных копий обновлений пока нет.';

  @override
  String settingsUpgradeBackupEntry(String date, String size) {
    return '$date · $size';
  }

  @override
  String get settingsUpgradeBackupRestoreAction => 'Восстановить';

  @override
  String get settingsUpgradeBackupRestoreTitle =>
      'Восстановить эту резервную копию?';

  @override
  String settingsUpgradeBackupRestoreMessage(String date) {
    return 'История будет заменена копией от $date. Элементы, добавленные после этого, будут удалены.';
  }

  @override
  String get settingsDownloadUpdatesTitle => 'Загружать обновления в фоне';

  @override
  String get settingsDownloadUpdatesDescription =>
      'Заранее загружает и проверяет найденное обновление, чтобы установка была мгновенной. Не загружает в лимитированных сетях и в режиме экономии энергии.';

  @override
  String settingsUpdateDownloadedReady(String available) {
    return 'CopyPaste $available загружен и готов к установке.';
  }

  @override
  String get devicesSpaceTitle => 'Ваше пространство';

  @override
  String devicesSpaceSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count устройства',
      many: '$count устройств',
      few: '$count устройства',
      one: 'Только это устройство',
    );
    return '$_temp0';
  }

  @override
  String get devicesSpaceKeyPending =>
      'Устройства не в сети получат новый ключ, когда появятся в сети.';

  @override
  String get devicesSpaceWaitingForKey => 'Требуется новый ключ';

  @override
  String get devicesSpaceRemovedTitle => 'Не в пространстве';

  @override
  String get devicesSpaceRemovedMessage =>
      'Это устройство было удалено или вышло. Его история остаётся здесь. Присоединитесь к пространству, чтобы снова синхронизироваться.';

  @override
  String get devicesSpaceJoin => 'Присоединиться к пространству';

  @override
  String get devicesRemoveFromSpace => 'Удалить из пространства';

  @override
  String devicesRemoveDialogTitle(String device) {
    return 'Удалить $device?';
  }

  @override
  String get devicesRemoveDialogMessage =>
      'Оно будет удалено на всех ваших устройствах и с этого момента не сможет читать ничего синхронизированного.';

  @override
  String get devicesRemoveNewPassphrase =>
      'Новая парольная фраза синхронизации (необязательно)';

  @override
  String get devicesRemoveNewPassphraseHint =>
      'Удалённое устройство знает текущую парольную фразу. Без новой фразы добавляйте устройства только по приглашению.';

  @override
  String get devicesRemoveSignOutOthers =>
      'Выйти из облачной учётной записи на других устройствах';

  @override
  String get devicesPassphraseTooShort => 'Используйте не менее 12 символов.';

  @override
  String get devicesLeaveSpace => 'Покинуть пространство';

  @override
  String get devicesLeaveDialogTitle => 'Покинуть это пространство?';

  @override
  String get devicesLeaveDialogMessage =>
      'Ваши другие устройства удалят его. Его история останется здесь.';

  @override
  String get devicesLeave => 'Покинуть';

  @override
  String get onboardingSpaceTitle => 'Ваши устройства';

  @override
  String get onboardingSpaceSubtitle =>
      'Синхронизируйте буфер обмена между своими устройствами. Всё остаётся зашифрованным.';

  @override
  String get onboardingSpaceCreateTitle => 'Создать пространство';

  @override
  String get onboardingSpaceCreateDescription =>
      'Это моё первое устройство с CopyPaste.';

  @override
  String get onboardingSpaceJoinTitle => 'Присоединиться к пространству';

  @override
  String get onboardingSpaceJoinDescription =>
      'Я уже использую CopyPaste на другом устройстве.';

  @override
  String get onboardingSpaceLater => 'Позже';

  @override
  String get onboardingReadySpaceCreated =>
      'Ваше пространство готово. Добавьте другие устройства сейчас или позже в разделе «Устройства».';

  @override
  String get onboardingReadyJoin =>
      'Теперь подтвердите код, показанный на другом устройстве.';

  @override
  String get onboardingAddDeviceNow => 'Добавить устройство';

  @override
  String get onboardingJoinNow => 'Присоединиться';

  @override
  String get onboardingBadgeWelcome => 'Добро пожаловать';

  @override
  String get onboardingBadgeSetup => 'Настройка';

  @override
  String get onboardingBadgeDevices => 'Устройства';

  @override
  String get onboardingBadgeReady => 'Готово';

  @override
  String get onboardingPairJoinTitle => 'Присоединитесь к своему пространству';

  @override
  String get onboardingPairJoinSubtitle =>
      'Введите код или отсканируйте QR-код с другого устройства. Затем проверьте, что оба устройства показывают одинаковый код проверки.';

  @override
  String get onboardingPairAddDeviceSubtitle =>
      'На другом устройстве откройте CopyPaste и присоединитесь по этому QR-коду или коду. Затем проверьте, что оба устройства показывают одинаковый код проверки.';

  @override
  String get onboardingPairSkip => 'Пока пропустить';

  @override
  String get onboardingPairInterruptedTitle => 'Сопряжение не завершено';

  @override
  String get onboardingPairInterruptedMessage =>
      'Попробуйте ещё раз или продолжите и выполните сопряжение позже в разделе «Устройства».';

  @override
  String onboardingReadyPaired(String device) {
    return 'Сопряжено с $device. Ваш буфер обмена теперь синхронизируется между ними.';
  }

  @override
  String get onboardingReadyPairedUnnamed =>
      'Ваши устройства сопряжены. Буфер обмена теперь синхронизируется между ними.';

  @override
  String get onboardingReadyPairLater =>
      'Сопрячьте устройства в любое время в разделе «Устройства».';

  @override
  String get settingsSectionShortcuts => 'Сочетания клавиш';

  @override
  String get settingsSectionShortcutsSummary =>
      'Глобальные сочетания клавиш. Отключите те, которыми не пользуетесь.';

  @override
  String get settingsShortcutEnabledLabel => 'Отключить это сочетание клавиш';

  @override
  String get settingsWelcomeTourTitle => 'Приветственный тур';

  @override
  String get settingsWelcomeTourDescription =>
      'Пройти начальную настройку ещё раз.';

  @override
  String get settingsWelcomeTourAction => 'Показать снова';

  @override
  String get modulesDetails => 'Подробности';

  @override
  String get modulesDetailsFeatures => 'Что добавляет';

  @override
  String get modulesDetailsSearch =>
      'Ищет элементы по смыслу с помощью локальной модели.';

  @override
  String get modulesDetailsPermissions => 'Разрешения';

  @override
  String get modulesDetailsPermissionsPending =>
      'Показываются после установки. Вы подтверждаете их при включении модуля.';

  @override
  String get modulesDetailsNoPermissions => 'Не использует службы CopyPaste.';

  @override
  String get modulesDetailsInformation => 'Информация';

  @override
  String get modulesDetailsVersion => 'Версия';

  @override
  String get modulesDetailsSize => 'Размер';

  @override
  String get modulesDetailsRequirements => 'Требования';

  @override
  String get modulesDetailsStatus => 'Состояние';

  @override
  String get errorUnexpected => 'Что-то пошло не так. Попробуйте ещё раз.';

  @override
  String get errorRuntimeUnavailable =>
      'CopyPaste не работает в фоне. Перезапустите приложение.';

  @override
  String get errorRuntimeStartFailed =>
      'Не удалось запустить фоновую службу CopyPaste. Перезапустите приложение.';

  @override
  String get errorRuntimeStarting =>
      'CopyPaste ещё запускается. Попробуйте немного позже.';

  @override
  String get errorRuntimeTimeout =>
      'CopyPaste не ответил вовремя. Попробуйте ещё раз.';

  @override
  String get errorRuntimeMismatch =>
      'CopyPaste обновлён. Перезапустите приложение, чтобы завершить.';

  @override
  String get errorPeerUnreachable =>
      'Не удалось связаться с другим устройством. Проверьте, что оно в сети.';

  @override
  String get errorPeerVersion =>
      'На другом устройстве несовместимая версия CopyPaste. Обновите оба устройства.';

  @override
  String get errorRateLimited =>
      'Слишком много попыток. Подождите немного и попробуйте ещё раз.';

  @override
  String devicesSyncFailedWith(String devices) {
    return 'Не удалось синхронизироваться с $devices.';
  }

  @override
  String get transfersUnavailableTitle => 'Не удалось загрузить передачи';

  @override
  String get historyTagsLoadFailedTitle => 'Не удалось загрузить теги';

  @override
  String get settingsEraseAllDataTitle => 'Удалить все данные';

  @override
  String get settingsEraseAllDataDescription =>
      'Удаляет историю, настройки и ключ шифрования, затем закрывает CopyPaste.';

  @override
  String get settingsEraseAllDataAction => 'Удалить всё';

  @override
  String get settingsEraseAllDataConfirmTitle =>
      'Удалить все данные CopyPaste?';

  @override
  String get settingsEraseAllDataConfirmMessage =>
      'История, файлы, настройки и ключ шифрования на этом устройстве будут удалены, а CopyPaste закроется. Сопряжённые устройства и облачные копии останутся. Это нельзя отменить.';

  @override
  String get settingsEraseAllDataFailed =>
      'Не удалось удалить часть данных CopyPaste.';
}
