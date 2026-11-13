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
  String pasteQueuePasted(int position, int total) {
    return 'Incollato $position di $total';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'L\'elemento $position di $total è negli appunti';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementi incollati',
      one: '1 elemento incollato',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementi sono negli appunti',
      one: '1 elemento è negli appunti',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Coda completata';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Aggiunto, $total in coda',
      one: 'Aggiunto, 1 in coda',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'Coda vuota, nulla incollato';

  @override
  String get pasteQueueMixed =>
      'Incolla tutto funziona con code di solo testo o solo file';

  @override
  String get pasteQueueSeveralFiles =>
      'Non è ancora possibile incollare più file insieme. Incollali uno alla volta.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementi non sono più disponibili',
      one: '1 elemento non è più disponibile',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => 'Incolla successivo copia soltanto';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Consenti Accessibilità per incollare automaticamente. Nel frattempo l\'elemento è negli appunti.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Impossibile incollare automaticamente. L\'elemento è negli appunti; incollalo con la scorciatoia abituale.';

  @override
  String get pasteQueueTargetElevated =>
      'Impossibile incollare in un\'app eseguita come amministratore. L\'elemento resta in coda ed è negli appunti.';

  @override
  String get pasteQueueNoTarget =>
      'Nessuna app in cui incollare. L\'elemento resta in coda ed è negli appunti.';

  @override
  String get pasteQueueCollectingStarted => 'Raccolta delle copie in corso';

  @override
  String get pasteQueueCollectingStopped => 'Raccolta interrotta';

  @override
  String get pasteQueueFailed =>
      'Impossibile completare l\'azione di Paste Queue';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count in attesa',
      one: '1 in attesa',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count in attesa · Raccolta',
      one: '1 in attesa · Raccolta',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Raccolta';

  @override
  String get pasteQueueFinishedSummary => 'Completata';

  @override
  String pasteQueueFull(int max) {
    return 'Coda piena ($max)';
  }

  @override
  String get pasteQueueFullMessage =>
      'Le nuove copie non vengono più aggiunte. Non è stato rimosso nulla.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue è vuota';

  @override
  String get pasteQueueEmptyMessage =>
      'Raccogli le copie per incollarle in ordine, oppure aggiungi elementi dalla cronologia.';

  @override
  String get pasteQueueCollectingEmptyTitle => 'Copia elementi per aggiungerli';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Ogni copia entra in coda finché non interrompi la raccolta.';

  @override
  String get pasteQueueStartCollecting => 'Avvia raccolta';

  @override
  String get pasteQueueStopCollecting => 'Interrompi raccolta';

  @override
  String get pasteQueueLoading => 'Caricamento della coda…';

  @override
  String get pasteQueueLoadFailed => 'Impossibile caricare la coda';

  @override
  String get pasteQueuePasteNext => 'Incolla successivo';

  @override
  String get pasteQueueCopyNext => 'Copia successivo';

  @override
  String get pasteQueuePasteAll => 'Incolla tutto';

  @override
  String get pasteQueueCopyAll => 'Copia tutto';

  @override
  String get pasteQueueReverse => 'Inverti ordine';

  @override
  String get pasteQueueStepBack => 'Torna indietro';

  @override
  String get pasteQueueSkip => 'Salta';

  @override
  String get pasteQueueRemovePasted => 'Rimuovi incollati';

  @override
  String get pasteQueueCleared => 'Coda svuotata';

  @override
  String get pasteQueueUndo => 'Annulla';

  @override
  String get pasteQueueStartFresh => 'Ricomincia da capo';

  @override
  String get pasteQueueAppend => 'Aggiungi in coda';

  @override
  String get pasteQueueRestartTitle => 'Avviare una nuova raccolta?';

  @override
  String get pasteQueueRestartMessage =>
      'Ricomincia da capo rimuove gli elementi incollati. Aggiungi in coda li mantiene.';

  @override
  String pasteQueueStale(String time) {
    return 'Coda del $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Incollato';

  @override
  String get pasteQueueStatusSkipped => 'Saltato';

  @override
  String get pasteQueueStatusUnavailable => 'Non più disponibile';

  @override
  String get pasteQueueEdited => 'Modificato';

  @override
  String get pasteQueueSecret => 'Segreto';

  @override
  String get pasteQueueKindImage => 'Immagine';

  @override
  String get pasteQueueKindFiles => 'File';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Generato alle $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Elemento $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'Spostato in posizione $position';
  }

  @override
  String get pasteQueueMoveUp => 'Sposta su';

  @override
  String get pasteQueueMoveDown => 'Sposta giù';

  @override
  String get pasteQueueMoveToTop => 'Sposta in cima';

  @override
  String get pasteQueueMoveToBottom => 'Sposta in fondo';

  @override
  String get pasteQueueReorder => 'Trascina per riordinare';

  @override
  String get pasteQueueMoreActions => 'Altre azioni';

  @override
  String get pasteQueuePasteNow => 'Incolla ora';

  @override
  String get pasteQueueEdit => 'Modifica testo';

  @override
  String get pasteQueueUseOriginal => 'Usa il testo originale';

  @override
  String get pasteQueueRemove => 'Rimuovi dalla coda';

  @override
  String get pasteQueueEditTitle => 'Modifica il testo in coda';

  @override
  String get pasteQueueEditMessage =>
      'Le modifiche valgono solo per questa coda. La cronologia mantiene l\'originale.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Imposta scorciatoie';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Scegli le scorciatoie per raccogliere e incollare senza aprire CopyPaste.';

  @override
  String get pasteQueueShortcutToggleCollecting =>
      'Avvia o interrompi la raccolta';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Incolla successivo come testo semplice';

  @override
  String get pasteQueueShortcutDescription =>
      'Una scorciatoia globale che funziona in qualsiasi app.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Un\'altra scorciatoia di CopyPaste usa questa combinazione. Resta quella precedente.';

  @override
  String get pasteQueueShortcutInUse =>
      'Un\'altra app usa questa combinazione. Resta quella precedente; scegline un\'altra.';

  @override
  String get pasteQueueShortcutRejected =>
      'Il sistema ha rifiutato questa combinazione. Resta quella precedente.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt digita caratteri in molti layout di tastiera (AltGr). Scegli un\'altra combinazione.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Le scorciatoie globali non sono disponibili in questa sessione. Usa la coda dalla finestra o da Quick Paste.';

  @override
  String get pasteQueueShortcutFailed =>
      'Impossibile impostare questa scorciatoia.';

  @override
  String get pasteQueueEnableTitle => 'Attiva Paste Queue';

  @override
  String get pasteQueueEnableDescription =>
      'Se disattivata, le scorciatoie della coda vengono rilasciate e non si raccoglie nulla.';

  @override
  String get pasteQueuePasteNextStyleTitle => 'Incolla successivo esegue';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Incollare nell\'app in primo piano richiede il permesso di inviare tasti. Solo appunti lascia a te l\'incolla.';

  @override
  String get pasteQueuePasteIntoApp => 'Incolla nell\'app in primo piano';

  @override
  String get pasteQueueClipboardOnly => 'Solo negli appunti';

  @override
  String get pasteQueueInsertTitle => 'I nuovi elementi vanno';

  @override
  String get pasteQueueInsertDescription =>
      'Dove entrano in coda gli elementi raccolti e aggiunti.';

  @override
  String get pasteQueueInsertEnd => 'In fondo (primo entrato, primo uscito)';

  @override
  String get pasteQueueInsertStart => 'In cima (ultimo entrato, primo uscito)';

  @override
  String get pasteQueueEndBehaviorTitle => 'Dopo l\'ultimo elemento';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'Cosa fa Incolla successivo quando tutti gli elementi sono stati incollati.';

  @override
  String get pasteQueueEndStop => 'Fermati';

  @override
  String get pasteQueueEndLoop => 'Ricomincia';

  @override
  String get pasteQueueEndKeepLast => 'Continua a incollare l\'ultimo';

  @override
  String get pasteQueueSeparatorTitle => 'Separatore di Incolla tutto';

  @override
  String get pasteQueueSeparatorDescription =>
      'Inserito tra gli elementi quando li incolli tutti insieme.';

  @override
  String get pasteQueueSeparatorNewLine => 'Nuova riga';

  @override
  String get pasteQueueSeparatorBlankLine => 'Riga vuota';

  @override
  String get pasteQueueSeparatorTab => 'Tabulazione';

  @override
  String get pasteQueueSeparatorSpace => 'Spazio';

  @override
  String get pasteQueueSeparatorComma => 'Virgola';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Virgola e spazio';

  @override
  String get pasteQueueSeparatorNone => 'Nessuno';

  @override
  String get pasteQueueSeparatorCustom => 'Personalizzato';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'Fino a 32 caratteri; \\n e \\t consentiti';

  @override
  String get pasteQueuePlainTextTitle => 'Incolla sempre come testo semplice';

  @override
  String get pasteQueuePlainTextDescription =>
      'Rimuove la formattazione dagli elementi in coda.';

  @override
  String get pasteQueueIdleStopTitle => 'Interrompi la raccolta dopo';

  @override
  String get pasteQueueIdleStopDescription =>
      'La raccolta si interrompe se non copi nulla per questo tempo. Gli elementi restano.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes min';
  }

  @override
  String get pasteQueueIdleNever => 'Mai';

  @override
  String get pasteQueueMaxEntriesTitle => 'Limite della coda';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Quando è piena, non si aggiungono altri elementi. Nulla viene scartato.';

  @override
  String get pasteQueueRestoreTitle =>
      'Ripristina gli appunti precedenti alla fine';

  @override
  String get pasteQueueRestoreDescription =>
      'Rimette ciò che avevi copiato prima che la coda iniziasse a incollare.';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue coda raccogliere ordine scorciatoia separatore';

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

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'sbloccare gli elementi segreti',
      'other': 'Sblocca gli elementi segreti',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'usare uno snippet nascosto',
      'other': 'Usa uno snippet nascosto',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'copiare un elemento segreto',
      'other': 'Copia un elemento segreto',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'Non disponibile su questo sistema';

  @override
  String get deviceAuthNotConfigured =>
      'Configura prima una password, un PIN o la biometria del dispositivo';

  @override
  String get commonOn => 'Attivato';

  @override
  String get commonOff => 'Disattivato';

  @override
  String get settingsQuickPasteStartFailed =>
      'Impossibile avviare Quick Paste, quindi la sua scorciatoia non funziona.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut è usata da un\'altra app. Scegli un\'altra scorciatoia.';
  }

  @override
  String get devicesEmptyTitle => 'Nessun altro dispositivo abbinato';

  @override
  String get devicesEmptyMessage =>
      'Abbina un altro dispositivo per mantenere sincronizzata la cronologia degli appunti tra i due. Entrambi i dispositivi devono essere sulla stessa rete e ogni abbinamento viene confermato su entrambi.';

  @override
  String get pairingStateNotCompleted => 'Abbinamento non completato';

  @override
  String get pairingStepInvite => 'Invito';

  @override
  String get pairingStepScan => 'Scansione';

  @override
  String get pairingStepCode => 'Codice';

  @override
  String get pairingStepRequest => 'Richiesta';

  @override
  String get pairingStepConnect => 'Connessione';

  @override
  String get pairingStepConfirm => 'Conferma';

  @override
  String get pairingStepPaired => 'Abbinato';

  @override
  String get pairingOtherDevice => 'Altro dispositivo';

  @override
  String get pairingDecisionNotConfirmed => 'Non ancora confermato';

  @override
  String get pairingDecisionSending => 'Invio della decisione…';

  @override
  String get pairingDecisionConfirmed => 'Confermato';

  @override
  String get pairingDecisionRejected => 'Rifiutato';

  @override
  String get pairingDecisionWaiting => 'In attesa di conferma';

  @override
  String get pairingDecisionUnknown => 'Risposta non nota';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dispositivi',
      many: '$count dispositivi',
      one: '$count dispositivo',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count righe',
      many: '$count righe',
      one: '$count riga',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Navigazione principale';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Passaggio $step di $count: $name';
  }

  @override
  String get modulesPhaseDownload => 'Download';

  @override
  String get modulesPhaseInstall => 'Installazione';

  @override
  String modulesInstallingModule(String module) {
    return 'Installazione di $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return 'Aggiornamento di $module';
  }

  @override
  String get commonDismiss => 'Ignora';

  @override
  String get settingsRetentionConfirmTitle =>
      'Rimuovere la cronologia meno recente?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Nuovo limite: $limit. Gli elementi non fissati meno recenti vengono rimossi ora da questo dispositivo e non possono essere recuperati. Gli elementi fissati vengono conservati.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle =>
      'Ridurre la quota di archiviazione?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Nuova quota: $size. Se la cronologia non fissata occupa più spazio, gli elementi meno recenti vengono rimossi ora e non possono essere recuperati. Gli elementi fissati vengono conservati.';
  }

  @override
  String get settingsLimitConfirmAction => 'Applica limite';

  @override
  String get settingsHideEverywhereTitle =>
      'Nascondere completamente CopyPaste?';

  @override
  String get settingsHideEverywhereMessage =>
      'Con entrambe le opzioni disattivate, CopyPaste non ha alcuna icona visibile. Per riaprirlo, avvia di nuovo CopyPaste.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Con entrambe le opzioni disattivate, CopyPaste non ha alcuna icona visibile. Per riaprirlo, premi $shortcut o avvia di nuovo CopyPaste.';
  }

  @override
  String get settingsHideEverywhereAction => 'Nascondi';

  @override
  String get settingsExportDoneTitle => 'Cronologia dei testi esportata';

  @override
  String get settingsBackupDoneTitle => 'Backup creato';

  @override
  String get settingsRestoreDoneTitle => 'Cronologia ripristinata';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementi di testo esportati',
      many: '$count elementi di testo esportati',
      one: '$count elemento di testo esportato',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count non leggibili',
      many: '$count non leggibili',
      one: '$count non leggibile',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Backup crittografato salvato · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'La cronologia degli appunti di questo dispositivo ora corrisponde al backup.';

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
    return '$used di $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return '$used usati su $quota';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '$used usati';
  }

  @override
  String get storageUsageOverQuota => 'Quota superata';

  @override
  String storageUsageOldestClip(String date) {
    return 'Elemento meno recente: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Archiviazione: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Nuova quota: $size. La cronologia non fissata occupa $used, quindi i suoi elementi meno recenti vengono rimossi ora finché non rientra. Non possono essere recuperati. Gli elementi fissati vengono conservati.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementi',
      many: '$count elementi',
      one: '$count elemento',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'Apri CopyPaste';

  @override
  String get trayOpenQuickPaste => 'Apri Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Apri Quick Paste ($shortcut)';
  }

  @override
  String get trayNewSnippet => 'Nuovo frammento';

  @override
  String get trayRecent => 'Recenti';

  @override
  String get trayRecentLoading => 'Caricamento…';

  @override
  String get trayRecentFailed => 'Impossibile caricare i clip';

  @override
  String get trayRecentEmpty => 'Ancora nessun clip';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clip recenti nascosti',
      one: '1 clip recente nascosto',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Segreto';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Copia l\'ultimo da un altro dispositivo';

  @override
  String get trayCaptureNow => 'Acquisisci ora';

  @override
  String trayPausedUntil(String time) {
    return 'In pausa fino alle $time';
  }

  @override
  String get trayPauseFor => 'Sospendi per…';

  @override
  String get trayPauseFor15Minutes => '15 minuti';

  @override
  String get trayPauseForOneHour => '1 ora';

  @override
  String get trayPauseUntilResumed => 'Finché non riprendo';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count elementi',
      one: 'Paste Queue: 1 elemento',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Raccolta: $count elementi',
      one: 'Raccolta: 1 elemento',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Saltati di recente ($count)';
  }

  @override
  String get trayOpenContentRules => 'Apri le regole sui contenuti';

  @override
  String get trayRecentSettingsTitle => 'Barra dei menu e area di notifica';

  @override
  String get trayRecentSettingsDescription =>
      'Cosa mostra il menu della barra dei menu o dell\'area di notifica in Recenti.';

  @override
  String get trayRecentCountTitle => 'Clip recenti';

  @override
  String get trayRecentCountDescription =>
      'Quanti dei clip più recenti mostra il menu.';

  @override
  String get trayRecentPreviewTitle => 'Anteprima dei clip recenti';

  @override
  String get trayRecentPreviewDescription =>
      'Il sistema conserva il testo dei menu fuori dalla cifratura di CopyPaste. Mostra meno per tenerne fuori il contenuto dei clip. I segreti sono sempre mascherati.';

  @override
  String get trayRecentPreviewFull => 'Anteprima completa';

  @override
  String get trayRecentPreviewTitles => 'Solo tipo e origine';

  @override
  String get trayRecentPreviewHidden => 'Nascosto';

  @override
  String get trayRecentSettingsKeywords =>
      'area di notifica barra dei menu recenti anteprima privacy numero';

  @override
  String get transformsPaletteTitle => 'Trasforma testo';

  @override
  String get transformsFilterPlaceholder => 'Filtra trasformazioni';

  @override
  String get transformsNoMatches => 'Nessuna trasformazione corrispondente';

  @override
  String get transformsGroupResults => 'Risultati';

  @override
  String get transformsGroupSuggested => 'Suggerite';

  @override
  String get transformsGroupRecent => 'Recenti';

  @override
  String get transformsGroupFavorites => 'Preferite';

  @override
  String get transformsGroupPipelines => 'Pipeline';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV e XML';

  @override
  String get transformsCategoryEncode => 'Codifica e decodifica';

  @override
  String get transformsCategoryCase => 'Maiuscole e minuscole';

  @override
  String get transformsCategoryHash => 'Hash';

  @override
  String get transformsCategoryLines => 'Righe e spazi';

  @override
  String get transformsCategoryClean => 'Pulizia';

  @override
  String get transformsCategoryUrl => 'Link';

  @override
  String get transformsCategoryMarkup => 'Testo formattato e Markdown';

  @override
  String get transformsCategoryDev => 'Sviluppatori';

  @override
  String get transformsCategoryOther => 'Altro';

  @override
  String get transformsChainEmpty => 'Ancora nessun passaggio';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Poi: $name';
  }

  @override
  String get transformsStepEnabled => 'Esegui questo passaggio';

  @override
  String get transformsMoveEarlier => 'Sposta su';

  @override
  String get transformsMoveLater => 'Sposta giù';

  @override
  String get transformsRemoveStep => 'Rimuovi passaggio';

  @override
  String get transformsSavePipeline => 'Salva come pipeline';

  @override
  String get transformsSavePipelineTitle => 'Salva pipeline';

  @override
  String get transformsSavePipelineFailed => 'Impossibile salvare la pipeline.';

  @override
  String get transformsPipelineNameLabel => 'Nome';

  @override
  String get transformsPipelineDescriptionLabel => 'Descrizione (facoltativa)';

  @override
  String get transformsPipelineStepsLabel => 'Passaggi';

  @override
  String get transformsPipelineNoSteps => 'Aggiungi almeno un passaggio.';

  @override
  String get transformsPipelineNameRequired => 'Inserisci un nome.';

  @override
  String get transformsPipelineNameTooLong => 'Usa al massimo 80 caratteri.';

  @override
  String get transformsPipelineNameTaken =>
      'Un\'altra pipeline ha già questo nome.';

  @override
  String get transformsPipelineSavedTitle => 'Pipeline salvata';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name è disponibile in ogni palette di trasformazione.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Passaggi: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Pipeline eliminata';

  @override
  String get transformsNeedsNewerVersion =>
      'Richiede una versione più recente di CopyPaste';

  @override
  String get transformsLoadFailed => 'Impossibile caricare le trasformazioni.';

  @override
  String get transformsNotTextTitle => 'Le trasformazioni funzionano sul testo';

  @override
  String get transformsNotTextMessage =>
      'Questo elemento è un\'immagine, un file o un colore. Scegli un elemento di testo o copia del testo.';

  @override
  String get transformsNothingTitle => 'Niente da trasformare';

  @override
  String get transformsNothingMessage =>
      'Copia del testo o scegli un elemento della cronologia.';

  @override
  String get transformsPickTransform => 'Scegli una trasformazione';

  @override
  String get transformsPickTransformHint =>
      'Tab la aggiunge come passaggio. Invio applica il risultato.';

  @override
  String get transformsPreviewing => 'Aggiornamento anteprima…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Righe: $lines · Parole: $words · Caratteri: $chars';
  }

  @override
  String get transformsNoChanges => 'Nessuna modifica';

  @override
  String transformsTruncated(String size) {
    return 'Visualizzati i primi 64 KiB di $size';
  }

  @override
  String get transformsRandom => 'Casuale';

  @override
  String get transformsNotRun => 'Non eseguito';

  @override
  String get transformsSensitiveHidden =>
      'Il risultato è riservato e nascosto.';

  @override
  String get transformsReveal => 'Mostra';

  @override
  String get transformsPaneOutput => 'Output';

  @override
  String get transformsPaneInput => 'Input';

  @override
  String get transformsPaneChanges => 'Modifiche';

  @override
  String get transformsCannotTransform =>
      'Impossibile trasformare questo testo';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Passaggio $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Mostra in esadecimale';

  @override
  String get transformsActionPaste => 'Incolla';

  @override
  String get transformsActionSave => 'Salva come nuovo elemento';

  @override
  String get transformsCopiedMessage =>
      'Il risultato è negli appunti e nella cronologia.';

  @override
  String get transformsSavedTitle => 'Salvato';

  @override
  String get transformsSavedMessage =>
      'Il risultato è un nuovo elemento della cronologia. Gli appunti non cambiano.';

  @override
  String get transformsPasteFailedTitle => 'Impossibile incollare';

  @override
  String get transformsPasteFailedMessage =>
      'Il risultato è negli appunti. Incollalo manualmente.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message alla riga $line, colonna $column';
  }

  @override
  String get transformsErrorBinary =>
      'I dati decodificati sono binari, non testo';

  @override
  String get transformsErrorTooLarge => 'Troppo grande da trasformare';

  @override
  String get transformsErrorInvalidParam =>
      'Un\'impostazione di questo passaggio non è valida';

  @override
  String get transformsErrorCancelled => 'Annullato';

  @override
  String get transformsErrorTooExpensive => 'Schema troppo oneroso';

  @override
  String get transformsErrorPattern => 'Lo schema non è valido';

  @override
  String get transformsErrorTimestampUnit =>
      'Scegli se il numero è in secondi o millisecondi';

  @override
  String get transformsErrorExternalEntity =>
      'Le entità esterne non sono consentite';

  @override
  String get transformsErrorJwtEncrypted =>
      'I token cifrati non possono essere decodificati';

  @override
  String get transformsErrorJson => 'JSON non valido';

  @override
  String get transformsErrorYaml => 'YAML non valido';

  @override
  String get transformsErrorXml => 'XML non valido';

  @override
  String get transformsErrorCsv => 'CSV non valido';

  @override
  String get transformsErrorBase64 => 'Base64 non valido';

  @override
  String get transformsErrorHex => 'Esadecimale non valido';

  @override
  String get transformsErrorEscape => 'Sequenza di escape non valida';

  @override
  String get transformsErrorUrl => 'Codifica percentuale non valida';

  @override
  String get transformsErrorJwt => 'Non è un JWT valido';

  @override
  String get transformsErrorDate => 'Data o timestamp non riconosciuti';

  @override
  String get transformsErrorGeneric =>
      'Questo passaggio non può trasformare il suo input';

  @override
  String get transformsWarningSignedLink => 'Link firmato lasciato invariato';

  @override
  String get transformsWarningExpired => 'Scaduto';

  @override
  String get transformsWarningNotYetValid => 'Non ancora valido';

  @override
  String get transformsWarningSignatureNotVerified => 'Firma non verificata';

  @override
  String get transformsWarningDuplicateKeys => 'Chiavi duplicate';

  @override
  String get transformsWarningLenientJson => 'JSON non standard accettato';

  @override
  String get transformsWarningDoubleEncoded =>
      'Il JSON era codificato due volte';

  @override
  String get transformsWarningEscapes =>
      'Sequenze di escape sconosciute mantenute come scritte';

  @override
  String get transformsWarningLocalZone => 'Fuso orario locale presunto';

  @override
  String get transformsWarningNoMatches => 'Nessuna corrispondenza';

  @override
  String get transformsWarningGeneric => 'Controlla il risultato';

  @override
  String get transformsRestoreTitle => 'Ripristina gli appunti dopo l\'incolla';

  @override
  String get transformsRestoreDescription =>
      'Dopo un incolla trasformato, tornano gli appunti precedenti, a meno che nel frattempo non sia stato copiato altro.';

  @override
  String get transformsDefaultTitle =>
      'Trasformazione predefinita all\'incolla';

  @override
  String get transformsDefaultDescription =>
      'Quick Paste applica questa pipeline al testo e la mostra prima di incollare. Se fallisce, viene incollato l\'originale.';

  @override
  String get transformsNoDefault => 'Nessuna';

  @override
  String get transformsAppDefaultsTitle => 'Predefiniti per app';

  @override
  String get transformsAppDefaultsDescription =>
      'Sostituisci il predefinito di un\'app tramite bundle ID, eseguibile o nome del pacchetto. Le voci di un\'altra piattaforma si sincronizzano ma qui non hanno effetto.';

  @override
  String get transformsAppIdentifierPlaceholder => 'Identificativo dell\'app';

  @override
  String transformsOtherPlatform(String platform) {
    return 'solo su $platform';
  }

  @override
  String get transformsAdd => 'Aggiungi';

  @override
  String get transformsRemove => 'Rimuovi';

  @override
  String get transformsFavoritesTitle => 'Preferite';

  @override
  String get transformsFavoritesDescription =>
      'Le preferite compaiono per prime in ogni palette. Le prime nove si eseguono con Mod+1–Mod+9.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Mod+$number nella palette';
  }

  @override
  String get transformsAddFavorite => 'Aggiungi preferita';

  @override
  String get transformsPipelinesTitle => 'Pipeline';

  @override
  String get transformsPipelinesDescription =>
      'Catene di trasformazioni salvate con le loro impostazioni. Si sincronizzano con gli altri dispositivi.';

  @override
  String get transformsNewPipeline => 'Nuova pipeline';

  @override
  String get transformsEditPipeline => 'Modifica pipeline';

  @override
  String get transformsDuplicatePipeline => 'Duplica';

  @override
  String get transformsDeletePipeline => 'Elimina';

  @override
  String transformsDeletePipelineTitle(String name) {
    return 'Eliminare $name?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Preferite e predefiniti che la usano tornano a nessuno su ogni dispositivo.';

  @override
  String get transformsDeletePipelineFailed =>
      'Impossibile eliminare la pipeline.';

  @override
  String get transformsAddStep => 'Aggiungi un passaggio';

  @override
  String get transformsHiddenTitle => 'Trasformazioni nascoste';

  @override
  String get transformsHiddenDescription =>
      'Le trasformazioni nascoste non compaiono in palette e suggerimenti. Le pipeline che le usano funzionano comunque.';

  @override
  String get transformsHideTransform => 'Nascondi una trasformazione';

  @override
  String get transformsShow => 'Mostra';

  @override
  String get transformsSuggestionsTitle => 'Suggerimenti';

  @override
  String get transformsSuggestionsDescription =>
      'All\'apertura di una palette, i primi 64 KiB del testo vengono controllati per i tipi seguenti per suggerire fino a tre trasformazioni. Nulla viene eseguito in background.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web Token';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl =>
      'Link con parametri di tracciamento';

  @override
  String get transformsDetectorTimestamp => 'Timestamp';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Parametri di tracciamento';

  @override
  String get transformsTrackingDescription =>
      'L\'elenco integrato è incluso nell\'app e cambia solo con gli aggiornamenti. Aggiungi nomi da rimuovere o mantenere; scrivi nome@dominio per limitarlo a un dominio.';

  @override
  String get transformsTrackingRemoveLabel => 'Rimuovi anche';

  @override
  String get transformsTrackingAllowLabel => 'Mantieni sempre';

  @override
  String get transformsTrackingPlaceholder => 'nome o nome@dominio, poi Invio';

  @override
  String get quickAccessUnavailableTitle =>
      'Stato dell\'accesso rapido non disponibile';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste non è riuscito a leggere lo stato di tastiera, riquadri e widget. Riprova.';

  @override
  String get quickAccessActionFailedTitle => 'Impossibile completare l\'azione';

  @override
  String get quickAccessActionFailedMessage =>
      'Android non ha aperto la schermata richiesta o non ha salvato la modifica. Riprova.';

  @override
  String get quickAccessKeyboardTitle => 'Tastiera degli elementi';

  @override
  String get quickAccessKeyboardDescription =>
      'Attiva Elementi CopyPaste nelle impostazioni della tastiera di sistema. Android avvisa che una tastiera può vedere ciò che digiti: il pannello degli elementi non registra mai i tasti premuti, non ha tasti di digitazione per altre app e inserisce solo gli elementi che tocchi.';

  @override
  String get quickAccessKeyboardOn => 'Attivata';

  @override
  String get quickAccessKeyboardOff => 'Disattivata';

  @override
  String get quickAccessOpenKeyboardSettings => 'Apri impostazioni tastiera';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'Attiva il pannello tastiera Elementi CopyPaste.';

  @override
  String get quickAccessKeyboardKeywords =>
      'tastiera metodo di inserimento ime pannello attivare';

  @override
  String get quickAccessSwitchKeyboardTitle =>
      'Passa alla tastiera degli elementi';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'Scegli Elementi CopyPaste come tastiera attuale. Finché resta selezionata, ogni copia viene salvata.';

  @override
  String get quickAccessSwitchKeyboardAction => 'Scegli tastiera';

  @override
  String get quickAccessKeyboardCurrent => 'Tastiera attuale';

  @override
  String get quickAccessCaptureTierTitle => 'Modalità di acquisizione';

  @override
  String get quickAccessCaptureTierDescription =>
      'Senza accesso tramite Shizuku o ADB, le copie vengono salvate solo quando si apre il pannello degli elementi, finché resta selezionato, oppure dall\'app CopyPaste, da un riquadro, dal widget o da Condividi.';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'Scopri come vengono salvate le copie dalle altre app.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'acquisizione livello completa pannello manuale shizuku raccogliere';

  @override
  String get quickAccessTierFull =>
      'Completa: ogni copia viene salvata in background';

  @override
  String get quickAccessTierPanelCollecting =>
      'Pannello selezionato: raccolta in corso';

  @override
  String get quickAccessTierPanelCatchUp =>
      'Modalità pannello: recupero all\'apertura';

  @override
  String get quickAccessTierManual =>
      'Manuale: salva dall\'app, da un riquadro, dal widget o da Condividi';

  @override
  String get quickAccessTierPaused => 'Acquisizione in pausa';

  @override
  String get quickAccessAutoReturnTitle =>
      'Torna alla tastiera precedente dopo l\'inserimento';

  @override
  String get quickAccessAutoReturnDescription =>
      'Torna indietro dopo un singolo inserimento. Il pannello resta aperto mentre usi la scheda Coda o le azioni di un elemento. Disattiva per continuare a raccogliere copie.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'tornare tastiera precedente automatico inserire';

  @override
  String get quickAccessFocusSearchTitle =>
      'Attiva la ricerca all\'apertura del pannello';

  @override
  String get quickAccessFocusSearchDescription =>
      'Apre il pannello sul campo di ricerca. Se disattivata, i tuoi elementi sono a un tocco di distanza.';

  @override
  String get quickAccessTilesTitle => 'Riquadri delle Impostazioni rapide';

  @override
  String get quickAccessTilesSearchDescription =>
      'Aggiungi i riquadri Salva appunti e Acquisizione degli appunti.';

  @override
  String get quickAccessTilesKeywords =>
      'impostazioni rapide riquadro tendina salvare pausa';

  @override
  String get quickAccessTileInstructions =>
      'Scorri due volte verso il basso dalla parte superiore dello schermo, tocca Modifica e trascina un riquadro di CopyPaste nelle Impostazioni rapide.';

  @override
  String get quickAccessSaveTileTitle => 'Riquadro Salva appunti';

  @override
  String get quickAccessSaveTileDescription =>
      'Salva il contenuto degli appunti dalle Impostazioni rapide.';

  @override
  String get quickAccessCaptureTileTitle =>
      'Riquadro Acquisizione degli appunti';

  @override
  String get quickAccessCaptureTileDescription =>
      'Mette in pausa e riprende l\'acquisizione dalle Impostazioni rapide.';

  @override
  String get quickAccessAddTile => 'Aggiungi riquadro';

  @override
  String get quickAccessTileAdded => 'Aggiunto alle Impostazioni rapide';

  @override
  String get quickAccessWidgetTitle => 'Widget Elementi recenti';

  @override
  String get quickAccessWidgetDescription =>
      'Mostra gli elementi più recenti nella schermata Home. Toccane uno per copiarlo.';

  @override
  String get quickAccessWidgetInstructions =>
      'Tocca e tieni premuta la schermata Home, tocca Widget e trascina Elementi recenti di CopyPaste.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count widget aggiunti',
      one: '1 widget aggiunto',
      zero: 'Non aggiunto',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Aggiungi widget';

  @override
  String get quickAccessWidgetKeywords =>
      'schermata home widget launcher recenti';

  @override
  String snippetsAbbreviationBadge(String abbreviation) {
    return 'Abbreviazione $abbreviation';
  }

  @override
  String get snippetsAbbreviationLabel => 'Abbreviazione';

  @override
  String get snippetsAbbreviationPlaceholder => 'Facoltativa, ad esempio firma';

  @override
  String snippetsAbbreviationTaken(String title) {
    return 'Già usata da «$title»';
  }

  @override
  String snippetsAbbreviationTooLong(int max) {
    return 'Usa al massimo $max caratteri.';
  }

  @override
  String get snippetsAbbreviationWhitespace =>
      'Un\'abbreviazione non può contenere spazi o caratteri di controllo.';

  @override
  String snippetsAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count frammenti da controllare dopo una sincronizzazione',
      one: '1 frammento da controllare dopo una sincronizzazione',
      zero: 'Nessun frammento da controllare',
    );
    return '$_temp0';
  }

  @override
  String get snippetsBodyLabel => 'Testo';

  @override
  String get snippetsBodyPlaceholder => 'Scrivi il testo del frammento';

  @override
  String get snippetsClearSelection => 'Annulla selezione';

  @override
  String get snippetsClipTooLarge =>
      'Troppo grande per essere modificato. Questo elemento si apre in sola lettura.';

  @override
  String snippetsCollectionAccent(String accent) {
    String _temp0 = intl.Intl.selectLogic(accent, {
      'neutral': 'Neutro',
      'blue': 'Blu',
      'green': 'Verde',
      'amber': 'Ambra',
      'red': 'Rosso',
      'other': 'Neutro',
    });
    return '$_temp0';
  }

  @override
  String get snippetsCollectionAccentLabel => 'Colore di accento';

  @override
  String get snippetsCollectionIcon => 'Icona';

  @override
  String get snippetsCollectionLabel => 'Raccolta';

  @override
  String get snippetsCollectionName => 'Nome';

  @override
  String get snippetsCollectionNameTaken =>
      'Esiste già una raccolta con questo nome.';

  @override
  String snippetsCollectionNameTooLong(int max) {
    return 'Usa al massimo $max caratteri.';
  }

  @override
  String get snippetsConcealedBody => 'Questo frammento è nascosto.';

  @override
  String get snippetsConcealedConfirmAction => 'Continua';

  @override
  String get snippetsConcealedConfirmMessage =>
      'Questo dispositivo non ha password, PIN o biometria per verificarti. Continua solo se è il tuo dispositivo.';

  @override
  String get snippetsConcealedConfirmTitle => 'Conferma che sei tu';

  @override
  String get snippetsConcealedPreview => 'Frammento nascosto';

  @override
  String get snippetsConcealedSwitch => 'Nascosto';

  @override
  String get snippetsConcealedTitle => 'Frammento nascosto';

  @override
  String get snippetsConflictBadge => 'Copia in conflitto';

  @override
  String get snippetsCopied => 'Copiato';

  @override
  String get snippetsCreate => 'Crea frammento';

  @override
  String snippetsCreateNamed(String query) {
    return 'Crea il frammento «$query»';
  }

  @override
  String get snippetsDeleteCollection => 'Elimina raccolta';

  @override
  String get snippetsDeleteCollectionMove =>
      'Sposta i suoi frammenti in Senza raccolta';

  @override
  String snippetsDeleteCollectionTitle(String name) {
    return 'Eliminare «$name»?';
  }

  @override
  String get snippetsDeleteCollectionWithSnippets =>
      'Elimina anche i suoi frammenti';

  @override
  String get snippetsDeletedMessage => 'Spostato in Eliminati di recente.';

  @override
  String snippetsDeletedTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count frammenti eliminati',
      one: 'Frammento eliminato',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDestination => 'Salva in';

  @override
  String snippetsDestinationName(String destination) {
    String _temp0 = intl.Intl.selectLogic(destination, {
      'historyClip': 'Elemento della cronologia',
      'snippet': 'Frammento',
      'other': 'Elemento della cronologia',
    });
    return '$_temp0';
  }

  @override
  String get snippetsDiscard => 'Scarta';

  @override
  String snippetsDragging(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count frammenti',
      one: '1 frammento',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDuplicateAbbreviation =>
      'Un altro frammento ha ricevuto la stessa abbreviazione durante una sincronizzazione. Cambiane una.';

  @override
  String get snippetsDuplicateBadge => 'Abbreviazione duplicata';

  @override
  String get snippetsEditClip => 'Modifica elemento';

  @override
  String get snippetsEditCollection => 'Modifica raccolta';

  @override
  String get snippetsEditSnippet => 'Modifica frammento';

  @override
  String get snippetsEditTab => 'Modifica';

  @override
  String get snippetsEmptyMessage =>
      'Assegna un\'abbreviazione a un frammento, poi digitala in Quick Paste.';

  @override
  String get snippetsEmptyTitle => 'Ancora nessun frammento';

  @override
  String get snippetsEnableTemplate => 'Attiva';

  @override
  String get snippetsEnableTemplateMessage =>
      'I segnaposto funzionano solo nei modelli. Anche il testo che sembra già un segnaposto verrà compilato.';

  @override
  String get snippetsEnableTemplateTitle => 'Attivare Modello?';

  @override
  String get snippetsExport => 'Esporta';

  @override
  String get snippetsExportCollection => 'Esporta raccolta';

  @override
  String get snippetsExportConcealedConfirm =>
      'Includi i frammenti nascosti nel file leggibile';

  @override
  String snippetsExportCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count frammenti scritti',
      one: '1 frammento scritto',
      zero: 'Nessun frammento scritto',
    );
    return '$_temp0';
  }

  @override
  String get snippetsExportDone => 'Frammenti esportati';

  @override
  String get snippetsExportEncrypt => 'Cifra con passphrase';

  @override
  String get snippetsExportFailed => 'Esportazione non riuscita';

  @override
  String get snippetsExportPlainConfirm =>
      'Ho capito che chiunque abbia il file può leggerlo';

  @override
  String snippetsExportScope(String scope, int count) {
    String _temp0 = intl.Intl.selectLogic(scope, {
      'all': 'Tutti i frammenti',
      'collections': 'Questa raccolta',
      'snippets': 'Frammenti selezionati ($count)',
      'other': 'Tutti i frammenti',
    });
    return '$_temp0';
  }

  @override
  String get snippetsExportTitle => 'Esporta frammenti';

  @override
  String get snippetsFieldBuilderTitle => 'Inserisci campo';

  @override
  String get snippetsFieldDefault => 'Valore predefinito';

  @override
  String get snippetsFieldName => 'Nome';

  @override
  String get snippetsFieldOptions => 'Opzioni, separate da virgole';

  @override
  String get snippetsFieldRequired => 'Compila questo campo.';

  @override
  String get snippetsFieldType => 'Tipo';

  @override
  String snippetsFieldTypeName(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'text': 'Una riga',
      'multiline': 'Più righe',
      'choice': 'Scelta',
      'other': 'Una riga',
    });
    return '$_temp0';
  }

  @override
  String get snippetsFormattingNotice =>
      'La formattazione verrà rimossa al salvataggio.';

  @override
  String get snippetsGoToSnippet => 'Vai al frammento';

  @override
  String get snippetsImport => 'Importa';

  @override
  String get snippetsImportAbbreviationPolicy => 'Abbreviazione già in uso';

  @override
  String snippetsImportAbbreviationPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'importWithout': 'Importa senza abbreviazione',
      'renameWithSuffix': 'Aggiungi un numero all\'abbreviazione',
      'skip': 'Salta',
      'other': 'Importa senza abbreviazione',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportCollectionPolicy => 'Raccolta con lo stesso nome';

  @override
  String snippetsImportCollectionPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'merge': 'Aggiungi a quella esistente',
      'createNew': 'Creane una nuova',
      'other': 'Aggiungi a quella esistente',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportConvert => 'Converti i segnaposto noti';

  @override
  String snippetsImportCounts(
    int added,
    int identical,
    int updated,
    int conflicts,
    int invalid,
  ) {
    return 'Nuovi: $added. Identici, saltati: $identical. Modificati: $updated. Conflitti di abbreviazione: $conflicts. Non validi: $invalid.';
  }

  @override
  String get snippetsImportDone => 'Frammenti importati';

  @override
  String get snippetsImportFailed => 'Importazione non riuscita';

  @override
  String get snippetsImportFileType => 'File di frammenti';

  @override
  String get snippetsImportFormat => 'Formato';

  @override
  String snippetsImportFormatName(String format) {
    String _temp0 = intl.Intl.selectLogic(format, {
      'bundle': 'Pacchetto CopyPaste',
      'json': 'Elenco JSON',
      'csv': 'CSV',
      'other': 'Pacchetto CopyPaste',
    });
    return '$_temp0';
  }

  @override
  String snippetsImportInvalidRow(int row, String reason) {
    return 'Riga $row: $reason';
  }

  @override
  String get snippetsImportInvalidRows => 'Righe che non verranno importate';

  @override
  String get snippetsImportPassphraseHint => 'Passphrase, se il file è cifrato';

  @override
  String get snippetsImportPreview => 'Anteprima';

  @override
  String snippetsImportReport(
    int inserted,
    int updated,
    int skipped,
    int flagged,
    int unrecognized,
  ) {
    return 'Aggiunti: $inserted. Aggiornati: $updated. Saltati: $skipped. Abbreviazioni da controllare: $flagged. Segnaposto non riconosciuti: $unrecognized.';
  }

  @override
  String get snippetsImportSameIdPolicy =>
      'Stesso frammento, contenuto diverso';

  @override
  String snippetsImportSameIdPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'keepBoth': 'Mantieni entrambi',
      'replace': 'Sostituisci l\'esistente',
      'skip': 'Salta',
      'other': 'Mantieni entrambi',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportTitle => 'Importa frammenti';

  @override
  String get snippetsInsert => 'Inserisci';

  @override
  String get snippetsInsertPlaceholder => 'Inserisci segnaposto';

  @override
  String get snippetsKeepReferences => 'Lasciali così';

  @override
  String get snippetsLibraryDescription =>
      'Sfoglia, organizza e modifica frammenti e raccolte.';

  @override
  String get snippetsLibraryKeywords =>
      'frammenti modelli abbreviazioni raccolte testo snippet';

  @override
  String get snippetsLibraryTitle => 'Libreria dei frammenti';

  @override
  String snippetsLint(String code, String detail) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'unclosed': 'Questo segnaposto non viene mai chiuso.',
      'malformed': 'Impossibile leggere questo segnaposto.',
      'unknownPlaceholder':
          'Segnaposto sconosciuto «$detail». Viene incollato così com\'è.',
      'unknownParameter': 'Opzione sconosciuta «$detail».',
      'duplicateParameter': 'L\'opzione «$detail» è impostata due volte.',
      'missingParameter': 'Manca l\'opzione «$detail».',
      'invalidValue': 'L\'opzione «$detail» ha un valore non valido.',
      'conflictingParameters':
          'Queste opzioni non possono essere usate insieme.',
      'unknownLocale': 'Lingua sconosciuta.',
      'unknownTimeZone': 'Fuso orario sconosciuto.',
      'invalidPattern': 'Formato di data non valido.',
      'dateOutOfRange': 'La data è fuori intervallo.',
      'unknownModifier': 'Modificatore sconosciuto «$detail».',
      'modifierFailed': 'Impossibile applicare il modificatore «$detail».',
      'emptyClipboard': 'L\'elemento $detail degli appunti è vuoto.',
      'extraCursor': 'Viene usato solo il primo segnaposto del cursore.',
      'nestedCursor': 'Un cursore in un frammento inserito viene ignorato.',
      'tooManyFields': 'Il campo «$detail» supera il limite di 10 campi.',
      'conflictingField':
          'Il campo «$detail» è definito due volte con impostazioni diverse.',
      'choiceDefaultNotOption':
          'Il valore predefinito di «$detail» non è una delle sue opzioni.',
      'cycle': 'I frammenti si inseriscono a vicenda in un ciclo: $detail.',
      'depthExceeded':
          'I frammenti sono inseriti a più di 3 livelli di profondità.',
      'nestingLimit': 'Troppi frammenti inseriti.',
      'unknownSnippet': 'Nessun frammento ha questa abbreviazione.',
      'containsPlaceholders':
          'Il testo contiene segnaposto. Attiva Modello per compilarli.',
      'other': 'Questo segnaposto ha un problema.',
    });
    return '$_temp0';
  }

  @override
  String get snippetsLoadFailed => 'Impossibile caricare i frammenti';

  @override
  String get snippetsLoadMore => 'Carica altri';

  @override
  String get snippetsMerge => 'Unisci';

  @override
  String get snippetsMergeCollection => 'Unisci a un\'altra raccolta';

  @override
  String snippetsMergeCollectionTitle(String name) {
    return 'Unisci «$name» a';
  }

  @override
  String get snippetsMoveTo => 'Sposta in';

  @override
  String get snippetsNewCollection => 'Nuova raccolta';

  @override
  String get snippetsNewSnippet => 'Nuovo frammento';

  @override
  String get snippetsNewText => 'Nuovo testo';

  @override
  String snippetsNoMatch(String query) {
    return 'Nessun frammento corrisponde a «$query»';
  }

  @override
  String get snippetsPassphraseConfirm => 'Ripeti la passphrase';

  @override
  String snippetsPassphraseHint(int min) {
    return 'Passphrase, almeno $min caratteri';
  }

  @override
  String get snippetsPassphraseLabel => 'Passphrase';

  @override
  String get snippetsPassphraseNoRecovery =>
      'Una passphrase dimenticata non può essere recuperata.';

  @override
  String snippetsPassphraseStrength(String score) {
    String _temp0 = intl.Intl.selectLogic(score, {
      'tooShort': 'Troppo corta',
      'weak': 'Debole',
      'fair': 'Discreta',
      'good': 'Buona',
      'other': 'Forte',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPinnedSwitch => 'Fissato';

  @override
  String get snippetsPlaceholderGroup => 'Segnaposto';

  @override
  String snippetsPlaceholderKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'today': 'Data',
      'dateShort': 'Data, breve',
      'dateLong': 'Data, estesa',
      'dateIso': 'Data, ISO 8601',
      'dateNextWeek': 'Data tra una settimana',
      'now': 'Ora',
      'dateTime': 'Data e ora',
      'clipboard': 'Appunti',
      'clipboardPrevious': 'Elemento precedente degli appunti',
      'field': 'Campo da compilare…',
      'cursor': 'Posizione del cursore',
      'uuid': 'UUID',
      'random': 'Numero casuale',
      'upper': 'Modificatore: maiuscole',
      'lower': 'Modificatore: minuscole',
      'trim': 'Modificatore: rimuovi spazi',
      'other': 'Segnaposto',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPlaceholderNestedGroup => 'Inserisci un altro frammento';

  @override
  String get snippetsPlaceholderNoMatch => 'Nessun segnaposto corrisponde';

  @override
  String get snippetsPreviewEmpty => 'Niente da mostrare';

  @override
  String get snippetsPreviewTab => 'Anteprima';

  @override
  String get snippetsPurge => 'Elimina definitivamente';

  @override
  String get snippetsPurgeMessage =>
      'Verranno cancellati su tutti i tuoi dispositivi. L\'operazione è irreversibile.';

  @override
  String snippetsPurgeTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Eliminare definitivamente $count frammenti?',
      one: 'Eliminare definitivamente il frammento?',
    );
    return '$_temp0';
  }

  @override
  String get snippetsRecentlyDeletedEmpty => 'Eliminati di recente è vuoto';

  @override
  String get snippetsRecordPastedDescription =>
      'Aggiunge alla cronologia il testo dei frammenti incollati. I frammenti nascosti non vengono mai registrati.';

  @override
  String get snippetsRecordPastedKeywords =>
      'cronologia registra incollati frammenti';

  @override
  String get snippetsRecordPastedTitle =>
      'Registra i frammenti incollati nella cronologia';

  @override
  String get snippetsReplaceOriginal => 'Sostituisci l\'originale';

  @override
  String get snippetsReplaceOriginalHint =>
      'Elimina l\'elemento originale su tutti i dispositivi dopo il salvataggio. Se disattivato, restano entrambi.';

  @override
  String get snippetsRequireAuthDescription =>
      'Verifica la tua identità prima di usare o mostrare un frammento nascosto. Ogni dispositivo lo chiede separatamente.';

  @override
  String get snippetsRequireAuthKeywords =>
      'nascosto password biometria impronta volto blocco verifica';

  @override
  String get snippetsRequireAuthTitle =>
      'Verifica prima di usare frammenti nascosti';

  @override
  String get snippetsRestore => 'Ripristina';

  @override
  String get snippetsSaveBlockEmpty => 'Aggiungi del testo per salvare.';

  @override
  String snippetsSaveBlockTitle(int max) {
    return 'Il titolo supera i $max caratteri.';
  }

  @override
  String get snippetsSaveBlockTooLarge => 'Il testo supera i 64 KB.';

  @override
  String get snippetsSaveFailed => 'Salvataggio non riuscito';

  @override
  String get snippetsScopeAll => 'Tutti';

  @override
  String get snippetsScopeEmpty => 'Nessun frammento qui';

  @override
  String get snippetsScopePinned => 'Fissati';

  @override
  String get snippetsScopeRecentlyDeleted => 'Eliminati di recente';

  @override
  String get snippetsSearchPlaceholder => 'Cerca frammenti';

  @override
  String snippetsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count selezionati',
      one: '1 selezionato',
    );
    return '$_temp0';
  }

  @override
  String get snippetsShow => 'Mostra';

  @override
  String get snippetsShowAll => 'Mostra tutti';

  @override
  String get snippetsShowAttention => 'Controlla';

  @override
  String snippetsSoftLimit(int count) {
    return 'Hai $count frammenti. Ne sono consentiti fino a 20.000.';
  }

  @override
  String get snippetsTemplateBadge => 'Modello';

  @override
  String get snippetsTemplateSwitch => 'Modello';

  @override
  String get snippetsTitleLabel => 'Titolo';

  @override
  String get snippetsTitlePlaceholder => 'Facoltativo';

  @override
  String get snippetsTransferDescription =>
      'Sposta frammenti tra app o conserva un backup. Le esportazioni sono cifrate per impostazione predefinita.';

  @override
  String get snippetsTransferKeywords =>
      'importa esporta backup pacchetto json csv passphrase';

  @override
  String get snippetsTransferTitle => 'Importa ed esporta';

  @override
  String get snippetsUndo => 'Annulla';

  @override
  String get snippetsUnfiled => 'Senza raccolta';

  @override
  String snippetsUnfilledWarning(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count segnaposto vengono incollati così come sono',
      one: '1 segnaposto viene incollato così com\'è',
    );
    return '$_temp0';
  }

  @override
  String get snippetsUnpin => 'Non fissare più';

  @override
  String get snippetsUnsavedMessage =>
      'Salvare le modifiche prima di chiudere?';

  @override
  String get snippetsUnsavedTitle => 'Modifiche non salvate';

  @override
  String get snippetsUntitled => 'Frammento senza titolo';

  @override
  String get snippetsUpdateReferences => 'Aggiorna';

  @override
  String snippetsUpdateReferencesMessage(String from, String to) {
    return 'Altri modelli potrebbero inserire «$from». Aggiornarli per usare «$to»?';
  }

  @override
  String get snippetsUpdateReferencesTitle => 'Aggiornare i riferimenti?';

  @override
  String get snippetsUseFailed => 'Impossibile usare il frammento';

  @override
  String get quickPasteTabQueue => 'Coda';

  @override
  String get quickPasteMaskedMode => 'Campo password: anteprime nascoste';

  @override
  String quickPasteMaskedItem(String kind) {
    return 'Nascosto: $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'Sblocca gli elementi segreti';

  @override
  String quickPastePastingWith(String name) {
    return 'Incolla con: $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut incolla l\'originale';
  }

  @override
  String get quickPasteDefaultSkipped => 'Trasformazione predefinita saltata';

  @override
  String get quickPasteDefaultSkippedMessage => 'È stato usato l\'originale.';

  @override
  String get quickPastePasteManually =>
      'Il contenuto è negli appunti. Incollalo manualmente.';

  @override
  String get quickPastePaste => 'Incolla';

  @override
  String get quickPasteSnippetsUnavailable =>
      'I frammenti non sono disponibili';
}
