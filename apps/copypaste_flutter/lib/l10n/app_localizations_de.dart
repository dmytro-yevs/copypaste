// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for German (`de`).
class AppLocalizationsDe extends AppLocalizations {
  AppLocalizationsDe([String locale = 'de']) : super(locale);

  @override
  String get commonCancel => 'Abbrechen';

  @override
  String get commonClose => 'Schließen';

  @override
  String get commonDone => 'Fertig';

  @override
  String get commonRetry => 'Wiederholen';

  @override
  String get commonTryAgain => 'Erneut versuchen';

  @override
  String get commonCopy => 'Kopieren';

  @override
  String get commonDelete => 'Löschen';

  @override
  String get commonSave => 'Speichern';

  @override
  String get commonContinue => 'Weiter';

  @override
  String get commonBack => 'Zurück';

  @override
  String get commonOpenSettings => 'Einstellungen öffnen';

  @override
  String get commonLoading => 'Wird geladen';

  @override
  String get commonApply => 'Anwenden';

  @override
  String get commonClear => 'Leeren';

  @override
  String get commonPin => 'Anheften';

  @override
  String get commonEnable => 'Aktivieren';

  @override
  String get settingsLanguageTitle => 'Sprache';

  @override
  String get settingsLanguageDescription =>
      'Sprache der CopyPaste-Oberfläche wählen.';

  @override
  String get settingsLanguageSystem => 'Systemstandard';

  @override
  String get settingsLanguageKeywords =>
      'Sprache Gebietsschema Übersetzung Oberfläche language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Start bei Anmeldung konnte nicht geändert werden. Erneut versuchen.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'Die Systemeinstellungen für den Start konnten nicht geöffnet werden.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Einstellungen konnten nicht aktualisiert werden. Erneut versuchen.';

  @override
  String get settingsRuntimeLoadFailed =>
      'Einstellungen konnten nicht geladen werden.';

  @override
  String get settingsRuntimeAppAlreadyExcluded =>
      'Diese App ist bereits ausgeschlossen.';

  @override
  String get settingsRuntimeChangeFailed =>
      'Die Änderung konnte nicht ausgeführt werden.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Die Erfassung der Zwischenablage kann erst fortgesetzt werden, wenn alle Berechtigungen erteilt sind.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'Die Erfassung der Zwischenablage ist pausiert. Der Android-Erfassungsdienst hat das Beenden nicht bestätigt.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'Für Erfassungsbenachrichtigungen ist die Berechtigung für Benachrichtigungen erforderlich.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste konnte die gespeicherten Einstellungen nicht lesen. Sichere Standardwerte sind aktiv; zur Reparatur die Einstellungen prüfen und speichern.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste hat unlesbare Einstellungen durch sichere Standardwerte ersetzt. Zur Reparatur die Einstellungen prüfen und speichern.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'App-Kennungen dürfen nicht leer und höchstens 256 Byte lang sein.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'Es können bis zu 256 Apps ausgeschlossen werden.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Automatisches Einfügen konnte nicht geändert werden.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'Dieses Tastenkürzel konnte nicht registriert werden.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'Das aktuelle Tastenkürzel konnte nicht pausiert werden.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'Das Tastenkürzel konnte nicht wiederhergestellt werden.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'Quick Paste konnte nicht geöffnet werden.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Die Berechtigung für Bedienungshilfen konnte nicht angefordert werden.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Der Status der Linux-Integration konnte nicht aktualisiert werden.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'Die Berechtigung zur Eingabe aus der Zwischenablage wurde nicht erteilt.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Die Berechtigung zur Eingabe aus der Zwischenablage konnte nicht angefordert werden.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Die Einrichtung der Desktop-Integration konnte nicht geöffnet werden.';

  @override
  String get settingsRuntimeTextHistoryFileType => 'CopyPaste-Textverlauf';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'Verschlüsseltes CopyPaste-Backup';

  @override
  String get settingsRuntimeNotificationImage => 'Bild';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Bild · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'Datei';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste konnte die Update-Installation nicht wiederherstellen.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Diese Linux-Installation kann nicht automatisch aktualisiert werden.';

  @override
  String get updateMacosPackageMissing =>
      'Diese Version hat kein signiertes macOS-Update-Paket.';

  @override
  String get updateInstalledVersionInvalid =>
      'Die installierte CopyPaste-Version ist ungültig.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste konnte GitHub nicht erreichen, um nach Updates zu suchen.';

  @override
  String get updateCheckTimedOut =>
      'Zeitüberschreitung bei der Suche nach Updates.';

  @override
  String get updateCheckDeviceFailed =>
      'CopyPaste konnte auf diesem Gerät nicht nach Updates suchen.';

  @override
  String get updateCheckFailed => 'CopyPaste konnte nicht nach Updates suchen.';

  @override
  String get updateDownloadFailed =>
      'CopyPaste konnte das Update nicht herunterladen.';

  @override
  String get updateDownloadTimedOut =>
      'Zeitüberschreitung beim Herunterladen des Updates.';

  @override
  String get updateInstallFailed =>
      'CopyPaste konnte das Update nicht installieren.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Im Android-Installationsprogramm fortfahren.';

  @override
  String get updateContinueInPackageManager =>
      'In der Paketverwaltung des Systems fortfahren.';

  @override
  String get updateInstallerStarting =>
      'Das Installationsprogramm wird gestartet.';

  @override
  String get updatePermissionRequiredAndroid =>
      'CopyPaste in den Android-Einstellungen das Installieren von Apps erlauben, dann fortfahren.';

  @override
  String get updatePermissionRequiredLinux =>
      'In der Paketverwaltung des Systems authentifizieren, um dieses Update zu installieren.';

  @override
  String get updatePermissionRequired =>
      'CopyPaste das Installieren dieses Updates erlauben, dann fortfahren.';

  @override
  String get updateRestartRequired =>
      'Das Update ist installiert. CopyPaste neu starten, um es zu verwenden.';

  @override
  String get updateRestarting => 'CopyPaste wird neu gestartet.';

  @override
  String get updateRestartFailed =>
      'CopyPaste konnte nicht neu gestartet werden. Erneut versuchen.';

  @override
  String get updateReleasePageOpenFailed =>
      'CopyPaste konnte die Release-Seite nicht öffnen.';

  @override
  String get updatePackageInvalid =>
      'Das heruntergeladene Update ist kein gültiges CopyPaste-Paket.';

  @override
  String get updateSignatureInvalid =>
      'Das heruntergeladene Update hat eine ungültige Signatur.';

  @override
  String get updateDowngradeRefused =>
      'Das heruntergeladene Paket ist nicht neuer als diese Version.';

  @override
  String get updateHomebrewRequired =>
      'CopyPaste mit Homebrew installieren, um es hier zu aktualisieren.';

  @override
  String get updateInstallationCancelled =>
      'Die Update-Installation wurde abgebrochen.';

  @override
  String get updateInstallationBlocked =>
      'Android hat die Update-Installation blockiert.';

  @override
  String get updateInstallationConflict =>
      'Das Update steht in Konflikt mit der installierten App.';

  @override
  String get updateInstallationIncompatible =>
      'Das Update ist mit diesem Gerät nicht kompatibel.';

  @override
  String get updateInstallationStorage =>
      'Speicherplatz freigeben, um das Update zu installieren.';

  @override
  String get updateInstallationInterrupted =>
      'Die Update-Installation wurde unterbrochen. Erneut versuchen.';

  @override
  String get updateInstallationBusy =>
      'Eine Update-Installation läuft bereits.';

  @override
  String get updatePackageDetailsInvalid =>
      'Die Angaben zum Update-Paket sind ungültig.';

  @override
  String get updateIntegrityCheckFailed =>
      'Das heruntergeladene Update hat die Integritätsprüfung nicht bestanden.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste konnte das System-Installationsprogramm nicht starten.';

  @override
  String get updateInstallLocationTranslocated =>
      'CopyPaste in den Ordner „Programme“ verschieben und von dort öffnen, um es zu aktualisieren.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste kann sich im aktuellen Ordner nicht ersetzen. Zum Aktualisieren die Release-Seite verwenden.';

  @override
  String get updateReplacementBlocked =>
      'macOS hat das Ersetzen von CopyPaste blockiert. In den Einstellungen unter „App-Verwaltung“ erlauben oder die Release-Seite verwenden.';

  @override
  String get updatePackageNotDownloadable =>
      'Diese Plattform verwendet kein herunterladbares Update-Paket.';

  @override
  String get updatePackageSizeInvalid =>
      'Die Größe des Update-Pakets ist ungültig.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'Das heruntergeladene Update hat eine ungültige Updater-Signatur.';

  @override
  String get updatePackageUrlUntrusted =>
      'Die URL des Update-Pakets ist nicht vertrauenswürdig.';

  @override
  String get updateDownloadLeftGitHub =>
      'Der Update-Download hat GitHub verlassen.';

  @override
  String get updateDownloadTooManyRedirects =>
      'Der Update-Download wurde zu oft umgeleitet.';

  @override
  String get updatePackageTooLarge => 'Das Update-Paket ist zu groß.';

  @override
  String get updateSignatureSizeInvalid =>
      'Die Größe der Updater-Signatur ist ungültig.';

  @override
  String get updateMetadataInvalid =>
      'GitHub hat ungültige Update-Metadaten geliefert.';

  @override
  String get updateMetadataTooLarge => 'Die Update-Metadaten sind zu groß.';

  @override
  String get quickPasteActionFailedTitle => 'Aktion fehlgeschlagen';

  @override
  String get quickPasteOpenAppSemantics => 'CopyPaste öffnen';

  @override
  String get quickPasteSearchPlaceholder => 'Suchen …';

  @override
  String get quickPasteLoadingHistory =>
      'Verlauf der Zwischenablage wird geladen.';

  @override
  String get quickPasteHistoryUnavailable => 'Verlauf nicht verfügbar';

  @override
  String get quickPasteNoClipsFound => 'Keine Einträge gefunden';

  @override
  String get quickPasteLoadMore => 'Mehr laden';

  @override
  String get quickPasteTransferFailed => 'Fehlgeschlagen';

  @override
  String get quickPasteClipUnavailable => 'Eintrag nicht verfügbar';

  @override
  String get quickPasteFileFallbackName => 'Datei';

  @override
  String get quickPastePathUnavailable => 'Pfad nicht verfügbar';

  @override
  String get quickPastePreferences => 'Einstellungen …';

  @override
  String get quickPasteAbout => 'Info';

  @override
  String get quickPasteQuit => 'Beenden';

  @override
  String get quickPasteClearHistoryTitle => 'Verlauf löschen?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Angeheftete Einträge bleiben im Verlauf.';

  @override
  String get quickPasteClearHistoryFailed =>
      'Der Verlauf konnte nicht gelöscht werden.';

  @override
  String get quickPasteAboutDescription =>
      'Privater Verlauf der Zwischenablage für macOS, Android, Windows und Linux.';

  @override
  String get transfersTitle => 'Downloads und Übertragungen';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count aktive Übertragungen',
      one: '$count aktive Übertragung',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count aktiv',
      one: '$count aktiv',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Neueste';

  @override
  String get transfersShowEarlier => 'Ältere';

  @override
  String get transfersClearCompletedHistory => 'Abgeschlossene entfernen';

  @override
  String get transfersErrorTitle => 'Übertragungen';

  @override
  String get transfersEmptyTitle => 'Keine Übertragungen';

  @override
  String get transfersEmptyMessage =>
      'Dateiimporte, Downloads und ausgehende Übertragungen erscheinen hier.';

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
    return '$percent %';
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
  String get transfersRemoveFromHistory => 'Aus Verlauf entfernen';

  @override
  String get transfersCancelDownload => 'Download abbrechen';

  @override
  String get transfersShowFile => 'Datei anzeigen';

  @override
  String get transfersPhaseSending => 'Wird gesendet';

  @override
  String get transfersPhaseDownloading => 'Wird heruntergeladen';

  @override
  String get transfersPhaseAdding => 'Wird hinzugefügt';

  @override
  String get transfersPhaseAdded => 'Hinzugefügt';

  @override
  String get transfersPhaseSent => 'Gesendet';

  @override
  String get transfersPhaseCompleted => 'Abgeschlossen';

  @override
  String get transfersPhaseCancelled => 'Abgebrochen';

  @override
  String get transfersPhaseInterrupted => 'Unterbrochen';

  @override
  String get transfersPhaseFailed => 'Fehlgeschlagen';

  @override
  String get transfersUpdatesUnavailable =>
      'Übertragungsstatus nicht verfügbar. Zum Aktualisieren erneut öffnen.';

  @override
  String get transfersLoadFailed =>
      'Übertragungen konnten nicht geladen werden. Erneut versuchen.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Der Übertragungsstatus konnte nicht geladen werden.';

  @override
  String get transfersRetryFailed =>
      'Der Download dieser Datei konnte nicht wiederholt werden.';

  @override
  String get transfersClearHistoryFailed =>
      'Der Übertragungsverlauf konnte nicht gelöscht werden.';

  @override
  String get transfersRemoveRecordFailed =>
      'Dieser Verlaufseintrag konnte nicht entfernt werden.';

  @override
  String get transfersCancelFailed =>
      'Die Übertragung konnte nicht abgebrochen werden und läuft möglicherweise noch.';

  @override
  String get transfersSavedFileUnavailable =>
      'Die gespeicherte Datei ist nicht verfügbar und wurde möglicherweise verschoben oder gelöscht.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Benachrichtigungen · $count ungelesen',
      one: 'Benachrichtigungen · $count ungelesen',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Benachrichtigungen';

  @override
  String get notificationsClearRead => 'Gelesene entfernen';

  @override
  String get notificationsEmptyTitle => 'Keine Benachrichtigungen';

  @override
  String get notificationsEmptyMessage =>
      'Verbindungsanfragen von Geräten und ihre Ergebnisse erscheinen hier.';

  @override
  String get notificationsSystemAlertTitle => 'Systembenachrichtigungen';

  @override
  String get notificationsReview => 'Prüfen';

  @override
  String get notificationsConnectionRequest => 'Verbindungsanfrage';

  @override
  String get notificationsConfirmConnection => 'Verbindung bestätigen';

  @override
  String get notificationsDeviceConnected => 'Gerät verbunden';

  @override
  String get notificationsConnectionRejected => 'Verbindung abgelehnt';

  @override
  String get notificationsConnectionCancelled => 'Verbindung abgebrochen';

  @override
  String get notificationsConnectionRequestExpired =>
      'Verbindungsanfrage abgelaufen';

  @override
  String get notificationsConnectionFailed => 'Verbindung fehlgeschlagen';

  @override
  String get notificationsNearbyDevice => 'Gerät in der Nähe';

  @override
  String get notificationsPermissionDenied =>
      'Systembenachrichtigungen in den Systemeinstellungen erlauben, um Anfragen im Hintergrund zu erhalten.';

  @override
  String get notificationsSystemUnavailable =>
      'Systembenachrichtigungen sind nicht verfügbar. Verbindungsanfragen bleiben hier.';

  @override
  String get platformCaptureStateUnavailable => 'Nicht verfügbar';

  @override
  String get platformCaptureStatePaused => 'Pausiert';

  @override
  String get platformCaptureStateRunning => 'Aktiv';

  @override
  String get platformCaptureStateStopped => 'Nicht aktiv';

  @override
  String get platformCaptureActionUnavailable => 'Erfassung nicht verfügbar';

  @override
  String get platformCaptureActionResume => 'Erfassung fortsetzen';

  @override
  String get platformCaptureActionPause => 'Erfassung pausieren';

  @override
  String get platformCaptureActionResumeStopped =>
      'Erfassung fortsetzen (nicht aktiv)';

  @override
  String get platformTrayOpen => 'Öffnen';

  @override
  String get platformTraySettings => 'Einstellungen';

  @override
  String get platformTrayQuit => 'Beenden';

  @override
  String get platformWindowControlsUnavailable =>
      'Die Fenstersteuerung ist nicht verfügbar. CopyPaste bleibt sichtbar.';

  @override
  String get platformTrayUnavailable =>
      'Die Systemleiste ist nicht verfügbar. CopyPaste geöffnet lassen, dann erneut versuchen oder beenden.';

  @override
  String get platformQuitFailed =>
      'CopyPaste konnte nicht beendet werden. Erneut versuchen.';

  @override
  String get platformVisibilityChangeFailed =>
      'Die Sichtbarkeit der App konnte nicht geändert werden. Erneut versuchen.';

  @override
  String get platformCaptureNotificationTitle => 'Zwischenablage gespeichert';

  @override
  String get platformCaptureNotificationBody =>
      'Ein neuer Eintrag wurde zu CopyPaste hinzugefügt.';

  @override
  String get platformPairingNotificationTitle => 'Verbindungsanfrage';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName möchte sich verbinden. CopyPaste öffnen, um den Code zu vergleichen und zu bestätigen.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Verbindungsanfragen von Geräten';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Verbindungsanfragen von Geräten in der Nähe';

  @override
  String get platformNotificationDefaultAction => 'Öffnen';

  @override
  String get platformFilesSelectionFailed =>
      'Dateien konnten nicht ausgewählt werden.';

  @override
  String get platformFilesOnlyRegular =>
      'Nur reguläre Dateien können importiert werden.';

  @override
  String get platformModulesUnavailable =>
      'Module sind auf diesem Gerät nicht verfügbar.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'Der SMS-Zugriff konnte nicht eingerichtet werden.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'Der SMS-Zugriff konnte nicht überprüft werden.';

  @override
  String get platformUpdatesUnavailableHere =>
      'App-Updates sind hier nicht verfügbar.';

  @override
  String get platformUpdateVersionUnavailable =>
      'Die installierte Version ist nicht verfügbar.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'App-Updates sind für diese Installation nicht verfügbar.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Diese Linux-Installation kann nicht automatisch aktualisiert werden.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Dieses Linux-Update passt nicht zur laufenden Architektur.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Diese macOS-Installation kann nicht automatisch aktualisiert werden.';

  @override
  String get platformUpdateStartFailed =>
      'CopyPaste konnte das Update nicht starten.';

  @override
  String get platformGlobalShortcutDescription => 'Quick Paste öffnen';

  @override
  String get settingsSectionGeneral => 'Allgemein';

  @override
  String get settingsSectionGeneralSummary =>
      'Sprache und Sichtbarkeit der App.';

  @override
  String get settingsSectionClipboard => 'Zwischenablage';

  @override
  String get settingsSectionClipboardSummary => 'Erfassung und Verlaufslimits.';

  @override
  String get settingsSectionPrivacy => 'Datenschutz';

  @override
  String get settingsSectionPrivacySummary =>
      'App-Ausschlüsse und Bildschirmschutz.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Tastenkürzel und automatisches Einfügen.';

  @override
  String get settingsSectionSync => 'Sync';

  @override
  String get settingsSectionSyncSummary =>
      'Synchronisierung und Erkennung von Geräten in der Nähe.';

  @override
  String get settingsSectionNotifications => 'Benachrichtigungen';

  @override
  String get settingsSectionNotificationsSummary =>
      'Erfassungsbenachrichtigungen und Töne.';

  @override
  String get settingsSectionData => 'Daten';

  @override
  String get settingsSectionDataSummary =>
      'Export, Backup und Wiederherstellung.';

  @override
  String get settingsSectionModules => 'Module';

  @override
  String get settingsSectionModulesSummary =>
      'Optionale Module installieren und verwalten.';

  @override
  String get settingsSectionRules => 'Inhaltsregeln';

  @override
  String get settingsSectionRulesSummary =>
      'Clips automatisch taggen, überspringen, schützen oder ablaufen lassen.';

  @override
  String get settingsSectionQuickAccess => 'Schnellzugriff';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Tastatur, Kacheln und Startbildschirm-Widget.';

  @override
  String get settingsSectionSnippets => 'Textbausteine';

  @override
  String get settingsSectionSnippetsSummary =>
      'Wiederverwendbare Texte und Vorlagen.';

  @override
  String get settingsSectionTransforms => 'Texttransformationen';

  @override
  String get settingsSectionTransformsSummary =>
      'Favoriten, Abläufe und Standardtransformationen beim Einfügen.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Mehrere Kopien sammeln und der Reihe nach einfügen.';

  @override
  String pasteQueuePasted(int position, int total) {
    return '$position von $total eingefügt';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'Eintrag $position von $total ist in der Zwischenablage';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Einträge eingefügt',
      one: '1 Eintrag eingefügt',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Einträge sind in der Zwischenablage',
      one: '1 Eintrag ist in der Zwischenablage',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Warteschlange abgeschlossen';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Hinzugefügt, $total in der Warteschlange',
      one: 'Hinzugefügt, 1 in der Warteschlange',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'Warteschlange leer, nichts eingefügt';

  @override
  String get pasteQueueMixed =>
      '„Alle einfügen“ funktioniert nur mit reinem Text oder nur mit Dateien';

  @override
  String get pasteQueueSeveralFiles => 'Füge Dateien einzeln ein.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Einträge nicht mehr verfügbar',
      one: '1 Eintrag nicht mehr verfügbar',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => '„Nächsten einfügen“ kopiert nur';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Eintrag kopiert. Erlaube „Bedienungshilfen“, um automatisch einzufügen.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Automatisches Einfügen nicht möglich. Eintrag stattdessen kopiert.';

  @override
  String get pasteQueueTargetElevated =>
      'In eine Admin-App kann nicht eingefügt werden. Eintrag stattdessen kopiert.';

  @override
  String get pasteQueueNoTarget =>
      'Keine App zum Einfügen. Eintrag stattdessen kopiert.';

  @override
  String get pasteQueueCollectingStarted => 'Kopien werden gesammelt';

  @override
  String get pasteQueueCollectingStopped => 'Sammeln beendet';

  @override
  String get pasteQueueFailed =>
      'Die Aktion der Paste Queue konnte nicht ausgeführt werden';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ausstehend',
      one: '1 ausstehend',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ausstehend · Sammelt',
      one: '1 ausstehend · Sammelt',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Sammelt';

  @override
  String get pasteQueueFinishedSummary => 'Abgeschlossen';

  @override
  String pasteQueueFull(int max) {
    return 'Warteschlange voll ($max)';
  }

  @override
  String get pasteQueueFullMessage =>
      'Neue Kopien werden nicht mehr hinzugefügt.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue ist leer';

  @override
  String get pasteQueueEmptyMessage =>
      'Sammle Kopien oder füge Einträge aus dem Verlauf hinzu.';

  @override
  String get pasteQueueCollectingEmptyTitle =>
      'Kopiere Inhalte, um sie hinzuzufügen';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Jede Kopie kommt in die Warteschlange, bis du das Sammeln beendest.';

  @override
  String get pasteQueueStartCollecting => 'Sammeln starten';

  @override
  String get pasteQueueStopCollecting => 'Sammeln beenden';

  @override
  String get pasteQueueLoading => 'Warteschlange wird geladen…';

  @override
  String get pasteQueueLoadFailed =>
      'Warteschlange konnte nicht geladen werden';

  @override
  String get pasteQueuePasteNext => 'Nächsten einfügen';

  @override
  String get pasteQueueCopyNext => 'Nächsten kopieren';

  @override
  String get pasteQueuePasteAll => 'Alle einfügen';

  @override
  String get pasteQueueCopyAll => 'Alle kopieren';

  @override
  String get pasteQueueReverse => 'Reihenfolge umkehren';

  @override
  String get pasteQueueStepBack => 'Einen Schritt zurück';

  @override
  String get pasteQueueSkip => 'Überspringen';

  @override
  String get pasteQueueRemovePasted => 'Eingefügte entfernen';

  @override
  String get pasteQueueCleared => 'Warteschlange geleert';

  @override
  String get pasteQueueUndo => 'Rückgängig';

  @override
  String get pasteQueueStartFresh => 'Neu beginnen';

  @override
  String get pasteQueueAppend => 'Anhängen';

  @override
  String get pasteQueueRestartTitle => 'Neue Sammlung beginnen?';

  @override
  String get pasteQueueRestartMessage =>
      '„Neu beginnen“ entfernt die eingefügten Einträge. „Anhängen“ behält sie.';

  @override
  String pasteQueueStale(String time) {
    return 'Warteschlange vom $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Eingefügt';

  @override
  String get pasteQueueStatusSkipped => 'Übersprungen';

  @override
  String get pasteQueueStatusUnavailable => 'Nicht mehr verfügbar';

  @override
  String get pasteQueueEdited => 'Bearbeitet';

  @override
  String get pasteQueueSecret => 'Geheim';

  @override
  String get pasteQueueKindImage => 'Bild';

  @override
  String get pasteQueueKindFiles => 'Dateien';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Erstellt um $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Eintrag $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'An Position $position verschoben';
  }

  @override
  String get pasteQueueMoveUp => 'Nach oben';

  @override
  String get pasteQueueMoveDown => 'Nach unten';

  @override
  String get pasteQueueMoveToTop => 'Ganz nach oben';

  @override
  String get pasteQueueMoveToBottom => 'Ganz nach unten';

  @override
  String get pasteQueueReorder => 'Zum Sortieren ziehen';

  @override
  String get pasteQueueMoreActions => 'Weitere Aktionen';

  @override
  String get pasteQueuePasteNow => 'Jetzt einfügen';

  @override
  String get pasteQueueEdit => 'Text bearbeiten';

  @override
  String get pasteQueueUseOriginal => 'Originaltext verwenden';

  @override
  String get pasteQueueRemove => 'Aus Warteschlange entfernen';

  @override
  String get pasteQueueEditTitle => 'Text in der Warteschlange bearbeiten';

  @override
  String get pasteQueueEditMessage =>
      'Änderungen gelten nur für diese Warteschlange.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Tastenkürzel festlegen';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Sammeln und einfügen, ohne CopyPaste zu öffnen.';

  @override
  String get pasteQueueShortcutToggleCollecting =>
      'Sammeln starten oder beenden';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Nächsten als reinen Text einfügen';

  @override
  String get pasteQueueShortcutDescription =>
      'Ein globales Tastenkürzel, das in jeder App funktioniert.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Ein anderes CopyPaste-Kürzel nutzt diese Kombination. Das bisherige bleibt erhalten.';

  @override
  String get pasteQueueShortcutInUse =>
      'Eine andere App nutzt diese Kombination.';

  @override
  String get pasteQueueShortcutRejected =>
      'Das System hat diese Kombination abgelehnt. Das bisherige Kürzel bleibt.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Strg+Alt erzeugt auf vielen Layouts Zeichen. Wähle eine andere.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Globale Tastenkürzel sind in dieser Sitzung nicht verfügbar.';

  @override
  String get pasteQueueShortcutFailed =>
      'Dieses Tastenkürzel konnte nicht festgelegt werden.';

  @override
  String get pasteQueueEnableTitle => 'Paste Queue aktivieren';

  @override
  String get pasteQueueEnableDescription =>
      'Wenn aus, sind Kürzel und Sammeln deaktiviert.';

  @override
  String get pasteQueuePasteNextStyleTitle => '„Nächsten einfügen“ bewirkt';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Automatisch einfügen oder nur in die Zwischenablage kopieren.';

  @override
  String get pasteQueuePasteIntoApp => 'In die vordere App einfügen';

  @override
  String get pasteQueueClipboardOnly => 'Nur in die Zwischenablage';

  @override
  String get pasteQueueInsertTitle => 'Neue Einträge kommen';

  @override
  String get pasteQueueInsertDescription =>
      'Wo gesammelte und hinzugefügte Einträge in die Warteschlange kommen.';

  @override
  String get pasteQueueInsertEnd => 'Ans Ende (zuerst rein, zuerst raus)';

  @override
  String get pasteQueueInsertStart =>
      'An den Anfang (zuletzt rein, zuerst raus)';

  @override
  String get pasteQueueEndBehaviorTitle => 'Nach dem letzten Eintrag';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'Was „Nächsten einfügen“ tut, wenn alle Einträge eingefügt sind.';

  @override
  String get pasteQueueEndStop => 'Anhalten';

  @override
  String get pasteQueueEndLoop => 'Von vorn beginnen';

  @override
  String get pasteQueueEndKeepLast => 'Letzten Eintrag weiter einfügen';

  @override
  String get pasteQueueSeparatorTitle => 'Trennzeichen für „Alle einfügen“';

  @override
  String get pasteQueueSeparatorDescription =>
      'Steht zwischen den Einträgen, wenn alle auf einmal eingefügt werden.';

  @override
  String get pasteQueueSeparatorNewLine => 'Neue Zeile';

  @override
  String get pasteQueueSeparatorBlankLine => 'Leerzeile';

  @override
  String get pasteQueueSeparatorTab => 'Tabulator';

  @override
  String get pasteQueueSeparatorSpace => 'Leerzeichen';

  @override
  String get pasteQueueSeparatorComma => 'Komma';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Komma und Leerzeichen';

  @override
  String get pasteQueueSeparatorNone => 'Keines';

  @override
  String get pasteQueueSeparatorCustom => 'Eigenes';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'Bis zu 32 Zeichen; \\n und \\t erlaubt';

  @override
  String get pasteQueuePlainTextTitle => 'Immer als reinen Text einfügen';

  @override
  String get pasteQueuePlainTextDescription =>
      'Entfernt die Formatierung der Einträge.';

  @override
  String get pasteQueueIdleStopTitle => 'Sammeln beenden nach';

  @override
  String get pasteQueueIdleStopDescription =>
      'Sammeln beenden, wenn so lange nichts kopiert wurde.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes Min.';
  }

  @override
  String get pasteQueueIdleNever => 'Nie';

  @override
  String get pasteQueueMaxEntriesTitle => 'Größe der Warteschlange';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Ist sie voll, kommen keine neuen Kopien hinzu.';

  @override
  String get pasteQueueRestoreTitle =>
      'Vorherige Zwischenablage am Ende wiederherstellen';

  @override
  String get pasteQueueRestoreDescription =>
      'Stellt wieder her, was vor dem Einfügen kopiert war.';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue warteschlange sammeln reihenfolge tastenkürzel trennzeichen';

  @override
  String get settingsSectionAbout => 'Info';

  @override
  String get settingsSectionAboutSummary => 'Version und App-Updates.';

  @override
  String get settingsGeneralDescription =>
      'Sprache, automatischer Start und Sichtbarkeit der App.';

  @override
  String get settingsModulesDescription =>
      'Optionale Funktionen für CopyPaste.';

  @override
  String get settingsClipboardDescription =>
      'Änderungen der Zwischenablage erfassen und Verlaufslimits verwalten.';

  @override
  String get settingsDataDescription =>
      'Verlauf der Zwischenablage exportieren, sichern und wiederherstellen.';

  @override
  String get settingsSyncDescription =>
      'Synchronisierung mit gekoppelten Geräten steuern.';

  @override
  String get settingsPrivacyDescription =>
      'Festlegen, welche Apps erfasst werden, und Inhalte der Zwischenablage schützen.';

  @override
  String get settingsNotificationsDescription =>
      'Benachrichtigungen und Töne für erfasste Einträge wählen.';

  @override
  String get settingsAboutDescription => 'CopyPaste-Version und App-Updates.';

  @override
  String get settingsQuickPasteDescription =>
      'Verlauf der Zwischenablage überall öffnen, ohne das Fenster zu wechseln.';

  @override
  String get settingsLoadingMessage => 'Einstellungen werden geladen.';

  @override
  String get settingsUnavailableTitle => 'Einstellungen nicht verfügbar';

  @override
  String get settingsNoResultsTitle => 'Keine Einstellungen gefunden';

  @override
  String get settingsNoResultsMessage => 'Andere Suchbegriffe versuchen.';

  @override
  String get settingsAttentionTitle => 'Problem mit den Einstellungen';

  @override
  String get settingsSearchPlaceholder => 'Einstellungen durchsuchen';

  @override
  String get settingsBackToSettings => 'Zurück zu den Einstellungen';

  @override
  String get settingsStartOnBootTitle => 'Beim Gerätestart starten';

  @override
  String get settingsStartAtLoginTitle => 'Bei Anmeldung starten';

  @override
  String get settingsStartupSearchDescription =>
      'CopyPaste automatisch starten.';

  @override
  String get settingsStartupKeywords =>
      'Start Anmeldung Hochfahren starten automatisch Autostart startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Im Task-Manager deaktiviert. CopyPaste unter Einstellungen → Apps → Autostart aktivieren.';

  @override
  String get settingsStartupMacApproval =>
      'CopyPaste unter Systemeinstellungen → Allgemein → Anmeldeobjekte erlauben.';

  @override
  String get settingsStartupUnavailable =>
      'Automatischer Start ist in dieser Installation nicht verfügbar.';

  @override
  String get settingsStartupAndroidDescription =>
      'Hintergrunddienste nach einem Neustart fortsetzen.';

  @override
  String get settingsStartupDesktopDescription =>
      'CopyPaste bei der Anmeldung automatisch öffnen.';

  @override
  String get settingsShowInDock => 'Im Dock anzeigen';

  @override
  String get settingsShowInTaskbar => 'In Taskleiste anzeigen';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'App-Symbol zum Wechseln der Fenster anzeigen.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'Dock Taskleiste App Symbol Sichtbarkeit dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'In Menüleiste anzeigen';

  @override
  String get settingsShowInSystemTray => 'In Systemleiste anzeigen';

  @override
  String get settingsTrayVisibilityDescription =>
      'Schnellzugriff auf CopyPaste und die Erfassung anzeigen.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'Systemleiste Menü Leiste Menüleiste Status Symbol Sichtbarkeit tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle =>
      'Sichtbarkeit konnte nicht geändert werden';

  @override
  String get settingsModulesSearchDescription =>
      'Optionale Module installieren, aktualisieren und entfernen.';

  @override
  String get settingsModulesKeywords =>
      'Funktionen Befehle Erweiterungen features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Android-Hintergrunderfassung';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Vollständige oder eingeschränkte Erfassung, eingerichtet über Shizuku oder ADB.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'Hintergrund Berechtigungen Einrichtung background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Voller oder eingeschränkter Modus, Einrichtung per Shizuku oder ADB.';

  @override
  String get settingsOpenSetup => 'Einrichtung öffnen';

  @override
  String get settingsSaveScreenshotsTitle => 'Screenshots speichern';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Neue Android-Screenshots automatisch im Verlauf speichern.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'Bilder Fotos Erfassung Berechtigungen images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Neue Screenshots automatisch zum Verlauf hinzufügen.';

  @override
  String get settingsScreenshotAccessTitle => 'Zugriff für Screenshots';

  @override
  String get settingsScreenshotAccessDescription =>
      'Erforderlich, um Screenshots im Hintergrund zu speichern.';

  @override
  String get settingsAllowAccess => 'Zugriff erlauben';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'Screenshot-Erfassung ist nicht aktiv';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'Neue Screenshots werden nicht gespeichert. Erfassung fortsetzen, um den Dienst neu zu starten.';

  @override
  String get settingsResumeCapture => 'Erfassung fortsetzen';

  @override
  String get settingsClipboardCaptureTitle => 'Erfassung der Zwischenablage';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Erfassung der Zwischenablage pausieren oder fortsetzen.';

  @override
  String get settingsCaptureStateUnavailable => 'Status nicht verfügbar';

  @override
  String get settingsCaptureStoppedTitle => 'Erfassung ist nicht aktiv';

  @override
  String get settingsCaptureStoppedMessage =>
      'CopyPaste empfängt auf diesem Gerät derzeit keine Änderungen der Zwischenablage.';

  @override
  String get settingsRetentionTitle => 'Aufbewahrung';

  @override
  String get settingsRetentionDescription =>
      'Alte, nicht angeheftete Einträge automatisch entfernen.';

  @override
  String get settingsRetentionForever => 'Für immer behalten';

  @override
  String get settingsStorageQuotaTitle => 'Speicherkontingent';

  @override
  String get settingsStorageQuotaDescription =>
      'Maximaler lokaler Speicher für nicht angehefteten Verlauf.';

  @override
  String get settingsStorageQuotaKeywords =>
      'Festplatte Speicherplatz Limit disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Maximale Dateigröße';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Limit zum Hinzufügen und Herunterladen von Dateien.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'Import Download Übertragung Größe Limit import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Größte Datei, die du hinzufügen oder herunterladen kannst.';

  @override
  String get settingsHistoryFilesTitle => 'Verlaufsdateien';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Verlauf exportieren, Backups erstellen oder ein Backup wiederherstellen.';

  @override
  String get settingsHistoryFilesKeywords =>
      'Text verschlüsselt Backup Daten text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Textverlauf exportieren';

  @override
  String get settingsExportTextDescription =>
      'Texteinträge in einer portablen Datei speichern.';

  @override
  String get settingsExportAction => 'Exportieren';

  @override
  String get settingsBackupTitle => 'Verschlüsseltes Backup';

  @override
  String get settingsBackupDescription =>
      'Den gesamten lokalen Verlauf dieses Geräts speichern.';

  @override
  String get settingsCreateBackupAction => 'Backup erstellen';

  @override
  String get settingsRestoreTitle => 'Backup wiederherstellen';

  @override
  String get settingsRestoreDescription =>
      'Lokalen Verlauf durch ein verschlüsseltes Backup ersetzen.';

  @override
  String get settingsRestoreAction => 'Wiederherstellen';

  @override
  String get settingsRestoreDialogTitle => 'Lokalen Verlauf ersetzen?';

  @override
  String get settingsRestoreDialogMessage =>
      'Das Backup ersetzt den Verlauf auf diesem Gerät. Einstellungen bleiben erhalten.';

  @override
  String get settingsRestoreFailed =>
      'Das Backup konnte nicht wiederhergestellt werden.';

  @override
  String get settingsSyncSwitchTitle => 'Sync';

  @override
  String get settingsSyncSwitchDescription =>
      'Verlauf mit gekoppelten Geräten synchronisieren.';

  @override
  String get settingsSyncSearchDescription =>
      'Synchronisierung gekoppelter Geräte.';

  @override
  String get settingsInstantClipboardTitle => 'Sofort-Zwischenablage';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Neue Einträge von anderen Geräten automatisch kopieren.';

  @override
  String get settingsInstantClipboardKeywords =>
      'automatisch kopieren einfügen Sync empfangen automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Neue Einträge von anderen Geräten in diese Zwischenablage kopieren.';

  @override
  String get settingsLanVisibilityTitle => 'Sichtbarkeit im LAN';

  @override
  String get settingsLanVisibilityDescription =>
      'Geräten in der Nähe erlauben, dieses Gerät zu finden.';

  @override
  String get settingsLanVisibilityKeywords =>
      'lokal Netzwerk Erkennung local network discovery';

  @override
  String get settingsExcludedAppsTitle => 'Ausgeschlossene Apps';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Keine automatische Erfassung aus ausgeschlossenen Apps.';

  @override
  String get settingsExcludedAppsKeywords =>
      'Datenschutz App Kennungen privacy app identifiers';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Keine automatische Erfassung aus diesen Apps.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Solange Apps ausgeschlossen sind, pausiert die automatische Erfassung.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'App-Ausschlüsse werden unter macOS, Windows, Android und Linux unterstützt.';

  @override
  String get settingsExcludedAppsAdd => 'Hinzufügen';

  @override
  String get settingsExcludedAppsEmpty => 'Keine Apps ausgeschlossen.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return '$app entfernen';
  }

  @override
  String get applicationPickerSearch => 'Apps durchsuchen';

  @override
  String get applicationPickerEmpty => 'Keine Apps gefunden';

  @override
  String get settingsSkipSecretTitle => 'Vertrauliches überspringen';

  @override
  String get settingsSkipSecretDescription =>
      'Keine Kopien speichern, die Apps als geheim markieren.';

  @override
  String get settingsSkipSecretKeywords =>
      'geheim Passwort sensibel verborgen Spoiler secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Temporäres überspringen';

  @override
  String get settingsSkipTransientDescription =>
      'Keine Kopien speichern, die Apps als temporär markieren.';

  @override
  String get settingsSkipTransientKeywords =>
      'flüchtig temporär Datenschutz transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Screenshots blockieren';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Screenshots und Bildschirmaufnahmen von CopyPaste verhindern.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'Das Blockieren von Screenshots ist auf dieser Plattform nicht verfügbar.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'Datenschutz Bildschirm Aufnahme Schutz Kopplung QR Sicherheitscode privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Benachrichtigung beim Kopieren';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Nach einer Erfassung im Hintergrund eine Benachrichtigung anzeigen.';

  @override
  String get settingsNotificationPreviewTitle =>
      'Inhalt der Zwischenablage anzeigen';

  @override
  String get settingsNotificationPreviewDescription =>
      'Eine Vorschau des Eintrags in Kopierbenachrichtigungen anzeigen.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'Benachrichtigung Vorschau Text Bild Datenschutz notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Ton beim Kopieren';

  @override
  String get settingsSoundOnCopyDescription =>
      'Nach erfolgreicher Erfassung das System-Feedback abspielen.';

  @override
  String get settingsVersionTitle => 'Version';

  @override
  String get settingsVersionDescription => 'Installierte CopyPaste-Version.';

  @override
  String get settingsVersionUnavailable => 'Nicht verfügbar';

  @override
  String get settingsUpdatesTitle => 'App-Updates';

  @override
  String get settingsUpdatesSearchDescription =>
      'CopyPaste-Updates suchen und installieren.';

  @override
  String get settingsUpdatesKeywords =>
      'Version GitHub Release version github release';

  @override
  String get settingsUpdateIdle =>
      'Nach einer neueren CopyPaste-Version suchen.';

  @override
  String get settingsUpdateChecking => 'Suche nach Updates.';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste ist auf dem neuesten Stand.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'Version $version ist auf dem neuesten Stand.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'Version $available ist verfügbar. Auf diesem Gerät ist Version $current installiert.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Version $version wird heruntergeladen · $percent %.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Updates installierter Module werden vorbereitet.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Version $version wird installiert.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'App-Updates erfordern Aufmerksamkeit.';

  @override
  String get settingsUpdateNowAction => 'Jetzt aktualisieren';

  @override
  String get settingsUpdateOpenReleaseAction => 'Release öffnen';

  @override
  String get settingsUpdateCheckingAction => 'Wird gesucht';

  @override
  String get settingsUpdateDownloadingAction => 'Wird geladen';

  @override
  String get settingsUpdateInstallingAction => 'Wird installiert';

  @override
  String get settingsUpdateRestartAction => 'CopyPaste neu starten';

  @override
  String get settingsUpdateCheckAgainAction => 'Erneut prüfen';

  @override
  String get settingsUpdateNoneTitle => 'Keine Updates verfügbar';

  @override
  String get settingsUpdateLatest => 'CopyPaste ist die neueste Version.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version ist die neueste Version.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Update verfügbar';

  @override
  String get settingsUpdateReady =>
      'Eine neuere CopyPaste-Version kann installiert werden.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version kann installiert werden.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Update nicht verfügbar';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Diese Installation kann nicht automatisch aktualisiert werden.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Suche nach Updates fehlgeschlagen';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'CopyPaste konnte nicht nach Updates suchen.';

  @override
  String get settingsQuickPasteSetupTitle => 'Quick Paste einrichten';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Die signierte Begleiterweiterung für GNOME oder KDE aktivieren und die Tastatursteuerung erlauben, um zur vorherigen App zurückzukehren.';

  @override
  String get settingsQuickPasteDesktopIntegration => 'Desktop-Integration';

  @override
  String get settingsQuickPasteAllowKeyboard => 'Tastatursteuerung erlauben';

  @override
  String get settingsQuickPasteCheckAgain => 'Erneut prüfen';

  @override
  String get settingsQuickPasteLoading =>
      'Einstellungen für Quick Paste werden geladen.';

  @override
  String get settingsQuickPasteUnavailableTitle =>
      'Quick Paste ist nicht verfügbar';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'Dieses Gerät bietet kein Fenster für Quick Paste.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Quick Paste öffnen';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Globales Tastenkürzel für Quick Paste festlegen.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'Tastatur Tastenkürzel Hotkey keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'Funktioniert, während CopyPaste im Hintergrund läuft.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Registriertes Tastenkürzel: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Automatisch einfügen';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Ausgewählten Eintrag in die vorherige App einfügen.';

  @override
  String get settingsAutoPasteKeywords =>
      'Bedienungshilfen automatisch accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Ein ausgewählter Eintrag wird direkt in die vorherige App eingefügt.';

  @override
  String get settingsAccessibilityRequiredTitle =>
      'Bedienungshilfen erforderlich';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Bis die Berechtigung erteilt ist, wird ein ausgewählter Eintrag nur kopiert, nicht eingefügt.';

  @override
  String get settingsQuickPasteAttentionTitle => 'Problem mit Quick Paste';

  @override
  String get settingsShortcutRecording => 'Tastenkürzel drücken …';

  @override
  String get settingsShortcutNotSet => 'Nicht festgelegt';

  @override
  String get settingsShortcutReset => 'Tastenkürzel zurücksetzen';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Tage',
      one: '$count Tag',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Geräte werden geladen …';

  @override
  String get devicesUnavailableTitle => 'Geräte nicht verfügbar';

  @override
  String get devicesNearbyTitle => 'Geräte in der Nähe';

  @override
  String get devicesScan => 'Suchen';

  @override
  String get devicesScanning => 'Suche läuft …';

  @override
  String get devicesNoneNearby => 'Keine Geräte in der Nähe';

  @override
  String get devicesThisDevice => 'Dieses Gerät';

  @override
  String get devicesTrusted => 'Vertrauenswürdig';

  @override
  String get devicesConnect => 'Verbinden';

  @override
  String devicesCardPing(String ping) {
    return 'Ping: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Ping:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'Dieses Gerät umbenennen';

  @override
  String get devicesNamePlaceholder => 'Gerätename';

  @override
  String get devicesRenameFailed =>
      'Der Gerätename konnte nicht gespeichert werden.';

  @override
  String devicesRevokeDialogTitle(String device) {
    return 'Kopplung mit $device widerrufen?';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return 'Kopplung mit $device aufheben?';
  }

  @override
  String get devicesRevokeDialogMessage =>
      'Das Gerät wird aus der Gruppe entfernt und diese Kopplung dauerhaft gesperrt. Andere Geräte übernehmen das Entfernen, sobald sie sich erneut verbinden.';

  @override
  String get devicesUnpairDialogMessage =>
      'Das Gerät wird aus der Gruppe entfernt. Andere Geräte übernehmen das Entfernen, sobald sie sich erneut verbinden. Ein erneutes Hinzufügen erfordert eine neue Bestätigung.';

  @override
  String get devicesRevoke => 'Widerrufen';

  @override
  String get devicesUnpair => 'Entkoppeln';

  @override
  String get devicesActionFailedMessage =>
      'Die Geräteaktion konnte nicht ausgeführt werden.';

  @override
  String get devicesCloseDetails => 'Gerätedetails schließen';

  @override
  String get devicesDetailType => 'Gerätetyp';

  @override
  String get devicesDetailOperatingSystem => 'Betriebssystem';

  @override
  String get devicesDetailModel => 'Modell';

  @override
  String get devicesDetailAppVersion => 'CopyPaste-Version';

  @override
  String get devicesDetailProtocolVersion => 'Protokollversion';

  @override
  String get devicesDetailStatus => 'Status';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => 'Gemessen';

  @override
  String get devicesDetailLastSeen => 'Zuletzt gesehen';

  @override
  String get devicesDetailLanEndpoint => 'LAN-Endpunkt';

  @override
  String get devicesDetailDeviceId => 'Geräte-ID';

  @override
  String get devicesDetailPairingId => 'Kopplungs-ID';

  @override
  String get devicesDetailProfileProvenance => 'Profilherkunft';

  @override
  String get devicesDetailProfileTrust => 'Profilvertrauen';

  @override
  String get devicesDetailProfileUpdated => 'Profil aktualisiert';

  @override
  String get devicesValueUnavailable => 'Nicht verfügbar';

  @override
  String get devicesRenameDevice => 'Gerät umbenennen';

  @override
  String get devicesSyncNow => 'Jetzt synchronisieren';

  @override
  String get devicesRevokePairing => 'Kopplung widerrufen';

  @override
  String get devicesLocal => 'Lokal';

  @override
  String get devicesTrustAuthenticated => 'Authentifiziert';

  @override
  String get devicesTrustUnverified => 'Nicht verifiziert';

  @override
  String get devicesProvenanceSelfReported => 'Selbst gemeldet';

  @override
  String get devicesProvenanceObserved => 'Beobachtet';

  @override
  String get devicesProvenanceMeasured => 'Gemessen';

  @override
  String get devicesConnectionOnline => 'Online';

  @override
  String get devicesConnectionOffline => 'Offline';

  @override
  String get devicesConnectionVisible => 'Sichtbar';

  @override
  String get devicesConnectionNotSeen => 'Nicht gesehen';

  @override
  String get devicesConnectionUnknown => 'Unbekannt';

  @override
  String get devicesPeerAvailable => 'Verfügbar';

  @override
  String get devicesPeerVisibleNow => 'Jetzt sichtbar';

  @override
  String get devicesPeerStatusUnknown => 'Status unbekannt';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => 'Desktop';

  @override
  String get devicesClassLaptop => 'Laptop';

  @override
  String get devicesClassPhone => 'Smartphone';

  @override
  String get devicesClassTablet => 'Tablet';

  @override
  String get devicesClassUnknown => 'Gerät';

  @override
  String get devicesUnknownOs => 'Unbekanntes System';

  @override
  String get devicesAddDevice => 'Gerät hinzufügen';

  @override
  String get devicesScanQr => 'QR scannen';

  @override
  String get devicesEnterCode => 'Code eingeben';

  @override
  String get devicesScannerUnavailableTitle => 'Scanner nicht verfügbar';

  @override
  String get devicesActionFailedTitle => 'Aktion fehlgeschlagen';

  @override
  String get devicesRefreshFailedTitle =>
      'Geräte konnten nicht aktualisiert werden';

  @override
  String get devicesNearbyPairingUnavailable =>
      'Kopplung in der Nähe ist nicht verfügbar.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'Kopplung in der Nähe ist nicht verfügbar. Über „Gerät hinzufügen“ einen Kopplungscode anzeigen.';

  @override
  String get pairingTitle => 'Kopplung';

  @override
  String get pairingPairDevice => 'Gerät koppeln';

  @override
  String get pairingScanQrCode => 'QR-Code scannen';

  @override
  String get pairingEnterPairingCode => 'Kopplungscode eingeben';

  @override
  String get pairingConnectNearbyDevice => 'Gerät in der Nähe verbinden';

  @override
  String get pairingSendingRequest => 'Verbindungsanfrage wird gesendet …';

  @override
  String get pairingEmptyTitle => 'Kopplungsmethode wählen';

  @override
  String get pairingEmptyMessage =>
      'Hier erscheinen die Optionen zur Kopplung.';

  @override
  String get pairingCreatingInvitation => 'Kopplungseinladung wird erstellt.';

  @override
  String get pairingCode => 'Kopplungscode';

  @override
  String get pairingCodeHint =>
      'Den 8-stelligen Code vom anderen Gerät eingeben.';

  @override
  String get pairingDeviceAddress => 'Geräteadresse';

  @override
  String get pairingDeviceAddressHint =>
      'Die vom anderen Gerät angezeigte host:port-Adresse verwenden.';

  @override
  String get pairingCheckDetailsTitle => 'Kopplungsdetails prüfen';

  @override
  String get pairingActionFailed => 'Kopplung fehlgeschlagen';

  @override
  String get pairingConnectingDevice => 'Gerät wird verbunden …';

  @override
  String get pairingQrScannerTitle => 'QR-Scanner';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'Hier erscheint die Kameravorschau.';

  @override
  String get pairingScannerNeedsAttention => 'Problem mit dem Scanner';

  @override
  String get pairingCodeIncomplete =>
      'Den vollständigen 8-stelligen Kopplungscode eingeben.';

  @override
  String get pairingAddressMissing =>
      'Die host:port-Adresse des Geräts eingeben.';

  @override
  String get pairingWaitingForOtherDevice => 'Warten auf das andere Gerät';

  @override
  String get pairingInviteInstructions =>
      'Auf dem anderen Gerät „Gerät hinzufügen“ → „Code eingeben“ wählen oder den QR-Code scannen.';

  @override
  String get pairingDetailsTitle => 'Kopplungsdetails';

  @override
  String get pairingAddress => 'Adresse';

  @override
  String get pairingVerificationCode => 'Bestätigungscode';

  @override
  String get pairingVerificationCodeHint =>
      'Nur bestätigen, wenn dieser Code auf beiden Geräten übereinstimmt.';

  @override
  String get pairingReject => 'Ablehnen';

  @override
  String get pairingConfirm => 'Bestätigen';

  @override
  String get pairingAccept => 'Annehmen';

  @override
  String get pairingJoinDevice => 'Beitreten';

  @override
  String get pairingStartScanner => 'Scanner starten';

  @override
  String get pairingActionNotStarted =>
      'Die Kopplung konnte nicht gestartet werden.';

  @override
  String get pairingStateHandshaking => 'Gerät wird überprüft';

  @override
  String get pairingStateAwaitingConfirmation =>
      'Auf beiden Geräten bestätigen';

  @override
  String get pairingStateConfirmed => 'Gerät gekoppelt';

  @override
  String get pairingStateRejected => 'Kopplung abgelehnt';

  @override
  String get pairingStateCancelled => 'Kopplung abgebrochen';

  @override
  String get pairingStateTimedOut => 'Kopplung abgelaufen';

  @override
  String get pairingStateFailed => 'Kopplung fehlgeschlagen';

  @override
  String get pairingHandshakingDescription =>
      'Die Geräte bauen einen sicheren Kanal auf.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Den geschützten Bestätigungscode vergleichen und dann auf beiden Geräten bestätigen.';

  @override
  String get pairingConfirmedDescription =>
      'Das Gerät ist verbunden. Die Gruppenmitgliedschaft wird automatisch synchronisiert.';

  @override
  String get pairingNoDeviceAdded => 'Es wurde kein Gerät hinzugefügt.';

  @override
  String get pairingTimedOutDescription =>
      'Zum Fortfahren eine neue Kopplung starten.';

  @override
  String get pairingIdleDescription =>
      'Eine Kopplung starten, um ein Gerät zu verbinden.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'Geschützte Kopplung nicht verfügbar';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Diese Kopplung muss in einem eigenen geschützten Fenster geöffnet werden.';

  @override
  String get pairingCancel => 'Kopplung abbrechen';

  @override
  String get pairingOrEnterCode => 'Oder Kopplungscode eingeben';

  @override
  String get pairingJoinSecurely => 'Sicher beitreten';

  @override
  String get pairingProtectedWaitingDescription =>
      'In diesem geschützten Fenster einen QR-Code einblenden, einen scannen oder einen Kopplungscode eingeben.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Läuft in $seconds s ab';
  }

  @override
  String get pairingInvalidQr =>
      'Einen QR-Code zur CopyPaste-Kopplung scannen.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Der Google-Scanner ist nicht verfügbar. Stattdessen den Kopplungscode eingeben.';

  @override
  String get pairingScannerCouldNotOpen =>
      'Der Scanner konnte nicht geöffnet werden. Stattdessen den Kopplungscode eingeben.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'Die sichere Anzeige der Kopplung ist nicht verfügbar.';

  @override
  String get pairingVerificationCodeRequired =>
      'Zuerst den Bestätigungscode einblenden und vergleichen.';

  @override
  String get syncStatusUnavailable => 'Sync-Status nicht verfügbar';

  @override
  String get syncStatusOff => 'Sync aus';

  @override
  String get syncStatusWaitingForSync => 'Warten auf Sync';

  @override
  String get syncStatusSyncing => 'Wird synchronisiert';

  @override
  String get syncStatusUpToDate => 'Aktuell';

  @override
  String get syncStatusInterrupted => 'Synchronisierung unterbrochen';

  @override
  String get syncStatusPartiallySynchronized => 'Teilweise synchronisiert';

  @override
  String get syncStatusRetrying => 'Wird wiederholt';

  @override
  String get syncStatusWaitingToSync => 'Wartet auf Sync';

  @override
  String get syncStatusWaitingForDevice => 'Warten auf Gerät';

  @override
  String get syncStatusNoPairedDevices => 'Keine gekoppelten Geräte';

  @override
  String get syncStatusDevicesOffline => 'Geräte offline';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · Aktivitätsprotokoll nicht verfügbar';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Sync aus · $count Probleme',
      one: 'Sync aus · $count Problem',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count offline',
      one: '$status · $count offline',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Geräte erfordern Aufmerksamkeit',
      one: '$count Gerät erfordert Aufmerksamkeit',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Geräte offline · $count Probleme',
      one: 'Geräte offline · $count Problem',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Verbindung wird geprüft';

  @override
  String get syncConnectionUnknown => 'Verbindung unbekannt';

  @override
  String get syncLogDeviceOffline => 'Gerät ist offline.';

  @override
  String get syncLogConnectionRestored => 'Verbindung wiederhergestellt.';

  @override
  String get syncLogProblemDetected => 'Synchronisierungsproblem erkannt.';

  @override
  String get syncLogProblemResolved => 'Synchronisierungsproblem behoben.';

  @override
  String get syncLogFailed => 'Synchronisierung fehlgeschlagen.';

  @override
  String get syncLogRecovered => 'Synchronisierung wiederhergestellt.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent Einträge',
      one: '$sent Eintrag',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received Einträge',
      one: '$received Eintrag',
    );
    return 'Gesendet: $_temp0 · Empfangen: $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Synchronisierung: $status';
  }

  @override
  String get syncDetailsTitle => 'Synchronisierung';

  @override
  String get syncDetailsClose => 'Synchronisierungsdetails schließen';

  @override
  String get syncNoPairedDevicesMessage =>
      'Ein Gerät koppeln, um den Verlauf der Zwischenablage zu synchronisieren.';

  @override
  String get syncActivityLogUnavailableTitle =>
      'Aktivitätsprotokoll nicht verfügbar';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Die Sync-Aktivität konnte nicht gelesen oder gespeichert werden.';

  @override
  String get syncDetailsDevicesHeading => 'Geräte';

  @override
  String get syncDetailsActivityHeading => 'Aktivität';

  @override
  String get syncDetailsNoRecentProblems =>
      'Keine aktuellen Synchronisierungsprobleme';

  @override
  String get syncDetailsUnknownPeer => 'Gekoppeltes Gerät';

  @override
  String get syncPeerConnection => 'Verbindung';

  @override
  String get syncPeerLastCheckStarted => 'Letzte Prüfung gestartet';

  @override
  String get syncPeerItemsOverSizeLimit => 'Einträge über dem Größenlimit';

  @override
  String syncPeerLastSeen(String time) {
    return 'Zuletzt gesehen: $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Zuletzt synchronisiert: $time';
  }

  @override
  String get syncPeerLastSuccessfulSync =>
      'Letzte erfolgreiche Synchronisierung';

  @override
  String get syncPeerConnectionDetails => 'Verbindungsdetails';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Verbindungsdetails für $device';
  }

  @override
  String get historyKindText => 'Text';

  @override
  String get historyKindLink => 'Link';

  @override
  String get historyKindEmail => 'E-Mail';

  @override
  String get historyKindColor => 'Farbe';

  @override
  String get historyKindPhone => 'Telefon';

  @override
  String get historyKindCode => 'Code';

  @override
  String get historyKindPath => 'Pfad';

  @override
  String get historyKindCredentials => 'Zugangsdaten';

  @override
  String get historyKindImage => 'Bild';

  @override
  String get historyKindFile => 'Datei';

  @override
  String get historyKindOther => 'Sonstiges';

  @override
  String get historySortNewest => 'Neueste zuerst';

  @override
  String get historySortOldest => 'Älteste zuerst';

  @override
  String get historySortRelevance => 'Relevanz';

  @override
  String get historyErrorUpdatesUnavailable =>
      'Verlaufsaktualisierungen sind vorübergehend nicht verfügbar.';

  @override
  String get historyErrorBulkDelete =>
      'Einige ausgewählte Einträge konnten nicht gelöscht werden. Erneut versuchen.';

  @override
  String get historyErrorBulkPin =>
      'Einige ausgewählte Einträge konnten nicht angeheftet oder gelöst werden. Erneut versuchen.';

  @override
  String get historyErrorRefresh =>
      'Der Verlauf konnte nicht aktualisiert werden. Erneut versuchen.';

  @override
  String get historyErrorLoad =>
      'Der Verlauf konnte nicht geladen werden. Erneut versuchen.';

  @override
  String get historyErrorLoadMore =>
      'Weitere Einträge konnten nicht geladen werden.';

  @override
  String get historyErrorLoadEarlier =>
      'Frühere Einträge konnten nicht geladen werden.';

  @override
  String get historyErrorOpenClip =>
      'Dieser Eintrag konnte nicht geöffnet werden.';

  @override
  String get historyErrorCopyPlainText =>
      'Reiner Text konnte nicht kopiert werden.';

  @override
  String get historyErrorCopyClip =>
      'Dieser Eintrag konnte nicht kopiert werden.';

  @override
  String get historyErrorCopyText => 'Dieser Text konnte nicht kopiert werden.';

  @override
  String get historyErrorPrepareFile =>
      'Diese Datei konnte nicht heruntergeladen werden. Quellgerät prüfen und erneut versuchen.';

  @override
  String get historyErrorDownloadFile =>
      'Diese Datei konnte nicht heruntergeladen werden.';

  @override
  String get historyErrorSelectFiles =>
      'Dateien konnten nicht ausgewählt werden. Erneut versuchen.';

  @override
  String get historyErrorReadSelectedFile =>
      'Die ausgewählte Datei konnte nicht gelesen werden. Zugriff und freien Speicherplatz prüfen.';

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
          'Import fehlgeschlagen: $failed von $total Dateien. Erster Fehler: $name. $reason',
      one:
          'Import fehlgeschlagen: $failed von $total Datei. Erster Fehler: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'Die Vertraulichkeit des Eintrags konnte nicht geändert werden.';

  @override
  String get historyErrorPinUpdate =>
      'Das Anheften konnte nicht geändert werden.';

  @override
  String get historyErrorPinnedOrderSave =>
      'Die Reihenfolge der angehefteten Einträge konnte nicht gespeichert werden. Erneut versuchen.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'Die Reihenfolge wurde gespeichert, aber der Verlauf konnte nicht aktualisiert werden.';

  @override
  String get historyErrorDeleteClip =>
      'Dieser Eintrag konnte nicht gelöscht werden.';

  @override
  String get historyErrorDeleteAll =>
      'Der Verlauf konnte nicht gelöscht werden.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'Die ausgewählten Einträge konnten nicht gelöscht werden.';

  @override
  String get historyOcrErrorUnavailable =>
      'OCR ist nicht verfügbar. Das Modul in den Einstellungen prüfen und erneut versuchen.';

  @override
  String get historyOcrErrorStart =>
      'OCR konnte nicht gestartet werden. Erneut versuchen.';

  @override
  String get historyOcrErrorRecognize =>
      'Der Text im Bild konnte nicht erkannt werden. Erneut versuchen.';

  @override
  String get historyOcrRecognizing => 'Text im Bild wird erkannt.';

  @override
  String get historyOcrFailedTitle => 'OCR fehlgeschlagen';

  @override
  String get historyOcrNoText => 'Kein Text gefunden';

  @override
  String get historyOcrTitle => 'Text im Bild';

  @override
  String get historyOcrTextCopied => 'Erkannter Text kopiert.';

  @override
  String get historyOcrCopyError =>
      'Text konnte nicht kopiert werden. Erneut versuchen.';

  @override
  String get historyTagsErrorLoad =>
      'Tags konnten nicht geladen werden. Erneut versuchen.';

  @override
  String get historyTagsErrorSave =>
      'Die Tag-Änderung konnte nicht gespeichert werden. Einen eindeutigen Namen mit bis zu 64 Zeichen verwenden und angeheftete Einträge auswählen.';

  @override
  String get historyTags => 'Tags';

  @override
  String get historyTagsUpdateErrorTitle =>
      'Tags konnten nicht aktualisiert werden';

  @override
  String get historyTagsEmpty => 'Noch keine Tags';

  @override
  String get historyTagRename => 'Umbenennen';

  @override
  String get historyTagActions => 'Tag-Aktionen';

  @override
  String get historyTagNamePlaceholder => 'Tag-Name';

  @override
  String get historyCreateTag => 'Tag erstellen';

  @override
  String get historySaveTag => 'Tag speichern';

  @override
  String get historyTagsFilterAll => 'Alle Tags';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Tags',
      one: '$count Tag',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Nach Tags filtern: $selection';
  }

  @override
  String get historyManageTags => 'Tags verwalten';

  @override
  String get historyTagsFilterSemantics =>
      'Angeheftete Einträge nach Tags filtern';

  @override
  String get historyDownloadErrorCancelled =>
      'Der Download der Datei wurde abgebrochen.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'Der Download-Dienst ist nicht verfügbar. CopyPaste erneut öffnen und erneut versuchen.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'Der Download-Dienst hat sich nicht verbunden. Erneut versuchen.';

  @override
  String get historyFileErrorAccess =>
      'Auf die ausgewählte Datei konnte nicht zugegriffen werden.';

  @override
  String get historySpoilerRevealSemantics =>
      'Vertraulicher Inhalt. Spoiler einblenden';

  @override
  String get historyActionFailedTitle => 'Aktion fehlgeschlagen';

  @override
  String get historyLoadingMessage =>
      'Verlauf der Zwischenablage wird geladen.';

  @override
  String get historyUnavailableTitle => 'Verlauf nicht verfügbar';

  @override
  String get historyEmptyTitle => 'Keine Einträge gefunden';

  @override
  String get historyEmptyMessage =>
      'Erfasste Einträge und passende Treffer erscheinen hier.';

  @override
  String get historyAttentionTitle => 'Problem mit dem Verlauf';

  @override
  String get historyLoadMore => 'Mehr laden';

  @override
  String get historySectionPinned => 'Angeheftet';

  @override
  String get historySectionToday => 'Heute';

  @override
  String get historySectionYesterday => 'Gestern';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Einträge wurden übersprungen',
      one: '$count Eintrag wurde übersprungen',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Einige gespeicherte Einträge konnten nicht gelesen werden und werden in dieser Liste nicht angezeigt.';

  @override
  String get historySearch => 'Verlauf durchsuchen';

  @override
  String get historyResetFilters => 'Filter zurücksetzen';

  @override
  String get historyImportFiles => 'Dateien importieren';

  @override
  String get historySelectClips => 'Einträge auswählen';

  @override
  String get historyCloseSearch => 'Suche schließen';

  @override
  String get historyFilterAllClips => 'Alle Einträge';

  @override
  String get historyFilterAllDevices => 'Alle Geräte';

  @override
  String get historyFilterAllApps => 'Alle Apps';

  @override
  String get historyPinClip => 'Eintrag anheften';

  @override
  String get historyUnpinClip => 'Eintrag lösen';

  @override
  String get historyReorderPinnedClip => 'Angehefteten Eintrag verschieben';

  @override
  String get historyReorderPinnedHint =>
      'Zum Verschieben ziehen. Alt + Pfeil nach oben oder unten verschiebt um eine Position.';

  @override
  String get historyClipUnavailable => 'Eintrag nicht verfügbar';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Eintragstyp $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Eintrag auswählen';

  @override
  String get historyInspectorEmptyMessage =>
      'Hier erscheinen der vollständige Inhalt und die Aktionen.';

  @override
  String get historyNoPreview => 'Keine Vorschau verfügbar.';

  @override
  String get historyCloseClipDetails => 'Eintragsdetails schließen';

  @override
  String get historyCopyOptions => 'Kopieroptionen';

  @override
  String get historyCopyPlainText => 'Als reinen Text kopieren';

  @override
  String get historyManageClipTags => 'Tags des Eintrags verwalten';

  @override
  String get historyPinnedButton => 'Angeheftet';

  @override
  String get historyAddSpoiler => 'Spoiler hinzufügen';

  @override
  String get historyRemoveSpoiler => 'Spoiler entfernen';

  @override
  String get historyUpdateFailedTitle => 'Änderung fehlgeschlagen';

  @override
  String get historyRecognizeImageText => 'Text im Bild erkennen';

  @override
  String get historyCopiedTitle => 'Kopiert';

  @override
  String get historyCopyFailedTitle => 'Kopieren fehlgeschlagen';

  @override
  String get historyPlainTextCopied => 'Reiner Text kopiert.';

  @override
  String get historyClipCopied => 'Eintrag kopiert.';

  @override
  String get historyDownloadedTitle => 'Heruntergeladen';

  @override
  String get historyFileSaved => 'Datei gespeichert.';

  @override
  String get historyDownload => 'Herunterladen';

  @override
  String get historyDownloading => 'Wird heruntergeladen …';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Wird heruntergeladen: $percent %';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent %';
  }

  @override
  String get historyDownloadCancelling => 'Wird abgebrochen …';

  @override
  String get historyDownloadCancellingTooltip => 'Download wird abgebrochen';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Wird heruntergeladen: $percent % · Zum Abbrechen klicken';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Download fehlgeschlagen · Zum Wiederholen klicken';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Download abbrechen, $percent % abgeschlossen';
  }

  @override
  String get historyRetryDownload => 'Download wiederholen';

  @override
  String get historyDownloadFailedTitle => 'Download fehlgeschlagen';

  @override
  String get historyShowFile => 'Datei anzeigen';

  @override
  String get historyMetadataPrivacy => 'Datenschutz';

  @override
  String get historyMetadataConfidential => 'Vertraulich';

  @override
  String get historyMetadataCaptured => 'Erfasst';

  @override
  String get historyMetadataObservedApp => 'Erkannte App';

  @override
  String get historyMetadataDevice => 'Gerät';

  @override
  String get historyMetadataClipType => 'Eintragstyp';

  @override
  String get historyMetadataType => 'Typ';

  @override
  String get historyMetadataPath => 'Pfad';

  @override
  String get historyMetadataFile => 'Datei';

  @override
  String get historyMetadataResolution => 'Auflösung';

  @override
  String get historyMetadataSize => 'Größe';

  @override
  String get historyMetadataFiles => 'Dateien';

  @override
  String get historyMetadataSync => 'Sync';

  @override
  String get historyMetadataTooLargeToSync => 'Zu groß für Sync';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ausgewählte Einträge löschen?',
      one: '$count ausgewählten Eintrag löschen?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Diesen Eintrag löschen?';

  @override
  String get historyBulkTagTooltip => 'Ausgewählte angeheftete Einträge taggen';

  @override
  String get historyBulkExit => 'Auswahl beenden';

  @override
  String get historyBulkPin => 'Ausgewählte anheften';

  @override
  String get historyBulkUnpin => 'Ausgewählte lösen';

  @override
  String get historyBulkDelete => 'Ausgewählte löschen';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ausgewählt',
      one: '$count ausgewählt',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Nach Datum filtern';

  @override
  String get historyDatePresetToday => 'Heute';

  @override
  String get historyDatePresetYesterday => 'Gestern';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Letzte $days Tage',
      one: 'Letzter $days Tag',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'Gesamter Zeitraum';

  @override
  String get appRuntimeStartFailed =>
      'CopyPaste konnte die Laufzeitumgebung nicht starten.';

  @override
  String get appPreparing => 'CopyPaste wird vorbereitet.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste wartet auf Zugriff auf den geschützten Verlauf. Die verfügbaren Teile der App bleiben nutzbar.';

  @override
  String get appHistoryUnavailableTitle => 'Verlaufsdienst nicht verfügbar';

  @override
  String get appHistoryUnavailableMessage =>
      'Vor dem Öffnen des Verlaufs die App-Laufzeitumgebung starten.';

  @override
  String get appDevicesUnavailableTitle => 'Gerätedienst nicht verfügbar';

  @override
  String get appDevicesUnavailableMessage =>
      'Vor dem Öffnen der Geräte die App-Laufzeitumgebung starten.';

  @override
  String get appSettingsUnavailableTitle =>
      'Einstellungsdienst nicht verfügbar';

  @override
  String get appSettingsUnavailableMessage =>
      'Vor dem Öffnen der Einstellungen die App-Laufzeitumgebung starten.';

  @override
  String get appQuit => 'CopyPaste beenden';

  @override
  String get appWindowSetupTitle => 'Problem bei der Fenstereinrichtung';

  @override
  String get appOpenHistory => 'Verlauf öffnen';

  @override
  String get appDestinationHistory => 'Verlauf';

  @override
  String get appDestinationDevices => 'Geräte';

  @override
  String get appDestinationSettings => 'Einstellungen';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, ausgewählt';
  }

  @override
  String get appActiveFilter => 'Aktiver Filter';

  @override
  String get appActionFailed => 'Aktion fehlgeschlagen';

  @override
  String get sharedAndroidVersionRequired =>
      'Android 11 oder neuer erforderlich';

  @override
  String get sharedUseAdbTab => 'Auf diesem Gerät den Tab „ADB“ verwenden.';

  @override
  String get sharedOneTimeAccessApplied => 'Einmaliger Zugriff erteilt';

  @override
  String get sharedInstallShizuku => 'Shizuku installieren';

  @override
  String get sharedPairAndStartShizuku => 'Shizuku koppeln und starten';

  @override
  String get sharedAllowCopyPaste => 'CopyPaste erlauben';

  @override
  String get sharedShizukuNoLongerNeeded => 'Shizuku wird nicht mehr benötigt.';

  @override
  String get sharedGetShizukuDescription =>
      'Shizuku herunterladen, um den einmaligen Zugriff zu erteilen.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'In Shizuku „Debugging über WLAN“ verwenden, dann zurückkehren.';

  @override
  String get sharedApproveInShizuku =>
      'CopyPaste einmalig in Shizuku genehmigen.';

  @override
  String get sharedGetShizuku => 'Shizuku herunterladen';

  @override
  String get sharedOpenShizuku => 'Shizuku öffnen';

  @override
  String get sharedRunOnComputer => 'Auf dem Computer ausführen';

  @override
  String get sharedCopyAllCommands => 'Alle Befehle kopieren';

  @override
  String get sharedEnableUsbDebugging =>
      'USB-Debugging aktivieren und dieses Smartphone verbinden.';

  @override
  String get sharedAllow => 'Erlauben';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Die Einrichtungsbefehle konnten nicht kopiert werden.';

  @override
  String get sharedShizukuOpenFailed => 'Shizuku konnte nicht geöffnet werden.';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => 'CopyPaste einrichten';

  @override
  String get onboardingWelcomeTitle => 'Willkommen bei CopyPaste';

  @override
  String get onboardingWelcomeSubtitle =>
      'Die Zwischenablage – jederzeit bereit.';

  @override
  String get onboardingReadyTitle => 'CopyPaste ist bereit';

  @override
  String get onboardingGetStarted => 'Loslegen';

  @override
  String get onboardingOptionalSetupTitle =>
      'Problem bei optionaler Einrichtung';

  @override
  String get onboardingOpenSystemSettings => 'Einstellungen öffnen';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return 'SCHRITT $step VON $count · $platform';
  }

  @override
  String get onboardingSetupErrorTitle => 'Problem bei der Einrichtung';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste konnte den Einrichtungsstatus von $platform nicht lesen.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste konnte den Fortschritt der Einrichtung nicht speichern.';

  @override
  String get onboardingStartAtLogin => 'Bei Anmeldung starten';

  @override
  String get onboardingStartAtLoginDescription =>
      'CopyPaste bei der Anmeldung öffnen';

  @override
  String get onboardingStartAtLoginSemantics =>
      'CopyPaste bei Anmeldung starten';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Start bei Anmeldung konnte nicht aktiviert werden. Fortfahren ist auch ohne möglich.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Start bei Anmeldung konnte nicht geändert werden. Fortfahren ist auch ohne möglich.';

  @override
  String get onboardingMacosTitle => 'Mac einrichten';

  @override
  String get onboardingAccessibility => 'Bedienungshilfen';

  @override
  String get onboardingAutoPasteEnabled => 'Automatisches Einfügen aktiv';

  @override
  String get onboardingAutoPasteOptional =>
      'Optional für automatisches Einfügen';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'In der installierten CopyPaste-App verfügbar.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'Freigabe als Anmeldeobjekt erforderlich';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'CopyPaste in den Systemeinstellungen erlauben, dann zurückkehren.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste konnte den Einrichtungsstatus von macOS nicht lesen.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Die Berechtigung für Bedienungshilfen konnte nicht angefordert werden.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste konnte den Einrichtungsstatus von macOS nicht aktualisieren.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Der Status der Bedienungshilfen konnte nicht aktualisiert werden.';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Die Einstellungen für Anmeldeobjekte konnten nicht geöffnet werden.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'Start bei Anmeldung erfordert eine Freigabe in den Systemeinstellungen.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Start bei Anmeldung konnte nicht deaktiviert werden.';

  @override
  String get onboardingWindowsTitle => 'PC einrichten';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'In dieser Installation nicht verfügbar. Fortfahren ist auch ohne möglich.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Im Task-Manager deaktiviert';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'CopyPaste in den Windows-Autostart-Apps aktivieren, dann zurückkehren.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'Start bei Anmeldung ist im Task-Manager deaktiviert. Dies kann später in den Windows-Einstellungen aktiviert werden.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'Start bei Anmeldung ist nicht verfügbar. Fortfahren ist auch ohne möglich.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste konnte den Einrichtungsstatus von Windows nicht lesen.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Die Windows-Autostart-Einstellungen konnten nicht geöffnet werden.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Start bei Anmeldung konnte nicht geändert werden. Dies ist später in den Einstellungen möglich.';

  @override
  String get onboardingLinuxChecking => 'Linux-Desktopsitzung wird geprüft.';

  @override
  String get onboardingLinuxTitle => 'Linux-Desktop einrichten';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste benötigt eine verifizierte Desktop-Integration für die Zuordnung der Zwischenablage und Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Desktopsitzung';

  @override
  String get onboardingLinuxX11Detected => 'X11-Sitzung erkannt.';

  @override
  String get onboardingLinuxWaylandDetected => 'Wayland-Sitzung erkannt.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'Keine unterstützte X11- oder Wayland-Desktopsitzung erkannt.';

  @override
  String get onboardingLinuxCompanion =>
      'Begleiterweiterung für GNOME oder KDE';

  @override
  String get onboardingLinuxCompanionReady =>
      'Die authentifizierte Clipboard-v2-Brücke ist bereit.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'Die Erfassung der Zwischenablage ist durch Ausschlüsse von Quell-Apps pausiert. Zum Fortsetzen die signierte Begleiterweiterung aktivieren.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Das signierte Sitzungspaket der Begleiterweiterung installieren und aktivieren, dann hierher zurückkehren.';

  @override
  String get onboardingLinuxOpenSetup => 'Einrichtung öffnen';

  @override
  String get onboardingLinuxKeyboardControl => 'Tastatursteuerung';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'Tastatursteuerung ist für Quick Paste erlaubt.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'Tastatursteuerung ist in dieser Desktopsitzung nicht verfügbar.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Tastatursteuerung erlauben';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste ist bereit.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Quick Paste ist bereit, sobald die erforderliche Desktop-Integration aktiv ist.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'CopyPaste bei der Anmeldung öffnen.';

  @override
  String get onboardingLinuxLinks => 'CopyPaste-Links';

  @override
  String get onboardingLinuxLinksRegistered =>
      'CopyPaste-Links werden in dieser App geöffnet.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'CopyPaste-Links für Kopplungseinladungen registrieren.';

  @override
  String get onboardingLinuxRegisterLinks => 'Links registrieren';

  @override
  String get onboardingLinuxScreenshotBlocking => 'Screenshot-Blockierung';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Screenshot-Blockierung ist unter Linux nicht verfügbar. Geschützte gespeicherte Daten bleiben verschlüsselt.';

  @override
  String get onboardingLinuxCheckAgain => 'Erneut prüfen';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste konnte den Status der Linux-Integration nicht lesen.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Die Einrichtung der Desktop-Integration konnte nicht geöffnet werden.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'Die Berechtigung zur Tastatursteuerung wurde nicht erteilt.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Start bei Anmeldung konnte nicht geändert werden.';

  @override
  String get onboardingLinuxLinksFailed =>
      'CopyPaste-Links konnten nicht registriert werden.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Der Status der Linux-Integration konnte nicht aktualisiert werden.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'In dieser X11-Sitzung ist kein Zugriff auf die Zwischenablage verfügbar.';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste erfordert eine X11- oder Wayland-Desktopsitzung.';

  @override
  String get onboardingAndroidFullCapture => 'Vollständige Erfassung';

  @override
  String get onboardingAndroidBackgroundCapture => 'Hintergrunderfassung';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Einmal einrichten, dann mit einer echten Kopie prüfen.';

  @override
  String get onboardingAndroidLimitedCapture => 'Eingeschränkte Erfassung';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'An CopyPaste teilen oder nach dem Kopieren zurückkehren.';

  @override
  String get onboardingAndroidNotification => 'Erfassungsbenachrichtigung';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Erforderlich, solange die Erfassung läuft';

  @override
  String get onboardingAndroidBackgroundActivity => 'Hintergrundaktivität';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Empfohlen, um Erfassungsverluste zu verringern';

  @override
  String get onboardingAndroidOpen => 'Öffnen';

  @override
  String get onboardingAndroidApplyAccess => 'Erfassungszugriff erteilen';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Einmaligen Erfassungszugriff erteilen.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Die vollständige Erfassung kann später eingerichtet werden.';

  @override
  String get onboardingAndroidCaptureWorking =>
      'Hintergrunderfassung funktioniert';

  @override
  String get onboardingAndroidVerifyCopy => 'Echte Kopie prüfen';

  @override
  String get onboardingAndroidCopyReached =>
      'Eine neue Kopie ist im lokalen Verlauf angekommen.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'In einer anderen App kopieren, dann hierher zurückkehren.';

  @override
  String get onboardingAndroidStartCapture => 'Erfassung starten';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste konnte den Einrichtungsstatus von Android nicht lesen.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Der Erfassungsstatus von Android konnte nicht aktualisiert werden.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Die Überwachung der Android-Einrichtung ist nicht verfügbar.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'Die Übernahme aus der Zwischenablage konnte nicht aktiviert werden.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Die Hintergrunderfassung konnte nicht beendet werden.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Die Akkueinstellungen konnten nicht geöffnet werden.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste ist in Shizuku erlaubt, aber die Android-Erfassungsberechtigungen konnten nicht erteilt werden.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'CopyPaste in Shizuku erlauben, um die Erfassungsberechtigungen zu erteilen.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Zuerst Zugriff auf die Zwischenablage im Hintergrund erteilen.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Vor dem Start der Hintergrunderfassung Benachrichtigungen erlauben.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Die Hintergrunderfassung konnte nicht gestartet werden.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Die Einrichtung der Android-Erfassung konnte nicht geöffnet werden.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Die Auswahl für die Android-Einrichtung konnte nicht gespeichert werden.';

  @override
  String get modulesMarketplaceTab => 'Marktplatz';

  @override
  String get modulesInstalledTab => 'Installiert';

  @override
  String get modulesRefresh => 'Aktualisieren';

  @override
  String get modulesSearchPlaceholder => 'Module durchsuchen';

  @override
  String get modulesOperationFailed => 'Modulvorgang fehlgeschlagen';

  @override
  String get modulesLoading => 'Module werden geladen.';

  @override
  String get modulesUnavailableTitle => 'Module nicht verfügbar';

  @override
  String get modulesUnavailableMessage => 'Module erneut laden.';

  @override
  String get modulesNoneInstalled => 'Keine Module installiert';

  @override
  String get modulesNoMatches => 'Keine passenden Module';

  @override
  String get modulesChooseFromMarketplace =>
      'Ein Modul aus dem Marktplatz wählen.';

  @override
  String get modulesTryDifferentSearch => 'Andere Suchbegriffe versuchen.';

  @override
  String get modulesMarketplaceLoading => 'Marktplatz wird geladen.';

  @override
  String get modulesMarketplaceUnavailableTitle => 'Marktplatz nicht verfügbar';

  @override
  String get modulesMarketplaceUnavailableMessage => 'Marktplatz erneut laden.';

  @override
  String get modulesNoneAvailable => 'Keine Module verfügbar';

  @override
  String get modulesNoneAvailableMessage =>
      'Module erscheinen hier, sobald sie veröffentlicht sind.';

  @override
  String modulesEnableModule(String module) {
    return '$module aktivieren';
  }

  @override
  String get modulesInstall => 'Installieren';

  @override
  String modulesUpdateTo(String version) {
    return 'Auf $version aktualisieren';
  }

  @override
  String get modulesRemoveModule => 'Modul entfernen';

  @override
  String get modulesModuleSettings => 'Moduleinstellungen';

  @override
  String modulesSettingsTitle(String module) {
    return 'Einstellungen für $module';
  }

  @override
  String get modulesDownloadingModel => 'Sprachmodell wird heruntergeladen …';

  @override
  String get modulesRun => 'Ausführen';

  @override
  String modulesRemoveTitle(String module) {
    return '$module entfernen?';
  }

  @override
  String get modulesRemoveMessage =>
      'Das Modul und seine lokalen Einstellungen werden entfernt.';

  @override
  String get modulesRemove => 'Entfernen';

  @override
  String get modulesRemoveFailed => 'Das Modul konnte nicht entfernt werden.';

  @override
  String get modulesCloseSettings => 'Moduleinstellungen schließen';

  @override
  String get modulesRemoved => 'Modul entfernt';

  @override
  String get modulesNoSettingsTitle => 'Keine weiteren Einstellungen';

  @override
  String get modulesNoSettingsMessage =>
      'Dieses Modul benötigt keine zusätzliche Konfiguration.';

  @override
  String get modulesSetUpSmsAccess => 'SMS-Zugriff einrichten';

  @override
  String get modulesRestartApp => 'CopyPaste neu starten';

  @override
  String get modulesSmsAccessTitle => 'SMS-Zugriff';

  @override
  String get modulesCloseSmsAccess => 'SMS-Zugriff schließen';

  @override
  String get modulesSmsChecking => 'SMS-Zugriff wird geprüft.';

  @override
  String get modulesSmsReady =>
      'Der Zugriff ist bereit. SMS Codes aktivieren, um neue Codes zu kopieren.';

  @override
  String get modulesSmsAllowAccess =>
      'Zugriff erlauben, um Codes aus neuen SMS zu kopieren.';

  @override
  String get modulesSmsNotification => 'SMS-Benachrichtigung';

  @override
  String get modulesSmsNotificationDescription =>
      'Erforderlich, solange SMS Codes läuft';

  @override
  String get modulesSmsApplyAccess => 'SMS-Zugriff erteilen';

  @override
  String get modulesSmsApplyAccessDescription =>
      'Einmaligen SMS-Zugriff erteilen.';

  @override
  String get modulesSmsSetupFailed => 'SMS-Einrichtung fehlgeschlagen';

  @override
  String get modulesFileSelectionFailed => 'Dateiauswahl fehlgeschlagen';

  @override
  String get modulesChooseLanguages => 'Sprachen wählen';

  @override
  String get modulesChooseFile => 'Datei wählen';

  @override
  String modulesModelReady(String model) {
    return 'Modell $model ist bereit';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return 'Modell $model · $size MiB Download';
  }

  @override
  String get modulesPlatformsUnknown => 'Plattformen: Unbekannt';

  @override
  String modulesPlatforms(String platforms) {
    return 'Plattformen: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'Die Signatur des Modulkatalogs ist ungültig.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Verbindung zum Modul-Marktplatz fehlgeschlagen.';

  @override
  String get modulesMarketplaceTimeout =>
      'Der Modul-Marktplatz hat nicht geantwortet.';

  @override
  String get modulesIncompatible =>
      'Dieses Modul ist mit diesem Gerät nicht kompatibel.';

  @override
  String get modulesPackageSizeInvalid =>
      'Die Größe des Modulpakets ist ungültig.';

  @override
  String get modulesDownloadInterrupted =>
      'Der Modul-Download wurde unterbrochen.';

  @override
  String get modulesDownloadTimeout =>
      'Der Modul-Download hat nicht geantwortet.';

  @override
  String get modulesPackageUrlUntrusted =>
      'Die URL des Modulpakets ist nicht vertrauenswürdig.';

  @override
  String get modulesDownloadLeftGithub =>
      'Der Modul-Download hat GitHub verlassen.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'Der Modul-Download wurde zu oft umgeleitet.';

  @override
  String get modulesDownloadTooLarge => 'Der Modul-Download ist zu groß.';

  @override
  String get modulesPackageIntegrityFailed =>
      'Das Modulpaket hat die Integritätsprüfung nicht bestanden.';

  @override
  String get modulesDownloadUnavailable =>
      'Dieser Modul-Download ist derzeit nicht verfügbar.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'Der Modul-Marktplatz ist nicht verfügbar. Erneut versuchen.';

  @override
  String get modulesCatalogInvalid => 'Der Modulkatalog ist ungültig.';

  @override
  String get modulesThisSystem => 'dieses System';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version oder neuer';
  }

  @override
  String get modulesNotAvailableForDevice =>
      'Für dieses Gerät nicht verfügbar.';

  @override
  String get modulesAppVersionUnverified =>
      'Die installierte CopyPaste-Version konnte nicht überprüft werden.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Dieses Modul unterstützt CopyPaste $version nicht.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Erfordert $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed =>
      'Der Modul-Marktplatz konnte nicht geladen werden.';

  @override
  String get modulesSmsSetupRequired =>
      'Vor dem Aktivieren dieses Moduls den SMS-Zugriff einrichten.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'Die SMS-Überwachung konnte nicht gestartet werden. Das Modul ist weiterhin aktiviert; für einen neuen Versuch erneut deaktivieren.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'Die SMS-Überwachung konnte nicht gestartet werden. Das Modul wurde deaktiviert.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'Die SMS-Überwachung konnte nicht gestartet werden. Der Aktivierungsstatus des Moduls konnte nicht bestätigt werden.';

  @override
  String get modulesSmsMonitoringFailed =>
      'Die SMS-Überwachung konnte nicht gestartet werden.';

  @override
  String get modulesSmsAndroidOnly =>
      'SMS-Zugriff ist nur unter Android verfügbar.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'Die SMS-Überwachung muss eingerichtet werden.';

  @override
  String get modulesOperationIncomplete =>
      'Der Modulvorgang konnte nicht abgeschlossen werden.';

  @override
  String get modulesUnsupportedEvent =>
      'Das Modul verwendet ein nicht unterstütztes Ereignis.';

  @override
  String get modulesUnsupportedResult =>
      'Das Modul hat ein nicht unterstütztes Ergebnis geliefert.';

  @override
  String get modulesUnsupportedField =>
      'Das Modul verwendet ein nicht unterstütztes Feld.';

  @override
  String get modulesResponseInvalid => 'Die Antwort des Moduls ist ungültig.';

  @override
  String get modulesFileSelectionUnavailable =>
      'Dateiauswahl ist nicht verfügbar.';

  @override
  String get modulesFileSelectFailed =>
      'Die Datei konnte nicht ausgewählt werden.';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field ist keine zulässige Datei.';
  }

  @override
  String get modulesSmsVerifyFailed =>
      'Der SMS-Zugriff konnte nicht überprüft werden.';

  @override
  String get modulesSmsAllowNotifications =>
      'Benachrichtigungen erlauben, damit SMS Codes im Hintergrund aktiv bleibt.';

  @override
  String get modulesSmsNotGranted => 'Der SMS-Zugriff wurde nicht erteilt.';

  @override
  String get modulesSmsConfigureFailed =>
      'Der SMS-Zugriff konnte nicht eingerichtet werden.';

  @override
  String get onboardingLinuxContinueBasic =>
      'Mit einfacher Erfassung fortfahren';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Optional. Erforderlich für Quick Paste und Ausschlüsse von Quell-Apps.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Optional. Erforderlich, damit Quick Paste zur vorherigen App zurückkehren kann.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'In dieser Wayland-Sitzung ist kein Zugriff auf die Zwischenablage verfügbar. Die signierte Begleiterweiterung für GNOME oder KDE aktivieren, dann erneut prüfen.';

  @override
  String get historyMoreActions => 'Weitere Aktionen';

  @override
  String get historyTagsAction => 'Tags …';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'geheime Einträge zu entsperren',
      'other': 'Geheime Einträge entsperren',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'ein verborgenes Snippet zu verwenden',
      'other': 'Verborgenes Snippet verwenden',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'einen geheimen Eintrag zu kopieren',
      'other': 'Geheimen Eintrag kopieren',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'Auf diesem System nicht verfügbar';

  @override
  String get deviceAuthNotConfigured =>
      'Zuerst ein Gerätepasswort, eine PIN oder Biometrie einrichten';

  @override
  String get commonOn => 'Ein';

  @override
  String get commonOff => 'Aus';

  @override
  String get settingsQuickPasteStartFailed =>
      'Quick Paste konnte nicht gestartet werden, daher funktioniert das Tastenkürzel nicht.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut wird von einer anderen App verwendet. Wählen Sie ein anderes Tastenkürzel.';
  }

  @override
  String get devicesEmptyTitle => 'Noch keine weiteren Geräte gekoppelt';

  @override
  String get devicesEmptyMessage =>
      'Kopple ein Gerät im selben Netzwerk, um den Verlauf zu synchronisieren.';

  @override
  String get pairingStateNotCompleted => 'Kopplung nicht abgeschlossen';

  @override
  String get pairingStepInvite => 'Einladung';

  @override
  String get pairingStepScan => 'Scan';

  @override
  String get pairingStepCode => 'Code';

  @override
  String get pairingStepRequest => 'Anfrage';

  @override
  String get pairingStepConnect => 'Verbindung';

  @override
  String get pairingStepConfirm => 'Bestätigung';

  @override
  String get pairingStepPaired => 'Gekoppelt';

  @override
  String get pairingOtherDevice => 'Anderes Gerät';

  @override
  String get pairingDecisionNotConfirmed => 'Noch nicht bestätigt';

  @override
  String get pairingDecisionSending => 'Entscheidung wird gesendet …';

  @override
  String get pairingDecisionConfirmed => 'Bestätigt';

  @override
  String get pairingDecisionRejected => 'Abgelehnt';

  @override
  String get pairingDecisionWaiting => 'Warten auf Bestätigung';

  @override
  String get pairingDecisionUnknown => 'Antwort nicht bekannt';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Geräte',
      one: '$count Gerät',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Zeilen',
      one: '$count Zeile',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Hauptnavigation';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Schritt $step von $count: $name';
  }

  @override
  String get modulesPhaseDownload => 'Download';

  @override
  String get modulesPhaseInstall => 'Installation';

  @override
  String modulesInstallingModule(String module) {
    return '$module wird installiert';
  }

  @override
  String modulesUpdatingModule(String module) {
    return '$module wird aktualisiert';
  }

  @override
  String get commonDismiss => 'Ausblenden';

  @override
  String get settingsRetentionConfirmTitle => 'Älteren Verlauf entfernen?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Neues Limit: $limit. Ältere, nicht angeheftete Einträge werden endgültig gelöscht.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle =>
      'Speicherkontingent verringern?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Neues Kontingent: $size. Die ältesten nicht angehefteten Einträge darüber werden endgültig gelöscht.';
  }

  @override
  String get settingsLimitConfirmAction => 'Limit anwenden';

  @override
  String get settingsHideEverywhereTitle => 'CopyPaste vollständig ausblenden?';

  @override
  String get settingsHideEverywhereMessage =>
      'Sind beide aus, gibt es kein Symbol. Starte CopyPaste erneut, um es zu öffnen.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Sind beide aus, gibt es kein Symbol. Drücke zum Öffnen $shortcut oder starte CopyPaste erneut.';
  }

  @override
  String get settingsHideEverywhereAction => 'Ausblenden';

  @override
  String get settingsExportDoneTitle => 'Textverlauf exportiert';

  @override
  String get settingsBackupDoneTitle => 'Backup erstellt';

  @override
  String get settingsRestoreDoneTitle => 'Verlauf wiederhergestellt';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Texteinträge exportiert',
      one: '$count Texteintrag exportiert',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count konnten nicht gelesen werden',
      one: '$count konnte nicht gelesen werden',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Verschlüsseltes Backup gespeichert · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'Der Zwischenablageverlauf auf diesem Gerät entspricht jetzt dem Backup.';

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
    return '$used von $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return '$used von $quota belegt';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '$used belegt';
  }

  @override
  String get storageUsageOverQuota => 'Kontingent überschritten';

  @override
  String storageUsageOldestClip(String date) {
    return 'Ältester Eintrag: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Speicher: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Neues Kontingent: $size. Belegt sind $used, daher werden die ältesten nicht angehefteten Einträge endgültig gelöscht.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Einträge',
      one: '$count Eintrag',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'CopyPaste öffnen';

  @override
  String get trayOpenQuickPaste => 'Quick Paste öffnen';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Quick Paste öffnen ($shortcut)';
  }

  @override
  String get trayNewSnippet => 'Neuer Textbaustein';

  @override
  String get trayRecent => 'Zuletzt';

  @override
  String get trayRecentLoading => 'Wird geladen…';

  @override
  String get trayRecentFailed => 'Clips konnten nicht geladen werden';

  @override
  String get trayRecentEmpty => 'Noch keine Clips';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count letzte Clips ausgeblendet',
      one: '1 letzter Clip ausgeblendet',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Geheim';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Neuesten Clip von anderem Gerät kopieren';

  @override
  String get trayCaptureNow => 'Jetzt erfassen';

  @override
  String trayPausedUntil(String time) {
    return 'Pausiert bis $time';
  }

  @override
  String get trayPauseFor => 'Pausieren für…';

  @override
  String get trayPauseFor15Minutes => '15 Minuten';

  @override
  String get trayPauseForOneHour => '1 Stunde';

  @override
  String get trayPauseUntilResumed => 'Bis ich fortsetze';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count Einträge',
      one: 'Paste Queue: 1 Eintrag',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Sammeln: $count Einträge',
      one: 'Sammeln: 1 Eintrag',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Kürzlich übersprungen ($count)';
  }

  @override
  String get trayOpenContentRules => 'Inhaltsregeln öffnen';

  @override
  String get trayRecentSettingsTitle => 'Menüleiste und Systemleiste';

  @override
  String get trayRecentSettingsDescription =>
      'Was das Menü in der Menüleiste oder Systemleiste unter „Zuletzt“ anzeigt.';

  @override
  String get trayRecentCountTitle => 'Letzte Clips';

  @override
  String get trayRecentCountDescription =>
      'Wie viele der neuesten Clips das Menü anzeigt.';

  @override
  String get trayRecentPreviewTitle => 'Vorschau letzter Clips';

  @override
  String get trayRecentPreviewDescription =>
      'Wie viel Clip-Text das Menü zeigt.';

  @override
  String get trayRecentPreviewFull => 'Vollständige Vorschau';

  @override
  String get trayRecentPreviewTitles => 'Nur Typ und Quelle';

  @override
  String get trayRecentPreviewHidden => 'Ausgeblendet';

  @override
  String get trayRecentSettingsKeywords =>
      'systemleiste menüleiste zuletzt vorschau datenschutz anzahl tray';

  @override
  String get transformsPaletteTitle => 'Text umwandeln';

  @override
  String get transformsFilterPlaceholder => 'Umwandlungen filtern';

  @override
  String get transformsNoMatches => 'Keine passenden Umwandlungen';

  @override
  String get transformsGroupResults => 'Ergebnisse';

  @override
  String get transformsGroupSuggested => 'Vorgeschlagen';

  @override
  String get transformsGroupRecent => 'Zuletzt verwendet';

  @override
  String get transformsGroupFavorites => 'Favoriten';

  @override
  String get transformsGroupPipelines => 'Pipelines';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV und XML';

  @override
  String get transformsCategoryEncode => 'Kodieren und dekodieren';

  @override
  String get transformsCategoryCase => 'Groß-/Kleinschreibung';

  @override
  String get transformsCategoryHash => 'Hashes';

  @override
  String get transformsCategoryLines => 'Zeilen und Leerraum';

  @override
  String get transformsCategoryClean => 'Bereinigen';

  @override
  String get transformsCategoryUrl => 'Links';

  @override
  String get transformsCategoryMarkup => 'Rich Text und Markdown';

  @override
  String get transformsCategoryDev => 'Entwickler';

  @override
  String get transformsCategoryOther => 'Sonstige';

  @override
  String get transformsChainEmpty => 'Noch keine Schritte';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Dann: $name';
  }

  @override
  String get transformsStepEnabled => 'Diesen Schritt ausführen';

  @override
  String get transformsMoveEarlier => 'Nach oben';

  @override
  String get transformsMoveLater => 'Nach unten';

  @override
  String get transformsRemoveStep => 'Schritt entfernen';

  @override
  String get transformsSavePipeline => 'Als Pipeline speichern';

  @override
  String get transformsSavePipelineTitle => 'Pipeline speichern';

  @override
  String get transformsSavePipelineFailed =>
      'Die Pipeline konnte nicht gespeichert werden.';

  @override
  String get transformsPipelineNameLabel => 'Name';

  @override
  String get transformsPipelineDescriptionLabel => 'Beschreibung (optional)';

  @override
  String get transformsPipelineStepsLabel => 'Schritte';

  @override
  String get transformsPipelineNoSteps =>
      'Fügen Sie mindestens einen Schritt hinzu.';

  @override
  String get transformsPipelineNameRequired => 'Geben Sie einen Namen ein.';

  @override
  String get transformsPipelineNameTooLong => 'Höchstens 80 Zeichen.';

  @override
  String get transformsPipelineNameTaken =>
      'Eine andere Pipeline hat bereits diesen Namen.';

  @override
  String get transformsPipelineSavedTitle => 'Pipeline gespeichert';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name ist in jeder Umwandlungspalette verfügbar.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Schritte: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Gelöschte Pipeline';

  @override
  String get transformsNeedsNewerVersion =>
      'Erfordert eine neuere CopyPaste-Version';

  @override
  String get transformsLoadFailed =>
      'Umwandlungen konnten nicht geladen werden.';

  @override
  String get transformsNotTextTitle => 'Umwandlungen funktionieren mit Text';

  @override
  String get transformsNotTextMessage =>
      'Dieses Element ist ein Bild, eine Datei oder eine Farbe. Wählen Sie ein Textelement oder kopieren Sie Text.';

  @override
  String get transformsNothingTitle => 'Nichts umzuwandeln';

  @override
  String get transformsNothingMessage =>
      'Kopieren Sie Text oder wählen Sie ein Element aus dem Verlauf.';

  @override
  String get transformsPickTransform => 'Wählen Sie eine Umwandlung';

  @override
  String get transformsPickTransformHint =>
      'Tab fügt sie als Schritt hinzu. Enter übernimmt das Ergebnis.';

  @override
  String get transformsPreviewing => 'Vorschau wird aktualisiert…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Zeilen: $lines · Wörter: $words · Zeichen: $chars';
  }

  @override
  String get transformsNoChanges => 'Keine Änderungen';

  @override
  String transformsTruncated(String size) {
    return 'Die ersten 64 KiB von $size werden angezeigt';
  }

  @override
  String get transformsRandom => 'Zufällig';

  @override
  String get transformsNotRun => 'Nicht ausgeführt';

  @override
  String get transformsSensitiveHidden =>
      'Das Ergebnis ist vertraulich und ausgeblendet.';

  @override
  String get transformsReveal => 'Anzeigen';

  @override
  String get transformsPaneOutput => 'Ausgabe';

  @override
  String get transformsPaneInput => 'Eingabe';

  @override
  String get transformsPaneChanges => 'Änderungen';

  @override
  String get transformsCannotTransform =>
      'Dieser Text kann nicht umgewandelt werden';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Schritt $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Als Hex anzeigen';

  @override
  String get transformsActionPaste => 'Einfügen';

  @override
  String get transformsActionSave => 'Als neuen Eintrag speichern';

  @override
  String get transformsCopiedMessage =>
      'Das Ergebnis ist in der Zwischenablage und im Verlauf.';

  @override
  String get transformsSavedTitle => 'Gespeichert';

  @override
  String get transformsSavedMessage => 'Als neuer Verlaufseintrag gespeichert.';

  @override
  String get transformsPasteFailedTitle => 'Einfügen nicht möglich';

  @override
  String get transformsPasteFailedMessage =>
      'Das Ergebnis ist in der Zwischenablage. Fügen Sie es manuell ein.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message in Zeile $line, Spalte $column';
  }

  @override
  String get transformsErrorBinary =>
      'Die dekodierten Daten sind binär, kein Text';

  @override
  String get transformsErrorTooLarge => 'Zu groß zum Umwandeln';

  @override
  String get transformsErrorInvalidParam =>
      'Eine Einstellung dieses Schritts ist ungültig';

  @override
  String get transformsErrorCancelled => 'Abgebrochen';

  @override
  String get transformsErrorTooExpensive => 'Muster zu aufwendig';

  @override
  String get transformsErrorPattern => 'Das Muster ist ungültig';

  @override
  String get transformsErrorTimestampUnit =>
      'Wählen Sie, ob die Zahl in Sekunden oder Millisekunden angegeben ist';

  @override
  String get transformsErrorExternalEntity =>
      'Externe Entitäten sind nicht erlaubt';

  @override
  String get transformsErrorJwtEncrypted =>
      'Verschlüsselte Token können nicht dekodiert werden';

  @override
  String get transformsErrorJson => 'Ungültiges JSON';

  @override
  String get transformsErrorYaml => 'Ungültiges YAML';

  @override
  String get transformsErrorXml => 'Ungültiges XML';

  @override
  String get transformsErrorCsv => 'Ungültiges CSV';

  @override
  String get transformsErrorBase64 => 'Ungültiges Base64';

  @override
  String get transformsErrorHex => 'Ungültiges Hex';

  @override
  String get transformsErrorEscape => 'Ungültige Escape-Sequenz';

  @override
  String get transformsErrorUrl => 'Fehlerhafte Prozent-Kodierung';

  @override
  String get transformsErrorJwt => 'Kein gültiges JWT';

  @override
  String get transformsErrorDate => 'Kein erkanntes Datum und kein Zeitstempel';

  @override
  String get transformsErrorGeneric =>
      'Dieser Schritt kann seine Eingabe nicht umwandeln';

  @override
  String get transformsWarningSignedLink => 'Signierter Link unverändert';

  @override
  String get transformsWarningExpired => 'Abgelaufen';

  @override
  String get transformsWarningNotYetValid => 'Noch nicht gültig';

  @override
  String get transformsWarningSignatureNotVerified => 'Signatur nicht geprüft';

  @override
  String get transformsWarningDuplicateKeys => 'Doppelte Schlüssel';

  @override
  String get transformsWarningLenientJson =>
      'Nicht standardkonformes JSON akzeptiert';

  @override
  String get transformsWarningDoubleEncoded => 'Das JSON wurde doppelt kodiert';

  @override
  String get transformsWarningEscapes =>
      'Unbekannte Escape-Sequenzen unverändert übernommen';

  @override
  String get transformsWarningLocalZone => 'Lokale Zeitzone angenommen';

  @override
  String get transformsWarningNoMatches => 'Keine Treffer';

  @override
  String get transformsWarningGeneric => 'Prüfen Sie das Ergebnis';

  @override
  String get transformsRestoreTitle =>
      'Zwischenablage nach dem Einfügen wiederherstellen';

  @override
  String get transformsRestoreDescription =>
      'Vorherige Zwischenablage nach dem Einfügen wiederherstellen.';

  @override
  String get transformsDefaultTitle => 'Standardumwandlung beim Einfügen';

  @override
  String get transformsDefaultDescription =>
      'Wird auf Text angewendet, den du aus Quick Paste einfügst.';

  @override
  String get transformsNoDefault => 'Keine';

  @override
  String get transformsAppDefaultsTitle => 'Standards pro App';

  @override
  String get transformsAppDefaultsDescription =>
      'Einen anderen Standard für bestimmte Apps verwenden.';

  @override
  String transformsOtherPlatform(String platform) {
    return 'nur auf $platform';
  }

  @override
  String get transformsAddApp => 'App hinzufügen';

  @override
  String get transformsRemove => 'Entfernen';

  @override
  String get transformsFavoritesTitle => 'Favoriten';

  @override
  String get transformsFavoritesDescription =>
      'Erscheinen in jeder Palette zuerst.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Mod+$number in der Palette';
  }

  @override
  String get transformsAddFavorite => 'Favorit hinzufügen';

  @override
  String get transformsPipelinesTitle => 'Pipelines';

  @override
  String get transformsPipelinesDescription =>
      'Gespeicherte Ketten von Umwandlungen.';

  @override
  String get transformsNewPipeline => 'Neue Pipeline';

  @override
  String get transformsEditPipeline => 'Pipeline bearbeiten';

  @override
  String get transformsDuplicatePipeline => 'Duplizieren';

  @override
  String get transformsDeletePipeline => 'Löschen';

  @override
  String transformsDeletePipelineTitle(String name) {
    return '$name löschen?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Favoriten und Einfüge-Standards, die sie nutzen, werden entfernt.';

  @override
  String get transformsDeletePipelineFailed =>
      'Die Pipeline konnte nicht gelöscht werden.';

  @override
  String get transformsAddStep => 'Schritt hinzufügen';

  @override
  String get transformsHiddenTitle => 'Ausgeblendete Umwandlungen';

  @override
  String get transformsHiddenDescription =>
      'Erscheinen nicht in Paletten und Vorschlägen.';

  @override
  String get transformsHideTransform => 'Umwandlung ausblenden';

  @override
  String get transformsShow => 'Einblenden';

  @override
  String get transformsSuggestionsTitle => 'Vorschläge';

  @override
  String get transformsSuggestionsDescription =>
      'Passende Umwandlungen für den Text vorschlagen.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web Tokens';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl => 'Links mit Tracking-Parametern';

  @override
  String get transformsDetectorTimestamp => 'Zeitstempel';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Tracking-Parameter';

  @override
  String get transformsTrackingDescription =>
      'Link-Parameter, die zusätzlich entfernt oder immer behalten werden.';

  @override
  String get transformsTrackingRemoveLabel => 'Zusätzlich entfernen';

  @override
  String get transformsTrackingAllowLabel => 'Immer behalten';

  @override
  String get transformsTrackingPlaceholder =>
      'name oder name@domain, dann Enter';

  @override
  String get quickAccessUnavailableTitle =>
      'Schnellzugriffsstatus nicht verfügbar';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste konnte den Status von Tastatur, Kacheln und Widget nicht lesen. Versuche es erneut.';

  @override
  String get quickAccessActionFailedTitle => 'Aktion nicht abgeschlossen';

  @override
  String get quickAccessActionFailedMessage =>
      'Android hat den gewünschten Bildschirm nicht geöffnet oder die Änderung nicht gespeichert. Versuche es erneut.';

  @override
  String get quickAccessKeyboardTitle => 'Einträge-Tastatur';

  @override
  String get quickAccessKeyboardDescription =>
      'Aktiviere „CopyPaste-Einträge“ in den Tastatureinstellungen.';

  @override
  String get quickAccessKeyboardOn => 'Aktiviert';

  @override
  String get quickAccessKeyboardOff => 'Deaktiviert';

  @override
  String get quickAccessOpenKeyboardSettings => 'Tastatureinstellungen öffnen';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'Das Tastaturfeld „CopyPaste-Einträge“ aktivieren.';

  @override
  String get quickAccessKeyboardKeywords =>
      'tastatur eingabemethode ime feld aktivieren';

  @override
  String get quickAccessSwitchKeyboardTitle => 'Zur Einträge-Tastatur wechseln';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'Mache „CopyPaste-Einträge“ zur aktuellen Tastatur.';

  @override
  String get quickAccessSwitchKeyboardAction => 'Tastatur wählen';

  @override
  String get quickAccessKeyboardCurrent => 'Aktuelle Tastatur';

  @override
  String get quickAccessCaptureTierTitle => 'Erfassungsmodus';

  @override
  String get quickAccessCaptureTierDescription =>
      'Wie Kopien aus anderen Apps gespeichert werden.';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'Sieh, wie Kopien aus anderen Apps gespeichert werden.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'erfassung stufe vollständig feld manuell shizuku sammeln';

  @override
  String get quickAccessTierFull =>
      'Vollständig: Jede Kopie wird im Hintergrund gespeichert';

  @override
  String get quickAccessTierPanelCollecting => 'Feld ausgewählt: sammelt';

  @override
  String get quickAccessTierPanelCatchUp => 'Feldmodus: Abgleich beim Öffnen';

  @override
  String get quickAccessTierManual =>
      'Manuell: über die App, eine Kachel, das Widget oder „Teilen“ speichern';

  @override
  String get quickAccessTierPaused => 'Erfassung pausiert';

  @override
  String get quickAccessAutoReturnTitle =>
      'Nach dem Einfügen zur vorherigen Tastatur zurückkehren';

  @override
  String get quickAccessAutoReturnDescription =>
      'Nach dem Einfügen eines Eintrags zurückwechseln.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'zurück vorherige tastatur automatisch einfügen';

  @override
  String get quickAccessFocusSearchTitle =>
      'Suche beim Öffnen des Felds fokussieren';

  @override
  String get quickAccessFocusSearchDescription =>
      'Öffnet das Panel mit aktiver Suche.';

  @override
  String get quickAccessTilesTitle => 'Schnelleinstellungs-Kacheln';

  @override
  String get quickAccessTilesSearchDescription =>
      'Die Kacheln „Zwischenablage speichern“ und „Erfassung der Zwischenablage“ hinzufügen.';

  @override
  String get quickAccessTilesKeywords =>
      'schnelleinstellungen kachel benachrichtigungsleiste speichern pause';

  @override
  String get quickAccessTileInstructions =>
      'Wische zweimal nach unten, tippe auf „Bearbeiten“ und füge eine CopyPaste-Kachel hinzu.';

  @override
  String get quickAccessSaveTileTitle => 'Kachel „Zwischenablage speichern“';

  @override
  String get quickAccessSaveTileDescription =>
      'Speichert den Inhalt der Zwischenablage über die Schnelleinstellungen.';

  @override
  String get quickAccessCaptureTileTitle =>
      'Kachel „Erfassung der Zwischenablage“';

  @override
  String get quickAccessCaptureTileDescription =>
      'Pausiert und setzt die Erfassung über die Schnelleinstellungen fort.';

  @override
  String get quickAccessAddTile => 'Kachel hinzufügen';

  @override
  String get quickAccessTileAdded => 'Zu den Schnelleinstellungen hinzugefügt';

  @override
  String get quickAccessWidgetTitle => 'Widget „Letzte Einträge“';

  @override
  String get quickAccessWidgetDescription =>
      'Neueste Einträge auf dem Startbildschirm, mit einem Tippen kopiert.';

  @override
  String get quickAccessWidgetInstructions =>
      'Halte den Startbildschirm gedrückt, tippe auf „Widgets“ und füge „Letzte Einträge“ hinzu.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Widgets hinzugefügt',
      one: '1 Widget hinzugefügt',
      zero: 'Nicht hinzugefügt',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Widget hinzufügen';

  @override
  String get quickAccessWidgetKeywords =>
      'startbildschirm widget launcher letzte';

  @override
  String snippetsAbbreviationBadge(String abbreviation) {
    return 'Kürzel $abbreviation';
  }

  @override
  String get snippetsAbbreviationLabel => 'Kürzel';

  @override
  String get snippetsAbbreviationPlaceholder => 'Optional, zum Beispiel mfg';

  @override
  String snippetsAbbreviationTaken(String title) {
    return 'Bereits verwendet von „$title“';
  }

  @override
  String snippetsAbbreviationTooLong(int max) {
    return 'Höchstens $max Zeichen verwenden.';
  }

  @override
  String get snippetsAbbreviationWhitespace =>
      'Ein Kürzel darf keine Leer- oder Steuerzeichen enthalten.';

  @override
  String snippetsAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          '$count Textbausteine müssen nach einer Synchronisierung geprüft werden',
      one: '1 Textbaustein muss nach einer Synchronisierung geprüft werden',
      zero: 'Keine Textbausteine zu prüfen',
    );
    return '$_temp0';
  }

  @override
  String get snippetsBodyLabel => 'Text';

  @override
  String get snippetsBodyPlaceholder => 'Text des Textbausteins eingeben';

  @override
  String get snippetsClearSelection => 'Auswahl aufheben';

  @override
  String get snippetsClipTooLarge =>
      'Zu groß zum Bearbeiten. Dieser Clip wird schreibgeschützt geöffnet.';

  @override
  String snippetsCollectionAccent(String accent) {
    String _temp0 = intl.Intl.selectLogic(accent, {
      'neutral': 'Neutral',
      'blue': 'Blau',
      'green': 'Grün',
      'amber': 'Bernstein',
      'red': 'Rot',
      'other': 'Neutral',
    });
    return '$_temp0';
  }

  @override
  String get snippetsCollectionAccentLabel => 'Akzentfarbe';

  @override
  String get snippetsCollectionIcon => 'Symbol';

  @override
  String get snippetsCollectionLabel => 'Sammlung';

  @override
  String get snippetsCollectionName => 'Name';

  @override
  String get snippetsCollectionNameTaken =>
      'Eine Sammlung mit diesem Namen gibt es bereits.';

  @override
  String snippetsCollectionNameTooLong(int max) {
    return 'Höchstens $max Zeichen verwenden.';
  }

  @override
  String get snippetsConcealedBody => 'Dieser Textbaustein ist verborgen.';

  @override
  String get snippetsConcealedConfirmAction => 'Fortfahren';

  @override
  String get snippetsConcealedConfirmMessage =>
      'Dieses Gerät kann dich nicht prüfen. Fahre nur auf deinem eigenen Gerät fort.';

  @override
  String get snippetsConcealedConfirmTitle => 'Bestätige, dass du es bist';

  @override
  String get snippetsConcealedPreview => 'Verborgener Textbaustein';

  @override
  String get snippetsConcealedSwitch => 'Verborgen';

  @override
  String get snippetsConcealedTitle => 'Verborgener Textbaustein';

  @override
  String get snippetsConflictBadge => 'Konfliktkopie';

  @override
  String get snippetsCopied => 'Kopiert';

  @override
  String get snippetsCreate => 'Textbaustein erstellen';

  @override
  String snippetsCreateNamed(String query) {
    return 'Textbaustein „$query“ erstellen';
  }

  @override
  String get snippetsDeleteCollection => 'Sammlung löschen';

  @override
  String get snippetsDeleteCollectionMove =>
      'Textbausteine nach „Ohne Sammlung“ verschieben';

  @override
  String snippetsDeleteCollectionTitle(String name) {
    return '„$name“ löschen?';
  }

  @override
  String get snippetsDeleteCollectionWithSnippets =>
      'Textbausteine ebenfalls löschen';

  @override
  String get snippetsDeletedMessage => 'In „Zuletzt gelöscht“ verschoben.';

  @override
  String snippetsDeletedTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Textbausteine gelöscht',
      one: 'Textbaustein gelöscht',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDestination => 'Speichern in';

  @override
  String snippetsDestinationName(String destination) {
    String _temp0 = intl.Intl.selectLogic(destination, {
      'historyClip': 'Clip im Verlauf',
      'snippet': 'Textbaustein',
      'other': 'Clip im Verlauf',
    });
    return '$_temp0';
  }

  @override
  String get snippetsDiscard => 'Verwerfen';

  @override
  String snippetsDragging(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Textbausteine',
      one: '1 Textbaustein',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDuplicateAbbreviation =>
      'Ein anderer Textbaustein hat dieses Kürzel. Ändere eines davon.';

  @override
  String get snippetsDuplicateBadge => 'Doppeltes Kürzel';

  @override
  String get snippetsEditClip => 'Clip bearbeiten';

  @override
  String get snippetsEditCollection => 'Sammlung bearbeiten';

  @override
  String get snippetsEditSnippet => 'Textbaustein bearbeiten';

  @override
  String get snippetsEditTab => 'Bearbeiten';

  @override
  String get snippetsEmptyMessage =>
      'Gib einem Textbaustein ein Kürzel und tippe es dann in Quick Paste.';

  @override
  String get snippetsEmptyTitle => 'Noch keine Textbausteine';

  @override
  String get snippetsEnableTemplate => 'Einschalten';

  @override
  String get snippetsEnableTemplateMessage =>
      'Text, der wie ein Platzhalter aussieht, wird ausgefüllt.';

  @override
  String get snippetsEnableTemplateTitle => 'Vorlage einschalten?';

  @override
  String get snippetsExport => 'Exportieren';

  @override
  String get snippetsExportCollection => 'Sammlung exportieren';

  @override
  String get snippetsExportConcealedConfirm =>
      'Verborgene Textbausteine in die lesbare Datei aufnehmen';

  @override
  String snippetsExportCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Textbausteine geschrieben',
      one: '1 Textbaustein geschrieben',
      zero: 'Keine Textbausteine geschrieben',
    );
    return '$_temp0';
  }

  @override
  String get snippetsExportDone => 'Textbausteine exportiert';

  @override
  String get snippetsExportEncrypt => 'Mit Passphrase verschlüsseln';

  @override
  String get snippetsExportFailed => 'Export fehlgeschlagen';

  @override
  String get snippetsExportPlainConfirm =>
      'Mir ist klar, dass jeder mit der Datei sie lesen kann';

  @override
  String snippetsExportScope(String scope, int count) {
    String _temp0 = intl.Intl.selectLogic(scope, {
      'all': 'Alle Textbausteine',
      'collections': 'Diese Sammlung',
      'snippets': 'Ausgewählte Textbausteine ($count)',
      'other': 'Alle Textbausteine',
    });
    return '$_temp0';
  }

  @override
  String get snippetsExportTitle => 'Textbausteine exportieren';

  @override
  String get snippetsFieldBuilderTitle => 'Feld einfügen';

  @override
  String get snippetsFieldDefault => 'Standardwert';

  @override
  String get snippetsFieldName => 'Name';

  @override
  String get snippetsFieldOptions => 'Optionen, durch Kommas getrennt';

  @override
  String get snippetsFieldRequired => 'Fülle dieses Feld aus.';

  @override
  String get snippetsFieldType => 'Typ';

  @override
  String snippetsFieldTypeName(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'text': 'Eine Zeile',
      'multiline': 'Mehrere Zeilen',
      'choice': 'Auswahl',
      'other': 'Eine Zeile',
    });
    return '$_temp0';
  }

  @override
  String get snippetsFormattingNotice =>
      'Die Formatierung wird beim Speichern entfernt.';

  @override
  String get snippetsGoToSnippet => 'Zum Textbaustein';

  @override
  String get snippetsImport => 'Importieren';

  @override
  String get snippetsImportAbbreviationPolicy => 'Kürzel bereits verwendet';

  @override
  String snippetsImportAbbreviationPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'importWithout': 'Ohne Kürzel importieren',
      'renameWithSuffix': 'Zahl an das Kürzel anhängen',
      'skip': 'Überspringen',
      'other': 'Ohne Kürzel importieren',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportCollectionPolicy => 'Sammlung mit demselben Namen';

  @override
  String snippetsImportCollectionPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'merge': 'Zur vorhandenen hinzufügen',
      'createNew': 'Neue erstellen',
      'other': 'Zur vorhandenen hinzufügen',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportConvert => 'Bekannte Platzhalter umwandeln';

  @override
  String snippetsImportCounts(
    int added,
    int identical,
    int updated,
    int conflicts,
    int invalid,
  ) {
    return 'Neu: $added. Identisch, übersprungen: $identical. Geändert: $updated. Kürzelkonflikte: $conflicts. Ungültig: $invalid.';
  }

  @override
  String get snippetsImportDone => 'Textbausteine importiert';

  @override
  String get snippetsImportFailed => 'Import fehlgeschlagen';

  @override
  String get snippetsImportFileType => 'Textbaustein-Dateien';

  @override
  String get snippetsImportFormat => 'Format';

  @override
  String snippetsImportFormatName(String format) {
    String _temp0 = intl.Intl.selectLogic(format, {
      'bundle': 'CopyPaste-Paket',
      'json': 'JSON-Liste',
      'csv': 'CSV',
      'other': 'CopyPaste-Paket',
    });
    return '$_temp0';
  }

  @override
  String snippetsImportInvalidRow(int row, String reason) {
    return 'Zeile $row: $reason';
  }

  @override
  String get snippetsImportInvalidRows => 'Zeilen, die nicht importiert werden';

  @override
  String get snippetsImportPassphraseHint =>
      'Passphrase, falls die Datei verschlüsselt ist';

  @override
  String get snippetsImportPreview => 'Vorschau';

  @override
  String snippetsImportReport(
    int inserted,
    int updated,
    int skipped,
    int flagged,
    int unrecognized,
  ) {
    return 'Hinzugefügt: $inserted. Aktualisiert: $updated. Übersprungen: $skipped. Zu prüfende Kürzel: $flagged. Unbekannte Platzhalter: $unrecognized.';
  }

  @override
  String get snippetsImportSameIdPolicy =>
      'Gleicher Textbaustein, anderer Inhalt';

  @override
  String snippetsImportSameIdPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'keepBoth': 'Beide behalten',
      'replace': 'Vorhandenen ersetzen',
      'skip': 'Überspringen',
      'other': 'Beide behalten',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportTitle => 'Textbausteine importieren';

  @override
  String get snippetsInsert => 'Einfügen';

  @override
  String get snippetsInsertPlaceholder => 'Platzhalter einfügen';

  @override
  String get snippetsKeepReferences => 'Beibehalten';

  @override
  String get snippetsLibraryDescription =>
      'Textbausteine und Sammlungen durchsuchen, ordnen und bearbeiten.';

  @override
  String get snippetsLibraryKeywords =>
      'textbausteine snippets vorlagen kürzel sammlungen text';

  @override
  String get snippetsLibraryTitle => 'Textbaustein-Bibliothek';

  @override
  String snippetsLint(String code, String detail) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'unclosed': 'Dieser Platzhalter wird nie geschlossen.',
      'malformed': 'Dieser Platzhalter kann nicht gelesen werden.',
      'unknownPlaceholder':
          'Unbekannter Platzhalter „$detail“. Er wird unverändert eingefügt.',
      'unknownParameter': 'Unbekannte Option „$detail“.',
      'duplicateParameter': 'Option „$detail“ ist doppelt gesetzt.',
      'missingParameter': 'Option „$detail“ fehlt.',
      'invalidValue': 'Option „$detail“ hat einen ungültigen Wert.',
      'conflictingParameters':
          'Diese Optionen können nicht zusammen verwendet werden.',
      'unknownLocale': 'Unbekannte Sprache.',
      'unknownTimeZone': 'Unbekannte Zeitzone.',
      'invalidPattern': 'Ungültiges Datumsmuster.',
      'dateOutOfRange': 'Das Datum liegt außerhalb des Bereichs.',
      'unknownModifier': 'Unbekannter Modifikator „$detail“.',
      'modifierFailed': 'Modifikator „$detail“ konnte nicht angewendet werden.',
      'emptyClipboard': 'Zwischenablage-Eintrag $detail ist leer.',
      'extraCursor': 'Nur der erste Cursor-Platzhalter wird verwendet.',
      'nestedCursor':
          'Ein Cursor in einem eingefügten Textbaustein wird ignoriert.',
      'tooManyFields':
          'Feld „$detail“ überschreitet die Grenze von 10 Feldern.',
      'conflictingField':
          'Feld „$detail“ ist zweimal mit unterschiedlichen Einstellungen definiert.',
      'choiceDefaultNotOption':
          'Der Standardwert von „$detail“ ist keine seiner Optionen.',
      'cycle':
          'Textbausteine fügen sich gegenseitig in einer Schleife ein: $detail.',
      'depthExceeded': 'Textbausteine sind mehr als 3 Ebenen tief eingefügt.',
      'nestingLimit': 'Zu viele eingefügte Textbausteine.',
      'unknownSnippet': 'Kein Textbaustein hat dieses Kürzel.',
      'containsPlaceholders':
          'Der Text enthält Platzhalter. Schalte „Vorlage“ ein, um sie auszufüllen.',
      'other': 'Mit diesem Platzhalter stimmt etwas nicht.',
    });
    return '$_temp0';
  }

  @override
  String get snippetsLoadFailed => 'Textbausteine konnten nicht geladen werden';

  @override
  String get snippetsLoadMore => 'Mehr laden';

  @override
  String get snippetsMerge => 'Zusammenführen';

  @override
  String get snippetsMergeCollection => 'Mit anderer Sammlung zusammenführen';

  @override
  String snippetsMergeCollectionTitle(String name) {
    return '„$name“ zusammenführen mit';
  }

  @override
  String get snippetsMoveTo => 'Verschieben nach';

  @override
  String get snippetsNewCollection => 'Neue Sammlung';

  @override
  String get snippetsNewSnippet => 'Neuer Textbaustein';

  @override
  String get snippetsNewText => 'Neuer Text';

  @override
  String snippetsNoMatch(String query) {
    return 'Keine Textbausteine für „$query“';
  }

  @override
  String get snippetsPassphraseConfirm => 'Passphrase wiederholen';

  @override
  String snippetsPassphraseHint(int min) {
    return 'Passphrase, mindestens $min Zeichen';
  }

  @override
  String get snippetsPassphraseLabel => 'Passphrase';

  @override
  String get snippetsPassphraseNoRecovery =>
      'Eine vergessene Passphrase kann nicht wiederhergestellt werden.';

  @override
  String snippetsPassphraseStrength(String score) {
    String _temp0 = intl.Intl.selectLogic(score, {
      'tooShort': 'Zu kurz',
      'weak': 'Schwach',
      'fair': 'Mittel',
      'good': 'Gut',
      'other': 'Stark',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPinnedSwitch => 'Angeheftet';

  @override
  String get snippetsPlaceholderGroup => 'Platzhalter';

  @override
  String snippetsPlaceholderKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'today': 'Datum',
      'dateShort': 'Datum, kurz',
      'dateLong': 'Datum, lang',
      'dateIso': 'Datum, ISO 8601',
      'dateNextWeek': 'Datum in einer Woche',
      'now': 'Uhrzeit',
      'dateTime': 'Datum und Uhrzeit',
      'clipboard': 'Zwischenablage',
      'clipboardPrevious': 'Vorheriger Zwischenablage-Eintrag',
      'field': 'Ausfüllfeld …',
      'cursor': 'Cursorposition',
      'uuid': 'UUID',
      'random': 'Zufallszahl',
      'upper': 'Modifikator: Großbuchstaben',
      'lower': 'Modifikator: Kleinbuchstaben',
      'trim': 'Modifikator: Leerzeichen entfernen',
      'other': 'Platzhalter',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPlaceholderNestedGroup => 'Anderen Textbaustein einfügen';

  @override
  String get snippetsPlaceholderNoMatch => 'Keine passenden Platzhalter';

  @override
  String get snippetsPreviewEmpty => 'Keine Vorschau';

  @override
  String get snippetsPreviewTab => 'Vorschau';

  @override
  String get snippetsPurge => 'Endgültig löschen';

  @override
  String get snippetsPurgeMessage =>
      'Sie werden auf allen deinen Geräten gelöscht. Das kann nicht rückgängig gemacht werden.';

  @override
  String snippetsPurgeTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Textbausteine endgültig löschen?',
      one: 'Textbaustein endgültig löschen?',
    );
    return '$_temp0';
  }

  @override
  String get snippetsRecentlyDeletedEmpty => '„Zuletzt gelöscht“ ist leer';

  @override
  String get snippetsRecordPastedDescription =>
      'Eingefügte Textbausteine zum Verlauf hinzufügen.';

  @override
  String get snippetsRecordPastedKeywords =>
      'verlauf speichern eingefügt textbausteine';

  @override
  String get snippetsRecordPastedTitle =>
      'Eingefügte Textbausteine im Verlauf speichern';

  @override
  String get snippetsReplaceOriginal => 'Original ersetzen';

  @override
  String get snippetsReplaceOriginalHint =>
      'Löscht den ursprünglichen Clip nach dem Speichern.';

  @override
  String get snippetsRequireAuthDescription =>
      'Vorher nach Passwort oder Biometrie fragen.';

  @override
  String get snippetsRequireAuthKeywords =>
      'verborgen passwort biometrie fingerabdruck gesicht sperre bestätigen';

  @override
  String get snippetsRequireAuthTitle =>
      'Vor verborgenen Textbausteinen bestätigen';

  @override
  String get snippetsRestore => 'Wiederherstellen';

  @override
  String get snippetsSaveBlockTooLarge => 'Der Text ist größer als 64 KB.';

  @override
  String get snippetsSaveFailed => 'Speichern fehlgeschlagen';

  @override
  String get snippetsScopeAll => 'Alle';

  @override
  String get snippetsScopeEmpty => 'Hier sind keine Textbausteine';

  @override
  String get snippetsScopePinned => 'Angeheftet';

  @override
  String get snippetsScopeRecentlyDeleted => 'Zuletzt gelöscht';

  @override
  String get snippetsSearchPlaceholder => 'Textbausteine durchsuchen';

  @override
  String snippetsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ausgewählt',
      one: '1 ausgewählt',
    );
    return '$_temp0';
  }

  @override
  String get snippetsShow => 'Anzeigen';

  @override
  String get snippetsShowAll => 'Alle anzeigen';

  @override
  String get snippetsShowAttention => 'Prüfen';

  @override
  String snippetsSoftLimit(int count) {
    return 'Du hast $count Textbausteine. Erlaubt sind bis zu 20.000.';
  }

  @override
  String get snippetsTemplateBadge => 'Vorlage';

  @override
  String get snippetsTemplateSwitch => 'Vorlage';

  @override
  String get snippetsTitleLabel => 'Titel';

  @override
  String get snippetsTitlePlaceholder => 'Optional';

  @override
  String get snippetsTransferDescription =>
      'Textbausteine sichern oder zwischen Apps übertragen.';

  @override
  String get snippetsTransferKeywords =>
      'import export sicherung paket json csv passphrase';

  @override
  String get snippetsTransferTitle => 'Import und Export';

  @override
  String get snippetsUndo => 'Rückgängig';

  @override
  String get snippetsUnfiled => 'Ohne Sammlung';

  @override
  String snippetsUnfilledWarning(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Platzhalter werden unverändert eingefügt',
      one: '1 Platzhalter wird unverändert eingefügt',
    );
    return '$_temp0';
  }

  @override
  String get snippetsUnpin => 'Lösen';

  @override
  String get snippetsUnsavedMessage =>
      'Änderungen vor dem Schließen speichern?';

  @override
  String get snippetsUnsavedTitle => 'Nicht gespeicherte Änderungen';

  @override
  String get snippetsUntitled => 'Textbaustein ohne Titel';

  @override
  String get snippetsUpdateReferences => 'Aktualisieren';

  @override
  String snippetsUpdateReferencesMessage(String from, String to) {
    return 'Andere Vorlagen fügen eventuell „$from“ ein. Sollen sie „$to“ verwenden?';
  }

  @override
  String get snippetsUpdateReferencesTitle => 'Verweise aktualisieren?';

  @override
  String get snippetsUseFailed => 'Textbaustein konnte nicht verwendet werden';

  @override
  String get quickPasteTabQueue => 'Warteschlange';

  @override
  String get quickPasteMaskedMode =>
      'Passwortfeld: Vorschauen sind ausgeblendet';

  @override
  String quickPasteMaskedItem(String kind) {
    return 'Verborgen: $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'Geheime Einträge entsperren';

  @override
  String quickPastePastingWith(String name) {
    return 'Einfügen mit: $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut fügt das Original ein';
  }

  @override
  String get quickPasteDefaultSkipped => 'Standardumwandlung übersprungen';

  @override
  String get quickPasteDefaultSkippedMessage =>
      'Stattdessen wurde das Original verwendet.';

  @override
  String get quickPastePasteManually =>
      'Der Inhalt ist in der Zwischenablage. Fügen Sie ihn manuell ein.';

  @override
  String get quickPastePaste => 'Einfügen';

  @override
  String get quickPasteSnippetsUnavailable =>
      'Textbausteine sind nicht verfügbar';

  @override
  String get rulesLoadFailed => 'Inhaltsregeln konnten nicht geladen werden.';

  @override
  String get rulesNewRule => 'Neue Regel';

  @override
  String get rulesEditRule => 'Regel bearbeiten';

  @override
  String get rulesUntitled => 'Unbenannte Regel';

  @override
  String get rulesPrivacyDefaultsTitle => 'Datenschutz-Standards';

  @override
  String get rulesPrivacyDefaultsDescription =>
      'Datenschutzeinstellungen gelten vor allen Regeln.';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'datenschutz verborgen temporär ausgeschlossene apps integriert zugangsdaten passwörter zahlungskarten';

  @override
  String get rulesOpenPrivacy => 'Datenschutz öffnen';

  @override
  String get rulesMyRulesTitle => 'Meine Regeln';

  @override
  String get rulesMyRulesDescription =>
      'Laufen der Reihe nach auf neuen Kopien.';

  @override
  String get rulesMyRulesKeywords =>
      'inhaltsregeln automatisierung tag überspringen geheim ablaufen löschen muster regex';

  @override
  String get rulesEmptyTitle => 'Noch keine Regeln';

  @override
  String get rulesEmptyMessage =>
      'Regeln taggen, schützen, befristen oder überspringen neue Kopien.';

  @override
  String get rulesDeletedTitle => 'Kürzlich gelöscht';

  @override
  String get rulesDeletedDescription =>
      'Innerhalb von 30 Tagen wiederherstellbar.';

  @override
  String rulesDeletedAt(String time) {
    return 'Gelöscht: $time';
  }

  @override
  String get rulesRestore => 'Wiederherstellen';

  @override
  String get rulesDeletedToast => 'Regel gelöscht';

  @override
  String get rulesDeletedToastMessage =>
      'Du kannst sie 30 Tage lang wiederherstellen.';

  @override
  String get rulesUndo => 'Rückgängig';

  @override
  String rulesConflictMessage(String rule, String device) {
    return 'Eine gleichzeitige Änderung von „$rule“ auf $device wurde durch eine neuere ersetzt.';
  }

  @override
  String get rulesAnotherDevice => 'einem anderen Gerät';

  @override
  String get rulesRestoreConflict => 'Diese Version verwenden';

  @override
  String get rulesSkipLogTitle => 'Kürzlich übersprungen';

  @override
  String get rulesSkipLogDescription =>
      'Kopien, die deine Regeln auf diesem Gerät nicht gespeichert haben.';

  @override
  String get rulesSkipLogKeywords =>
      'übersprungen ignoriert nicht gespeichert protokoll';

  @override
  String get rulesSkipLogShow => 'Anzeigen';

  @override
  String get rulesSkipLogEmpty => 'Es wurden keine Kopien übersprungen.';

  @override
  String get rulesUnknownApp => 'Unbekannte App';

  @override
  String get rulesMatchesEverything => 'Jede Kopie';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary, dann stoppen';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': ' oder ',
      'other': ' und ',
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
    return 'Läuft auf $devices';
  }

  @override
  String rulesNewFromDevice(String device) {
    return 'Neu von $device';
  }

  @override
  String get rulesNeedsUpdateBadge => 'Update nötig';

  @override
  String get rulesUpdateRequired =>
      'Aktualisiere CopyPaste, um diese Regel auszuführen.';

  @override
  String get rulesErrorBadge => 'Fehler';

  @override
  String rulesErrorHint(String problem) {
    return '$problem Bis zur Behebung werden Treffer geschützt.';
  }

  @override
  String get rulesReorder => 'Zum Umsortieren ziehen';

  @override
  String get rulesMoreActions => 'Weitere Aktionen';

  @override
  String get rulesEdit => 'Bearbeiten';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': 'Nach oben',
      'down': 'Nach unten',
      'top': 'Ganz nach oben',
      'other': 'Ganz nach unten',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => 'Auf vorhandene Elemente anwenden…';

  @override
  String get rulesResetBuiltin => 'Auf Standard zurücksetzen';

  @override
  String get rulesNameLabel => 'Name';

  @override
  String get rulesNamePlaceholder => 'Zum Beispiel Arbeitslinks';

  @override
  String get rulesWhenTitle => 'Wenn';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'Eine Bedingung trifft zu',
      'other': 'Alle Bedingungen treffen zu',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions =>
      'Keine Bedingungen: Diese Regel gilt für jede Kopie.';

  @override
  String get rulesAddCondition => 'Bedingung hinzufügen';

  @override
  String get rulesRemoveCondition => 'Bedingung entfernen';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'Text',
      'content_type': 'Inhaltstyp',
      'detected_kind': 'Erkannte Art',
      'source_app': 'Quell-App',
      'size': 'Größe',
      'privacy_marker': 'Datenschutzmarkierung',
      'capture_origin': 'Herkunft',
      'other': 'Unbekannte Bedingung',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'enthält',
      'equals': 'ist',
      'starts_with': 'beginnt mit',
      'ends_with': 'endet mit',
      'matches_regex': 'passt zum Muster',
      'other': 'passt',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'enthält nicht',
      'equals': 'ist nicht',
      'starts_with': 'beginnt nicht mit',
      'ends_with': 'endet nicht mit',
      'matches_regex': 'passt nicht zum Muster',
      'other': 'passt nicht',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'ist nicht',
      'other': 'ist',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'ist kleiner als',
      'other': 'ist größer als',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'ist nicht kleiner als',
      'other': 'ist nicht größer als',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'Zeilen',
      'bytes': 'Bytes',
      'other': 'Zeichen',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'Zeilen',
      'bytes': 'Bytes',
      'other': 'Zeichen',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'Reiner Text',
      'rich_text': 'Formatierter Text',
      'image': 'Bild',
      'files': 'Dateien',
      'color': 'Farbe',
      'link': 'Link',
      'other': 'Andere',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'Passwort oder Geheimnis',
      'url': 'Webadresse',
      'email': 'E-Mail-Adresse',
      'phone': 'Telefonnummer',
      'code': 'Code',
      'color': 'Farbwert',
      'json': 'JSON',
      'file_path': 'Dateipfad',
      'number': 'Zahl',
      'one_time_code': 'Einmalcode',
      'payment_card': 'Kartennummer',
      'other': 'Andere',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': 'Verborgen',
      'transient': 'Temporär',
      'auto_generated': 'Automatisch erzeugt',
      'excluded_from_history': 'Vom Verlauf ausgeschlossen',
      'other': 'Andere',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'Auf diesem Gerät kopiert',
      'received': 'Von einem anderen Gerät empfangen',
      'created_in_app': 'In CopyPaste erstellt',
      'other': 'Andere',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => 'Gesuchter Text';

  @override
  String get rulesPatternPlaceholder => 'Muster, zum Beispiel ^ticket-[0-9]+';

  @override
  String get rulesPatternHint =>
      'Lookaround und Rückverweise werden nicht unterstützt.';

  @override
  String get rulesCaseSensitive => 'Groß-/Kleinschreibung beachten';

  @override
  String get rulesChooseValues => 'Auswählen';

  @override
  String get rulesUnknownConditionNotice =>
      'Diese Bedingung stammt aus einer neueren Version von CopyPaste.';

  @override
  String get rulesSourceAppNotice =>
      'Unter Android trifft das nur auf Kopien über die Einträge-Tastatur zu.';

  @override
  String get rulesPrivacyMarkerNotice =>
      'Nicht jedes System meldet jede Markierung.';

  @override
  String get rulesThenTitle => 'Dann';

  @override
  String get rulesNoActions => 'Füge mindestens eine Aktion hinzu.';

  @override
  String get rulesAddAction => 'Aktion hinzufügen';

  @override
  String get rulesRemoveAction => 'Aktion entfernen';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': 'Nicht speichern',
      'mark_secret': 'Als geheim markieren',
      'delete_after': 'Löschen nach',
      'add_tags': 'Tags hinzufügen',
      'transform': 'Transformieren',
      'other': 'Unbekannte Aktion',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone':
          '„Nicht speichern“ lässt sich nicht mit anderen Aktionen kombinieren.',
      'present': 'Diese Regel hat diese Aktion bereits.',
      'other': 'Eine Regel kann bis zu 8 Aktionen haben.',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'Diese Aktion stammt aus einer neueren Version von CopyPaste.';

  @override
  String get rulesDontSaveWarning =>
      'Passende Kopien werden nie gespeichert, indiziert oder synchronisiert.';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Minuten',
      one: '1 Minute',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Stunden',
      one: '1 Stunde',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Tage',
      one: '1 Tag',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry =>
      'Auch die Zwischenablage leeren, wenn sie diese Kopie noch enthält.';

  @override
  String get rulesNoTags =>
      'Erstelle Tags im Verlauf, um sie hier zu verwenden.';

  @override
  String get rulesNoTransforms => 'Keine Transformationen verfügbar.';

  @override
  String get rulesReplaceClipboard =>
      'Auch die Zwischenablage durch das Ergebnis ersetzen';

  @override
  String get rulesOptionsTitle => 'Optionen';

  @override
  String get rulesEnabledSwitch => 'Aktiviert';

  @override
  String get rulesStopProcessing =>
      'Spätere Regeln nicht mehr ausführen, wenn diese zutrifft';

  @override
  String get rulesRunsOnLabel => 'Läuft auf';

  @override
  String get rulesAllDevices => 'Alle Geräte';

  @override
  String get rulesChosenDevices => 'Ausgewählte Geräte';

  @override
  String get rulesTestTitle => 'Testen';

  @override
  String get rulesTestDescription => 'Probiere die Regel mit Beispieltext aus.';

  @override
  String get rulesSamplePlaceholder => 'Beispieltext einfügen oder eingeben';

  @override
  String get rulesTestFullSet => 'Zusammen mit allen Regeln testen';

  @override
  String get rulesRunTest => 'Test ausführen';

  @override
  String get rulesTestFailed => 'Der Test konnte nicht ausgeführt werden.';

  @override
  String get rulesPreviewHistory => 'Mit Verlauf vorschauen';

  @override
  String get rulesSaveFailed => 'Die Regel konnte nicht gespeichert werden.';

  @override
  String get rulesReadOnlyNotice =>
      'Von einer neueren CopyPaste-Version erstellt. Aktualisiere, um sie zu bearbeiten oder auszuführen.';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'Die Regel konnte nicht gelesen werden.',
      'unsupported_schema':
          'Diese Regel erfordert eine neuere Version von CopyPaste.',
      'unknown_condition_type':
          'Diese Bedingung erfordert eine neuere Version von CopyPaste.',
      'unknown_action_type':
          'Diese Aktion erfordert eine neuere Version von CopyPaste.',
      'too_many_conditions': 'Eine Regel kann bis zu 20 Bedingungen haben.',
      'too_many_actions': 'Eine Regel kann bis zu 8 Aktionen haben.',
      'too_many_rules': 'Nur die ersten 200 Regeln werden ausgeführt.',
      'no_actions': 'Füge mindestens eine Aktion hinzu.',
      'conflicting_actions':
          '„Nicht speichern“ lässt sich nicht mit anderen Aktionen kombinieren.',
      'duplicate_action': 'Jede Aktion kann nur einmal verwendet werden.',
      'empty_value': 'Gib einen Wert ein.',
      'empty_list': 'Wähle mindestens einen Wert.',
      'too_many_values': 'Wähle bis zu 64 Werte.',
      'invalid_value': 'Dieser Wert ist ungültig.',
      'pattern_too_long': 'Muster können bis zu 2.000 Zeichen lang sein.',
      'unsupported_look_around':
          'Lookahead und Lookbehind werden nicht unterstützt. Verwende stattdessen zwei Bedingungen mit „Alle Bedingungen treffen zu“.',
      'unsupported_backreference': 'Rückverweise werden nicht unterstützt.',
      'pattern_too_large':
          'Dieses Muster ist zu komplex. Vereinfache seine Wiederholungen.',
      'invalid_pattern': 'Dieses Muster ist ungültig.',
      'invalid_duration': 'Wähle eine Dauer zwischen 1 Minute und 90 Tagen.',
      'invalid_tags': 'Wähle mindestens einen Tag.',
      'invalid_transform': 'Wähle eine Transformation.',
      'other': 'Diese Regel hat ein Problem.',
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
      'matched': 'zutreffend',
      'not_matched': 'nicht zutreffend',
      'unknown': 'unbekannt',
      'skipped': 'nicht ausgeführt',
      'other': 'Fehler',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return 'Bedingung $number';
  }

  @override
  String get rulesTraceNoRules => 'Es wurde keine Regel ausgewertet.';

  @override
  String get rulesTraceStopped => 'Spätere Regeln wurden nicht ausgeführt.';

  @override
  String get rulesTraceUnknownReason =>
      'Für diese Kopie unbekannt, daher kein Treffer.';

  @override
  String get rulesTraceRuleError =>
      'Diese Regel konnte nicht laufen, daher werden Treffer geschützt.';

  @override
  String get rulesTraceOutcomeTitle => 'Ergebnis';

  @override
  String get rulesTraceNoEffect => 'Keine Regel ändert diese Kopie.';

  @override
  String get rulesTraceSkipped => 'Nicht gespeichert';

  @override
  String get rulesTraceSecret => 'Als geheim gespeichert';

  @override
  String rulesTraceExpires(String duration) {
    return 'Gelöscht nach $duration';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'Getaggt: $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return 'Transformiert mit $names';
  }

  @override
  String get rulesTraceOutput => 'Transformierter Text';

  @override
  String get rulesPreviewTitle => 'Auf vorhandene Elemente anwenden';

  @override
  String rulesPreviewRunning(int count) {
    return 'Die letzten $count Clips werden geprüft…';
  }

  @override
  String get rulesPreviewNone => 'Kein aktueller Clip passt zu dieser Regel.';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Clips passen',
      one: '1 Clip passt',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count würden gelöscht',
      one: '1 würde gelöscht',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count würden als geheim markiert',
      one: '1 würde als geheim markiert',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count würden eine Löschzeit erhalten',
      one: '1 würde eine Löschzeit erhalten',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count würden getaggt',
      one: '1 würde getaggt',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return 'Nur die letzten $count Clips wurden geprüft.';
  }

  @override
  String rulesPreviewMore(int count) {
    return 'und $count weitere';
  }

  @override
  String get rulesPreviewNote => 'Bis zum Anwenden ändert sich nichts.';

  @override
  String get rulesSecretItem => 'Geheimes Element';

  @override
  String rulesSecretItemFrom(String app) {
    return 'Geheimes Element aus $app';
  }

  @override
  String get rulesPreviewCancelled =>
      'Die Vorschau wurde gestoppt. Es wurde nichts geändert.';

  @override
  String get rulesPreviewFailed =>
      'Die Vorschau konnte nicht ausgeführt werden.';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Clips löschen?',
      one: '1 Clip löschen?',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage =>
      'Passende Clips werden auf allen deinen Geräten gelöscht. Das lässt sich nicht rückgängig machen.';

  @override
  String get rulesApplyDeleteConfirm => 'Löschen und anwenden';

  @override
  String rulesApplyProgress(int done, int total) {
    return 'Wird angewendet: $done von $total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Clips geändert',
      one: '1 Clip geändert',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Clips gelöscht',
      one: '1 Clip gelöscht',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Clips konnten nicht geändert werden',
      one: '1 Clip konnte nicht geändert werden',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled => 'Gestoppt. Bisherige Änderungen bleiben.';

  @override
  String get historyActionEdit => 'Bearbeiten';

  @override
  String get historyActionSaveAsSnippet => 'Als Textbaustein speichern';

  @override
  String get historyActionAddToQueue => 'Zur Paste Queue hinzufügen';

  @override
  String get historyActionTransform => 'Umwandeln…';

  @override
  String get historyActionExplain => 'Warum ist das hier?';

  @override
  String get historyActionKeep => 'Behalten';

  @override
  String get historyNewClip => 'Neuer Eintrag';

  @override
  String get historySavedTitle => 'Gespeichert';

  @override
  String get historyEditSaved =>
      'Der bearbeitete Text ist ein neuer Eintrag oben im Verlauf.';

  @override
  String get historyNewClipSaved => 'Dein Text wurde gespeichert.';

  @override
  String get historySnippetSaved =>
      'Der Text des Eintrags ist jetzt ein Textbaustein.';

  @override
  String get historySnippetSaveFailed =>
      'Dieser Eintrag konnte nicht als Textbaustein gespeichert werden.';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Einträge wurden zur Warteschlange hinzugefügt.',
      one: '1 Eintrag wurde zur Warteschlange hinzugefügt.',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed =>
      'Hinzufügen zur Paste Queue fehlgeschlagen. Prüfe, ob sie eingeschaltet ist.';

  @override
  String get historyKeepFailed =>
      'Dieser Eintrag konnte nicht behalten werden.';

  @override
  String get historyExplainFailed =>
      'Die Erklärung konnte nicht geladen werden.';

  @override
  String get historyDeletesSoon => 'Wird in weniger als einer Minute gelöscht';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Wird in $count Min. gelöscht',
      one: 'Wird in 1 Min. gelöscht',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Wird in $count Std. gelöscht',
      one: 'Wird in 1 Std. gelöscht',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Wird in $count Tagen gelöscht',
      one: 'Wird in 1 Tag gelöscht',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => 'Manuell';

  @override
  String get historyProvenanceEdited => 'Bearbeitet';

  @override
  String get historyProvenanceTransformed => 'Umgewandelt';

  @override
  String historyRuleTag(String name) {
    return '$name (automatisch)';
  }

  @override
  String get historyMetadataExpiry => 'Löschung';

  @override
  String get historyMetadataRuleTags => 'Automatische Tags';

  @override
  String modulesEnableTitle(String module) {
    return '$module aktivieren?';
  }

  @override
  String get modulesEnableAccess => 'Das Modul kann dann:';

  @override
  String modulesCapability(String capability) {
    String _temp0 = intl.Intl.selectLogic(capability, {
      'history_read': 'Deinen Zwischenablageverlauf lesen',
      'clipboard_write': 'In deine Zwischenablage kopieren',
      'library_write': 'Textbausteine speichern und Einträge anheften',
      'snippets': 'Deine Textbausteine lesen',
      'synced_records': 'Seine Daten zwischen deinen Geräten synchronisieren',
      'local_state': 'Seine Daten auf diesem Gerät speichern',
      'prompts': 'Dich um Bestätigung bitten',
      'endpoints': 'Verbindungen lokaler Apps annehmen',
      'other': 'Einen CopyPaste-Dienst nutzen',
    });
    return '$_temp0';
  }

  @override
  String modulesPromptFrom(String module) {
    return 'Von $module';
  }

  @override
  String get modulesUnknownModule => 'Unbekanntes Modul';

  @override
  String get modulesPromptAnswerFailed => 'Deine Antwort wurde nicht gesendet.';

  @override
  String get modulesPromptNotificationBody =>
      'CopyPaste öffnen, um zu antworten.';

  @override
  String get modulesUnknownDevice => 'Unbekanntes Gerät';

  @override
  String get modulesChooseDevices => 'Geräte wählen';

  @override
  String get modulesAddApp => 'App hinzufügen';

  @override
  String modulesRemoveApp(String app) {
    return '$app entfernen';
  }

  @override
  String get modulesNoApps => 'Keine Apps gewählt';
}
