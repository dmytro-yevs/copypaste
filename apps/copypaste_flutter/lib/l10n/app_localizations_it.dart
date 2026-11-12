// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Italian (`it`).
class AppLocalizationsIt extends AppLocalizations {
  AppLocalizationsIt([String locale = 'it']) : super(locale);

  @override
  String get commonCancel => 'Annulla';

  @override
  String get commonClose => 'Chiudi';

  @override
  String get commonDone => 'Fine';

  @override
  String get commonRetry => 'Riprova';

  @override
  String get commonTryAgain => 'Riprova';

  @override
  String get commonCopy => 'Copia';

  @override
  String get commonDelete => 'Elimina';

  @override
  String get commonSave => 'Salva';

  @override
  String get commonContinue => 'Continua';

  @override
  String get commonBack => 'Indietro';

  @override
  String get commonOpenSettings => 'Apri Impostazioni';

  @override
  String get commonLoading => 'Caricamento';

  @override
  String get commonApply => 'Applica';

  @override
  String get commonClear => 'Cancella';

  @override
  String get commonPin => 'Fissa';

  @override
  String get commonEnable => 'Attiva';

  @override
  String get settingsLanguageTitle => 'Lingua';

  @override
  String get settingsLanguageDescription =>
      'Scegli la lingua dell\'interfaccia di CopyPaste.';

  @override
  String get settingsLanguageSystem => 'Predefinita di sistema';

  @override
  String get settingsLanguageKeywords =>
      'lingua impostazioni locali traduzione interfaccia language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Impossibile aggiornare l\'apertura al login. Riprova.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'Impossibile aprire le impostazioni di avvio del sistema.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Impossibile aggiornare le impostazioni. Riprova.';

  @override
  String get settingsRuntimeLoadFailed =>
      'Impossibile caricare le impostazioni.';

  @override
  String get settingsRuntimeAppAlreadyExcluded => 'Questa app è già esclusa.';

  @override
  String get settingsRuntimeChangeFailed =>
      'Impossibile completare la modifica richiesta.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Per riprendere l\'acquisizione, concedi tutte le autorizzazioni di acquisizione degli appunti.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'L\'acquisizione degli appunti è in pausa. Il servizio di acquisizione di Android non ha confermato l\'arresto.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'Per attivare le notifiche di acquisizione è necessaria l\'autorizzazione alle notifiche.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste non è riuscito a leggere le impostazioni salvate. Sono attivi valori di protezione; rivedi e salva le tue preferenze per ripristinarle.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste ha recuperato le impostazioni illeggibili con valori di protezione. Rivedi e salva le tue preferenze per ripristinarle.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Gli identificatori delle app non possono essere vuoti e non devono superare 256 byte.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'Puoi escludere fino a 256 app.';

  @override
  String get settingsRuntimeQuickPastePrepareFailed =>
      'Impossibile preparare Quick Paste.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Impossibile aggiornare l\'incolla automatico.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'Impossibile registrare questa scorciatoia.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'Impossibile sospendere la scorciatoia attuale.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'Impossibile ripristinare la scorciatoia.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'Impossibile aprire Quick Paste.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Impossibile richiedere l\'autorizzazione Accessibilità.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Impossibile aggiornare lo stato dell\'integrazione con Linux.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'L\'autorizzazione di input per gli appunti non è stata concessa.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Impossibile richiedere l\'autorizzazione di input per gli appunti.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Impossibile aprire la configurazione dell\'integrazione desktop.';

  @override
  String get settingsRuntimeTextHistoryFileType =>
      'Cronologia testi di CopyPaste';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'Backup crittografato di CopyPaste';

  @override
  String get settingsRuntimeNotificationImage => 'Immagine';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Immagine · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'File';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste non è riuscito a ripristinare l\'installazione dell\'aggiornamento.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Questa installazione Linux non può aggiornarsi automaticamente.';

  @override
  String get updateMacosPackageMissing =>
      'Questa versione non include un pacchetto di aggiornamento firmato per macOS.';

  @override
  String get updateInstalledVersionInvalid =>
      'La versione di CopyPaste installata non è valida.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste non è riuscito a raggiungere GitHub per verificare la disponibilità di aggiornamenti.';

  @override
  String get updateCheckTimedOut =>
      'Tempo scaduto durante la verifica degli aggiornamenti.';

  @override
  String get updateCheckDeviceFailed =>
      'CopyPaste non è riuscito a verificare gli aggiornamenti su questo dispositivo.';

  @override
  String get updateCheckFailed =>
      'CopyPaste non è riuscito a verificare gli aggiornamenti.';

  @override
  String get updateDownloadFailed =>
      'CopyPaste non è riuscito a scaricare l\'aggiornamento.';

  @override
  String get updateDownloadTimedOut =>
      'Tempo scaduto durante il download dell\'aggiornamento.';

  @override
  String get updateInstallFailed =>
      'CopyPaste non è riuscito a installare l\'aggiornamento.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Continua nel programma di installazione di Android.';

  @override
  String get updateContinueInPackageManager =>
      'Continua nel gestore di pacchetti del sistema.';

  @override
  String get updateInstallerStarting =>
      'Avvio del programma di installazione in corso.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Consenti a CopyPaste di installare app nelle impostazioni di Android, poi continua.';

  @override
  String get updatePermissionRequiredLinux =>
      'Esegui l\'autenticazione nel gestore di pacchetti del sistema per installare questo aggiornamento.';

  @override
  String get updatePermissionRequired =>
      'Consenti a CopyPaste di installare questo aggiornamento, poi continua.';

  @override
  String get updateRestartRequired =>
      'L\'aggiornamento è installato. Riavvia CopyPaste per usarlo.';

  @override
  String get updateRestarting => 'Riavvio di CopyPaste in corso.';

  @override
  String get updateRestartFailed =>
      'CopyPaste non è riuscito a riavviarsi. Riprova.';

  @override
  String get updateReleasePageOpenFailed =>
      'CopyPaste non è riuscito ad aprire la pagina della versione.';

  @override
  String get updatePackageInvalid =>
      'L\'aggiornamento scaricato non è un pacchetto CopyPaste valido.';

  @override
  String get updateSignatureInvalid =>
      'L\'aggiornamento scaricato ha una firma non valida.';

  @override
  String get updateDowngradeRefused =>
      'Il pacchetto scaricato non è più recente di questa versione.';

  @override
  String get updateHomebrewRequired =>
      'Installa CopyPaste con Homebrew per aggiornarlo da qui.';

  @override
  String get updateInstallationCancelled =>
      'L\'installazione dell\'aggiornamento è stata annullata.';

  @override
  String get updateInstallationBlocked =>
      'Android ha bloccato l\'installazione dell\'aggiornamento.';

  @override
  String get updateInstallationConflict =>
      'L\'aggiornamento è in conflitto con l\'app installata.';

  @override
  String get updateInstallationIncompatible =>
      'L\'aggiornamento non è compatibile con questo dispositivo.';

  @override
  String get updateInstallationStorage =>
      'Libera spazio di archiviazione per installare l\'aggiornamento.';

  @override
  String get updateInstallationInterrupted =>
      'L\'installazione dell\'aggiornamento è stata interrotta. Riprova.';

  @override
  String get updateInstallationBusy =>
      'È già in corso l\'installazione di un aggiornamento.';

  @override
  String get updatePackageDetailsInvalid =>
      'I dettagli del pacchetto di aggiornamento non sono validi.';

  @override
  String get updateIntegrityCheckFailed =>
      'L\'aggiornamento scaricato non ha superato il controllo di integrità.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste non è riuscito ad avviare il programma di installazione del sistema.';

  @override
  String get updateInstallLocationTranslocated =>
      'Sposta CopyPaste nella cartella Applicazioni e aprilo da lì per aggiornarlo.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste non può sostituirsi nella cartella attuale. Usa la pagina della versione per aggiornarlo.';

  @override
  String get updateReplacementBlocked =>
      'macOS ha bloccato la sostituzione di CopyPaste. Consentila nelle impostazioni Gestione app o usa la pagina della versione.';

  @override
  String get updatePackageNotDownloadable =>
      'Questa piattaforma non usa un pacchetto di aggiornamento scaricabile.';

  @override
  String get updatePackageSizeInvalid =>
      'La dimensione del pacchetto di aggiornamento non è valida.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'L\'aggiornamento scaricato ha una firma del programma di aggiornamento non valida.';

  @override
  String get updatePackageUrlUntrusted =>
      'L\'URL del pacchetto di aggiornamento non è attendibile.';

  @override
  String get updateDownloadLeftGitHub =>
      'Il download dell\'aggiornamento è stato reindirizzato fuori da GitHub.';

  @override
  String get updateDownloadTooManyRedirects =>
      'Il download dell\'aggiornamento ha usato troppi reindirizzamenti.';

  @override
  String get updatePackageTooLarge =>
      'Il pacchetto di aggiornamento è troppo grande.';

  @override
  String get updateSignatureSizeInvalid =>
      'La dimensione della firma del programma di aggiornamento non è valida.';

  @override
  String get updateMetadataInvalid =>
      'GitHub ha restituito metadati di aggiornamento non validi.';

  @override
  String get updateMetadataTooLarge =>
      'I metadati dell\'aggiornamento sono troppo grandi.';

  @override
  String get quickPasteActionFailedTitle => 'Azione non riuscita';

  @override
  String get quickPasteOpenAppSemantics => 'Apri CopyPaste';

  @override
  String get quickPasteSearchPlaceholder => 'Digita per cercare…';

  @override
  String get quickPasteLoadingHistory =>
      'Caricamento della cronologia degli appunti.';

  @override
  String get quickPasteHistoryUnavailable => 'Cronologia non disponibile';

  @override
  String get quickPasteNoClipsFound => 'Nessun elemento trovato';

  @override
  String get quickPasteLoadMore => 'Carica altri';

  @override
  String get quickPasteTransferFailed => 'Non riuscito';

  @override
  String get quickPasteClipUnavailable => 'Elemento non disponibile';

  @override
  String get quickPasteFileFallbackName => 'File';

  @override
  String get quickPastePathUnavailable => 'Percorso non disponibile';

  @override
  String get quickPastePreferences => 'Preferenze…';

  @override
  String get quickPasteAbout => 'Informazioni';

  @override
  String get quickPasteQuit => 'Esci';

  @override
  String get quickPasteClearHistoryTitle => 'Cancellare la cronologia?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Gli elementi fissati resteranno nella cronologia.';

  @override
  String get quickPasteClearHistoryFailed =>
      'Impossibile eliminare la cronologia.';

  @override
  String get quickPasteAboutDescription =>
      'Cronologia degli appunti privata su macOS, Android, Windows e Linux.';

  @override
  String get transfersTitle => 'Download e trasferimenti';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count trasferimenti attivi',
      many: '$count trasferimenti attivi',
      one: '$count trasferimento attivo',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count attivi',
      many: '$count attivi',
      one: '$count attivo',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Più recenti';

  @override
  String get transfersShowEarlier => 'Precedenti';

  @override
  String get transfersClearCompletedHistory =>
      'Cancella i trasferimenti completati';

  @override
  String get transfersErrorTitle => 'Trasferimenti';

  @override
  String get transfersEmptyTitle => 'Nessun trasferimento';

  @override
  String get transfersEmptyMessage =>
      'Le importazioni di file, i download e i trasferimenti in uscita vengono visualizzati qui.';

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
  String get transfersRemoveFromHistory => 'Rimuovi dalla cronologia';

  @override
  String get transfersCancelDownload => 'Annulla download';

  @override
  String get transfersShowFile => 'Mostra file';

  @override
  String get transfersPhaseSending => 'Invio in corso';

  @override
  String get transfersPhaseDownloading => 'Download in corso';

  @override
  String get transfersPhaseAdding => 'Aggiunta in corso';

  @override
  String get transfersPhaseAdded => 'Aggiunto';

  @override
  String get transfersPhaseSent => 'Inviato';

  @override
  String get transfersPhaseCompleted => 'Completato';

  @override
  String get transfersPhaseCancelled => 'Annullato';

  @override
  String get transfersPhaseInterrupted => 'Interrotto';

  @override
  String get transfersPhaseFailed => 'Non riuscito';

  @override
  String get transfersUpdatesUnavailable =>
      'Gli aggiornamenti dei trasferimenti non sono disponibili. Riapri per aggiornare.';

  @override
  String get transfersLoadFailed =>
      'Impossibile caricare i trasferimenti. Riprova.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Impossibile caricare gli aggiornamenti dei trasferimenti.';

  @override
  String get transfersRetryFailed =>
      'Impossibile riprovare il download di questo file.';

  @override
  String get transfersClearHistoryFailed =>
      'Impossibile cancellare la cronologia dei trasferimenti.';

  @override
  String get transfersRemoveRecordFailed =>
      'Impossibile rimuovere questa voce della cronologia.';

  @override
  String get transfersCancelFailed =>
      'Impossibile annullare il trasferimento. Potrebbe essere ancora in corso.';

  @override
  String get transfersSavedFileUnavailable =>
      'Il file salvato non è disponibile. Potrebbe essere stato spostato o eliminato.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Notifiche · $count non lette',
      many: 'Notifiche · $count non lette',
      one: 'Notifiche · $count non letta',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Notifiche';

  @override
  String get notificationsClearRead => 'Cancella notifiche lette';

  @override
  String get notificationsEmptyTitle => 'Nessuna notifica';

  @override
  String get notificationsEmptyMessage =>
      'Le richieste di connessione dei dispositivi e il loro esito vengono visualizzati qui.';

  @override
  String get notificationsSystemAlertTitle => 'Notifiche di sistema';

  @override
  String get notificationsReview => 'Controlla';

  @override
  String get notificationsConnectionRequest => 'Richiesta di connessione';

  @override
  String get notificationsConfirmConnection => 'Conferma connessione';

  @override
  String get notificationsDeviceConnected => 'Dispositivo connesso';

  @override
  String get notificationsConnectionRejected => 'Connessione rifiutata';

  @override
  String get notificationsConnectionCancelled => 'Connessione annullata';

  @override
  String get notificationsConnectionRequestExpired =>
      'Richiesta di connessione scaduta';

  @override
  String get notificationsConnectionFailed => 'Connessione non riuscita';

  @override
  String get notificationsNearbyDevice => 'Dispositivo nelle vicinanze';

  @override
  String get notificationsPermissionDenied =>
      'Consenti le notifiche di sistema nelle impostazioni del sistema operativo per ricevere le richieste in background.';

  @override
  String get notificationsSystemUnavailable =>
      'Le notifiche di sistema non sono disponibili. Le richieste di connessione restano qui.';

  @override
  String get platformCaptureStateUnavailable => 'Non disponibile';

  @override
  String get platformCaptureStatePaused => 'In pausa';

  @override
  String get platformCaptureStateRunning => 'Acquisizione in corso';

  @override
  String get platformCaptureStateStopped => 'Non in esecuzione';

  @override
  String get platformCaptureActionUnavailable => 'Acquisizione non disponibile';

  @override
  String get platformCaptureActionResume => 'Riprendi acquisizione';

  @override
  String get platformCaptureActionPause => 'Sospendi acquisizione';

  @override
  String get platformCaptureActionResumeStopped =>
      'Riprendi acquisizione (non in esecuzione)';

  @override
  String get platformTrayOpen => 'Apri';

  @override
  String get platformTraySettings => 'Impostazioni';

  @override
  String get platformTrayQuit => 'Esci';

  @override
  String get platformWindowControlsUnavailable =>
      'I controlli della finestra desktop non sono disponibili. CopyPaste resterà visibile.';

  @override
  String get platformTrayUnavailable =>
      'L\'area di notifica non è disponibile. Tieni aperto CopyPaste, poi riprova o esci.';

  @override
  String get platformQuitFailed => 'Impossibile uscire da CopyPaste. Riprova.';

  @override
  String get platformVisibilityChangeFailed =>
      'Impossibile modificare la visibilità dell\'app. Riprova.';

  @override
  String get platformCaptureNotificationTitle => 'Appunti salvati';

  @override
  String get platformCaptureNotificationBody =>
      'Un nuovo elemento è stato aggiunto a CopyPaste.';

  @override
  String get platformPairingNotificationTitle => 'Richiesta di connessione';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName vuole connettersi. Apri CopyPaste per confrontare il codice e confermare.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Richieste di connessione dei dispositivi';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Richieste di connessione dai dispositivi nelle vicinanze';

  @override
  String get platformNotificationDefaultAction => 'Apri';

  @override
  String get platformFilesSelectionFailed => 'Impossibile selezionare i file.';

  @override
  String get platformFilesOnlyRegular =>
      'È possibile importare solo file normali.';

  @override
  String get platformModulesUnavailable =>
      'I moduli non sono disponibili su questo dispositivo.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'Impossibile configurare l\'accesso agli SMS.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'Impossibile verificare l\'accesso agli SMS.';

  @override
  String get platformUpdatesUnavailableHere =>
      'Gli aggiornamenti dell\'app non sono disponibili qui.';

  @override
  String get platformUpdateVersionUnavailable =>
      'La versione installata non è disponibile.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'Gli aggiornamenti dell\'app non sono disponibili per questa installazione.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Questa installazione Linux non può aggiornarsi automaticamente.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Questo aggiornamento Linux non corrisponde all\'architettura in uso.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Questa installazione macOS non può aggiornarsi automaticamente.';

  @override
  String get platformUpdateStartFailed =>
      'CopyPaste non è riuscito ad avviare l\'aggiornamento.';

  @override
  String get platformGlobalShortcutDescription => 'Apri Quick Paste';

  @override
  String get settingsSectionGeneral => 'Generali';

  @override
  String get settingsSectionGeneralSummary => 'Lingua e visibilità dell\'app.';

  @override
  String get settingsSectionClipboard => 'Appunti';

  @override
  String get settingsSectionClipboardSummary =>
      'Acquisizione e limiti della cronologia.';

  @override
  String get settingsSectionPrivacy => 'Privacy';

  @override
  String get settingsSectionPrivacySummary =>
      'Esclusione delle app e protezione dello schermo.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Scorciatoia e comportamento dell\'incolla automatico.';

  @override
  String get settingsSectionSync => 'Sincronizzazione';

  @override
  String get settingsSectionSyncSummary =>
      'Sincronizzazione e rilevamento dei dispositivi nelle vicinanze.';

  @override
  String get settingsSectionNotifications => 'Notifiche';

  @override
  String get settingsSectionNotificationsSummary =>
      'Notifiche e suoni di acquisizione.';

  @override
  String get settingsSectionData => 'Dati';

  @override
  String get settingsSectionDataSummary => 'Esportazione, backup e ripristino.';

  @override
  String get settingsSectionModules => 'Moduli';

  @override
  String get settingsSectionModulesSummary =>
      'Installa e gestisci i moduli opzionali.';

  @override
  String get settingsSectionRules => 'Regole sui contenuti';

  @override
  String get settingsSectionRulesSummary =>
      'Etichetta, ignora, proteggi o fai scadere gli elementi automaticamente.';

  @override
  String get settingsSectionQuickAccess => 'Accesso rapido';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Tastiera, riquadri e widget della schermata Home.';

  @override
  String get settingsSectionSnippets => 'Frammenti';

  @override
  String get settingsSectionSnippetsSummary =>
      'Testi e modelli riutilizzabili.';

  @override
  String get settingsSectionTransforms => 'Trasformazioni del testo';

  @override
  String get settingsSectionTransformsSummary =>
      'Preferiti, sequenze e trasformazioni predefinite per l\'incolla.';

  @override
  String get settingsSectionAiTools => 'Strumenti IA';

  @override
  String get settingsSectionAiToolsSummary =>
      'Collega assistenti IA e controlla cosa possono vedere.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Raccogli più copie e incollale in ordine.';

  @override
  String get settingsSectionAbout => 'Informazioni';

  @override
  String get settingsSectionAboutSummary =>
      'Versione e aggiornamenti dell\'app.';

  @override
  String get settingsGeneralDescription =>
      'Lingua, avvio automatico e visibilità dell\'app.';

  @override
  String get settingsModulesDescription => 'Funzioni opzionali per CopyPaste.';

  @override
  String get settingsClipboardDescription =>
      'Acquisisci le modifiche agli appunti e gestisci i limiti della cronologia.';

  @override
  String get settingsDataDescription =>
      'Esporta, esegui il backup e ripristina la cronologia degli appunti.';

  @override
  String get settingsSyncDescription =>
      'Gestisci la sincronizzazione con i dispositivi abbinati.';

  @override
  String get settingsPrivacyDescription =>
      'Scegli quali app acquisire e proteggi il contenuto degli appunti.';

  @override
  String get settingsNotificationsDescription =>
      'Scegli notifiche e suoni per gli elementi acquisiti.';

  @override
  String get settingsAboutDescription =>
      'Versione di CopyPaste e aggiornamenti dell\'app.';

  @override
  String get settingsQuickPasteDescription =>
      'Apri la cronologia degli appunti ovunque, senza cambiare finestra.';

  @override
  String get settingsLoadingMessage => 'Caricamento delle impostazioni.';

  @override
  String get settingsUnavailableTitle => 'Impostazioni non disponibili';

  @override
  String get settingsNoResultsTitle => 'Nessuna impostazione trovata';

  @override
  String get settingsNoResultsMessage => 'Prova con un\'altra ricerca.';

  @override
  String get settingsAttentionTitle => 'Le impostazioni richiedono attenzione';

  @override
  String get settingsSearchPlaceholder => 'Cerca nelle impostazioni';

  @override
  String get settingsBackToSettings => 'Torna a Impostazioni';

  @override
  String get settingsStartOnBootTitle =>
      'Avvia all\'accensione del dispositivo';

  @override
  String get settingsStartAtLoginTitle => 'Apri al login';

  @override
  String get settingsStartupSearchDescription =>
      'Avvia CopyPaste automaticamente.';

  @override
  String get settingsStartupKeywords =>
      'avvio login accensione apertura automaticamente autoavvio startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Disattivata in Gestione attività. Attiva CopyPaste in Impostazioni → App → Avvio.';

  @override
  String get settingsStartupMacApproval =>
      'Consenti CopyPaste in Impostazioni di Sistema → Generali → Elementi login.';

  @override
  String get settingsStartupUnavailable =>
      'L\'avvio automatico non è disponibile in questa installazione.';

  @override
  String get settingsStartupAndroidDescription =>
      'Riprendi i servizi in background attivati dopo il riavvio del dispositivo.';

  @override
  String get settingsStartupDesktopDescription =>
      'Apri CopyPaste automaticamente quando esegui il login.';

  @override
  String get settingsShowInDock => 'Mostra nel Dock';

  @override
  String get settingsShowInTaskbar => 'Mostra nella barra delle applicazioni';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Mostra l\'icona dell\'app per passare da una finestra all\'altra.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'dock barra applicazioni icona app visibilità dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Mostra nella barra dei menu';

  @override
  String get settingsShowInSystemTray => 'Mostra nell\'area di notifica';

  @override
  String get settingsTrayVisibilityDescription =>
      'Mostra l\'accesso rapido a CopyPaste e ai controlli di acquisizione.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'area notifica barra menu stato icona visibilità tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle =>
      'Impossibile modificare la visibilità';

  @override
  String get settingsModulesSearchDescription =>
      'Installa, aggiorna e rimuovi i moduli opzionali.';

  @override
  String get settingsModulesKeywords =>
      'funzioni comandi estensioni features commands extensions';

  @override
  String get settingsAndroidCaptureTitle =>
      'Acquisizione in background su Android';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Modalità Completa o Limitata con configurazione tramite Shizuku o ADB.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'background autorizzazioni configurazione background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Scegli la modalità Completa o Limitata e gestisci la configurazione una tantum di Shizuku o ADB.';

  @override
  String get settingsOpenSetup => 'Apri configurazione';

  @override
  String get settingsSaveScreenshotsTitle => 'Salva screenshot';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Salva automaticamente i nuovi screenshot di Android nella Cronologia.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'immagini foto acquisizione autorizzazioni images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Aggiungi automaticamente i nuovi screenshot alla Cronologia.';

  @override
  String get settingsScreenshotAccessTitle => 'Accesso agli screenshot';

  @override
  String get settingsScreenshotAccessDescription =>
      'Consenti l\'accesso a tutte le foto e alle notifiche per salvare i nuovi screenshot in background.';

  @override
  String get settingsAllowAccess => 'Consenti accesso';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'L\'acquisizione degli screenshot non è in esecuzione';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'I nuovi screenshot non vengono salvati. Riprendi l\'acquisizione per riavviare il servizio.';

  @override
  String get settingsResumeCapture => 'Riprendi acquisizione';

  @override
  String get settingsClipboardCaptureTitle => 'Acquisizione degli appunti';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Sospendi o riprendi l\'acquisizione degli appunti.';

  @override
  String get settingsCaptureStateUnavailable => 'Stato non disponibile';

  @override
  String get settingsCaptureStoppedTitle =>
      'L\'acquisizione non è in esecuzione';

  @override
  String get settingsCaptureStoppedMessage =>
      'Al momento CopyPaste non riceve le modifiche agli appunti su questo dispositivo.';

  @override
  String get settingsRetentionTitle => 'Conservazione';

  @override
  String get settingsRetentionDescription =>
      'Rimuovi automaticamente i vecchi elementi degli appunti non fissati.';

  @override
  String get settingsRetentionForever => 'Conserva per sempre';

  @override
  String get settingsStorageQuotaTitle => 'Quota di archiviazione';

  @override
  String get settingsStorageQuotaDescription =>
      'Spazio di archiviazione locale massimo usato dalla cronologia non fissata.';

  @override
  String get settingsStorageQuotaKeywords =>
      'disco spazio limite disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Dimensione massima dei file';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Limite per l\'aggiunta e il download di file.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'importazione download trasferimento dimensione limite import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Limite per l\'aggiunta e il download di file. Il contenuto dei file viene trasferito solo quando scegli Scarica.';

  @override
  String get settingsHistoryFilesTitle => 'File della cronologia';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Esporta la cronologia, crea backup o ripristina un backup.';

  @override
  String get settingsHistoryFilesKeywords =>
      'testo crittografato backup dati text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Esporta la cronologia dei testi';

  @override
  String get settingsExportTextDescription =>
      'Salva gli elementi di testo in un file portabile.';

  @override
  String get settingsExportAction => 'Esporta';

  @override
  String get settingsBackupTitle => 'Backup crittografato';

  @override
  String get settingsBackupDescription =>
      'Salva l\'intera cronologia locale di questo dispositivo.';

  @override
  String get settingsCreateBackupAction => 'Crea backup';

  @override
  String get settingsRestoreTitle => 'Ripristina backup';

  @override
  String get settingsRestoreDescription =>
      'Sostituisci la cronologia locale con un backup crittografato.';

  @override
  String get settingsRestoreAction => 'Ripristina';

  @override
  String get settingsRestoreDialogTitle => 'Sostituire la cronologia locale?';

  @override
  String get settingsRestoreDialogMessage =>
      'Il backup crittografato selezionato sostituirà la cronologia degli appunti su questo dispositivo. Le impostazioni attuali e l\'identità del dispositivo verranno mantenute.';

  @override
  String get settingsRestoreFailed => 'Impossibile ripristinare il backup.';

  @override
  String get settingsExportIncompleteTitle =>
      'Esportazione del testo incompleta';

  @override
  String settingsExportIncompleteMessage(String saved, String omissions) {
    return '$saved; $omissions.';
  }

  @override
  String settingsExportOmissionsJoin(String first, String second) {
    return '$first e $second';
  }

  @override
  String get settingsSyncSwitchTitle => 'Sincronizzazione';

  @override
  String get settingsSyncSwitchDescription =>
      'Interruttore principale della sincronizzazione con i dispositivi abbinati.';

  @override
  String get settingsSyncSearchDescription =>
      'Sincronizzazione con i dispositivi abbinati.';

  @override
  String get settingsInstantClipboardTitle => 'Appunti istantanei';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Copia automaticamente i nuovi elementi dagli altri dispositivi.';

  @override
  String get settingsInstantClipboardKeywords =>
      'automatico copia incolla sincronizzazione ricevuti automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Copia automaticamente negli appunti di questo dispositivo gli elementi più recenti degli altri dispositivi.';

  @override
  String get settingsLanVisibilityTitle => 'Visibilità LAN';

  @override
  String get settingsLanVisibilityDescription =>
      'Consenti ai dispositivi nelle vicinanze di rilevare questo dispositivo.';

  @override
  String get settingsLanVisibilityKeywords =>
      'rete locale rilevamento local network discovery';

  @override
  String get settingsExcludedAppsTitle => 'App escluse';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Ignora l\'acquisizione automatica dalle app escluse.';

  @override
  String get settingsExcludedAppsKeywords =>
      'privacy app identificatori privacy app identifiers';

  @override
  String get settingsExcludedAppsMacDescription =>
      'Ignora l\'acquisizione automatica durante l\'attività di queste app. Le copie in background potrebbero aggirare le esclusioni.';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Ignora l\'acquisizione automatica dai proprietari degli appunti identificati. Se sono impostate esclusioni e l\'origine non può essere identificata, l\'acquisizione viene sospesa.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Android ignora l\'acquisizione automatica quando sono impostate esclusioni, perché non può identificare le app di origine.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'L\'esclusione delle app è supportata su macOS, Windows, Android e Linux.';

  @override
  String get settingsExcludedAppsAdd => 'Aggiungi';

  @override
  String get settingsExcludedAppsEmpty => 'Nessuna app esclusa.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return 'Rimuovi $app';
  }

  @override
  String get settingsExcludeDialogTitle => 'Escludi app';

  @override
  String get settingsExcludeDialogAction => 'Escludi';

  @override
  String get settingsExcludeDialogPlaceholder => 'Identificatore dell\'app';

  @override
  String get settingsExcludeFailed => 'Impossibile escludere l\'app.';

  @override
  String get settingsSkipSecretTitle => 'Ignora appunti riservati';

  @override
  String get settingsSkipSecretDescription =>
      'Tieni fuori dalla Cronologia i segreti contrassegnati dall\'app di origine.';

  @override
  String get settingsSkipSecretKeywords =>
      'segreto password sensibile nascosto spoiler secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Ignora appunti temporanei';

  @override
  String get settingsSkipTransientDescription =>
      'Tieni fuori dalla Cronologia le copie temporanee contrassegnate dall\'app di origine.';

  @override
  String get settingsSkipTransientKeywords =>
      'transitorio temporaneo privacy transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Blocca screenshot';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Impedisci screenshot e registrazioni dello schermo di CopyPaste.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'Il blocco degli screenshot non è disponibile su questa piattaforma.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'privacy schermo acquisizione protezione abbinamento qr sicurezza codice privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Notifica alla copia';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Mostra una notifica dopo un\'acquisizione in background.';

  @override
  String get settingsNotificationPreviewTitle =>
      'Mostra contenuto degli appunti';

  @override
  String get settingsNotificationPreviewDescription =>
      'Includi un\'anteprima dell\'elemento nelle notifiche di copia.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'notifica anteprima testo immagine privacy notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Suono alla copia';

  @override
  String get settingsSoundOnCopyDescription =>
      'Riproduci il feedback della piattaforma dopo un\'acquisizione riuscita.';

  @override
  String get settingsVersionTitle => 'Versione';

  @override
  String get settingsVersionDescription => 'Versione di CopyPaste installata.';

  @override
  String get settingsVersionUnavailable => 'Non disponibile';

  @override
  String get settingsUpdatesTitle => 'Aggiornamenti dell\'app';

  @override
  String get settingsUpdatesSearchDescription =>
      'Verifica e installa gli aggiornamenti di CopyPaste.';

  @override
  String get settingsUpdatesKeywords =>
      'versione github release version github release';

  @override
  String get settingsUpdateIdle =>
      'Verifica se è disponibile una versione più recente di CopyPaste.';

  @override
  String get settingsUpdateChecking => 'Verifica degli aggiornamenti in corso.';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste è aggiornato.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'La versione $version è aggiornata.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'È disponibile la versione $available. Su questo dispositivo è installata la versione $current.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Download della versione $version · $percent%.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Preparazione degli aggiornamenti dei moduli installati.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Installazione della versione $version.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'Gli aggiornamenti dell\'app richiedono attenzione.';

  @override
  String get settingsUpdateNowAction => 'Aggiorna ora';

  @override
  String get settingsUpdateOpenReleaseAction => 'Apri pagina versione';

  @override
  String get settingsUpdateCheckingAction => 'Verifica in corso';

  @override
  String get settingsUpdateDownloadingAction => 'Download in corso';

  @override
  String get settingsUpdateInstallingAction => 'Installazione in corso';

  @override
  String get settingsUpdateRestartAction => 'Riavvia CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => 'Verifica di nuovo';

  @override
  String get settingsUpdateNoneTitle => 'Nessun aggiornamento disponibile';

  @override
  String get settingsUpdateLatest => 'CopyPaste è alla versione più recente.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version è la versione più recente.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Aggiornamento disponibile';

  @override
  String get settingsUpdateReady =>
      'Una versione più recente di CopyPaste è pronta per l\'installazione.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version è pronto per l\'installazione.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Aggiornamento non disponibile';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Questa installazione non può aggiornarsi automaticamente.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Verifica degli aggiornamenti non riuscita';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'CopyPaste non è riuscito a verificare gli aggiornamenti.';

  @override
  String get settingsQuickPasteSetupTitle => 'Configura Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Attiva l\'estensione firmata per GNOME o KDE e consenti il controllo della tastiera per tornare all\'app precedente.';

  @override
  String get settingsQuickPasteDesktopIntegration => 'Integrazione desktop';

  @override
  String get settingsQuickPasteAllowKeyboard => 'Consenti controllo tastiera';

  @override
  String get settingsQuickPasteCheckAgain => 'Verifica di nuovo';

  @override
  String get settingsQuickPasteLoading =>
      'Caricamento delle impostazioni di Quick Paste.';

  @override
  String get settingsQuickPasteUnavailableTitle =>
      'Quick Paste non è disponibile';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'Questo dispositivo non supporta la finestra di Quick Paste.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Apri Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Configura la scorciatoia globale di Quick Paste.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'tastiera scorciatoia tasto rapido keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'La scorciatoia funziona mentre CopyPaste è in esecuzione in background.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Scorciatoia registrata: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Incolla automaticamente';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Incolla l\'elemento selezionato nell\'app precedente.';

  @override
  String get settingsAutoPasteKeywords =>
      'accessibilità automatico accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Quando selezioni un elemento, torni all\'app precedente e l\'elemento viene incollato.';

  @override
  String get settingsAccessibilityRequiredTitle =>
      'È richiesta l\'autorizzazione Accessibilità';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Finché l\'autorizzazione non viene concessa, selezionando un elemento lo copi senza incollarlo.';

  @override
  String get settingsQuickPasteAttentionTitle =>
      'Quick Paste richiede attenzione';

  @override
  String get settingsShortcutRecording => 'Premi la scorciatoia…';

  @override
  String get settingsShortcutNotSet => 'Non impostata';

  @override
  String get settingsShortcutReset => 'Reimposta scorciatoia';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count giorni',
      many: '$count giorni',
      one: '$count giorno',
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
      other: '$count elementi di testo sono stati salvati',
      many: '$count elementi di testo sono stati salvati',
      one: '$count elemento di testo è stato salvato',
    );
    return '$_temp0';
  }

  @override
  String settingsExportSkippedNonText(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementi non testuali sono stati esclusi',
      many: '$count elementi non testuali sono stati esclusi',
      one: '$count elemento non testuale è stato escluso',
    );
    return '$_temp0';
  }

  @override
  String settingsExportSkippedUnreadable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementi di testo non sono leggibili',
      many: '$count elementi di testo non sono leggibili',
      one: '$count elemento di testo non è leggibile',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Caricamento dei dispositivi…';

  @override
  String get devicesUnavailableTitle => 'Dispositivi non disponibili';

  @override
  String get devicesNearbyTitle => 'Dispositivi nelle vicinanze';

  @override
  String get devicesScan => 'Cerca';

  @override
  String get devicesScanning => 'Ricerca in corso…';

  @override
  String get devicesNoneNearby => 'Nessun dispositivo nelle vicinanze';

  @override
  String get devicesThisDevice => 'Questo dispositivo';

  @override
  String get devicesTrusted => 'Attendibile';

  @override
  String get devicesConnect => 'Connetti';

  @override
  String devicesCardPing(String ping) {
    return 'Ping: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Ping:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'Rinomina questo dispositivo';

  @override
  String get devicesNamePlaceholder => 'Nome del dispositivo';

  @override
  String get devicesRenameFailed =>
      'Impossibile salvare il nome del dispositivo.';

  @override
  String devicesRevokeDialogTitle(String device) {
    return 'Revocare $device?';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return 'Rimuovere l\'abbinamento di $device?';
  }

  @override
  String get devicesRevokeDialogMessage =>
      'Il dispositivo verrà rimosso dal tuo gruppo e questo abbinamento verrà bloccato in modo permanente. Gli altri dispositivi applicheranno la rimozione alla successiva riconnessione.';

  @override
  String get devicesUnpairDialogMessage =>
      'Il dispositivo verrà rimosso dal tuo gruppo. Gli altri dispositivi applicheranno la rimozione alla successiva riconnessione. Per aggiungerlo di nuovo sarà necessaria una nuova conferma.';

  @override
  String get devicesRevoke => 'Revoca';

  @override
  String get devicesUnpair => 'Rimuovi abbinamento';

  @override
  String get devicesActionFailedMessage =>
      'Impossibile completare l\'azione sul dispositivo.';

  @override
  String get devicesCloseDetails => 'Chiudi dettagli dispositivo';

  @override
  String get devicesDetailType => 'Tipo di dispositivo';

  @override
  String get devicesDetailOperatingSystem => 'Sistema operativo';

  @override
  String get devicesDetailModel => 'Modello';

  @override
  String get devicesDetailAppVersion => 'Versione di CopyPaste';

  @override
  String get devicesDetailProtocolVersion => 'Versione del protocollo';

  @override
  String get devicesDetailStatus => 'Stato';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => 'Misurato';

  @override
  String get devicesDetailLastSeen => 'Ultimo rilevamento';

  @override
  String get devicesDetailLanEndpoint => 'Endpoint LAN';

  @override
  String get devicesDetailDeviceId => 'ID dispositivo';

  @override
  String get devicesDetailPairingId => 'ID abbinamento';

  @override
  String get devicesDetailProfileProvenance => 'Provenienza del profilo';

  @override
  String get devicesDetailProfileTrust => 'Attendibilità del profilo';

  @override
  String get devicesDetailProfileUpdated => 'Profilo aggiornato';

  @override
  String get devicesValueUnavailable => 'Non disponibile';

  @override
  String get devicesRenameDevice => 'Rinomina dispositivo';

  @override
  String get devicesSyncNow => 'Sincronizza ora';

  @override
  String get devicesRevokePairing => 'Revoca abbinamento';

  @override
  String get devicesLocal => 'Locale';

  @override
  String get devicesTrustAuthenticated => 'Autenticato';

  @override
  String get devicesTrustUnverified => 'Non verificato';

  @override
  String get devicesProvenanceSelfReported => 'Autodichiarato';

  @override
  String get devicesProvenanceObserved => 'Osservato';

  @override
  String get devicesProvenanceMeasured => 'Misurato';

  @override
  String get devicesConnectionOnline => 'Online';

  @override
  String get devicesConnectionOffline => 'Offline';

  @override
  String get devicesConnectionVisible => 'Visibile';

  @override
  String get devicesConnectionNotSeen => 'Non rilevato';

  @override
  String get devicesConnectionUnknown => 'Sconosciuto';

  @override
  String get devicesPeerAvailable => 'Disponibile';

  @override
  String get devicesPeerVisibleNow => 'Visibile ora';

  @override
  String get devicesPeerStatusUnknown => 'Stato sconosciuto';

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
  String get devicesClassLaptop => 'Portatile';

  @override
  String get devicesClassPhone => 'Telefono';

  @override
  String get devicesClassTablet => 'Tablet';

  @override
  String get devicesClassUnknown => 'Dispositivo';

  @override
  String get devicesUnknownOs => 'Sistema operativo sconosciuto';

  @override
  String get devicesAddDevice => 'Aggiungi dispositivo';

  @override
  String get devicesScanQr => 'Scansiona QR';

  @override
  String get devicesEnterCode => 'Inserisci codice';

  @override
  String get devicesScannerUnavailableTitle => 'Scanner non disponibile';

  @override
  String get devicesActionFailedTitle => 'Azione non riuscita';

  @override
  String get devicesRefreshFailedTitle =>
      'Aggiornamento dei dispositivi non riuscito';

  @override
  String get devicesNearbyPairingUnavailable =>
      'L\'abbinamento con i dispositivi nelle vicinanze non è disponibile.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'L\'abbinamento con i dispositivi nelle vicinanze non è disponibile. Usa Aggiungi dispositivo per mostrare un codice di abbinamento.';

  @override
  String get pairingTitle => 'Abbinamento';

  @override
  String get pairingPairDevice => 'Abbina un dispositivo';

  @override
  String get pairingScanQrCode => 'Scansiona codice QR';

  @override
  String get pairingEnterPairingCode => 'Inserisci il codice di abbinamento';

  @override
  String get pairingConnectNearbyDevice =>
      'Connetti dispositivo nelle vicinanze';

  @override
  String get pairingSendingRequest => 'Invio della richiesta di connessione…';

  @override
  String get pairingEmptyTitle => 'Scegli un\'azione di abbinamento';

  @override
  String get pairingEmptyMessage =>
      'I controlli di abbinamento vengono visualizzati qui.';

  @override
  String get pairingCreatingInvitation =>
      'Creazione di un invito di abbinamento.';

  @override
  String get pairingCode => 'Codice di abbinamento';

  @override
  String get pairingCodeHint =>
      'Inserisci il codice di 8 caratteri mostrato sull\'altro dispositivo.';

  @override
  String get pairingDeviceAddress => 'Indirizzo del dispositivo';

  @override
  String get pairingDeviceAddressHint =>
      'Usa l\'indirizzo host:port indicato dall\'altro dispositivo.';

  @override
  String get pairingCheckDetailsTitle =>
      'Controlla i dettagli dell\'abbinamento';

  @override
  String get pairingActionFailed => 'Azione di abbinamento non riuscita';

  @override
  String get pairingConnectingDevice => 'Connessione del dispositivo…';

  @override
  String get pairingQrScannerTitle => 'Scanner QR';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'L\'anteprima della fotocamera viene visualizzata qui.';

  @override
  String get pairingScannerNeedsAttention => 'Lo scanner richiede attenzione';

  @override
  String get pairingCodeIncomplete =>
      'Inserisci il codice di abbinamento completo di 8 caratteri.';

  @override
  String get pairingAddressMissing =>
      'Inserisci l\'indirizzo host:port del dispositivo.';

  @override
  String get pairingWaitingForOtherDevice =>
      'In attesa dell\'altro dispositivo';

  @override
  String get pairingInviteInstructions =>
      'Sull\'altro dispositivo, scegli Aggiungi dispositivo → Inserisci codice oppure scansiona il codice QR.';

  @override
  String get pairingDetailsTitle => 'Dettagli dell\'abbinamento';

  @override
  String get pairingAddress => 'Indirizzo';

  @override
  String get pairingVerificationCode => 'Codice di verifica';

  @override
  String get pairingVerificationCodeHint =>
      'Conferma solo se il codice corrisponde su entrambi i dispositivi.';

  @override
  String get pairingReject => 'Rifiuta';

  @override
  String get pairingConfirm => 'Conferma';

  @override
  String get pairingAccept => 'Accetta';

  @override
  String get pairingJoinDevice => 'Abbina dispositivo';

  @override
  String get pairingStartScanner => 'Avvia scanner';

  @override
  String get pairingActionNotStarted =>
      'Impossibile avviare l\'azione di abbinamento.';

  @override
  String get pairingStateHandshaking => 'Verifica del dispositivo';

  @override
  String get pairingStateAwaitingConfirmation =>
      'Conferma su entrambi i dispositivi';

  @override
  String get pairingStateConfirmed => 'Dispositivo abbinato';

  @override
  String get pairingStateRejected => 'Abbinamento rifiutato';

  @override
  String get pairingStateCancelled => 'Abbinamento annullato';

  @override
  String get pairingStateTimedOut => 'Abbinamento scaduto';

  @override
  String get pairingStateFailed => 'Abbinamento non riuscito';

  @override
  String get pairingHandshakingDescription =>
      'I dispositivi stanno stabilendo un canale sicuro.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Confronta il codice di verifica protetto, poi conferma su entrambi i dispositivi.';

  @override
  String get pairingConfirmedDescription =>
      'Il dispositivo è connesso. L\'appartenenza al gruppo verrà sincronizzata automaticamente.';

  @override
  String get pairingNoDeviceAdded => 'Nessun dispositivo è stato aggiunto.';

  @override
  String get pairingTimedOutDescription =>
      'Avvia un nuovo tentativo di abbinamento per continuare.';

  @override
  String get pairingIdleDescription =>
      'Avvia un tentativo di abbinamento per connettere un dispositivo.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'L\'abbinamento protetto non è disponibile';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Questa sessione di abbinamento deve essere aperta in una finestra protetta dedicata.';

  @override
  String get pairingCancel => 'Annulla abbinamento';

  @override
  String get pairingOrEnterCode => 'Oppure inserisci un codice di abbinamento';

  @override
  String get pairingJoinSecurely => 'Abbina in modo sicuro';

  @override
  String get pairingProtectedWaitingDescription =>
      'Mostra un codice QR, scansionane uno o inserisci un codice di abbinamento in questa finestra protetta.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Scade tra $seconds s';
  }

  @override
  String get pairingInvalidQr =>
      'Scansiona un codice QR di abbinamento di CopyPaste.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Lo scanner di Google non è disponibile. Inserisci invece il codice di abbinamento.';

  @override
  String get pairingScannerCouldNotOpen =>
      'Impossibile aprire lo scanner. Inserisci invece il codice di abbinamento.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'La visualizzazione protetta dell\'abbinamento non è disponibile.';

  @override
  String get pairingVerificationCodeRequired =>
      'Prima mostra e confronta il codice di verifica.';

  @override
  String get syncStatusUnavailable =>
      'Stato della sincronizzazione non disponibile';

  @override
  String get syncStatusOff => 'Sincronizzazione disattivata';

  @override
  String get syncStatusWaitingForSync => 'In attesa della sincronizzazione';

  @override
  String get syncStatusSyncing => 'Sincronizzazione in corso';

  @override
  String get syncStatusUpToDate => 'Aggiornato';

  @override
  String get syncStatusInterrupted => 'Sincronizzazione interrotta';

  @override
  String get syncStatusPartiallySynchronized => 'Sincronizzato parzialmente';

  @override
  String get syncStatusRetrying => 'Nuovo tentativo in corso';

  @override
  String get syncStatusWaitingToSync => 'In attesa di sincronizzazione';

  @override
  String get syncStatusWaitingForDevice => 'In attesa del dispositivo';

  @override
  String get syncStatusNoPairedDevices => 'Nessun dispositivo abbinato';

  @override
  String get syncStatusDevicesOffline => 'Dispositivi offline';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · registro attività non disponibile';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Sincronizzazione disattivata · $count problemi',
      many: 'Sincronizzazione disattivata · $count problemi',
      one: 'Sincronizzazione disattivata · $count problema',
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
      one: '$status · $count offline',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dispositivi richiedono attenzione',
      many: '$count dispositivi richiedono attenzione',
      one: '$count dispositivo richiede attenzione',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Dispositivi offline · $count problemi',
      many: 'Dispositivi offline · $count problemi',
      one: 'Dispositivi offline · $count problema',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Verifica della connessione';

  @override
  String get syncConnectionUnknown => 'Connessione sconosciuta';

  @override
  String get syncLogDeviceOffline => 'Il dispositivo è andato offline.';

  @override
  String get syncLogConnectionRestored => 'Connessione ripristinata.';

  @override
  String get syncLogProblemDetected =>
      'Rilevato un problema di sincronizzazione.';

  @override
  String get syncLogProblemResolved => 'Problema di sincronizzazione risolto.';

  @override
  String get syncLogFailed => 'Sincronizzazione non riuscita.';

  @override
  String get syncLogRecovered => 'Sincronizzazione ripristinata.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent elementi inviati',
      many: '$sent elementi inviati',
      one: '$sent elemento inviato',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received elementi ricevuti',
      many: '$received elementi ricevuti',
      one: '$received elemento ricevuto',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Sincronizzazione: $status';
  }

  @override
  String get syncDetailsTitle => 'Sincronizzazione';

  @override
  String get syncDetailsClose => 'Chiudi dettagli sincronizzazione';

  @override
  String get syncNoPairedDevicesMessage =>
      'Abbina un dispositivo per sincronizzare la cronologia degli appunti.';

  @override
  String get syncActivityLogUnavailableTitle =>
      'Registro attività non disponibile';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Impossibile leggere o salvare l\'attività di sincronizzazione.';

  @override
  String get syncDetailsDevicesHeading => 'Dispositivi';

  @override
  String get syncDetailsActivityHeading => 'Attività';

  @override
  String get syncDetailsNoRecentProblems =>
      'Nessun problema di sincronizzazione recente';

  @override
  String get syncDetailsUnknownPeer => 'Dispositivo abbinato';

  @override
  String get syncPeerConnection => 'Connessione';

  @override
  String get syncPeerLastCheckStarted => 'Inizio ultimo controllo';

  @override
  String get syncPeerItemsOverSizeLimit =>
      'Elementi oltre il limite di dimensione';

  @override
  String syncPeerLastSeen(String time) {
    return 'Ultimo rilevamento: $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Ultima sincronizzazione: $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Ultima sincronizzazione riuscita';

  @override
  String get syncPeerConnectionDetails => 'Dettagli connessione';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Dettagli connessione di $device';
  }

  @override
  String get historyKindText => 'Testo';

  @override
  String get historyKindLink => 'Link';

  @override
  String get historyKindEmail => 'Email';

  @override
  String get historyKindColor => 'Colore';

  @override
  String get historyKindPhone => 'Telefono';

  @override
  String get historyKindCode => 'Codice';

  @override
  String get historyKindPath => 'Percorso';

  @override
  String get historyKindCredentials => 'Credenziali';

  @override
  String get historyKindImage => 'Immagine';

  @override
  String get historyKindFile => 'File';

  @override
  String get historyKindOther => 'Altro';

  @override
  String get historySortNewest => 'Prima i più recenti';

  @override
  String get historySortOldest => 'Prima i meno recenti';

  @override
  String get historySortRelevance => 'Più pertinenti';

  @override
  String get historyErrorUpdatesUnavailable =>
      'Gli aggiornamenti della cronologia sono temporaneamente non disponibili.';

  @override
  String get historyErrorBulkDelete =>
      'Impossibile eliminare alcuni elementi selezionati. Riprova.';

  @override
  String get historyErrorBulkPin =>
      'Impossibile aggiornare il fissaggio di alcuni elementi selezionati. Riprova.';

  @override
  String get historyErrorRefresh =>
      'Impossibile aggiornare la cronologia. Riprova.';

  @override
  String get historyErrorLoad => 'Impossibile caricare la cronologia. Riprova.';

  @override
  String get historyErrorLoadMore =>
      'Impossibile caricare altri elementi della cronologia.';

  @override
  String get historyErrorLoadEarlier =>
      'Impossibile caricare la cronologia precedente.';

  @override
  String get historyErrorOpenClip => 'Impossibile aprire questo elemento.';

  @override
  String get historyErrorCopyPlainText =>
      'Impossibile copiare il testo semplice.';

  @override
  String get historyErrorCopyClip => 'Impossibile copiare questo elemento.';

  @override
  String get historyErrorCopyText => 'Impossibile copiare questo testo.';

  @override
  String get historyErrorPrepareFile =>
      'Impossibile scaricare questo file. Controlla il dispositivo di origine e riprova.';

  @override
  String get historyErrorDownloadFile => 'Impossibile scaricare questo file.';

  @override
  String get historyErrorSelectFiles =>
      'Impossibile selezionare i file. Riprova.';

  @override
  String get historyErrorReadSelectedFile =>
      'Impossibile leggere il file selezionato. Controlla l\'accesso e lo spazio di archiviazione disponibile.';

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
          'Impossibile importare $failed file su $total. Primo: $name. $reason',
      many:
          'Impossibile importare $failed file su $total. Primo: $name. $reason',
      one:
          'Impossibile importare $failed file su $total. Primo: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'Impossibile aggiornare lo stato riservato dell\'elemento.';

  @override
  String get historyErrorPinUpdate => 'Impossibile aggiornare il fissaggio.';

  @override
  String get historyErrorPinnedOrderSave =>
      'Impossibile salvare l\'ordine degli elementi fissati. Riprova.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'L\'ordine è stato salvato, ma non è stato possibile aggiornare la cronologia.';

  @override
  String get historyErrorDeleteClip => 'Impossibile eliminare questo elemento.';

  @override
  String get historyErrorDeleteAll => 'Impossibile eliminare la cronologia.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'Impossibile eliminare gli elementi selezionati.';

  @override
  String get historyOcrErrorUnavailable =>
      'L\'OCR non è disponibile. Controlla il modulo in Impostazioni e riprova.';

  @override
  String get historyOcrErrorStart => 'Impossibile avviare l\'OCR. Riprova.';

  @override
  String get historyOcrErrorRecognize =>
      'Impossibile riconoscere il testo dell\'immagine. Riprova.';

  @override
  String get historyOcrRecognizing =>
      'Riconoscimento del testo dell\'immagine in corso.';

  @override
  String get historyOcrFailedTitle => 'OCR non riuscito';

  @override
  String get historyOcrNoText => 'Nessun testo trovato';

  @override
  String get historyOcrTitle => 'Testo dell\'immagine';

  @override
  String get historyOcrTextCopied => 'Testo riconosciuto copiato.';

  @override
  String get historyOcrCopyError => 'Impossibile copiare il testo. Riprova.';

  @override
  String get historyTagsErrorLoad => 'Impossibile caricare i tag. Riprova.';

  @override
  String get historyTagsErrorSave =>
      'Impossibile salvare la modifica del tag. Usa un nome univoco di massimo 64 caratteri e seleziona elementi fissati.';

  @override
  String get historyTags => 'Tag';

  @override
  String get historyTagsUpdateErrorTitle => 'Impossibile aggiornare i tag';

  @override
  String get historyTagsEmpty => 'Ancora nessun tag';

  @override
  String get historyTagRename => 'Rinomina';

  @override
  String get historyTagActions => 'Azioni del tag';

  @override
  String get historyTagNamePlaceholder => 'Nome del tag';

  @override
  String get historyCreateTag => 'Crea tag';

  @override
  String get historySaveTag => 'Salva tag';

  @override
  String get historyTagsFilterAll => 'Tutti i tag';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count tag',
      many: '$count tag',
      one: '$count tag',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Filtra per tag: $selection';
  }

  @override
  String get historyManageTags => 'Gestisci tag';

  @override
  String get historyTagsFilterSemantics =>
      'Filtra gli elementi fissati per tag';

  @override
  String get historyDownloadErrorCancelled =>
      'Il download del file è stato annullato.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'Il servizio di download non è disponibile. Riapri CopyPaste e riprova.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'Il servizio di download non si è connesso. Riprova.';

  @override
  String get historyFileErrorAccess =>
      'Impossibile accedere al file selezionato.';

  @override
  String get historySpoilerRevealSemantics =>
      'Contenuto riservato. Mostra spoiler';

  @override
  String get historyActionFailedTitle => 'Azione non riuscita';

  @override
  String get historyLoadingMessage =>
      'Caricamento della cronologia degli appunti.';

  @override
  String get historyUnavailableTitle => 'Cronologia non disponibile';

  @override
  String get historyEmptyTitle => 'Nessun elemento trovato';

  @override
  String get historyEmptyMessage =>
      'Gli elementi acquisiti e i risultati della cronologia vengono visualizzati qui.';

  @override
  String get historyAttentionTitle => 'La cronologia richiede attenzione';

  @override
  String get historyLoadMore => 'Carica altri';

  @override
  String get historySectionPinned => 'Fissati';

  @override
  String get historySectionToday => 'Oggi';

  @override
  String get historySectionYesterday => 'Ieri';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count voci degli appunti sono state ignorate',
      many: '$count voci degli appunti sono state ignorate',
      one: '$count voce degli appunti è stata ignorata',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Alcuni elementi conservati non possono essere letti e non sono mostrati in questo elenco.';

  @override
  String get historySearch => 'Cerca nella cronologia';

  @override
  String get historyResetFilters => 'Reimposta filtri';

  @override
  String get historyImportFiles => 'Importa file';

  @override
  String get historySelectClips => 'Seleziona elementi';

  @override
  String get historyCloseSearch => 'Chiudi ricerca';

  @override
  String get historyFilterAllClips => 'Tutti gli elementi';

  @override
  String get historyFilterAllDevices => 'Tutti i dispositivi';

  @override
  String get historyFilterAllApps => 'Tutte le app';

  @override
  String get historyPinClip => 'Fissa elemento';

  @override
  String get historyUnpinClip => 'Rimuovi fissaggio';

  @override
  String get historyReorderPinnedClip => 'Riordina elemento fissato';

  @override
  String get historyReorderPinnedHint =>
      'Trascina per spostare. Alt + Freccia su o giù sposta di una posizione.';

  @override
  String get historyClipUnavailable => 'Elemento non disponibile';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Tipo di elemento: $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Seleziona un elemento';

  @override
  String get historyInspectorEmptyMessage =>
      'Il contenuto completo e le azioni vengono visualizzati qui.';

  @override
  String get historyNoPreview => 'Nessuna anteprima disponibile.';

  @override
  String get historyCloseClipDetails => 'Chiudi dettagli elemento';

  @override
  String get historyCopyOptions => 'Opzioni di copia';

  @override
  String get historyCopyPlainText => 'Copia come testo semplice';

  @override
  String get historyManageClipTags => 'Gestisci tag dell\'elemento';

  @override
  String get historyPinnedButton => 'Fissato';

  @override
  String get historyAddSpoiler => 'Aggiungi spoiler';

  @override
  String get historyRemoveSpoiler => 'Rimuovi spoiler';

  @override
  String get historyUpdateFailedTitle => 'Aggiornamento non riuscito';

  @override
  String get historyRecognizeImageText => 'Riconosci testo nell\'immagine';

  @override
  String get historyCopiedTitle => 'Copiato';

  @override
  String get historyCopyFailedTitle => 'Copia non riuscita';

  @override
  String get historyPlainTextCopied => 'Testo semplice copiato.';

  @override
  String get historyClipCopied => 'Elemento copiato.';

  @override
  String get historyDownloadedTitle => 'Scaricato';

  @override
  String get historyFileSaved => 'File salvato.';

  @override
  String get historyDownload => 'Scarica';

  @override
  String get historyDownloading => 'Download in corso…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Download in corso: $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => 'Annullamento…';

  @override
  String get historyDownloadCancellingTooltip => 'Annullamento del download';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Download in corso: $percent% · Fai clic per annullare';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Download non riuscito · Fai clic per riprovare';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Annulla download, $percent% completato';
  }

  @override
  String get historyRetryDownload => 'Riprova download';

  @override
  String get historyDownloadFailedTitle => 'Download non riuscito';

  @override
  String get historyShowFile => 'Mostra file';

  @override
  String get historyMetadataPrivacy => 'Privacy';

  @override
  String get historyMetadataConfidential => 'Riservato';

  @override
  String get historyMetadataCaptured => 'Acquisito';

  @override
  String get historyMetadataObservedApp => 'App rilevata';

  @override
  String get historyMetadataDevice => 'Dispositivo';

  @override
  String get historyMetadataClipType => 'Tipo di elemento';

  @override
  String get historyMetadataType => 'Tipo';

  @override
  String get historyMetadataPath => 'Percorso';

  @override
  String get historyMetadataFile => 'File';

  @override
  String get historyMetadataResolution => 'Risoluzione';

  @override
  String get historyMetadataSize => 'Dimensione';

  @override
  String get historyMetadataFiles => 'File';

  @override
  String get historyMetadataSync => 'Sincronizzazione';

  @override
  String get historyMetadataTooLargeToSync =>
      'Troppo grande per la sincronizzazione';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Eliminare $count elementi selezionati?',
      many: 'Eliminare $count elementi selezionati?',
      one: 'Eliminare $count elemento selezionato?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Eliminare questo elemento?';

  @override
  String get historyBulkTagTooltip =>
      'Aggiungi tag agli elementi fissati selezionati';

  @override
  String get historyBulkExit => 'Esci dalla selezione';

  @override
  String get historyBulkPin => 'Fissa selezionati';

  @override
  String get historyBulkUnpin => 'Rimuovi fissaggio dai selezionati';

  @override
  String get historyBulkDelete => 'Elimina selezionati';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count selezionati',
      many: '$count selezionati',
      one: '$count selezionato',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Filtra per data';

  @override
  String get historyDatePresetToday => 'Oggi';

  @override
  String get historyDatePresetYesterday => 'Ieri';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Ultimi $days giorni',
      many: 'Ultimi $days giorni',
      one: 'Ultimo giorno',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'Da sempre';

  @override
  String get appRuntimeStartFailed =>
      'CopyPaste non è riuscito ad avviare il proprio runtime.';

  @override
  String get appPreparing => 'Preparazione di CopyPaste.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste è in attesa dell\'accesso alla cronologia protetta. Le parti disponibili dell\'app restano utilizzabili.';

  @override
  String get appHistoryUnavailableTitle =>
      'Il runtime della cronologia non è disponibile';

  @override
  String get appHistoryUnavailableMessage =>
      'Avvia il runtime dell\'app prima di aprire la cronologia.';

  @override
  String get appDevicesUnavailableTitle =>
      'Il runtime dei dispositivi non è disponibile';

  @override
  String get appDevicesUnavailableMessage =>
      'Avvia il runtime dell\'app prima di aprire i dispositivi.';

  @override
  String get appSettingsUnavailableTitle =>
      'Il runtime delle impostazioni non è disponibile';

  @override
  String get appSettingsUnavailableMessage =>
      'Avvia il runtime dell\'app prima di aprire le impostazioni.';

  @override
  String get appQuit => 'Esci da CopyPaste';

  @override
  String get appWindowSetupTitle =>
      'La configurazione della finestra richiede attenzione';

  @override
  String get appCollapseNavigation => 'Comprimi navigazione';

  @override
  String get appExpandNavigation => 'Espandi navigazione';

  @override
  String get appOpenHistory => 'Apri Cronologia';

  @override
  String get appDestinationHistory => 'Cronologia';

  @override
  String get appDestinationDevices => 'Dispositivi';

  @override
  String get appDestinationSettings => 'Impostazioni';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, selezionato';
  }

  @override
  String get appActiveFilter => 'Filtro attivo';

  @override
  String get appActionFailed => 'Azione non riuscita';

  @override
  String get sharedAndroidVersionRequired =>
      'È richiesto Android 11 o versioni successive';

  @override
  String get sharedUseAdbTab => 'Usa la scheda ADB su questo dispositivo.';

  @override
  String get sharedOneTimeAccessApplied => 'Accesso una tantum applicato';

  @override
  String get sharedInstallShizuku => 'Installa Shizuku';

  @override
  String get sharedPairAndStartShizuku => 'Accoppia e avvia Shizuku';

  @override
  String get sharedAllowCopyPaste => 'Consenti CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded => 'Shizuku non è più necessario.';

  @override
  String get sharedGetShizukuDescription =>
      'Scarica Shizuku per applicare l\'accesso una tantum.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Usa Debug wireless in Shizuku, poi torna qui.';

  @override
  String get sharedApproveInShizuku =>
      'Approva CopyPaste una volta in Shizuku.';

  @override
  String get sharedGetShizuku => 'Scarica Shizuku';

  @override
  String get sharedOpenShizuku => 'Apri Shizuku';

  @override
  String get sharedRunOnComputer => 'Esegui sul computer';

  @override
  String get sharedCopyAllCommands => 'Copia tutti i comandi';

  @override
  String get sharedEnableUsbDebugging =>
      'Attiva Debug USB e collega questo telefono.';

  @override
  String get sharedAllow => 'Consenti';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Impossibile copiare i comandi di configurazione.';

  @override
  String get sharedShizukuOpenFailed => 'Impossibile aprire Shizuku.';

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
  String get onboardingTitle => 'Configura CopyPaste';

  @override
  String get onboardingWelcomeTitle => 'Ti diamo il benvenuto in CopyPaste';

  @override
  String get onboardingWelcomeSubtitle =>
      'I tuoi appunti, pronti quando lo sei tu.';

  @override
  String get onboardingReadyTitle => 'CopyPaste è pronto';

  @override
  String get onboardingGetStarted => 'Inizia';

  @override
  String get onboardingOptionalSetupTitle =>
      'La configurazione facoltativa richiede attenzione';

  @override
  String get onboardingOpenSystemSettings => 'Apri Impostazioni';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return 'PASSO $step DI $count · $platform';
  }

  @override
  String get onboardingSetupErrorTitle =>
      'La configurazione richiede attenzione';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste non è riuscito a leggere lo stato di configurazione di $platform.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste non è riuscito a salvare l\'avanzamento della configurazione iniziale.';

  @override
  String get onboardingStartAtLogin => 'Apri al login';

  @override
  String get onboardingStartAtLoginDescription =>
      'Apri CopyPaste quando esegui il login';

  @override
  String get onboardingStartAtLoginSemantics => 'Apri CopyPaste al login';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Impossibile attivare l\'apertura al login. Puoi continuare senza.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Impossibile aggiornare l\'apertura al login. Puoi continuare senza.';

  @override
  String get onboardingMacosTitle => 'Configura il Mac';

  @override
  String get onboardingAccessibility => 'Accessibilità';

  @override
  String get onboardingAutoPasteEnabled => 'Incolla automatico attivato';

  @override
  String get onboardingAutoPasteOptional =>
      'Facoltativo per l\'incolla automatico';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Disponibile nell\'app CopyPaste installata.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'È richiesta l\'approvazione dell\'elemento login';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Consenti CopyPaste in Impostazioni di Sistema, poi torna qui.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste non è riuscito a leggere lo stato di configurazione di macOS.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Impossibile richiedere l\'autorizzazione Accessibilità.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste non è riuscito ad aggiornare lo stato di configurazione di macOS.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Impossibile aggiornare lo stato di Accessibilità.';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Impossibile aprire le impostazioni Elementi login.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'L\'apertura al login richiede l\'approvazione in Impostazioni di Sistema.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Impossibile disattivare l\'apertura al login.';

  @override
  String get onboardingWindowsTitle => 'Configura il PC';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'Non disponibile in questa installazione. Puoi continuare senza.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Disattivata in Gestione attività';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Attiva CopyPaste nelle app di avvio di Windows, poi torna qui.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'L\'apertura al login è disattivata in Gestione attività. Puoi attivarla più tardi nelle Impostazioni di Windows.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'L\'apertura al login non è disponibile. Puoi continuare senza.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste non è riuscito a leggere lo stato di configurazione di Windows.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Impossibile aprire le impostazioni di avvio di Windows.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Impossibile aggiornare l\'apertura al login. Puoi modificarla più tardi in Impostazioni.';

  @override
  String get onboardingLinuxChecking =>
      'Verifica della sessione desktop Linux in corso.';

  @override
  String get onboardingLinuxTitle => 'Configura il desktop Linux';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste richiede un\'integrazione desktop verificata per l\'attribuzione degli appunti e Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Sessione desktop';

  @override
  String get onboardingLinuxX11Detected => 'Rilevata sessione X11.';

  @override
  String get onboardingLinuxWaylandDetected => 'Rilevata sessione Wayland.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'Non è stata rilevata alcuna sessione desktop X11 o Wayland supportata.';

  @override
  String get onboardingLinuxCompanion => 'Estensione per GNOME o KDE';

  @override
  String get onboardingLinuxCompanionReady =>
      'Il bridge Clipboard v2 autenticato è pronto.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'L\'acquisizione degli appunti è sospesa a causa delle esclusioni delle app di origine. Attiva l\'estensione firmata per riprenderla.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Installa e attiva il pacchetto di sessione dell\'estensione firmata, poi torna qui.';

  @override
  String get onboardingLinuxOpenSetup => 'Apri configurazione';

  @override
  String get onboardingLinuxKeyboardControl => 'Controllo tastiera';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'Il controllo della tastiera è consentito per Quick Paste.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'Il controllo della tastiera non è disponibile in questa sessione desktop.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Consenti controllo tastiera';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste è pronto.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Quick Paste sarà pronto dopo l\'attivazione dell\'integrazione desktop richiesta.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'Apri CopyPaste quando esegui il login.';

  @override
  String get onboardingLinuxLinks => 'Link di CopyPaste';

  @override
  String get onboardingLinuxLinksRegistered =>
      'I link di CopyPaste si aprono in questa app.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Registra i link di CopyPaste per gli inviti di abbinamento.';

  @override
  String get onboardingLinuxRegisterLinks => 'Registra link';

  @override
  String get onboardingLinuxScreenshotBlocking => 'Blocco degli screenshot';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Il blocco degli screenshot non è disponibile su Linux. I dati archiviati protetti restano crittografati.';

  @override
  String get onboardingLinuxCheckAgain => 'Verifica di nuovo';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste non è riuscito a leggere lo stato dell\'integrazione con Linux.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Impossibile aprire la configurazione dell\'integrazione desktop.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'L\'autorizzazione al controllo della tastiera non è stata concessa.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Impossibile aggiornare l\'apertura al login.';

  @override
  String get onboardingLinuxLinksFailed =>
      'Impossibile registrare i link di CopyPaste.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Impossibile aggiornare lo stato dell\'integrazione con Linux.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'L\'accesso agli appunti non è disponibile in questa sessione X11.';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste richiede una sessione desktop X11 o Wayland.';

  @override
  String get onboardingAndroidFullCapture => 'Acquisizione completa';

  @override
  String get onboardingAndroidBackgroundCapture => 'Acquisizione in background';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Configura una volta, poi verifica con una copia reale.';

  @override
  String get onboardingAndroidLimitedCapture => 'Acquisizione limitata';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'Condividi con CopyPaste o torna qui dopo aver copiato.';

  @override
  String get onboardingAndroidNotification => 'Notifica di acquisizione';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Necessaria durante l\'acquisizione';

  @override
  String get onboardingAndroidBackgroundActivity => 'Attività in background';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Consigliata per non perdere acquisizioni';

  @override
  String get onboardingAndroidOpen => 'Apri';

  @override
  String get onboardingAndroidApplyAccess => 'Applica accesso di acquisizione';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Applica l\'accesso di acquisizione una tantum.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Puoi configurare l\'Acquisizione completa più tardi.';

  @override
  String get onboardingAndroidCaptureWorking =>
      'L\'acquisizione in background funziona';

  @override
  String get onboardingAndroidVerifyCopy => 'Verifica con una copia reale';

  @override
  String get onboardingAndroidCopyReached =>
      'Una nuova copia è arrivata nella Cronologia locale.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Copia qualcosa in un\'altra app, poi torna qui.';

  @override
  String get onboardingAndroidStartCapture => 'Avvia acquisizione';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste non è riuscito a leggere lo stato di configurazione di Android.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Impossibile aggiornare lo stato di acquisizione di Android.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Il monitoraggio della configurazione di Android non è disponibile.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'Impossibile attivare la ricezione degli appunti.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Impossibile interrompere l\'acquisizione in background.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Impossibile aprire le impostazioni della batteria.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste è consentito in Shizuku, ma non è stato possibile applicare le autorizzazioni di acquisizione di Android.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Consenti CopyPaste in Shizuku per applicare le autorizzazioni di acquisizione.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Prima concedi l\'accesso agli appunti in background.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Consenti le notifiche prima di avviare l\'acquisizione in background.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Impossibile avviare l\'acquisizione in background.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Impossibile aprire la configurazione dell\'acquisizione di Android.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Impossibile salvare la scelta di configurazione di Android.';

  @override
  String get modulesMarketplaceTab => 'Marketplace';

  @override
  String get modulesInstalledTab => 'Installati';

  @override
  String get modulesRefresh => 'Aggiorna';

  @override
  String get modulesSearchPlaceholder => 'Cerca moduli';

  @override
  String get modulesOperationFailed => 'Operazione sul modulo non riuscita';

  @override
  String get modulesLoading => 'Caricamento dei moduli.';

  @override
  String get modulesUnavailableTitle => 'Moduli non disponibili';

  @override
  String get modulesUnavailableMessage => 'Prova a caricare di nuovo i moduli.';

  @override
  String get modulesNoneInstalled => 'Nessun modulo installato';

  @override
  String get modulesNoMatches => 'Nessun modulo corrispondente';

  @override
  String get modulesChooseFromMarketplace =>
      'Scegli un modulo dal Marketplace.';

  @override
  String get modulesTryDifferentSearch => 'Prova con un\'altra ricerca.';

  @override
  String get modulesMarketplaceLoading => 'Caricamento del Marketplace.';

  @override
  String get modulesMarketplaceUnavailableTitle =>
      'Marketplace non disponibile';

  @override
  String get modulesMarketplaceUnavailableMessage =>
      'Prova a caricare di nuovo il Marketplace.';

  @override
  String get modulesNoneAvailable => 'Nessun modulo disponibile';

  @override
  String get modulesNoneAvailableMessage =>
      'I moduli verranno visualizzati qui quando saranno pubblicati.';

  @override
  String modulesEnableModule(String module) {
    return 'Attiva $module';
  }

  @override
  String get modulesInstall => 'Installa';

  @override
  String modulesUpdateTo(String version) {
    return 'Aggiorna alla versione $version';
  }

  @override
  String get modulesRemoveModule => 'Rimuovi modulo';

  @override
  String get modulesModuleSettings => 'Impostazioni del modulo';

  @override
  String modulesSettingsTitle(String module) {
    return 'Impostazioni di $module';
  }

  @override
  String get modulesDownloadingModel => 'Download del modello linguistico…';

  @override
  String get modulesInstalling => 'Installazione…';

  @override
  String modulesDownloadingProgress(int percent) {
    return 'Download: $percent%';
  }

  @override
  String get modulesRun => 'Esegui';

  @override
  String modulesRemoveTitle(String module) {
    return 'Rimuovere $module?';
  }

  @override
  String get modulesRemoveMessage =>
      'Il modulo e le sue impostazioni locali verranno rimossi.';

  @override
  String get modulesRemove => 'Rimuovi';

  @override
  String get modulesRemoveFailed => 'Impossibile rimuovere il modulo.';

  @override
  String get modulesCloseSettings => 'Chiudi impostazioni del modulo';

  @override
  String get modulesRemoved => 'Modulo rimosso';

  @override
  String get modulesNoSettingsTitle => 'Nessuna impostazione aggiuntiva';

  @override
  String get modulesNoSettingsMessage =>
      'Questo modulo non richiede configurazioni aggiuntive.';

  @override
  String get modulesSetUpSmsAccess => 'Configura accesso agli SMS';

  @override
  String get modulesRestartApp => 'Riavvia CopyPaste';

  @override
  String get modulesSmsAccessTitle => 'Accesso agli SMS';

  @override
  String get modulesCloseSmsAccess => 'Chiudi accesso agli SMS';

  @override
  String get modulesSmsChecking => 'Verifica dell\'accesso agli SMS.';

  @override
  String get modulesSmsReady =>
      'L\'accesso è pronto. Attiva SMS Codes per iniziare a copiare i nuovi codici.';

  @override
  String get modulesSmsAllowAccess =>
      'Consenti l\'accesso per copiare i codici dai nuovi SMS.';

  @override
  String get modulesSmsNotification => 'Notifica SMS';

  @override
  String get modulesSmsNotificationDescription =>
      'Necessaria durante l\'esecuzione di SMS Codes';

  @override
  String get modulesSmsApplyAccess => 'Applica accesso agli SMS';

  @override
  String get modulesSmsApplyAccessDescription =>
      'Applica l\'accesso agli SMS una tantum.';

  @override
  String get modulesSmsSetupFailed => 'Configurazione SMS non riuscita';

  @override
  String get modulesFileSelectionFailed => 'Selezione del file non riuscita';

  @override
  String get modulesChooseLanguages => 'Scegli le lingue';

  @override
  String get modulesChooseFile => 'Scegli file';

  @override
  String modulesModelReady(String model) {
    return 'Il modello $model è pronto';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return 'Modello $model · download di $size MiB';
  }

  @override
  String get modulesPlatformsUnknown => 'Piattaforme: sconosciute';

  @override
  String modulesPlatforms(String platforms) {
    return 'Piattaforme: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'La firma del catalogo dei moduli non è valida.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Impossibile connettersi al Marketplace dei moduli.';

  @override
  String get modulesMarketplaceTimeout =>
      'Il Marketplace dei moduli non ha risposto.';

  @override
  String get modulesIncompatible =>
      'Questo modulo non è compatibile con questo dispositivo.';

  @override
  String get modulesPackageSizeInvalid =>
      'La dimensione del pacchetto del modulo non è valida.';

  @override
  String get modulesDownloadInterrupted =>
      'Il download del modulo è stato interrotto.';

  @override
  String get modulesDownloadTimeout =>
      'Il download del modulo non ha risposto.';

  @override
  String get modulesPackageUrlUntrusted =>
      'L\'URL del pacchetto del modulo non è attendibile.';

  @override
  String get modulesDownloadLeftGithub =>
      'Il download del modulo è stato reindirizzato fuori da GitHub.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'Il download del modulo ha usato troppi reindirizzamenti.';

  @override
  String get modulesDownloadTooLarge =>
      'Il download del modulo è troppo grande.';

  @override
  String get modulesPackageIntegrityFailed =>
      'Il pacchetto del modulo non ha superato il controllo di integrità.';

  @override
  String get modulesDownloadUnavailable =>
      'Il download di questo modulo non è al momento disponibile.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'Il Marketplace dei moduli non è disponibile. Riprova.';

  @override
  String get modulesCatalogInvalid => 'Il catalogo dei moduli non è valido.';

  @override
  String get modulesThisSystem => 'questo sistema';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version o versioni successive';
  }

  @override
  String get modulesNotAvailableForDevice =>
      'Non disponibile per questo dispositivo.';

  @override
  String get modulesAppVersionUnverified =>
      'Impossibile verificare la versione di CopyPaste installata.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Questo modulo non supporta CopyPaste $version.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Richiede $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed =>
      'Impossibile caricare il Marketplace dei moduli.';

  @override
  String get modulesSmsSetupRequired =>
      'Configura l\'accesso agli SMS prima di attivare questo modulo.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'Impossibile avviare il monitoraggio degli SMS. Il modulo è ancora attivo; disattivalo di nuovo per riprovare.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'Impossibile avviare il monitoraggio degli SMS. Il modulo è stato disattivato.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'Impossibile avviare il monitoraggio degli SMS. Non è stato possibile confermare lo stato di attivazione del modulo.';

  @override
  String get modulesSmsMonitoringFailed =>
      'Impossibile avviare il monitoraggio degli SMS.';

  @override
  String get modulesSmsAndroidOnly =>
      'L\'accesso agli SMS è disponibile solo su Android.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'Il monitoraggio degli SMS richiede la configurazione.';

  @override
  String get modulesOperationIncomplete =>
      'Impossibile completare l\'operazione sul modulo.';

  @override
  String get modulesUnsupportedEvent =>
      'Il modulo usa un evento non supportato.';

  @override
  String get modulesUnsupportedResult =>
      'Il modulo ha restituito un risultato non supportato.';

  @override
  String get modulesUnsupportedField =>
      'Il modulo usa un campo non supportato.';

  @override
  String get modulesResponseInvalid => 'La risposta del modulo non è valida.';

  @override
  String get modulesFileSelectionUnavailable =>
      'La selezione dei file non è disponibile.';

  @override
  String get modulesFileSelectFailed => 'Impossibile selezionare il file.';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field non è un file accettato.';
  }

  @override
  String get modulesSmsVerifyFailed =>
      'Impossibile verificare l\'accesso agli SMS.';

  @override
  String get modulesSmsAllowNotifications =>
      'Consenti le notifiche per mantenere attivo SMS Codes in background.';

  @override
  String get modulesSmsNotGranted =>
      'L\'accesso agli SMS non è stato concesso.';

  @override
  String get modulesSmsConfigureFailed =>
      'Impossibile configurare l\'accesso agli SMS.';

  @override
  String get onboardingLinuxContinueBasic =>
      'Continua con l\'acquisizione di base';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Facoltativa. Necessaria per Quick Paste e per le esclusioni delle app di origine.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Facoltativo. Necessario affinché Quick Paste possa tornare all\'app precedente.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'L\'accesso agli appunti non è disponibile in questa sessione Wayland. Attiva l\'estensione firmata per GNOME o KDE, poi verifica di nuovo.';

  @override
  String get historyMoreActions => 'Altre azioni';

  @override
  String get historyTagsAction => 'Tag…';
}
