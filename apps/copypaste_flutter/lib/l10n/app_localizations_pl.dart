// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Polish (`pl`).
class AppLocalizationsPl extends AppLocalizations {
  AppLocalizationsPl([String locale = 'pl']) : super(locale);

  @override
  String get commonCancel => 'Anuluj';

  @override
  String get commonClose => 'Zamknij';

  @override
  String get commonDone => 'Gotowe';

  @override
  String get commonRetry => 'Ponów';

  @override
  String get commonTryAgain => 'Spróbuj ponownie';

  @override
  String get commonCopy => 'Kopiuj';

  @override
  String get commonDelete => 'Usuń';

  @override
  String get commonSave => 'Zapisz';

  @override
  String get commonContinue => 'Kontynuuj';

  @override
  String get commonBack => 'Wstecz';

  @override
  String get commonOpenSettings => 'Otwórz ustawienia';

  @override
  String get commonLoading => 'Wczytywanie';

  @override
  String get commonApply => 'Zastosuj';

  @override
  String get commonClear => 'Wyczyść';

  @override
  String get commonPin => 'Przypnij';

  @override
  String get commonEnable => 'Włącz';

  @override
  String get commonChoose => 'Wybierz';

  @override
  String get settingsLanguageTitle => 'Język';

  @override
  String get settingsLanguageDescription =>
      'Wybierz język interfejsu CopyPaste.';

  @override
  String get settingsLanguageSystem => 'Domyślny systemowy';

  @override
  String get settingsLanguageKeywords =>
      'język lokalizacja tłumaczenie interfejs language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Nie można zmienić uruchamiania przy logowaniu. Spróbuj ponownie.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'Nie można otworzyć systemowych ustawień uruchamiania.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Nie można odświeżyć ustawień. Spróbuj ponownie.';

  @override
  String get settingsRuntimeAppAlreadyExcluded =>
      'Ta aplikacja jest już wykluczona.';

  @override
  String get settingsRuntimeChangeFailed =>
      'Nie można wprowadzić żądanej zmiany.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Aby wznowić przechwytywanie schowka, przyznaj wszystkie wymagane uprawnienia.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'Przechwytywanie schowka zostało wstrzymane. Usługa przechwytywania w systemie Android nie potwierdziła zatrzymania.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'Aby włączyć powiadomienia o przechwyceniu, wymagane jest uprawnienie do powiadomień.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste nie może odczytać zapisanych ustawień. Aktywne są wartości ochronne; przejrzyj i zapisz preferencje, aby je naprawić.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'Nieczytelne ustawienia CopyPaste zostały odtworzone z wartościami ochronnymi. Przejrzyj i zapisz preferencje, aby je naprawić.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Identyfikator aplikacji nie może być pusty i może mieć maksymalnie 256 bajtów.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'Możesz wykluczyć maksymalnie 256 aplikacji.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Nie można zmienić automatycznego wklejania.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'Nie można zarejestrować tego skrótu.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'Nie można wstrzymać bieżącego skrótu.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'Nie można przywrócić skrótu.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'Nie można otworzyć Quick Paste.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Nie można poprosić o uprawnienie Dostępność.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Nie można odświeżyć stanu integracji z systemem Linux.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'Nie przyznano uprawnienia do wprowadzania ze schowka.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Nie można poprosić o uprawnienie do wprowadzania ze schowka.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Nie można otworzyć konfiguracji integracji z pulpitem.';

  @override
  String get settingsRuntimeTextHistoryFileType =>
      'Historia tekstowa CopyPaste';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'Zaszyfrowana kopia zapasowa CopyPaste';

  @override
  String get settingsRuntimeNotificationImage => 'Obraz';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Obraz · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'Plik';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste nie może przywrócić stanu instalacji aktualizacji.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Ta instalacja systemu Linux nie może aktualizować się automatycznie.';

  @override
  String get updateMacosPackageMissing =>
      'To wydanie nie zawiera podpisanego pakietu aktualizacji dla systemu macOS.';

  @override
  String get updateInstalledVersionInvalid =>
      'Zainstalowana wersja CopyPaste jest nieprawidłowa.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste nie może połączyć się z serwisem GitHub, aby sprawdzić dostępność aktualizacji.';

  @override
  String get updateCheckTimedOut =>
      'Przekroczono limit czasu sprawdzania aktualizacji.';

  @override
  String get updateCheckDeviceFailed =>
      'CopyPaste nie może sprawdzić dostępności aktualizacji na tym urządzeniu.';

  @override
  String get updateCheckFailed =>
      'CopyPaste nie może sprawdzić dostępności aktualizacji.';

  @override
  String get updateDownloadFailed => 'CopyPaste nie może pobrać aktualizacji.';

  @override
  String get updateDownloadTimedOut =>
      'Przekroczono limit czasu pobierania aktualizacji.';

  @override
  String get updateInstallFailed =>
      'CopyPaste nie może zainstalować aktualizacji.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Kontynuuj w systemowym instalatorze Android.';

  @override
  String get updateContinueInPackageManager =>
      'Kontynuuj w systemowym menedżerze pakietów.';

  @override
  String get updateInstallerStarting => 'Uruchamianie instalatora.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Zezwól CopyPaste na instalowanie aplikacji w ustawieniach systemu Android, a następnie kontynuuj.';

  @override
  String get updatePermissionRequiredLinux =>
      'Uwierzytelnij się w systemowym menedżerze pakietów, aby zainstalować tę aktualizację.';

  @override
  String get updatePermissionRequired =>
      'Zezwól CopyPaste na zainstalowanie tej aktualizacji, a następnie kontynuuj.';

  @override
  String get updateRestartRequired =>
      'Aktualizacja została zainstalowana. Uruchom ponownie CopyPaste, aby z niej korzystać.';

  @override
  String get updateRestarting => 'Ponowne uruchamianie CopyPaste.';

  @override
  String get updateRestartFailed =>
      'Nie można ponownie uruchomić CopyPaste. Spróbuj ponownie.';

  @override
  String get updateReleasePageOpenFailed =>
      'CopyPaste nie może otworzyć strony wydania.';

  @override
  String get updatePackageInvalid =>
      'Pobrana aktualizacja nie jest prawidłowym pakietem CopyPaste.';

  @override
  String get updateSignatureInvalid =>
      'Pobrana aktualizacja ma nieprawidłowy podpis.';

  @override
  String get updateDowngradeRefused =>
      'Pobrany pakiet nie jest nowszy niż ta wersja.';

  @override
  String get updateHomebrewRequired =>
      'Zainstaluj CopyPaste za pomocą Homebrew, aby aktualizować aplikację tutaj.';

  @override
  String get updateInstallationCancelled =>
      'Instalacja aktualizacji została anulowana.';

  @override
  String get updateInstallationBlocked =>
      'Ochrona urządzenia (Play Protect, Auto Blocker) zablokowała tę aktualizację. Zezwól na nią tam, a następnie sprawdź ponownie.';

  @override
  String get updateInstallationConflict =>
      'Aktualizacja powoduje konflikt z zainstalowaną aplikacją.';

  @override
  String get updateInstallationIncompatible =>
      'Aktualizacja nie jest zgodna z tym urządzeniem.';

  @override
  String get updateInstallationStorage =>
      'Zwolnij miejsce na urządzeniu, aby zainstalować aktualizację.';

  @override
  String get updateInstallationInterrupted =>
      'Instalacja aktualizacji została przerwana. Spróbuj ponownie.';

  @override
  String get updateInstallationBusy => 'Instalacja aktualizacji już trwa.';

  @override
  String get updatePackageDetailsInvalid =>
      'Informacje o pakiecie aktualizacji są nieprawidłowe.';

  @override
  String get updateIntegrityCheckFailed =>
      'Pobrana aktualizacja nie przeszła weryfikacji integralności.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste nie może uruchomić instalatora systemowego.';

  @override
  String get updateInstallLocationTranslocated =>
      'Przenieś CopyPaste do folderu Aplikacje i otwórz stamtąd, aby zaktualizować.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste nie może zastąpić samego siebie w bieżącym folderze. Zaktualizuj aplikację ze strony wydania.';

  @override
  String get updateReplacementBlocked =>
      'macOS zablokował zastąpienie CopyPaste. Zezwól na to w ustawieniach Zarządzanie aplikacjami lub użyj strony wydania.';

  @override
  String get updatePackageNotDownloadable =>
      'Ta platforma nie korzysta z pobieranego pakietu aktualizacji.';

  @override
  String get updatePackageSizeInvalid =>
      'Rozmiar pakietu aktualizacji jest nieprawidłowy.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'Pobrana aktualizacja ma nieprawidłowy podpis aktualizatora.';

  @override
  String get updatePackageUrlUntrusted =>
      'Adres URL pakietu aktualizacji nie jest zaufany.';

  @override
  String get updateDownloadLeftGitHub =>
      'Pobieranie aktualizacji zostało przekierowane poza serwis GitHub.';

  @override
  String get updateDownloadTooManyRedirects =>
      'Pobieranie aktualizacji wymagało zbyt wielu przekierowań.';

  @override
  String get updatePackageTooLarge => 'Pakiet aktualizacji jest za duży.';

  @override
  String get updateSignatureSizeInvalid =>
      'Rozmiar podpisu aktualizatora jest nieprawidłowy.';

  @override
  String get updateMetadataInvalid =>
      'GitHub zwrócił nieprawidłowe metadane aktualizacji.';

  @override
  String get updateMetadataTooLarge => 'Metadane aktualizacji są za duże.';

  @override
  String get quickPasteActionFailedTitle => 'Działanie nie powiodło się';

  @override
  String get quickPasteOpenAppSemantics => 'Otwórz CopyPaste';

  @override
  String get quickPasteToggleClipDetails => 'Pokaż lub ukryj szczegóły wpisu';

  @override
  String get quickPasteSearchPlaceholder => 'Wpisz, aby wyszukać…';

  @override
  String get quickPasteHistoryUnavailable => 'Historia jest niedostępna';

  @override
  String get quickPasteNoClipsFound => 'Nie znaleziono wpisów';

  @override
  String get quickPasteLoadMore => 'Wczytaj więcej';

  @override
  String get quickPasteTransferFailed => 'Nie powiodło się';

  @override
  String get quickPasteClipUnavailable => 'Wpis niedostępny';

  @override
  String get quickPasteFileFallbackName => 'Plik';

  @override
  String get quickPastePathUnavailable => 'Ścieżka niedostępna';

  @override
  String get quickPasteSettings => 'Ustawienia…';

  @override
  String get quickPasteAbout => 'Informacje';

  @override
  String get quickPasteQuit => 'Zakończ';

  @override
  String get quickPasteClearHistory => 'Wyczyść historię…';

  @override
  String get quickPasteClearHistoryTitle => 'Wyczyścić historię?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Przypięte wpisy pozostaną w historii.';

  @override
  String get quickPasteClearHistoryFailed => 'Nie można usunąć historii.';

  @override
  String get quickPasteAboutDescription =>
      'Prywatna historia schowka w systemach macOS, Android, Windows i Linux.';

  @override
  String get transfersTitle => 'Transfery';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count aktywnego transferu',
      many: '$count aktywnych transferów',
      few: '$count aktywne transfery',
      one: '$count aktywny transfer',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count aktywnego',
      many: '$count aktywnych',
      few: '$count aktywne',
      one: '$count aktywny',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Najnowsze';

  @override
  String get transfersShowEarlier => 'Wcześniejsze';

  @override
  String get transfersClearCompletedHistory => 'Wyczyść historię';

  @override
  String get transfersEmptyTitle => 'Brak transferów';

  @override
  String get transfersEmptyMessage =>
      'Tutaj pojawią się importowane pliki, pobierania i transfery wychodzące.';

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
  String get transfersRemoveFromHistory => 'Usuń z historii';

  @override
  String get transfersCancelDownload => 'Anuluj pobieranie';

  @override
  String get transfersShowFile => 'Pokaż plik';

  @override
  String get transfersHiddenFile => 'Ukryty plik';

  @override
  String get transfersPhaseSending => 'Wysyłanie';

  @override
  String get transfersPhaseDownloading => 'Pobieranie';

  @override
  String get transfersPhaseAdding => 'Dodawanie';

  @override
  String get transfersPhaseAdded => 'Dodano';

  @override
  String get transfersPhaseSent => 'Wysłano';

  @override
  String get transfersPhaseCompleted => 'Ukończono';

  @override
  String get transfersPhaseCancelled => 'Anulowano';

  @override
  String get transfersPhaseInterrupted => 'Przerwano';

  @override
  String get transfersPhaseFailed => 'Nie powiodło się';

  @override
  String get transfersUpdatesUnavailable =>
      'Bieżący stan transferów jest niedostępny. Otwórz ponownie, aby odświeżyć.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Nie można wczytać bieżącego stanu transferów.';

  @override
  String get transfersRetryFailed => 'Nie można ponowić transferu tego pliku.';

  @override
  String get transfersClearHistoryFailed =>
      'Nie można wyczyścić historii transferów.';

  @override
  String get transfersRemoveRecordFailed =>
      'Nie można usunąć tej pozycji historii.';

  @override
  String get transfersCancelFailed =>
      'Nie można anulować transferu. Może on nadal trwać.';

  @override
  String get transfersSavedFileUnavailable =>
      'Zapisany plik jest niedostępny. Mógł zostać przeniesiony lub usunięty.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count nieprzeczytanego powiadomienia',
      many: '$count nieprzeczytanych powiadomień',
      few: '$count nieprzeczytane powiadomienia',
      one: '$count nieprzeczytane powiadomienie',
      zero: 'Powiadomienia',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Powiadomienia';

  @override
  String get notificationsClearRead => 'Wyczyść przeczytane powiadomienia';

  @override
  String get notificationsEmptyTitle => 'Brak powiadomień';

  @override
  String get notificationsEmptyMessage =>
      'Tutaj pojawią się prośby o połączenie urządzeń i ich wyniki.';

  @override
  String get notificationsSystemAlertTitle => 'Powiadomienia systemowe';

  @override
  String get notificationsReview => 'Przejrzyj';

  @override
  String get notificationsConnectionRequest => 'Prośba o połączenie';

  @override
  String get notificationsConfirmConnection => 'Potwierdź połączenie';

  @override
  String get notificationsDeviceConnected => 'Urządzenie połączone';

  @override
  String get notificationsConnectionRejected => 'Połączenie odrzucone';

  @override
  String get notificationsConnectionCancelled => 'Połączenie anulowane';

  @override
  String get notificationsConnectionRequestExpired =>
      'Prośba o połączenie wygasła';

  @override
  String get notificationsConnectionFailed => 'Nie udało się połączyć';

  @override
  String get notificationsNearbyDevice => 'Urządzenie w pobliżu';

  @override
  String get notificationsPermissionDenied =>
      'Zezwól na powiadomienia w ustawieniach systemu, aby otrzymywać prośby w tle.';

  @override
  String get notificationsSystemUnavailable =>
      'Powiadomienia systemowe są niedostępne. Prośby o połączenie pozostaną tutaj.';

  @override
  String get platformCaptureStateUnavailable => 'Niedostępne';

  @override
  String get platformCaptureStatePaused => 'Wstrzymane';

  @override
  String get platformCaptureStateRunning => 'Przechwytywanie';

  @override
  String get platformCaptureStateStopped => 'Nieuruchomione';

  @override
  String get platformCaptureActionUnavailable => 'Przechwytywanie niedostępne';

  @override
  String get platformCaptureActionResume => 'Wznów przechwytywanie';

  @override
  String get platformCaptureActionPause => 'Wstrzymaj przechwytywanie';

  @override
  String get platformCaptureActionResumeStopped =>
      'Wznów przechwytywanie (nieuruchomione)';

  @override
  String get platformTraySettings => 'Ustawienia';

  @override
  String get platformTrayQuit => 'Zakończ';

  @override
  String get platformWindowControlsUnavailable =>
      'Sterowanie oknami jest niedostępne. Okno CopyPaste pozostanie widoczne.';

  @override
  String get platformTrayUnavailable =>
      'Zasobnik systemowy jest niedostępny. Pozostaw aplikację CopyPaste otwartą, a następnie ponów próbę lub zakończ ją.';

  @override
  String get platformQuitFailed => 'Nie można zakończyć CopyPaste.';

  @override
  String get platformVisibilityChangeFailed =>
      'Nie można zmienić widoczności aplikacji. Spróbuj ponownie.';

  @override
  String get platformCaptureNotificationTitle => 'Zapisano schowek';

  @override
  String get platformCaptureNotificationBody =>
      'Do CopyPaste dodano nowy element.';

  @override
  String get platformPairingNotificationTitle => 'Prośba o połączenie';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName chce się połączyć. Otwórz CopyPaste, aby porównać kod i potwierdzić.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Prośby o połączenie urządzeń';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Prośby o połączenie z urządzeniami w pobliżu';

  @override
  String get platformNotificationDefaultAction => 'Otwórz';

  @override
  String get platformFilesSelectionFailed => 'Nie można wybrać plików.';

  @override
  String get platformFilesOnlyRegular => 'Można importować tylko zwykłe pliki.';

  @override
  String get platformModulesUnavailable =>
      'Moduły są niedostępne na tym urządzeniu.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'Nie można skonfigurować dostępu do SMS-ów.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'Nie można zweryfikować dostępu do SMS-ów.';

  @override
  String get platformUpdatesUnavailableHere =>
      'Aktualizacje aplikacji są tutaj niedostępne.';

  @override
  String get platformUpdateVersionUnavailable =>
      'Zainstalowana wersja jest niedostępna.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'Aktualizacje aplikacji są niedostępne w tej instalacji.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Ta instalacja systemu Linux nie może aktualizować się automatycznie.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Ta aktualizacja dla systemu Linux nie pasuje do bieżącej architektury.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Ta instalacja systemu macOS nie może aktualizować się automatycznie.';

  @override
  String get platformUpdateStartFailed =>
      'CopyPaste nie może rozpocząć aktualizacji.';

  @override
  String get platformGlobalShortcutDescription => 'Otwórz Quick Paste';

  @override
  String get settingsSectionGeneral => 'Ogólne';

  @override
  String get settingsSectionAppearance => 'Wygląd';

  @override
  String get settingsThemeTitle => 'Motyw';

  @override
  String get settingsThemeDescription => 'Jasny, ciemny lub zgodny z systemem.';

  @override
  String get settingsThemeKeywords => 'tryb ciemny tryb jasny schemat kolorów';

  @override
  String get settingsThemeSystem => 'Systemowy';

  @override
  String get settingsThemeLight => 'Jasny';

  @override
  String get settingsThemeDark => 'Ciemny';

  @override
  String get settingsAccentTitle => 'Kolor akcentu';

  @override
  String get settingsAccentDescription =>
      'Wyróżnia zaznaczenie i bieżące miejsce.';

  @override
  String get settingsAccentKeywords => 'kolor wyróżnienia odcień';

  @override
  String get settingsAccentSystem => 'Systemowy';

  @override
  String get settingsAccentStandard => 'Domyślny';

  @override
  String get settingsAccentPurple => 'Fioletowy';

  @override
  String get settingsAccentPink => 'Różowy';

  @override
  String get settingsAccentRed => 'Czerwony';

  @override
  String get settingsAccentOrange => 'Pomarańczowy';

  @override
  String get settingsAccentYellow => 'Żółty';

  @override
  String get settingsAccentGreen => 'Zielony';

  @override
  String get settingsAccentTeal => 'Morski';

  @override
  String get settingsAccentGraphite => 'Grafitowy';

  @override
  String get settingsSectionClipboard => 'Schowek';

  @override
  String get settingsSectionPrivacy => 'Prywatność';

  @override
  String get settingsSectionQuickPaste => 'Quick Paste';

  @override
  String get settingsSectionSync => 'Synchronizacja';

  @override
  String get settingsSectionNotifications => 'Powiadomienia';

  @override
  String get settingsSectionData => 'Dane';

  @override
  String get settingsSectionModules => 'Moduły';

  @override
  String get settingsSectionRules => 'Reguły treści';

  @override
  String get settingsSectionRulesSummary =>
      'Automatyczne tagowanie, pomijanie, ochrona i wygasanie elementów.';

  @override
  String get settingsSectionQuickAccess => 'Szybki dostęp';

  @override
  String get settingsSectionTransforms => 'Przekształcenia tekstu';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Zbieraj kilka kopii i wklejaj je po kolei.';

  @override
  String pasteQueuePasted(int position, int total) {
    return 'Wklejono $position z $total';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'Element $position z $total jest w schowku';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Wklejono $count elementu',
      many: 'Wklejono $count elementów',
      few: 'Wklejono $count elementy',
      one: 'Wklejono $count element',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementu jest w schowku',
      many: '$count elementów jest w schowku',
      few: '$count elementy są w schowku',
      one: '$count element jest w schowku',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Kolejka zakończona';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Dodano, $total w kolejce',
      many: 'Dodano, $total w kolejce',
      few: 'Dodano, $total w kolejce',
      one: 'Dodano, $total w kolejce',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'Kolejka pusta, nic nie wklejono';

  @override
  String get pasteQueueMixed =>
      'Wklej wszystko działa tylko z kolejką samego tekstu lub samych plików';

  @override
  String get pasteQueueSeveralFiles => 'Wklejaj pliki pojedynczo.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementu nie jest już dostępne',
      many: '$count elementów nie jest już dostępnych',
      few: '$count elementy nie są już dostępne',
      one: '$count element nie jest już dostępny',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => 'Wklej następny tylko kopiuje';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Skopiowano element. Zezwól na dostęp „Dostępność”, aby wklejać automatycznie.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Nie udało się wkleić automatycznie. Skopiowano element.';

  @override
  String get pasteQueueTargetElevated =>
      'Nie można wkleić do aplikacji administratora. Skopiowano element.';

  @override
  String get pasteQueueNoTarget =>
      'Brak aplikacji do wklejenia. Skopiowano element.';

  @override
  String get pasteQueueCollectingStarted => 'Zbieranie kopii';

  @override
  String get pasteQueueCollectingStopped => 'Zatrzymano zbieranie';

  @override
  String get pasteQueueFailed => 'Nie udało się wykonać działania Paste Queue';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count oczekującego',
      many: '$count oczekujących',
      few: '$count oczekujące',
      one: '$count oczekujący',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count oczekującego · Zbieranie',
      many: '$count oczekujących · Zbieranie',
      few: '$count oczekujące · Zbieranie',
      one: '$count oczekujący · Zbieranie',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Zbieranie';

  @override
  String get pasteQueueFinishedSummary => 'Zakończona';

  @override
  String pasteQueueFull(int max) {
    return 'Kolejka pełna ($max)';
  }

  @override
  String get pasteQueueFullMessage => 'Nowe kopie nie są już dodawane.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue jest pusta';

  @override
  String get pasteQueueEmptyMessage =>
      'Zbieraj kopie lub dodawaj elementy z historii.';

  @override
  String get pasteQueueCollectingEmptyTitle => 'Kopiuj elementy, aby je dodać';

  @override
  String get pasteQueueStartCollecting => 'Zacznij zbierać';

  @override
  String get pasteQueueStopCollecting => 'Zatrzymaj zbieranie';

  @override
  String get pasteQueueLoadFailed => 'Nie udało się wczytać kolejki';

  @override
  String get pasteQueuePasteNext => 'Wklej następny';

  @override
  String get pasteQueueCopyNext => 'Kopiuj następny';

  @override
  String get pasteQueuePasteAll => 'Wklej wszystko';

  @override
  String get pasteQueueCopyAll => 'Kopiuj wszystko';

  @override
  String get pasteQueueReverse => 'Odwróć kolejność';

  @override
  String get pasteQueueStepBack => 'Cofnij o krok';

  @override
  String get pasteQueueSkip => 'Pomiń';

  @override
  String get pasteQueueRemovePasted => 'Usuń wklejone';

  @override
  String get pasteQueueCleared => 'Wyczyszczono kolejkę';

  @override
  String get pasteQueueUndo => 'Cofnij';

  @override
  String get pasteQueueStartFresh => 'Zacznij od nowa';

  @override
  String get pasteQueueAppend => 'Dołącz';

  @override
  String get pasteQueueRestartTitle => 'Rozpocząć nowe zbieranie?';

  @override
  String get pasteQueueRestartMessage =>
      '„Zacznij od nowa” usuwa wklejone elementy. „Dołącz” je zachowuje.';

  @override
  String pasteQueueStale(String time) {
    return 'Kolejka z $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Wklejony';

  @override
  String get pasteQueueStatusSkipped => 'Pominięty';

  @override
  String get pasteQueueStatusUnavailable => 'Już niedostępny';

  @override
  String get pasteQueueEdited => 'Edytowany';

  @override
  String get pasteQueueSecret => 'Tajne';

  @override
  String get pasteQueueKindImage => 'Obraz';

  @override
  String get pasteQueueKindFiles => 'Pliki';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Wygenerowano o $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Element $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'Przeniesiono na pozycję $position';
  }

  @override
  String get pasteQueueMoveUp => 'Przenieś w górę';

  @override
  String get pasteQueueMoveDown => 'Przenieś w dół';

  @override
  String get pasteQueueMoveToTop => 'Przenieś na początek';

  @override
  String get pasteQueueMoveToBottom => 'Przenieś na koniec';

  @override
  String get pasteQueueReorder => 'Przeciągnij, aby zmienić kolejność';

  @override
  String get pasteQueueMoreActions => 'Więcej działań';

  @override
  String get pasteQueuePasteNow => 'Wklej teraz';

  @override
  String get pasteQueueEdit => 'Edytuj tekst';

  @override
  String get pasteQueueUseOriginal => 'Użyj oryginalnego tekstu';

  @override
  String get pasteQueueRemove => 'Usuń z kolejki';

  @override
  String get pasteQueueEditTitle => 'Edytuj tekst w kolejce';

  @override
  String get pasteQueueEditMessage => 'Zmiany dotyczą tylko tej kolejki.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Ustaw skróty';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Zbieraj i wklejaj bez otwierania CopyPaste.';

  @override
  String get pasteQueueShortcutToggleCollecting =>
      'Zacznij lub zatrzymaj zbieranie';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Wklej następny jako zwykły tekst';

  @override
  String get pasteQueueShortcutDescription =>
      'Globalny skrót działający w każdej aplikacji.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Inny skrót CopyPaste używa tej kombinacji. Zachowano poprzedni.';

  @override
  String get pasteQueueShortcutInUse => 'Inna aplikacja używa tej kombinacji.';

  @override
  String get pasteQueueShortcutRejected =>
      'System odrzucił tę kombinację. Zachowano poprzednią.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt wpisuje znaki w wielu układach. Wybierz inną.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Skróty globalne są niedostępne w tej sesji.';

  @override
  String get pasteQueueShortcutFailed => 'Nie udało się ustawić tego skrótu.';

  @override
  String get pasteQueueEnableTitle => 'Włącz Paste Queue';

  @override
  String get pasteQueueEnableDescription =>
      'Po wyłączeniu skróty i zbieranie przestają działać.';

  @override
  String get pasteQueuePasteNextStyleTitle => 'Wklej następny wykonuje';

  @override
  String get pasteQueuePasteIntoApp => 'Wklej do aplikacji na pierwszym planie';

  @override
  String get pasteQueueClipboardOnly => 'Tylko do schowka';

  @override
  String get pasteQueueInsertTitle => 'Nowe elementy trafiają';

  @override
  String get pasteQueueInsertEnd =>
      'Na koniec (pierwszy wchodzi, pierwszy wychodzi)';

  @override
  String get pasteQueueInsertStart =>
      'Na początek (ostatni wchodzi, pierwszy wychodzi)';

  @override
  String get pasteQueueEndBehaviorTitle => 'Po ostatnim elemencie';

  @override
  String get pasteQueueEndStop => 'Zatrzymaj';

  @override
  String get pasteQueueEndLoop => 'Zacznij od początku';

  @override
  String get pasteQueueEndKeepLast => 'Wklejaj dalej ostatni';

  @override
  String get pasteQueueSeparatorTitle => 'Separator dla Wklej wszystko';

  @override
  String get pasteQueueSeparatorDescription =>
      'Wstawiany między elementy podczas wklejania wszystkich naraz.';

  @override
  String get pasteQueueSeparatorNewLine => 'Nowy wiersz';

  @override
  String get pasteQueueSeparatorBlankLine => 'Pusty wiersz';

  @override
  String get pasteQueueSeparatorTab => 'Tabulator';

  @override
  String get pasteQueueSeparatorSpace => 'Spacja';

  @override
  String get pasteQueueSeparatorComma => 'Przecinek';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Przecinek i spacja';

  @override
  String get pasteQueueSeparatorNone => 'Brak';

  @override
  String get pasteQueueSeparatorCustom => 'Własny';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'Do 32 znaków; dozwolone \\n i \\t';

  @override
  String get pasteQueuePlainTextTitle => 'Zawsze wklejaj jako zwykły tekst';

  @override
  String get pasteQueueIdleStopTitle => 'Zatrzymaj zbieranie po';

  @override
  String get pasteQueueIdleStopDescription => 'Czas bez nowej kopii.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes min';
  }

  @override
  String get pasteQueueIdleNever => 'Nigdy';

  @override
  String get pasteQueueMaxEntriesTitle => 'Limit rozmiaru kolejki';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Gdy kolejka jest pełna, nowe kopie nie są dodawane.';

  @override
  String get pasteQueueRestoreTitle =>
      'Przywróć poprzedni schowek po zakończeniu';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue kolejka zbieranie kolejność skrót separator';

  @override
  String get settingsSectionAbout => 'Informacje';

  @override
  String get settingsModulesDescription => 'Opcjonalne funkcje CopyPaste.';

  @override
  String get settingsQuickPasteDescription =>
      'Otwieraj historię schowka z dowolnego miejsca bez przełączania okien.';

  @override
  String get settingsUnavailableTitle => 'Ustawienia są niedostępne';

  @override
  String get settingsNoResultsTitle => 'Nie znaleziono ustawień';

  @override
  String get settingsNoResultsMessage => 'Spróbuj wyszukać coś innego.';

  @override
  String get settingsAttentionTitle => 'Ustawienia wymagają uwagi';

  @override
  String get settingsSearchPlaceholder => 'Szukaj w ustawieniach';

  @override
  String get settingsBackToSettings => 'Wróć do ustawień';

  @override
  String get settingsStartOnBootTitle => 'Uruchamiaj przy starcie urządzenia';

  @override
  String get settingsStartAtLoginTitle => 'Uruchamiaj przy logowaniu';

  @override
  String get settingsStartupSearchDescription =>
      'Uruchamiaj CopyPaste automatycznie.';

  @override
  String get settingsStartupKeywords =>
      'uruchamianie logowanie rozruch start automatycznie autostart startup login boot launch automatically';

  @override
  String get settingsStartupWindowsApproval =>
      'Wyłączono w Menedżerze zadań. Włącz CopyPaste w obszarze Ustawienia → Aplikacje → Uruchamianie.';

  @override
  String get settingsStartupMacApproval =>
      'Zezwól na CopyPaste w obszarze Ustawienia systemowe → Ogólne → Rzeczy otwierane podczas logowania.';

  @override
  String get settingsStartupUnavailable =>
      'Automatyczne uruchamianie jest niedostępne w tej instalacji.';

  @override
  String get settingsStartupAndroidDescription =>
      'Wznawiaj usługi w tle po ponownym uruchomieniu.';

  @override
  String get settingsShowInDock => 'Pokazuj w Docku';

  @override
  String get settingsShowInTaskbar => 'Pokazuj na pasku zadań';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Pokazuj ikonę aplikacji do przełączania okien.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'dock pasek zadań ikona aplikacja widoczność dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Pokazuj na pasku menu';

  @override
  String get settingsShowInSystemTray => 'Pokazuj w zasobniku systemowym';

  @override
  String get settingsTrayVisibilityDescription =>
      'Pokazuj szybki dostęp do CopyPaste i sterowania przechwytywaniem.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'zasobnik pasek menu stan ikona widoczność tray menu bar menubar status icon visibility';

  @override
  String get settingsLightweightModeTitle => 'Tryb lekki';

  @override
  String get settingsLightweightModeDescription =>
      'Zamknięcie okna zamyka aplikację; przechwytywanie i synchronizacja działają dalej. Zasobnik, Quick Paste i skróty wrócą po jej otwarciu.';

  @override
  String get settingsLightweightModeKeywords =>
      'lekki zamknij zakończ tło pamięć lightweight close quit background memory';

  @override
  String get settingsModulesSearchDescription =>
      'Instaluj, aktualizuj i usuwaj opcjonalne moduły.';

  @override
  String get settingsModulesKeywords =>
      'funkcje polecenia rozszerzenia features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Przechwytywanie w tle (Android)';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Pełne lub ograniczone przechwytywanie z konfiguracją przez Shizuku lub ADB.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'tło uprawnienia konfiguracja background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Tryb pełny lub ograniczony i jednorazowa konfiguracja Shizuku lub ADB.';

  @override
  String get settingsOpenSetup => 'Otwórz konfigurację';

  @override
  String get settingsSaveScreenshotsTitle => 'Zapisuj zrzuty ekranu';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Automatycznie zapisuj nowe zrzuty ekranu z Androida w Historii.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'obrazy zdjęcia przechwytywanie uprawnienia images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Automatycznie dodawaj nowe zrzuty ekranu do Historii.';

  @override
  String get settingsScreenshotAccessTitle => 'Dostęp do zrzutów ekranu';

  @override
  String get settingsScreenshotAccessDescription =>
      'Potrzebne do zapisywania zrzutów ekranu w tle.';

  @override
  String get settingsAllowAccess => 'Zezwól na dostęp';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'Przechwytywanie zrzutów ekranu nie działa';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'Nowe zrzuty ekranu nie są zapisywane. Wznów przechwytywanie, aby ponownie uruchomić usługę.';

  @override
  String get settingsResumeCapture => 'Wznów przechwytywanie';

  @override
  String get settingsClipboardCaptureTitle => 'Przechwytywanie schowka';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Wstrzymuj lub wznawiaj przechwytywanie schowka.';

  @override
  String get settingsCaptureStateUnavailable => 'Stan niedostępny';

  @override
  String get settingsCaptureStoppedTitle => 'Przechwytywanie nie działa';

  @override
  String get settingsCaptureStoppedMessage =>
      'CopyPaste nie odbiera obecnie zmian schowka na tym urządzeniu.';

  @override
  String get settingsRetentionTitle => 'Przechowywanie';

  @override
  String get settingsRetentionDescription =>
      'Automatycznie usuwaj stare nieprzypięte wpisy.';

  @override
  String get settingsRetentionForever => 'Zachowuj na zawsze';

  @override
  String get settingsStorageQuotaTitle => 'Limit miejsca';

  @override
  String get settingsStorageQuotaDescription =>
      'Maksymalne miejsce lokalne zajmowane przez nieprzypiętą historię.';

  @override
  String get settingsStorageQuotaKeywords =>
      'dysk miejsce limit disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Maksymalny rozmiar pliku';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Limit dodawania i pobierania plików.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'import pobieranie transfer rozmiar limit import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Największy plik, który możesz dodać lub pobrać.';

  @override
  String get settingsFileStorageTitle => 'Przechowywanie plików';

  @override
  String get settingsFileStorageDescription =>
      'Link działa tylko, gdy istnieje oryginał; kopia zachowuje plik.';

  @override
  String get settingsFileStorageKeywords =>
      'pliki kopia link oryginał zachować przechowywać offline files copy link original keep store offline';

  @override
  String get settingsFileStorageLink => 'Link do oryginału';

  @override
  String get settingsFileStorageCopy => 'Zachowaj kopię';

  @override
  String get settingsSaveFolderTitle => 'Zapisuj pliki w';

  @override
  String get settingsSaveFolderAsk => 'Pytaj za każdym razem';

  @override
  String get settingsSaveFolderAskAction => 'Pytaj za każdym razem';

  @override
  String get settingsSaveFolderChooseAction => 'Wybierz folder';

  @override
  String get settingsSaveFolderSearchDescription => 'Folder na pobrane pliki.';

  @override
  String get settingsSaveFolderKeywords =>
      'pobieranie folder cel katalog zapisz lokalizacja download folder destination directory save location';

  @override
  String get settingsAutoDownloadTitle => 'Pobieraj odebrane pliki';

  @override
  String get settingsAutoDownloadDescription =>
      'Zapisuje w folderze pliki z innych urządzeń, gdy są online.';

  @override
  String get settingsAutoDownloadNeedsFolder =>
      'Najpierw wybierz folder do zapisywania plików.';

  @override
  String get settingsAutoDownloadKeywords =>
      'automatycznie pobierać odbierać synchronizacja pliki automatic download receive sync files';

  @override
  String get settingsHistoryFilesTitle => 'Pliki historii';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Eksportuj historię, twórz kopie zapasowe lub przywracaj kopię zapasową.';

  @override
  String get settingsHistoryFilesKeywords =>
      'tekst zaszyfrowana kopia zapasowa dane text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Eksportuj historię tekstową';

  @override
  String get settingsExportTextDescription =>
      'Zapisz wpisy tekstowe jako plik JSON.';

  @override
  String get settingsExportAction => 'Eksportuj';

  @override
  String get settingsBackupTitle => 'Zaszyfrowana kopia zapasowa';

  @override
  String get settingsBackupDescription =>
      'Zapisz pełną historię lokalną tego urządzenia.';

  @override
  String get settingsCreateBackupAction => 'Utwórz kopię';

  @override
  String get settingsRestoreTitle => 'Przywróć kopię zapasową';

  @override
  String get settingsRestoreDescription =>
      'Zastąp historię lokalną zaszyfrowaną kopią zapasową.';

  @override
  String get settingsRestoreAction => 'Przywróć';

  @override
  String get settingsRestoreDialogTitle => 'Zastąpić historię lokalną?';

  @override
  String get settingsRestoreDialogMessage =>
      'Kopia zapasowa zastąpi historię na tym urządzeniu. Ustawienia zostaną zachowane.';

  @override
  String get settingsRestoreFailed => 'Nie można przywrócić kopii zapasowej.';

  @override
  String get settingsSyncSwitchTitle => 'Synchronizacja';

  @override
  String get settingsSyncSwitchDescription =>
      'Synchronizuj historię ze sparowanymi urządzeniami.';

  @override
  String get settingsSyncSearchDescription =>
      'Synchronizacja sparowanych urządzeń.';

  @override
  String get settingsInstantClipboardTitle => 'Natychmiastowy schowek';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Automatycznie kopiuj nowe wpisy z innych urządzeń.';

  @override
  String get settingsInstantClipboardKeywords =>
      'automatyczne kopiuj wklej synchronizacja odebrane automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Kopiuj nowe wpisy z innych urządzeń do tego schowka.';

  @override
  String get settingsLanVisibilityTitle => 'Widoczność w sieci LAN';

  @override
  String get settingsLanVisibilityDescription =>
      'Zezwalaj urządzeniom w pobliżu na wykrywanie tego urządzenia.';

  @override
  String get settingsLanVisibilityKeywords =>
      'sieć lokalna wykrywanie local network discovery';

  @override
  String get settingsRelayTitle => 'Serwer przekaźnikowy';

  @override
  String get settingsRelayDescription =>
      'Synchronizuj sparowane urządzenia między sieciami przez własny przekaźnik. Pozostaw puste, aby wyłączyć.';

  @override
  String get settingsRelayKeywords =>
      'przekaźnik serwer zdalny internet sieć relay server remote internet network websocket';

  @override
  String get settingsRelayPlaceholder => 'wss://relay.example.com';

  @override
  String get settingsRelayConnecting => 'Łączenie z przekaźnikiem…';

  @override
  String get settingsRelayConnected => 'Połączono z przekaźnikiem.';

  @override
  String get settingsRelayUnauthorized => 'Przekaźnik odrzucił token dostępu.';

  @override
  String get settingsRelayUnreachable =>
      'Nie można połączyć się z przekaźnikiem.';

  @override
  String get settingsPortMappingTitle => 'Przekierowanie portu na routerze';

  @override
  String get settingsPortMappingDescription =>
      'Pozwól synchronizacji bezpośredniej prosić router (UPnP) o otwarcie portu na kilka minut. Pomaga, gdy obie sieci blokują połączenia bezpośrednie.';

  @override
  String get settingsPortMappingKeywords =>
      'router port przekierowanie bezpośrednie router port mapping upnp nat forwarding direct';

  @override
  String get settingsExcludedAppsTitle => 'Wykluczone aplikacje';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Pomijaj automatyczne przechwytywanie z wykluczonych aplikacji.';

  @override
  String get settingsExcludedAppsKeywords =>
      'prywatność aplikacja identyfikatory privacy app identifiers';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Pomijaj automatyczne przechwytywanie z tych aplikacji.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Gdy wykluczono aplikację, automatyczne przechwytywanie jest wstrzymane.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'Wykluczenia aplikacji są obsługiwane w systemach macOS, Windows, Android i Linux.';

  @override
  String get settingsExcludedAppsAdd => 'Dodaj';

  @override
  String get settingsExcludedAppsEmpty => 'Brak wykluczonych aplikacji.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return 'Usuń $app';
  }

  @override
  String get applicationPickerSearch => 'Szukaj aplikacji';

  @override
  String get applicationPickerEmpty => 'Nie znaleziono aplikacji';

  @override
  String get settingsSkipSecretTitle => 'Pomijaj tajną zawartość schowka';

  @override
  String get settingsSkipSecretDescription =>
      'Nie zapisuj kopii oznaczonych przez aplikacje jako tajne.';

  @override
  String get settingsSkipSecretKeywords =>
      'poufne hasło wrażliwe ukryte spoiler secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle =>
      'Pomijaj tymczasową zawartość schowka';

  @override
  String get settingsSkipTransientDescription =>
      'Nie zapisuj kopii oznaczonych przez aplikacje jako tymczasowe.';

  @override
  String get settingsSkipTransientKeywords =>
      'przejściowe tymczasowe prywatność transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Blokuj zrzuty ekranu';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Uniemożliwiaj wykonywanie zrzutów i nagrań ekranu CopyPaste.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'Blokowanie zrzutów ekranu jest niedostępne na tej platformie.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'prywatność ekran zrzut ochrona parowanie qr bezpieczeństwo kod privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Powiadomienie po skopiowaniu';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Pokazuj powiadomienie po przechwyceniu w tle.';

  @override
  String get settingsNotificationPreviewTitle => 'Pokazuj zawartość schowka';

  @override
  String get settingsNotificationPreviewDescription =>
      'Dołączaj podgląd wpisu do powiadomień o skopiowaniu.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'powiadomienie podgląd tekst obraz prywatność notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Dźwięk po skopiowaniu';

  @override
  String get settingsSoundOnCopyDescription =>
      'Odtwarzaj systemowy sygnał po udanym przechwyceniu.';

  @override
  String get settingsVersionTitle => 'Wersja';

  @override
  String get settingsVersionUnavailable => 'Niedostępna';

  @override
  String get settingsUpdatesTitle => 'Aktualizacje aplikacji';

  @override
  String get settingsUpdatesSearchDescription =>
      'Sprawdzaj i instaluj aktualizacje CopyPaste.';

  @override
  String get settingsUpdatesKeywords =>
      'wersja github wydanie version github release';

  @override
  String get settingsUpdateChecking => 'Sprawdzanie dostępności aktualizacji.';

  @override
  String get settingsUpdateUpToDate => 'Masz najnowszą wersję CopyPaste.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'Wersja $version jest aktualna.';
  }

  @override
  String settingsUpdateVersion(String version) {
    return 'Wersja $version';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'Dostępna jest wersja $available. To urządzenie ma wersję $current.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Pobieranie wersji $version · $percent%.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Przygotowywanie aktualizacji zainstalowanych modułów.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Instalowanie wersji $version.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'Aktualizacje aplikacji wymagają uwagi.';

  @override
  String get settingsUpdateNowAction => 'Aktualizuj teraz';

  @override
  String get settingsUpdateOpenReleaseAction => 'Otwórz wydanie';

  @override
  String get settingsUpdateCheckingAction => 'Sprawdzanie';

  @override
  String get settingsUpdateDownloadingAction => 'Pobieranie';

  @override
  String get settingsUpdateInstallingAction => 'Instalowanie';

  @override
  String get settingsUpdateRestartAction => 'Uruchom ponownie CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => 'Sprawdź ponownie';

  @override
  String get settingsUpdateCheckAction => 'Sprawdź teraz';

  @override
  String get settingsUpdateNoneTitle => 'Brak dostępnych aktualizacji';

  @override
  String get settingsUpdateAvailableTitle => 'Dostępna aktualizacja';

  @override
  String get settingsUpdateUnavailableTitle => 'Aktualizacja niedostępna';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Ta instalacja nie może aktualizować się automatycznie.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Sprawdzanie aktualizacji nie powiodło się';

  @override
  String get settingsQuickPasteSetupTitle => 'Skonfiguruj Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Włącz podpisany komponent towarzyszący dla GNOME lub KDE i zezwól na sterowanie klawiaturą, aby wracać do poprzedniej aplikacji.';

  @override
  String get settingsQuickPasteDesktopIntegration => 'Integracja z pulpitem';

  @override
  String get settingsQuickPasteAllowKeyboard =>
      'Zezwól na sterowanie klawiaturą';

  @override
  String get settingsQuickPasteCheckAgain => 'Sprawdź ponownie';

  @override
  String get settingsQuickPasteUnavailableTitle =>
      'Funkcja Quick Paste jest niedostępna';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'To urządzenie nie obsługuje okna Quick Paste.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Otwieranie Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Skonfiguruj globalny skrót Quick Paste.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'klawiatura skrót klawiszowy keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'Działa, gdy CopyPaste pracuje w tle.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Zarejestrowany skrót: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Wklejaj automatycznie';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Wklejaj wybrany wpis do poprzedniej aplikacji.';

  @override
  String get settingsAutoPasteKeywords =>
      'dostępność automatyczne accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Wybranie wpisu powoduje powrót do poprzedniej aplikacji i wklejenie go.';

  @override
  String get settingsMaskPasswordFieldsTitle =>
      'Ukrywaj podglądy w polach hasła';

  @override
  String get settingsMaskPasswordFieldsDescription =>
      'Wpisy są zamaskowane, gdy aktywne jest pole hasła.';

  @override
  String get settingsQuickPasteQueueTabTitle => 'Karta Kolejka';

  @override
  String get settingsQuickPasteQueueTabDescription =>
      'Pokazuje Paste Queue w Quick Paste.';

  @override
  String get settingsAccessibilityRequiredTitle =>
      'Wymagane uprawnienie Dostępność';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Dopóki uprawnienie nie zostanie przyznane, wybranie wpisu kopiuje go bez wklejania.';

  @override
  String get settingsQuickPasteAttentionTitle => 'Quick Paste wymaga uwagi';

  @override
  String get settingsShortcutRecording => 'Naciśnij skrót…';

  @override
  String get settingsShortcutNotSet => 'Nie ustawiono';

  @override
  String get settingsShortcutReset => 'Przywróć domyślny skrót';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dnia',
      many: '$count dni',
      few: '$count dni',
      one: '$count dzień',
    );
    return '$_temp0';
  }

  @override
  String get devicesUnavailableTitle => 'Urządzenia są niedostępne';

  @override
  String get devicesNearbyTitle => 'Urządzenia w pobliżu';

  @override
  String get devicesScan => 'Skanuj';

  @override
  String get devicesScanInvalidTitle => 'Niewłaściwy kod QR';

  @override
  String get devicesScanning => 'Skanowanie…';

  @override
  String get devicesNoneNearby => 'Brak urządzeń w pobliżu';

  @override
  String get devicesThisDevice => 'To urządzenie';

  @override
  String get devicesTrusted => 'Zaufane';

  @override
  String get devicesConnect => 'Połącz';

  @override
  String devicesCardPing(String ping) {
    return 'Ping: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Ping:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'Zmień nazwę tego urządzenia';

  @override
  String get devicesNamePlaceholder => 'Nazwa urządzenia';

  @override
  String get devicesRenameFailed => 'Nie można zapisać nazwy urządzenia.';

  @override
  String get devicesActionFailedMessage =>
      'Nie można wykonać działania na urządzeniu.';

  @override
  String get devicesCloseDetails => 'Zamknij szczegóły urządzenia';

  @override
  String get devicesDetailType => 'Typ urządzenia';

  @override
  String get devicesDetailOperatingSystem => 'System operacyjny';

  @override
  String get devicesDetailModel => 'Model';

  @override
  String get devicesDetailAppVersion => 'Wersja CopyPaste';

  @override
  String get devicesDetailProtocolVersion => 'Wersja protokołu';

  @override
  String get devicesDetailStatus => 'Stan';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => 'Zmierzono';

  @override
  String get devicesDetailLastSeen => 'Ostatnio widziane';

  @override
  String get devicesDetailLanEndpoint => 'Adres w sieci LAN';

  @override
  String get devicesDetailDeviceId => 'Identyfikator urządzenia';

  @override
  String get devicesDetailPairingId => 'Identyfikator parowania';

  @override
  String get devicesDetailProfileProvenance => 'Pochodzenie profilu';

  @override
  String get devicesDetailProfileTrust => 'Zaufanie do profilu';

  @override
  String get devicesDetailProfileUpdated => 'Aktualizacja profilu';

  @override
  String get devicesValueUnavailable => 'Niedostępne';

  @override
  String get devicesRenameDevice => 'Zmień nazwę urządzenia';

  @override
  String get devicesSyncNow => 'Synchronizuj teraz';

  @override
  String get devicesSyncTitle => 'Synchronizacja';

  @override
  String get devicesSyncDescription =>
      'Wyłączenie zachowuje już zapisane wpisy.';

  @override
  String get devicesSyncThisDevice => 'Synchronizuj to urządzenie';

  @override
  String get devicesSyncPeer => 'Synchronizuj z tym urządzeniem';

  @override
  String get devicesSyncPause => 'Wstrzymaj automatyczną synchronizację';

  @override
  String get devicesSyncText => 'Tekst';

  @override
  String get devicesSyncImages => 'Obrazy';

  @override
  String get devicesSyncFiles => 'Pliki';

  @override
  String get devicesSyncAfterFirstSync =>
      'Ustawienia pojawią się po pierwszej synchronizacji z tym urządzeniem.';

  @override
  String get devicesLocal => 'Lokalne';

  @override
  String get devicesTrustAuthenticated => 'Uwierzytelnione';

  @override
  String get devicesTrustUnverified => 'Niezweryfikowane';

  @override
  String get devicesProvenanceSelfReported => 'Zgłoszone przez urządzenie';

  @override
  String get devicesProvenanceObserved => 'Zaobserwowane';

  @override
  String get devicesProvenanceMeasured => 'Zmierzone';

  @override
  String get devicesConnectionOnline => 'Online';

  @override
  String get devicesConnectionOffline => 'Offline';

  @override
  String get devicesConnectionVisible => 'Widoczne';

  @override
  String get devicesConnectionNotSeen => 'Nie wykryto';

  @override
  String get devicesConnectionUnknown => 'Nieznany';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => 'Komputer stacjonarny';

  @override
  String get devicesClassLaptop => 'Laptop';

  @override
  String get devicesClassPhone => 'Telefon';

  @override
  String get devicesClassTablet => 'Tablet';

  @override
  String get devicesClassUnknown => 'Urządzenie';

  @override
  String get devicesUnknownOs => 'Nieznany system';

  @override
  String get devicesAddDevice => 'Dodaj urządzenie';

  @override
  String get devicesScanQr => 'Skanuj kod QR';

  @override
  String get devicesEnterCode => 'Wpisz kod';

  @override
  String get devicesScannerUnavailableTitle => 'Skaner niedostępny';

  @override
  String get devicesActionFailedTitle => 'Działanie nie powiodło się';

  @override
  String get devicesRefreshFailedTitle => 'Nie można odświeżyć urządzeń';

  @override
  String get devicesNearbyPairingUnavailable =>
      'Parowanie z urządzeniami w pobliżu jest niedostępne.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'Parowanie z urządzeniami w pobliżu jest niedostępne. Użyj opcji Dodaj urządzenie, aby wyświetlić kod parowania.';

  @override
  String get pairingTitle => 'Parowanie';

  @override
  String get pairingPairDevice => 'Sparuj urządzenie';

  @override
  String get pairingScanQrCode => 'Skanuj kod QR';

  @override
  String get pairingEnterPairingCode => 'Wpisz kod parowania';

  @override
  String get pairingConnectNearbyDevice => 'Połącz urządzenie w pobliżu';

  @override
  String get pairingSendingRequest => 'Wysyłanie prośby o połączenie…';

  @override
  String get pairingCreatingInvitation => 'Tworzenie zaproszenia do parowania.';

  @override
  String get pairingCode => 'Kod parowania';

  @override
  String get pairingCodeHint => 'Wyświetlany na drugim urządzeniu.';

  @override
  String get pairingDeviceAddress => 'Adres urządzenia';

  @override
  String get pairingDeviceAddressHint =>
      'Użyj adresu host:port wyświetlonego na drugim urządzeniu.';

  @override
  String get pairingCheckDetailsTitle => 'Sprawdź dane parowania';

  @override
  String get pairingActionFailed => 'Parowanie nie powiodło się';

  @override
  String get pairingConnectingDevice => 'Łączenie urządzenia…';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'Tutaj pojawi się podgląd z aparatu.';

  @override
  String get pairingCodeIncomplete => 'Wpisz pełny 8-znakowy kod parowania.';

  @override
  String get pairingAddressMissing =>
      'Wpisz adres urządzenia w formacie host:port lub skonfiguruj serwer przekaźnikowy w ustawieniach.';

  @override
  String get pairingWaitingForOtherDevice => 'Oczekiwanie na drugie urządzenie';

  @override
  String get pairingInviteInstructions =>
      'Na drugim urządzeniu wybierz „Wpisz kod” lub „Skanuj kod QR”.';

  @override
  String get pairingAddress => 'Adres';

  @override
  String get pairingVerificationCode => 'Kod weryfikacyjny';

  @override
  String get pairingVerificationCodeHint =>
      'Potwierdź tylko wtedy, gdy ten kod jest taki sam na obu urządzeniach.';

  @override
  String get pairingReject => 'Odrzuć';

  @override
  String get pairingConfirm => 'Potwierdź';

  @override
  String get pairingAccept => 'Akceptuj';

  @override
  String get pairingJoinDevice => 'Dołącz urządzenie';

  @override
  String get pairingStartScanner => 'Uruchom skaner';

  @override
  String get pairingStateHandshaking => 'Weryfikowanie urządzenia';

  @override
  String get pairingStateAwaitingConfirmation =>
      'Potwierdź na obu urządzeniach';

  @override
  String get pairingStateConfirmed => 'Urządzenie sparowane';

  @override
  String get pairingStateRejected => 'Parowanie odrzucone';

  @override
  String get pairingStateCancelled => 'Parowanie anulowane';

  @override
  String get pairingStateTimedOut => 'Upłynął limit czasu parowania';

  @override
  String get pairingStateFailed => 'Parowanie nie powiodło się';

  @override
  String get pairingHandshakingDescription =>
      'Urządzenia nawiązują bezpieczne połączenie.';

  @override
  String get pairingConfirmedDescription =>
      'Pozostałe urządzenia w Twojej przestrzeni połączą się automatycznie.';

  @override
  String get pairingNoDeviceAdded => 'Nie dodano żadnego urządzenia.';

  @override
  String get pairingTimedOutDescription =>
      'Rozpocznij nową próbę parowania, aby kontynuować.';

  @override
  String get pairingIdleDescription =>
      'Rozpocznij próbę parowania, aby połączyć urządzenie.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'Chronione parowanie jest niedostępne';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Ta sesja parowania musi zostać otwarta w osobnym chronionym oknie.';

  @override
  String get pairingCancel => 'Anuluj parowanie';

  @override
  String get pairingClose => 'Zamknij parowanie';

  @override
  String get pairingOrEnterCode => 'Lub wpisz kod parowania';

  @override
  String get pairingJoinSecurely => 'Dołącz bezpiecznie';

  @override
  String get pairingProtectedWaitingDescription =>
      'Pokaż kod QR, zeskanuj kod lub wpisz kod parowania w tym chronionym oknie.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Wygasa za $seconds s';
  }

  @override
  String get pairingInvalidQr => 'Zeskanuj kod QR parowania CopyPaste.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Skaner Google jest niedostępny. Zamiast tego wpisz kod parowania.';

  @override
  String get pairingScannerCouldNotOpen =>
      'Nie można otworzyć skanera. Zamiast tego wpisz kod parowania.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'Bezpieczne wyświetlanie parowania jest niedostępne.';

  @override
  String get pairingVerificationCodeRequired =>
      'Najpierw pokaż i porównaj kod weryfikacyjny.';

  @override
  String get syncStatusUnavailable => 'Stan synchronizacji niedostępny';

  @override
  String get syncStatusOff => 'Synchronizacja wyłączona';

  @override
  String get syncStatusWaitingForSync => 'Oczekiwanie na synchronizację';

  @override
  String get syncStatusSyncing => 'Synchronizowanie';

  @override
  String get syncStatusUpToDate => 'Aktualne';

  @override
  String get syncStatusInterrupted => 'Synchronizacja przerwana';

  @override
  String get syncStatusPartiallySynchronized => 'Częściowo zsynchronizowano';

  @override
  String get syncStatusRetrying => 'Ponawianie';

  @override
  String get syncStatusWaitingToSync => 'Oczekuje na synchronizację';

  @override
  String get syncStatusWaitingForDevice => 'Oczekiwanie na urządzenie';

  @override
  String get syncStatusNoPairedDevices => 'Brak sparowanych urządzeń';

  @override
  String get syncStatusDevicesOffline => 'Urządzenia offline';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · dziennik aktywności niedostępny';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Synchronizacja wyłączona · $count problemu',
      many: 'Synchronizacja wyłączona · $count problemów',
      few: 'Synchronizacja wyłączona · $count problemy',
      one: 'Synchronizacja wyłączona · $count problem',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count offline',
      many: '$status · $count offline',
      few: '$status · $count offline',
      one: '$status · $count offline',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count urządzenia wymaga uwagi',
      many: '$count urządzeń wymaga uwagi',
      few: '$count urządzenia wymagają uwagi',
      one: '$count urządzenie wymaga uwagi',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Urządzenia offline · $count problemu',
      many: 'Urządzenia offline · $count problemów',
      few: 'Urządzenia offline · $count problemy',
      one: 'Urządzenia offline · $count problem',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Sprawdzanie połączenia';

  @override
  String get syncConnectionUnknown => 'Nieznany stan połączenia';

  @override
  String get syncLogDeviceOffline => 'Urządzenie przeszło w tryb offline.';

  @override
  String get syncLogConnectionRestored => 'Przywrócono połączenie.';

  @override
  String get syncLogProblemDetected => 'Wykryto problem z synchronizacją.';

  @override
  String get syncLogProblemResolved => 'Rozwiązano problem z synchronizacją.';

  @override
  String get syncLogFailed => 'Synchronizacja nie powiodła się.';

  @override
  String get syncLogRecovered => 'Przywrócono synchronizację.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent wpisu',
      many: '$sent wpisów',
      few: '$sent wpisy',
      one: '$sent wpis',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received wpisu',
      many: '$received wpisów',
      few: '$received wpisy',
      one: '$received wpis',
    );
    return 'Wysłano: $_temp0 · Odebrano: $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Synchronizacja: $status';
  }

  @override
  String get syncDetailsTitle => 'Synchronizacja';

  @override
  String get syncDetailsClose => 'Zamknij szczegóły synchronizacji';

  @override
  String get syncNoPairedDevicesMessage =>
      'Sparuj urządzenie, aby synchronizować historię schowka.';

  @override
  String get syncActivityLogUnavailableTitle =>
      'Dziennik aktywności niedostępny';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Nie można odczytać ani zapisać aktywności synchronizacji.';

  @override
  String get syncDetailsDevicesHeading => 'Urządzenia';

  @override
  String get syncDetailsActivityHeading => 'Aktywność';

  @override
  String get syncDetailsNoRecentProblems =>
      'Brak ostatnich problemów z synchronizacją';

  @override
  String get syncDetailsUnknownPeer => 'Sparowane urządzenie';

  @override
  String get syncPeerConnection => 'Połączenie';

  @override
  String get syncPeerLastCheckStarted => 'Początek ostatniego sprawdzania';

  @override
  String get syncPeerItemsOverSizeLimit =>
      'Wpisy przekraczające limit rozmiaru';

  @override
  String syncPeerLastSeen(String time) {
    return 'Ostatnio widziane: $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Ostatnia synchronizacja: $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Ostatnia udana synchronizacja';

  @override
  String get syncPeerConnectionDetails => 'Szczegóły połączenia';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Szczegóły połączenia: $device';
  }

  @override
  String get historyKindText => 'Tekst';

  @override
  String get historyKindLink => 'Link';

  @override
  String get historyKindEmail => 'E-mail';

  @override
  String get historyKindColor => 'Kolor';

  @override
  String get historyKindPhone => 'Telefon';

  @override
  String get historyKindCode => 'Kod';

  @override
  String get historyKindPath => 'Ścieżka';

  @override
  String get historyKindCredentials => 'Dane logowania';

  @override
  String get historyKindImage => 'Obraz';

  @override
  String get historyKindFile => 'Plik';

  @override
  String get historyKindOther => 'Inne';

  @override
  String get historySortNewest => 'Od najnowszych';

  @override
  String get historySortOldest => 'Od najstarszych';

  @override
  String get historySortRelevance => 'Najlepsze dopasowanie';

  @override
  String get historyErrorUpdatesUnavailable =>
      'Aktualizacje historii są tymczasowo niedostępne.';

  @override
  String get historyErrorBulkDelete =>
      'Nie można usunąć niektórych zaznaczonych wpisów. Spróbuj ponownie.';

  @override
  String get historyErrorBulkPin =>
      'Nie można zmienić przypięcia niektórych zaznaczonych wpisów. Spróbuj ponownie.';

  @override
  String get historyErrorRefresh =>
      'Nie można odświeżyć historii. Spróbuj ponownie.';

  @override
  String get historyErrorLoadMore => 'Nie można wczytać dalszej historii.';

  @override
  String get historyErrorLoadEarlier =>
      'Nie można wczytać wcześniejszej historii.';

  @override
  String get historyErrorOpenClip => 'Nie można otworzyć tego wpisu.';

  @override
  String get historyErrorCopyPlainText =>
      'Nie można skopiować zwykłego tekstu.';

  @override
  String get historyErrorCopyClip => 'Nie można skopiować tego wpisu.';

  @override
  String get historyErrorCopyText => 'Nie można skopiować tego tekstu.';

  @override
  String get historyErrorPrepareFile =>
      'Nie można pobrać tego pliku. Sprawdź urządzenie źródłowe i spróbuj ponownie.';

  @override
  String get historyErrorDownloadFile => 'Nie można pobrać tego pliku.';

  @override
  String get historyErrorSelectFiles =>
      'Nie można wybrać plików. Spróbuj ponownie.';

  @override
  String get historyErrorReadSelectedFile =>
      'Nie można odczytać wybranego pliku. Sprawdź dostęp i ilość wolnego miejsca.';

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
          'Nie można zaimportować $failed z $total pliku. Pierwszy: $name. $reason',
      many:
          'Nie można zaimportować $failed z $total plików. Pierwszy: $name. $reason',
      few:
          'Nie można zaimportować $failed z $total plików. Pierwszy: $name. $reason',
      one:
          'Nie można zaimportować $failed z $total pliku. Pierwszy: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate => 'Nie można zmienić spoilera.';

  @override
  String get historyErrorPinUpdate => 'Nie można zmienić przypięcia.';

  @override
  String get historyErrorPinnedOrderSave =>
      'Nie można zapisać kolejności przypiętych wpisów. Spróbuj ponownie.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'Kolejność została zapisana, ale nie można odświeżyć historii.';

  @override
  String get historyErrorDeleteClip => 'Nie można usunąć tego wpisu.';

  @override
  String get historyErrorDeleteAll => 'Nie można usunąć historii.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'Nie można usunąć zaznaczonych wpisów.';

  @override
  String get historyOcrErrorUnavailable =>
      'Rozpoznawanie tekstu (OCR) jest niedostępne. Sprawdź moduł w Ustawieniach i spróbuj ponownie.';

  @override
  String get historyOcrErrorStart =>
      'Nie można uruchomić rozpoznawania tekstu (OCR).';

  @override
  String get historyOcrErrorRecognize =>
      'Nie można rozpoznać tekstu na obrazie.';

  @override
  String get historyOcrRecognizing => 'Rozpoznawanie tekstu na obrazie.';

  @override
  String get historyOcrFailedTitle => 'Rozpoznawanie tekstu nie powiodło się';

  @override
  String get historyOcrNoText => 'Nie znaleziono tekstu';

  @override
  String get historyOcrTitle => 'Tekst z obrazu';

  @override
  String get historyOcrCopyError =>
      'Nie można skopiować tekstu. Spróbuj ponownie.';

  @override
  String get historyTagsErrorLoad =>
      'Nie można wczytać tagów. Spróbuj ponownie.';

  @override
  String get historyTagsErrorSave =>
      'Nie można zapisać zmiany tagu. Użyj unikalnej nazwy o długości do 64 znaków i wybierz przypięte wpisy.';

  @override
  String get historyTags => 'Tagi';

  @override
  String get historyTagsUpdateErrorTitle => 'Nie można zaktualizować tagów';

  @override
  String get historyTagsEmpty => 'Brak tagów';

  @override
  String get historyTagRename => 'Zmień nazwę';

  @override
  String get historyTagActions => 'Działania tagu';

  @override
  String get historyTagNamePlaceholder => 'Nazwa tagu';

  @override
  String get historyCreateTag => 'Utwórz tag';

  @override
  String get historySaveTag => 'Zapisz tag';

  @override
  String get historyTagsFilterAll => 'Wszystkie tagi';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count tagu',
      many: '$count tagów',
      few: '$count tagi',
      one: '$count tag',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Filtruj według tagów: $selection';
  }

  @override
  String get historyManageTags => 'Zarządzaj tagami';

  @override
  String get historyTagsFilterSemantics =>
      'Filtruj przypięte wpisy według tagów';

  @override
  String get historyDownloadErrorCancelled =>
      'Pobieranie pliku zostało anulowane.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'Usługa pobierania jest niedostępna. Otwórz ponownie CopyPaste i spróbuj jeszcze raz.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'Usługa pobierania nie nawiązała połączenia. Spróbuj ponownie.';

  @override
  String get historyFileErrorAccess =>
      'Nie można uzyskać dostępu do wybranego pliku.';

  @override
  String get historySpoilerRevealSemantics =>
      'Poufna zawartość. Odkryj spoiler';

  @override
  String get historyActionFailedTitle => 'Działanie nie powiodło się';

  @override
  String get historyUnavailableTitle => 'Historia jest niedostępna';

  @override
  String get historyEmptyTitle => 'Nie znaleziono wpisów';

  @override
  String get historyEmptyMessage =>
      'Tutaj pojawią się przechwycone wpisy i pasująca historia.';

  @override
  String get historyAttentionTitle => 'Historia wymaga uwagi';

  @override
  String get historyLoadMore => 'Wczytaj więcej';

  @override
  String get historySectionPinned => 'Przypięte';

  @override
  String get historySectionToday => 'Dzisiaj';

  @override
  String get historySectionYesterday => 'Wczoraj';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Pominięto $count wpisu',
      many: 'Pominięto $count wpisów',
      few: 'Pominięto $count wpisy',
      one: 'Pominięto $count wpis',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Nie można odczytać niektórych wpisów, dlatego nie są widoczne na tej liście.';

  @override
  String get historySearch => 'Szukaj w historii';

  @override
  String get historySearchSuggestions => 'Podpowiedzi wyszukiwania';

  @override
  String get historySearchSuggestionRecent => 'Ostatnie';

  @override
  String get historySearchSuggestionTag => 'Tag';

  @override
  String get historySearchSuggestionApp => 'Aplikacja';

  @override
  String get historySearchSuggestionKind => 'Typ';

  @override
  String get historySearchSuggestionDevice => 'Urządzenie';

  @override
  String get historySearchClearRecent => 'Wyczyść ostatnie wyszukiwania';

  @override
  String get historyResetFilters => 'Resetuj filtry';

  @override
  String get historyImportFiles => 'Importuj pliki';

  @override
  String get historySelectClips => 'Zaznacz wpisy';

  @override
  String get historyCloseSearch => 'Zamknij wyszukiwanie';

  @override
  String get historyFilterAllClips => 'Wszystkie wpisy';

  @override
  String get historyFilterAllDevices => 'Wszystkie urządzenia';

  @override
  String get historyFilterAllApps => 'Wszystkie aplikacje';

  @override
  String get historyPinClip => 'Przypnij wpis';

  @override
  String get historyUnpinClip => 'Odepnij wpis';

  @override
  String get historyReorderPinnedClip => 'Zmień kolejność przypiętego wpisu';

  @override
  String get historyReorderPinnedHint =>
      'Przeciągnij, aby przenieść. Alt + strzałka w górę lub w dół przesuwa o jedną pozycję.';

  @override
  String get historyClipUnavailable => 'Wpis niedostępny';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Typ wpisu: $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Wybierz wpis';

  @override
  String get historyInspectorEmptyMessage =>
      'Tutaj pojawi się jego pełna zawartość i dostępne działania.';

  @override
  String get historyNoPreview => 'Podgląd jest niedostępny.';

  @override
  String get historyCloseClipDetails => 'Zamknij szczegóły wpisu';

  @override
  String get historyCopyOptions => 'Opcje kopiowania';

  @override
  String get historyCopyPlainText => 'Kopiuj jako zwykły tekst';

  @override
  String get historyPinnedButton => 'Przypięty';

  @override
  String get historyAddSpoiler => 'Dodaj spoiler';

  @override
  String get historyRemoveSpoiler => 'Usuń spoiler';

  @override
  String get historyUpdateFailedTitle => 'Zmiana nie powiodła się';

  @override
  String get historyRecognizeImageText => 'Rozpoznaj tekst na obrazie';

  @override
  String get historyCopiedTitle => 'Skopiowano';

  @override
  String get historyCopyFailedTitle => 'Kopiowanie nie powiodło się';

  @override
  String get historyPlainTextCopied => 'Jako zwykły tekst.';

  @override
  String get historyDownloadedTitle => 'Pobrano';

  @override
  String get historyShare => 'Udostępnij';

  @override
  String get historyErrorShareClip => 'Nie można udostępnić tego wpisu.';

  @override
  String get historyErrorShareClips =>
      'Nie można udostępnić zaznaczonych wpisów.';

  @override
  String get historyBulkShare => 'Udostępnij zaznaczone';

  @override
  String get historyBulkShareFilesOnlyTitle => 'Udostępniono tylko pliki';

  @override
  String get historyBulkShareFilesOnly =>
      'Pominięto wpisy tekstowe, ponieważ ten system nie może udostępniać tekstu razem z plikami. Udostępnij je osobno.';

  @override
  String get historyDownload => 'Pobierz';

  @override
  String get historyDownloading => 'Pobieranie…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Pobieranie $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => 'Anulowanie…';

  @override
  String get historyDownloadCancellingTooltip => 'Anulowanie pobierania';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Pobieranie $percent% · Kliknij, aby anulować';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Pobieranie nie powiodło się · Kliknij, aby ponowić';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Anuluj pobieranie, ukończono $percent%';
  }

  @override
  String get historyRetryDownload => 'Ponów pobieranie';

  @override
  String get historyShowFile => 'Pokaż plik';

  @override
  String get historyMetadataCaptured => 'Przechwycono';

  @override
  String get historyMetadataObservedApp => 'Wykryta aplikacja';

  @override
  String get historyMetadataDevice => 'Urządzenie';

  @override
  String get historyMetadataType => 'Typ';

  @override
  String get historyMetadataPath => 'Ścieżka';

  @override
  String get historyMetadataFile => 'Plik';

  @override
  String get historyMetadataResolution => 'Rozdzielczość';

  @override
  String get historyMetadataSize => 'Rozmiar';

  @override
  String get historyMetadataFiles => 'Pliki';

  @override
  String get historyMetadataSync => 'Synchronizacja';

  @override
  String get historyMetadataTooLargeToSync => 'Za duży';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Usunąć $count zaznaczonego wpisu?',
      many: 'Usunąć $count zaznaczonych wpisów?',
      few: 'Usunąć $count zaznaczone wpisy?',
      one: 'Usunąć $count zaznaczony wpis?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Usunąć ten wpis?';

  @override
  String get historyBulkTagTooltip =>
      'Dodaj tagi do zaznaczonych przypiętych wpisów';

  @override
  String get historyBulkExit => 'Zakończ zaznaczanie';

  @override
  String get historyBulkPin => 'Przypnij zaznaczone';

  @override
  String get historyBulkUnpin => 'Odepnij zaznaczone';

  @override
  String get historyBulkDelete => 'Usuń zaznaczone';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Zaznaczono: $count',
      many: 'Zaznaczono: $count',
      few: 'Zaznaczono: $count',
      one: 'Zaznaczono: $count',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Filtruj według daty';

  @override
  String get historyDatePresetToday => 'Dzisiaj';

  @override
  String get historyDatePresetYesterday => 'Wczoraj';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Ostatnie $days dnia',
      many: 'Ostatnie $days dni',
      few: 'Ostatnie $days dni',
      one: 'Ostatni $days dzień',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'Cały okres';

  @override
  String get appRuntimeStartFailed =>
      'CopyPaste nie może uruchomić usługi w tle.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste czeka na dostęp do chronionej historii.';

  @override
  String get appHistoryUnavailableTitle => 'Historia jest niedostępna';

  @override
  String get appDevicesUnavailableTitle => 'Urządzenia są niedostępne';

  @override
  String get appSettingsUnavailableTitle => 'Ustawienia są niedostępne';

  @override
  String get appQuit => 'Zakończ CopyPaste';

  @override
  String get appWindowSetupTitle => 'Konfiguracja okna wymaga uwagi';

  @override
  String get appOpenHistory => 'Otwórz Historię';

  @override
  String get appDestinationHistory => 'Historia';

  @override
  String get appDestinationDevices => 'Urządzenia';

  @override
  String get appDestinationSettings => 'Ustawienia';

  @override
  String get appActiveFilter => 'Aktywny filtr';

  @override
  String get appActionFailed => 'Działanie nie powiodło się';

  @override
  String get sharedAndroidVersionRequired =>
      'Wymagany jest Android 11 lub nowszy';

  @override
  String get sharedUseAdbTab => 'Użyj karty ADB na tym urządzeniu.';

  @override
  String get sharedOneTimeAccessApplied => 'Zastosowano jednorazowy dostęp';

  @override
  String get sharedInstallShizuku => 'Zainstaluj Shizuku';

  @override
  String get sharedPairAndStartShizuku => 'Sparuj i uruchom Shizuku';

  @override
  String get sharedAllowCopyPaste => 'Zezwól CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded =>
      'Aplikacja Shizuku nie jest już potrzebna.';

  @override
  String get sharedGetShizukuDescription =>
      'Shizuku stosuje jednorazowy dostęp.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Użyj funkcji Debugowanie bezprzewodowe w Shizuku, a następnie wróć.';

  @override
  String get sharedApproveInShizuku =>
      'Zatwierdź jednorazowo CopyPaste w Shizuku.';

  @override
  String get sharedGetShizuku => 'Pobierz Shizuku';

  @override
  String get sharedOpenShizuku => 'Otwórz Shizuku';

  @override
  String get sharedRunOnComputer => 'Uruchom na komputerze';

  @override
  String get sharedCopyAllCommands => 'Kopiuj wszystkie polecenia';

  @override
  String get sharedEnableUsbDebugging =>
      'Włącz debugowanie USB i podłącz ten telefon.';

  @override
  String get sharedAllow => 'Zezwól';

  @override
  String get sharedApply => 'Zastosuj';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Nie można skopiować poleceń konfiguracji.';

  @override
  String get sharedShizukuOpenFailed => 'Nie można otworzyć Shizuku.';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => 'Skonfiguruj CopyPaste';

  @override
  String get onboardingWelcomeTitle => 'Witamy w CopyPaste';

  @override
  String get onboardingWelcomeSubtitle =>
      'Twój schowek, gotowy, kiedy go potrzebujesz.';

  @override
  String get onboardingReadyTitle => 'Aplikacja CopyPaste jest gotowa';

  @override
  String get onboardingGetStarted => 'Zaczynamy';

  @override
  String get onboardingOptionalSetupTitle =>
      'Opcjonalna konfiguracja wymaga uwagi';

  @override
  String get onboardingOpenSystemSettings => 'Otwórz Ustawienia';

  @override
  String get onboardingSetupErrorTitle => 'Konfiguracja wymaga uwagi';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste nie może odczytać stanu konfiguracji systemu $platform.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste nie może zapisać postępu konfiguracji.';

  @override
  String get onboardingStartAtLogin => 'Uruchamiaj przy logowaniu';

  @override
  String get onboardingStartAtLoginSemantics =>
      'Uruchamiaj CopyPaste przy logowaniu';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Nie można włączyć uruchamiania przy logowaniu. Możesz kontynuować bez tego.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Nie można zmienić uruchamiania przy logowaniu. Możesz kontynuować bez tego.';

  @override
  String get onboardingMacosTitle => 'Skonfiguruj Maca';

  @override
  String get onboardingAccessibility => 'Dostępność';

  @override
  String get onboardingAutoPasteEnabled => 'Automatyczne wklejanie włączone';

  @override
  String get onboardingAutoPasteOptional =>
      'Opcjonalne, do automatycznego wklejania';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Dostępne w zainstalowanej aplikacji CopyPaste.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'Wymagane zatwierdzenie rzeczy otwieranej podczas logowania';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Zezwól na CopyPaste w Ustawieniach systemowych, a następnie wróć.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste nie może odczytać stanu konfiguracji systemu macOS.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Nie można poprosić o uprawnienie Dostępność.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste nie może odświeżyć stanu konfiguracji systemu macOS.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Nie można odświeżyć stanu uprawnienia Dostępność.';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Nie można otworzyć ustawień Rzeczy otwierane podczas logowania.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'Uruchamianie przy logowaniu wymaga zatwierdzenia w Ustawieniach systemowych.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Nie można wyłączyć uruchamiania przy logowaniu.';

  @override
  String get onboardingWindowsTitle => 'Skonfiguruj komputer';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'Niedostępne w tej instalacji. Możesz kontynuować bez tego.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Wyłączono w Menedżerze zadań';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Włącz CopyPaste w aplikacjach uruchamianych przy starcie systemu Windows, a następnie wróć.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'Uruchamianie przy logowaniu jest wyłączone w Menedżerze zadań. Możesz je później włączyć w Ustawieniach systemu Windows.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'Uruchamianie przy logowaniu jest niedostępne. Możesz kontynuować bez tego.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste nie może odczytać stanu konfiguracji systemu Windows.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Nie można otworzyć ustawień uruchamiania systemu Windows.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Nie można zmienić uruchamiania przy logowaniu. Możesz to zrobić później w Ustawieniach.';

  @override
  String get onboardingLinuxChecking =>
      'Sprawdzanie sesji pulpitu systemu Linux.';

  @override
  String get onboardingLinuxTitle => 'Skonfiguruj pulpit systemu Linux';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste wymaga zweryfikowanej integracji z pulpitem, aby rozpoznawać źródło wpisów i obsługiwać Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Sesja pulpitu';

  @override
  String get onboardingLinuxX11Detected => 'Wykryto sesję X11.';

  @override
  String get onboardingLinuxWaylandDetected => 'Wykryto sesję Wayland.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'Nie wykryto obsługiwanej sesji pulpitu X11 ani Wayland.';

  @override
  String get onboardingLinuxCompanion => 'Komponent towarzyszący GNOME lub KDE';

  @override
  String get onboardingLinuxCompanionReady => 'Gotowy.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'Przechwytywanie schowka zostało wstrzymane przez wykluczenia aplikacji źródłowych. Włącz podpisany komponent towarzyszący, aby je wznowić.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Zainstaluj i włącz podpisany pakiet sesji komponentu towarzyszącego, a następnie wróć tutaj.';

  @override
  String get onboardingLinuxOpenSetup => 'Otwórz konfigurację';

  @override
  String get onboardingLinuxKeyboardControl => 'Sterowanie klawiaturą';

  @override
  String get onboardingLinuxKeyboardAllowed => 'Dozwolone dla Quick Paste.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'Niedostępne w tej sesji pulpitu.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Zezwól na sterowanie klawiaturą';

  @override
  String get onboardingLinuxQuickPasteReady => 'Gotowe.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Gotowe, gdy wymagana integracja z pulpitem będzie aktywna.';

  @override
  String get onboardingLinuxLinks => 'Linki CopyPaste';

  @override
  String get onboardingLinuxLinksRegistered => 'Otwierają się w tej aplikacji.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Zarejestruj linki CopyPaste dla zaproszeń do parowania.';

  @override
  String get onboardingLinuxRegisterLinks => 'Zarejestruj linki';

  @override
  String get onboardingLinuxScreenshotBlocking => 'Blokowanie zrzutów ekranu';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Niedostępne w systemie Linux. Przechowywane dane pozostają zaszyfrowane.';

  @override
  String get onboardingLinuxCheckAgain => 'Sprawdź ponownie';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste nie może odczytać stanu integracji z systemem Linux.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Nie można otworzyć konfiguracji integracji z pulpitem.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'Nie przyznano uprawnienia do sterowania klawiaturą.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Nie można zmienić uruchamiania przy logowaniu.';

  @override
  String get onboardingLinuxLinksFailed =>
      'Nie można zarejestrować linków CopyPaste.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Nie można odświeżyć stanu integracji z systemem Linux.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'Dostęp do schowka jest niedostępny w tej sesji X11.';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste wymaga sesji pulpitu X11 lub Wayland.';

  @override
  String get onboardingAndroidFullCapture => 'Pełne przechwytywanie';

  @override
  String get onboardingAndroidBackgroundCapture => 'Przechwytywanie w tle';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Skonfiguruj raz, a następnie sprawdź, kopiując coś naprawdę.';

  @override
  String get onboardingAndroidLimitedCapture => 'Ograniczone przechwytywanie';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'Udostępniaj do CopyPaste lub wracaj do aplikacji po skopiowaniu.';

  @override
  String get onboardingAndroidNotification => 'Powiadomienie o przechwytywaniu';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Wymagane podczas przechwytywania';

  @override
  String get onboardingAndroidBackgroundActivity => 'Aktywność w tle';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Zalecane, aby ograniczyć utratę przechwyconych danych';

  @override
  String get onboardingAndroidOpen => 'Otwórz';

  @override
  String get onboardingAndroidApplyAccess =>
      'Zastosuj dostęp do przechwytywania';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Jednorazowo. Potem aplikacja Shizuku nie jest potrzebna.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Pełne przechwytywanie możesz skonfigurować później.';

  @override
  String get onboardingAndroidCaptureWorking => 'Przechwytywanie w tle działa';

  @override
  String get onboardingAndroidVerifyCopy => 'Sprawdź, kopiując coś naprawdę';

  @override
  String get onboardingAndroidCopyReached =>
      'Nowy wpis trafił do lokalnej Historii.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Skopiuj coś w innej aplikacji, a następnie wróć tutaj.';

  @override
  String get onboardingAndroidStartCapture => 'Rozpocznij przechwytywanie';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste nie może odczytać stanu konfiguracji systemu Android.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Nie można odświeżyć stanu przechwytywania w systemie Android.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Monitorowanie konfiguracji systemu Android jest niedostępne.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'Nie można włączyć przechwytywania schowka.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Nie można zatrzymać przechwytywania w tle.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Nie można otworzyć ustawień baterii.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste ma zezwolenie w Shizuku, ale nie można zastosować dostępu do przechwytywania.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Zezwól na CopyPaste w Shizuku, aby zastosować dostęp do przechwytywania.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Najpierw przyznaj dostęp do schowka w tle.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Zezwól na powiadomienia przed rozpoczęciem przechwytywania w tle.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Nie można rozpocząć przechwytywania w tle.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Nie można otworzyć konfiguracji przechwytywania w systemie Android.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Nie można zapisać wybranej konfiguracji systemu Android.';

  @override
  String get modulesMarketplaceTab => 'Katalog';

  @override
  String get modulesInstalledTab => 'Zainstalowane';

  @override
  String get modulesRefresh => 'Odśwież';

  @override
  String get modulesInstallFromFile => 'Zainstaluj z pliku';

  @override
  String get modulesPackageFileType => 'Moduł CopyPaste';

  @override
  String get modulesSearchPlaceholder => 'Szukaj modułów';

  @override
  String get modulesOperationFailed => 'Operacja na module nie powiodła się';

  @override
  String get modulesUnavailableTitle => 'Moduły są niedostępne';

  @override
  String get modulesNoneInstalled => 'Brak zainstalowanych modułów';

  @override
  String get modulesNoMatches => 'Brak pasujących modułów';

  @override
  String get modulesChooseFromMarketplace => 'Wybierz moduł z katalogu.';

  @override
  String get modulesTryDifferentSearch => 'Spróbuj wyszukać coś innego.';

  @override
  String get modulesMarketplaceUnavailableTitle => 'Katalog jest niedostępny';

  @override
  String get modulesNoneAvailable => 'Brak dostępnych modułów';

  @override
  String get modulesNoneAvailableMessage =>
      'Moduły pojawią się tutaj po ich opublikowaniu.';

  @override
  String modulesEnableModule(String module) {
    return 'Włącz $module';
  }

  @override
  String get modulesInstall => 'Zainstaluj';

  @override
  String modulesUpdateTo(String version) {
    return 'Zaktualizuj do wersji $version';
  }

  @override
  String get modulesRemoveModule => 'Usuń moduł';

  @override
  String get modulesModuleSettings => 'Ustawienia modułu';

  @override
  String modulesSettingsTitle(String module) {
    return 'Ustawienia modułu $module';
  }

  @override
  String get modulesDownloadingModel => 'Pobieranie modelu językowego…';

  @override
  String get modulesRun => 'Uruchom';

  @override
  String modulesRemoveTitle(String module) {
    return 'Usunąć moduł $module?';
  }

  @override
  String get modulesRemoveMessage =>
      'Moduł i jego ustawienia lokalne zostaną usunięte.';

  @override
  String get modulesRemove => 'Usuń';

  @override
  String get modulesRemoveFailed => 'Nie można usunąć modułu.';

  @override
  String get modulesCloseSettings => 'Zamknij ustawienia modułu';

  @override
  String get modulesRemoved => 'Moduł usunięty';

  @override
  String get modulesNoSettingsTitle => 'Brak dodatkowych ustawień';

  @override
  String get modulesSetUpSmsAccess => 'Skonfiguruj dostęp do SMS-ów';

  @override
  String get modulesRestartApp => 'Uruchom ponownie CopyPaste';

  @override
  String get modulesSmsAccessTitle => 'Dostęp do SMS-ów';

  @override
  String get modulesCloseSmsAccess => 'Zamknij panel dostępu do SMS-ów';

  @override
  String get modulesSmsReady =>
      'Dostęp jest gotowy. Włącz moduł SMS Codes, aby zacząć kopiować nowe kody.';

  @override
  String get modulesSmsAllowAccess =>
      'Zezwól na dostęp, aby kopiować kody z nowych SMS-ów.';

  @override
  String get modulesSmsNotification => 'Powiadomienie modułu SMS';

  @override
  String get modulesSmsNotificationDescription =>
      'Wymagane podczas działania modułu SMS Codes';

  @override
  String get modulesSmsApp => 'CopyPaste SMS';

  @override
  String get modulesSmsAppDescription =>
      'Osobna aplikacja, jedyna czytająca SMS-y';

  @override
  String get modulesSmsApplyAccess => 'Zastosuj dostęp do SMS-ów';

  @override
  String get modulesSmsApplyAccessDescription => 'Wymagane tylko raz.';

  @override
  String get modulesSmsSetupFailed => 'Konfiguracja SMS-ów nie powiodła się';

  @override
  String get modulesFileSelectionFailed => 'Wybór pliku nie powiódł się';

  @override
  String get modulesChooseFile => 'Wybierz plik';

  @override
  String modulesModelReady(String model) {
    return 'Model $model jest gotowy';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return 'Model $model · do pobrania $size MiB';
  }

  @override
  String get modulesPlatformsUnknown => 'Platformy: nieznane';

  @override
  String modulesPlatforms(String platforms) {
    return 'Platformy: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'Podpis katalogu modułów jest nieprawidłowy.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Nie można połączyć się z katalogiem modułów.';

  @override
  String get modulesMarketplaceTimeout => 'Katalog modułów nie odpowiada.';

  @override
  String get modulesIncompatible =>
      'Ten moduł nie jest zgodny z tym urządzeniem.';

  @override
  String get modulesPackageSizeInvalid =>
      'Rozmiar pakietu modułu jest nieprawidłowy.';

  @override
  String get modulesDownloadInterrupted =>
      'Pobieranie modułu zostało przerwane.';

  @override
  String get modulesDownloadTimeout =>
      'Przekroczono limit czasu pobierania modułu.';

  @override
  String get modulesPackageUrlUntrusted =>
      'Adres URL pakietu modułu nie jest zaufany.';

  @override
  String get modulesDownloadLeftGithub =>
      'Pobieranie modułu zostało przekierowane poza serwis GitHub.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'Pobieranie modułu wymagało zbyt wielu przekierowań.';

  @override
  String get modulesDownloadTooLarge => 'Pobierany moduł jest za duży.';

  @override
  String get modulesPackageIntegrityFailed =>
      'Pakiet modułu nie przeszedł weryfikacji integralności.';

  @override
  String get modulesDownloadUnavailable =>
      'Pobieranie tego modułu jest obecnie niedostępne.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'Katalog modułów jest niedostępny. Spróbuj ponownie.';

  @override
  String get modulesCatalogInvalid => 'Katalog modułów jest nieprawidłowy.';

  @override
  String get modulesThisSystem => 'ten system';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version lub nowszy';
  }

  @override
  String get modulesNotAvailableForDevice => 'Niedostępny dla tego urządzenia.';

  @override
  String get modulesAppVersionUnverified =>
      'Nie można zweryfikować zainstalowanej wersji CopyPaste.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Ten moduł nie obsługuje CopyPaste $version.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Wymagany: $requirement.';
  }

  @override
  String get modulesSmsSetupRequired =>
      'Skonfiguruj dostęp do SMS-ów przed włączeniem tego modułu.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'Nie można uruchomić monitorowania SMS-ów. Moduł jest nadal włączony; wyłącz go ponownie, aby ponowić próbę.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'Nie można uruchomić monitorowania SMS-ów. Moduł został wyłączony.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'Nie można uruchomić monitorowania SMS-ów. Nie można potwierdzić, czy moduł jest włączony.';

  @override
  String get modulesSmsMonitoringFailed =>
      'Nie można uruchomić monitorowania SMS-ów.';

  @override
  String get modulesSmsAndroidOnly =>
      'Dostęp do SMS-ów jest możliwy tylko w systemie Android.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'Monitorowanie SMS-ów wymaga konfiguracji.';

  @override
  String get modulesSmsMonitoringUnavailable =>
      'Nie udało się sprawdzić monitorowania SMS-ów.';

  @override
  String get modulesOperationIncomplete =>
      'Nie można ukończyć operacji na module.';

  @override
  String get modulesUnsupportedEvent =>
      'Moduł używa nieobsługiwanego zdarzenia.';

  @override
  String get modulesUnsupportedResult => 'Moduł zwrócił nieobsługiwany wynik.';

  @override
  String get modulesUnsupportedField => 'Moduł używa nieobsługiwanego pola.';

  @override
  String get modulesResponseInvalid => 'Odpowiedź modułu jest nieprawidłowa.';

  @override
  String get modulesFileSelectionUnavailable => 'Tutaj niedostępny.';

  @override
  String modulesFileNotAccepted(String field) {
    return 'Plik w polu $field nie jest akceptowany.';
  }

  @override
  String get modulesSmsVerifyFailed =>
      'Nie można zweryfikować dostępu do SMS-ów.';

  @override
  String get modulesSmsAllowNotifications =>
      'Zezwól na powiadomienia, aby moduł SMS Codes pozostał aktywny w tle.';

  @override
  String get modulesSmsNotGranted => 'Nie przyznano dostępu do SMS-ów.';

  @override
  String get modulesSmsConfigureFailed =>
      'Nie można skonfigurować dostępu do SMS-ów.';

  @override
  String get onboardingLinuxContinueBasic =>
      'Kontynuuj z podstawowym przechwytywaniem';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Opcjonalne. Wymagane dla Quick Paste i wykluczeń aplikacji źródłowych.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Opcjonalne. Wymagane, aby funkcja Quick Paste mogła wracać do poprzedniej aplikacji.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'Dostęp do schowka jest niedostępny w tej sesji Wayland. Włącz podpisany komponent towarzyszący dla GNOME lub KDE, a następnie sprawdź ponownie.';

  @override
  String get historyMoreActions => 'Więcej działań';

  @override
  String get historyTagsAction => 'Tagi…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'odblokować poufne elementy',
      'other': 'Odblokuj poufne elementy',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'skopiować poufny element',
      'other': 'Skopiuj poufny element',
    });
    return '$_temp0';
  }

  @override
  String get commonOn => 'Włączone';

  @override
  String get commonOff => 'Wyłączone';

  @override
  String get settingsQuickPasteStartFailed =>
      'Nie można uruchomić Quick Paste, więc jego skrót nie działa.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut jest używany przez inną aplikację. Wybierz inny skrót.';
  }

  @override
  String get devicesEmptyTitle => 'Nie sparowano jeszcze innych urządzeń';

  @override
  String get devicesEmptyMessage =>
      'Sparuj urządzenie w tej samej sieci, aby synchronizować historię.';

  @override
  String get pairingStateNotCompleted => 'Parowanie nie zostało ukończone';

  @override
  String get pairingStepInvite => 'Zaproszenie';

  @override
  String get pairingStepScan => 'Skanowanie';

  @override
  String get pairingStepCode => 'Kod';

  @override
  String get pairingStepRequest => 'Prośba';

  @override
  String get pairingStepConnect => 'Połączenie';

  @override
  String get pairingStepConfirm => 'Potwierdzenie';

  @override
  String get pairingStepPaired => 'Sparowano';

  @override
  String get pairingOtherDevice => 'Drugie urządzenie';

  @override
  String get pairingDecisionNotConfirmed => 'Jeszcze nie potwierdzono';

  @override
  String get pairingDecisionSending => 'Wysyłanie decyzji…';

  @override
  String get pairingDecisionConfirmed => 'Potwierdzono';

  @override
  String get pairingDecisionRejected => 'Odrzucono';

  @override
  String get pairingDecisionWaiting => 'Oczekiwanie na potwierdzenie';

  @override
  String get pairingDecisionUnknown => 'Odpowiedź nieznana';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count urządzenia',
      many: '$count urządzeń',
      few: '$count urządzenia',
      one: '$count urządzenie',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count wiersza',
      many: '$count wierszy',
      few: '$count wiersze',
      one: '$count wiersz',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Nawigacja główna';

  @override
  String sharedStepCounter(int step, int count) {
    return 'Krok $step z $count';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Krok $step z $count: $name';
  }

  @override
  String get modulesPhaseDownload => 'Pobieranie';

  @override
  String get modulesPhaseInstall => 'Instalacja';

  @override
  String modulesInstallingModule(String module) {
    return 'Instalowanie: $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return 'Aktualizowanie: $module';
  }

  @override
  String get commonDismiss => 'Odrzuć';

  @override
  String get settingsRetentionConfirmTitle => 'Usunąć starszą historię?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Nowy limit: $limit. Starsze nieprzypięte wpisy zostaną trwale usunięte.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle => 'Zmniejszyć limit miejsca?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Nowy limit: $size. Najstarsze nieprzypięte wpisy ponad limit zostaną trwale usunięte.';
  }

  @override
  String get settingsLimitConfirmAction => 'Zastosuj limit';

  @override
  String get settingsHideEverywhereTitle => 'Całkowicie ukryć CopyPaste?';

  @override
  String get settingsHideEverywhereMessage =>
      'Gdy oba są wyłączone, nie ma ikony. Uruchom CopyPaste ponownie, aby je otworzyć.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Gdy oba są wyłączone, nie ma ikony. Aby otworzyć, naciśnij $shortcut lub uruchom CopyPaste ponownie.';
  }

  @override
  String get settingsHideEverywhereAction => 'Ukryj';

  @override
  String get settingsExportDoneTitle => 'Wyeksportowano historię tekstową';

  @override
  String get settingsBackupDoneTitle => 'Utworzono kopię zapasową';

  @override
  String get settingsRestoreDoneTitle => 'Przywrócono historię';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count wpisu tekstowego',
      many: '$count wpisów tekstowych',
      few: '$count wpisy tekstowe',
      one: '$count wpis tekstowy',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'nie można odczytać $count',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

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
    return '$used z $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return 'Zajęte $used z $quota';
  }

  @override
  String storageUsageNoQuota(String used) {
    return 'Zajęte $used';
  }

  @override
  String get storageUsageOverQuota => 'Limit przekroczony';

  @override
  String storageUsageOldestClip(String date) {
    return 'Najstarszy wpis: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Miejsce: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Nowy limit: $size. Zajęte jest $used, więc najstarsze nieprzypięte wpisy zostaną trwale usunięte.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count wpisu',
      many: '$count wpisów',
      few: '$count wpisy',
      one: '$count wpis',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'Otwórz CopyPaste';

  @override
  String get trayOpenQuickPaste => 'Otwórz Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Otwórz Quick Paste ($shortcut)';
  }

  @override
  String get trayRecent => 'Ostatnie';

  @override
  String get trayRecentLoading => 'Wczytywanie…';

  @override
  String get trayRecentFailed => 'Nie udało się wczytać klipów';

  @override
  String get trayRecentEmpty => 'Brak klipów';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ostatniego klipu ukryte',
      many: '$count ostatnich klipów ukrytych',
      few: '$count ostatnie klipy ukryte',
      one: '$count ostatni klip ukryty',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Tajne';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Kopiuj najnowszy z innego urządzenia';

  @override
  String get trayCaptureNow => 'Przechwyć teraz';

  @override
  String trayPausedUntil(String time) {
    return 'Wstrzymano do $time';
  }

  @override
  String get trayPauseFor => 'Wstrzymaj na…';

  @override
  String get trayPauseFor15Minutes => '15 minut';

  @override
  String get trayPauseForOneHour => '1 godzinę';

  @override
  String get trayPauseUntilResumed => 'Do wznowienia';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count elementu',
      many: 'Paste Queue: $count elementów',
      few: 'Paste Queue: $count elementy',
      one: 'Paste Queue: $count element',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Zbieranie: $count elementu',
      many: 'Zbieranie: $count elementów',
      few: 'Zbieranie: $count elementy',
      one: 'Zbieranie: $count element',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Ostatnio pominięte ($count)';
  }

  @override
  String get trayOpenContentRules => 'Otwórz reguły treści';

  @override
  String get trayRecentSettingsTitle => 'Pasek menu i zasobnik';

  @override
  String get trayRecentSettingsDescription =>
      'Co menu na pasku menu lub w zasobniku pokazuje w sekcji Ostatnie.';

  @override
  String get trayRecentCountTitle => 'Ostatnie klipy';

  @override
  String get trayRecentCountDescription =>
      'Ile najnowszych klipów pokazuje menu.';

  @override
  String get trayRecentPreviewTitle => 'Podgląd ostatnich klipów';

  @override
  String get trayRecentPreviewDescription => 'Ile tekstu klipów pokazuje menu.';

  @override
  String get trayRecentPreviewFull => 'Pełny podgląd';

  @override
  String get trayRecentPreviewTitles => 'Tylko typ i źródło';

  @override
  String get trayRecentPreviewHidden => 'Ukryte';

  @override
  String get trayRecentSettingsKeywords =>
      'zasobnik pasek menu ostatnie podgląd prywatność liczba';

  @override
  String get transformsPaletteTitle => 'Przekształć tekst';

  @override
  String get transformsFilterPlaceholder => 'Filtruj przekształcenia';

  @override
  String get transformsNoMatches => 'Brak pasujących przekształceń';

  @override
  String get transformsGroupResults => 'Wyniki';

  @override
  String get transformsGroupSuggested => 'Sugerowane';

  @override
  String get transformsGroupRecent => 'Ostatnie';

  @override
  String get transformsGroupFavorites => 'Ulubione';

  @override
  String get transformsGroupPipelines => 'Potoki';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV i XML';

  @override
  String get transformsCategoryEncode => 'Kodowanie i dekodowanie';

  @override
  String get transformsCategoryCase => 'Wielkość liter';

  @override
  String get transformsCategoryHash => 'Skróty';

  @override
  String get transformsCategoryLines => 'Wiersze i odstępy';

  @override
  String get transformsCategoryClean => 'Porządkowanie';

  @override
  String get transformsCategoryUrl => 'Linki';

  @override
  String get transformsCategoryMarkup => 'Tekst sformatowany i Markdown';

  @override
  String get transformsCategoryDev => 'Dla programistów';

  @override
  String get transformsCategoryOther => 'Inne';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Następnie: $name';
  }

  @override
  String get transformsStepEnabled => 'Wykonaj ten krok';

  @override
  String get transformsMoveEarlier => 'Przenieś wyżej';

  @override
  String get transformsMoveLater => 'Przenieś niżej';

  @override
  String get transformsRemoveStep => 'Usuń krok';

  @override
  String get transformsSavePipeline => 'Zapisz jako potok';

  @override
  String get transformsSavePipelineTitle => 'Zapisz potok';

  @override
  String get transformsSavePipelineFailed => 'Nie udało się zapisać potoku.';

  @override
  String get transformsPipelineNameLabel => 'Nazwa';

  @override
  String get transformsPipelineDescriptionLabel => 'Opis (opcjonalnie)';

  @override
  String get transformsPipelineStepsLabel => 'Kroki';

  @override
  String get transformsPipelineNoSteps => 'Dodaj co najmniej jeden krok.';

  @override
  String get transformsPipelineNameRequired => 'Wpisz nazwę.';

  @override
  String get transformsPipelineNameTooLong => 'Użyj maksymalnie 80 znaków.';

  @override
  String get transformsPipelineNameTaken => 'Inny potok ma już tę nazwę.';

  @override
  String get transformsPipelineSavedTitle => 'Zapisano potok';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name jest teraz w sekcji Potoki.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Kroki: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Usunięty potok';

  @override
  String get transformsNeedsNewerVersion => 'Wymaga nowszej wersji CopyPaste';

  @override
  String get transformsLoadFailed => 'Nie udało się wczytać przekształceń.';

  @override
  String get transformsNotTextTitle => 'Przekształcenia działają na tekście';

  @override
  String get transformsNotTextMessage =>
      'Ten wpis to obraz, plik lub kolor. Wybierz wpis tekstowy lub skopiuj tekst.';

  @override
  String get transformsNothingTitle => 'Nie ma czego przekształcić';

  @override
  String get transformsNothingMessage =>
      'Skopiuj tekst lub wybierz wpis z historii.';

  @override
  String get transformsPickTransform => 'Wybierz przekształcenie';

  @override
  String get transformsPickTransformHint =>
      'Tab dodaje je jako krok. Enter stosuje wynik.';

  @override
  String get transformsPreviewing => 'Aktualizowanie podglądu…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Wiersze: $lines · Słowa: $words · Znaki: $chars';
  }

  @override
  String get transformsNoChanges => 'Brak zmian';

  @override
  String transformsTruncated(String size) {
    return 'Wyświetlanie pierwszych 64 KiB z $size';
  }

  @override
  String get transformsRandom => 'Losowe';

  @override
  String get transformsNotRun => 'Nie wykonano';

  @override
  String get transformsSensitiveHidden => 'Wynik jest poufny i ukryty.';

  @override
  String get transformsReveal => 'Pokaż';

  @override
  String get transformsPaneOutput => 'Wynik';

  @override
  String get transformsPaneInput => 'Wejście';

  @override
  String get transformsPaneChanges => 'Zmiany';

  @override
  String get transformsCannotTransform => 'Nie można przekształcić tego tekstu';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Krok $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Pokaż jako hex';

  @override
  String get transformsActionPaste => 'Wklej';

  @override
  String get transformsActionSave => 'Zapisz jako nowy element';

  @override
  String get transformsCopiedMessage => 'Wynik jest też w historii.';

  @override
  String get transformsSavedTitle => 'Zapisano';

  @override
  String get transformsSavedMessage => 'Wynik jest nowym wpisem w historii.';

  @override
  String get transformsPasteFailedTitle => 'Nie udało się wkleić';

  @override
  String get transformsPasteFailedMessage =>
      'Wynik jest w schowku. Wklej go ręcznie.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message w wierszu $line, kolumnie $column';
  }

  @override
  String get transformsErrorBinary =>
      'Zdekodowane dane są binarne, nie tekstowe';

  @override
  String get transformsErrorTooLarge => 'Za duże do przekształcenia';

  @override
  String get transformsErrorInvalidParam =>
      'Ustawienie tego kroku jest nieprawidłowe';

  @override
  String get transformsErrorCancelled => 'Anulowano';

  @override
  String get transformsErrorTooExpensive => 'Wzorzec jest zbyt kosztowny';

  @override
  String get transformsErrorPattern => 'Wzorzec jest nieprawidłowy';

  @override
  String get transformsErrorTimestampUnit =>
      'Wybierz, czy liczba jest w sekundach, czy milisekundach';

  @override
  String get transformsErrorTimeLimit => 'Trwało zbyt długo';

  @override
  String get transformsErrorExternalEntity =>
      'Encje zewnętrzne są niedozwolone';

  @override
  String get transformsErrorJwtEncrypted =>
      'Zaszyfrowanych tokenów nie można zdekodować';

  @override
  String get transformsErrorJson => 'Nieprawidłowy JSON';

  @override
  String get transformsErrorYaml => 'Nieprawidłowy YAML';

  @override
  String get transformsErrorXml => 'Nieprawidłowy XML';

  @override
  String get transformsErrorCsv => 'Nieprawidłowy CSV';

  @override
  String get transformsErrorBase64 => 'Nieprawidłowy Base64';

  @override
  String get transformsErrorHex => 'Nieprawidłowy hex';

  @override
  String get transformsErrorEscape => 'Nieprawidłowa sekwencja ucieczki';

  @override
  String get transformsErrorUrl => 'Nieprawidłowe kodowanie procentowe';

  @override
  String get transformsErrorJwt => 'Nieprawidłowy JWT';

  @override
  String get transformsErrorDate => 'Nierozpoznana data lub znacznik czasu';

  @override
  String get transformsErrorGeneric =>
      'Ten krok nie może przekształcić danych wejściowych';

  @override
  String get transformsWarningSignedLink =>
      'Podpisany link pozostawiono bez zmian';

  @override
  String get transformsWarningExpired => 'Wygasł';

  @override
  String get transformsWarningNotYetValid => 'Jeszcze nieważny';

  @override
  String get transformsWarningSignatureNotVerified => 'Podpis niezweryfikowany';

  @override
  String get transformsWarningDuplicateKeys => 'Zduplikowane klucze';

  @override
  String get transformsWarningLenientJson =>
      'Zaakceptowano niestandardowy JSON';

  @override
  String get transformsWarningDoubleEncoded => 'JSON był zakodowany dwukrotnie';

  @override
  String get transformsWarningEscapes =>
      'Nieznane sekwencje ucieczki pozostawiono bez zmian';

  @override
  String get transformsWarningLocalZone => 'Przyjęto lokalną strefę czasową';

  @override
  String get transformsWarningNoMatches => 'Brak dopasowań';

  @override
  String get transformsWarningGeneric => 'Sprawdź wynik';

  @override
  String get transformsRestoreTitle => 'Przywróć schowek po wklejeniu';

  @override
  String get transformsRestoreDescription =>
      'Przywróć poprzedni schowek po wklejeniu.';

  @override
  String get transformsDefaultTitle =>
      'Domyślne przekształcenie przy wklejaniu';

  @override
  String get transformsDefaultDescription =>
      'Stosowane do tekstu wklejanego z Quick Paste.';

  @override
  String get transformsNoDefault => 'Brak';

  @override
  String get transformsAppDefaultsTitle => 'Domyślne dla aplikacji';

  @override
  String get transformsAppDefaultsDescription =>
      'Używaj innego ustawienia domyślnego w wybranych aplikacjach.';

  @override
  String transformsOtherPlatform(String platform) {
    return 'tylko na $platform';
  }

  @override
  String get transformsAddApp => 'Dodaj aplikację';

  @override
  String get transformsRemove => 'Usuń';

  @override
  String get transformsFavoritesTitle => 'Ulubione';

  @override
  String get transformsFavoritesDescription =>
      'Wyświetlane u góry, gdy przekształcasz tekst.';

  @override
  String transformsFavoriteShortcut(String shortcut) {
    return '$shortcut w oknie „Przekształć tekst”';
  }

  @override
  String get transformsAddFavorite => 'Dodaj ulubione';

  @override
  String get transformsPipelinesTitle => 'Potoki';

  @override
  String get transformsPipelinesDescription =>
      'Zapisane łańcuchy przekształceń.';

  @override
  String get transformsNewPipeline => 'Nowy potok';

  @override
  String get transformsEditPipeline => 'Edytuj potok';

  @override
  String get transformsDuplicatePipeline => 'Duplikuj';

  @override
  String get transformsDeletePipeline => 'Usuń';

  @override
  String transformsDeletePipelineTitle(String name) {
    return 'Usunąć $name?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Ulubione i ustawienia domyślne wklejania, które go używają, zostaną wyczyszczone.';

  @override
  String get transformsDeletePipelineFailed => 'Nie udało się usunąć potoku.';

  @override
  String get transformsAddStep => 'Dodaj krok';

  @override
  String get transformsHiddenTitle => 'Ukryte przekształcenia';

  @override
  String get transformsHiddenDescription => 'Poza listą i sugestiami.';

  @override
  String get transformsHideTransform => 'Ukryj przekształcenie';

  @override
  String get transformsShow => 'Pokaż';

  @override
  String get transformsSuggestionsTitle => 'Sugestie';

  @override
  String get transformsSuggestionsDescription =>
      'Proponuj przekształcenia pasujące do tekstu.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'Tokeny JSON Web';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl => 'Linki z parametrami śledzącymi';

  @override
  String get transformsDetectorTimestamp => 'Znaczniki czasu';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Parametry śledzące';

  @override
  String get transformsTrackingDescription =>
      'Parametry linków do dodatkowego usuwania lub zawsze zachowywane.';

  @override
  String get transformsTrackingRemoveLabel => 'Usuwaj także';

  @override
  String get transformsTrackingAllowLabel => 'Zawsze zachowuj';

  @override
  String get transformsTrackingPlaceholder =>
      'nazwa lub nazwa@domena, potem Enter';

  @override
  String get quickAccessUnavailableTitle =>
      'Stan szybkiego dostępu jest niedostępny';

  @override
  String get quickAccessActionFailedTitle => 'Nie udało się wykonać działania';

  @override
  String get quickAccessActionFailedMessage =>
      'Android nie otworzył żądanego ekranu lub nie zapisał zmiany. Spróbuj ponownie.';

  @override
  String get quickAccessKeyboardTitle => 'Klawiatura wpisów';

  @override
  String get quickAccessKeyboardDescription =>
      'Włącz „Wpisy CopyPaste” w ustawieniach klawiatury.';

  @override
  String get quickAccessKeyboardOn => 'Włączona';

  @override
  String get quickAccessKeyboardOff => 'Wyłączona';

  @override
  String get quickAccessOpenKeyboardSettings => 'Otwórz ustawienia klawiatury';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'Włącz klawiaturę „Wpisy CopyPaste”.';

  @override
  String get quickAccessKeyboardKeywords =>
      'klawiatura metoda wprowadzania ime panel włącz';

  @override
  String get quickAccessSwitchKeyboardTitle => 'Przełącz na klawiaturę wpisów';

  @override
  String get quickAccessSwitchKeyboardAction => 'Wybierz klawiaturę';

  @override
  String get quickAccessKeyboardCurrent => 'Bieżąca klawiatura';

  @override
  String get quickAccessCaptureTierTitle => 'Tryb przechwytywania';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'Zobacz, jak zapisywane są kopie z innych aplikacji.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'przechwytywanie poziom pełny panel ręczny shizuku zbieranie';

  @override
  String get quickAccessTierFull => 'Pełny: każda kopia jest zapisywana w tle';

  @override
  String get quickAccessTierPanelCollecting =>
      'Klawiatura wpisów: zapisuje każdą kopię, gdy jest wybrana';

  @override
  String get quickAccessTierPanelCatchUp =>
      'Klawiatura wpisów: zapisuje schowek po otwarciu';

  @override
  String get quickAccessTierManual =>
      'Ręczny: zapisuj z aplikacji, kafelka, widżetu lub menu Udostępnij';

  @override
  String get quickAccessTierPaused => 'Przechwytywanie wstrzymane';

  @override
  String get quickAccessAutoReturnTitle =>
      'Wracaj do poprzedniej klawiatury po wstawieniu';

  @override
  String get quickAccessAutoReturnDescription =>
      'Przełącz z powrotem po wstawieniu wpisu.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'powrót poprzednia klawiatura automatycznie wstawianie';

  @override
  String get quickAccessFocusSearchTitle =>
      'Aktywuj wyszukiwanie po otwarciu klawiatury wpisów';

  @override
  String get quickAccessTilesTitle => 'Kafelki szybkich ustawień';

  @override
  String get quickAccessTilesSearchDescription =>
      'Dodaj kafelki „Zapisz schowek” i „Przechwytywanie schowka”.';

  @override
  String get quickAccessTilesKeywords =>
      'szybkie ustawienia kafelek panel zapisz wstrzymaj';

  @override
  String get quickAccessTileInstructions =>
      'Przesuń dwukrotnie w dół, kliknij Edytuj i dodaj kafelek CopyPaste.';

  @override
  String get quickAccessSaveTileTitle => 'Kafelek „Zapisz schowek”';

  @override
  String get quickAccessSaveTileDescription =>
      'Zapisuje zawartość schowka z szybkich ustawień.';

  @override
  String get quickAccessCaptureTileTitle => 'Kafelek „Przechwytywanie schowka”';

  @override
  String get quickAccessCaptureTileDescription =>
      'Wstrzymuje i wznawia przechwytywanie z szybkich ustawień.';

  @override
  String get quickAccessAddTile => 'Dodaj kafelek';

  @override
  String get quickAccessTileAdded => 'Dodano do szybkich ustawień';

  @override
  String get quickAccessWidgetTitle => 'Widżet „Ostatnie wpisy”';

  @override
  String get quickAccessWidgetDescription =>
      'Ostatnie wpisy na ekranie głównym, kopiowane jednym dotknięciem.';

  @override
  String get quickAccessWidgetInstructions =>
      'Przytrzymaj ekran główny, kliknij Widżety i dodaj „Ostatnie wpisy”.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Dodano $count widżetu',
      many: 'Dodano $count widżetów',
      few: 'Dodano $count widżety',
      one: 'Dodano 1 widżet',
      zero: 'Nie dodano',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Dodaj widżet';

  @override
  String get quickAccessWidgetKeywords =>
      'ekran główny widżet launcher ostatnie';

  @override
  String get snippetsClipTooLarge =>
      'Za duży do edycji. Ten wpis otwiera się tylko do odczytu.';

  @override
  String get snippetsConcealedConfirmAction => 'Kontynuuj';

  @override
  String get snippetsConcealedConfirmMessage =>
      'To urządzenie nie może cię zweryfikować. Kontynuuj tylko na własnym urządzeniu.';

  @override
  String get snippetsConcealedConfirmTitle => 'Potwierdź, że to ty';

  @override
  String get snippetsDiscard => 'Odrzuć';

  @override
  String get snippetsEditClip => 'Edytuj wpis';

  @override
  String get snippetsFormattingNotice =>
      'Formatowanie zostanie usunięte przy zapisie.';

  @override
  String get snippetsNewText => 'Nowy tekst';

  @override
  String get snippetsReplaceOriginal => 'Zastąp oryginał';

  @override
  String get snippetsReplaceOriginalHint =>
      'Usuwa oryginalny wpis po zapisaniu.';

  @override
  String get snippetsSaveFailed => 'Nie udało się zapisać';

  @override
  String get snippetsUnsavedMessage => 'Zapisać zmiany przed zamknięciem?';

  @override
  String get snippetsUnsavedTitle => 'Niezapisane zmiany';

  @override
  String get quickPasteTabQueue => 'Kolejka';

  @override
  String get quickPasteMaskedMode => 'Pole hasła: podglądy są ukryte';

  @override
  String quickPasteMaskedItem(String kind) {
    return 'Ukryte: $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'Odblokuj tajne elementy';

  @override
  String quickPastePastingWith(String name) {
    return 'Wklejanie z: $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut wkleja oryginał';
  }

  @override
  String get quickPasteDefaultSkipped => 'Pominięto domyślne przekształcenie';

  @override
  String get quickPasteDefaultSkippedMessage => 'Zamiast tego użyto oryginału.';

  @override
  String get quickPastePasteManually => 'Wklej ręcznie.';

  @override
  String get rulesLoadFailed => 'Nie udało się wczytać reguł treści.';

  @override
  String get rulesNewRule => 'Nowa reguła';

  @override
  String get rulesEditRule => 'Edytuj regułę';

  @override
  String get rulesUntitled => 'Reguła bez nazwy';

  @override
  String get rulesBuiltinCredentials => 'Wykryte dane logowania';

  @override
  String get rulesBuiltinPaymentCards => 'Numery kart';

  @override
  String get rulesBuiltinCredentialsExpiry => 'Usuwaj wykryte dane logowania';

  @override
  String get rulesPrivacyDefaultsTitle => 'Domyślne ustawienia prywatności';

  @override
  String get rulesPrivacyDefaultsDescription =>
      'Ustawienia prywatności działają przed regułami.';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'prywatność ukryte tymczasowe wykluczone aplikacje wbudowane dane logowania hasła karty';

  @override
  String get rulesOpenPrivacy => 'Otwórz Prywatność';

  @override
  String get rulesMyRulesTitle => 'Moje reguły';

  @override
  String get rulesMyRulesDescription => 'Działają po kolei na nowych kopiach.';

  @override
  String get rulesMyRulesKeywords =>
      'reguły automatyzacja tag pomiń tajne wygaś usuń wzorzec regex';

  @override
  String get rulesEmptyTitle => 'Brak reguł';

  @override
  String get rulesDeletedTitle => 'Ostatnio usunięte';

  @override
  String get rulesDeletedDescription => 'Można przywrócić w ciągu 30 dni.';

  @override
  String rulesDeletedAt(String time) {
    return 'Usunięto: $time';
  }

  @override
  String get rulesRestore => 'Przywróć';

  @override
  String get rulesDeletedToast => 'Reguła usunięta';

  @override
  String get rulesDeletedToastMessage => 'Możesz ją przywrócić przez 30 dni.';

  @override
  String get rulesUndo => 'Cofnij';

  @override
  String rulesConflictMessage(String rule, String device) {
    return 'Równoczesna zmiana reguły „$rule” z urządzenia $device została zastąpiona nowszą.';
  }

  @override
  String get rulesAnotherDevice => 'innego urządzenia';

  @override
  String get rulesRestoreConflict => 'Użyj tej wersji';

  @override
  String get rulesSkipLogTitle => 'Ostatnio pominięte';

  @override
  String get rulesSkipLogDescription =>
      'Kopie, których twoje reguły nie zapisały na tym urządzeniu.';

  @override
  String get rulesSkipLogKeywords =>
      'pominięte zignorowane niezapisane dziennik';

  @override
  String get rulesSkipLogShow => 'Pokaż';

  @override
  String get rulesSkipLogEmpty => 'Nie pominięto żadnych kopii.';

  @override
  String get rulesUnknownApp => 'Nieznana aplikacja';

  @override
  String get rulesMatchesEverything => 'Dowolna kopia';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary, potem zatrzymaj';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': ' lub ',
      'other': ' i ',
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
    return 'Działa na: $devices';
  }

  @override
  String rulesNewFromDevice(String device) {
    return 'Nowa z $device';
  }

  @override
  String get rulesNeedsUpdateBadge => 'Wymaga aktualizacji';

  @override
  String get rulesUpdateRequired =>
      'Zaktualizuj CopyPaste, aby uruchomić tę regułę.';

  @override
  String get rulesErrorBadge => 'Błąd';

  @override
  String rulesErrorHint(String problem) {
    return '$problem Do czasu poprawki dopasowania są zapisywane jako tajne.';
  }

  @override
  String get rulesReorder => 'Przeciągnij, aby zmienić kolejność';

  @override
  String get rulesMoreActions => 'Więcej działań';

  @override
  String get rulesEdit => 'Edytuj';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': 'Przenieś w górę',
      'down': 'Przenieś w dół',
      'top': 'Przenieś na początek',
      'other': 'Przenieś na koniec',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => 'Zastosuj do istniejących wpisów…';

  @override
  String get rulesResetBuiltin => 'Przywróć domyślne';

  @override
  String get rulesNameLabel => 'Nazwa';

  @override
  String get rulesNamePlaceholder => 'Na przykład Linki do pracy';

  @override
  String get rulesWhenTitle => 'Kiedy';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'Dowolny warunek spełniony',
      'other': 'Wszystkie warunki spełnione',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions =>
      'Brak warunków: ta reguła dotyczy każdej kopii.';

  @override
  String get rulesAddCondition => 'Dodaj warunek';

  @override
  String get rulesRemoveCondition => 'Usuń warunek';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'Tekst',
      'content_type': 'Typ treści',
      'detected_kind': 'Wykryty rodzaj',
      'source_app': 'Aplikacja źródłowa',
      'size': 'Rozmiar',
      'privacy_marker': 'Znacznik prywatności',
      'capture_origin': 'Pochodzenie',
      'other': 'Nieznany warunek',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'zawiera',
      'equals': 'to',
      'starts_with': 'zaczyna się od',
      'ends_with': 'kończy się na',
      'matches_regex': 'pasuje do wzorca',
      'other': 'pasuje',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'nie zawiera',
      'equals': 'to nie',
      'starts_with': 'nie zaczyna się od',
      'ends_with': 'nie kończy się na',
      'matches_regex': 'nie pasuje do wzorca',
      'other': 'nie pasuje',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'to nie',
      'other': 'to',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'jest mniejszy niż',
      'other': 'jest większy niż',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'nie jest mniejszy niż',
      'other': 'nie jest większy niż',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'wiersze',
      'bytes': 'bajty',
      'other': 'znaki',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'wierszy',
      'bytes': 'bajtów',
      'other': 'znaków',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'Zwykły tekst',
      'rich_text': 'Tekst sformatowany',
      'image': 'Obraz',
      'files': 'Pliki',
      'color': 'Kolor',
      'link': 'Link',
      'other': 'Inne',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'Hasło lub sekret',
      'url': 'Adres internetowy',
      'email': 'Adres e-mail',
      'phone': 'Numer telefonu',
      'code': 'Kod',
      'color': 'Wartość koloru',
      'json': 'JSON',
      'file_path': 'Ścieżka pliku',
      'number': 'Liczba',
      'one_time_code': 'Kod jednorazowy',
      'payment_card': 'Numer karty',
      'other': 'Inne',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': 'Ukryte',
      'transient': 'Tymczasowe',
      'auto_generated': 'Wygenerowane automatycznie',
      'excluded_from_history': 'Wykluczone z historii',
      'other': 'Inne',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'Skopiowane na tym urządzeniu',
      'received': 'Odebrane z innego urządzenia',
      'created_in_app': 'Utworzone w CopyPaste',
      'other': 'Inne',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => 'Szukany tekst';

  @override
  String get rulesPatternPlaceholder => 'Wzorzec, na przykład ^ticket-[0-9]+';

  @override
  String get rulesPatternHint =>
      'Asercje w przód i w tył oraz odwołania wsteczne nie są obsługiwane.';

  @override
  String get rulesCaseSensitive => 'Uwzględniaj wielkość liter';

  @override
  String get rulesUnknownConditionNotice =>
      'Ten warunek pochodzi z nowszej wersji CopyPaste.';

  @override
  String get rulesSourceAppNotice =>
      'Na Androidzie pasują tylko kopie z klawiatury wpisów.';

  @override
  String get rulesPrivacyMarkerNotice =>
      'Nie każdy system zgłasza każdy znacznik.';

  @override
  String get rulesThenTitle => 'Wtedy';

  @override
  String get rulesNoActions => 'Dodaj co najmniej jedną akcję.';

  @override
  String get rulesAddAction => 'Dodaj akcję';

  @override
  String get rulesRemoveAction => 'Usuń akcję';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': 'Nie zapisuj',
      'mark_secret': 'Oznacz jako tajne',
      'delete_after': 'Usuń po',
      'add_tags': 'Dodaj tagi',
      'transform': 'Przekształć',
      'other': 'Nieznana akcja',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone': '„Nie zapisuj” nie można łączyć z innymi akcjami.',
      'present': 'Ta reguła ma już tę akcję.',
      'other': 'Reguła może mieć maksymalnie 8 akcji.',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'Ta akcja pochodzi z nowszej wersji CopyPaste.';

  @override
  String get rulesDontSaveWarning =>
      'Pasujące kopie nigdy nie są zapisywane, indeksowane ani synchronizowane.';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count minuty',
      many: '$count minut',
      few: '$count minuty',
      one: '$count minuta',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count godziny',
      many: '$count godzin',
      few: '$count godziny',
      one: '$count godzina',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dnia',
      many: '$count dni',
      few: '$count dni',
      one: '$count dzień',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry =>
      'Wyczyść też schowek, jeśli nadal zawiera tę kopię.';

  @override
  String get rulesNoTags => 'Utwórz tagi w Historii, aby ich tu użyć.';

  @override
  String get rulesNoTransforms => 'Brak dostępnych przekształceń.';

  @override
  String get rulesReplaceClipboard => 'Zastąp też schowek wynikiem';

  @override
  String get rulesOptionsTitle => 'Opcje';

  @override
  String get rulesEnabledSwitch => 'Włączona';

  @override
  String get rulesStopProcessing =>
      'Nie przetwarzaj kolejnych reguł, gdy ta pasuje';

  @override
  String get rulesRunsOnLabel => 'Działa na';

  @override
  String get rulesAllDevices => 'Wszystkie urządzenia';

  @override
  String get rulesChosenDevices => 'Wybrane urządzenia';

  @override
  String get rulesTestTitle => 'Test';

  @override
  String get rulesSamplePlaceholder => 'Wklej lub wpisz przykładowy tekst';

  @override
  String get rulesTestFullSet => 'Testuj razem ze wszystkimi regułami';

  @override
  String get rulesRunTest => 'Uruchom test';

  @override
  String get rulesTestFailed => 'Nie udało się uruchomić testu.';

  @override
  String get rulesPreviewHistory => 'Podgląd na historii';

  @override
  String get rulesSaveFailed => 'Nie udało się zapisać reguły.';

  @override
  String get rulesReadOnlyNotice =>
      'Utworzona w nowszym CopyPaste. Zaktualizuj, aby ją edytować lub uruchomić.';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'Nie udało się odczytać reguły.',
      'unsupported_schema': 'Ta reguła wymaga nowszej wersji CopyPaste.',
      'unknown_condition_type': 'Ten warunek wymaga nowszej wersji CopyPaste.',
      'unknown_action_type': 'Ta akcja wymaga nowszej wersji CopyPaste.',
      'too_many_conditions': 'Reguła może mieć maksymalnie 20 warunków.',
      'too_many_actions': 'Reguła może mieć maksymalnie 8 akcji.',
      'too_many_rules': 'Działa tylko pierwszych 200 reguł.',
      'no_actions': 'Dodaj co najmniej jedną akcję.',
      'conflicting_actions': '„Nie zapisuj” nie można łączyć z innymi akcjami.',
      'duplicate_action': 'Każdej akcji można użyć tylko raz.',
      'empty_value': 'Wpisz wartość.',
      'empty_list': 'Wybierz co najmniej jedną wartość.',
      'too_many_values': 'Wybierz maksymalnie 64 wartości.',
      'invalid_value': 'Ta wartość jest nieprawidłowa.',
      'pattern_too_long': 'Wzorce mogą mieć do 2000 znaków.',
      'unsupported_look_around':
          'Asercje w przód i w tył nie są obsługiwane. Zamiast tego użyj dwóch warunków z opcją „Wszystkie warunki spełnione”.',
      'unsupported_backreference': 'Odwołania wsteczne nie są obsługiwane.',
      'pattern_too_large':
          'Ten wzorzec jest zbyt złożony. Uprość jego powtórzenia.',
      'invalid_pattern': 'Ten wzorzec jest nieprawidłowy.',
      'invalid_duration': 'Wybierz czas od 1 minuty do 90 dni.',
      'invalid_tags': 'Wybierz co najmniej jeden tag.',
      'invalid_transform': 'Wybierz przekształcenie.',
      'other': 'Ta reguła ma problem.',
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
      'matched': 'pasuje',
      'not_matched': 'nie pasuje',
      'unknown': 'nieznane',
      'skipped': 'nie uruchomiono',
      'other': 'błąd',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return 'Warunek $number';
  }

  @override
  String get rulesTraceNoRules => 'Nie oceniono żadnej reguły.';

  @override
  String get rulesTraceStopped => 'Kolejne reguły nie zostały uruchomione.';

  @override
  String get rulesTraceUnknownReason =>
      'Nieznane dla tej kopii, więc brak dopasowania.';

  @override
  String get rulesTraceRuleError =>
      'Ta reguła nie zadziałała, więc dopasowania są zapisywane jako tajne.';

  @override
  String get rulesTraceOutcomeTitle => 'Wynik';

  @override
  String get rulesTraceNoEffect => 'Żadna reguła nie zmienia tej kopii.';

  @override
  String get rulesTraceSkipped => 'Nie zostanie zapisana';

  @override
  String get rulesTraceSecret => 'Zapisana jako tajna';

  @override
  String rulesTraceExpires(String duration) {
    return 'Usunięta po $duration';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'Tagi: $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return 'Przekształcona: $names';
  }

  @override
  String get rulesTraceOutput => 'Przekształcony tekst';

  @override
  String get rulesPreviewTitle => 'Zastosuj do istniejących wpisów';

  @override
  String rulesPreviewRunning(int count) {
    return 'Sprawdzanie ostatnich $count wpisów…';
  }

  @override
  String get rulesPreviewNone => 'Żaden ostatni wpis nie pasuje do tej reguły.';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Pasuje $count wpisu',
      many: 'Pasuje $count wpisów',
      few: 'Pasują $count wpisy',
      one: 'Pasuje $count wpis',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count zostałoby usunięte',
      many: '$count zostałoby usuniętych',
      few: '$count zostałyby usunięte',
      one: '$count zostałby usunięty',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count zostałoby oznaczone jako tajne',
      many: '$count zostałoby oznaczonych jako tajne',
      few: '$count zostałyby oznaczone jako tajne',
      one: '$count zostałby oznaczony jako tajny',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count otrzymałoby czas usunięcia',
      many: '$count otrzymałoby czas usunięcia',
      few: '$count otrzymałyby czas usunięcia',
      one: '$count otrzymałby czas usunięcia',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count zostałoby otagowane',
      many: '$count zostałoby otagowanych',
      few: '$count zostałyby otagowane',
      one: '$count zostałby otagowany',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return 'Sprawdzono tylko ostatnie $count wpisów.';
  }

  @override
  String rulesPreviewMore(int count) {
    return 'i $count więcej';
  }

  @override
  String get rulesPreviewNote => 'Nic się nie zmieni, dopóki nie zastosujesz.';

  @override
  String get rulesSecretItem => 'Tajny wpis';

  @override
  String get rulesPreviewCancelled =>
      'Podgląd zatrzymano. Nic się nie zmieniło.';

  @override
  String get rulesPreviewFailed => 'Nie udało się uruchomić podglądu.';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Usunąć $count wpisu?',
      many: 'Usunąć $count wpisów?',
      few: 'Usunąć $count wpisy?',
      one: 'Usunąć $count wpis?',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage =>
      'Pasujące wpisy zostaną usunięte na wszystkich urządzeniach. Tego nie można cofnąć.';

  @override
  String get rulesApplyDeleteConfirm => 'Usuń i zastosuj';

  @override
  String rulesApplyProgress(int done, int total) {
    return 'Stosowanie: $done z $total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Zmieniono $count wpisu',
      many: 'Zmieniono $count wpisów',
      few: 'Zmieniono $count wpisy',
      one: 'Zmieniono $count wpis',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Usunięto $count wpisu',
      many: 'Usunięto $count wpisów',
      few: 'Usunięto $count wpisy',
      one: 'Usunięto $count wpis',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Nie udało się zmienić $count wpisu',
      many: 'Nie udało się zmienić $count wpisów',
      few: 'Nie udało się zmienić $count wpisów',
      one: 'Nie udało się zmienić $count wpisu',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled =>
      'Zatrzymano. Dotychczasowe zmiany pozostają.';

  @override
  String get historyActionEdit => 'Edytuj';

  @override
  String get historyActionAddToQueue => 'Dodaj do Paste Queue';

  @override
  String get historyActionSendTo => 'Wyślij do…';

  @override
  String get historySendDialogTitle => 'Wyślij do urządzenia';

  @override
  String get historySendDescription =>
      'Wysyła od razu, nawet gdy automatyczna synchronizacja jest wstrzymana.';

  @override
  String get historySendNoDevices =>
      'Żadne sparowane urządzenie nie zostało jeszcze zsynchronizowane.';

  @override
  String get historySendAction => 'Wyślij';

  @override
  String get historySentTitle => 'Wysłano';

  @override
  String get historySendFailedTitle => 'Nie wysłano';

  @override
  String historySendBlocked(String device) {
    return 'Ustawienia synchronizacji urządzenia $device nie zezwalają na ten wpis.';
  }

  @override
  String historySendFailed(String device) {
    return '$device nie odebrało wpisu. Sprawdź, czy jest online, i spróbuj ponownie.';
  }

  @override
  String get historySendUnavailable => 'Wysyłanie jest teraz niedostępne.';

  @override
  String get historyActionTransform => 'Przekształć…';

  @override
  String get historyActionExplain => 'Dlaczego to tu jest?';

  @override
  String get historyActionKeep => 'Zachowaj';

  @override
  String get historyNewClip => 'Nowy wpis';

  @override
  String get historySavedTitle => 'Zapisano';

  @override
  String get historyEditSaved =>
      'Edytowany tekst jest nowym wpisem na początku historii.';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Dodano $count wpisu do kolejki.',
      many: 'Dodano $count wpisów do kolejki.',
      few: 'Dodano $count wpisy do kolejki.',
      one: 'Dodano $count wpis do kolejki.',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed =>
      'Nie można dodać do Paste Queue. Sprawdź, czy jest włączona.';

  @override
  String get historyKeepFailed => 'Nie można zachować tego wpisu.';

  @override
  String get historyExplainFailed => 'Nie można wczytać wyjaśnienia.';

  @override
  String get historyDeletesSoon => 'Usunięcie za mniej niż minutę';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Usunięcie za $count min',
      many: 'Usunięcie za $count min',
      few: 'Usunięcie za $count min',
      one: 'Usunięcie za $count min',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Usunięcie za $count godz.',
      many: 'Usunięcie za $count godz.',
      few: 'Usunięcie za $count godz.',
      one: 'Usunięcie za $count godz.',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Usunięcie za $count dnia',
      many: 'Usunięcie za $count dni',
      few: 'Usunięcie za $count dni',
      one: 'Usunięcie za $count dzień',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => 'Ręcznie';

  @override
  String get historyProvenanceEdited => 'Edytowany';

  @override
  String get historyProvenanceTransformed => 'Przekształcony';

  @override
  String get historyProvenanceModule => 'Moduł';

  @override
  String historyRuleTag(String name) {
    return '$name (auto)';
  }

  @override
  String get historyMetadataExpiry => 'Usunięcie';

  @override
  String get historyMetadataRuleTags => 'Tagi automatyczne';

  @override
  String modulesEnableTitle(String module) {
    return 'Włączyć moduł $module?';
  }

  @override
  String get modulesEnableAccess => 'Moduł będzie mógł:';

  @override
  String modulesCapability(String capability) {
    String _temp0 = intl.Intl.selectLogic(capability, {
      'history_read': 'Czytać historię schowka',
      'clipboard_write': 'Kopiować do schowka',
      'library_write': 'Zapisywać fragmenty i przypinać elementy',
      'snippets': 'Czytać Twoje fragmenty',
      'synced_records': 'Synchronizować swoje dane między urządzeniami',
      'local_state': 'Przechowywać swoje dane na tym urządzeniu',
      'prompts': 'Prosić o potwierdzenie działań',
      'endpoints': 'Przyjmować połączenia z lokalnych aplikacji',
      'clipboard_read': 'Czytać tekst ze schowka',
      'other': 'Korzystać z usługi CopyPaste',
    });
    return '$_temp0';
  }

  @override
  String modulesPromptFrom(String module) {
    return 'Od $module';
  }

  @override
  String get modulesUnknownModule => 'Nieznany moduł';

  @override
  String get modulesPromptAnswerFailed => 'Nie wysłano odpowiedzi.';

  @override
  String get modulesPromptNotificationBody =>
      'Otwórz CopyPaste, aby odpowiedzieć.';

  @override
  String get modulesUnknownDevice => 'Nieznane urządzenie';

  @override
  String get modulesChooseDevices => 'Wybierz urządzenia';

  @override
  String get modulesAddApp => 'Dodaj aplikację';

  @override
  String modulesRemoveApp(String app) {
    return 'Usuń $app';
  }

  @override
  String get modulesNoApps => 'Nie wybrano aplikacji';

  @override
  String get settingsSectionDiagnostics => 'Diagnostyka';

  @override
  String get settingsSectionDiagnosticsSummary =>
      'Dzienniki działania do rozwiązywania problemów. Zawartość schowka nigdy nie jest rejestrowana.';

  @override
  String get diagnosticsSearchPlaceholder => 'Szukaj w dziennikach';

  @override
  String get diagnosticsAllLevels => 'Wszystkie poziomy';

  @override
  String get diagnosticsAllProcesses => 'Wszystkie procesy';

  @override
  String get diagnosticsLevelError => 'Błąd';

  @override
  String get diagnosticsLevelWarning => 'Ostrzeżenie';

  @override
  String get diagnosticsLevelInfo => 'Informacja';

  @override
  String get diagnosticsLevelDebug => 'Debugowanie';

  @override
  String get diagnosticsLevelTrace => 'Śledzenie';

  @override
  String get diagnosticsProcessApp => 'Aplikacja';

  @override
  String get diagnosticsProcessDaemon => 'Usługa w tle';

  @override
  String get diagnosticsRefresh => 'Odśwież';

  @override
  String get diagnosticsExport => 'Eksportuj';

  @override
  String get diagnosticsExportFileType => 'Plik tekstowy';

  @override
  String get diagnosticsExported => 'Wyeksportowano diagnostykę';

  @override
  String get diagnosticsExportFailed => 'Eksport nie powiódł się';

  @override
  String get diagnosticsExportFailedMessage =>
      'Nie udało się zapisać pliku dziennika.';

  @override
  String get diagnosticsLoadFailed => 'Nie udało się wczytać dzienników';

  @override
  String get diagnosticsEmpty => 'Brak wpisów w dzienniku';

  @override
  String get diagnosticsLogLevelTitle => 'Poziom logowania';

  @override
  String get diagnosticsLogLevelDescription =>
      'Ile szczegółów jest zapisywanych na tym urządzeniu.';

  @override
  String get onboardingAndroidCaptureMethod => 'Metoda wykrywania';

  @override
  String get onboardingAndroidLogTrigger => 'Dziennik systemowy (zalecane)';

  @override
  String get onboardingAndroidLogTriggerDescription =>
      'Natychmiast, bez dodatkowego zużycia baterii. Wymaga jednorazowego dostępu przez Shizuku lub ADB.';

  @override
  String get onboardingAndroidPolling => 'Odpytywanie';

  @override
  String get onboardingAndroidPollingDescription =>
      'Sprawdza co sekundę, gdy ekran jest włączony. Wymaga tylko dostępu do nakładki; zużywa więcej baterii.';

  @override
  String get onboardingAndroidOverlay => 'Wyświetlanie nad innymi aplikacjami';

  @override
  String get onboardingAndroidOverlayDescription =>
      'Pozwala niewidocznemu oknu 1 piksela odczytywać schowek.';

  @override
  String get onboardingAndroidOverlaySettingsFailed =>
      'Nie udało się otworzyć ustawień nakładki.';

  @override
  String get settingsBackgroundSyncTitle => 'Synchronizacja w tle';

  @override
  String get settingsBackgroundSyncDescription =>
      'Synchronizuj, gdy aplikacja jest zamknięta. Wyświetla powiadomienie.';

  @override
  String get settingsBackgroundSyncKeywords =>
      'tło stała usługa powiadomienie bateria keep alive persistent service notification battery';

  @override
  String get settingsBackgroundSyncStoppedTitle =>
      'Synchronizacja w tle nie działa';

  @override
  String get settingsBackgroundSyncStoppedMessage =>
      'Zezwól na powiadomienia, aby Android utrzymywał synchronizację.';

  @override
  String get settingsBackgroundSyncResume => 'Wznów synchronizację';

  @override
  String get quickAccessSendShortcutTitle => 'Skrót „Wyślij schowek”';

  @override
  String get quickAccessSendShortcutDescription =>
      'Zapisuje bieżący schowek i wysyła go na Twoje urządzenia.';

  @override
  String get quickAccessTransformShortcutTitle => 'Skrót „Przekształć schowek”';

  @override
  String get quickAccessTransformShortcutDescription =>
      'Otwiera przekształcenia bieżącego schowka.';

  @override
  String get quickAccessAddToHomeScreen => 'Dodaj do ekranu głównego';

  @override
  String get quickAccessShortcutKeywords =>
      'skrót ekran główny launcher ikona shortcut home screen launcher icon';

  @override
  String fileSetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pliku',
      many: '$count plików',
      few: '$count pliki',
      one: '$count plik',
    );
    return '$_temp0';
  }

  @override
  String get fileSetShowFolder => 'Pokaż folder';

  @override
  String get transfersResume => 'Wznów';

  @override
  String get settingsStartInBackgroundTitle => 'Uruchamiaj w tle';

  @override
  String get settingsStartInBackgroundDescription =>
      'Gdy CopyPaste uruchamia się po zalogowaniu, okno pozostaje zamknięte.';

  @override
  String get settingsWindowGlassTitle => 'Efekt szkła';

  @override
  String get settingsWindowGlassDescription =>
      'Rozmywa pulpit za oknem. Wyłącza się, gdy system ogranicza przezroczystość.';

  @override
  String get settingsRestoreClipboardTitle => 'Przywracaj schowek przy starcie';

  @override
  String get settingsRestoreClipboardDescription =>
      'Przy starcie wstawia najnowszy wpis do pustego schowka. Wpisy tajne są pomijane.';

  @override
  String get settingsStorageUsageTitle => 'Wykorzystanie miejsca';

  @override
  String get settingsStorageUsageDescription =>
      'Miejsce zajmowane przez CopyPaste na tym urządzeniu.';

  @override
  String get settingsStorageUsageUnavailable =>
      'Nie udało się odczytać wykorzystania miejsca.';

  @override
  String get settingsStorageHistory => 'Historia';

  @override
  String get settingsStorageFiles => 'Pliki';

  @override
  String get settingsStorageCaches => 'Pamięć podręczna';

  @override
  String get settingsStorageUpgradeBackups => 'Kopie przed aktualizacją';

  @override
  String get settingsStorageOther => 'Logi, moduły i ustawienia';

  @override
  String get settingsStorageTotal => 'Razem';

  @override
  String get settingsClearCacheTitle => 'Wyczyść pamięć podręczną';

  @override
  String get settingsClearCacheDescription =>
      'Usuwa podglądy i pliki tymczasowe. Historia zostaje.';

  @override
  String get settingsClearCacheAction => 'Wyczyść';

  @override
  String get settingsClearCacheDoneTitle => 'Wyczyszczono pamięć podręczną';

  @override
  String get settingsClearCacheDoneMessage =>
      'Podglądy są odtwarzane w razie potrzeby.';

  @override
  String get settingsClearCacheFailed =>
      'Nie udało się wyczyścić pamięci podręcznej.';

  @override
  String get settingsDataFolderTitle => 'Folder danych';

  @override
  String get settingsDataFolderDescription =>
      'Pokazuje, gdzie są historia, kopie i moduły.';

  @override
  String get settingsLogsFolderTitle => 'Folder logów';

  @override
  String get settingsOpenFolderAction => 'Otwórz';

  @override
  String get settingsOpenFolderFailed => 'Nie udało się otworzyć folderu.';

  @override
  String get settingsConfirmDeleteTitle => 'Potwierdzaj usuwanie';

  @override
  String get settingsConfirmDeleteDescription =>
      'Pytaj przed usunięciem jednego wpisu. Usuwanie kilku zawsze pyta.';

  @override
  String get settingsUpgradeBackupsTitle => 'Kopie przed aktualizacją';

  @override
  String get settingsUpgradeBackupsDescription =>
      'Zaszyfrowane kopie sprzed aktualizacji danych. Zachowywanych jest 5 ostatnich.';

  @override
  String get settingsUpgradeBackupsEmpty => 'Brak kopii przed aktualizacją.';

  @override
  String settingsUpgradeBackupEntry(String date, String size) {
    return '$date · $size';
  }

  @override
  String get settingsUpgradeBackupRestoreAction => 'Przywróć';

  @override
  String get settingsUpgradeBackupRestoreTitle => 'Przywrócić tę kopię?';

  @override
  String settingsUpgradeBackupRestoreMessage(String date) {
    return 'Historia zostanie zastąpiona kopią z $date. Wpisy dodane później zostaną usunięte.';
  }

  @override
  String get settingsDownloadUpdatesTitle => 'Pobieraj aktualizacje w tle';

  @override
  String get settingsDownloadUpdatesDescription =>
      'Pobiera i weryfikuje znalezioną aktualizację, by instalacja była natychmiastowa. Czeka w sieciach z limitem i w trybie oszczędzania energii.';

  @override
  String settingsUpdateDownloadedReady(String available) {
    return 'CopyPaste $available jest pobrany i gotowy do instalacji.';
  }

  @override
  String get devicesSpaceTitle => 'Twoja przestrzeń';

  @override
  String devicesSpaceSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count urządzenia',
      many: '$count urządzeń',
      few: '$count urządzenia',
      one: 'Tylko to urządzenie',
    );
    return '$_temp0';
  }

  @override
  String get devicesSpaceKeyPending =>
      'Urządzenia offline otrzymają nowy klucz, gdy będą online.';

  @override
  String get devicesSpaceWaitingForKey => 'Wymaga nowego klucza';

  @override
  String get devicesSpaceRemovedTitle => 'Poza przestrzenią';

  @override
  String get devicesSpaceRemovedMessage =>
      'To urządzenie zostało usunięte lub opuściło przestrzeń. Jego historia zostaje tutaj.';

  @override
  String get devicesSpaceJoin => 'Dołącz do przestrzeni';

  @override
  String get devicesRemoveFromSpace => 'Usuń z przestrzeni';

  @override
  String devicesRemoveDialogTitle(String device) {
    return 'Usunąć $device?';
  }

  @override
  String get devicesRemoveDialogMessage =>
      'Zostanie usunięte ze wszystkich Twoich urządzeń i nie odczyta niczego, co zostanie zsynchronizowane od teraz.';

  @override
  String get devicesRemoveNewPassphrase =>
      'Nowe hasło synchronizacji (opcjonalnie)';

  @override
  String get devicesRemoveNewPassphraseHint =>
      'Usunięte urządzenie zna obecne hasło. Bez nowego dodawaj urządzenia tylko przez zaproszenie.';

  @override
  String get devicesRemoveSignOutOthers =>
      'Wyloguj inne urządzenia z konta w chmurze';

  @override
  String get devicesPassphraseTooShort => 'Użyj co najmniej 12 znaków.';

  @override
  String get devicesLeaveSpace => 'Opuść przestrzeń';

  @override
  String get devicesLeaveDialogTitle => 'Opuścić tę przestrzeń?';

  @override
  String get devicesLeaveDialogMessage =>
      'Twoje pozostałe urządzenia usuną to urządzenie. Jego historia zostaje tutaj.';

  @override
  String get devicesLeave => 'Opuść';

  @override
  String get onboardingSpaceTitle => 'Twoje urządzenia';

  @override
  String get onboardingSpaceSubtitle =>
      'Synchronizuj schowek między swoimi urządzeniami. Wszystko pozostaje zaszyfrowane.';

  @override
  String get onboardingSpaceCreateTitle => 'Utwórz przestrzeń';

  @override
  String get onboardingSpaceCreateDescription =>
      'To moje pierwsze urządzenie z CopyPaste.';

  @override
  String get onboardingSpaceJoinTitle => 'Dołącz do przestrzeni';

  @override
  String get onboardingSpaceJoinDescription =>
      'Używam już CopyPaste na innym urządzeniu.';

  @override
  String get onboardingSpaceLater => 'Później';

  @override
  String get onboardingReadySpaceCreated =>
      'Dodaj pozostałe urządzenia teraz lub później w sekcji Urządzenia.';

  @override
  String get onboardingReadyJoin =>
      'Teraz potwierdź kod wyświetlony na drugim urządzeniu.';

  @override
  String get onboardingAddDeviceNow => 'Dodaj urządzenie';

  @override
  String get onboardingJoinNow => 'Dołącz teraz';

  @override
  String get onboardingBadgeWelcome => 'Witaj';

  @override
  String get onboardingBadgeSetup => 'Konfiguracja';

  @override
  String get onboardingBadgeDevices => 'Urządzenia';

  @override
  String get onboardingBadgeReady => 'Gotowe';

  @override
  String get onboardingPairJoinTitle => 'Dołącz do swojej przestrzeni';

  @override
  String get onboardingPairAddDeviceSubtitle =>
      'Na drugim urządzeniu otwórz CopyPaste i dołącz za pomocą tego kodu lub kodu QR.';

  @override
  String get onboardingPairSkip => 'Później';

  @override
  String get onboardingPairInterruptedTitle =>
      'Parowanie nie zostało ukończone';

  @override
  String get onboardingPairInterruptedMessage =>
      'Spróbuj ponownie albo kontynuuj i sparuj później w sekcji Urządzenia.';

  @override
  String get onboardingReadyPaired =>
      'Schowek synchronizuje się teraz między Twoimi urządzeniami.';

  @override
  String get onboardingReadyPairLater =>
      'Urządzenia możesz sparować w dowolnej chwili w sekcji Urządzenia.';

  @override
  String get settingsSectionShortcuts => 'Skróty';

  @override
  String get settingsSectionShortcutsSummary =>
      'Globalne skróty klawiaturowe. Wyłącz te, których nie używasz.';

  @override
  String get settingsShortcutEnabledLabel => 'Wyłącz ten skrót';

  @override
  String get settingsWelcomeTourTitle => 'Przewodnik powitalny';

  @override
  String get settingsWelcomeTourDescription =>
      'Przejdź ponownie konfigurację początkową.';

  @override
  String get settingsWelcomeTourAction => 'Pokaż ponownie';

  @override
  String get modulesDetails => 'Szczegóły';

  @override
  String get modulesDetailsFeatures => 'Co dodaje';

  @override
  String get modulesDetailsSearch =>
      'Wyszukuje wycinki według znaczenia lokalnym modelem.';

  @override
  String get modulesDetailsPermissions => 'Uprawnienia';

  @override
  String get modulesDetailsPermissionsPending =>
      'Widoczne po instalacji. Potwierdzasz je przy włączaniu modułu.';

  @override
  String get modulesDetailsNoPermissions => 'Nie wymaga uprawnień.';

  @override
  String get modulesDetailsInformation => 'Informacje';

  @override
  String get modulesDetailsVersion => 'Wersja';

  @override
  String get modulesDetailsSize => 'Rozmiar';

  @override
  String get modulesDetailsRequirements => 'Wymagania';

  @override
  String get modulesDetailsStatus => 'Stan';

  @override
  String get errorUnexpected => 'Coś poszło nie tak. Spróbuj ponownie.';

  @override
  String get errorRuntimeUnavailable =>
      'CopyPaste nie działa w tle. Uruchom aplikację ponownie.';

  @override
  String get errorRuntimeStartFailed =>
      'CopyPaste nie może uruchomić usługi w tle. Uruchom aplikację ponownie.';

  @override
  String get errorRuntimeStarting =>
      'CopyPaste jeszcze się uruchamia. Spróbuj za chwilę.';

  @override
  String get errorRuntimeTimeout =>
      'CopyPaste nie odpowiedział na czas. Spróbuj ponownie.';

  @override
  String get errorRuntimeMismatch =>
      'CopyPaste został zaktualizowany. Uruchom aplikację ponownie, aby zakończyć.';

  @override
  String get errorPeerUnreachable =>
      'Nie można połączyć się z drugim urządzeniem. Sprawdź, czy jest online.';

  @override
  String get errorPeerVersion =>
      'Drugie urządzenie ma niezgodną wersję CopyPaste. Zaktualizuj oba urządzenia.';

  @override
  String get errorRateLimited =>
      'Zbyt wiele prób. Odczekaj chwilę i spróbuj ponownie.';

  @override
  String devicesSyncFailedWith(String devices) {
    return 'Nie udało się zsynchronizować z: $devices.';
  }

  @override
  String get transfersUnavailableTitle => 'Nie udało się wczytać transferów';

  @override
  String get historyTagsLoadFailedTitle => 'Nie udało się wczytać tagów';

  @override
  String get settingsEraseAllDataTitle => 'Usuń wszystkie dane';

  @override
  String get settingsEraseAllDataDescription =>
      'Usuwa historię, ustawienia i klucz szyfrowania, a potem zamyka CopyPaste.';

  @override
  String get settingsEraseAllDataAction => 'Usuń wszystko';

  @override
  String get settingsEraseAllDataConfirmTitle =>
      'Usunąć wszystkie dane CopyPaste?';

  @override
  String get settingsEraseAllDataConfirmMessage =>
      'Historia, pliki, ustawienia i klucz szyfrowania na tym urządzeniu zostaną usunięte, a CopyPaste się zamknie. Sparowane urządzenia i kopie w chmurze pozostaną. Tej operacji nie można cofnąć.';

  @override
  String get settingsEraseAllDataFailed =>
      'Nie udało się usunąć części danych CopyPaste.';
}
