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
      'Relancer les services d’arrière-plan activés après le redémarrage de l’appareil.';

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
      'Choisissez la capture complète ou limitée et gérez la configuration unique avec Shizuku ou ADB.';

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
      'Autorisez l’accès à toutes les photos et aux notifications pour enregistrer les nouvelles captures d’écran en arrière-plan.';

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
      'Limite pour l’ajout et le téléchargement de fichiers. Le contenu des fichiers n’est transféré que lorsque vous choisissez Télécharger.';

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
      'La sauvegarde chiffrée sélectionnée remplacera l’historique du presse-papiers de cet appareil. Les paramètres actuels et l’identité de l’appareil sont conservés.';

  @override
  String get settingsRestoreFailed => 'Impossible de restaurer la sauvegarde.';

  @override
  String get settingsSyncSwitchTitle => 'Synchronisation';

  @override
  String get settingsSyncSwitchDescription =>
      'Interrupteur principal de la synchronisation des appareils associés.';

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
      'Copier automatiquement dans ce presse-papiers les éléments plus récents provenant d’autres appareils.';

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
  String get settingsExcludedAppsMacDescription =>
      'Ignorer la capture automatique pendant l’activité de ces apps. Les copies en arrière-plan peuvent contourner les exclusions.';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Ignorer la capture automatique depuis les propriétaires du presse-papiers identifiés. La capture est suspendue lorsque des exclusions sont définies et que la source ne peut pas être identifiée.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'Android ignore la capture automatique tant que des exclusions sont définies, car il ne peut pas identifier les apps sources.';

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
      'Exclure de l’Historique les secrets signalés par l’app d’origine.';

  @override
  String get settingsSkipSecretKeywords =>
      'secret mot de passe sensible masqué spoiler secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle =>
      'Ignorer le presse-papiers temporaire';

  @override
  String get settingsSkipTransientDescription =>
      'Exclure de l’Historique les copies temporaires signalées par l’app d’origine.';

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
      'Le raccourci fonctionne lorsque CopyPaste s’exécute en arrière-plan.';

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
      'Associez un autre appareil pour synchroniser l’historique du presse-papiers entre eux. Les deux appareils doivent être sur le même réseau, et chaque association est confirmée sur les deux appareils.';

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
    return 'Nouvelle limite : $limit. Les éléments non épinglés plus anciens sont supprimés de cet appareil maintenant et ne peuvent pas être récupérés. Les éléments épinglés sont conservés.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle =>
      'Réduire le quota de stockage ?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Nouveau quota : $size. Si l’historique non épinglé occupe davantage, les éléments les plus anciens sont supprimés maintenant et ne peuvent pas être récupérés. Les éléments épinglés sont conservés.';
  }

  @override
  String get settingsLimitConfirmAction => 'Appliquer la limite';

  @override
  String get settingsHideEverywhereTitle => 'Masquer complètement CopyPaste ?';

  @override
  String get settingsHideEverywhereMessage =>
      'Lorsque les deux options sont désactivées, CopyPaste n’a aucune icône visible. Pour l’ouvrir de nouveau, relancez CopyPaste.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Lorsque les deux options sont désactivées, CopyPaste n’a aucune icône visible. Pour l’ouvrir de nouveau, appuyez sur $shortcut ou relancez CopyPaste.';
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
    return 'Nouveau quota : $size. L’historique non épinglé occupe $used, donc ses éléments les plus anciens sont supprimés maintenant jusqu’à ce qu’il tienne. Ils ne peuvent pas être récupérés. Les éléments épinglés sont conservés.';
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
      'Le système conserve le texte des menus hors du chiffrement de CopyPaste. Affichez-en moins pour que le contenu des clips n\'y figure pas. Les secrets sont toujours masqués.';

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
      'Le résultat est un nouvel élément de l\'historique. Le presse-papiers est inchangé.';

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
      'Après un collage transformé, le presse-papiers précédent revient, sauf si autre chose a été copié entre-temps.';

  @override
  String get transformsDefaultTitle => 'Transformation par défaut au collage';

  @override
  String get transformsDefaultDescription =>
      'Quick Paste applique cette chaîne au texte et l\'indique avant le collage. En cas d\'échec, l\'original est collé.';

  @override
  String get transformsNoDefault => 'Aucune';

  @override
  String get transformsAppDefaultsTitle => 'Valeurs par défaut par app';

  @override
  String get transformsAppDefaultsDescription =>
      'Remplacez la valeur par défaut d\'une app via son identifiant de bundle, son exécutable ou son nom de paquet. Les entrées d\'une autre plateforme se synchronisent sans effet ici.';

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
      'Les favoris apparaissent en premier dans chaque palette. Les neuf premiers s\'exécutent avec Mod+1 à Mod+9.';

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
      'Enchaînements de transformations enregistrés avec leurs réglages. Ils se synchronisent avec vos autres appareils.';

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
      'Les favoris et les valeurs par défaut qui l\'utilisent repassent à aucune sur tous les appareils.';

  @override
  String get transformsDeletePipelineFailed =>
      'Impossible de supprimer la chaîne.';

  @override
  String get transformsAddStep => 'Ajouter une étape';

  @override
  String get transformsHiddenTitle => 'Transformations masquées';

  @override
  String get transformsHiddenDescription =>
      'Les transformations masquées n\'apparaissent ni dans les palettes ni dans les suggestions. Les chaînes qui les utilisent fonctionnent toujours.';

  @override
  String get transformsHideTransform => 'Masquer une transformation';

  @override
  String get transformsShow => 'Afficher';

  @override
  String get transformsSuggestionsTitle => 'Suggestions';

  @override
  String get transformsSuggestionsDescription =>
      'À l\'ouverture d\'une palette, les 64 premiers Kio de son texte sont analysés selon les types ci-dessous pour suggérer jusqu\'à trois transformations. Rien ne s\'exécute en arrière-plan.';

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
      'La liste intégrée est fournie avec l\'app et ne change qu\'avec les mises à jour. Ajoutez des noms à supprimer ou à conserver ; écrivez nom@domaine pour limiter à un domaine.';

  @override
  String get transformsTrackingRemoveLabel => 'Supprimer aussi';

  @override
  String get transformsTrackingAllowLabel => 'Toujours conserver';

  @override
  String get transformsTrackingPlaceholder => 'nom ou nom@domaine, puis Entrée';
}
