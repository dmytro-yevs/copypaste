// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for French (`fr`).
class AppLocalizationsFr extends AppLocalizations {
  AppLocalizationsFr([String locale = 'fr']) : super(locale);

  @override
  String get commonCancel => 'Annuler';

  @override
  String get commonClose => 'Fermer';

  @override
  String get commonDone => 'Terminé';

  @override
  String get commonRetry => 'Réessayer';

  @override
  String get commonTryAgain => 'Réessayer';

  @override
  String get commonCopy => 'Copier';

  @override
  String get commonDelete => 'Supprimer';

  @override
  String get commonSave => 'Enregistrer';

  @override
  String get commonContinue => 'Continuer';

  @override
  String get commonBack => 'Retour';

  @override
  String get commonOpenSettings => 'Ouvrir les paramètres';

  @override
  String get commonLoading => 'Chargement';

  @override
  String get commonApply => 'Appliquer';

  @override
  String get commonClear => 'Effacer';

  @override
  String get commonPin => 'Épingler';

  @override
  String get commonEnable => 'Activer';

  @override
  String get settingsLanguageTitle => 'Langue';

  @override
  String get settingsLanguageDescription =>
      'Choisissez la langue de l’interface de CopyPaste.';

  @override
  String get settingsLanguageSystem => 'Langue du système';

  @override
  String get settingsLanguageKeywords =>
      'langue paramètres régionaux traduction interface language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Impossible de modifier le lancement à l’ouverture de session. Réessayez.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'Impossible d’ouvrir les paramètres de démarrage du système.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Impossible d’actualiser les paramètres. Réessayez.';

  @override
  String get settingsRuntimeLoadFailed =>
      'Impossible de charger les paramètres.';

  @override
  String get settingsRuntimeAppAlreadyExcluded =>
      'Cette application est déjà exclue.';

  @override
  String get settingsRuntimeChangeFailed =>
      'Impossible d’effectuer la modification demandée.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Accordez toutes les autorisations de capture du presse-papiers pour pouvoir reprendre la capture.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'La capture du presse-papiers est en pause. Le service de capture Android n’a pas confirmé son arrêt.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'L’autorisation des notifications est requise pour activer les notifications de capture.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste n’a pas pu lire les paramètres enregistrés. Des valeurs de protection sont appliquées ; vérifiez et enregistrez vos préférences pour les réparer.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste a remplacé des paramètres illisibles par des valeurs de protection. Vérifiez et enregistrez vos préférences pour les réparer.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Les identifiants d’application ne doivent pas être vides et ne doivent pas dépasser 256 octets.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'Vous pouvez exclure jusqu’à 256 applications.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Impossible de modifier le collage automatique.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'Impossible d’enregistrer ce raccourci.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'Impossible de suspendre le raccourci actuel.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'Impossible de rétablir le raccourci.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'Impossible d’ouvrir Quick Paste.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Impossible de demander l’autorisation Accessibilité.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Impossible d’actualiser l’état de l’intégration Linux.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'L’autorisation de saisie du presse-papiers n’a pas été accordée.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Impossible de demander l’autorisation de saisie du presse-papiers.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Impossible d’ouvrir la configuration de l’intégration au bureau.';

  @override
  String get settingsRuntimeTextHistoryFileType => 'Historique texte CopyPaste';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'Sauvegarde chiffrée CopyPaste';

  @override
  String get settingsRuntimeNotificationImage => 'Image';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Image · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'Fichier';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste n’a pas pu rétablir l’installation de la mise à jour.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Cette installation Linux ne peut pas se mettre à jour automatiquement.';

  @override
  String get updateMacosPackageMissing =>
      'Cette version ne comporte aucun paquet de mise à jour macOS signé.';

  @override
  String get updateInstalledVersionInvalid =>
      'La version installée de CopyPaste n’est pas valide.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste n’a pas pu joindre GitHub pour rechercher des mises à jour.';

  @override
  String get updateCheckTimedOut =>
      'Le délai de recherche de mises à jour a expiré.';

  @override
  String get updateCheckDeviceFailed =>
      'CopyPaste n’a pas pu rechercher de mises à jour sur cet appareil.';

  @override
  String get updateCheckFailed =>
      'CopyPaste n’a pas pu rechercher de mises à jour.';

  @override
  String get updateDownloadFailed =>
      'CopyPaste n’a pas pu télécharger la mise à jour.';

  @override
  String get updateDownloadTimedOut =>
      'Le délai de téléchargement de la mise à jour a expiré.';

  @override
  String get updateInstallFailed =>
      'CopyPaste n’a pas pu installer la mise à jour.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Poursuivez dans le programme d’installation du système Android.';

  @override
  String get updateContinueInPackageManager =>
      'Poursuivez dans le gestionnaire de paquets de votre système.';

  @override
  String get updateInstallerStarting => 'Le programme d’installation démarre.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Autorisez CopyPaste à installer des applications dans les paramètres Android, puis continuez.';

  @override
  String get updatePermissionRequiredLinux =>
      'Authentifiez-vous dans le gestionnaire de paquets de votre système pour installer cette mise à jour.';

  @override
  String get updatePermissionRequired =>
      'Autorisez CopyPaste à installer cette mise à jour, puis continuez.';

  @override
  String get updateRestartRequired =>
      'La mise à jour est installée. Redémarrez CopyPaste pour l’utiliser.';

  @override
  String get updateRestarting => 'Redémarrage de CopyPaste.';

  @override
  String get updateRestartFailed =>
      'CopyPaste n’a pas pu redémarrer. Réessayez.';

  @override
  String get updateReleasePageOpenFailed =>
      'CopyPaste n’a pas pu ouvrir la page de la version.';

  @override
  String get updatePackageInvalid =>
      'La mise à jour téléchargée n’est pas un paquet CopyPaste valide.';

  @override
  String get updateSignatureInvalid =>
      'La signature de la mise à jour téléchargée n’est pas valide.';

  @override
  String get updateDowngradeRefused =>
      'Le paquet téléchargé n’est pas plus récent que cette version.';

  @override
  String get updateHomebrewRequired =>
      'Installez CopyPaste avec Homebrew pour pouvoir le mettre à jour ici.';

  @override
  String get updateInstallationCancelled =>
      'L’installation de la mise à jour a été annulée.';

  @override
  String get updateInstallationBlocked =>
      'Android a bloqué l’installation de la mise à jour.';

  @override
  String get updateInstallationConflict =>
      'La mise à jour est en conflit avec l’application installée.';

  @override
  String get updateInstallationIncompatible =>
      'La mise à jour n’est pas compatible avec cet appareil.';

  @override
  String get updateInstallationStorage =>
      'Libérez de l’espace de stockage pour installer la mise à jour.';

  @override
  String get updateInstallationInterrupted =>
      'L’installation de la mise à jour a été interrompue. Réessayez.';

  @override
  String get updateInstallationBusy =>
      'Une installation de mise à jour est déjà en cours.';

  @override
  String get updatePackageDetailsInvalid =>
      'Les informations du paquet de mise à jour ne sont pas valides.';

  @override
  String get updateIntegrityCheckFailed =>
      'La mise à jour téléchargée n’a pas réussi la vérification d’intégrité.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste n’a pas pu lancer le programme d’installation du système.';

  @override
  String get updateInstallLocationTranslocated =>
      'Déplacez CopyPaste dans le dossier Applications et ouvrez-le depuis cet emplacement pour le mettre à jour.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste ne peut pas se remplacer dans son dossier actuel. Utilisez la page de la version pour le mettre à jour.';

  @override
  String get updateReplacementBlocked =>
      'macOS a bloqué le remplacement de CopyPaste. Autorisez-le dans les réglages Gestion des apps ou utilisez la page de la version.';

  @override
  String get updatePackageNotDownloadable =>
      'Cette plateforme n’utilise pas de paquet de mise à jour téléchargeable.';

  @override
  String get updatePackageSizeInvalid =>
      'La taille du paquet de mise à jour n’est pas valide.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'La signature du programme de mise à jour de la mise à jour téléchargée n’est pas valide.';

  @override
  String get updatePackageUrlUntrusted =>
      'L’URL du paquet de mise à jour n’est pas fiable.';

  @override
  String get updateDownloadLeftGitHub =>
      'Le téléchargement de la mise à jour a quitté GitHub.';

  @override
  String get updateDownloadTooManyRedirects =>
      'Le téléchargement de la mise à jour a subi trop de redirections.';

  @override
  String get updatePackageTooLarge =>
      'Le paquet de mise à jour est trop volumineux.';

  @override
  String get updateSignatureSizeInvalid =>
      'La taille de la signature du programme de mise à jour n’est pas valide.';

  @override
  String get updateMetadataInvalid =>
      'GitHub a renvoyé des métadonnées de mise à jour non valides.';

  @override
  String get updateMetadataTooLarge =>
      'Les métadonnées de mise à jour sont trop volumineuses.';

  @override
  String get quickPasteActionFailedTitle => 'Échec de l’action';

  @override
  String get quickPasteOpenAppSemantics => 'Ouvrir CopyPaste';

  @override
  String get quickPasteSearchPlaceholder => 'Tapez pour rechercher…';

  @override
  String get quickPasteLoadingHistory =>
      'Chargement de l’historique du presse-papiers.';

  @override
  String get quickPasteHistoryUnavailable =>
      'L’historique n’est pas disponible';

  @override
  String get quickPasteNoClipsFound => 'Aucun élément trouvé';

  @override
  String get quickPasteLoadMore => 'Charger plus';

  @override
  String get quickPasteTransferFailed => 'Échec';

  @override
  String get quickPasteClipUnavailable => 'Élément indisponible';

  @override
  String get quickPasteFileFallbackName => 'Fichier';

  @override
  String get quickPastePathUnavailable => 'Chemin indisponible';

  @override
  String get quickPastePreferences => 'Préférences…';

  @override
  String get quickPasteAbout => 'À propos';

  @override
  String get quickPasteQuit => 'Quitter';

  @override
  String get quickPasteClearHistoryTitle => 'Effacer l’historique ?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Les éléments épinglés resteront dans votre historique.';

  @override
  String get quickPasteClearHistoryFailed =>
      'Impossible de supprimer l’historique.';

  @override
  String get quickPasteAboutDescription =>
      'Historique privé du presse-papiers sur macOS, Android, Windows et Linux.';

  @override
  String get transfersTitle => 'Téléchargements et transferts';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count transferts actifs',
      many: '$count transferts actifs',
      one: '$count transfert actif',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count en cours',
      many: '$count en cours',
      one: '$count en cours',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Récents';

  @override
  String get transfersShowEarlier => 'Plus anciens';

  @override
  String get transfersClearCompletedHistory =>
      'Effacer les transferts terminés';

  @override
  String get transfersErrorTitle => 'Transferts';

  @override
  String get transfersEmptyTitle => 'Aucun transfert';

  @override
  String get transfersEmptyMessage =>
      'Les importations de fichiers, les téléchargements et les transferts sortants s’affichent ici.';

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
    return '$percent %';
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
  String get transfersRemoveFromHistory => 'Retirer de l’historique';

  @override
  String get transfersCancelDownload => 'Annuler le téléchargement';

  @override
  String get transfersShowFile => 'Afficher le fichier';

  @override
  String get transfersPhaseSending => 'Envoi';

  @override
  String get transfersPhaseDownloading => 'Téléchargement';

  @override
  String get transfersPhaseAdding => 'Ajout';

  @override
  String get transfersPhaseAdded => 'Ajouté';

  @override
  String get transfersPhaseSent => 'Envoyé';

  @override
  String get transfersPhaseCompleted => 'Terminé';

  @override
  String get transfersPhaseCancelled => 'Annulé';

  @override
  String get transfersPhaseInterrupted => 'Interrompu';

  @override
  String get transfersPhaseFailed => 'Échec';

  @override
  String get transfersUpdatesUnavailable =>
      'La progression des transferts n’est plus disponible. Rouvrez pour actualiser.';

  @override
  String get transfersLoadFailed =>
      'Impossible de charger les transferts. Réessayez.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Impossible de charger l’état des transferts.';

  @override
  String get transfersRetryFailed => 'Impossible de relancer ce fichier.';

  @override
  String get transfersClearHistoryFailed =>
      'Impossible d’effacer l’historique des transferts.';

  @override
  String get transfersRemoveRecordFailed =>
      'Impossible de retirer cette entrée de l’historique.';

  @override
  String get transfersCancelFailed =>
      'Impossible d’annuler le transfert. Il est peut-être toujours en cours.';

  @override
  String get transfersSavedFileUnavailable =>
      'Le fichier enregistré n’est pas disponible. Il a peut-être été déplacé ou supprimé.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Notifications · $count non lues',
      many: 'Notifications · $count non lues',
      one: 'Notifications · $count non lue',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Notifications';

  @override
  String get notificationsClearRead => 'Effacer les notifications lues';

  @override
  String get notificationsEmptyTitle => 'Aucune notification';

  @override
  String get notificationsEmptyMessage =>
      'Les demandes de connexion d’appareils et leurs résultats s’affichent ici.';

  @override
  String get notificationsSystemAlertTitle => 'Notifications du système';

  @override
  String get notificationsReview => 'Examiner';

  @override
  String get notificationsConnectionRequest => 'Demande de connexion';

  @override
  String get notificationsConfirmConnection => 'Confirmer la connexion';

  @override
  String get notificationsDeviceConnected => 'Appareil connecté';

  @override
  String get notificationsConnectionRejected => 'Connexion refusée';

  @override
  String get notificationsConnectionCancelled => 'Connexion annulée';

  @override
  String get notificationsConnectionRequestExpired =>
      'Demande de connexion expirée';

  @override
  String get notificationsConnectionFailed => 'Échec de la connexion';

  @override
  String get notificationsNearbyDevice => 'Appareil à proximité';

  @override
  String get notificationsPermissionDenied =>
      'Autorisez les notifications dans les paramètres de votre système pour recevoir les demandes en arrière-plan.';

  @override
  String get notificationsSystemUnavailable =>
      'Les notifications du système ne sont pas disponibles. Les demandes de connexion restent ici.';

  @override
  String get platformCaptureStateUnavailable => 'Indisponible';

  @override
  String get platformCaptureStatePaused => 'En pause';

  @override
  String get platformCaptureStateRunning => 'Capture en cours';

  @override
  String get platformCaptureStateStopped => 'À l’arrêt';

  @override
  String get platformCaptureActionUnavailable => 'Capture indisponible';

  @override
  String get platformCaptureActionResume => 'Reprendre la capture';

  @override
  String get platformCaptureActionPause => 'Suspendre la capture';

  @override
  String get platformCaptureActionResumeStopped =>
      'Reprendre la capture (à l’arrêt)';

  @override
  String get platformTrayOpen => 'Ouvrir';

  @override
  String get platformTraySettings => 'Paramètres';

  @override
  String get platformTrayQuit => 'Quitter';

  @override
  String get platformWindowControlsUnavailable =>
      'Les commandes de fenêtre du bureau ne sont pas disponibles. CopyPaste restera visible.';

  @override
  String get platformTrayUnavailable =>
      'La zone de notification n’est pas disponible. Laissez CopyPaste ouvert, puis réessayez ou quittez.';

  @override
  String get platformQuitFailed => 'CopyPaste n’a pas pu quitter. Réessayez.';

  @override
  String get platformVisibilityChangeFailed =>
      'Impossible de modifier la visibilité de l’application. Réessayez.';

  @override
  String get platformCaptureNotificationTitle => 'Presse-papiers enregistré';

  @override
  String get platformCaptureNotificationBody =>
      'Un nouvel élément a été ajouté à CopyPaste.';

  @override
  String get platformPairingNotificationTitle => 'Demande de connexion';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName souhaite se connecter. Ouvrez CopyPaste pour comparer le code et confirmer.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Demandes de connexion d’appareils';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Demandes de connexion d’appareils à proximité';

  @override
  String get platformNotificationDefaultAction => 'Ouvrir';

  @override
  String get platformFilesSelectionFailed =>
      'Impossible de sélectionner les fichiers.';

  @override
  String get platformFilesOnlyRegular =>
      'Seuls les fichiers standard peuvent être importés.';

  @override
  String get platformModulesUnavailable =>
      'Les modules ne sont pas disponibles sur cet appareil.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'Impossible de configurer l’accès aux SMS.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'Impossible de vérifier l’accès aux SMS.';

  @override
  String get platformUpdatesUnavailableHere =>
      'Les mises à jour de l’application ne sont pas disponibles ici.';

  @override
  String get platformUpdateVersionUnavailable =>
      'La version installée n’est pas disponible.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'Les mises à jour de l’application ne sont pas disponibles pour cette installation.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Cette installation Linux ne peut pas se mettre à jour automatiquement.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Cette mise à jour Linux ne correspond pas à l’architecture utilisée.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Cette installation macOS ne peut pas se mettre à jour automatiquement.';

  @override
  String get platformUpdateStartFailed =>
      'CopyPaste n’a pas pu lancer la mise à jour.';

  @override
  String get platformGlobalShortcutDescription => 'Ouvrir Quick Paste';

  @override
  String get settingsSectionGeneral => 'Général';

  @override
  String get settingsSectionGeneralSummary =>
      'Langue et visibilité de l’application.';

  @override
  String get settingsSectionClipboard => 'Presse-papiers';

  @override
  String get settingsSectionClipboardSummary =>
      'Capture et limites de l’historique.';

  @override
  String get settingsSectionPrivacy => 'Confidentialité';

  @override
  String get settingsSectionPrivacySummary =>
      'Exclusions d’applications et protection de l’écran.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Raccourci et collage automatique.';

  @override
  String get settingsSectionSync => 'Synchronisation';

  @override
  String get settingsSectionSyncSummary =>
      'Synchronisation et détection des appareils à proximité.';

  @override
  String get settingsSectionNotifications => 'Notifications';

  @override
  String get settingsSectionNotificationsSummary =>
      'Notifications et sons de capture.';

  @override
  String get settingsSectionData => 'Données';

  @override
  String get settingsSectionDataSummary =>
      'Exportation, sauvegarde et restauration.';

  @override
  String get settingsSectionModules => 'Modules';

  @override
  String get settingsSectionModulesSummary =>
      'Installer et gérer les modules facultatifs.';

  @override
  String get settingsSectionRules => 'Règles de contenu';

  @override
  String get settingsSectionRulesSummary =>
      'Étiqueter, ignorer, protéger ou faire expirer les éléments automatiquement.';

  @override
  String get settingsSectionQuickAccess => 'Accès rapide';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Clavier, tuiles et widget d’écran d’accueil.';

  @override
  String get settingsSectionSnippets => 'Extraits';

  @override
  String get settingsSectionSnippetsSummary =>
      'Textes et modèles réutilisables.';

  @override
  String get settingsSectionTransforms => 'Transformations de texte';

  @override
  String get settingsSectionTransformsSummary =>
      'Favoris, enchaînements et transformations par défaut au collage.';

  @override
  String get settingsSectionAiTools => 'Outils d’IA';

  @override
  String get settingsSectionAiToolsSummary =>
      'Connecter des assistants d’IA et contrôler ce qu’ils peuvent voir.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Rassembler plusieurs copies et les coller dans l’ordre.';

  @override
  String pasteQueuePasted(int position, int total) {
    return '$position sur $total collé';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'L\'élément $position sur $total est dans le presse-papiers';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments collés',
      one: '1 élément collé',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments sont dans le presse-papiers',
      one: '1 élément est dans le presse-papiers',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'File terminée';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Ajouté, $total dans la file',
      one: 'Ajouté, 1 dans la file',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'File vide, rien n\'a été collé';

  @override
  String get pasteQueueMixed =>
      'Tout coller fonctionne avec des files contenant uniquement du texte ou uniquement des fichiers';

  @override
  String get pasteQueueSeveralFiles => 'Collez les fichiers un par un.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments ne sont plus disponibles',
      one: '1 élément n\'est plus disponible',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => 'Coller le suivant ne fait que copier';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Élément copié. Autorisez l’accessibilité pour coller automatiquement.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Collage automatique impossible. L’élément a été copié.';

  @override
  String get pasteQueueTargetElevated =>
      'Impossible de coller dans une app administrateur. L’élément a été copié.';

  @override
  String get pasteQueueNoTarget =>
      'Aucune app où coller. L’élément a été copié.';

  @override
  String get pasteQueueCollectingStarted => 'Collecte des copies';

  @override
  String get pasteQueueCollectingStopped => 'Collecte arrêtée';

  @override
  String get pasteQueueFailed =>
      'Impossible de terminer l\'action de Paste Queue';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count en attente',
      one: '1 en attente',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count en attente · Collecte',
      one: '1 en attente · Collecte',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Collecte';

  @override
  String get pasteQueueFinishedSummary => 'Terminée';

  @override
  String pasteQueueFull(int max) {
    return 'File pleine ($max)';
  }

  @override
  String get pasteQueueFullMessage =>
      'Les nouvelles copies ne sont plus ajoutées.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue est vide';

  @override
  String get pasteQueueEmptyMessage =>
      'Rassemblez des copies ou ajoutez des éléments depuis l’historique.';

  @override
  String get pasteQueueCollectingEmptyTitle =>
      'Copiez des éléments pour les ajouter';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Chaque copie rejoint la file jusqu\'à ce que vous arrêtiez la collecte.';

  @override
  String get pasteQueueStartCollecting => 'Commencer la collecte';

  @override
  String get pasteQueueStopCollecting => 'Arrêter la collecte';

  @override
  String get pasteQueueLoading => 'Chargement de la file…';

  @override
  String get pasteQueueLoadFailed => 'Impossible de charger la file';

  @override
  String get pasteQueuePasteNext => 'Coller le suivant';

  @override
  String get pasteQueueCopyNext => 'Copier le suivant';

  @override
  String get pasteQueuePasteAll => 'Tout coller';

  @override
  String get pasteQueueCopyAll => 'Tout copier';

  @override
  String get pasteQueueReverse => 'Inverser l\'ordre';

  @override
  String get pasteQueueStepBack => 'Revenir en arrière';

  @override
  String get pasteQueueSkip => 'Ignorer';

  @override
  String get pasteQueueRemovePasted => 'Retirer les éléments collés';

  @override
  String get pasteQueueCleared => 'File vidée';

  @override
  String get pasteQueueUndo => 'Annuler';

  @override
  String get pasteQueueStartFresh => 'Repartir de zéro';

  @override
  String get pasteQueueAppend => 'Ajouter à la suite';

  @override
  String get pasteQueueRestartTitle => 'Commencer une nouvelle collecte ?';

  @override
  String get pasteQueueRestartMessage =>
      'Repartir de zéro retire les éléments collés. Ajouter à la suite les conserve.';

  @override
  String pasteQueueStale(String time) {
    return 'File du $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Collé';

  @override
  String get pasteQueueStatusSkipped => 'Ignoré';

  @override
  String get pasteQueueStatusUnavailable => 'Plus disponible';

  @override
  String get pasteQueueEdited => 'Modifié';

  @override
  String get pasteQueueSecret => 'Secret';

  @override
  String get pasteQueueKindImage => 'Image';

  @override
  String get pasteQueueKindFiles => 'Fichiers';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Généré à $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Élément $position : $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'Déplacé en position $position';
  }

  @override
  String get pasteQueueMoveUp => 'Monter';

  @override
  String get pasteQueueMoveDown => 'Descendre';

  @override
  String get pasteQueueMoveToTop => 'Placer en premier';

  @override
  String get pasteQueueMoveToBottom => 'Placer en dernier';

  @override
  String get pasteQueueReorder => 'Faites glisser pour réorganiser';

  @override
  String get pasteQueueMoreActions => 'Plus d\'actions';

  @override
  String get pasteQueuePasteNow => 'Coller maintenant';

  @override
  String get pasteQueueEdit => 'Modifier le texte';

  @override
  String get pasteQueueUseOriginal => 'Utiliser le texte d\'origine';

  @override
  String get pasteQueueRemove => 'Retirer de la file';

  @override
  String get pasteQueueEditTitle => 'Modifier le texte de la file';

  @override
  String get pasteQueueEditMessage =>
      'Les modifications ne s’appliquent qu’à cette file.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Définir des raccourcis';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Rassemblez et collez sans ouvrir CopyPaste.';

  @override
  String get pasteQueueShortcutToggleCollecting =>
      'Démarrer ou arrêter la collecte';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Coller le suivant en texte brut';

  @override
  String get pasteQueueShortcutDescription =>
      'Un raccourci global qui fonctionne dans toutes les apps.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Un autre raccourci CopyPaste utilise cette combinaison. Le précédent est conservé.';

  @override
  String get pasteQueueShortcutInUse =>
      'Une autre app utilise cette combinaison.';

  @override
  String get pasteQueueShortcutRejected =>
      'Le système a refusé cette combinaison. La précédente est conservée.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt saisit des caractères sur de nombreuses dispositions. Choisissez-en une autre.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Les raccourcis globaux ne sont pas disponibles dans cette session.';

  @override
  String get pasteQueueShortcutFailed => 'Impossible de définir ce raccourci.';

  @override
  String get pasteQueueEnableTitle => 'Activer Paste Queue';

  @override
  String get pasteQueueEnableDescription =>
      'Désactivée, les raccourcis et la collecte s’arrêtent.';

  @override
  String get pasteQueuePasteNextStyleTitle => 'Coller le suivant';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Coller automatiquement ou seulement copier dans le presse-papiers.';

  @override
  String get pasteQueuePasteIntoApp => 'Coller dans l\'app au premier plan';

  @override
  String get pasteQueueClipboardOnly => 'Presse-papiers uniquement';

  @override
  String get pasteQueueInsertTitle => 'Les nouveaux éléments vont';

  @override
  String get pasteQueueInsertDescription =>
      'Où les éléments collectés et ajoutés rejoignent la file.';

  @override
  String get pasteQueueInsertEnd => 'À la fin (premier entré, premier sorti)';

  @override
  String get pasteQueueInsertStart => 'Au début (dernier entré, premier sorti)';

  @override
  String get pasteQueueEndBehaviorTitle => 'Après le dernier élément';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'Ce que fait Coller le suivant une fois tous les éléments collés.';

  @override
  String get pasteQueueEndStop => 'Arrêter';

  @override
  String get pasteQueueEndLoop => 'Recommencer';

  @override
  String get pasteQueueEndKeepLast => 'Continuer à coller le dernier';

  @override
  String get pasteQueueSeparatorTitle => 'Séparateur de Tout coller';

  @override
  String get pasteQueueSeparatorDescription =>
      'Placé entre les éléments lorsqu\'ils sont tous collés d\'un coup.';

  @override
  String get pasteQueueSeparatorNewLine => 'Nouvelle ligne';

  @override
  String get pasteQueueSeparatorBlankLine => 'Ligne vide';

  @override
  String get pasteQueueSeparatorTab => 'Tabulation';

  @override
  String get pasteQueueSeparatorSpace => 'Espace';

  @override
  String get pasteQueueSeparatorComma => 'Virgule';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Virgule et espace';

  @override
  String get pasteQueueSeparatorNone => 'Aucun';

  @override
  String get pasteQueueSeparatorCustom => 'Personnalisé';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'Jusqu\'à 32 caractères ; \\n et \\t autorisés';

  @override
  String get pasteQueuePlainTextTitle => 'Toujours coller en texte brut';

  @override
  String get pasteQueuePlainTextDescription =>
      'Supprime la mise en forme des éléments de la file.';

  @override
  String get pasteQueueIdleStopTitle => 'Arrêter la collecte après';

  @override
  String get pasteQueueIdleStopDescription =>
      'Arrêter la collecte après cette durée sans copie.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes min';
  }

  @override
  String get pasteQueueIdleNever => 'Jamais';

  @override
  String get pasteQueueMaxEntriesTitle => 'Taille maximale de la file';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Une fois pleine, les nouvelles copies ne sont plus ajoutées.';

  @override
  String get pasteQueueRestoreTitle =>
      'Restaurer le presse-papiers précédent à la fin';

  @override
  String get pasteQueueRestoreDescription =>
      'Rétablit ce que vous aviez copié avant de coller.';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue file collecter ordre raccourci séparateur';

  @override
  String get settingsSectionAbout => 'À propos';

  @override
  String get settingsSectionAboutSummary =>
      'Version et mises à jour de l’application.';

  @override
  String get settingsGeneralDescription =>
      'Langue, lancement automatique et visibilité de l’application.';

  @override
  String get settingsModulesDescription =>
      'Fonctionnalités facultatives pour CopyPaste.';

  @override
  String get settingsClipboardDescription =>
      'Capturez les modifications du presse-papiers et gérez les limites de l’historique.';

  @override
  String get settingsDataDescription =>
      'Exportez, sauvegardez et restaurez l’historique du presse-papiers.';

  @override
  String get settingsSyncDescription =>
      'Gérez la synchronisation avec les appareils associés.';

  @override
  String get settingsPrivacyDescription =>
      'Choisissez les apps capturées et protégez le contenu du presse-papiers.';

  @override
  String get settingsNotificationsDescription =>
      'Choisissez les notifications et les sons des éléments capturés.';

  @override
  String get settingsAboutDescription =>
      'Version de CopyPaste et mises à jour de l’application.';

  @override
  String get settingsQuickPasteDescription =>
      'Ouvrez l’historique du presse-papiers depuis n’importe où sans changer de fenêtre.';

  @override
  String get settingsLoadingMessage => 'Chargement des paramètres.';

  @override
  String get settingsUnavailableTitle =>
      'Les paramètres ne sont pas disponibles';

  @override
  String get settingsNoResultsTitle => 'Aucun paramètre trouvé';

  @override
  String get settingsNoResultsMessage => 'Essayez une autre recherche.';

  @override
  String get settingsAttentionTitle =>
      'Les paramètres requièrent votre attention';

  @override
  String get settingsSearchPlaceholder => 'Rechercher dans les paramètres';

  @override
  String get settingsBackToSettings => 'Retour aux paramètres';

  @override
  String get settingsStartOnBootTitle => 'Lancer au démarrage de l’appareil';

  @override
  String get settingsStartAtLoginTitle => 'Lancer à l’ouverture de session';

  @override
  String get settingsStartupSearchDescription =>
      'Lancer CopyPaste automatiquement.';

  @override
  String get settingsStartupKeywords =>
      'démarrage ouverture session connexion lancer automatiquement startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Désactivé dans le Gestionnaire des tâches. Activez CopyPaste dans Paramètres → Applications → Démarrage.';

  @override
  String get settingsStartupMacApproval =>
      'Autorisez CopyPaste dans Réglages Système → Général → Ouverture.';

  @override
  String get settingsStartupUnavailable =>
      'Le lancement automatique n’est pas disponible dans cette installation.';

  @override
  String get settingsStartupAndroidDescription =>
      'Reprendre les services en arrière-plan après un redémarrage.';

  @override
  String get settingsStartupDesktopDescription =>
      'Ouvrir CopyPaste automatiquement à l’ouverture de session.';

  @override
  String get settingsShowInDock => 'Afficher dans le Dock';

  @override
  String get settingsShowInTaskbar => 'Afficher dans la barre des tâches';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Afficher l’icône de l’application pour passer d’une fenêtre à l’autre.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'dock barre des tâches application icône visibilité dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Afficher dans la barre des menus';

  @override
  String get settingsShowInSystemTray =>
      'Afficher dans la zone de notification';

  @override
  String get settingsTrayVisibilityDescription =>
      'Afficher un accès rapide à CopyPaste et aux commandes de capture.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'zone de notification barre des menus état icône visibilité tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle =>
      'Impossible de modifier la visibilité';

  @override
  String get settingsModulesSearchDescription =>
      'Installer, mettre à jour et supprimer des modules facultatifs.';

  @override
  String get settingsModulesKeywords =>
      'fonctionnalités commandes extensions features commands extensions';

  @override
  String get settingsAndroidCaptureTitle => 'Capture en arrière-plan Android';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Capture complète ou limitée, configurée avec Shizuku ou ADB.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'arrière-plan autorisations configuration background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Mode Complet ou Limité et configuration unique via Shizuku ou ADB.';

  @override
  String get settingsOpenSetup => 'Ouvrir la configuration';

  @override
  String get settingsSaveScreenshotsTitle => 'Enregistrer les captures d’écran';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Enregistrer automatiquement les nouvelles captures d’écran Android dans l’Historique.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'images photos capture autorisations images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Ajouter automatiquement les nouvelles captures d’écran à l’Historique.';

  @override
  String get settingsScreenshotAccessTitle => 'Accès aux captures d’écran';

  @override
  String get settingsScreenshotAccessDescription =>
      'Nécessaire pour enregistrer les captures en arrière-plan.';

  @override
  String get settingsAllowAccess => 'Autoriser l’accès';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'L’enregistrement des captures d’écran est à l’arrêt';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'Les nouvelles captures d’écran ne sont pas enregistrées. Reprenez la capture pour relancer le service.';

  @override
  String get settingsResumeCapture => 'Reprendre la capture';

  @override
  String get settingsClipboardCaptureTitle => 'Capture du presse-papiers';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Suspendre ou reprendre la capture du presse-papiers.';

  @override
  String get settingsCaptureStateUnavailable => 'État indisponible';

  @override
  String get settingsCaptureStoppedTitle => 'La capture est à l’arrêt';

  @override
  String get settingsCaptureStoppedMessage =>
      'CopyPaste ne reçoit actuellement pas les modifications du presse-papiers sur cet appareil.';

  @override
  String get settingsRetentionTitle => 'Conservation';

  @override
  String get settingsRetentionDescription =>
      'Supprimer automatiquement les anciens éléments non épinglés du presse-papiers.';

  @override
  String get settingsRetentionForever => 'Conserver indéfiniment';

  @override
  String get settingsStorageQuotaTitle => 'Quota de stockage';

  @override
  String get settingsStorageQuotaDescription =>
      'Espace de stockage local maximal utilisé par l’historique non épinglé.';

  @override
  String get settingsStorageQuotaKeywords =>
      'disque espace limite disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Taille de fichier maximale';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Limite pour l’ajout et le téléchargement de fichiers.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'importer télécharger transfert taille limite import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Taille maximale des fichiers à ajouter ou télécharger.';

  @override
  String get settingsHistoryFilesTitle => 'Fichiers d’historique';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Exporter l’historique, créer des sauvegardes ou restaurer une sauvegarde.';

  @override
  String get settingsHistoryFilesKeywords =>
      'texte chiffré sauvegarde données text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Exporter l’historique texte';

  @override
  String get settingsExportTextDescription =>
      'Enregistrer les éléments texte dans un fichier portable.';

  @override
  String get settingsExportAction => 'Exporter';

  @override
  String get settingsBackupTitle => 'Sauvegarde chiffrée';

  @override
  String get settingsBackupDescription =>
      'Enregistrer l’historique local complet de cet appareil.';

  @override
  String get settingsCreateBackupAction => 'Créer une sauvegarde';

  @override
  String get settingsRestoreTitle => 'Restaurer une sauvegarde';

  @override
  String get settingsRestoreDescription =>
      'Remplacer l’historique local par une sauvegarde chiffrée.';

  @override
  String get settingsRestoreAction => 'Restaurer';

  @override
  String get settingsRestoreDialogTitle => 'Remplacer l’historique local ?';

  @override
  String get settingsRestoreDialogMessage =>
      'La sauvegarde remplace l’historique de cet appareil. Les réglages sont conservés.';

  @override
  String get settingsRestoreFailed => 'Impossible de restaurer la sauvegarde.';

  @override
  String get settingsSyncSwitchTitle => 'Synchronisation';

  @override
  String get settingsSyncSwitchDescription =>
      'Synchroniser l’historique avec les appareils associés.';

  @override
  String get settingsSyncSearchDescription =>
      'Synchronisation des appareils associés.';

  @override
  String get settingsInstantClipboardTitle => 'Presse-papiers instantané';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Copier automatiquement les nouveaux éléments provenant d’autres appareils.';

  @override
  String get settingsInstantClipboardKeywords =>
      'automatique copier coller synchronisation reçu automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Copier ici les nouveaux éléments des autres appareils.';

  @override
  String get settingsLanVisibilityTitle => 'Visibilité sur le réseau local';

  @override
  String get settingsLanVisibilityDescription =>
      'Permettre aux appareils à proximité de détecter cet appareil.';

  @override
  String get settingsLanVisibilityKeywords =>
      'réseau local détection local network discovery';

  @override
  String get settingsExcludedAppsTitle => 'Applications exclues';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Ignorer la capture automatique depuis les applications exclues.';

  @override
  String get settingsExcludedAppsKeywords =>
      'confidentialité app identifiants privacy app identifiers';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Ignorer la capture automatique de ces applications.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'La capture automatique est suspendue tant qu’une app est exclue.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'Les exclusions d’applications sont prises en charge sur macOS, Windows, Android et Linux.';

  @override
  String get settingsExcludedAppsAdd => 'Ajouter';

  @override
  String get settingsExcludedAppsEmpty => 'Aucune application n’est exclue.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return 'Retirer $app';
  }

  @override
  String get settingsExcludeDialogTitle => 'Exclure une application';

  @override
  String get settingsExcludeDialogAction => 'Exclure';

  @override
  String get settingsExcludeDialogPlaceholder => 'Identifiant de l’application';

  @override
  String get settingsExcludeFailed => 'Impossible d’exclure l’application.';

  @override
  String get settingsSkipSecretTitle =>
      'Ignorer le presse-papiers confidentiel';

  @override
  String get settingsSkipSecretDescription =>
      'Ne pas enregistrer les copies marquées secrètes par les apps.';

  @override
  String get settingsSkipSecretKeywords =>
      'secret mot de passe sensible masqué spoiler secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle =>
      'Ignorer le presse-papiers temporaire';

  @override
  String get settingsSkipTransientDescription =>
      'Ne pas enregistrer les copies marquées temporaires par les apps.';

  @override
  String get settingsSkipTransientKeywords =>
      'transitoire temporaire confidentialité transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Bloquer les captures d’écran';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Empêcher les captures et les enregistrements d’écran de CopyPaste.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'Le blocage des captures d’écran n’est pas disponible sur cette plateforme.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'confidentialité écran capture protection association qr sécurité code privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Notification lors de la copie';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Afficher une notification après une capture en arrière-plan.';

  @override
  String get settingsNotificationPreviewTitle =>
      'Afficher le contenu du presse-papiers';

  @override
  String get settingsNotificationPreviewDescription =>
      'Inclure un aperçu de l’élément dans les notifications de copie.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'notification aperçu texte image confidentialité notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Son lors de la copie';

  @override
  String get settingsSoundOnCopyDescription =>
      'Émettre le signal du système après une capture réussie.';

  @override
  String get settingsVersionTitle => 'Version';

  @override
  String get settingsVersionDescription => 'Version de CopyPaste installée.';

  @override
  String get settingsVersionUnavailable => 'Indisponible';

  @override
  String get settingsUpdatesTitle => 'Mises à jour de l’application';

  @override
  String get settingsUpdatesSearchDescription =>
      'Rechercher et installer les mises à jour de CopyPaste.';

  @override
  String get settingsUpdatesKeywords =>
      'version github publication version github release';

  @override
  String get settingsUpdateIdle =>
      'Rechercher une version plus récente de CopyPaste.';

  @override
  String get settingsUpdateChecking => 'Recherche de mises à jour.';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste est à jour.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'La version $version est à jour.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'La version $available est disponible. Cet appareil dispose de la version $current.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Téléchargement de la version $version · $percent %.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Préparation des mises à jour des modules installés.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Installation de la version $version.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'Les mises à jour de l’application requièrent votre attention.';

  @override
  String get settingsUpdateNowAction => 'Mettre à jour';

  @override
  String get settingsUpdateOpenReleaseAction => 'Ouvrir la page de la version';

  @override
  String get settingsUpdateCheckingAction => 'Recherche';

  @override
  String get settingsUpdateDownloadingAction => 'Téléchargement';

  @override
  String get settingsUpdateInstallingAction => 'Installation';

  @override
  String get settingsUpdateRestartAction => 'Redémarrer CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => 'Rechercher à nouveau';

  @override
  String get settingsUpdateNoneTitle => 'Aucune mise à jour disponible';

  @override
  String get settingsUpdateLatest =>
      'Vous disposez de la dernière version de CopyPaste.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version est la dernière version.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Mise à jour disponible';

  @override
  String get settingsUpdateReady =>
      'Une version plus récente de CopyPaste est prête à être installée.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version est prêt à être installé.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Mise à jour indisponible';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Cette installation ne peut pas se mettre à jour automatiquement.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Échec de la recherche de mises à jour';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'CopyPaste n’a pas pu rechercher de mises à jour.';

  @override
  String get settingsQuickPasteSetupTitle => 'Configurer Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Activez le compagnon signé GNOME ou KDE et autorisez le contrôle du clavier pour revenir à l’app précédente.';

  @override
  String get settingsQuickPasteDesktopIntegration => 'Intégration au bureau';

  @override
  String get settingsQuickPasteAllowKeyboard =>
      'Autoriser le contrôle du clavier';

  @override
  String get settingsQuickPasteCheckAgain => 'Vérifier à nouveau';

  @override
  String get settingsQuickPasteLoading =>
      'Chargement des paramètres de Quick Paste.';

  @override
  String get settingsQuickPasteUnavailableTitle =>
      'Quick Paste n’est pas disponible';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'Cet appareil ne propose pas la fenêtre Quick Paste.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Ouvrir Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Configurer le raccourci global de Quick Paste.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'clavier raccourci keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'Fonctionne tant que CopyPaste tourne en arrière-plan.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Raccourci enregistré : $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Coller automatiquement';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Coller l’élément sélectionné dans l’app précédente.';

  @override
  String get settingsAutoPasteKeywords =>
      'accessibilité automatique accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Sélectionner un élément ramène à l’app précédente et l’y colle.';

  @override
  String get settingsAccessibilityRequiredTitle =>
      'L’autorisation Accessibilité est requise';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Tant que l’autorisation n’est pas accordée, sélectionner un élément le copie sans le coller.';

  @override
  String get settingsQuickPasteAttentionTitle =>
      'Quick Paste requiert votre attention';

  @override
  String get settingsShortcutRecording => 'Appuyez sur le raccourci…';

  @override
  String get settingsShortcutNotSet => 'Non défini';

  @override
  String get settingsShortcutReset => 'Réinitialiser le raccourci';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count jours',
      many: '$count jours',
      one: '$count jour',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Chargement des appareils…';

  @override
  String get devicesUnavailableTitle => 'Les appareils ne sont pas disponibles';

  @override
  String get devicesNearbyTitle => 'Appareils à proximité';

  @override
  String get devicesScan => 'Rechercher';

  @override
  String get devicesScanning => 'Recherche…';

  @override
  String get devicesNoneNearby => 'Aucun appareil à proximité';

  @override
  String get devicesThisDevice => 'Cet appareil';

  @override
  String get devicesTrusted => 'Approuvé';

  @override
  String get devicesConnect => 'Connecter';

  @override
  String devicesCardPing(String ping) {
    return 'Ping : $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Ping :\n$ping';
  }

  @override
  String get devicesRenameDialogTitle => 'Renommer cet appareil';

  @override
  String get devicesNamePlaceholder => 'Nom de l’appareil';

  @override
  String get devicesRenameFailed =>
      'Impossible d’enregistrer le nom de l’appareil.';

  @override
  String devicesRevokeDialogTitle(String device) {
    return 'Révoquer $device ?';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return 'Dissocier $device ?';
  }

  @override
  String get devicesRevokeDialogMessage =>
      'L’appareil sera retiré de votre groupe et cette association sera définitivement bloquée. Les autres appareils appliqueront le retrait lors de leur reconnexion.';

  @override
  String get devicesUnpairDialogMessage =>
      'L’appareil sera retiré de votre groupe. Les autres appareils appliqueront le retrait lors de leur reconnexion. Pour l’ajouter de nouveau, une nouvelle confirmation sera nécessaire.';

  @override
  String get devicesRevoke => 'Révoquer';

  @override
  String get devicesUnpair => 'Dissocier';

  @override
  String get devicesActionFailedMessage =>
      'Impossible d’effectuer l’action sur l’appareil.';

  @override
  String get devicesCloseDetails => 'Fermer les détails de l’appareil';

  @override
  String get devicesDetailType => 'Type d’appareil';

  @override
  String get devicesDetailOperatingSystem => 'Système d’exploitation';

  @override
  String get devicesDetailModel => 'Modèle';

  @override
  String get devicesDetailAppVersion => 'Version de CopyPaste';

  @override
  String get devicesDetailProtocolVersion => 'Version du protocole';

  @override
  String get devicesDetailStatus => 'État';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => 'Date de mesure';

  @override
  String get devicesDetailLastSeen => 'Vu pour la dernière fois';

  @override
  String get devicesDetailLanEndpoint => 'Adresse sur le réseau local';

  @override
  String get devicesDetailDeviceId => 'Identifiant de l’appareil';

  @override
  String get devicesDetailPairingId => 'Identifiant de l’association';

  @override
  String get devicesDetailProfileProvenance => 'Provenance du profil';

  @override
  String get devicesDetailProfileTrust => 'Fiabilité du profil';

  @override
  String get devicesDetailProfileUpdated => 'Profil mis à jour';

  @override
  String get devicesValueUnavailable => 'Indisponible';

  @override
  String get devicesRenameDevice => 'Renommer l’appareil';

  @override
  String get devicesSyncNow => 'Synchroniser maintenant';

  @override
  String get devicesRevokePairing => 'Révoquer l’association';

  @override
  String get devicesLocal => 'Local';

  @override
  String get devicesTrustAuthenticated => 'Authentifié';

  @override
  String get devicesTrustUnverified => 'Non vérifié';

  @override
  String get devicesProvenanceSelfReported => 'Déclaré par l’appareil';

  @override
  String get devicesProvenanceObserved => 'Observé';

  @override
  String get devicesProvenanceMeasured => 'Mesuré';

  @override
  String get devicesConnectionOnline => 'En ligne';

  @override
  String get devicesConnectionOffline => 'Hors ligne';

  @override
  String get devicesConnectionVisible => 'Visible';

  @override
  String get devicesConnectionNotSeen => 'Non détecté';

  @override
  String get devicesConnectionUnknown => 'Inconnu';

  @override
  String get devicesPeerAvailable => 'Disponible';

  @override
  String get devicesPeerVisibleNow => 'Visible actuellement';

  @override
  String get devicesPeerStatusUnknown => 'État inconnu';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => 'Ordinateur de bureau';

  @override
  String get devicesClassLaptop => 'Ordinateur portable';

  @override
  String get devicesClassPhone => 'Téléphone';

  @override
  String get devicesClassTablet => 'Tablette';

  @override
  String get devicesClassUnknown => 'Appareil';

  @override
  String get devicesUnknownOs => 'Système inconnu';

  @override
  String get devicesAddDevice => 'Ajouter un appareil';

  @override
  String get devicesScanQr => 'Scanner un code QR';

  @override
  String get devicesEnterCode => 'Saisir un code';

  @override
  String get devicesScannerUnavailableTitle => 'Scanner indisponible';

  @override
  String get devicesActionFailedTitle => 'Échec de l’action';

  @override
  String get devicesRefreshFailedTitle =>
      'Échec de l’actualisation des appareils';

  @override
  String get devicesNearbyPairingUnavailable =>
      'L’association à proximité n’est pas disponible.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'L’association à proximité n’est pas disponible. Utilisez « Ajouter un appareil » pour afficher un code d’association.';

  @override
  String get pairingTitle => 'Association';

  @override
  String get pairingPairDevice => 'Associer un appareil';

  @override
  String get pairingScanQrCode => 'Scanner un code QR';

  @override
  String get pairingEnterPairingCode => 'Saisir le code d’association';

  @override
  String get pairingConnectNearbyDevice => 'Connecter un appareil à proximité';

  @override
  String get pairingSendingRequest => 'Envoi de la demande de connexion…';

  @override
  String get pairingEmptyTitle => 'Choisissez une méthode d’association';

  @override
  String get pairingEmptyMessage =>
      'Les commandes d’association s’affichent ici.';

  @override
  String get pairingCreatingInvitation =>
      'Création d’une invitation d’association.';

  @override
  String get pairingCode => 'Code d’association';

  @override
  String get pairingCodeHint =>
      'Saisissez le code à 8 caractères affiché sur l’autre appareil.';

  @override
  String get pairingDeviceAddress => 'Adresse de l’appareil';

  @override
  String get pairingDeviceAddressHint =>
      'Utilisez l’adresse host:port indiquée par l’autre appareil.';

  @override
  String get pairingCheckDetailsTitle =>
      'Vérifiez les informations d’association';

  @override
  String get pairingActionFailed => 'Échec de l’action d’association';

  @override
  String get pairingConnectingDevice => 'Connexion de l’appareil…';

  @override
  String get pairingQrScannerTitle => 'Scanner de code QR';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'L’aperçu de la caméra s’affiche ici.';

  @override
  String get pairingScannerNeedsAttention =>
      'Le scanner requiert votre attention';

  @override
  String get pairingCodeIncomplete =>
      'Saisissez le code d’association complet à 8 caractères.';

  @override
  String get pairingAddressMissing =>
      'Saisissez l’adresse host:port de l’appareil.';

  @override
  String get pairingWaitingForOtherDevice => 'En attente de l’autre appareil';

  @override
  String get pairingInviteInstructions =>
      'Sur l’autre appareil, choisissez « Ajouter un appareil » → « Saisir un code », ou scannez le code QR.';

  @override
  String get pairingDetailsTitle => 'Informations d’association';

  @override
  String get pairingAddress => 'Adresse';

  @override
  String get pairingVerificationCode => 'Code de vérification';

  @override
  String get pairingVerificationCodeHint =>
      'Ne confirmez que si ce code est identique sur les deux appareils.';

  @override
  String get pairingReject => 'Refuser';

  @override
  String get pairingConfirm => 'Confirmer';

  @override
  String get pairingAccept => 'Accepter';

  @override
  String get pairingJoinDevice => 'Associer l’appareil';

  @override
  String get pairingStartScanner => 'Lancer le scanner';

  @override
  String get pairingActionNotStarted =>
      'Impossible de lancer l’action d’association.';

  @override
  String get pairingStateHandshaking => 'Vérification de l’appareil';

  @override
  String get pairingStateAwaitingConfirmation =>
      'Confirmez sur les deux appareils';

  @override
  String get pairingStateConfirmed => 'Appareil associé';

  @override
  String get pairingStateRejected => 'Association refusée';

  @override
  String get pairingStateCancelled => 'Association annulée';

  @override
  String get pairingStateTimedOut => 'Association expirée';

  @override
  String get pairingStateFailed => 'Échec de l’association';

  @override
  String get pairingHandshakingDescription =>
      'Les appareils établissent un canal sécurisé.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Comparez le code de vérification protégé, puis confirmez sur les deux appareils.';

  @override
  String get pairingConfirmedDescription =>
      'L’appareil est connecté. L’appartenance à votre groupe sera synchronisée automatiquement.';

  @override
  String get pairingNoDeviceAdded => 'Aucun appareil n’a été ajouté.';

  @override
  String get pairingTimedOutDescription =>
      'Lancez une nouvelle tentative d’association pour continuer.';

  @override
  String get pairingIdleDescription =>
      'Lancez une tentative d’association pour connecter un appareil.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'L’association protégée n’est pas disponible';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Cette session d’association doit s’ouvrir dans une fenêtre protégée dédiée.';

  @override
  String get pairingCancel => 'Annuler l’association';

  @override
  String get pairingOrEnterCode => 'Ou saisissez un code d’association';

  @override
  String get pairingJoinSecurely => 'Associer en toute sécurité';

  @override
  String get pairingProtectedWaitingDescription =>
      'Affichez un code QR, scannez-en un ou saisissez un code d’association dans cette fenêtre protégée.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Expire dans $seconds s';
  }

  @override
  String get pairingInvalidQr => 'Scannez un code QR d’association CopyPaste.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'Le scanner Google n’est pas disponible. Saisissez plutôt le code d’association.';

  @override
  String get pairingScannerCouldNotOpen =>
      'Impossible d’ouvrir le scanner. Saisissez plutôt le code d’association.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'L’affichage sécurisé de l’association n’est pas disponible.';

  @override
  String get pairingVerificationCodeRequired =>
      'Affichez et comparez d’abord le code de vérification.';

  @override
  String get syncStatusUnavailable => 'État de la synchronisation indisponible';

  @override
  String get syncStatusOff => 'Synchronisation désactivée';

  @override
  String get syncStatusWaitingForSync => 'En attente de synchronisation';

  @override
  String get syncStatusSyncing => 'Synchronisation en cours';

  @override
  String get syncStatusUpToDate => 'À jour';

  @override
  String get syncStatusInterrupted => 'Synchronisation interrompue';

  @override
  String get syncStatusPartiallySynchronized => 'Partiellement synchronisé';

  @override
  String get syncStatusRetrying => 'Nouvelle tentative';

  @override
  String get syncStatusWaitingToSync => 'Synchronisation en attente';

  @override
  String get syncStatusWaitingForDevice => 'En attente de l’appareil';

  @override
  String get syncStatusNoPairedDevices => 'Aucun appareil associé';

  @override
  String get syncStatusDevicesOffline => 'Appareils hors ligne';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · journal d’activité indisponible';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Synchronisation désactivée · $count problèmes',
      many: 'Synchronisation désactivée · $count problèmes',
      one: 'Synchronisation désactivée · $count problème',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count hors ligne',
      many: '$status · $count hors ligne',
      one: '$status · $count hors ligne',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count appareils requièrent votre attention',
      many: '$count appareils requièrent votre attention',
      one: '$count appareil requiert votre attention',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Appareils hors ligne · $count problèmes',
      many: 'Appareils hors ligne · $count problèmes',
      one: 'Appareils hors ligne · $count problème',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Vérification de la connexion';

  @override
  String get syncConnectionUnknown => 'Connexion inconnue';

  @override
  String get syncLogDeviceOffline => 'L’appareil est passé hors ligne.';

  @override
  String get syncLogConnectionRestored => 'Connexion rétablie.';

  @override
  String get syncLogProblemDetected => 'Problème de synchronisation détecté.';

  @override
  String get syncLogProblemResolved => 'Problème de synchronisation résolu.';

  @override
  String get syncLogFailed => 'Échec de la synchronisation.';

  @override
  String get syncLogRecovered => 'Synchronisation rétablie.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent éléments envoyés',
      many: '$sent éléments envoyés',
      one: '$sent élément envoyé',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received éléments reçus',
      many: '$received éléments reçus',
      one: '$received élément reçu',
    );
    return '$_temp0 · $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Synchronisation : $status';
  }

  @override
  String get syncDetailsTitle => 'Synchronisation';

  @override
  String get syncDetailsClose => 'Fermer les détails de la synchronisation';

  @override
  String get syncNoPairedDevicesMessage =>
      'Associez un appareil pour synchroniser l’historique du presse-papiers.';

  @override
  String get syncActivityLogUnavailableTitle =>
      'Journal d’activité indisponible';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Impossible de lire ou d’enregistrer l’activité de synchronisation.';

  @override
  String get syncDetailsDevicesHeading => 'Appareils';

  @override
  String get syncDetailsActivityHeading => 'Activité';

  @override
  String get syncDetailsNoRecentProblems =>
      'Aucun problème de synchronisation récent';

  @override
  String get syncDetailsUnknownPeer => 'Appareil associé';

  @override
  String get syncPeerConnection => 'Connexion';

  @override
  String get syncPeerLastCheckStarted => 'Début de la dernière vérification';

  @override
  String get syncPeerItemsOverSizeLimit =>
      'Éléments dépassant la limite de taille';

  @override
  String syncPeerLastSeen(String time) {
    return 'Dernière activité : $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Dernière synchronisation : $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Dernière synchronisation réussie';

  @override
  String get syncPeerConnectionDetails => 'Détails de la connexion';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Détails de la connexion de $device';
  }

  @override
  String get historyKindText => 'Texte';

  @override
  String get historyKindLink => 'Lien';

  @override
  String get historyKindEmail => 'E-mail';

  @override
  String get historyKindColor => 'Couleur';

  @override
  String get historyKindPhone => 'Téléphone';

  @override
  String get historyKindCode => 'Code';

  @override
  String get historyKindPath => 'Chemin';

  @override
  String get historyKindCredentials => 'Identifiants';

  @override
  String get historyKindImage => 'Image';

  @override
  String get historyKindFile => 'Fichier';

  @override
  String get historyKindOther => 'Autre';

  @override
  String get historySortNewest => 'Plus récents d’abord';

  @override
  String get historySortOldest => 'Plus anciens d’abord';

  @override
  String get historySortRelevance => 'Pertinence';

  @override
  String get historyErrorUpdatesUnavailable =>
      'Les mises à jour de l’historique sont temporairement indisponibles.';

  @override
  String get historyErrorBulkDelete =>
      'Impossible de supprimer certains éléments sélectionnés. Réessayez.';

  @override
  String get historyErrorBulkPin =>
      'Impossible de modifier l’épinglage de certains éléments sélectionnés. Réessayez.';

  @override
  String get historyErrorRefresh =>
      'Impossible d’actualiser l’historique. Réessayez.';

  @override
  String get historyErrorLoad =>
      'Impossible de charger l’historique. Réessayez.';

  @override
  String get historyErrorLoadMore =>
      'Impossible de charger les éléments plus anciens.';

  @override
  String get historyErrorLoadEarlier =>
      'Impossible de charger l’historique précédent.';

  @override
  String get historyErrorOpenClip => 'Impossible d’ouvrir cet élément.';

  @override
  String get historyErrorCopyPlainText => 'Impossible de copier le texte brut.';

  @override
  String get historyErrorCopyClip => 'Impossible de copier cet élément.';

  @override
  String get historyErrorCopyText => 'Impossible de copier ce texte.';

  @override
  String get historyErrorPrepareFile =>
      'Impossible de télécharger ce fichier. Vérifiez l’appareil source et réessayez.';

  @override
  String get historyErrorDownloadFile =>
      'Impossible de télécharger ce fichier.';

  @override
  String get historyErrorSelectFiles =>
      'Impossible de sélectionner les fichiers. Réessayez.';

  @override
  String get historyErrorReadSelectedFile =>
      'Impossible de lire le fichier sélectionné. Vérifiez l’accès et l’espace de stockage disponible.';

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
          'Échec de l’importation : $failed sur $total fichiers. Premier : $name. $reason',
      many:
          'Échec de l’importation : $failed sur $total fichiers. Premier : $name. $reason',
      one:
          'Échec de l’importation de $failed fichier sur $total. Premier : $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'Impossible de modifier la confidentialité de l’élément.';

  @override
  String get historyErrorPinUpdate => 'Impossible de modifier l’épinglage.';

  @override
  String get historyErrorPinnedOrderSave =>
      'Impossible d’enregistrer l’ordre des éléments épinglés. Réessayez.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'L’ordre a été enregistré, mais l’historique n’a pas pu être actualisé.';

  @override
  String get historyErrorDeleteClip => 'Impossible de supprimer cet élément.';

  @override
  String get historyErrorDeleteAll => 'Impossible de supprimer l’historique.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'Impossible de supprimer les éléments sélectionnés.';

  @override
  String get historyOcrErrorUnavailable =>
      'L’OCR n’est pas disponible. Vérifiez le module dans Paramètres et réessayez.';

  @override
  String get historyOcrErrorStart => 'Impossible de lancer l’OCR. Réessayez.';

  @override
  String get historyOcrErrorRecognize =>
      'Impossible de reconnaître le texte de l’image. Réessayez.';

  @override
  String get historyOcrRecognizing => 'Reconnaissance du texte de l’image.';

  @override
  String get historyOcrFailedTitle => 'Échec de l’OCR';

  @override
  String get historyOcrNoText => 'Aucun texte trouvé';

  @override
  String get historyOcrTitle => 'Texte de l’image';

  @override
  String get historyOcrTextCopied => 'Texte reconnu copié.';

  @override
  String get historyOcrCopyError => 'Impossible de copier le texte. Réessayez.';

  @override
  String get historyTagsErrorLoad =>
      'Impossible de charger les étiquettes. Réessayez.';

  @override
  String get historyTagsErrorSave =>
      'Impossible d’enregistrer la modification de l’étiquette. Utilisez un nom unique de 64 caractères maximum et sélectionnez des éléments épinglés.';

  @override
  String get historyTags => 'Étiquettes';

  @override
  String get historyTagsUpdateErrorTitle =>
      'Impossible de modifier les étiquettes';

  @override
  String get historyTagsEmpty => 'Aucune étiquette';

  @override
  String get historyTagRename => 'Renommer';

  @override
  String get historyTagActions => 'Actions de l’étiquette';

  @override
  String get historyTagNamePlaceholder => 'Nom de l’étiquette';

  @override
  String get historyCreateTag => 'Créer l’étiquette';

  @override
  String get historySaveTag => 'Enregistrer l’étiquette';

  @override
  String get historyTagsFilterAll => 'Toutes les étiquettes';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count étiquettes',
      many: '$count étiquettes',
      one: '$count étiquette',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Filtrer par étiquettes : $selection';
  }

  @override
  String get historyManageTags => 'Gérer les étiquettes';

  @override
  String get historyTagsFilterSemantics =>
      'Filtrer les éléments épinglés par étiquettes';

  @override
  String get historyDownloadErrorCancelled =>
      'Le téléchargement du fichier a été annulé.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'Le service de téléchargement n’est pas disponible. Rouvrez CopyPaste et réessayez.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'Le service de téléchargement ne s’est pas connecté. Réessayez.';

  @override
  String get historyFileErrorAccess =>
      'Impossible d’accéder au fichier sélectionné.';

  @override
  String get historySpoilerRevealSemantics =>
      'Contenu confidentiel. Afficher le contenu masqué';

  @override
  String get historyActionFailedTitle => 'Échec de l’action';

  @override
  String get historyLoadingMessage =>
      'Chargement de l’historique du presse-papiers.';

  @override
  String get historyUnavailableTitle => 'L’historique n’est pas disponible';

  @override
  String get historyEmptyTitle => 'Aucun élément trouvé';

  @override
  String get historyEmptyMessage =>
      'Les éléments capturés et l’historique correspondant s’affichent ici.';

  @override
  String get historyAttentionTitle => 'L’historique requiert votre attention';

  @override
  String get historyLoadMore => 'Charger plus';

  @override
  String get historySectionPinned => 'Épinglés';

  @override
  String get historySectionToday => 'Aujourd’hui';

  @override
  String get historySectionYesterday => 'Hier';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count lignes du presse-papiers ignorées',
      many: '$count lignes du presse-papiers ignorées',
      one: '$count ligne du presse-papiers ignorée',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Certains éléments conservés n’ont pas pu être lus et ne figurent pas dans cette liste.';

  @override
  String get historySearch => 'Rechercher dans l’historique';

  @override
  String get historyResetFilters => 'Réinitialiser les filtres';

  @override
  String get historyImportFiles => 'Importer des fichiers';

  @override
  String get historySelectClips => 'Sélectionner des éléments';

  @override
  String get historyCloseSearch => 'Fermer la recherche';

  @override
  String get historyFilterAllClips => 'Tous les éléments';

  @override
  String get historyFilterAllDevices => 'Tous les appareils';

  @override
  String get historyFilterAllApps => 'Toutes les apps';

  @override
  String get historyPinClip => 'Épingler l’élément';

  @override
  String get historyUnpinClip => 'Désépingler l’élément';

  @override
  String get historyReorderPinnedClip => 'Réorganiser l’élément épinglé';

  @override
  String get historyReorderPinnedHint =>
      'Faites glisser pour déplacer. Alt + Flèche haut ou Flèche bas déplace d’une position.';

  @override
  String get historyClipUnavailable => 'Élément indisponible';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Type d’élément : $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Sélectionnez un élément';

  @override
  String get historyInspectorEmptyMessage =>
      'Son contenu complet et ses actions s’affichent ici.';

  @override
  String get historyNoPreview => 'Aucun aperçu disponible.';

  @override
  String get historyCloseClipDetails => 'Fermer les détails de l’élément';

  @override
  String get historyCopyOptions => 'Options de copie';

  @override
  String get historyCopyPlainText => 'Copier en texte brut';

  @override
  String get historyManageClipTags => 'Gérer les étiquettes de l’élément';

  @override
  String get historyPinnedButton => 'Épinglé';

  @override
  String get historyAddSpoiler => 'Masquer le contenu';

  @override
  String get historyRemoveSpoiler => 'Ne plus masquer';

  @override
  String get historyUpdateFailedTitle => 'Échec de la modification';

  @override
  String get historyRecognizeImageText => 'Reconnaître le texte de l’image';

  @override
  String get historyCopiedTitle => 'Copié';

  @override
  String get historyCopyFailedTitle => 'Échec de la copie';

  @override
  String get historyPlainTextCopied => 'Texte brut copié.';

  @override
  String get historyClipCopied => 'Élément copié.';

  @override
  String get historyDownloadedTitle => 'Téléchargé';

  @override
  String get historyFileSaved => 'Fichier enregistré.';

  @override
  String get historyDownload => 'Télécharger';

  @override
  String get historyDownloading => 'Téléchargement…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Téléchargement : $percent %';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent %';
  }

  @override
  String get historyDownloadCancelling => 'Annulation…';

  @override
  String get historyDownloadCancellingTooltip => 'Annulation du téléchargement';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Téléchargement : $percent % · Cliquez pour annuler';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Échec du téléchargement · Cliquez pour réessayer';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Annuler le téléchargement, $percent % effectués';
  }

  @override
  String get historyRetryDownload => 'Relancer le téléchargement';

  @override
  String get historyDownloadFailedTitle => 'Échec du téléchargement';

  @override
  String get historyShowFile => 'Afficher le fichier';

  @override
  String get historyMetadataPrivacy => 'Confidentialité';

  @override
  String get historyMetadataConfidential => 'Confidentiel';

  @override
  String get historyMetadataCaptured => 'Date de capture';

  @override
  String get historyMetadataObservedApp => 'App détectée';

  @override
  String get historyMetadataDevice => 'Appareil';

  @override
  String get historyMetadataClipType => 'Type d’élément';

  @override
  String get historyMetadataType => 'Type';

  @override
  String get historyMetadataPath => 'Chemin';

  @override
  String get historyMetadataFile => 'Fichier';

  @override
  String get historyMetadataResolution => 'Résolution';

  @override
  String get historyMetadataSize => 'Taille';

  @override
  String get historyMetadataFiles => 'Fichiers';

  @override
  String get historyMetadataSync => 'Synchronisation';

  @override
  String get historyMetadataTooLargeToSync =>
      'Trop volumineux pour être synchronisé';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Supprimer $count éléments sélectionnés ?',
      many: 'Supprimer $count éléments sélectionnés ?',
      one: 'Supprimer $count élément sélectionné ?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Supprimer cet élément ?';

  @override
  String get historyBulkTagTooltip =>
      'Étiqueter les éléments épinglés sélectionnés';

  @override
  String get historyBulkExit => 'Quitter la sélection';

  @override
  String get historyBulkPin => 'Épingler la sélection';

  @override
  String get historyBulkUnpin => 'Désépingler la sélection';

  @override
  String get historyBulkDelete => 'Supprimer la sélection';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count sélectionnés',
      many: '$count sélectionnés',
      one: '$count sélectionné',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Filtrer par date';

  @override
  String get historyDatePresetToday => 'Aujourd’hui';

  @override
  String get historyDatePresetYesterday => 'Hier';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: '$days derniers jours',
      many: '$days derniers jours',
      one: '$days dernier jour',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'Toutes les dates';

  @override
  String get appRuntimeStartFailed =>
      'CopyPaste n’a pas pu démarrer son environnement d’exécution.';

  @override
  String get appPreparing => 'Préparation de CopyPaste.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste attend l’accès à l’historique protégé. Les parties disponibles de l’app restent utilisables.';

  @override
  String get appHistoryUnavailableTitle =>
      'Le service Historique n’est pas disponible';

  @override
  String get appHistoryUnavailableMessage =>
      'Démarrez l’environnement d’exécution de l’application avant d’ouvrir l’historique.';

  @override
  String get appDevicesUnavailableTitle =>
      'Le service Appareils n’est pas disponible';

  @override
  String get appDevicesUnavailableMessage =>
      'Démarrez l’environnement d’exécution de l’application avant d’ouvrir les appareils.';

  @override
  String get appSettingsUnavailableTitle =>
      'Le service Paramètres n’est pas disponible';

  @override
  String get appSettingsUnavailableMessage =>
      'Démarrez l’environnement d’exécution de l’application avant d’ouvrir les paramètres.';

  @override
  String get appQuit => 'Quitter CopyPaste';

  @override
  String get appWindowSetupTitle =>
      'La configuration de la fenêtre requiert votre attention';

  @override
  String get appOpenHistory => 'Ouvrir l’Historique';

  @override
  String get appDestinationHistory => 'Historique';

  @override
  String get appDestinationDevices => 'Appareils';

  @override
  String get appDestinationSettings => 'Paramètres';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, sélectionné';
  }

  @override
  String get appActiveFilter => 'Filtre actif';

  @override
  String get appActionFailed => 'Échec de l’action';

  @override
  String get sharedAndroidVersionRequired =>
      'Android 11 ou version ultérieure requis';

  @override
  String get sharedUseAdbTab => 'Utilisez l’onglet ADB sur cet appareil.';

  @override
  String get sharedOneTimeAccessApplied => 'Accès unique appliqué';

  @override
  String get sharedInstallShizuku => 'Installer Shizuku';

  @override
  String get sharedPairAndStartShizuku => 'Associer et démarrer Shizuku';

  @override
  String get sharedAllowCopyPaste => 'Autoriser CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded => 'Shizuku n’est plus nécessaire.';

  @override
  String get sharedGetShizukuDescription =>
      'Installez Shizuku pour appliquer l’accès unique.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Utilisez le débogage sans fil dans Shizuku, puis revenez.';

  @override
  String get sharedApproveInShizuku =>
      'Autorisez CopyPaste une fois dans Shizuku.';

  @override
  String get sharedGetShizuku => 'Obtenir Shizuku';

  @override
  String get sharedOpenShizuku => 'Ouvrir Shizuku';

  @override
  String get sharedRunOnComputer => 'À exécuter sur votre ordinateur';

  @override
  String get sharedCopyAllCommands => 'Copier toutes les commandes';

  @override
  String get sharedEnableUsbDebugging =>
      'Activez le débogage USB et connectez ce téléphone.';

  @override
  String get sharedAllow => 'Autoriser';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Impossible de copier les commandes de configuration.';

  @override
  String get sharedShizukuOpenFailed => 'Impossible d’ouvrir Shizuku.';

  @override
  String sharedSizeBytes(String size) {
    return '$size o';
  }

  @override
  String get onboardingTitle => 'Configurer CopyPaste';

  @override
  String get onboardingWelcomeTitle => 'Bienvenue dans CopyPaste';

  @override
  String get onboardingWelcomeSubtitle =>
      'Votre presse-papiers, prêt quand vous l’êtes.';

  @override
  String get onboardingReadyTitle => 'CopyPaste est prêt';

  @override
  String get onboardingGetStarted => 'Commencer';

  @override
  String get onboardingOptionalSetupTitle =>
      'Une configuration facultative requiert votre attention';

  @override
  String get onboardingOpenSystemSettings => 'Ouvrir les paramètres';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return 'ÉTAPE $step SUR $count · $platform';
  }

  @override
  String get onboardingSetupErrorTitle =>
      'La configuration requiert votre attention';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste n’a pas pu lire l’état de la configuration de $platform.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste n’a pas pu enregistrer la progression de la configuration.';

  @override
  String get onboardingStartAtLogin => 'Lancer à l’ouverture de session';

  @override
  String get onboardingStartAtLoginDescription =>
      'Ouvrir CopyPaste à l’ouverture de session';

  @override
  String get onboardingStartAtLoginSemantics =>
      'Lancer CopyPaste à l’ouverture de session';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Impossible d’activer le lancement à l’ouverture de session. Vous pouvez continuer sans.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Impossible de modifier le lancement à l’ouverture de session. Vous pouvez continuer sans.';

  @override
  String get onboardingMacosTitle => 'Configurer votre Mac';

  @override
  String get onboardingAccessibility => 'Accessibilité';

  @override
  String get onboardingAutoPasteEnabled => 'Collage automatique activé';

  @override
  String get onboardingAutoPasteOptional =>
      'Facultatif pour le collage automatique';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Disponible dans l’app CopyPaste installée.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'L’approbation de l’élément d’ouverture est requise';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Autorisez CopyPaste dans Réglages Système, puis revenez.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste n’a pas pu lire l’état de la configuration de macOS.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Impossible de demander l’autorisation Accessibilité.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste n’a pas pu actualiser l’état de la configuration de macOS.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Impossible d’actualiser l’état de l’autorisation Accessibilité.';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Impossible d’ouvrir les réglages des éléments d’ouverture.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'Le lancement à l’ouverture de session doit être approuvé dans Réglages Système.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Impossible de désactiver le lancement à l’ouverture de session.';

  @override
  String get onboardingWindowsTitle => 'Configurer votre PC';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'Indisponible dans cette installation. Vous pouvez continuer sans.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Désactivé dans le Gestionnaire des tâches';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Activez CopyPaste dans les applications de démarrage de Windows, puis revenez.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'Le lancement à l’ouverture de session est désactivé dans le Gestionnaire des tâches. Vous pourrez l’activer plus tard dans les Paramètres Windows.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'Le lancement à l’ouverture de session n’est pas disponible. Vous pouvez continuer sans.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste n’a pas pu lire l’état de la configuration de Windows.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Impossible d’ouvrir les paramètres de démarrage de Windows.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Impossible de modifier le lancement à l’ouverture de session. Vous pourrez le changer plus tard dans Paramètres.';

  @override
  String get onboardingLinuxChecking =>
      'Vérification de votre session de bureau Linux.';

  @override
  String get onboardingLinuxTitle => 'Configurer votre bureau Linux';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste a besoin d’une intégration au bureau vérifiée pour identifier l’origine des copies et pour Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Session de bureau';

  @override
  String get onboardingLinuxX11Detected => 'Session X11 détectée.';

  @override
  String get onboardingLinuxWaylandDetected => 'Session Wayland détectée.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'Aucune session de bureau X11 ou Wayland prise en charge n’a été détectée.';

  @override
  String get onboardingLinuxCompanion => 'Compagnon GNOME ou KDE';

  @override
  String get onboardingLinuxCompanionReady =>
      'Le pont Clipboard v2 authentifié est prêt.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'La capture du presse-papiers est suspendue par les exclusions d’apps sources. Activez le compagnon signé pour la reprendre.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Installez et activez le paquet de session du compagnon signé, puis revenez ici.';

  @override
  String get onboardingLinuxOpenSetup => 'Ouvrir la configuration';

  @override
  String get onboardingLinuxKeyboardControl => 'Contrôle du clavier';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'Le contrôle du clavier est autorisé pour Quick Paste.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'Le contrôle du clavier n’est pas disponible dans cette session de bureau.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Autoriser le contrôle du clavier';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste est prêt.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Quick Paste sera prêt une fois l’intégration au bureau requise activée.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'Ouvrir CopyPaste à l’ouverture de session.';

  @override
  String get onboardingLinuxLinks => 'Liens CopyPaste';

  @override
  String get onboardingLinuxLinksRegistered =>
      'Les liens CopyPaste s’ouvrent dans cette app.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Enregistrez les liens CopyPaste pour les invitations d’association.';

  @override
  String get onboardingLinuxRegisterLinks => 'Enregistrer les liens';

  @override
  String get onboardingLinuxScreenshotBlocking =>
      'Blocage des captures d’écran';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'Le blocage des captures d’écran n’est pas disponible sur Linux. Les données stockées protégées restent chiffrées.';

  @override
  String get onboardingLinuxCheckAgain => 'Vérifier à nouveau';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste n’a pas pu lire l’état de l’intégration Linux.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Impossible d’ouvrir la configuration de l’intégration au bureau.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'L’autorisation de contrôle du clavier n’a pas été accordée.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Impossible de modifier le lancement à l’ouverture de session.';

  @override
  String get onboardingLinuxLinksFailed =>
      'Impossible d’enregistrer les liens CopyPaste.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Impossible d’actualiser l’état de l’intégration Linux.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'L’accès au presse-papiers n’est pas disponible dans cette session X11.';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste nécessite une session de bureau X11 ou Wayland.';

  @override
  String get onboardingAndroidFullCapture => 'Capture complète';

  @override
  String get onboardingAndroidBackgroundCapture => 'Capture en arrière-plan';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Configurez une fois, puis vérifiez avec une vraie copie.';

  @override
  String get onboardingAndroidLimitedCapture => 'Capture limitée';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'Partagez vers CopyPaste ou revenez après avoir copié.';

  @override
  String get onboardingAndroidNotification => 'Notification de capture';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Requise pendant la capture';

  @override
  String get onboardingAndroidBackgroundActivity => 'Activité en arrière-plan';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Recommandé pour limiter les pertes de capture';

  @override
  String get onboardingAndroidOpen => 'Ouvrir';

  @override
  String get onboardingAndroidApplyAccess => 'Appliquer l’accès de capture';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Appliquer un accès de capture unique.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Vous pourrez configurer la capture complète plus tard.';

  @override
  String get onboardingAndroidCaptureWorking =>
      'La capture en arrière-plan fonctionne';

  @override
  String get onboardingAndroidVerifyCopy => 'Vérifier avec une vraie copie';

  @override
  String get onboardingAndroidCopyReached =>
      'Une nouvelle copie est arrivée dans votre Historique local.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Copiez quelque chose dans une autre app, puis revenez ici.';

  @override
  String get onboardingAndroidStartCapture => 'Démarrer la capture';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste n’a pas pu lire l’état de la configuration d’Android.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Impossible d’actualiser l’état de la capture Android.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'Le suivi de la configuration Android n’est pas disponible.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'Impossible d’activer la réception du presse-papiers.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Impossible d’arrêter la capture en arrière-plan.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Impossible d’ouvrir les paramètres de la batterie.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste est autorisé dans Shizuku, mais les autorisations de capture Android n’ont pas pu être appliquées.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Autorisez CopyPaste dans Shizuku pour appliquer les autorisations de capture.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Accordez d’abord l’accès au presse-papiers en arrière-plan.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Autorisez les notifications avant de démarrer la capture en arrière-plan.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Impossible de démarrer la capture en arrière-plan.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Impossible d’ouvrir la configuration de la capture Android.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Impossible d’enregistrer le choix de configuration Android.';

  @override
  String get modulesMarketplaceTab => 'Catalogue';

  @override
  String get modulesInstalledTab => 'Installés';

  @override
  String get modulesRefresh => 'Actualiser';

  @override
  String get modulesSearchPlaceholder => 'Rechercher des modules';

  @override
  String get modulesOperationFailed => 'Échec de l’opération sur le module';

  @override
  String get modulesLoading => 'Chargement des modules.';

  @override
  String get modulesUnavailableTitle => 'Les modules ne sont pas disponibles';

  @override
  String get modulesUnavailableMessage => 'Essayez de recharger les modules.';

  @override
  String get modulesNoneInstalled => 'Aucun module installé';

  @override
  String get modulesNoMatches => 'Aucun module correspondant';

  @override
  String get modulesChooseFromMarketplace =>
      'Choisissez un module dans le catalogue.';

  @override
  String get modulesTryDifferentSearch => 'Essayez une autre recherche.';

  @override
  String get modulesMarketplaceLoading => 'Chargement du catalogue.';

  @override
  String get modulesMarketplaceUnavailableTitle =>
      'Le catalogue n’est pas disponible';

  @override
  String get modulesMarketplaceUnavailableMessage =>
      'Essayez de recharger le catalogue.';

  @override
  String get modulesNoneAvailable => 'Aucun module disponible';

  @override
  String get modulesNoneAvailableMessage =>
      'Les modules s’afficheront ici dès leur publication.';

  @override
  String modulesEnableModule(String module) {
    return 'Activer $module';
  }

  @override
  String get modulesInstall => 'Installer';

  @override
  String modulesUpdateTo(String version) {
    return 'Mettre à jour vers $version';
  }

  @override
  String get modulesRemoveModule => 'Supprimer le module';

  @override
  String get modulesModuleSettings => 'Paramètres du module';

  @override
  String modulesSettingsTitle(String module) {
    return 'Paramètres de $module';
  }

  @override
  String get modulesDownloadingModel => 'Téléchargement du modèle de langue…';

  @override
  String get modulesRun => 'Exécuter';

  @override
  String modulesRemoveTitle(String module) {
    return 'Supprimer $module ?';
  }

  @override
  String get modulesRemoveMessage =>
      'Le module et ses paramètres locaux seront supprimés.';

  @override
  String get modulesRemove => 'Supprimer';

  @override
  String get modulesRemoveFailed => 'Impossible de supprimer le module.';

  @override
  String get modulesCloseSettings => 'Fermer les paramètres du module';

  @override
  String get modulesRemoved => 'Module supprimé';

  @override
  String get modulesNoSettingsTitle => 'Aucun paramètre supplémentaire';

  @override
  String get modulesNoSettingsMessage =>
      'Ce module ne nécessite aucune configuration supplémentaire.';

  @override
  String get modulesSetUpSmsAccess => 'Configurer l’accès aux SMS';

  @override
  String get modulesRestartApp => 'Redémarrer CopyPaste';

  @override
  String get modulesSmsAccessTitle => 'Accès aux SMS';

  @override
  String get modulesCloseSmsAccess => 'Fermer l’accès aux SMS';

  @override
  String get modulesSmsChecking => 'Vérification de l’accès aux SMS.';

  @override
  String get modulesSmsReady =>
      'L’accès est prêt. Activez SMS Codes pour commencer à copier les nouveaux codes.';

  @override
  String get modulesSmsAllowAccess =>
      'Autorisez l’accès pour copier les codes des nouveaux SMS.';

  @override
  String get modulesSmsNotification => 'Notification SMS';

  @override
  String get modulesSmsNotificationDescription =>
      'Requise pendant l’exécution de SMS Codes';

  @override
  String get modulesSmsApplyAccess => 'Appliquer l’accès aux SMS';

  @override
  String get modulesSmsApplyAccessDescription =>
      'Appliquer un accès unique aux SMS.';

  @override
  String get modulesSmsSetupFailed => 'Échec de la configuration des SMS';

  @override
  String get modulesFileSelectionFailed => 'Échec de la sélection du fichier';

  @override
  String get modulesChooseLanguages => 'Choisir les langues';

  @override
  String get modulesChooseFile => 'Choisir un fichier';

  @override
  String modulesModelReady(String model) {
    return 'Le modèle $model est prêt';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return 'Modèle $model · téléchargement de $size Mio';
  }

  @override
  String get modulesPlatformsUnknown => 'Plateformes : inconnues';

  @override
  String modulesPlatforms(String platforms) {
    return 'Plateformes : $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'La signature du catalogue de modules n’est pas valide.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Impossible de se connecter au catalogue de modules.';

  @override
  String get modulesMarketplaceTimeout =>
      'Le catalogue de modules n’a pas répondu.';

  @override
  String get modulesIncompatible =>
      'Ce module n’est pas compatible avec cet appareil.';

  @override
  String get modulesPackageSizeInvalid =>
      'La taille du paquet du module n’est pas valide.';

  @override
  String get modulesDownloadInterrupted =>
      'Le téléchargement du module a été interrompu.';

  @override
  String get modulesDownloadTimeout =>
      'Le téléchargement du module ne répond pas.';

  @override
  String get modulesPackageUrlUntrusted =>
      'L’URL du paquet du module n’est pas fiable.';

  @override
  String get modulesDownloadLeftGithub =>
      'Le téléchargement du module a quitté GitHub.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'Le téléchargement du module a subi trop de redirections.';

  @override
  String get modulesDownloadTooLarge =>
      'Le téléchargement du module est trop volumineux.';

  @override
  String get modulesPackageIntegrityFailed =>
      'Le paquet du module n’a pas réussi la vérification d’intégrité.';

  @override
  String get modulesDownloadUnavailable =>
      'Le téléchargement de ce module est actuellement indisponible.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'Le catalogue de modules n’est pas disponible. Réessayez.';

  @override
  String get modulesCatalogInvalid =>
      'Le catalogue de modules n’est pas valide.';

  @override
  String get modulesThisSystem => 'ce système';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version ou version ultérieure';
  }

  @override
  String get modulesNotAvailableForDevice =>
      'Non disponible pour cet appareil.';

  @override
  String get modulesAppVersionUnverified =>
      'Impossible de vérifier la version installée de CopyPaste.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Ce module ne prend pas en charge CopyPaste $version.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Nécessite $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed =>
      'Impossible de charger le catalogue de modules.';

  @override
  String get modulesSmsSetupRequired =>
      'Configurez l’accès aux SMS avant d’activer ce module.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'Impossible de démarrer la surveillance des SMS. Le module est toujours activé ; désactivez-le à nouveau pour réessayer.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'Impossible de démarrer la surveillance des SMS. Le module a été désactivé.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'Impossible de démarrer la surveillance des SMS. L’état d’activation du module n’a pas pu être confirmé.';

  @override
  String get modulesSmsMonitoringFailed =>
      'Impossible de démarrer la surveillance des SMS.';

  @override
  String get modulesSmsAndroidOnly =>
      'L’accès aux SMS n’est disponible que sur Android.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'La surveillance des SMS doit être configurée.';

  @override
  String get modulesOperationIncomplete =>
      'Impossible de terminer l’opération sur le module.';

  @override
  String get modulesUnsupportedEvent =>
      'Le module utilise un événement non pris en charge.';

  @override
  String get modulesUnsupportedResult =>
      'Le module a renvoyé un résultat non pris en charge.';

  @override
  String get modulesUnsupportedField =>
      'Le module utilise un champ non pris en charge.';

  @override
  String get modulesResponseInvalid => 'La réponse du module n’est pas valide.';

  @override
  String get modulesFileSelectionUnavailable =>
      'La sélection de fichiers n’est pas disponible.';

  @override
  String get modulesFileSelectFailed =>
      'Impossible de sélectionner le fichier.';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field n’est pas un fichier accepté.';
  }

  @override
  String get modulesSmsVerifyFailed =>
      'Impossible de vérifier l’accès aux SMS.';

  @override
  String get modulesSmsAllowNotifications =>
      'Autorisez les notifications pour que SMS Codes reste actif en arrière-plan.';

  @override
  String get modulesSmsNotGranted => 'L’accès aux SMS n’a pas été accordé.';

  @override
  String get modulesSmsConfigureFailed =>
      'Impossible de configurer l’accès aux SMS.';

  @override
  String get onboardingLinuxContinueBasic =>
      'Continuer avec la capture de base';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Facultatif. Nécessaire pour Quick Paste et les exclusions d’apps sources.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Facultatif. Nécessaire pour que Quick Paste puisse revenir à l’app précédente.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'L’accès au presse-papiers n’est pas disponible dans cette session Wayland. Activez le compagnon signé GNOME ou KDE, puis vérifiez à nouveau.';

  @override
  String get historyMoreActions => 'Plus d’actions';

  @override
  String get historyTagsAction => 'Étiquettes…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'déverrouiller les éléments secrets',
      'other': 'Déverrouiller les éléments secrets',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'utiliser un extrait masqué',
      'other': 'Utiliser un extrait masqué',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'copier un élément secret',
      'other': 'Copier un élément secret',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'Non disponible sur ce système';

  @override
  String get deviceAuthNotConfigured =>
      'Configurez d’abord un mot de passe, un code PIN ou la biométrie de l’appareil';

  @override
  String get commonOn => 'Activé';

  @override
  String get commonOff => 'Désactivé';

  @override
  String get settingsQuickPasteStartFailed =>
      'Impossible de démarrer Quick Paste, son raccourci ne fonctionne donc pas.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut est utilisé par une autre application. Choisissez un autre raccourci.';
  }

  @override
  String get devicesEmptyTitle => 'Aucun autre appareil associé pour l’instant';

  @override
  String get devicesEmptyMessage =>
      'Associez un appareil du même réseau pour synchroniser l’historique.';

  @override
  String get pairingStateNotCompleted => 'Association non terminée';

  @override
  String get pairingStepInvite => 'Invitation';

  @override
  String get pairingStepScan => 'Scan';

  @override
  String get pairingStepCode => 'Code';

  @override
  String get pairingStepRequest => 'Demande';

  @override
  String get pairingStepConnect => 'Connexion';

  @override
  String get pairingStepConfirm => 'Confirmation';

  @override
  String get pairingStepPaired => 'Associé';

  @override
  String get pairingOtherDevice => 'Autre appareil';

  @override
  String get pairingDecisionNotConfirmed => 'Pas encore confirmé';

  @override
  String get pairingDecisionSending => 'Envoi de votre décision…';

  @override
  String get pairingDecisionConfirmed => 'Confirmé';

  @override
  String get pairingDecisionRejected => 'Refusé';

  @override
  String get pairingDecisionWaiting => 'En attente de confirmation';

  @override
  String get pairingDecisionUnknown => 'Réponse inconnue';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count appareils',
      many: '$count appareils',
      one: '$count appareil',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count lignes',
      many: '$count lignes',
      one: '$count ligne',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Navigation principale';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Étape $step sur $count : $name';
  }

  @override
  String get modulesPhaseDownload => 'Téléchargement';

  @override
  String get modulesPhaseInstall => 'Installation';

  @override
  String modulesInstallingModule(String module) {
    return 'Installation de $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return 'Mise à jour de $module';
  }

  @override
  String get commonDismiss => 'Ignorer';

  @override
  String get settingsRetentionConfirmTitle =>
      'Supprimer l’historique plus ancien ?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Nouvelle limite : $limit. Les éléments non épinglés plus anciens sont supprimés définitivement.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle =>
      'Réduire le quota de stockage ?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Nouveau quota : $size. Les plus anciens éléments non épinglés au-delà sont supprimés définitivement.';
  }

  @override
  String get settingsLimitConfirmAction => 'Appliquer la limite';

  @override
  String get settingsHideEverywhereTitle => 'Masquer complètement CopyPaste ?';

  @override
  String get settingsHideEverywhereMessage =>
      'Si les deux sont désactivés, aucune icône n’est visible. Relancez CopyPaste pour l’ouvrir.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Si les deux sont désactivés, aucune icône n’est visible. Pour l’ouvrir, appuyez sur $shortcut ou relancez CopyPaste.';
  }

  @override
  String get settingsHideEverywhereAction => 'Masquer';

  @override
  String get settingsExportDoneTitle => 'Historique texte exporté';

  @override
  String get settingsBackupDoneTitle => 'Sauvegarde créée';

  @override
  String get settingsRestoreDoneTitle => 'Historique restauré';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments texte exportés',
      many: '$count éléments texte exportés',
      one: '$count élément texte exporté',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count illisibles',
      many: '$count illisibles',
      one: '$count illisible',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Sauvegarde chiffrée enregistrée · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'L’historique du presse-papiers de cet appareil correspond maintenant à la sauvegarde.';

  @override
  String sharedSizeKibibytes(String size) {
    return '$size Kio';
  }

  @override
  String sharedSizeMebibytes(String size) {
    return '$size Mio';
  }

  @override
  String sharedSizeGibibytes(String size) {
    return '$size Gio';
  }

  @override
  String storageUsageShort(String used, String quota) {
    return '$used sur $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return '$used utilisés sur $quota';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '$used utilisés';
  }

  @override
  String get storageUsageOverQuota => 'Quota dépassé';

  @override
  String storageUsageOldestClip(String date) {
    return 'Élément le plus ancien : $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Stockage : $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Nouveau quota : $size. $used sont utilisés, donc les plus anciens éléments non épinglés sont supprimés définitivement.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments',
      many: '$count éléments',
      one: '$count élément',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'Ouvrir CopyPaste';

  @override
  String get trayOpenQuickPaste => 'Ouvrir Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Ouvrir Quick Paste ($shortcut)';
  }

  @override
  String get trayNewSnippet => 'Nouvel extrait';

  @override
  String get trayRecent => 'Récents';

  @override
  String get trayRecentLoading => 'Chargement…';

  @override
  String get trayRecentFailed => 'Impossible de charger les clips';

  @override
  String get trayRecentEmpty => 'Aucun clip pour l\'instant';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips récents masqués',
      one: '1 clip récent masqué',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Secret';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Copier le dernier clip d\'un autre appareil';

  @override
  String get trayCaptureNow => 'Capturer maintenant';

  @override
  String trayPausedUntil(String time) {
    return 'En pause jusqu\'à $time';
  }

  @override
  String get trayPauseFor => 'Suspendre pendant…';

  @override
  String get trayPauseFor15Minutes => '15 minutes';

  @override
  String get trayPauseForOneHour => '1 heure';

  @override
  String get trayPauseUntilResumed => 'Jusqu\'à la reprise';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue : $count éléments',
      one: 'Paste Queue : 1 élément',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Collecte : $count éléments',
      one: 'Collecte : 1 élément',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Ignorés récemment ($count)';
  }

  @override
  String get trayOpenContentRules => 'Ouvrir les règles de contenu';

  @override
  String get trayRecentSettingsTitle =>
      'Barre des menus et zone de notification';

  @override
  String get trayRecentSettingsDescription =>
      'Ce que le menu de la barre des menus ou de la zone de notification affiche sous Récents.';

  @override
  String get trayRecentCountTitle => 'Clips récents';

  @override
  String get trayRecentCountDescription =>
      'Nombre de clips les plus récents affichés dans le menu.';

  @override
  String get trayRecentPreviewTitle => 'Aperçu des clips récents';

  @override
  String get trayRecentPreviewDescription =>
      'Quantité de texte des clips affichée dans le menu.';

  @override
  String get trayRecentPreviewFull => 'Aperçu complet';

  @override
  String get trayRecentPreviewTitles => 'Type et source uniquement';

  @override
  String get trayRecentPreviewHidden => 'Masqué';

  @override
  String get trayRecentSettingsKeywords =>
      'zone de notification barre des menus récents aperçu confidentialité nombre';

  @override
  String get transformsPaletteTitle => 'Transformer le texte';

  @override
  String get transformsFilterPlaceholder => 'Filtrer les transformations';

  @override
  String get transformsNoMatches => 'Aucune transformation ne correspond';

  @override
  String get transformsGroupResults => 'Résultats';

  @override
  String get transformsGroupSuggested => 'Suggestions';

  @override
  String get transformsGroupRecent => 'Récentes';

  @override
  String get transformsGroupFavorites => 'Favoris';

  @override
  String get transformsGroupPipelines => 'Chaînes';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV et XML';

  @override
  String get transformsCategoryEncode => 'Encoder et décoder';

  @override
  String get transformsCategoryCase => 'Casse';

  @override
  String get transformsCategoryHash => 'Hachages';

  @override
  String get transformsCategoryLines => 'Lignes et espaces';

  @override
  String get transformsCategoryClean => 'Nettoyer';

  @override
  String get transformsCategoryUrl => 'Liens';

  @override
  String get transformsCategoryMarkup => 'Texte enrichi et Markdown';

  @override
  String get transformsCategoryDev => 'Développeur';

  @override
  String get transformsCategoryOther => 'Autres';

  @override
  String get transformsChainEmpty => 'Aucune étape';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Puis : $name';
  }

  @override
  String get transformsStepEnabled => 'Exécuter cette étape';

  @override
  String get transformsMoveEarlier => 'Monter';

  @override
  String get transformsMoveLater => 'Descendre';

  @override
  String get transformsRemoveStep => 'Supprimer l\'étape';

  @override
  String get transformsSavePipeline => 'Enregistrer comme chaîne';

  @override
  String get transformsSavePipelineTitle => 'Enregistrer la chaîne';

  @override
  String get transformsSavePipelineFailed =>
      'Impossible d\'enregistrer la chaîne.';

  @override
  String get transformsPipelineNameLabel => 'Nom';

  @override
  String get transformsPipelineDescriptionLabel => 'Description (facultatif)';

  @override
  String get transformsPipelineStepsLabel => 'Étapes';

  @override
  String get transformsPipelineNoSteps => 'Ajoutez au moins une étape.';

  @override
  String get transformsPipelineNameRequired => 'Saisissez un nom.';

  @override
  String get transformsPipelineNameTooLong => '80 caractères au maximum.';

  @override
  String get transformsPipelineNameTaken =>
      'Une autre chaîne porte déjà ce nom.';

  @override
  String get transformsPipelineSavedTitle => 'Chaîne enregistrée';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name est disponible dans chaque palette de transformation.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Étapes : $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Chaîne supprimée';

  @override
  String get transformsNeedsNewerVersion =>
      'Nécessite une version plus récente de CopyPaste';

  @override
  String get transformsLoadFailed =>
      'Impossible de charger les transformations.';

  @override
  String get transformsNotTextTitle =>
      'Les transformations s\'appliquent au texte';

  @override
  String get transformsNotTextMessage =>
      'Cet élément est une image, un fichier ou une couleur. Choisissez un élément texte ou copiez du texte.';

  @override
  String get transformsNothingTitle => 'Rien à transformer';

  @override
  String get transformsNothingMessage =>
      'Copiez du texte ou choisissez un élément de l\'historique.';

  @override
  String get transformsPickTransform => 'Choisissez une transformation';

  @override
  String get transformsPickTransformHint =>
      'Tab l\'ajoute comme étape. Entrée applique le résultat.';

  @override
  String get transformsPreviewing => 'Mise à jour de l\'aperçu…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Lignes : $lines · Mots : $words · Caractères : $chars';
  }

  @override
  String get transformsNoChanges => 'Aucun changement';

  @override
  String transformsTruncated(String size) {
    return 'Affichage des 64 premiers Kio sur $size';
  }

  @override
  String get transformsRandom => 'Aléatoire';

  @override
  String get transformsNotRun => 'Non exécutée';

  @override
  String get transformsSensitiveHidden => 'Le résultat est sensible et masqué.';

  @override
  String get transformsReveal => 'Afficher';

  @override
  String get transformsPaneOutput => 'Sortie';

  @override
  String get transformsPaneInput => 'Entrée';

  @override
  String get transformsPaneChanges => 'Modifications';

  @override
  String get transformsCannotTransform => 'Impossible de transformer ce texte';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Étape $step · $name : $message';
  }

  @override
  String get transformsUseHexOutput => 'Afficher en hexadécimal';

  @override
  String get transformsActionPaste => 'Coller';

  @override
  String get transformsActionSave => 'Enregistrer comme nouvel élément';

  @override
  String get transformsCopiedMessage =>
      'Le résultat est dans le presse-papiers et l\'historique.';

  @override
  String get transformsSavedTitle => 'Enregistré';

  @override
  String get transformsSavedMessage =>
      'Enregistré comme nouvel élément de l’historique.';

  @override
  String get transformsPasteFailedTitle => 'Collage impossible';

  @override
  String get transformsPasteFailedMessage =>
      'Le résultat est dans le presse-papiers. Collez-le manuellement.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message à la ligne $line, colonne $column';
  }

  @override
  String get transformsErrorBinary =>
      'Les données décodées sont binaires, pas du texte';

  @override
  String get transformsErrorTooLarge => 'Trop volumineux pour être transformé';

  @override
  String get transformsErrorInvalidParam =>
      'Un réglage de cette étape n\'est pas valide';

  @override
  String get transformsErrorCancelled => 'Annulé';

  @override
  String get transformsErrorTooExpensive => 'Motif trop coûteux';

  @override
  String get transformsErrorPattern => 'Le motif n\'est pas valide';

  @override
  String get transformsErrorTimestampUnit =>
      'Indiquez si le nombre est en secondes ou en millisecondes';

  @override
  String get transformsErrorExternalEntity =>
      'Les entités externes ne sont pas autorisées';

  @override
  String get transformsErrorJwtEncrypted =>
      'Les jetons chiffrés ne peuvent pas être décodés';

  @override
  String get transformsErrorJson => 'JSON non valide';

  @override
  String get transformsErrorYaml => 'YAML non valide';

  @override
  String get transformsErrorXml => 'XML non valide';

  @override
  String get transformsErrorCsv => 'CSV non valide';

  @override
  String get transformsErrorBase64 => 'Base64 non valide';

  @override
  String get transformsErrorHex => 'Hexadécimal non valide';

  @override
  String get transformsErrorEscape => 'Séquence d\'échappement non valide';

  @override
  String get transformsErrorUrl => 'Encodage pourcent incorrect';

  @override
  String get transformsErrorJwt => 'JWT non valide';

  @override
  String get transformsErrorDate => 'Date ou horodatage non reconnu';

  @override
  String get transformsErrorGeneric =>
      'Cette étape ne peut pas transformer son entrée';

  @override
  String get transformsWarningSignedLink => 'Lien signé laissé inchangé';

  @override
  String get transformsWarningExpired => 'Expiré';

  @override
  String get transformsWarningNotYetValid => 'Pas encore valide';

  @override
  String get transformsWarningSignatureNotVerified => 'Signature non vérifiée';

  @override
  String get transformsWarningDuplicateKeys => 'Clés en double';

  @override
  String get transformsWarningLenientJson => 'JSON non standard accepté';

  @override
  String get transformsWarningDoubleEncoded => 'Le JSON était encodé deux fois';

  @override
  String get transformsWarningEscapes =>
      'Séquences d\'échappement inconnues conservées telles quelles';

  @override
  String get transformsWarningLocalZone => 'Fuseau horaire local supposé';

  @override
  String get transformsWarningNoMatches => 'Aucune correspondance';

  @override
  String get transformsWarningGeneric => 'Vérifiez le résultat';

  @override
  String get transformsRestoreTitle =>
      'Restaurer le presse-papiers après le collage';

  @override
  String get transformsRestoreDescription =>
      'Rétablir le presse-papiers précédent après le collage.';

  @override
  String get transformsDefaultTitle => 'Transformation par défaut au collage';

  @override
  String get transformsDefaultDescription =>
      'Appliquée au texte collé depuis Quick Paste.';

  @override
  String get transformsNoDefault => 'Aucune';

  @override
  String get transformsAppDefaultsTitle => 'Valeurs par défaut par app';

  @override
  String get transformsAppDefaultsDescription =>
      'Utiliser une autre valeur par défaut dans certaines apps.';

  @override
  String get transformsAppIdentifierPlaceholder => 'Identifiant de l\'app';

  @override
  String transformsOtherPlatform(String platform) {
    return 'uniquement sur $platform';
  }

  @override
  String get transformsAdd => 'Ajouter';

  @override
  String get transformsRemove => 'Supprimer';

  @override
  String get transformsFavoritesTitle => 'Favoris';

  @override
  String get transformsFavoritesDescription =>
      'Affichés en premier dans chaque palette.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Mod+$number dans la palette';
  }

  @override
  String get transformsAddFavorite => 'Ajouter un favori';

  @override
  String get transformsPipelinesTitle => 'Chaînes';

  @override
  String get transformsPipelinesDescription =>
      'Enchaînements de transformations enregistrés.';

  @override
  String get transformsNewPipeline => 'Nouvelle chaîne';

  @override
  String get transformsEditPipeline => 'Modifier la chaîne';

  @override
  String get transformsDuplicatePipeline => 'Dupliquer';

  @override
  String get transformsDeletePipeline => 'Supprimer';

  @override
  String transformsDeletePipelineTitle(String name) {
    return 'Supprimer $name ?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Les favoris et valeurs par défaut qui l’utilisent sont effacés.';

  @override
  String get transformsDeletePipelineFailed =>
      'Impossible de supprimer la chaîne.';

  @override
  String get transformsAddStep => 'Ajouter une étape';

  @override
  String get transformsHiddenTitle => 'Transformations masquées';

  @override
  String get transformsHiddenDescription =>
      'Absentes des palettes et des suggestions.';

  @override
  String get transformsHideTransform => 'Masquer une transformation';

  @override
  String get transformsShow => 'Afficher';

  @override
  String get transformsSuggestionsTitle => 'Suggestions';

  @override
  String get transformsSuggestionsDescription =>
      'Suggérer des transformations adaptées au texte.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'Jetons JSON Web';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl => 'Liens avec paramètres de suivi';

  @override
  String get transformsDetectorTimestamp => 'Horodatages';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Paramètres de suivi';

  @override
  String get transformsTrackingDescription =>
      'Paramètres de lien à retirer aussi ou à toujours garder.';

  @override
  String get transformsTrackingRemoveLabel => 'Supprimer aussi';

  @override
  String get transformsTrackingAllowLabel => 'Toujours conserver';

  @override
  String get transformsTrackingPlaceholder => 'nom ou nom@domaine, puis Entrée';

  @override
  String get quickAccessUnavailableTitle =>
      'État de l\'accès rapide indisponible';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste n\'a pas pu lire l\'état du clavier, des vignettes et du widget. Réessayez.';

  @override
  String get quickAccessActionFailedTitle => 'Action impossible';

  @override
  String get quickAccessActionFailedMessage =>
      'Android n\'a pas ouvert l\'écran demandé ou n\'a pas enregistré la modification. Réessayez.';

  @override
  String get quickAccessKeyboardTitle => 'Clavier d\'éléments';

  @override
  String get quickAccessKeyboardDescription =>
      'Activez Éléments CopyPaste dans les paramètres du clavier.';

  @override
  String get quickAccessKeyboardOn => 'Activé';

  @override
  String get quickAccessKeyboardOff => 'Désactivé';

  @override
  String get quickAccessOpenKeyboardSettings =>
      'Ouvrir les paramètres du clavier';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'Activer le panneau de clavier Éléments CopyPaste.';

  @override
  String get quickAccessKeyboardKeywords =>
      'clavier mode de saisie ime panneau activer';

  @override
  String get quickAccessSwitchKeyboardTitle => 'Passer au clavier d\'éléments';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'Choisissez Éléments CopyPaste comme clavier actuel.';

  @override
  String get quickAccessSwitchKeyboardAction => 'Choisir le clavier';

  @override
  String get quickAccessKeyboardCurrent => 'Clavier actuel';

  @override
  String get quickAccessCaptureTierTitle => 'Mode de capture';

  @override
  String get quickAccessCaptureTierDescription =>
      'Comment les copies des autres apps sont enregistrées.';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'Voir comment les copies des autres applis sont enregistrées.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'capture niveau complet panneau manuel shizuku collecter';

  @override
  String get quickAccessTierFull =>
      'Complet : chaque copie est enregistrée en arrière-plan';

  @override
  String get quickAccessTierPanelCollecting => 'Panneau sélectionné : collecte';

  @override
  String get quickAccessTierPanelCatchUp =>
      'Mode panneau : rattrapage à l\'ouverture';

  @override
  String get quickAccessTierManual =>
      'Manuel : enregistrez depuis l\'appli, une vignette, le widget ou Partager';

  @override
  String get quickAccessTierPaused => 'Capture en pause';

  @override
  String get quickAccessAutoReturnTitle =>
      'Revenir au clavier précédent après l\'insertion';

  @override
  String get quickAccessAutoReturnDescription =>
      'Revenir après l’insertion d’un élément.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'revenir clavier précédent automatique insérer';

  @override
  String get quickAccessFocusSearchTitle =>
      'Activer la recherche à l\'ouverture du panneau';

  @override
  String get quickAccessFocusSearchDescription =>
      'Ouvrir le panneau avec la recherche prête.';

  @override
  String get quickAccessTilesTitle => 'Vignettes des Réglages rapides';

  @override
  String get quickAccessTilesSearchDescription =>
      'Ajouter les vignettes Enregistrer le presse-papiers et Capture du presse-papiers.';

  @override
  String get quickAccessTilesKeywords =>
      'réglages rapides vignette volet enregistrer pause';

  @override
  String get quickAccessTileInstructions =>
      'Balayez deux fois vers le bas, appuyez sur Modifier et ajoutez une vignette CopyPaste.';

  @override
  String get quickAccessSaveTileTitle =>
      'Vignette Enregistrer le presse-papiers';

  @override
  String get quickAccessSaveTileDescription =>
      'Enregistre le contenu du presse-papiers depuis les Réglages rapides.';

  @override
  String get quickAccessCaptureTileTitle =>
      'Vignette Capture du presse-papiers';

  @override
  String get quickAccessCaptureTileDescription =>
      'Met en pause et reprend la capture depuis les Réglages rapides.';

  @override
  String get quickAccessAddTile => 'Ajouter la vignette';

  @override
  String get quickAccessTileAdded => 'Ajoutée aux Réglages rapides';

  @override
  String get quickAccessWidgetTitle => 'Widget Éléments récents';

  @override
  String get quickAccessWidgetDescription =>
      'Éléments récents sur l’écran d’accueil, copiés d’un toucher.';

  @override
  String get quickAccessWidgetInstructions =>
      'Maintenez l’écran d’accueil, touchez Widgets et ajoutez Éléments récents.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count widgets ajoutés',
      one: '1 widget ajouté',
      zero: 'Non ajouté',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Ajouter le widget';

  @override
  String get quickAccessWidgetKeywords =>
      'écran d\'accueil widget lanceur récents';

  @override
  String snippetsAbbreviationBadge(String abbreviation) {
    return 'Abréviation $abbreviation';
  }

  @override
  String get snippetsAbbreviationLabel => 'Abréviation';

  @override
  String get snippetsAbbreviationPlaceholder => 'Facultatif, par exemple sig';

  @override
  String snippetsAbbreviationTaken(String title) {
    return 'Déjà utilisée par « $title »';
  }

  @override
  String snippetsAbbreviationTooLong(int max) {
    return 'Utilisez $max caractères au maximum.';
  }

  @override
  String get snippetsAbbreviationWhitespace =>
      'Une abréviation ne peut pas contenir d\'espaces ni de caractères de contrôle.';

  @override
  String snippetsAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count extraits à vérifier après une synchronisation',
      one: '1 extrait à vérifier après une synchronisation',
      zero: 'Aucun extrait à vérifier',
    );
    return '$_temp0';
  }

  @override
  String get snippetsBodyLabel => 'Texte';

  @override
  String get snippetsBodyPlaceholder => 'Saisissez le texte de l\'extrait';

  @override
  String get snippetsClearSelection => 'Effacer la sélection';

  @override
  String get snippetsClipTooLarge =>
      'Trop volumineux pour être modifié. Cet élément s\'ouvre en lecture seule.';

  @override
  String snippetsCollectionAccent(String accent) {
    String _temp0 = intl.Intl.selectLogic(accent, {
      'neutral': 'Neutre',
      'blue': 'Bleu',
      'green': 'Vert',
      'amber': 'Ambre',
      'red': 'Rouge',
      'other': 'Neutre',
    });
    return '$_temp0';
  }

  @override
  String get snippetsCollectionAccentLabel => 'Couleur d\'accent';

  @override
  String get snippetsCollectionIcon => 'Icône';

  @override
  String get snippetsCollectionLabel => 'Collection';

  @override
  String get snippetsCollectionName => 'Nom';

  @override
  String get snippetsCollectionNameTaken => 'Une collection porte déjà ce nom.';

  @override
  String snippetsCollectionNameTooLong(int max) {
    return 'Utilisez $max caractères au maximum.';
  }

  @override
  String get snippetsConcealedBody => 'Cet extrait est masqué.';

  @override
  String get snippetsConcealedConfirmAction => 'Continuer';

  @override
  String get snippetsConcealedConfirmMessage =>
      'Cet appareil ne peut pas vous vérifier. Continuez uniquement sur votre appareil.';

  @override
  String get snippetsConcealedConfirmTitle => 'Confirmez que c\'est vous';

  @override
  String get snippetsConcealedPreview => 'Extrait masqué';

  @override
  String get snippetsConcealedSwitch => 'Masqué';

  @override
  String get snippetsConcealedTitle => 'Extrait masqué';

  @override
  String get snippetsConflictBadge => 'Copie en conflit';

  @override
  String get snippetsCopied => 'Copié';

  @override
  String get snippetsCreate => 'Créer un extrait';

  @override
  String snippetsCreateNamed(String query) {
    return 'Créer l\'extrait « $query »';
  }

  @override
  String get snippetsDeleteCollection => 'Supprimer la collection';

  @override
  String get snippetsDeleteCollectionMove =>
      'Déplacer ses extraits vers Sans collection';

  @override
  String snippetsDeleteCollectionTitle(String name) {
    return 'Supprimer « $name » ?';
  }

  @override
  String get snippetsDeleteCollectionWithSnippets =>
      'Supprimer aussi ses extraits';

  @override
  String get snippetsDeletedMessage => 'Déplacé vers Supprimés récemment.';

  @override
  String snippetsDeletedTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count extraits supprimés',
      one: 'Extrait supprimé',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDestination => 'Enregistrer dans';

  @override
  String snippetsDestinationName(String destination) {
    String _temp0 = intl.Intl.selectLogic(destination, {
      'historyClip': 'Élément de l\'historique',
      'snippet': 'Extrait',
      'other': 'Élément de l\'historique',
    });
    return '$_temp0';
  }

  @override
  String get snippetsDiscard => 'Ignorer';

  @override
  String snippetsDragging(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count extraits',
      one: '1 extrait',
    );
    return '$_temp0';
  }

  @override
  String get snippetsDuplicateAbbreviation =>
      'Un autre extrait a cette abréviation. Modifiez-en une.';

  @override
  String get snippetsDuplicateBadge => 'Abréviation en double';

  @override
  String get snippetsEditClip => 'Modifier l\'élément';

  @override
  String get snippetsEditCollection => 'Modifier la collection';

  @override
  String get snippetsEditSnippet => 'Modifier l\'extrait';

  @override
  String get snippetsEditTab => 'Modifier';

  @override
  String get snippetsEmptyMessage =>
      'Donnez une abréviation à un extrait, puis tapez-la dans Quick Paste.';

  @override
  String get snippetsEmptyTitle => 'Aucun extrait pour l\'instant';

  @override
  String get snippetsEnableTemplate => 'Activer';

  @override
  String get snippetsEnableTemplateMessage =>
      'Le texte qui ressemble à un espace réservé sera rempli.';

  @override
  String get snippetsEnableTemplateTitle => 'Activer Modèle ?';

  @override
  String get snippetsExport => 'Exporter';

  @override
  String get snippetsExportCollection => 'Exporter la collection';

  @override
  String get snippetsExportConcealedConfirm =>
      'Inclure les extraits masqués dans le fichier lisible';

  @override
  String snippetsExportCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count extraits écrits',
      one: '1 extrait écrit',
      zero: 'Aucun extrait écrit',
    );
    return '$_temp0';
  }

  @override
  String get snippetsExportDone => 'Extraits exportés';

  @override
  String get snippetsExportEncrypt => 'Chiffrer avec une phrase secrète';

  @override
  String get snippetsExportFailed => 'Échec de l\'exportation';

  @override
  String get snippetsExportPlainConfirm =>
      'Je comprends que toute personne ayant le fichier peut le lire';

  @override
  String snippetsExportScope(String scope, int count) {
    String _temp0 = intl.Intl.selectLogic(scope, {
      'all': 'Tous les extraits',
      'collections': 'Cette collection',
      'snippets': 'Extraits sélectionnés ($count)',
      'other': 'Tous les extraits',
    });
    return '$_temp0';
  }

  @override
  String get snippetsExportTitle => 'Exporter des extraits';

  @override
  String get snippetsFieldBuilderTitle => 'Insérer un champ';

  @override
  String get snippetsFieldDefault => 'Valeur par défaut';

  @override
  String get snippetsFieldName => 'Nom';

  @override
  String get snippetsFieldOptions => 'Options, séparées par des virgules';

  @override
  String get snippetsFieldRequired => 'Remplissez ce champ.';

  @override
  String get snippetsFieldType => 'Type';

  @override
  String snippetsFieldTypeName(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'text': 'Une ligne',
      'multiline': 'Plusieurs lignes',
      'choice': 'Choix',
      'other': 'Une ligne',
    });
    return '$_temp0';
  }

  @override
  String get snippetsFormattingNotice =>
      'La mise en forme sera supprimée à l\'enregistrement.';

  @override
  String get snippetsGoToSnippet => 'Aller à l\'extrait';

  @override
  String get snippetsImport => 'Importer';

  @override
  String get snippetsImportAbbreviationPolicy => 'Abréviation déjà utilisée';

  @override
  String snippetsImportAbbreviationPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'importWithout': 'Importer sans abréviation',
      'renameWithSuffix': 'Ajouter un numéro à l\'abréviation',
      'skip': 'Ignorer',
      'other': 'Importer sans abréviation',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportCollectionPolicy => 'Collection du même nom';

  @override
  String snippetsImportCollectionPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'merge': 'Ajouter à l\'existante',
      'createNew': 'En créer une nouvelle',
      'other': 'Ajouter à l\'existante',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportConvert => 'Convertir les espaces réservés connus';

  @override
  String snippetsImportCounts(
    int added,
    int identical,
    int updated,
    int conflicts,
    int invalid,
  ) {
    return 'Nouveaux : $added. Identiques, ignorés : $identical. Modifiés : $updated. Conflits d\'abréviation : $conflicts. Non valides : $invalid.';
  }

  @override
  String get snippetsImportDone => 'Extraits importés';

  @override
  String get snippetsImportFailed => 'Échec de l\'importation';

  @override
  String get snippetsImportFileType => 'Fichiers d\'extraits';

  @override
  String get snippetsImportFormat => 'Format';

  @override
  String snippetsImportFormatName(String format) {
    String _temp0 = intl.Intl.selectLogic(format, {
      'bundle': 'Paquet CopyPaste',
      'json': 'Liste JSON',
      'csv': 'CSV',
      'other': 'Paquet CopyPaste',
    });
    return '$_temp0';
  }

  @override
  String snippetsImportInvalidRow(int row, String reason) {
    return 'Ligne $row : $reason';
  }

  @override
  String get snippetsImportInvalidRows => 'Lignes qui ne seront pas importées';

  @override
  String get snippetsImportPassphraseHint =>
      'Phrase secrète, si le fichier est chiffré';

  @override
  String get snippetsImportPreview => 'Aperçu';

  @override
  String snippetsImportReport(
    int inserted,
    int updated,
    int skipped,
    int flagged,
    int unrecognized,
  ) {
    return 'Ajoutés : $inserted. Mis à jour : $updated. Ignorés : $skipped. Abréviations à vérifier : $flagged. Espaces réservés inconnus : $unrecognized.';
  }

  @override
  String get snippetsImportSameIdPolicy => 'Même extrait, contenu différent';

  @override
  String snippetsImportSameIdPolicyName(String policy) {
    String _temp0 = intl.Intl.selectLogic(policy, {
      'keepBoth': 'Garder les deux',
      'replace': 'Remplacer l\'existant',
      'skip': 'Ignorer',
      'other': 'Garder les deux',
    });
    return '$_temp0';
  }

  @override
  String get snippetsImportTitle => 'Importer des extraits';

  @override
  String get snippetsInsert => 'Insérer';

  @override
  String get snippetsInsertPlaceholder => 'Insérer un espace réservé';

  @override
  String get snippetsKeepReferences => 'Les garder';

  @override
  String get snippetsLibraryDescription =>
      'Parcourez, organisez et modifiez extraits et collections.';

  @override
  String get snippetsLibraryKeywords =>
      'extraits modèles abréviations collections texte snippets';

  @override
  String get snippetsLibraryTitle => 'Bibliothèque d\'extraits';

  @override
  String snippetsLint(String code, String detail) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'unclosed': 'Cet espace réservé n\'est jamais fermé.',
      'malformed': 'Impossible de lire cet espace réservé.',
      'unknownPlaceholder':
          'Espace réservé inconnu « $detail ». Il est collé tel quel.',
      'unknownParameter': 'Option inconnue « $detail ».',
      'duplicateParameter': 'L\'option « $detail » est définie deux fois.',
      'missingParameter': 'L\'option « $detail » manque.',
      'invalidValue': 'L\'option « $detail » a une valeur non valide.',
      'conflictingParameters':
          'Ces options ne peuvent pas être utilisées ensemble.',
      'unknownLocale': 'Langue inconnue.',
      'unknownTimeZone': 'Fuseau horaire inconnu.',
      'invalidPattern': 'Modèle de date non valide.',
      'dateOutOfRange': 'La date est hors limites.',
      'unknownModifier': 'Modificateur inconnu « $detail ».',
      'modifierFailed': 'Impossible d\'appliquer le modificateur « $detail ».',
      'emptyClipboard': 'L\'élément $detail du presse-papiers est vide.',
      'extraCursor': 'Seul le premier espace réservé de curseur est utilisé.',
      'nestedCursor': 'Un curseur dans un extrait inséré est ignoré.',
      'tooManyFields': 'Le champ « $detail » dépasse la limite de 10 champs.',
      'conflictingField':
          'Le champ « $detail » est défini deux fois avec des réglages différents.',
      'choiceDefaultNotOption':
          'La valeur par défaut de « $detail » ne fait pas partie de ses options.',
      'cycle': 'Des extraits s\'insèrent mutuellement en boucle : $detail.',
      'depthExceeded': 'Des extraits sont insérés sur plus de 3 niveaux.',
      'nestingLimit': 'Trop d\'extraits insérés.',
      'unknownSnippet': 'Aucun extrait n\'a cette abréviation.',
      'containsPlaceholders':
          'Le texte contient des espaces réservés. Activez Modèle pour les remplir.',
      'other': 'Cet espace réservé pose problème.',
    });
    return '$_temp0';
  }

  @override
  String get snippetsLoadFailed => 'Impossible de charger les extraits';

  @override
  String get snippetsLoadMore => 'Charger plus';

  @override
  String get snippetsMerge => 'Fusionner';

  @override
  String get snippetsMergeCollection => 'Fusionner avec une autre collection';

  @override
  String snippetsMergeCollectionTitle(String name) {
    return 'Fusionner « $name » avec';
  }

  @override
  String get snippetsMoveTo => 'Déplacer vers';

  @override
  String get snippetsNewCollection => 'Nouvelle collection';

  @override
  String get snippetsNewSnippet => 'Nouvel extrait';

  @override
  String get snippetsNewText => 'Nouveau texte';

  @override
  String snippetsNoMatch(String query) {
    return 'Aucun extrait ne correspond à « $query »';
  }

  @override
  String get snippetsPassphraseConfirm => 'Répétez la phrase secrète';

  @override
  String snippetsPassphraseHint(int min) {
    return 'Phrase secrète, au moins $min caractères';
  }

  @override
  String get snippetsPassphraseLabel => 'Phrase secrète';

  @override
  String get snippetsPassphraseNoRecovery =>
      'Une phrase secrète oubliée ne peut pas être récupérée.';

  @override
  String snippetsPassphraseStrength(String score) {
    String _temp0 = intl.Intl.selectLogic(score, {
      'tooShort': 'Trop courte',
      'weak': 'Faible',
      'fair': 'Moyenne',
      'good': 'Bonne',
      'other': 'Robuste',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPinnedSwitch => 'Épinglé';

  @override
  String get snippetsPlaceholderGroup => 'Espaces réservés';

  @override
  String snippetsPlaceholderKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'today': 'Date',
      'dateShort': 'Date, courte',
      'dateLong': 'Date, longue',
      'dateIso': 'Date, ISO 8601',
      'dateNextWeek': 'Date dans une semaine',
      'now': 'Heure',
      'dateTime': 'Date et heure',
      'clipboard': 'Presse-papiers',
      'clipboardPrevious': 'Élément précédent du presse-papiers',
      'field': 'Champ à remplir…',
      'cursor': 'Position du curseur',
      'uuid': 'UUID',
      'random': 'Nombre aléatoire',
      'upper': 'Modificateur : majuscules',
      'lower': 'Modificateur : minuscules',
      'trim': 'Modificateur : supprimer les espaces',
      'other': 'Espace réservé',
    });
    return '$_temp0';
  }

  @override
  String get snippetsPlaceholderNestedGroup => 'Insérer un autre extrait';

  @override
  String get snippetsPlaceholderNoMatch => 'Aucun espace réservé ne correspond';

  @override
  String get snippetsPreviewEmpty => 'Rien à afficher';

  @override
  String get snippetsPreviewTab => 'Aperçu';

  @override
  String get snippetsPurge => 'Supprimer définitivement';

  @override
  String get snippetsPurgeMessage =>
      'Ils seront effacés sur tous vos appareils. Cette action est irréversible.';

  @override
  String snippetsPurgeTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Supprimer $count extraits définitivement ?',
      one: 'Supprimer l\'extrait définitivement ?',
    );
    return '$_temp0';
  }

  @override
  String get snippetsRecentlyDeletedEmpty => 'Supprimés récemment est vide';

  @override
  String get snippetsRecordPastedDescription =>
      'Ajouter les extraits collés à l’historique.';

  @override
  String get snippetsRecordPastedKeywords =>
      'historique enregistrer collé extraits';

  @override
  String get snippetsRecordPastedTitle =>
      'Enregistrer les extraits collés dans l\'historique';

  @override
  String get snippetsReplaceOriginal => 'Remplacer l\'original';

  @override
  String get snippetsReplaceOriginalHint =>
      'Supprime l’élément d’origine après l’enregistrement.';

  @override
  String get snippetsRequireAuthDescription =>
      'Demander d’abord le mot de passe ou la biométrie.';

  @override
  String get snippetsRequireAuthKeywords =>
      'masqué mot de passe biométrie empreinte visage verrou vérifier';

  @override
  String get snippetsRequireAuthTitle =>
      'Vérifier avant d\'utiliser les extraits masqués';

  @override
  String get snippetsRestore => 'Restaurer';

  @override
  String get snippetsSaveBlockTooLarge => 'Le texte dépasse 64 Ko.';

  @override
  String get snippetsSaveFailed => 'Échec de l\'enregistrement';

  @override
  String get snippetsScopeAll => 'Tous';

  @override
  String get snippetsScopeEmpty => 'Aucun extrait ici';

  @override
  String get snippetsScopePinned => 'Épinglés';

  @override
  String get snippetsScopeRecentlyDeleted => 'Supprimés récemment';

  @override
  String get snippetsSearchPlaceholder => 'Rechercher des extraits';

  @override
  String snippetsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count sélectionnés',
      one: '1 sélectionné',
    );
    return '$_temp0';
  }

  @override
  String get snippetsShow => 'Afficher';

  @override
  String get snippetsShowAll => 'Tout afficher';

  @override
  String get snippetsShowAttention => 'Vérifier';

  @override
  String snippetsSoftLimit(int count) {
    return 'Vous avez $count extraits. Jusqu\'à 20 000 sont autorisés.';
  }

  @override
  String get snippetsTemplateBadge => 'Modèle';

  @override
  String get snippetsTemplateSwitch => 'Modèle';

  @override
  String get snippetsTitleLabel => 'Titre';

  @override
  String get snippetsTitlePlaceholder => 'Facultatif';

  @override
  String get snippetsTransferDescription =>
      'Sauvegarder les extraits ou les transférer entre apps.';

  @override
  String get snippetsTransferKeywords =>
      'importer exporter sauvegarde paquet json csv phrase secrète';

  @override
  String get snippetsTransferTitle => 'Importer et exporter';

  @override
  String get snippetsUndo => 'Annuler';

  @override
  String get snippetsUnfiled => 'Sans collection';

  @override
  String snippetsUnfilledWarning(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count espaces réservés sont collés tels quels',
      one: '1 espace réservé est collé tel quel',
    );
    return '$_temp0';
  }

  @override
  String get snippetsUnpin => 'Désépingler';

  @override
  String get snippetsUnsavedMessage =>
      'Enregistrer les modifications avant de fermer ?';

  @override
  String get snippetsUnsavedTitle => 'Modifications non enregistrées';

  @override
  String get snippetsUntitled => 'Extrait sans titre';

  @override
  String get snippetsUpdateReferences => 'Mettre à jour';

  @override
  String snippetsUpdateReferencesMessage(String from, String to) {
    return 'D\'autres modèles peuvent insérer « $from ». Les mettre à jour pour utiliser « $to » ?';
  }

  @override
  String get snippetsUpdateReferencesTitle => 'Mettre à jour les références ?';

  @override
  String get snippetsUseFailed => 'Impossible d\'utiliser l\'extrait';

  @override
  String get aiToolsEnableTitle =>
      'Autoriser les outils d’IA à accéder à CopyPaste sur cet ordinateur';

  @override
  String get aiToolsEnableDescription =>
      'Les outils d’IA que vous connectez peuvent lire les éléments du presse-papiers que vous autorisez ci-dessous. Ce que vous partagez avec un outil d’IA est envoyé à son fournisseur.';

  @override
  String aiToolsStatusOn(int count) {
    return 'Activé · appareils qui partagent l’historique : $count';
  }

  @override
  String get aiToolsStatusOff =>
      'Désactivé · les outils connectés ne peuvent pas accéder à CopyPaste sur cet ordinateur';

  @override
  String get aiToolsThreatModelTitle => 'Ce que protègent ces limites';

  @override
  String get aiToolsThreatModel =>
      'Les autorisations, les validations et le masquage limitent ce qu’un outil d’IA voit via sa connexion à CopyPaste. Ils ne protègent pas contre les autres programmes exécutés sous votre compte, qui peuvent déjà lire votre presse-papiers et votre historique.';

  @override
  String get aiToolsGrantsTitle => 'Outils connectés';

  @override
  String get aiToolsGrantsDescription =>
      'Outils d’IA connectés sur l’un de vos ordinateurs. Une modification s’applique sur l’ordinateur propriétaire de la connexion une fois synchronisé.';

  @override
  String aiToolsServedThrough(String desktops) {
    return 'Servi par : $desktops';
  }

  @override
  String get aiToolsConnect => 'Connecter un outil d’IA';

  @override
  String get aiToolsEmptyTitle => 'Aucun outil d’IA connecté';

  @override
  String get aiToolsEmptyMessage =>
      'Connectez un outil d’IA pour qu’il puisse rechercher parmi les éléments du presse-papiers que vous autorisez.';

  @override
  String get aiToolsEmptyServedTitle =>
      'Les outils d’IA se connectent via votre ordinateur';

  @override
  String get aiToolsEmptyServedMessage =>
      'Activez les outils d’IA dans CopyPaste sur un ordinateur Mac, Windows ou Linux, et l’historique de votre téléphone y sera disponible.';

  @override
  String get aiToolsUnknownDevice => 'un autre ordinateur';

  @override
  String aiToolsGrantOwner(String device) {
    return 'Sur $device';
  }

  @override
  String get aiToolsGrantWaiting => 'En attente de la première connexion';

  @override
  String aiToolsGrantUsed(String date) {
    return 'Connecté · dernière utilisation le $date';
  }

  @override
  String get aiToolsGrantRevoked => 'Révoqué';

  @override
  String aiToolsGrantRevokedRemote(String device) {
    return 'Révoqué · effectif lors de la synchronisation de $device';
  }

  @override
  String get aiToolsGrantReissue =>
      'Nouvelle connexion nécessaire après une restauration';

  @override
  String get aiToolsGrantPaused =>
      'En pause : les outils d’IA sont désactivés sur cet ordinateur';

  @override
  String get aiToolsEdit => 'Modifier';

  @override
  String get aiToolsActivity => 'Voir l’activité';

  @override
  String get aiToolsRevoke => 'Révoquer';

  @override
  String aiToolsRevokeTitle(String name) {
    return 'Révoquer $name ?';
  }

  @override
  String get aiToolsRevokeMessage =>
      'L’outil perd l’accès à sa prochaine requête. Pour le reconnecter, créez une nouvelle connexion.';

  @override
  String aiToolsDeleteTitle(String name) {
    return 'Supprimer $name ?';
  }

  @override
  String get aiToolsDeleteMessage =>
      'La connexion est révoquée et supprimée de tous vos appareils.';

  @override
  String get aiToolsChangeFailed =>
      'La modification n’a pas pu être enregistrée.';

  @override
  String get aiToolsLoadFailed =>
      'Impossible de charger les réglages des outils d’IA';

  @override
  String get aiToolsEditTitle => 'Modifier la connexion';

  @override
  String get aiToolsNameLabel => 'Nom';

  @override
  String get aiToolsScopesLabel => 'Accès';

  @override
  String get aiToolsScopeReadHistory => 'Lire l’historique';

  @override
  String get aiToolsScopeReadSnippets => 'Lire les extraits';

  @override
  String get aiToolsScopeWriteClipboard => 'Écrire dans le presse-papiers';

  @override
  String get aiToolsScopeOrganize =>
      'Organiser (épingler, enregistrer des extraits)';

  @override
  String get aiToolsLookbackLabel => 'Période d’historique';

  @override
  String get aiToolsLookbackDay => 'Dernières 24 heures';

  @override
  String get aiToolsLookbackWeek => '7 derniers jours';

  @override
  String get aiToolsLookbackMonth => '30 derniers jours';

  @override
  String get aiToolsLookbackAll => 'Tout l’historique';

  @override
  String aiToolsActivityTitle(String name) {
    return 'Activité de $name';
  }

  @override
  String get aiToolsActivityEmpty => 'Aucune activité';

  @override
  String get aiToolsActivityFailed => 'Impossible de charger l’activité';

  @override
  String aiToolsActivityEntry(String tool, int count) {
    return '$tool · résultats : $count';
  }

  @override
  String get aiToolsClientLabel => 'Outil d’IA';

  @override
  String get aiToolsClientOther => 'Autre';

  @override
  String get aiToolsCreate => 'Créer la connexion';

  @override
  String aiToolsConfigHint(String client) {
    return 'Ajoutez cette configuration à $client. Sa clé d’accès ne s’affiche qu’une fois.';
  }

  @override
  String get aiToolsConfigHintOther =>
      'Ajoutez cette configuration aux réglages MCP de votre outil d’IA. Sa clé d’accès ne s’affiche qu’une fois.';

  @override
  String aiToolsRestartNote(String client) {
    return 'Ensuite : redémarrez $client ou ouvrez une nouvelle session.';
  }

  @override
  String get aiToolsRestartNoteOther =>
      'Ensuite : redémarrez votre outil d’IA ou ouvrez une nouvelle session.';

  @override
  String get aiToolsCopied => 'Copié';

  @override
  String get aiToolsCopiedMessage =>
      'Ce n’est pas enregistré dans l’historique, et le presse-papiers est vidé dans 60 secondes.';

  @override
  String get aiToolsCopyFailed => 'Impossible de copier la configuration.';

  @override
  String get aiToolsPolicyTitle => 'Ce que les outils d’IA peuvent voir';

  @override
  String get aiToolsPolicyNote =>
      'Les éléments sensibles et les identifiants détectés ne sont jamais partagés, quoi que vous autorisiez ici.';

  @override
  String get aiToolsOtherDevicesTitle =>
      'Inclure les éléments des autres appareils';

  @override
  String get aiToolsOtherDevicesDescription =>
      'Historique synchronisé depuis vos autres appareils. Désactivez un appareil pour exclure ses éléments.';

  @override
  String get aiToolsKindsTitle => 'Types de contenu';

  @override
  String get aiToolsKindsDescription =>
      'Types d’éléments du presse-papiers que les outils d’IA peuvent lire.';

  @override
  String get aiToolsKindText => 'Texte';

  @override
  String get aiToolsKindLink => 'Liens';

  @override
  String get aiToolsKindRichText => 'Texte enrichi';

  @override
  String get aiToolsKindImage => 'Images';

  @override
  String get aiToolsKindFileMetadata => 'Noms et tailles de fichiers';

  @override
  String get aiToolsExcludedAppsTitle => 'Apps exclues';

  @override
  String get aiToolsExcludedAppsDescription =>
      'Les éléments copiés depuis ces apps ne sont jamais partagés.';

  @override
  String get aiToolsAddApp => 'Ajouter une app';

  @override
  String get aiToolsRemove => 'Retirer';

  @override
  String get aiToolsDefaultLookbackTitle =>
      'Période d’historique des nouvelles connexions';

  @override
  String get aiToolsDefaultLookbackDescription =>
      'Les éléments plus anciens non épinglés restent invisibles pour une connexion. Les éléments épinglés et les extraits ne sont pas limités.';

  @override
  String get aiToolsKeywords =>
      'ia ai mcp assistant claude codex cursor modèle agent';

  @override
  String aiToolsApprovalTitle(String client) {
    return '$client souhaite accéder à CopyPaste';
  }

  @override
  String aiToolsReportedName(String name) {
    return 'Nom indiqué par l’app : $name';
  }

  @override
  String aiToolsReportedExecutable(String path) {
    return 'Programme : $path';
  }

  @override
  String get aiToolsApprovalNote =>
      'N’autorisez que des outils que vous avez lancés vous-même. Sans réponse, la demande est refusée au bout de 2 minutes.';

  @override
  String get aiToolsDeny => 'Refuser';

  @override
  String get aiToolsAllow => 'Autoriser';

  @override
  String get aiToolsVolumeTitle => 'Autoriser davantage de lecture ?';

  @override
  String aiToolsVolumeMessage(String client) {
    return '$client a lu beaucoup d’historique du presse-papiers pendant cette session. En autoriser davantage ?';
  }

  @override
  String get aiToolsNotificationBody =>
      'Ouvrez CopyPaste pour autoriser ou refuser.';

  @override
  String get aiToolsDecisionFailed =>
      'La réponse n’a pas pu être envoyée. La demande s’affichera de nouveau.';

  @override
  String get aiToolsHide => 'Masquer aux outils d’IA';

  @override
  String get aiToolsShow => 'Afficher aux outils d’IA';

  @override
  String get aiToolsHiddenState => 'Masqué aux outils d’IA';

  @override
  String get quickPasteTabQueue => 'File';

  @override
  String get quickPasteMaskedMode => 'Champ de mot de passe : aperçus masqués';

  @override
  String quickPasteMaskedItem(String kind) {
    return 'Masqué : $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'Déverrouiller les éléments secrets';

  @override
  String quickPastePastingWith(String name) {
    return 'Collage avec : $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut colle l\'original';
  }

  @override
  String get quickPasteDefaultSkipped => 'Transformation par défaut ignorée';

  @override
  String get quickPasteDefaultSkippedMessage =>
      'L\'original a été utilisé à la place.';

  @override
  String get quickPastePasteManually =>
      'Le contenu est dans le presse-papiers. Collez-le manuellement.';

  @override
  String get quickPastePaste => 'Coller';

  @override
  String get quickPasteSnippetsUnavailable =>
      'Les extraits ne sont pas disponibles';

  @override
  String get rulesLoadFailed => 'Impossible de charger les règles de contenu.';

  @override
  String get rulesNewRule => 'Nouvelle règle';

  @override
  String get rulesEditRule => 'Modifier la règle';

  @override
  String get rulesUntitled => 'Règle sans nom';

  @override
  String get rulesPrivacyDefaultsTitle =>
      'Paramètres de confidentialité par défaut';

  @override
  String get rulesPrivacyDefaultsDescription =>
      'Les réglages de confidentialité s’appliquent avant toute règle.';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'confidentialité masqué temporaire apps exclues intégrées identifiants mots de passe cartes';

  @override
  String get rulesOpenPrivacy => 'Ouvrir Confidentialité';

  @override
  String get rulesMyRulesTitle => 'Mes règles';

  @override
  String get rulesMyRulesDescription =>
      'S’exécutent dans l’ordre sur les nouvelles copies.';

  @override
  String get rulesMyRulesKeywords =>
      'règles automatisation étiquette ignorer secret expirer supprimer motif regex';

  @override
  String get rulesEmptyTitle => 'Aucune règle pour l’instant';

  @override
  String get rulesEmptyMessage =>
      'Les règles étiquettent, protègent, font expirer ou ignorent les nouvelles copies.';

  @override
  String get rulesDeletedTitle => 'Supprimées récemment';

  @override
  String get rulesDeletedDescription => 'Restaurables pendant 30 jours.';

  @override
  String rulesDeletedAt(String time) {
    return 'Supprimée : $time';
  }

  @override
  String get rulesRestore => 'Restaurer';

  @override
  String get rulesDeletedToast => 'Règle supprimée';

  @override
  String get rulesDeletedToastMessage =>
      'Vous pouvez la restaurer pendant 30 jours.';

  @override
  String get rulesUndo => 'Annuler';

  @override
  String rulesConflictMessage(String rule, String device) {
    return 'Une modification simultanée de « $rule » depuis $device a été remplacée par une plus récente.';
  }

  @override
  String get rulesAnotherDevice => 'un autre appareil';

  @override
  String get rulesRestoreConflict => 'Utiliser cette version';

  @override
  String get rulesSkipLogTitle => 'Ignorés récemment';

  @override
  String get rulesSkipLogDescription =>
      'Copies que vos règles n’ont pas enregistrées sur cet appareil.';

  @override
  String get rulesSkipLogKeywords => 'ignorés non enregistrés journal';

  @override
  String get rulesSkipLogShow => 'Afficher';

  @override
  String get rulesSkipLogEmpty => 'Aucune copie n’a été ignorée.';

  @override
  String get rulesUnknownApp => 'App inconnue';

  @override
  String get rulesMatchesEverything => 'Toute copie';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary, puis arrêter';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': ' ou ',
      'other': ' et ',
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
    return '$action : $value';
  }

  @override
  String rulesRunsOn(String devices) {
    return 'S’exécute sur $devices';
  }

  @override
  String rulesNewFromDevice(String device) {
    return 'Nouvelle de $device';
  }

  @override
  String get rulesNeedsUpdateBadge => 'Mise à jour requise';

  @override
  String get rulesUpdateRequired =>
      'Mettez à jour CopyPaste pour exécuter cette règle.';

  @override
  String get rulesErrorBadge => 'Erreur';

  @override
  String rulesErrorHint(String problem) {
    return '$problem D’ici la correction, les correspondances sont protégées.';
  }

  @override
  String get rulesReorder => 'Faire glisser pour réorganiser';

  @override
  String get rulesMoreActions => 'Plus d’actions';

  @override
  String get rulesEdit => 'Modifier';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': 'Monter',
      'down': 'Descendre',
      'top': 'Placer en haut',
      'other': 'Placer en bas',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => 'Appliquer aux éléments existants…';

  @override
  String get rulesResetBuiltin => 'Rétablir les valeurs par défaut';

  @override
  String get rulesNameLabel => 'Nom';

  @override
  String get rulesNamePlaceholder => 'Par exemple, Liens de travail';

  @override
  String get rulesWhenTitle => 'Quand';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'Une condition est remplie',
      'other': 'Toutes les conditions sont remplies',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions =>
      'Aucune condition : cette règle s’applique à toutes les copies.';

  @override
  String get rulesAddCondition => 'Ajouter une condition';

  @override
  String get rulesRemoveCondition => 'Supprimer la condition';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'Texte',
      'content_type': 'Type de contenu',
      'detected_kind': 'Type détecté',
      'source_app': 'App source',
      'size': 'Taille',
      'privacy_marker': 'Marqueur de confidentialité',
      'capture_origin': 'Origine',
      'other': 'Condition inconnue',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'contient',
      'equals': 'est',
      'starts_with': 'commence par',
      'ends_with': 'se termine par',
      'matches_regex': 'correspond au motif',
      'other': 'correspond',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'ne contient pas',
      'equals': 'n’est pas',
      'starts_with': 'ne commence pas par',
      'ends_with': 'ne se termine pas par',
      'matches_regex': 'ne correspond pas au motif',
      'other': 'ne correspond pas',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'n’est pas',
      'other': 'est',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'est inférieure à',
      'other': 'est supérieure à',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'n’est pas inférieure à',
      'other': 'n’est pas supérieure à',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'lignes',
      'bytes': 'octets',
      'other': 'caractères',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'lignes',
      'bytes': 'octets',
      'other': 'caractères',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'Texte brut',
      'rich_text': 'Texte enrichi',
      'image': 'Image',
      'files': 'Fichiers',
      'color': 'Couleur',
      'link': 'Lien',
      'other': 'Autre',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'Mot de passe ou secret',
      'url': 'Adresse web',
      'email': 'Adresse e-mail',
      'phone': 'Numéro de téléphone',
      'code': 'Code',
      'color': 'Valeur de couleur',
      'json': 'JSON',
      'file_path': 'Chemin de fichier',
      'number': 'Nombre',
      'one_time_code': 'Code à usage unique',
      'payment_card': 'Numéro de carte bancaire',
      'other': 'Autre',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': 'Masqué',
      'transient': 'Temporaire',
      'auto_generated': 'Généré automatiquement',
      'excluded_from_history': 'Exclu de l’historique',
      'other': 'Autre',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'Copié sur cet appareil',
      'received': 'Reçu d’un autre appareil',
      'created_in_app': 'Créé dans CopyPaste',
      'other': 'Autre',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => 'Texte à rechercher';

  @override
  String get rulesPatternPlaceholder => 'Motif, par exemple ^ticket-[0-9]+';

  @override
  String get rulesPatternHint =>
      'Les assertions de contexte et les références arrière ne sont pas prises en charge.';

  @override
  String get rulesCaseSensitive => 'Respecter la casse';

  @override
  String get rulesChooseValues => 'Choisir';

  @override
  String get rulesUnknownConditionNotice =>
      'Cette condition provient d’une version plus récente de CopyPaste.';

  @override
  String get rulesSourceAppNotice =>
      'Sur Android, seules les copies faites avec le clavier d’éléments correspondent.';

  @override
  String get rulesPrivacyMarkerNotice =>
      'Tous les systèmes ne signalent pas tous les marqueurs.';

  @override
  String get rulesThenTitle => 'Alors';

  @override
  String get rulesNoActions => 'Ajoutez au moins une action.';

  @override
  String get rulesAddAction => 'Ajouter une action';

  @override
  String get rulesRemoveAction => 'Supprimer l’action';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': 'Ne pas enregistrer',
      'mark_secret': 'Marquer comme secret',
      'hide_from_ai': 'Masquer aux outils d’IA',
      'delete_after': 'Supprimer après',
      'add_tags': 'Ajouter des étiquettes',
      'transform': 'Transformer',
      'other': 'Action inconnue',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone':
          '« Ne pas enregistrer » ne peut pas être combiné à d’autres actions.',
      'present': 'Cette règle comporte déjà cette action.',
      'other': 'Une règle peut comporter jusqu’à 8 actions.',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'Cette action provient d’une version plus récente de CopyPaste.';

  @override
  String get rulesDontSaveWarning =>
      'Les copies correspondantes ne sont jamais enregistrées, indexées ni synchronisées.';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count minutes',
      one: '1 minute',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count heures',
      one: '1 heure',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count jours',
      one: '1 jour',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry =>
      'Vider aussi le presse-papiers s’il contient encore cette copie.';

  @override
  String get rulesNoTags =>
      'Créez des étiquettes dans l’Historique pour les utiliser ici.';

  @override
  String get rulesNoTransforms => 'Aucune transformation disponible.';

  @override
  String get rulesReplaceClipboard =>
      'Remplacer aussi le presse-papiers par le résultat';

  @override
  String get rulesOptionsTitle => 'Options';

  @override
  String get rulesEnabledSwitch => 'Activée';

  @override
  String get rulesStopProcessing =>
      'Ne pas traiter les règles suivantes si celle-ci correspond';

  @override
  String get rulesRunsOnLabel => 'S’exécute sur';

  @override
  String get rulesAllDevices => 'Tous les appareils';

  @override
  String get rulesChosenDevices => 'Appareils choisis';

  @override
  String get rulesTestTitle => 'Tester';

  @override
  String get rulesTestDescription =>
      'Essayez la règle sur un exemple de texte.';

  @override
  String get rulesSamplePlaceholder =>
      'Collez ou saisissez un exemple de texte';

  @override
  String get rulesTestFullSet => 'Tester avec toutes les règles';

  @override
  String get rulesRunTest => 'Lancer le test';

  @override
  String get rulesTestFailed => 'Le test n’a pas pu s’exécuter.';

  @override
  String get rulesPreviewHistory => 'Aperçu sur l’historique';

  @override
  String get rulesSaveFailed => 'Impossible d’enregistrer la règle.';

  @override
  String get rulesReadOnlyNotice =>
      'Créée par une version plus récente de CopyPaste. Mettez à jour pour la modifier ou l’exécuter.';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'Impossible de lire la règle.',
      'unsupported_schema':
          'Cette règle nécessite une version plus récente de CopyPaste.',
      'unknown_condition_type':
          'Cette condition nécessite une version plus récente de CopyPaste.',
      'unknown_action_type':
          'Cette action nécessite une version plus récente de CopyPaste.',
      'too_many_conditions': 'Une règle peut comporter jusqu’à 20 conditions.',
      'too_many_actions': 'Une règle peut comporter jusqu’à 8 actions.',
      'too_many_rules': 'Seules les 200 premières règles s’exécutent.',
      'no_actions': 'Ajoutez au moins une action.',
      'conflicting_actions':
          '« Ne pas enregistrer » ne peut pas être combiné à d’autres actions.',
      'duplicate_action': 'Chaque action ne peut être utilisée qu’une fois.',
      'empty_value': 'Saisissez une valeur.',
      'empty_list': 'Choisissez au moins une valeur.',
      'too_many_values': 'Choisissez jusqu’à 64 valeurs.',
      'invalid_value': 'Cette valeur n’est pas valide.',
      'pattern_too_long':
          'Les motifs peuvent comporter jusqu’à 2 000 caractères.',
      'unsupported_look_around':
          'Les assertions avant et arrière ne sont pas prises en charge. Utilisez plutôt deux conditions avec « Toutes les conditions sont remplies ».',
      'unsupported_backreference':
          'Les références arrière ne sont pas prises en charge.',
      'pattern_too_large':
          'Ce motif est trop complexe. Simplifiez ses répétitions.',
      'invalid_pattern': 'Ce motif n’est pas valide.',
      'invalid_duration': 'Choisissez une durée de 1 minute à 90 jours.',
      'invalid_tags': 'Choisissez au moins une étiquette.',
      'invalid_transform': 'Choisissez une transformation.',
      'other': 'Cette règle présente un problème.',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceLine(String label, String result) {
    return '$label : $result';
  }

  @override
  String rulesTraceResult(String result) {
    String _temp0 = intl.Intl.selectLogic(result, {
      'matched': 'correspond',
      'not_matched': 'ne correspond pas',
      'unknown': 'inconnu',
      'skipped': 'non exécutée',
      'other': 'erreur',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return 'Condition $number';
  }

  @override
  String get rulesTraceNoRules => 'Aucune règle n’a été évaluée.';

  @override
  String get rulesTraceStopped =>
      'Les règles suivantes n’ont pas été exécutées.';

  @override
  String get rulesTraceUnknownReason =>
      'Inconnu pour cette copie, donc pas de correspondance.';

  @override
  String get rulesTraceRuleError =>
      'Cette règle n’a pas pu s’exécuter, les correspondances sont donc protégées.';

  @override
  String get rulesTraceOutcomeTitle => 'Résultat';

  @override
  String get rulesTraceNoEffect => 'Aucune règle ne modifie cette copie.';

  @override
  String get rulesTraceSkipped => 'Non enregistrée';

  @override
  String get rulesTraceSecret => 'Enregistrée comme secrète';

  @override
  String get rulesTraceHidden => 'Masquée aux outils d’IA';

  @override
  String rulesTraceExpires(String duration) {
    return 'Supprimée après $duration';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'Étiquetée : $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return 'Transformée avec $names';
  }

  @override
  String get rulesTraceOutput => 'Texte transformé';

  @override
  String get rulesPreviewTitle => 'Appliquer aux éléments existants';

  @override
  String rulesPreviewRunning(int count) {
    return 'Vérification des $count derniers éléments…';
  }

  @override
  String get rulesPreviewNone =>
      'Aucun élément récent ne correspond à cette règle.';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments correspondent',
      one: '1 élément correspond',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count seraient supprimés',
      one: '1 serait supprimé',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count seraient marqués comme secrets',
      one: '1 serait marqué comme secret',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count seraient masqués aux outils d’IA',
      one: '1 serait masqué aux outils d’IA',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count recevraient une date de suppression',
      one: '1 recevrait une date de suppression',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count seraient étiquetés',
      one: '1 serait étiqueté',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return 'Seuls les $count derniers éléments ont été vérifiés.';
  }

  @override
  String rulesPreviewMore(int count) {
    return 'et $count de plus';
  }

  @override
  String get rulesPreviewNote => 'Rien ne change avant que vous n’appliquiez.';

  @override
  String get rulesSecretItem => 'Élément secret';

  @override
  String rulesSecretItemFrom(String app) {
    return 'Élément secret de $app';
  }

  @override
  String get rulesPreviewCancelled => 'L’aperçu a été arrêté. Rien n’a changé.';

  @override
  String get rulesPreviewFailed => 'L’aperçu n’a pas pu s’exécuter.';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Supprimer $count éléments ?',
      one: 'Supprimer 1 élément ?',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage =>
      'Les clips correspondants sont supprimés sur tous vos appareils. Action irréversible.';

  @override
  String get rulesApplyDeleteConfirm => 'Supprimer et appliquer';

  @override
  String rulesApplyProgress(int done, int total) {
    return 'Application : $done sur $total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments modifiés',
      one: '1 élément modifié',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments supprimés',
      one: '1 élément supprimé',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments n’ont pas pu être modifiés',
      one: '1 élément n’a pas pu être modifié',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled =>
      'Arrêté. Les modifications déjà faites sont conservées.';

  @override
  String get historyActionEdit => 'Modifier';

  @override
  String get historyActionSaveAsSnippet => 'Enregistrer comme extrait';

  @override
  String get historyActionAddToQueue => 'Ajouter à Paste Queue';

  @override
  String get historyActionTransform => 'Transformer…';

  @override
  String get historyActionExplain => 'Pourquoi est-ce ici ?';

  @override
  String get historyActionKeep => 'Conserver';

  @override
  String get historyNewClip => 'Nouvel élément';

  @override
  String get historySavedTitle => 'Enregistré';

  @override
  String get historyEditSaved =>
      'Le texte modifié est un nouvel élément en haut de l\'historique.';

  @override
  String get historyNewClipSaved => 'Votre texte a été enregistré.';

  @override
  String get historySnippetSaved =>
      'Le texte de l\'élément est maintenant un extrait.';

  @override
  String get historySnippetSaveFailed =>
      'Impossible d\'enregistrer cet élément comme extrait.';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count éléments ont rejoint la file.',
      one: '1 élément a rejoint la file.',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed =>
      'Impossible d\'ajouter à Paste Queue. Vérifiez qu\'elle est activée.';

  @override
  String get historyHideFromAiFailed =>
      'Impossible de modifier le réglage des outils d\'IA pour cet élément.';

  @override
  String get historyKeepFailed => 'Impossible de conserver cet élément.';

  @override
  String get historyExplainFailed => 'Impossible de charger l\'explication.';

  @override
  String get historyDeletesSoon => 'Supprimé dans moins d’une minute';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Supprimé dans $count min',
      one: 'Supprimé dans 1 min',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Supprimé dans $count h',
      one: 'Supprimé dans 1 h',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Supprimé dans $count jours',
      one: 'Supprimé dans 1 jour',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => 'Manuel';

  @override
  String get historyProvenanceEdited => 'Modifié';

  @override
  String get historyProvenanceTransformed => 'Transformé';

  @override
  String get historyProvenanceAiTool => 'Outil d\'IA';

  @override
  String historyRuleTag(String name) {
    return '$name (auto)';
  }

  @override
  String get historyMetadataExpiry => 'Suppression';

  @override
  String get historyMetadataRuleTags => 'Étiquettes automatiques';

  @override
  String get historyMetadataAiTools => 'Outils d\'IA';
}
