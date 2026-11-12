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
  String get settingsRuntimeQuickPastePrepareFailed =>
      'Quick Paste konnte nicht vorbereitet werden.';

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
  String get settingsSectionAiTools => 'KI-Tools';

  @override
  String get settingsSectionAiToolsSummary =>
      'KI-Assistenten verbinden und festlegen, was sie sehen dürfen.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Mehrere Kopien sammeln und der Reihe nach einfügen.';

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
      'Aktivierte Hintergrunddienste nach einem Neustart des Geräts fortsetzen.';

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
      'Vollständige oder eingeschränkte Erfassung wählen und die einmalige Einrichtung über Shizuku oder ADB verwalten.';

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
      'Zugriff auf alle Fotos und Benachrichtigungen erlauben, um neue Screenshots im Hintergrund zu speichern.';

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
      'Limit zum Hinzufügen und Herunterladen von Dateien. Dateiinhalte werden nur über „Herunterladen“ übertragen.';

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
      'Das ausgewählte verschlüsselte Backup ersetzt den Verlauf der Zwischenablage auf diesem Gerät. Aktuelle Einstellungen und die Geräteidentität bleiben erhalten.';

  @override
  String get settingsRestoreFailed =>
      'Das Backup konnte nicht wiederhergestellt werden.';

  @override
  String get settingsExportIncompleteTitle => 'Textexport ist unvollständig';

  @override
  String settingsExportIncompleteMessage(String saved, String omissions) {
    return '$saved; $omissions.';
  }

  @override
  String settingsExportOmissionsJoin(String first, String second) {
    return '$first und $second';
  }

  @override
  String get settingsSyncSwitchTitle => 'Sync';

  @override
  String get settingsSyncSwitchDescription =>
      'Hauptschalter für die Synchronisierung gekoppelter Geräte.';

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
      'Neuere Einträge von anderen Geräten automatisch in diese Zwischenablage kopieren.';

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
  String get settingsExcludedAppsMacDescription =>
      'Keine automatische Erfassung während der Aktivität dieser Apps. Kopien im Hintergrund können Ausschlüsse umgehen.';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Keine automatische Erfassung von erkannten Besitzern der Zwischenablage. Sind Ausschlüsse festgelegt und ist die Quelle unbekannt, wird die Erfassung pausiert.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Android überspringt die automatische Erfassung, solange Ausschlüsse festgelegt sind, da Quell-Apps nicht erkannt werden können.';

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
  String get settingsExcludeDialogTitle => 'App ausschließen';

  @override
  String get settingsExcludeDialogAction => 'Ausschließen';

  @override
  String get settingsExcludeDialogPlaceholder => 'App-Kennung';

  @override
  String get settingsExcludeFailed =>
      'Die App konnte nicht ausgeschlossen werden.';

  @override
  String get settingsSkipSecretTitle => 'Vertrauliches überspringen';

  @override
  String get settingsSkipSecretDescription =>
      'Als geheim markierte Inhalte vom Verlauf fernhalten.';

  @override
  String get settingsSkipSecretKeywords =>
      'geheim Passwort sensibel verborgen Spoiler secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Temporäres überspringen';

  @override
  String get settingsSkipTransientDescription =>
      'Als temporär markierte Kopien vom Verlauf fernhalten.';

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
      'Das Tastenkürzel funktioniert, während CopyPaste im Hintergrund läuft.';

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
      other: '$count Texteinträge wurden gespeichert',
      one: '$count Texteintrag wurde gespeichert',
    );
    return '$_temp0';
  }

  @override
  String settingsExportSkippedNonText(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Nicht-Text-Einträge wurden ausgelassen',
      one: '$count Nicht-Text-Eintrag wurde ausgelassen',
    );
    return '$_temp0';
  }

  @override
  String settingsExportSkippedUnreadable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Texteinträge konnten nicht gelesen werden',
      one: '$count Texteintrag konnte nicht gelesen werden',
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
  String get appCollapseNavigation => 'Navigation einklappen';

  @override
  String get appExpandNavigation => 'Navigation ausklappen';

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
  String sharedSizeKilobytes(String size) {
    return '$size KB';
  }

  @override
  String sharedSizeMegabytes(String size) {
    return '$size MB';
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
  String get modulesInstalling => 'Wird installiert …';

  @override
  String modulesDownloadingProgress(int percent) {
    return 'Download: $percent %';
  }

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
}
