// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Spanish Castilian (`es`).
class AppLocalizationsEs extends AppLocalizations {
  AppLocalizationsEs([String locale = 'es']) : super(locale);

  @override
  String get commonCancel => 'Cancelar';

  @override
  String get commonClose => 'Cerrar';

  @override
  String get commonDone => 'Listo';

  @override
  String get commonRetry => 'Reintentar';

  @override
  String get commonTryAgain => 'Volver a intentar';

  @override
  String get commonCopy => 'Copiar';

  @override
  String get commonDelete => 'Eliminar';

  @override
  String get commonSave => 'Guardar';

  @override
  String get commonContinue => 'Continuar';

  @override
  String get commonBack => 'Atrás';

  @override
  String get commonOpenSettings => 'Abrir ajustes';

  @override
  String get commonLoading => 'Cargando';

  @override
  String get commonApply => 'Aplicar';

  @override
  String get commonClear => 'Borrar';

  @override
  String get commonPin => 'Fijar';

  @override
  String get commonEnable => 'Activar';

  @override
  String get settingsLanguageTitle => 'Idioma';

  @override
  String get settingsLanguageDescription =>
      'Idioma de la interfaz de CopyPaste.';

  @override
  String get settingsLanguageSystem => 'Predeterminado del sistema';

  @override
  String get settingsLanguageKeywords =>
      'idioma región traducción interfaz language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'No se pudo actualizar la opción Abrir al iniciar sesión. Vuelve a intentarlo.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'No se pudieron abrir los ajustes de inicio del sistema.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'No se pudieron actualizar los ajustes. Vuelve a intentarlo.';

  @override
  String get settingsRuntimeLoadFailed => 'No se pudieron cargar los ajustes.';

  @override
  String get settingsRuntimeAppAlreadyExcluded =>
      'Esa aplicación ya está excluida.';

  @override
  String get settingsRuntimeChangeFailed =>
      'No se pudo completar el cambio solicitado.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Hay que conceder todos los permisos de captura del portapapeles para reanudar la captura.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'La captura del portapapeles está en pausa. El servicio de captura de Android no confirmó que se haya detenido.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'Se necesita el permiso de notificaciones para activar las notificaciones de captura.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'CopyPaste no pudo leer los ajustes guardados. Se aplican valores de protección; revisa y guarda tus preferencias para repararlos.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'CopyPaste recuperó los ajustes ilegibles con valores de protección. Revisa y guarda tus preferencias para repararlos.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Los identificadores de aplicación no pueden estar vacíos ni superar los 256 bytes.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'Se pueden excluir hasta 256 aplicaciones.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'No se pudo actualizar el pegado automático.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'No se pudo registrar ese atajo.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'No se pudo pausar el atajo actual.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'No se pudo restaurar el atajo.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'No se pudo abrir Quick Paste.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'No se pudo solicitar el permiso de Accesibilidad.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'No se pudo actualizar el estado de la integración con Linux.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'No se concedió el permiso de entrada al portapapeles.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'No se pudo solicitar el permiso de entrada al portapapeles.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'No se pudo abrir la configuración de la integración con el escritorio.';

  @override
  String get settingsRuntimeTextHistoryFileType =>
      'Historial de texto de CopyPaste';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'Copia de seguridad cifrada de CopyPaste';

  @override
  String get settingsRuntimeNotificationImage => 'Imagen';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Imagen · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'Archivo';

  @override
  String get updateRestoreInstallationFailed =>
      'CopyPaste no pudo restaurar la instalación de la actualización.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Esta instalación de Linux no se puede actualizar automáticamente.';

  @override
  String get updateMacosPackageMissing =>
      'Esta versión no incluye un paquete de actualización firmado para macOS.';

  @override
  String get updateInstalledVersionInvalid =>
      'La versión instalada de CopyPaste no es válida.';

  @override
  String get updateCheckUnreachable =>
      'CopyPaste no pudo conectarse a GitHub para buscar actualizaciones.';

  @override
  String get updateCheckTimedOut =>
      'Se agotó el tiempo de espera al buscar actualizaciones.';

  @override
  String get updateCheckDeviceFailed =>
      'CopyPaste no pudo buscar actualizaciones en este dispositivo.';

  @override
  String get updateCheckFailed => 'CopyPaste no pudo buscar actualizaciones.';

  @override
  String get updateDownloadFailed =>
      'CopyPaste no pudo descargar la actualización.';

  @override
  String get updateDownloadTimedOut =>
      'Se agotó el tiempo de espera de la descarga de la actualización.';

  @override
  String get updateInstallFailed =>
      'CopyPaste no pudo instalar la actualización.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Continúa en el instalador del sistema de Android.';

  @override
  String get updateContinueInPackageManager =>
      'Continúa en el gestor de paquetes del sistema.';

  @override
  String get updateInstallerStarting => 'Iniciando el instalador.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Permite que CopyPaste instale apps en los ajustes de Android y luego continúa.';

  @override
  String get updatePermissionRequiredLinux =>
      'Autentícate en el gestor de paquetes del sistema para instalar esta actualización.';

  @override
  String get updatePermissionRequired =>
      'Permite que CopyPaste instale esta actualización y luego continúa.';

  @override
  String get updateRestartRequired =>
      'La actualización está instalada. Reinicia CopyPaste para usarla.';

  @override
  String get updateRestarting => 'Reiniciando CopyPaste.';

  @override
  String get updateRestartFailed =>
      'CopyPaste no pudo reiniciarse. Vuelve a intentarlo.';

  @override
  String get updateReleasePageOpenFailed =>
      'CopyPaste no pudo abrir la página de la versión.';

  @override
  String get updatePackageInvalid =>
      'La actualización descargada no es un paquete válido de CopyPaste.';

  @override
  String get updateSignatureInvalid =>
      'La actualización descargada tiene una firma no válida.';

  @override
  String get updateDowngradeRefused =>
      'El paquete descargado no es más reciente que esta versión.';

  @override
  String get updateHomebrewRequired =>
      'Instala CopyPaste con Homebrew para actualizarlo aquí.';

  @override
  String get updateInstallationCancelled =>
      'Se canceló la instalación de la actualización.';

  @override
  String get updateInstallationBlocked =>
      'Android bloqueó la instalación de la actualización.';

  @override
  String get updateInstallationConflict =>
      'La actualización entra en conflicto con la app instalada.';

  @override
  String get updateInstallationIncompatible =>
      'La actualización no es compatible con este dispositivo.';

  @override
  String get updateInstallationStorage =>
      'Libera espacio de almacenamiento para instalar la actualización.';

  @override
  String get updateInstallationInterrupted =>
      'Se interrumpió la instalación de la actualización. Vuelve a intentarlo.';

  @override
  String get updateInstallationBusy =>
      'Ya hay una instalación de actualización en curso.';

  @override
  String get updatePackageDetailsInvalid =>
      'Los datos del paquete de actualización no son válidos.';

  @override
  String get updateIntegrityCheckFailed =>
      'La actualización descargada no superó la comprobación de integridad.';

  @override
  String get updateInstallerLaunchFailed =>
      'CopyPaste no pudo iniciar el instalador del sistema.';

  @override
  String get updateInstallLocationTranslocated =>
      'Mueve CopyPaste a la carpeta Aplicaciones y ábrelo desde allí para actualizarlo.';

  @override
  String get updateInstallLocationUnwritable =>
      'CopyPaste no puede reemplazarse en su carpeta actual. Usa la página de la versión para actualizarlo.';

  @override
  String get updateReplacementBlocked =>
      'macOS bloqueó el reemplazo de CopyPaste. Permítelo en los ajustes de Gestión de apps o usa la página de la versión.';

  @override
  String get updatePackageNotDownloadable =>
      'Esta plataforma no usa un paquete de actualización descargable.';

  @override
  String get updatePackageSizeInvalid =>
      'El tamaño del paquete de actualización no es válido.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'La actualización descargada tiene una firma del actualizador no válida.';

  @override
  String get updatePackageUrlUntrusted =>
      'La URL del paquete de actualización no es de confianza.';

  @override
  String get updateDownloadLeftGitHub =>
      'La descarga de la actualización salió de GitHub.';

  @override
  String get updateDownloadTooManyRedirects =>
      'La descarga de la actualización usó demasiadas redirecciones.';

  @override
  String get updatePackageTooLarge =>
      'El paquete de actualización es demasiado grande.';

  @override
  String get updateSignatureSizeInvalid =>
      'El tamaño de la firma del actualizador no es válido.';

  @override
  String get updateMetadataInvalid =>
      'GitHub devolvió metadatos de actualización no válidos.';

  @override
  String get updateMetadataTooLarge =>
      'Los metadatos de la actualización son demasiado grandes.';

  @override
  String get quickPasteActionFailedTitle => 'Error en la acción';

  @override
  String get quickPasteOpenAppSemantics => 'Abrir CopyPaste';

  @override
  String get quickPasteSearchPlaceholder => 'Escribe para buscar…';

  @override
  String get quickPasteLoadingHistory =>
      'Cargando el historial del portapapeles.';

  @override
  String get quickPasteHistoryUnavailable => 'El historial no está disponible';

  @override
  String get quickPasteNoClipsFound => 'No se encontraron recortes';

  @override
  String get quickPasteLoadMore => 'Cargar más';

  @override
  String get quickPasteTransferFailed => 'Error';

  @override
  String get quickPasteClipUnavailable => 'Recorte no disponible';

  @override
  String get quickPasteFileFallbackName => 'Archivo';

  @override
  String get quickPastePathUnavailable => 'Ruta no disponible';

  @override
  String get quickPastePreferences => 'Preferencias…';

  @override
  String get quickPasteAbout => 'Acerca de';

  @override
  String get quickPasteQuit => 'Salir';

  @override
  String get quickPasteClearHistoryTitle => '¿Borrar el historial?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Los recortes fijados se mantendrán en el historial.';

  @override
  String get quickPasteClearHistoryFailed =>
      'No se pudo eliminar el historial.';

  @override
  String get quickPasteAboutDescription =>
      'Historial privado del portapapeles en macOS, Android, Windows y Linux.';

  @override
  String get transfersTitle => 'Descargas y transferencias';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count transferencias activas',
      many: '$count transferencias activas',
      one: '$count transferencia activa',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count activas',
      many: '$count activas',
      one: '$count activa',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Recientes';

  @override
  String get transfersShowEarlier => 'Anteriores';

  @override
  String get transfersClearCompletedHistory =>
      'Borrar transferencias completadas';

  @override
  String get transfersErrorTitle => 'Transferencias';

  @override
  String get transfersEmptyTitle => 'No hay transferencias';

  @override
  String get transfersEmptyMessage =>
      'Aquí aparecen las importaciones de archivos, las descargas y las transferencias salientes.';

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
  String get transfersRemoveFromHistory => 'Quitar del historial';

  @override
  String get transfersCancelDownload => 'Cancelar descarga';

  @override
  String get transfersShowFile => 'Mostrar archivo';

  @override
  String get transfersPhaseSending => 'Enviando';

  @override
  String get transfersPhaseDownloading => 'Descargando';

  @override
  String get transfersPhaseAdding => 'Añadiendo';

  @override
  String get transfersPhaseAdded => 'Añadido';

  @override
  String get transfersPhaseSent => 'Enviado';

  @override
  String get transfersPhaseCompleted => 'Completado';

  @override
  String get transfersPhaseCancelled => 'Cancelado';

  @override
  String get transfersPhaseInterrupted => 'Interrumpido';

  @override
  String get transfersPhaseFailed => 'Error';

  @override
  String get transfersUpdatesUnavailable =>
      'El progreso de las transferencias no está disponible. Vuelve a abrir para actualizar.';

  @override
  String get transfersLoadFailed =>
      'No se pudieron cargar las transferencias. Vuelve a intentarlo.';

  @override
  String get transfersUpdatesLoadFailed =>
      'No se pudo cargar el estado actualizado de las transferencias.';

  @override
  String get transfersRetryFailed => 'No se pudo reintentar este archivo.';

  @override
  String get transfersClearHistoryFailed =>
      'No se pudo borrar el historial de transferencias.';

  @override
  String get transfersRemoveRecordFailed =>
      'No se pudo quitar este registro del historial.';

  @override
  String get transfersCancelFailed =>
      'No se pudo cancelar la transferencia. Es posible que siga en curso.';

  @override
  String get transfersSavedFileUnavailable =>
      'El archivo guardado no está disponible. Es posible que se haya movido o eliminado.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Notificaciones · $count sin leer',
      many: 'Notificaciones · $count sin leer',
      one: 'Notificaciones · $count sin leer',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Notificaciones';

  @override
  String get notificationsClearRead => 'Borrar notificaciones leídas';

  @override
  String get notificationsEmptyTitle => 'No hay notificaciones';

  @override
  String get notificationsEmptyMessage =>
      'Aquí aparecen las solicitudes de conexión de dispositivos y sus resultados.';

  @override
  String get notificationsSystemAlertTitle => 'Notificaciones del sistema';

  @override
  String get notificationsReview => 'Revisar';

  @override
  String get notificationsConnectionRequest => 'Solicitud de conexión';

  @override
  String get notificationsConfirmConnection => 'Confirmar conexión';

  @override
  String get notificationsDeviceConnected => 'Dispositivo conectado';

  @override
  String get notificationsConnectionRejected => 'Conexión rechazada';

  @override
  String get notificationsConnectionCancelled => 'Conexión cancelada';

  @override
  String get notificationsConnectionRequestExpired =>
      'La solicitud de conexión caducó';

  @override
  String get notificationsConnectionFailed => 'Error de conexión';

  @override
  String get notificationsNearbyDevice => 'Dispositivo cercano';

  @override
  String get notificationsPermissionDenied =>
      'Permite las notificaciones en los ajustes del sistema operativo para recibir solicitudes en segundo plano.';

  @override
  String get notificationsSystemUnavailable =>
      'Las notificaciones del sistema no están disponibles. Las solicitudes de conexión seguirán apareciendo aquí.';

  @override
  String get platformCaptureStateUnavailable => 'No disponible';

  @override
  String get platformCaptureStatePaused => 'En pausa';

  @override
  String get platformCaptureStateRunning => 'Capturando';

  @override
  String get platformCaptureStateStopped => 'Detenida';

  @override
  String get platformCaptureActionUnavailable => 'Captura no disponible';

  @override
  String get platformCaptureActionResume => 'Reanudar captura';

  @override
  String get platformCaptureActionPause => 'Pausar captura';

  @override
  String get platformCaptureActionResumeStopped =>
      'Reanudar captura (detenida)';

  @override
  String get platformTrayOpen => 'Abrir';

  @override
  String get platformTraySettings => 'Ajustes';

  @override
  String get platformTrayQuit => 'Salir';

  @override
  String get platformWindowControlsUnavailable =>
      'Los controles de ventana del escritorio no están disponibles. CopyPaste seguirá visible.';

  @override
  String get platformTrayUnavailable =>
      'La bandeja del sistema no está disponible. Mantén CopyPaste abierto y vuelve a intentarlo o sal de la app.';

  @override
  String get platformQuitFailed =>
      'CopyPaste no pudo cerrarse. Vuelve a intentarlo.';

  @override
  String get platformVisibilityChangeFailed =>
      'No se pudo cambiar la visibilidad de la aplicación. Vuelve a intentarlo.';

  @override
  String get platformCaptureNotificationTitle => 'Portapapeles guardado';

  @override
  String get platformCaptureNotificationBody =>
      'Se añadió un nuevo elemento a CopyPaste.';

  @override
  String get platformPairingNotificationTitle => 'Solicitud de conexión';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName quiere conectarse. Abre CopyPaste para comparar el código y confirmar.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Solicitudes de conexión de dispositivos';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Solicitudes para conectar dispositivos cercanos';

  @override
  String get platformNotificationDefaultAction => 'Abrir';

  @override
  String get platformFilesSelectionFailed =>
      'No se pudieron seleccionar los archivos.';

  @override
  String get platformFilesOnlyRegular =>
      'Solo se pueden importar archivos normales.';

  @override
  String get platformModulesUnavailable =>
      'Los módulos no están disponibles en este dispositivo.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'No se pudo configurar el acceso a SMS.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'No se pudo verificar el acceso a SMS.';

  @override
  String get platformUpdatesUnavailableHere =>
      'Las actualizaciones de la aplicación no están disponibles aquí.';

  @override
  String get platformUpdateVersionUnavailable =>
      'La versión instalada no está disponible.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'Las actualizaciones de la aplicación no están disponibles en esta instalación.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Esta instalación de Linux no se puede actualizar automáticamente.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Esta actualización de Linux no coincide con la arquitectura en uso.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Esta instalación de macOS no se puede actualizar automáticamente.';

  @override
  String get platformUpdateStartFailed =>
      'CopyPaste no pudo iniciar la actualización.';

  @override
  String get platformGlobalShortcutDescription => 'Abrir Quick Paste';

  @override
  String get settingsSectionGeneral => 'General';

  @override
  String get settingsSectionGeneralSummary =>
      'Idioma y visibilidad de la aplicación.';

  @override
  String get settingsSectionAppearance => 'Apariencia';

  @override
  String get settingsSectionAppearanceSummary =>
      'Tema y material de la ventana.';

  @override
  String get settingsAppearanceDescription => 'Tema y aspecto de las ventanas.';

  @override
  String get settingsThemeTitle => 'Tema';

  @override
  String get settingsThemeDescription => 'Claro, oscuro o como el sistema.';

  @override
  String get settingsThemeKeywords => 'modo oscuro modo claro esquema de color';

  @override
  String get settingsThemeSystem => 'Sistema';

  @override
  String get settingsThemeLight => 'Claro';

  @override
  String get settingsThemeDark => 'Oscuro';

  @override
  String get settingsAccentTitle => 'Color de acento';

  @override
  String get settingsAccentDescription =>
      'Resalta la selección y la ubicación actual.';

  @override
  String get settingsAccentKeywords => 'color de resaltado tono';

  @override
  String get settingsAccentSystem => 'Sistema';

  @override
  String get settingsAccentStandard => 'Predeterminado';

  @override
  String get settingsAccentPurple => 'Morado';

  @override
  String get settingsAccentPink => 'Rosa';

  @override
  String get settingsAccentRed => 'Rojo';

  @override
  String get settingsAccentOrange => 'Naranja';

  @override
  String get settingsAccentYellow => 'Amarillo';

  @override
  String get settingsAccentGreen => 'Verde';

  @override
  String get settingsAccentTeal => 'Turquesa';

  @override
  String get settingsAccentGraphite => 'Grafito';

  @override
  String get settingsSectionClipboard => 'Portapapeles';

  @override
  String get settingsSectionClipboardSummary =>
      'Captura y límites del historial.';

  @override
  String get settingsSectionPrivacy => 'Privacidad';

  @override
  String get settingsSectionPrivacySummary =>
      'Exclusiones de aplicaciones y protección de pantalla.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Atajo y comportamiento del pegado automático.';

  @override
  String get settingsSectionSync => 'Sincronización';

  @override
  String get settingsSectionSyncSummary =>
      'Sincronización y detección de dispositivos cercanos.';

  @override
  String get settingsSectionNotifications => 'Notificaciones';

  @override
  String get settingsSectionNotificationsSummary =>
      'Notificaciones y sonidos de captura.';

  @override
  String get settingsSectionData => 'Datos';

  @override
  String get settingsSectionDataSummary =>
      'Exportación, copia de seguridad y restauración.';

  @override
  String get settingsSectionModules => 'Módulos';

  @override
  String get settingsSectionModulesSummary =>
      'Instalación y gestión de módulos opcionales.';

  @override
  String get settingsSectionRules => 'Reglas de contenido';

  @override
  String get settingsSectionRulesSummary =>
      'Etiqueta, omite, protege o caduca clips automáticamente.';

  @override
  String get settingsSectionQuickAccess => 'Acceso rápido';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Teclado, iconos y widget de la pantalla de inicio.';

  @override
  String get settingsSectionTransforms => 'Transformaciones de texto';

  @override
  String get settingsSectionTransformsSummary =>
      'Favoritos, secuencias y transformaciones predeterminadas al pegar.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Reúne varias copias y pégalas en orden.';

  @override
  String pasteQueuePasted(int position, int total) {
    return 'Pegado $position de $total';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'El elemento $position de $total está en el portapapeles';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementos pegados',
      one: '1 elemento pegado',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementos están en el portapapeles',
      one: '1 elemento está en el portapapeles',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Cola terminada';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Añadido, $total en la cola',
      one: 'Añadido, 1 en la cola',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'Cola vacía, no se pegó nada';

  @override
  String get pasteQueueMixed =>
      'Pegar todo funciona con colas solo de texto o solo de archivos';

  @override
  String get pasteQueueSeveralFiles => 'Pega los archivos de uno en uno.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementos ya no están disponibles',
      one: '1 elemento ya no está disponible',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => 'Pegar siguiente solo copia';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Elemento copiado. Permite Accesibilidad para pegar automáticamente.';

  @override
  String get pasteQueuePasteUnsupported =>
      'No se pudo pegar automáticamente. Se copió el elemento.';

  @override
  String get pasteQueueTargetElevated =>
      'No se puede pegar en una app de administrador. Se copió el elemento.';

  @override
  String get pasteQueueNoTarget =>
      'No hay app donde pegar. Se copió el elemento.';

  @override
  String get pasteQueueCollectingStarted => 'Recopilando copias';

  @override
  String get pasteQueueCollectingStopped => 'Recopilación detenida';

  @override
  String get pasteQueueFailed =>
      'No se pudo completar la acción de Paste Queue';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pendientes',
      one: '1 pendiente',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pendientes · Recopilando',
      one: '1 pendiente · Recopilando',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Recopilando';

  @override
  String get pasteQueueFinishedSummary => 'Terminada';

  @override
  String pasteQueueFull(int max) {
    return 'Cola llena ($max)';
  }

  @override
  String get pasteQueueFullMessage => 'Las copias nuevas ya no se añaden.';

  @override
  String get pasteQueueEmptyTitle => 'Paste Queue está vacía';

  @override
  String get pasteQueueEmptyMessage =>
      'Recopila copias o añade elementos desde el historial.';

  @override
  String get pasteQueueCollectingEmptyTitle => 'Copia elementos para añadirlos';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Cada copia se une a la cola hasta que detengas la recopilación.';

  @override
  String get pasteQueueStartCollecting => 'Empezar a recopilar';

  @override
  String get pasteQueueStopCollecting => 'Dejar de recopilar';

  @override
  String get pasteQueueLoading => 'Cargando la cola…';

  @override
  String get pasteQueueLoadFailed => 'No se pudo cargar la cola';

  @override
  String get pasteQueuePasteNext => 'Pegar siguiente';

  @override
  String get pasteQueueCopyNext => 'Copiar siguiente';

  @override
  String get pasteQueuePasteAll => 'Pegar todo';

  @override
  String get pasteQueueCopyAll => 'Copiar todo';

  @override
  String get pasteQueueReverse => 'Invertir orden';

  @override
  String get pasteQueueStepBack => 'Retroceder';

  @override
  String get pasteQueueSkip => 'Omitir';

  @override
  String get pasteQueueRemovePasted => 'Quitar los pegados';

  @override
  String get pasteQueueCleared => 'Cola vaciada';

  @override
  String get pasteQueueUndo => 'Deshacer';

  @override
  String get pasteQueueStartFresh => 'Empezar de cero';

  @override
  String get pasteQueueAppend => 'Añadir al final';

  @override
  String get pasteQueueRestartTitle => '¿Empezar una nueva recopilación?';

  @override
  String get pasteQueueRestartMessage =>
      'Empezar de cero quita los elementos pegados. Añadir al final los conserva.';

  @override
  String pasteQueueStale(String time) {
    return 'Cola del $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Pegado';

  @override
  String get pasteQueueStatusSkipped => 'Omitido';

  @override
  String get pasteQueueStatusUnavailable => 'Ya no disponible';

  @override
  String get pasteQueueEdited => 'Editado';

  @override
  String get pasteQueueSecret => 'Secreto';

  @override
  String get pasteQueueKindImage => 'Imagen';

  @override
  String get pasteQueueKindFiles => 'Archivos';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Generado a las $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Elemento $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'Movido a la posición $position';
  }

  @override
  String get pasteQueueMoveUp => 'Subir';

  @override
  String get pasteQueueMoveDown => 'Bajar';

  @override
  String get pasteQueueMoveToTop => 'Mover al principio';

  @override
  String get pasteQueueMoveToBottom => 'Mover al final';

  @override
  String get pasteQueueReorder => 'Arrastra para reordenar';

  @override
  String get pasteQueueMoreActions => 'Más acciones';

  @override
  String get pasteQueuePasteNow => 'Pegar ahora';

  @override
  String get pasteQueueEdit => 'Editar texto';

  @override
  String get pasteQueueUseOriginal => 'Usar el texto original';

  @override
  String get pasteQueueRemove => 'Quitar de la cola';

  @override
  String get pasteQueueEditTitle => 'Editar texto en cola';

  @override
  String get pasteQueueEditMessage =>
      'Los cambios solo se aplican a esta cola.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Configurar atajos';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Recopila y pega sin abrir CopyPaste.';

  @override
  String get pasteQueueShortcutToggleCollecting =>
      'Iniciar o detener la recopilación';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Pegar siguiente como texto sin formato';

  @override
  String get pasteQueueShortcutDescription =>
      'Un atajo global que funciona en cualquier app.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Otro atajo de CopyPaste usa esta combinación. Se mantiene el anterior.';

  @override
  String get pasteQueueShortcutInUse => 'Otra app usa esta combinación.';

  @override
  String get pasteQueueShortcutRejected =>
      'El sistema rechazó esta combinación. Se mantiene la anterior.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt escribe caracteres en muchas distribuciones. Elige otra.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Los atajos globales no están disponibles en esta sesión.';

  @override
  String get pasteQueueShortcutFailed => 'No se pudo configurar este atajo.';

  @override
  String get pasteQueueEnableTitle => 'Activar Paste Queue';

  @override
  String get pasteQueueEnableDescription =>
      'Si está desactivada, los atajos y la recopilación se detienen.';

  @override
  String get pasteQueuePasteNextStyleTitle => 'Pegar siguiente hace';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Pega automáticamente o solo copia al portapapeles.';

  @override
  String get pasteQueuePasteIntoApp => 'Pegar en la app activa';

  @override
  String get pasteQueueClipboardOnly => 'Solo al portapapeles';

  @override
  String get pasteQueueInsertTitle => 'Los elementos nuevos van';

  @override
  String get pasteQueueInsertDescription =>
      'Dónde se unen a la cola los elementos recopilados y añadidos.';

  @override
  String get pasteQueueInsertEnd =>
      'Al final (primero en entrar, primero en salir)';

  @override
  String get pasteQueueInsertStart =>
      'Al principio (último en entrar, primero en salir)';

  @override
  String get pasteQueueEndBehaviorTitle => 'Tras el último elemento';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'Qué hace Pegar siguiente cuando ya se pegaron todos los elementos.';

  @override
  String get pasteQueueEndStop => 'Detener';

  @override
  String get pasteQueueEndLoop => 'Volver a empezar';

  @override
  String get pasteQueueEndKeepLast => 'Seguir pegando el último';

  @override
  String get pasteQueueSeparatorTitle => 'Separador de Pegar todo';

  @override
  String get pasteQueueSeparatorDescription =>
      'Se coloca entre los elementos al pegarlos todos a la vez.';

  @override
  String get pasteQueueSeparatorNewLine => 'Nueva línea';

  @override
  String get pasteQueueSeparatorBlankLine => 'Línea en blanco';

  @override
  String get pasteQueueSeparatorTab => 'Tabulación';

  @override
  String get pasteQueueSeparatorSpace => 'Espacio';

  @override
  String get pasteQueueSeparatorComma => 'Coma';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Coma y espacio';

  @override
  String get pasteQueueSeparatorNone => 'Ninguno';

  @override
  String get pasteQueueSeparatorCustom => 'Personalizado';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'Hasta 32 caracteres; se admiten \\n y \\t';

  @override
  String get pasteQueuePlainTextTitle => 'Pegar siempre como texto sin formato';

  @override
  String get pasteQueuePlainTextDescription =>
      'Quita el formato de los elementos en cola.';

  @override
  String get pasteQueueIdleStopTitle => 'Dejar de recopilar tras';

  @override
  String get pasteQueueIdleStopDescription =>
      'Deja de recopilar tras este tiempo sin copias.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes min';
  }

  @override
  String get pasteQueueIdleNever => 'Nunca';

  @override
  String get pasteQueueMaxEntriesTitle => 'Límite de tamaño de la cola';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Cuando está llena, no se añaden copias nuevas.';

  @override
  String get pasteQueueRestoreTitle =>
      'Restaurar el portapapeles anterior al terminar';

  @override
  String get pasteQueueRestoreDescription =>
      'Recupera lo que habías copiado antes de pegar.';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue cola recopilar orden atajo separador';

  @override
  String get settingsSectionAbout => 'Acerca de';

  @override
  String get settingsSectionAboutSummary =>
      'Versión y actualizaciones de la aplicación.';

  @override
  String get settingsGeneralDescription =>
      'Idioma, inicio automático y visibilidad de la aplicación.';

  @override
  String get settingsModulesDescription => 'Funciones opcionales de CopyPaste.';

  @override
  String get settingsClipboardDescription =>
      'Captura de los cambios del portapapeles y límites del historial.';

  @override
  String get settingsDataDescription =>
      'Exportación, copia de seguridad y restauración del historial del portapapeles.';

  @override
  String get settingsSyncDescription =>
      'Control de la sincronización con los dispositivos vinculados.';

  @override
  String get settingsPrivacyDescription =>
      'Control de las apps que se capturan y protección del contenido del portapapeles.';

  @override
  String get settingsNotificationsDescription =>
      'Notificaciones y sonidos para los recortes capturados.';

  @override
  String get settingsAboutDescription =>
      'Versión de CopyPaste y actualizaciones de la aplicación.';

  @override
  String get settingsQuickPasteDescription =>
      'Acceso al historial del portapapeles desde cualquier lugar sin cambiar de ventana.';

  @override
  String get settingsLoadingMessage => 'Cargando ajustes.';

  @override
  String get settingsUnavailableTitle => 'Los ajustes no están disponibles';

  @override
  String get settingsNoResultsTitle => 'No se encontraron ajustes';

  @override
  String get settingsNoResultsMessage => 'Prueba con otra búsqueda.';

  @override
  String get settingsAttentionTitle => 'Los ajustes requieren atención';

  @override
  String get settingsSearchPlaceholder => 'Buscar ajustes';

  @override
  String get settingsBackToSettings => 'Volver a Ajustes';

  @override
  String get settingsStartOnBootTitle => 'Iniciar al encender el dispositivo';

  @override
  String get settingsStartAtLoginTitle => 'Abrir al iniciar sesión';

  @override
  String get settingsStartupSearchDescription =>
      'Iniciar CopyPaste automáticamente.';

  @override
  String get settingsStartupKeywords =>
      'inicio sesión arranque abrir automáticamente autoarranque startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Desactivado en el Administrador de tareas. Activa CopyPaste en Configuración → Aplicaciones → Inicio.';

  @override
  String get settingsStartupMacApproval =>
      'Permite CopyPaste en Ajustes del Sistema → General → Ítems de inicio.';

  @override
  String get settingsStartupUnavailable =>
      'El inicio automático no está disponible en esta instalación.';

  @override
  String get settingsStartupAndroidDescription =>
      'Reanuda los servicios en segundo plano tras reiniciar.';

  @override
  String get settingsStartupDesktopDescription =>
      'Abre CopyPaste automáticamente al iniciar sesión.';

  @override
  String get settingsShowInDock => 'Mostrar en el Dock';

  @override
  String get settingsShowInTaskbar => 'Mostrar en la barra de tareas';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Muestra el icono de la aplicación para cambiar de ventana.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'dock barra tareas aplicación icono visibilidad dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Mostrar en la barra de menús';

  @override
  String get settingsShowInSystemTray => 'Mostrar en la bandeja del sistema';

  @override
  String get settingsTrayVisibilityDescription =>
      'Muestra un acceso rápido a CopyPaste y a los controles de captura.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'bandeja barra menús estado icono visibilidad tray menu bar menubar status icon visibility';

  @override
  String get settingsLightweightModeTitle => 'Modo ligero';

  @override
  String get settingsLightweightModeDescription =>
      'Al cerrar la ventana se sale de la app; la captura y la sincronización siguen activas. La bandeja, Quick Paste y los atajos vuelven al abrirla.';

  @override
  String get settingsLightweightModeKeywords =>
      'ligero cerrar salir segundo plano memoria lightweight close quit background memory';

  @override
  String get settingsVisibilityErrorTitle =>
      'No se pudo cambiar la visibilidad';

  @override
  String get settingsModulesSearchDescription =>
      'Instalar, actualizar y eliminar módulos opcionales.';

  @override
  String get settingsModulesKeywords =>
      'funciones comandos extensiones features commands extensions';

  @override
  String get settingsAndroidCaptureTitle =>
      'Captura en segundo plano de Android';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Modo Completo o Limitado con configuración mediante Shizuku o ADB.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'segundo plano permisos configuración background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Modo Completo o Limitado y configuración única con Shizuku o ADB.';

  @override
  String get settingsOpenSetup => 'Abrir configuración';

  @override
  String get settingsSaveScreenshotsTitle => 'Guardar capturas de pantalla';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Guardar automáticamente en el Historial las nuevas capturas de pantalla de Android.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'imágenes fotos captura permisos images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Añade automáticamente las nuevas capturas de pantalla al Historial.';

  @override
  String get settingsScreenshotAccessTitle => 'Acceso a capturas de pantalla';

  @override
  String get settingsScreenshotAccessDescription =>
      'Necesario para guardar capturas en segundo plano.';

  @override
  String get settingsAllowAccess => 'Permitir acceso';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'El guardado de capturas de pantalla está detenido';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'Las nuevas capturas de pantalla no se están guardando. Reanuda la captura para volver a iniciar el servicio.';

  @override
  String get settingsResumeCapture => 'Reanudar captura';

  @override
  String get settingsClipboardCaptureTitle => 'Captura del portapapeles';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Pausar o reanudar la captura del portapapeles.';

  @override
  String get settingsCaptureStateUnavailable => 'Estado no disponible';

  @override
  String get settingsCaptureStoppedTitle => 'La captura está detenida';

  @override
  String get settingsCaptureStoppedMessage =>
      'CopyPaste no está recibiendo los cambios del portapapeles en este dispositivo.';

  @override
  String get settingsRetentionTitle => 'Conservación';

  @override
  String get settingsRetentionDescription =>
      'Elimina automáticamente los elementos antiguos del portapapeles que no estén fijados.';

  @override
  String get settingsRetentionForever => 'Conservar siempre';

  @override
  String get settingsStorageQuotaTitle => 'Cuota de almacenamiento';

  @override
  String get settingsStorageQuotaDescription =>
      'Almacenamiento local máximo que puede ocupar el historial no fijado.';

  @override
  String get settingsStorageQuotaKeywords =>
      'disco espacio límite disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Tamaño máximo de archivo';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Límite para añadir y descargar archivos.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'importar descargar transferencia tamaño límite import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Archivo más grande que puedes añadir o descargar.';

  @override
  String get settingsFileStorageTitle => 'Almacenamiento de archivos';

  @override
  String get settingsFileStorageDescription =>
      'Un enlace solo funciona mientras exista el original; una copia conserva el archivo.';

  @override
  String get settingsFileStorageKeywords =>
      'archivos copia enlace original conservar guardar sin conexión files copy link original keep store offline';

  @override
  String get settingsFileStorageLink => 'Enlace al original';

  @override
  String get settingsFileStorageCopy => 'Guardar una copia';

  @override
  String get settingsSaveFolderTitle => 'Guardar archivos en';

  @override
  String get settingsSaveFolderAsk => 'Preguntar siempre';

  @override
  String get settingsSaveFolderAskAction => 'Preguntar siempre';

  @override
  String get settingsSaveFolderChooseAction => 'Elegir carpeta';

  @override
  String get settingsSaveFolderSearchDescription =>
      'Carpeta para los archivos descargados.';

  @override
  String get settingsSaveFolderKeywords =>
      'descarga carpeta destino directorio guardar ubicación download folder destination directory save location';

  @override
  String get settingsAutoDownloadTitle => 'Descargar archivos recibidos';

  @override
  String get settingsAutoDownloadDescription =>
      'Guarda en la carpeta los archivos de otros dispositivos mientras estén en línea.';

  @override
  String get settingsAutoDownloadNeedsFolder =>
      'Primero elige una carpeta para guardar archivos.';

  @override
  String get settingsAutoDownloadKeywords =>
      'automático descargar recibir sincronizar archivos automatic download receive sync files';

  @override
  String get settingsHistoryFilesTitle => 'Archivos del historial';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Exportar el historial, crear copias de seguridad o restaurar una copia.';

  @override
  String get settingsHistoryFilesKeywords =>
      'texto cifrado copia seguridad datos text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Exportar historial de texto';

  @override
  String get settingsExportTextDescription =>
      'Guarda los recortes de texto en un archivo portátil.';

  @override
  String get settingsExportAction => 'Exportar';

  @override
  String get settingsBackupTitle => 'Copia de seguridad cifrada';

  @override
  String get settingsBackupDescription =>
      'Guarda todo el historial local de este dispositivo.';

  @override
  String get settingsCreateBackupAction => 'Copia';

  @override
  String get settingsRestoreTitle => 'Restaurar copia de seguridad';

  @override
  String get settingsRestoreDescription =>
      'Reemplaza el historial local por una copia de seguridad cifrada.';

  @override
  String get settingsRestoreAction => 'Restaurar';

  @override
  String get settingsRestoreDialogTitle => '¿Reemplazar el historial local?';

  @override
  String get settingsRestoreDialogMessage =>
      'La copia de seguridad reemplaza el historial de este dispositivo. Los ajustes se conservan.';

  @override
  String get settingsRestoreFailed =>
      'No se pudo restaurar la copia de seguridad.';

  @override
  String get settingsSyncSwitchTitle => 'Sincronización';

  @override
  String get settingsSyncSwitchDescription =>
      'Sincroniza el historial con los dispositivos vinculados.';

  @override
  String get settingsSyncSearchDescription =>
      'Sincronización entre dispositivos vinculados.';

  @override
  String get settingsInstantClipboardTitle => 'Portapapeles instantáneo';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Copiar automáticamente los recortes nuevos de otros dispositivos.';

  @override
  String get settingsInstantClipboardKeywords =>
      'automático copiar pegar sincronización recibido automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Copia en este portapapeles los recortes nuevos de otros dispositivos.';

  @override
  String get settingsLanVisibilityTitle => 'Visibilidad en la red local';

  @override
  String get settingsLanVisibilityDescription =>
      'Permite que los dispositivos cercanos detecten este dispositivo.';

  @override
  String get settingsLanVisibilityKeywords =>
      'red local detección local network discovery';

  @override
  String get settingsRelayTitle => 'Servidor de retransmisión';

  @override
  String get settingsRelayDescription =>
      'Sincroniza dispositivos vinculados entre redes a través de tu propio servidor de retransmisión. Déjalo vacío para desactivarlo.';

  @override
  String get settingsRelayKeywords =>
      'retransmisión servidor remoto internet red relay server remote internet network websocket';

  @override
  String get settingsRelayPlaceholder => 'wss://relay.example.com';

  @override
  String get settingsRelayConnecting =>
      'Conectando con el servidor de retransmisión…';

  @override
  String get settingsRelayConnected =>
      'Conectado al servidor de retransmisión.';

  @override
  String get settingsRelayUnauthorized =>
      'El servidor de retransmisión rechazó el token de acceso.';

  @override
  String get settingsRelayUnreachable =>
      'No se puede contactar con el servidor de retransmisión.';

  @override
  String get settingsPortMappingTitle => 'Redirección de puertos del router';

  @override
  String get settingsPortMappingDescription =>
      'Permite que la sincronización directa pida al router (UPnP) abrir un puerto durante unos minutos. Ayuda cuando ambas redes bloquean las conexiones directas.';

  @override
  String get settingsPortMappingKeywords =>
      'router puerto redirección directa router port mapping upnp nat forwarding direct';

  @override
  String get settingsExcludedAppsTitle => 'Aplicaciones excluidas';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Omitir la captura automática de las aplicaciones excluidas.';

  @override
  String get settingsExcludedAppsKeywords =>
      'privacidad app identificadores privacy app identifiers';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'Omite la captura automática de estas apps.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'La captura automática se pausa mientras haya apps excluidas.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'Las exclusiones de aplicaciones son compatibles con macOS, Windows, Android y Linux.';

  @override
  String get settingsExcludedAppsAdd => 'Añadir';

  @override
  String get settingsExcludedAppsEmpty => 'No hay aplicaciones excluidas.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return 'Quitar $app';
  }

  @override
  String get applicationPickerSearch => 'Buscar aplicaciones';

  @override
  String get applicationPickerEmpty => 'No se encontraron aplicaciones';

  @override
  String get settingsSkipSecretTitle => 'Omitir portapapeles confidencial';

  @override
  String get settingsSkipSecretDescription =>
      'No guarda las copias que las apps marcan como secretas.';

  @override
  String get settingsSkipSecretKeywords =>
      'secreto contraseña sensible oculto spoiler secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Omitir portapapeles temporal';

  @override
  String get settingsSkipTransientDescription =>
      'No guarda las copias que las apps marcan como temporales.';

  @override
  String get settingsSkipTransientKeywords =>
      'transitorio temporal privacidad transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Bloquear capturas de pantalla';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Impide las capturas y grabaciones de pantalla de CopyPaste.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'El bloqueo de capturas de pantalla no está disponible en esta plataforma.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'privacidad pantalla captura protección vinculación qr seguridad código privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Notificación al copiar';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Muestra una notificación después de una captura en segundo plano.';

  @override
  String get settingsNotificationPreviewTitle =>
      'Mostrar contenido del portapapeles';

  @override
  String get settingsNotificationPreviewDescription =>
      'Incluye una vista previa del recorte en las notificaciones de copia.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'notificación vista previa texto imagen privacidad notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Sonido al copiar';

  @override
  String get settingsSoundOnCopyDescription =>
      'Reproduce la señal del sistema después de una captura correcta.';

  @override
  String get settingsVersionTitle => 'Versión';

  @override
  String get settingsVersionDescription => 'Versión instalada de CopyPaste.';

  @override
  String get settingsVersionUnavailable => 'No disponible';

  @override
  String get settingsUpdatesTitle => 'Actualizaciones de la aplicación';

  @override
  String get settingsUpdatesSearchDescription =>
      'Buscar e instalar actualizaciones de CopyPaste.';

  @override
  String get settingsUpdatesKeywords =>
      'versión github lanzamiento version github release';

  @override
  String get settingsUpdateIdle =>
      'Buscar una versión más reciente de CopyPaste.';

  @override
  String get settingsUpdateChecking => 'Buscando actualizaciones.';

  @override
  String get settingsUpdateUpToDate => 'CopyPaste está actualizado.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'La versión $version está actualizada.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'La versión $available está disponible. Este dispositivo tiene la versión $current.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Descargando la versión $version · $percent %.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Preparando las actualizaciones de los módulos instalados.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Instalando la versión $version.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'Las actualizaciones de la aplicación requieren atención.';

  @override
  String get settingsUpdateNowAction => 'Actualizar ahora';

  @override
  String get settingsUpdateOpenReleaseAction => 'Ver versión';

  @override
  String get settingsUpdateCheckingAction => 'Buscando';

  @override
  String get settingsUpdateDownloadingAction => 'Descargando';

  @override
  String get settingsUpdateInstallingAction => 'Instalando';

  @override
  String get settingsUpdateRestartAction => 'Reiniciar CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => 'Volver a buscar';

  @override
  String get settingsUpdateNoneTitle => 'No hay actualizaciones';

  @override
  String get settingsUpdateLatest => 'CopyPaste está en la última versión.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'CopyPaste $version es la última versión.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Actualización disponible';

  @override
  String get settingsUpdateReady =>
      'Hay una versión más reciente de CopyPaste lista para instalar.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'CopyPaste $version está listo para instalar.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Actualización no disponible';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Esta instalación no se puede actualizar automáticamente.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Error al buscar actualizaciones';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'CopyPaste no pudo buscar actualizaciones.';

  @override
  String get settingsQuickPasteSetupTitle => 'Configurar Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Activa el complemento firmado de GNOME o KDE y permite el control del teclado para volver a la app anterior.';

  @override
  String get settingsQuickPasteDesktopIntegration =>
      'Integración con el escritorio';

  @override
  String get settingsQuickPasteAllowKeyboard => 'Permitir control del teclado';

  @override
  String get settingsQuickPasteCheckAgain => 'Volver a comprobar';

  @override
  String get settingsQuickPasteLoading =>
      'Cargando los ajustes de Quick Paste.';

  @override
  String get settingsQuickPasteUnavailableTitle =>
      'Quick Paste no está disponible';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'Este dispositivo no ofrece la ventana de Quick Paste.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Abrir Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Configurar el atajo global de Quick Paste.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'teclado atajo keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'Funciona mientras CopyPaste se ejecuta en segundo plano.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Atajo registrado: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Pegar automáticamente';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Pegar el recorte seleccionado en la app anterior.';

  @override
  String get settingsAutoPasteKeywords =>
      'accesibilidad automático accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Al seleccionar un recorte, se vuelve a la app anterior y se pega.';

  @override
  String get settingsMaskPasswordFieldsTitle =>
      'Ocultar vistas previas en campos de contraseña';

  @override
  String get settingsMaskPasswordFieldsDescription =>
      'Oculta los recortes mientras se usa un campo de contraseña.';

  @override
  String get settingsAccessibilityRequiredTitle => 'Se requiere Accesibilidad';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Hasta que se conceda el permiso, al seleccionar un recorte se copia sin pegarlo.';

  @override
  String get settingsQuickPasteAttentionTitle =>
      'Quick Paste requiere atención';

  @override
  String get settingsShortcutRecording => 'Pulsa el atajo…';

  @override
  String get settingsShortcutNotSet => 'Sin asignar';

  @override
  String get settingsShortcutReset => 'Restablecer atajo';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count días',
      many: '$count días',
      one: '$count día',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Cargando dispositivos…';

  @override
  String get devicesUnavailableTitle => 'Los dispositivos no están disponibles';

  @override
  String get devicesNearbyTitle => 'Dispositivos cercanos';

  @override
  String get devicesScan => 'Buscar';

  @override
  String get devicesScanning => 'Buscando…';

  @override
  String get devicesNoneNearby => 'No hay dispositivos cercanos';

  @override
  String get devicesThisDevice => 'Este dispositivo';

  @override
  String get devicesTrusted => 'De confianza';

  @override
  String get devicesConnect => 'Conectar';

  @override
  String devicesCardPing(String ping) {
    return 'Ping: $ping';
  }

  @override
  String devicesCardPingStacked(String ping) {
    return 'Ping:\n$ping';
  }

  @override
  String get devicesRenameDialogTitle =>
      'Cambiar el nombre de este dispositivo';

  @override
  String get devicesNamePlaceholder => 'Nombre del dispositivo';

  @override
  String get devicesRenameFailed =>
      'No se pudo guardar el nombre del dispositivo.';

  @override
  String get devicesActionFailedMessage =>
      'No se pudo completar la acción en el dispositivo.';

  @override
  String get devicesCloseDetails => 'Cerrar detalles del dispositivo';

  @override
  String get devicesDetailType => 'Tipo de dispositivo';

  @override
  String get devicesDetailOperatingSystem => 'Sistema operativo';

  @override
  String get devicesDetailModel => 'Modelo';

  @override
  String get devicesDetailAppVersion => 'Versión de CopyPaste';

  @override
  String get devicesDetailProtocolVersion => 'Versión del protocolo';

  @override
  String get devicesDetailStatus => 'Estado';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => 'Medición';

  @override
  String get devicesDetailLastSeen => 'Visto por última vez';

  @override
  String get devicesDetailLanEndpoint => 'Dirección LAN';

  @override
  String get devicesDetailDeviceId => 'ID del dispositivo';

  @override
  String get devicesDetailPairingId => 'ID de vinculación';

  @override
  String get devicesDetailProfileProvenance => 'Origen del perfil';

  @override
  String get devicesDetailProfileTrust => 'Confianza del perfil';

  @override
  String get devicesDetailProfileUpdated => 'Perfil actualizado';

  @override
  String get devicesValueUnavailable => 'No disponible';

  @override
  String get devicesRenameDevice => 'Cambiar nombre';

  @override
  String get devicesSyncNow => 'Sincronizar ahora';

  @override
  String get devicesSyncTitle => 'Sincronización';

  @override
  String get devicesSyncDescription =>
      'Al desactivar algo se conservan los recortes guardados.';

  @override
  String get devicesSyncThisDevice => 'Sincronizar este dispositivo';

  @override
  String get devicesSyncPeer => 'Sincronizar con este dispositivo';

  @override
  String get devicesSyncPause => 'Pausar la sincronización automática';

  @override
  String get devicesSyncText => 'Texto';

  @override
  String get devicesSyncImages => 'Imágenes';

  @override
  String get devicesSyncFiles => 'Archivos';

  @override
  String get devicesSyncAfterFirstSync =>
      'Los ajustes aparecen tras la primera sincronización con este dispositivo.';

  @override
  String get devicesLocal => 'Local';

  @override
  String get devicesTrustAuthenticated => 'Autenticado';

  @override
  String get devicesTrustUnverified => 'Sin verificar';

  @override
  String get devicesProvenanceSelfReported => 'Autodeclarado';

  @override
  String get devicesProvenanceObserved => 'Observado';

  @override
  String get devicesProvenanceMeasured => 'Medido';

  @override
  String get devicesConnectionOnline => 'En línea';

  @override
  String get devicesConnectionOffline => 'Sin conexión';

  @override
  String get devicesConnectionVisible => 'Visible';

  @override
  String get devicesConnectionNotSeen => 'No detectado';

  @override
  String get devicesConnectionUnknown => 'Desconocido';

  @override
  String get devicesPeerAvailable => 'Disponible';

  @override
  String get devicesPeerVisibleNow => 'Visible ahora';

  @override
  String get devicesPeerStatusUnknown => 'Estado desconocido';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => 'Equipo de escritorio';

  @override
  String get devicesClassLaptop => 'Portátil';

  @override
  String get devicesClassPhone => 'Teléfono';

  @override
  String get devicesClassTablet => 'Tableta';

  @override
  String get devicesClassUnknown => 'Dispositivo';

  @override
  String get devicesUnknownOs => 'SO desconocido';

  @override
  String get devicesAddDevice => 'Añadir dispositivo';

  @override
  String get devicesScanQr => 'Escanear QR';

  @override
  String get devicesEnterCode => 'Introducir código';

  @override
  String get devicesScannerUnavailableTitle => 'Escáner no disponible';

  @override
  String get devicesActionFailedTitle => 'Error en la acción';

  @override
  String get devicesRefreshFailedTitle =>
      'No se pudieron actualizar los dispositivos';

  @override
  String get devicesNearbyPairingUnavailable =>
      'La vinculación con dispositivos cercanos no está disponible.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'La vinculación con dispositivos cercanos no está disponible. Usa Añadir dispositivo para mostrar un código de vinculación.';

  @override
  String get pairingTitle => 'Vinculación';

  @override
  String get pairingPairDevice => 'Vincular un dispositivo';

  @override
  String get pairingScanQrCode => 'Escanear código QR';

  @override
  String get pairingEnterPairingCode => 'Introducir código de vinculación';

  @override
  String get pairingConnectNearbyDevice => 'Conectar dispositivo cercano';

  @override
  String get pairingSendingRequest => 'Enviando solicitud de conexión…';

  @override
  String get pairingEmptyTitle => 'Elige un método de vinculación';

  @override
  String get pairingEmptyMessage =>
      'Aquí aparecen los controles de vinculación.';

  @override
  String get pairingCreatingInvitation =>
      'Creando una invitación de vinculación.';

  @override
  String get pairingCode => 'Código de vinculación';

  @override
  String get pairingCodeHint =>
      'Introduce el código de 8 caracteres que aparece en el otro dispositivo.';

  @override
  String get pairingDeviceAddress => 'Dirección del dispositivo';

  @override
  String get pairingDeviceAddressHint =>
      'Usa la dirección host:port que indica el otro dispositivo.';

  @override
  String get pairingCheckDetailsTitle => 'Revisa los datos de vinculación';

  @override
  String get pairingActionFailed => 'Error en la vinculación';

  @override
  String get pairingConnectingDevice => 'Conectando dispositivo…';

  @override
  String get pairingQrScannerTitle => 'Escáner QR';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'Aquí aparece la vista previa de la cámara.';

  @override
  String get pairingScannerNeedsAttention => 'El escáner requiere atención';

  @override
  String get pairingCodeIncomplete =>
      'Introduce el código de vinculación completo de 8 caracteres.';

  @override
  String get pairingAddressMissing =>
      'Introduce la dirección host:port del dispositivo o configura un servidor de retransmisión en Ajustes.';

  @override
  String get pairingWaitingForOtherDevice => 'Esperando al otro dispositivo';

  @override
  String get pairingInviteInstructions =>
      'En el otro dispositivo, elige Añadir dispositivo → Introducir código, o escanea el código QR.';

  @override
  String get pairingDetailsTitle => 'Datos de vinculación';

  @override
  String get pairingAddress => 'Dirección';

  @override
  String get pairingVerificationCode => 'Código de verificación';

  @override
  String get pairingVerificationCodeHint =>
      'Confirma solo si este código coincide en ambos dispositivos.';

  @override
  String get pairingReject => 'Rechazar';

  @override
  String get pairingConfirm => 'Confirmar';

  @override
  String get pairingAccept => 'Aceptar';

  @override
  String get pairingJoinDevice => 'Vincular dispositivo';

  @override
  String get pairingStartScanner => 'Iniciar escáner';

  @override
  String get pairingActionNotStarted => 'No se pudo iniciar la vinculación.';

  @override
  String get pairingStateHandshaking => 'Verificando dispositivo';

  @override
  String get pairingStateAwaitingConfirmation =>
      'Confirma en ambos dispositivos';

  @override
  String get pairingStateConfirmed => 'Dispositivo vinculado';

  @override
  String get pairingStateRejected => 'Vinculación rechazada';

  @override
  String get pairingStateCancelled => 'Vinculación cancelada';

  @override
  String get pairingStateTimedOut => 'La vinculación caducó';

  @override
  String get pairingStateFailed => 'Error en la vinculación';

  @override
  String get pairingHandshakingDescription =>
      'Los dispositivos están estableciendo un canal seguro.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Compara el código de verificación protegido y luego confirma en ambos dispositivos.';

  @override
  String get pairingConfirmedDescription =>
      'El dispositivo está conectado. La pertenencia al grupo se sincronizará automáticamente.';

  @override
  String get pairingNoDeviceAdded => 'No se añadió ningún dispositivo.';

  @override
  String get pairingTimedOutDescription =>
      'Inicia un nuevo intento de vinculación para continuar.';

  @override
  String get pairingIdleDescription =>
      'Inicia un intento de vinculación para conectar un dispositivo.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'La vinculación protegida no está disponible';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Esta sesión de vinculación debe abrirse en una ventana protegida exclusiva.';

  @override
  String get pairingCancel => 'Cancelar vinculación';

  @override
  String get pairingOrEnterCode => 'O introduce un código de vinculación';

  @override
  String get pairingJoinSecurely => 'Vincular de forma segura';

  @override
  String get pairingProtectedWaitingDescription =>
      'Muestra un código QR, escanea uno o introduce un código de vinculación en esta ventana protegida.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Caduca en $seconds s';
  }

  @override
  String get pairingInvalidQr =>
      'Escanea un código QR de vinculación de CopyPaste.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'El escáner de Google no está disponible. Introduce el código de vinculación.';

  @override
  String get pairingScannerCouldNotOpen =>
      'No se pudo abrir el escáner. Introduce el código de vinculación.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'No se puede mostrar la vinculación de forma segura.';

  @override
  String get pairingVerificationCodeRequired =>
      'Primero muestra y compara el código de verificación.';

  @override
  String get syncStatusUnavailable => 'Estado de sincronización no disponible';

  @override
  String get syncStatusOff => 'Sincronización desactivada';

  @override
  String get syncStatusWaitingForSync => 'Esperando la sincronización';

  @override
  String get syncStatusSyncing => 'Sincronizando';

  @override
  String get syncStatusUpToDate => 'Actualizado';

  @override
  String get syncStatusInterrupted => 'Sincronización interrumpida';

  @override
  String get syncStatusPartiallySynchronized => 'Sincronizado parcialmente';

  @override
  String get syncStatusRetrying => 'Reintentando';

  @override
  String get syncStatusWaitingToSync => 'Pendiente de sincronizar';

  @override
  String get syncStatusWaitingForDevice => 'Esperando al dispositivo';

  @override
  String get syncStatusNoPairedDevices => 'No hay dispositivos vinculados';

  @override
  String get syncStatusDevicesOffline => 'Dispositivos sin conexión';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · registro de actividad no disponible';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Sincronización desactivada · $count problemas',
      many: 'Sincronización desactivada · $count problemas',
      one: 'Sincronización desactivada · $count problema',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count sin conexión',
      many: '$status · $count sin conexión',
      one: '$status · $count sin conexión',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dispositivos requieren atención',
      many: '$count dispositivos requieren atención',
      one: '$count dispositivo requiere atención',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Dispositivos sin conexión · $count problemas',
      many: 'Dispositivos sin conexión · $count problemas',
      one: 'Dispositivos sin conexión · $count problema',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Comprobando conexión';

  @override
  String get syncConnectionUnknown => 'Conexión desconocida';

  @override
  String get syncLogDeviceOffline => 'El dispositivo se desconectó.';

  @override
  String get syncLogConnectionRestored => 'Conexión restablecida.';

  @override
  String get syncLogProblemDetected =>
      'Se detectó un problema de sincronización.';

  @override
  String get syncLogProblemResolved =>
      'Se resolvió el problema de sincronización.';

  @override
  String get syncLogFailed => 'Error de sincronización.';

  @override
  String get syncLogRecovered => 'Sincronización restablecida.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent recortes',
      many: '$sent recortes',
      one: '$sent recorte',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received recortes',
      many: '$received recortes',
      one: '$received recorte',
    );
    return 'Enviados: $_temp0 · Recibidos: $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Sincronización: $status';
  }

  @override
  String get syncDetailsTitle => 'Sincronización';

  @override
  String get syncDetailsClose => 'Cerrar detalles de sincronización';

  @override
  String get syncNoPairedDevicesMessage =>
      'Vincula un dispositivo para sincronizar el historial del portapapeles.';

  @override
  String get syncActivityLogUnavailableTitle =>
      'Registro de actividad no disponible';

  @override
  String get syncActivityLogUnavailableMessage =>
      'No se pudo leer ni guardar la actividad de sincronización.';

  @override
  String get syncDetailsDevicesHeading => 'Dispositivos';

  @override
  String get syncDetailsActivityHeading => 'Actividad';

  @override
  String get syncDetailsNoRecentProblems =>
      'No hay problemas de sincronización recientes';

  @override
  String get syncDetailsUnknownPeer => 'Dispositivo vinculado';

  @override
  String get syncPeerConnection => 'Conexión';

  @override
  String get syncPeerLastCheckStarted => 'Inicio de la última comprobación';

  @override
  String get syncPeerItemsOverSizeLimit =>
      'Elementos que superan el límite de tamaño';

  @override
  String syncPeerLastSeen(String time) {
    return 'Visto por última vez: $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Última sincronización: $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Última sincronización correcta';

  @override
  String get syncPeerConnectionDetails => 'Detalles de la conexión';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Detalles de la conexión de $device';
  }

  @override
  String get historyKindText => 'Texto';

  @override
  String get historyKindLink => 'Enlace';

  @override
  String get historyKindEmail => 'Correo';

  @override
  String get historyKindColor => 'Color';

  @override
  String get historyKindPhone => 'Teléfono';

  @override
  String get historyKindCode => 'Código';

  @override
  String get historyKindPath => 'Ruta';

  @override
  String get historyKindCredentials => 'Credenciales';

  @override
  String get historyKindImage => 'Imagen';

  @override
  String get historyKindFile => 'Archivo';

  @override
  String get historyKindOther => 'Otro';

  @override
  String get historySortNewest => 'Más recientes primero';

  @override
  String get historySortOldest => 'Más antiguos primero';

  @override
  String get historySortRelevance => 'Más relevantes';

  @override
  String get historyErrorUpdatesUnavailable =>
      'Las actualizaciones del historial no están disponibles temporalmente.';

  @override
  String get historyErrorBulkDelete =>
      'No se pudieron eliminar algunos recortes seleccionados. Vuelve a intentarlo.';

  @override
  String get historyErrorBulkPin =>
      'No se pudo cambiar la fijación de algunos recortes seleccionados. Vuelve a intentarlo.';

  @override
  String get historyErrorRefresh =>
      'No se pudo actualizar el historial. Vuelve a intentarlo.';

  @override
  String get historyErrorLoad =>
      'No se pudo cargar el historial. Vuelve a intentarlo.';

  @override
  String get historyErrorLoadMore => 'No se pudo cargar más historial.';

  @override
  String get historyErrorLoadEarlier =>
      'No se pudo cargar el historial anterior.';

  @override
  String get historyErrorOpenClip => 'No se pudo abrir este recorte.';

  @override
  String get historyErrorCopyPlainText =>
      'No se pudo copiar el texto sin formato.';

  @override
  String get historyErrorCopyClip => 'No se pudo copiar este recorte.';

  @override
  String get historyErrorCopyText => 'No se pudo copiar este texto.';

  @override
  String get historyErrorPrepareFile =>
      'No se pudo descargar este archivo. Comprueba el dispositivo de origen y vuelve a intentarlo.';

  @override
  String get historyErrorDownloadFile => 'No se pudo descargar este archivo.';

  @override
  String get historyErrorSelectFiles =>
      'No se pudieron seleccionar los archivos. Vuelve a intentarlo.';

  @override
  String get historyErrorReadSelectedFile =>
      'No se pudo leer el archivo seleccionado. Comprueba el acceso y el almacenamiento disponible.';

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
          'Archivos no importados: $failed de $total. Primer error: $name. $reason',
      many:
          'Archivos no importados: $failed de $total. Primer error: $name. $reason',
      one:
          'Archivos no importados: $failed de $total. Primer error: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'No se pudo actualizar la confidencialidad del recorte.';

  @override
  String get historyErrorPinUpdate =>
      'No se pudo cambiar la fijación del recorte.';

  @override
  String get historyErrorPinnedOrderSave =>
      'No se pudo guardar el orden de los recortes fijados. Vuelve a intentarlo.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'Se guardó el orden, pero no se pudo actualizar el historial.';

  @override
  String get historyErrorDeleteClip => 'No se pudo eliminar este recorte.';

  @override
  String get historyErrorDeleteAll => 'No se pudo eliminar el historial.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'No se pudieron eliminar los recortes seleccionados.';

  @override
  String get historyOcrErrorUnavailable =>
      'El OCR no está disponible. Comprueba el módulo en Ajustes y vuelve a intentarlo.';

  @override
  String get historyOcrErrorStart =>
      'No se pudo iniciar el OCR. Vuelve a intentarlo.';

  @override
  String get historyOcrErrorRecognize =>
      'No se pudo reconocer el texto de la imagen. Vuelve a intentarlo.';

  @override
  String get historyOcrRecognizing => 'Reconociendo el texto de la imagen.';

  @override
  String get historyOcrFailedTitle => 'Error de OCR';

  @override
  String get historyOcrNoText => 'No se encontró texto';

  @override
  String get historyOcrTitle => 'Texto de la imagen';

  @override
  String get historyOcrTextCopied => 'Texto reconocido copiado.';

  @override
  String get historyOcrCopyError =>
      'No se pudo copiar el texto. Vuelve a intentarlo.';

  @override
  String get historyTagsErrorLoad =>
      'No se pudieron cargar las etiquetas. Vuelve a intentarlo.';

  @override
  String get historyTagsErrorSave =>
      'No se pudo guardar el cambio de etiqueta. Usa un nombre único de hasta 64 caracteres y selecciona recortes fijados.';

  @override
  String get historyTags => 'Etiquetas';

  @override
  String get historyTagsUpdateErrorTitle =>
      'No se pudieron actualizar las etiquetas';

  @override
  String get historyTagsEmpty => 'Aún no hay etiquetas';

  @override
  String get historyTagRename => 'Cambiar nombre';

  @override
  String get historyTagActions => 'Acciones de la etiqueta';

  @override
  String get historyTagNamePlaceholder => 'Nombre de la etiqueta';

  @override
  String get historyCreateTag => 'Crear etiqueta';

  @override
  String get historySaveTag => 'Guardar etiqueta';

  @override
  String get historyTagsFilterAll => 'Todas las etiquetas';

  @override
  String historyTagsFilterCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count etiquetas',
      many: '$count etiquetas',
      one: '$count etiqueta',
    );
    return '$_temp0';
  }

  @override
  String historyTagsFilterTooltip(String selection) {
    return 'Filtrar por etiquetas: $selection';
  }

  @override
  String get historyManageTags => 'Gestionar etiquetas';

  @override
  String get historyTagsFilterSemantics =>
      'Filtrar recortes fijados por etiquetas';

  @override
  String get historyDownloadErrorCancelled =>
      'Se canceló la descarga del archivo.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'El servicio de descargas no está disponible. Vuelve a abrir CopyPaste e inténtalo de nuevo.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'El servicio de descargas no se conectó. Vuelve a intentarlo.';

  @override
  String get historyFileErrorAccess =>
      'No se pudo acceder al archivo seleccionado.';

  @override
  String get historySpoilerRevealSemantics =>
      'Contenido confidencial. Mostrar spoiler';

  @override
  String get historyActionFailedTitle => 'Error en la acción';

  @override
  String get historyLoadingMessage => 'Cargando el historial del portapapeles.';

  @override
  String get historyUnavailableTitle => 'El historial no está disponible';

  @override
  String get historyEmptyTitle => 'No se encontraron recortes';

  @override
  String get historyEmptyMessage =>
      'Aquí aparecen los recortes capturados y el historial que coincide.';

  @override
  String get historyAttentionTitle => 'El historial requiere atención';

  @override
  String get historyLoadMore => 'Cargar más';

  @override
  String get historySectionPinned => 'Fijados';

  @override
  String get historySectionToday => 'Hoy';

  @override
  String get historySectionYesterday => 'Ayer';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se omitieron $count elementos del portapapeles',
      many: 'Se omitieron $count elementos del portapapeles',
      one: 'Se omitió $count elemento del portapapeles',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Algunos recortes guardados no se pudieron leer y no aparecen en esta lista.';

  @override
  String get historySearch => 'Buscar en el historial';

  @override
  String get historySearchSuggestions => 'Sugerencias de búsqueda';

  @override
  String get historySearchSuggestionRecent => 'Reciente';

  @override
  String get historySearchSuggestionTag => 'Etiqueta';

  @override
  String get historySearchSuggestionApp => 'App';

  @override
  String get historySearchSuggestionKind => 'Tipo';

  @override
  String get historySearchSuggestionDevice => 'Dispositivo';

  @override
  String get historySearchClearRecent => 'Borrar búsquedas recientes';

  @override
  String get historyResetFilters => 'Restablecer filtros';

  @override
  String get historyImportFiles => 'Importar archivos';

  @override
  String get historySelectClips => 'Seleccionar recortes';

  @override
  String get historyCloseSearch => 'Cerrar búsqueda';

  @override
  String get historyFilterAllClips => 'Todos los recortes';

  @override
  String get historyFilterAllDevices => 'Todos los dispositivos';

  @override
  String get historyFilterAllApps => 'Todas las apps';

  @override
  String get historyPinClip => 'Fijar recorte';

  @override
  String get historyUnpinClip => 'Dejar de fijar recorte';

  @override
  String get historyReorderPinnedClip => 'Reordenar recorte fijado';

  @override
  String get historyReorderPinnedHint =>
      'Arrastra para mover. Alt + flecha arriba o abajo mueve una posición.';

  @override
  String get historyClipUnavailable => 'Recorte no disponible';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Tipo de recorte: $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Selecciona un recorte';

  @override
  String get historyInspectorEmptyMessage =>
      'Aquí aparecen su contenido completo y sus acciones.';

  @override
  String get historyNoPreview => 'No hay vista previa disponible.';

  @override
  String get historyCloseClipDetails => 'Cerrar detalles del recorte';

  @override
  String get historyCopyOptions => 'Opciones de copia';

  @override
  String get historyCopyPlainText => 'Copiar texto sin formato';

  @override
  String get historyManageClipTags => 'Gestionar etiquetas del recorte';

  @override
  String get historyPinnedButton => 'Fijado';

  @override
  String get historyAddSpoiler => 'Añadir spoiler';

  @override
  String get historyRemoveSpoiler => 'Quitar spoiler';

  @override
  String get historyUpdateFailedTitle => 'Error al actualizar';

  @override
  String get historyRecognizeImageText => 'Reconocer texto de la imagen';

  @override
  String get historyCopiedTitle => 'Copiado';

  @override
  String get historyCopyFailedTitle => 'Error al copiar';

  @override
  String get historyPlainTextCopied => 'Texto sin formato copiado.';

  @override
  String get historyClipCopied => 'Recorte copiado.';

  @override
  String get historyDownloadedTitle => 'Descargado';

  @override
  String get historyFileSaved => 'Archivo guardado.';

  @override
  String get historyShare => 'Compartir';

  @override
  String get historyErrorShareClip => 'No se pudo compartir este recorte.';

  @override
  String get historyErrorShareClips =>
      'No se pudieron compartir los recortes seleccionados.';

  @override
  String get historyBulkShare => 'Compartir seleccionados';

  @override
  String get historyBulkShareFilesOnlyTitle => 'Solo se compartieron archivos';

  @override
  String get historyBulkShareFilesOnly =>
      'Se omitieron los recortes de texto porque este sistema no puede compartir texto junto con archivos. Compártelos por separado.';

  @override
  String get historyDownload => 'Descargar';

  @override
  String get historyDownloading => 'Descargando…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Descargando $percent %';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent %';
  }

  @override
  String get historyDownloadCancelling => 'Cancelando…';

  @override
  String get historyDownloadCancellingTooltip => 'Cancelando descarga';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Descargando $percent % · Haz clic para cancelar';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Error en la descarga · Haz clic para reintentar';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Cancelar descarga, $percent % completado';
  }

  @override
  String get historyRetryDownload => 'Reintentar descarga';

  @override
  String get historyDownloadFailedTitle => 'Error en la descarga';

  @override
  String get historyShowFile => 'Mostrar archivo';

  @override
  String get historyMetadataPrivacy => 'Privacidad';

  @override
  String get historyMetadataConfidential => 'Confidencial';

  @override
  String get historyMetadataCaptured => 'Capturado';

  @override
  String get historyMetadataObservedApp => 'App detectada';

  @override
  String get historyMetadataDevice => 'Dispositivo';

  @override
  String get historyMetadataClipType => 'Tipo de recorte';

  @override
  String get historyMetadataType => 'Tipo';

  @override
  String get historyMetadataPath => 'Ruta';

  @override
  String get historyMetadataFile => 'Archivo';

  @override
  String get historyMetadataResolution => 'Resolución';

  @override
  String get historyMetadataSize => 'Tamaño';

  @override
  String get historyMetadataFiles => 'Archivos';

  @override
  String get historyMetadataSync => 'Sincronización';

  @override
  String get historyMetadataTooLargeToSync =>
      'Demasiado grande para sincronizar';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '¿Eliminar $count recortes seleccionados?',
      many: '¿Eliminar $count recortes seleccionados?',
      one: '¿Eliminar $count recorte seleccionado?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => '¿Eliminar este recorte?';

  @override
  String get historyBulkTagTooltip =>
      'Etiquetar los recortes fijados seleccionados';

  @override
  String get historyBulkExit => 'Salir de la selección';

  @override
  String get historyBulkPin => 'Fijar seleccionados';

  @override
  String get historyBulkUnpin => 'Dejar de fijar seleccionados';

  @override
  String get historyBulkDelete => 'Eliminar seleccionados';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count seleccionados',
      many: '$count seleccionados',
      one: '$count seleccionado',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Filtrar por fecha';

  @override
  String get historyDatePresetToday => 'Hoy';

  @override
  String get historyDatePresetYesterday => 'Ayer';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Últimos $days días',
      many: 'Últimos $days días',
      one: 'Último $days día',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'Todas las fechas';

  @override
  String get appRuntimeStartFailed =>
      'CopyPaste no pudo iniciar su entorno de ejecución.';

  @override
  String get appPreparing => 'Preparando CopyPaste.';

  @override
  String get appRuntimeWaiting =>
      'CopyPaste está esperando el acceso al historial protegido. Las partes disponibles de la app se pueden seguir usando.';

  @override
  String get appHistoryUnavailableTitle =>
      'El entorno de ejecución del historial no está disponible';

  @override
  String get appHistoryUnavailableMessage =>
      'Inicia el entorno de ejecución de la aplicación antes de abrir el historial.';

  @override
  String get appDevicesUnavailableTitle =>
      'El entorno de ejecución de dispositivos no está disponible';

  @override
  String get appDevicesUnavailableMessage =>
      'Inicia el entorno de ejecución de la aplicación antes de abrir los dispositivos.';

  @override
  String get appSettingsUnavailableTitle =>
      'El entorno de ejecución de ajustes no está disponible';

  @override
  String get appSettingsUnavailableMessage =>
      'Inicia el entorno de ejecución de la aplicación antes de abrir los ajustes.';

  @override
  String get appQuit => 'Salir de CopyPaste';

  @override
  String get appWindowSetupTitle =>
      'La configuración de la ventana requiere atención';

  @override
  String get appOpenHistory => 'Abrir Historial';

  @override
  String get appDestinationHistory => 'Historial';

  @override
  String get appDestinationDevices => 'Dispositivos';

  @override
  String get appDestinationSettings => 'Ajustes';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, seleccionado';
  }

  @override
  String get appActiveFilter => 'Filtro activo';

  @override
  String get appActionFailed => 'Error en la acción';

  @override
  String get sharedAndroidVersionRequired =>
      'Se requiere Android 11 o posterior';

  @override
  String get sharedUseAdbTab => 'Usa la pestaña ADB en este dispositivo.';

  @override
  String get sharedOneTimeAccessApplied => 'Acceso único aplicado';

  @override
  String get sharedInstallShizuku => 'Instalar Shizuku';

  @override
  String get sharedPairAndStartShizuku => 'Vincular e iniciar Shizuku';

  @override
  String get sharedAllowCopyPaste => 'Permitir CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded => 'Shizuku ya no es necesario.';

  @override
  String get sharedGetShizukuDescription =>
      'Obtén Shizuku para aplicar el acceso único.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Usa Depuración inalámbrica en Shizuku y luego vuelve.';

  @override
  String get sharedApproveInShizuku => 'Aprueba CopyPaste una vez en Shizuku.';

  @override
  String get sharedGetShizuku => 'Obtener Shizuku';

  @override
  String get sharedOpenShizuku => 'Abrir Shizuku';

  @override
  String get sharedRunOnComputer => 'Ejecutar en el equipo';

  @override
  String get sharedCopyAllCommands => 'Copiar todos los comandos';

  @override
  String get sharedEnableUsbDebugging =>
      'Activa Depuración por USB y conecta este teléfono.';

  @override
  String get sharedAllow => 'Permitir';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'No se pudieron copiar los comandos de configuración.';

  @override
  String get sharedShizukuOpenFailed => 'No se pudo abrir Shizuku.';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => 'Configurar CopyPaste';

  @override
  String get onboardingWelcomeTitle => 'Te damos la bienvenida a CopyPaste';

  @override
  String get onboardingWelcomeSubtitle =>
      'Tu portapapeles, listo cuando lo necesites.';

  @override
  String get onboardingReadyTitle => 'CopyPaste está listo';

  @override
  String get onboardingGetStarted => 'Empezar';

  @override
  String get onboardingOptionalSetupTitle =>
      'La configuración opcional requiere atención';

  @override
  String get onboardingOpenSystemSettings => 'Abrir Ajustes';

  @override
  String get onboardingSetupErrorTitle => 'La configuración requiere atención';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'CopyPaste no pudo leer el estado de configuración de $platform.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'CopyPaste no pudo guardar el progreso de la configuración inicial.';

  @override
  String get onboardingStartAtLogin => 'Abrir al iniciar sesión';

  @override
  String get onboardingStartAtLoginDescription =>
      'Abre CopyPaste al iniciar sesión';

  @override
  String get onboardingStartAtLoginSemantics =>
      'Abrir CopyPaste al iniciar sesión';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'No se pudo activar la opción Abrir al iniciar sesión. Puedes continuar sin ella.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'No se pudo actualizar la opción Abrir al iniciar sesión. Puedes continuar sin ella.';

  @override
  String get onboardingMacosTitle => 'Configura tu Mac';

  @override
  String get onboardingAccessibility => 'Accesibilidad';

  @override
  String get onboardingAutoPasteEnabled => 'Pegado automático activado';

  @override
  String get onboardingAutoPasteOptional =>
      'Opcional para el pegado automático';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Disponible en la app CopyPaste instalada.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'Se requiere aprobar el ítem de inicio';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Permite CopyPaste en Ajustes del Sistema y luego vuelve.';

  @override
  String get onboardingMacosStateReadFailed =>
      'CopyPaste no pudo leer el estado de configuración de macOS.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'No se pudo solicitar el permiso de Accesibilidad.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'CopyPaste no pudo actualizar el estado de configuración de macOS.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'No se pudo actualizar el estado de Accesibilidad.';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'No se pudieron abrir los ajustes de Ítems de inicio.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'Abrir al iniciar sesión requiere aprobación en Ajustes del Sistema.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'No se pudo desactivar la opción Abrir al iniciar sesión.';

  @override
  String get onboardingWindowsTitle => 'Configura tu PC';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'No disponible en esta instalación. Puedes continuar sin esta opción.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Desactivado en el Administrador de tareas';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Activa CopyPaste en las aplicaciones de inicio de Windows y luego vuelve.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'Abrir al iniciar sesión está desactivado en el Administrador de tareas. Puedes activarlo más tarde en la Configuración de Windows.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'Abrir al iniciar sesión no está disponible. Puedes continuar sin esta opción.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'CopyPaste no pudo leer el estado de configuración de Windows.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'No se pudo abrir la configuración de inicio de Windows.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'No se pudo actualizar la opción Abrir al iniciar sesión. Puedes cambiarla más tarde en Ajustes.';

  @override
  String get onboardingLinuxChecking =>
      'Comprobando la sesión de escritorio de Linux.';

  @override
  String get onboardingLinuxTitle => 'Configura tu escritorio Linux';

  @override
  String get onboardingLinuxSubtitle =>
      'CopyPaste necesita una integración verificada con el escritorio para identificar la app de origen de cada copia y para Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Sesión de escritorio';

  @override
  String get onboardingLinuxX11Detected => 'Se detectó una sesión X11.';

  @override
  String get onboardingLinuxWaylandDetected => 'Se detectó una sesión Wayland.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'No se detectó ninguna sesión de escritorio X11 o Wayland compatible.';

  @override
  String get onboardingLinuxCompanion => 'Complemento de GNOME o KDE';

  @override
  String get onboardingLinuxCompanionReady =>
      'El puente autenticado Clipboard v2 está listo.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'La captura del portapapeles está en pausa por las exclusiones de apps de origen. Activa el complemento firmado para reanudarla.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Instala y activa el paquete de sesión del complemento firmado y luego vuelve aquí.';

  @override
  String get onboardingLinuxOpenSetup => 'Abrir configuración';

  @override
  String get onboardingLinuxKeyboardControl => 'Control del teclado';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'El control del teclado está permitido para Quick Paste.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'El control del teclado no está disponible en esta sesión de escritorio.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Permitir control del teclado';

  @override
  String get onboardingLinuxQuickPasteReady => 'Quick Paste está listo.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'Quick Paste estará listo cuando la integración necesaria con el escritorio esté activa.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'Abre CopyPaste al iniciar sesión.';

  @override
  String get onboardingLinuxLinks => 'Enlaces de CopyPaste';

  @override
  String get onboardingLinuxLinksRegistered =>
      'Los enlaces de CopyPaste se abren en esta app.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Registra los enlaces de CopyPaste para las invitaciones de vinculación.';

  @override
  String get onboardingLinuxRegisterLinks => 'Registrar enlaces';

  @override
  String get onboardingLinuxScreenshotBlocking =>
      'Bloqueo de capturas de pantalla';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'El bloqueo de capturas de pantalla no está disponible en Linux. Los datos guardados protegidos siguen cifrados.';

  @override
  String get onboardingLinuxCheckAgain => 'Volver a comprobar';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'CopyPaste no pudo leer el estado de la integración con Linux.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'No se pudo abrir la configuración de la integración con el escritorio.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'No se concedió el permiso de control del teclado.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'No se pudo actualizar la opción Abrir al iniciar sesión.';

  @override
  String get onboardingLinuxLinksFailed =>
      'No se pudieron registrar los enlaces de CopyPaste.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'No se pudo actualizar el estado de la integración con Linux.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'El acceso al portapapeles no está disponible en esta sesión X11.';

  @override
  String get onboardingLinuxSessionRequired =>
      'CopyPaste requiere una sesión de escritorio X11 o Wayland.';

  @override
  String get onboardingAndroidFullCapture => 'Captura completa';

  @override
  String get onboardingAndroidBackgroundCapture => 'Captura en segundo plano';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Se configura una vez y se verifica con una copia real.';

  @override
  String get onboardingAndroidLimitedCapture => 'Captura limitada';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'Comparte con CopyPaste o vuelve a la app después de copiar.';

  @override
  String get onboardingAndroidNotification => 'Notificación de captura';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Necesaria mientras la captura está activa';

  @override
  String get onboardingAndroidBackgroundActivity =>
      'Actividad en segundo plano';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Recomendado para evitar capturas perdidas';

  @override
  String get onboardingAndroidOpen => 'Abrir';

  @override
  String get onboardingAndroidApplyAccess => 'Aplicar acceso de captura';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Aplica el acceso de captura único.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Puedes configurar la Captura completa más tarde.';

  @override
  String get onboardingAndroidCaptureWorking =>
      'La captura en segundo plano funciona';

  @override
  String get onboardingAndroidVerifyCopy => 'Verificar con una copia real';

  @override
  String get onboardingAndroidCopyReached =>
      'Una nueva copia llegó a tu Historial local.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Copia algo en otra app y luego vuelve aquí.';

  @override
  String get onboardingAndroidStartCapture => 'Iniciar captura';

  @override
  String get onboardingAndroidStateReadFailed =>
      'CopyPaste no pudo leer el estado de configuración de Android.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'No se pudo actualizar el estado de captura de Android.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'La supervisión de la configuración de Android no está disponible.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'No se pudo activar la recepción del portapapeles.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'No se pudo detener la captura en segundo plano.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'No se pudieron abrir los ajustes de batería.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'CopyPaste tiene permiso en Shizuku, pero no se pudieron aplicar los permisos de captura de Android.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Permite CopyPaste en Shizuku para aplicar los permisos de captura.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Primero concede el acceso al portapapeles en segundo plano.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Permite las notificaciones antes de iniciar la captura en segundo plano.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'No se pudo iniciar la captura en segundo plano.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'No se pudo abrir la configuración de captura de Android.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'No se pudo guardar la opción de configuración de Android.';

  @override
  String get modulesMarketplaceTab => 'Catálogo';

  @override
  String get modulesInstalledTab => 'Instalados';

  @override
  String get modulesRefresh => 'Actualizar';

  @override
  String get modulesInstallFromFile => 'Instalar desde archivo';

  @override
  String get modulesPackageFileType => 'Módulo de CopyPaste';

  @override
  String get modulesSearchPlaceholder => 'Buscar módulos';

  @override
  String get modulesOperationFailed => 'Error en la operación del módulo';

  @override
  String get modulesLoading => 'Cargando módulos.';

  @override
  String get modulesUnavailableTitle => 'Los módulos no están disponibles';

  @override
  String get modulesUnavailableMessage =>
      'Vuelve a intentar cargar los módulos.';

  @override
  String get modulesNoneInstalled => 'No hay módulos instalados';

  @override
  String get modulesNoMatches => 'No hay módulos que coincidan';

  @override
  String get modulesChooseFromMarketplace => 'Elige un módulo del catálogo.';

  @override
  String get modulesTryDifferentSearch => 'Prueba con otra búsqueda.';

  @override
  String get modulesMarketplaceLoading => 'Cargando el catálogo.';

  @override
  String get modulesMarketplaceUnavailableTitle =>
      'El catálogo no está disponible';

  @override
  String get modulesMarketplaceUnavailableMessage =>
      'Vuelve a intentar cargar el catálogo.';

  @override
  String get modulesNoneAvailable => 'No hay módulos disponibles';

  @override
  String get modulesNoneAvailableMessage =>
      'Los módulos aparecerán aquí cuando se publiquen.';

  @override
  String modulesEnableModule(String module) {
    return 'Activar $module';
  }

  @override
  String get modulesInstall => 'Instalar';

  @override
  String modulesUpdateTo(String version) {
    return 'Actualizar a $version';
  }

  @override
  String get modulesRemoveModule => 'Eliminar módulo';

  @override
  String get modulesModuleSettings => 'Ajustes del módulo';

  @override
  String modulesSettingsTitle(String module) {
    return 'Ajustes de $module';
  }

  @override
  String get modulesDownloadingModel => 'Descargando modelo de idioma…';

  @override
  String get modulesRun => 'Ejecutar';

  @override
  String modulesRemoveTitle(String module) {
    return '¿Eliminar $module?';
  }

  @override
  String get modulesRemoveMessage =>
      'Se eliminarán el módulo y sus ajustes locales.';

  @override
  String get modulesRemove => 'Eliminar';

  @override
  String get modulesRemoveFailed => 'No se pudo eliminar el módulo.';

  @override
  String get modulesCloseSettings => 'Cerrar ajustes del módulo';

  @override
  String get modulesRemoved => 'Módulo eliminado';

  @override
  String get modulesNoSettingsTitle => 'No hay ajustes adicionales';

  @override
  String get modulesNoSettingsMessage =>
      'Este módulo no necesita configuración adicional.';

  @override
  String get modulesSetUpSmsAccess => 'Configurar acceso a SMS';

  @override
  String get modulesRestartApp => 'Reiniciar CopyPaste';

  @override
  String get modulesSmsAccessTitle => 'Acceso a SMS';

  @override
  String get modulesCloseSmsAccess => 'Cerrar acceso a SMS';

  @override
  String get modulesSmsChecking => 'Comprobando el acceso a SMS.';

  @override
  String get modulesSmsReady =>
      'El acceso está listo. Activa SMS Codes para empezar a copiar los códigos nuevos.';

  @override
  String get modulesSmsAllowAccess =>
      'Permite el acceso para copiar los códigos de los SMS nuevos.';

  @override
  String get modulesSmsNotification => 'Notificación de SMS';

  @override
  String get modulesSmsNotificationDescription =>
      'Necesaria mientras SMS Codes está activo';

  @override
  String get modulesSmsApp => 'CopyPaste SMS';

  @override
  String get modulesSmsAppDescription => 'App aparte, la única que lee los SMS';

  @override
  String get modulesSmsApplyAccess => 'Aplicar acceso a SMS';

  @override
  String get modulesSmsApplyAccessDescription =>
      'Aplica el acceso único a SMS.';

  @override
  String get modulesSmsSetupFailed => 'Error en la configuración de SMS';

  @override
  String get modulesFileSelectionFailed => 'Error al seleccionar el archivo';

  @override
  String get modulesChooseLanguages => 'Elegir idiomas';

  @override
  String get modulesChooseFile => 'Elegir archivo';

  @override
  String modulesModelReady(String model) {
    return 'El modelo $model está listo';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return 'Modelo $model · descarga de $size MiB';
  }

  @override
  String get modulesPlatformsUnknown => 'Plataformas: desconocidas';

  @override
  String modulesPlatforms(String platforms) {
    return 'Plataformas: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'La firma del catálogo de módulos no es válida.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'No se pudo conectar con el catálogo de módulos.';

  @override
  String get modulesMarketplaceTimeout =>
      'El catálogo de módulos no respondió.';

  @override
  String get modulesIncompatible =>
      'Este módulo no es compatible con este dispositivo.';

  @override
  String get modulesPackageSizeInvalid =>
      'El tamaño del paquete del módulo no es válido.';

  @override
  String get modulesDownloadInterrupted =>
      'Se interrumpió la descarga del módulo.';

  @override
  String get modulesDownloadTimeout => 'La descarga del módulo no respondió.';

  @override
  String get modulesPackageUrlUntrusted =>
      'La URL del paquete del módulo no es de confianza.';

  @override
  String get modulesDownloadLeftGithub =>
      'La descarga del módulo salió de GitHub.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'La descarga del módulo usó demasiadas redirecciones.';

  @override
  String get modulesDownloadTooLarge =>
      'La descarga del módulo es demasiado grande.';

  @override
  String get modulesPackageIntegrityFailed =>
      'El paquete del módulo no superó la comprobación de integridad.';

  @override
  String get modulesDownloadUnavailable =>
      'La descarga de este módulo no está disponible en este momento.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'El catálogo de módulos no está disponible. Vuelve a intentarlo.';

  @override
  String get modulesCatalogInvalid => 'El catálogo de módulos no es válido.';

  @override
  String get modulesThisSystem => 'este sistema';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version o posterior';
  }

  @override
  String get modulesNotAvailableForDevice =>
      'No disponible para este dispositivo.';

  @override
  String get modulesAppVersionUnverified =>
      'No se pudo verificar la versión instalada de CopyPaste.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Este módulo no es compatible con CopyPaste $version.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Requiere $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed =>
      'No se pudo cargar el catálogo de módulos.';

  @override
  String get modulesSmsSetupRequired =>
      'Configura el acceso a SMS antes de activar este módulo.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'No se pudo iniciar la supervisión de SMS. El módulo sigue activado; vuelve a desactivarlo para reintentarlo.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'No se pudo iniciar la supervisión de SMS. Se desactivó el módulo.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'No se pudo iniciar la supervisión de SMS. No se pudo confirmar si el módulo está activado.';

  @override
  String get modulesSmsMonitoringFailed =>
      'No se pudo iniciar la supervisión de SMS.';

  @override
  String get modulesSmsAndroidOnly =>
      'El acceso a SMS solo está disponible en Android.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'La supervisión de SMS requiere configuración.';

  @override
  String get modulesSmsMonitoringUnavailable =>
      'No se pudo comprobar la supervisión de SMS.';

  @override
  String get modulesOperationIncomplete =>
      'No se pudo completar la operación del módulo.';

  @override
  String get modulesUnsupportedEvent =>
      'El módulo usa un evento no compatible.';

  @override
  String get modulesUnsupportedResult =>
      'El módulo devolvió un resultado no compatible.';

  @override
  String get modulesUnsupportedField => 'El módulo usa un campo no compatible.';

  @override
  String get modulesResponseInvalid => 'La respuesta del módulo no es válida.';

  @override
  String get modulesFileSelectionUnavailable =>
      'La selección de archivos no está disponible.';

  @override
  String get modulesFileSelectFailed => 'No se pudo seleccionar el archivo.';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field no es un archivo aceptado.';
  }

  @override
  String get modulesSmsVerifyFailed => 'No se pudo verificar el acceso a SMS.';

  @override
  String get modulesSmsAllowNotifications =>
      'Permite las notificaciones para mantener SMS Codes activo en segundo plano.';

  @override
  String get modulesSmsNotGranted => 'No se concedió el acceso a SMS.';

  @override
  String get modulesSmsConfigureFailed =>
      'No se pudo configurar el acceso a SMS.';

  @override
  String get onboardingLinuxContinueBasic => 'Continuar con la captura básica';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Opcional. Necesario para Quick Paste y las exclusiones de apps de origen.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Opcional. Necesario para que Quick Paste vuelva a la app anterior.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'El acceso al portapapeles no está disponible en esta sesión Wayland. Activa el complemento firmado de GNOME o KDE y vuelve a comprobarlo.';

  @override
  String get historyMoreActions => 'Más acciones';

  @override
  String get historyTagsAction => 'Etiquetas…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'desbloquear elementos secretos',
      'other': 'Desbloquear elementos secretos',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'copiar un elemento secreto',
      'other': 'Copiar un elemento secreto',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'No disponible en este sistema';

  @override
  String get deviceAuthNotConfigured =>
      'Primero configura una contraseña, un PIN o la biometría del dispositivo';

  @override
  String get commonOn => 'Activado';

  @override
  String get commonOff => 'Desactivado';

  @override
  String get settingsQuickPasteStartFailed =>
      'No se pudo iniciar Quick Paste, así que su atajo no funciona.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return 'Otra aplicación usa $shortcut. Elige otro atajo.';
  }

  @override
  String get devicesEmptyTitle => 'Aún no hay otros dispositivos vinculados';

  @override
  String get devicesEmptyMessage =>
      'Vincula un dispositivo de la misma red para sincronizar el historial.';

  @override
  String get pairingStateNotCompleted => 'Vinculación no completada';

  @override
  String get pairingStepInvite => 'Invitación';

  @override
  String get pairingStepScan => 'Escaneo';

  @override
  String get pairingStepCode => 'Código';

  @override
  String get pairingStepRequest => 'Solicitud';

  @override
  String get pairingStepConnect => 'Conexión';

  @override
  String get pairingStepConfirm => 'Confirmación';

  @override
  String get pairingStepPaired => 'Vinculado';

  @override
  String get pairingOtherDevice => 'Otro dispositivo';

  @override
  String get pairingDecisionNotConfirmed => 'Aún sin confirmar';

  @override
  String get pairingDecisionSending => 'Enviando tu decisión…';

  @override
  String get pairingDecisionConfirmed => 'Confirmado';

  @override
  String get pairingDecisionRejected => 'Rechazado';

  @override
  String get pairingDecisionWaiting => 'Esperando confirmación';

  @override
  String get pairingDecisionUnknown => 'Respuesta desconocida';

  @override
  String syncHeaderDeviceCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dispositivos',
      many: '$count dispositivos',
      one: '$count dispositivo',
    );
    return '$_temp0';
  }

  @override
  String historyClipLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count líneas',
      many: '$count líneas',
      one: '$count línea',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Navegación principal';

  @override
  String sharedStepCounter(int step, int count) {
    return 'Paso $step de $count';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Paso $step de $count: $name';
  }

  @override
  String get modulesPhaseDownload => 'Descarga';

  @override
  String get modulesPhaseInstall => 'Instalación';

  @override
  String modulesInstallingModule(String module) {
    return 'Instalando $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return 'Actualizando $module';
  }

  @override
  String get commonDismiss => 'Descartar';

  @override
  String get settingsRetentionConfirmTitle =>
      '¿Eliminar el historial más antiguo?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Nuevo límite: $limit. Los elementos no fijados más antiguos se eliminan para siempre.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle =>
      '¿Reducir la cuota de almacenamiento?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Nueva cuota: $size. Los elementos no fijados más antiguos que la superen se eliminan para siempre.';
  }

  @override
  String get settingsLimitConfirmAction => 'Aplicar límite';

  @override
  String get settingsHideEverywhereTitle => '¿Ocultar CopyPaste por completo?';

  @override
  String get settingsHideEverywhereMessage =>
      'Con ambas desactivadas, no hay icono. Vuelve a iniciar CopyPaste para abrirlo.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Con ambas desactivadas, no hay icono. Para abrirlo, pulsa $shortcut o vuelve a iniciar CopyPaste.';
  }

  @override
  String get settingsHideEverywhereAction => 'Ocultar';

  @override
  String get settingsExportDoneTitle => 'Historial de texto exportado';

  @override
  String get settingsBackupDoneTitle => 'Copia de seguridad creada';

  @override
  String get settingsRestoreDoneTitle => 'Historial restaurado';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count recortes de texto exportados',
      many: '$count recortes de texto exportados',
      one: '$count recorte de texto exportado',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count no se pudieron leer',
      many: '$count no se pudieron leer',
      one: '$count no se pudo leer',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Copia de seguridad cifrada guardada · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'El historial del portapapeles de este dispositivo ahora coincide con la copia de seguridad.';

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
    return '$used de $quota';
  }

  @override
  String storageUsageOfQuota(String used, String quota) {
    return '$used usados de $quota';
  }

  @override
  String storageUsageNoQuota(String used) {
    return '$used usados';
  }

  @override
  String get storageUsageOverQuota => 'Cuota superada';

  @override
  String storageUsageOldestClip(String date) {
    return 'Recorte más antiguo: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Almacenamiento: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Nueva cuota: $size. Hay $used en uso, así que los elementos no fijados más antiguos se eliminan para siempre.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count recortes',
      many: '$count recortes',
      one: '$count recorte',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'Abrir CopyPaste';

  @override
  String get trayOpenQuickPaste => 'Abrir Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Abrir Quick Paste ($shortcut)';
  }

  @override
  String get trayRecent => 'Recientes';

  @override
  String get trayRecentLoading => 'Cargando…';

  @override
  String get trayRecentFailed => 'No se pudieron cargar los clips';

  @override
  String get trayRecentEmpty => 'Aún no hay clips';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips recientes ocultos',
      one: '1 clip reciente oculto',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Secreto';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Copiar lo último de otro dispositivo';

  @override
  String get trayCaptureNow => 'Capturar ahora';

  @override
  String trayPausedUntil(String time) {
    return 'En pausa hasta las $time';
  }

  @override
  String get trayPauseFor => 'Pausar durante…';

  @override
  String get trayPauseFor15Minutes => '15 minutos';

  @override
  String get trayPauseForOneHour => '1 hora';

  @override
  String get trayPauseUntilResumed => 'Hasta que la reanude';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count elementos',
      one: 'Paste Queue: 1 elemento',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Recopilando: $count elementos',
      one: 'Recopilando: 1 elemento',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Omitidos recientemente ($count)';
  }

  @override
  String get trayOpenContentRules => 'Abrir reglas de contenido';

  @override
  String get trayRecentSettingsTitle => 'Barra de menús y bandeja';

  @override
  String get trayRecentSettingsDescription =>
      'Lo que el menú de la barra de menús o de la bandeja muestra en Recientes.';

  @override
  String get trayRecentCountTitle => 'Clips recientes';

  @override
  String get trayRecentCountDescription =>
      'Cuántos de los clips más nuevos muestra el menú.';

  @override
  String get trayRecentPreviewTitle => 'Vista previa de clips recientes';

  @override
  String get trayRecentPreviewDescription =>
      'Cuánto texto de los clips muestra el menú.';

  @override
  String get trayRecentPreviewFull => 'Vista previa completa';

  @override
  String get trayRecentPreviewTitles => 'Solo tipo y origen';

  @override
  String get trayRecentPreviewHidden => 'Oculto';

  @override
  String get trayRecentSettingsKeywords =>
      'bandeja barra de menús recientes vista previa privacidad cantidad';

  @override
  String get transformsPaletteTitle => 'Transformar texto';

  @override
  String get transformsFilterPlaceholder => 'Filtrar transformaciones';

  @override
  String get transformsNoMatches => 'No hay transformaciones que coincidan';

  @override
  String get transformsGroupResults => 'Resultados';

  @override
  String get transformsGroupSuggested => 'Sugeridas';

  @override
  String get transformsGroupRecent => 'Recientes';

  @override
  String get transformsGroupFavorites => 'Favoritas';

  @override
  String get transformsGroupPipelines => 'Secuencias';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV y XML';

  @override
  String get transformsCategoryEncode => 'Codificar y decodificar';

  @override
  String get transformsCategoryCase => 'Mayúsculas y minúsculas';

  @override
  String get transformsCategoryHash => 'Hashes';

  @override
  String get transformsCategoryLines => 'Líneas y espacios';

  @override
  String get transformsCategoryClean => 'Limpiar';

  @override
  String get transformsCategoryUrl => 'Enlaces';

  @override
  String get transformsCategoryMarkup => 'Texto enriquecido y Markdown';

  @override
  String get transformsCategoryDev => 'Desarrollo';

  @override
  String get transformsCategoryOther => 'Otras';

  @override
  String get transformsChainEmpty => 'Aún no hay pasos';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Luego: $name';
  }

  @override
  String get transformsStepEnabled => 'Ejecutar este paso';

  @override
  String get transformsMoveEarlier => 'Subir';

  @override
  String get transformsMoveLater => 'Bajar';

  @override
  String get transformsRemoveStep => 'Quitar paso';

  @override
  String get transformsSavePipeline => 'Guardar como secuencia';

  @override
  String get transformsSavePipelineTitle => 'Guardar secuencia';

  @override
  String get transformsSavePipelineFailed => 'No se pudo guardar la secuencia.';

  @override
  String get transformsPipelineNameLabel => 'Nombre';

  @override
  String get transformsPipelineDescriptionLabel => 'Descripción (opcional)';

  @override
  String get transformsPipelineStepsLabel => 'Pasos';

  @override
  String get transformsPipelineNoSteps => 'Añade al menos un paso.';

  @override
  String get transformsPipelineNameRequired => 'Escribe un nombre.';

  @override
  String get transformsPipelineNameTooLong => 'Usa como máximo 80 caracteres.';

  @override
  String get transformsPipelineNameTaken =>
      'Otra secuencia ya tiene este nombre.';

  @override
  String get transformsPipelineSavedTitle => 'Secuencia guardada';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name está disponible en todas las paletas de transformación.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Pasos: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Secuencia eliminada';

  @override
  String get transformsNeedsNewerVersion =>
      'Requiere una versión más reciente de CopyPaste';

  @override
  String get transformsLoadFailed =>
      'No se pudieron cargar las transformaciones.';

  @override
  String get transformsNotTextTitle =>
      'Las transformaciones funcionan con texto';

  @override
  String get transformsNotTextMessage =>
      'Este elemento es una imagen, un archivo o un color. Elige un elemento de texto o copia texto.';

  @override
  String get transformsNothingTitle => 'Nada que transformar';

  @override
  String get transformsNothingMessage =>
      'Copia texto o elige un elemento del historial.';

  @override
  String get transformsPickTransform => 'Elige una transformación';

  @override
  String get transformsPickTransformHint =>
      'Tab la añade como paso. Intro aplica el resultado.';

  @override
  String get transformsPreviewing => 'Actualizando vista previa…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Líneas: $lines · Palabras: $words · Caracteres: $chars';
  }

  @override
  String get transformsNoChanges => 'Sin cambios';

  @override
  String transformsTruncated(String size) {
    return 'Se muestran los primeros 64 KiB de $size';
  }

  @override
  String get transformsRandom => 'Aleatorio';

  @override
  String get transformsNotRun => 'No ejecutado';

  @override
  String get transformsSensitiveHidden =>
      'El resultado es confidencial y está oculto.';

  @override
  String get transformsReveal => 'Mostrar';

  @override
  String get transformsPaneOutput => 'Salida';

  @override
  String get transformsPaneInput => 'Entrada';

  @override
  String get transformsPaneChanges => 'Cambios';

  @override
  String get transformsCannotTransform => 'No se puede transformar este texto';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Paso $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Mostrar en hexadecimal';

  @override
  String get transformsActionPaste => 'Pegar';

  @override
  String get transformsActionSave => 'Guardar como nuevo elemento';

  @override
  String get transformsCopiedMessage =>
      'El resultado está en el portapapeles y en el historial.';

  @override
  String get transformsSavedTitle => 'Guardado';

  @override
  String get transformsSavedMessage =>
      'Guardado como nuevo elemento del historial.';

  @override
  String get transformsPasteFailedTitle => 'No se pudo pegar';

  @override
  String get transformsPasteFailedMessage =>
      'El resultado está en el portapapeles. Pégalo manualmente.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message en la línea $line, columna $column';
  }

  @override
  String get transformsErrorBinary =>
      'Los datos decodificados son binarios, no texto';

  @override
  String get transformsErrorTooLarge => 'Demasiado grande para transformar';

  @override
  String get transformsErrorInvalidParam =>
      'Un ajuste de este paso no es válido';

  @override
  String get transformsErrorCancelled => 'Cancelado';

  @override
  String get transformsErrorTooExpensive => 'Patrón demasiado costoso';

  @override
  String get transformsErrorPattern => 'El patrón no es válido';

  @override
  String get transformsErrorTimestampUnit =>
      'Elige si el número está en segundos o milisegundos';

  @override
  String get transformsErrorExternalEntity =>
      'No se permiten entidades externas';

  @override
  String get transformsErrorJwtEncrypted =>
      'Los tokens cifrados no se pueden decodificar';

  @override
  String get transformsErrorJson => 'JSON no válido';

  @override
  String get transformsErrorYaml => 'YAML no válido';

  @override
  String get transformsErrorXml => 'XML no válido';

  @override
  String get transformsErrorCsv => 'CSV no válido';

  @override
  String get transformsErrorBase64 => 'Base64 no válido';

  @override
  String get transformsErrorHex => 'Hexadecimal no válido';

  @override
  String get transformsErrorEscape => 'Secuencia de escape no válida';

  @override
  String get transformsErrorUrl => 'Codificación porcentual incorrecta';

  @override
  String get transformsErrorJwt => 'No es un JWT válido';

  @override
  String get transformsErrorDate =>
      'No es una fecha ni una marca de tiempo reconocida';

  @override
  String get transformsErrorGeneric =>
      'Este paso no puede transformar su entrada';

  @override
  String get transformsWarningSignedLink => 'Enlace firmado sin cambios';

  @override
  String get transformsWarningExpired => 'Caducado';

  @override
  String get transformsWarningNotYetValid => 'Aún no es válido';

  @override
  String get transformsWarningSignatureNotVerified => 'Firma no verificada';

  @override
  String get transformsWarningDuplicateKeys => 'Claves duplicadas';

  @override
  String get transformsWarningLenientJson => 'Se aceptó JSON no estándar';

  @override
  String get transformsWarningDoubleEncoded =>
      'El JSON estaba codificado dos veces';

  @override
  String get transformsWarningEscapes =>
      'Secuencias de escape desconocidas conservadas tal cual';

  @override
  String get transformsWarningLocalZone => 'Se asumió la zona horaria local';

  @override
  String get transformsWarningNoMatches => 'Sin coincidencias';

  @override
  String get transformsWarningGeneric => 'Revisa el resultado';

  @override
  String get transformsRestoreTitle => 'Restaurar el portapapeles tras pegar';

  @override
  String get transformsRestoreDescription =>
      'Recupera el portapapeles anterior después de pegar.';

  @override
  String get transformsDefaultTitle => 'Transformación predeterminada al pegar';

  @override
  String get transformsDefaultDescription =>
      'Se aplica al texto que pegas desde Quick Paste.';

  @override
  String get transformsNoDefault => 'Ninguna';

  @override
  String get transformsAppDefaultsTitle => 'Predeterminados por aplicación';

  @override
  String get transformsAppDefaultsDescription =>
      'Usa otro valor predeterminado en apps concretas.';

  @override
  String transformsOtherPlatform(String platform) {
    return 'solo en $platform';
  }

  @override
  String get transformsAddApp => 'Añadir aplicación';

  @override
  String get transformsRemove => 'Quitar';

  @override
  String get transformsFavoritesTitle => 'Favoritas';

  @override
  String get transformsFavoritesDescription =>
      'Aparecen primero en cada paleta.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Mod+$number en la paleta';
  }

  @override
  String get transformsAddFavorite => 'Añadir favorita';

  @override
  String get transformsPipelinesTitle => 'Secuencias';

  @override
  String get transformsPipelinesDescription =>
      'Cadenas guardadas de transformaciones.';

  @override
  String get transformsNewPipeline => 'Nueva secuencia';

  @override
  String get transformsEditPipeline => 'Editar secuencia';

  @override
  String get transformsDuplicatePipeline => 'Duplicar';

  @override
  String get transformsDeletePipeline => 'Eliminar';

  @override
  String transformsDeletePipelineTitle(String name) {
    return '¿Eliminar $name?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Se borran las favoritas y los valores predeterminados que la usan.';

  @override
  String get transformsDeletePipelineFailed =>
      'No se pudo eliminar la secuencia.';

  @override
  String get transformsAddStep => 'Añadir un paso';

  @override
  String get transformsHiddenTitle => 'Transformaciones ocultas';

  @override
  String get transformsHiddenDescription =>
      'No aparecen en paletas ni sugerencias.';

  @override
  String get transformsHideTransform => 'Ocultar una transformación';

  @override
  String get transformsShow => 'Mostrar';

  @override
  String get transformsSuggestionsTitle => 'Sugerencias';

  @override
  String get transformsSuggestionsDescription =>
      'Sugiere transformaciones adecuadas para el texto.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web Tokens';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl =>
      'Enlaces con parámetros de seguimiento';

  @override
  String get transformsDetectorTimestamp => 'Marcas de tiempo';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Parámetros de seguimiento';

  @override
  String get transformsTrackingDescription =>
      'Parámetros de enlace que también quitar o conservar siempre.';

  @override
  String get transformsTrackingRemoveLabel => 'Quitar también';

  @override
  String get transformsTrackingAllowLabel => 'Conservar siempre';

  @override
  String get transformsTrackingPlaceholder =>
      'nombre o nombre@dominio y luego Intro';

  @override
  String get quickAccessUnavailableTitle =>
      'Estado del acceso rápido no disponible';

  @override
  String get quickAccessUnavailableMessage =>
      'CopyPaste no pudo leer el estado del teclado, los mosaicos y el widget. Inténtalo de nuevo.';

  @override
  String get quickAccessActionFailedTitle => 'No se pudo completar';

  @override
  String get quickAccessActionFailedMessage =>
      'Android no abrió la pantalla solicitada ni guardó el cambio. Inténtalo de nuevo.';

  @override
  String get quickAccessKeyboardTitle => 'Teclado de recortes';

  @override
  String get quickAccessKeyboardDescription =>
      'Activa Recortes de CopyPaste en los ajustes de teclado.';

  @override
  String get quickAccessKeyboardOn => 'Activado';

  @override
  String get quickAccessKeyboardOff => 'Desactivado';

  @override
  String get quickAccessOpenKeyboardSettings => 'Abrir ajustes de teclado';

  @override
  String get quickAccessKeyboardSearchDescription =>
      'Activa el panel de teclado Recortes de CopyPaste.';

  @override
  String get quickAccessKeyboardKeywords =>
      'teclado método de entrada ime panel activar';

  @override
  String get quickAccessSwitchKeyboardTitle => 'Cambiar al teclado de recortes';

  @override
  String get quickAccessSwitchKeyboardDescription =>
      'Usa Recortes de CopyPaste como teclado actual.';

  @override
  String get quickAccessSwitchKeyboardAction => 'Elegir teclado';

  @override
  String get quickAccessKeyboardCurrent => 'Teclado actual';

  @override
  String get quickAccessCaptureTierTitle => 'Modo de captura';

  @override
  String get quickAccessCaptureTierDescription =>
      'Cómo se guardan las copias de otras apps.';

  @override
  String get quickAccessCaptureTierSearchDescription =>
      'Consulta cómo se guardan las copias de otras apps.';

  @override
  String get quickAccessCaptureTierKeywords =>
      'captura nivel completo panel manual shizuku recopilar';

  @override
  String get quickAccessTierFull =>
      'Completo: cada copia se guarda en segundo plano';

  @override
  String get quickAccessTierPanelCollecting =>
      'Panel seleccionado: recopilando';

  @override
  String get quickAccessTierPanelCatchUp => 'Modo panel: recuperación al abrir';

  @override
  String get quickAccessTierManual =>
      'Manual: guarda desde la app, un mosaico, el widget o Compartir';

  @override
  String get quickAccessTierPaused => 'Captura en pausa';

  @override
  String get quickAccessAutoReturnTitle =>
      'Volver al teclado anterior después de insertar';

  @override
  String get quickAccessAutoReturnDescription =>
      'Vuelve al teclado anterior tras insertar un recorte.';

  @override
  String get quickAccessAutoReturnKeywords =>
      'volver teclado anterior automático insertar';

  @override
  String get quickAccessFocusSearchTitle =>
      'Enfocar la búsqueda al abrir el panel';

  @override
  String get quickAccessFocusSearchDescription =>
      'Abre el panel con la búsqueda lista.';

  @override
  String get quickAccessTilesTitle => 'Mosaicos de ajustes rápidos';

  @override
  String get quickAccessTilesSearchDescription =>
      'Añade los mosaicos Guardar portapapeles y Captura del portapapeles.';

  @override
  String get quickAccessTilesKeywords =>
      'ajustes rápidos mosaico panel guardar pausa';

  @override
  String get quickAccessTileInstructions =>
      'Desliza dos veces hacia abajo, toca Editar y añade un mosaico de CopyPaste.';

  @override
  String get quickAccessSaveTileTitle => 'Mosaico Guardar portapapeles';

  @override
  String get quickAccessSaveTileDescription =>
      'Guarda el contenido del portapapeles desde los ajustes rápidos.';

  @override
  String get quickAccessCaptureTileTitle => 'Mosaico Captura del portapapeles';

  @override
  String get quickAccessCaptureTileDescription =>
      'Pausa y reanuda la captura desde los ajustes rápidos.';

  @override
  String get quickAccessAddTile => 'Añadir mosaico';

  @override
  String get quickAccessTileAdded => 'Añadido a los ajustes rápidos';

  @override
  String get quickAccessWidgetTitle => 'Widget Recortes recientes';

  @override
  String get quickAccessWidgetDescription =>
      'Recortes recientes en la pantalla de inicio; toca uno para copiarlo.';

  @override
  String get quickAccessWidgetInstructions =>
      'Mantén pulsada la pantalla de inicio, toca Widgets y añade Recortes recientes.';

  @override
  String quickAccessWidgetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count widgets añadidos',
      one: '1 widget añadido',
      zero: 'No añadido',
    );
    return '$_temp0';
  }

  @override
  String get quickAccessAddWidget => 'Añadir widget';

  @override
  String get quickAccessWidgetKeywords =>
      'pantalla de inicio widget launcher recientes';

  @override
  String get snippetsClipTooLarge =>
      'Demasiado grande para editar. Este recorte se abre en modo de solo lectura.';

  @override
  String get snippetsConcealedConfirmAction => 'Continuar';

  @override
  String get snippetsConcealedConfirmMessage =>
      'Este dispositivo no puede verificarte. Continúa solo si es tu dispositivo.';

  @override
  String get snippetsConcealedConfirmTitle => 'Confirma que eres tú';

  @override
  String get snippetsDiscard => 'Descartar';

  @override
  String get snippetsEditClip => 'Editar recorte';

  @override
  String get snippetsFormattingNotice => 'El formato se eliminará al guardar.';

  @override
  String get snippetsNewText => 'Texto nuevo';

  @override
  String get snippetsReplaceOriginal => 'Reemplazar el original';

  @override
  String get snippetsReplaceOriginalHint =>
      'Elimina el recorte original después de guardar.';

  @override
  String get snippetsSaveFailed => 'No se pudo guardar';

  @override
  String get snippetsUnsavedMessage => '¿Guardar los cambios antes de cerrar?';

  @override
  String get snippetsUnsavedTitle => 'Cambios sin guardar';

  @override
  String get quickPasteTabQueue => 'Cola';

  @override
  String get quickPasteMaskedMode =>
      'Campo de contraseña: las vistas previas están ocultas';

  @override
  String quickPasteMaskedItem(String kind) {
    return 'Oculto: $kind';
  }

  @override
  String get quickPasteUnlockSecrets => 'Desbloquear elementos secretos';

  @override
  String quickPastePastingWith(String name) {
    return 'Pegando con: $name';
  }

  @override
  String quickPastePasteOriginalHint(String shortcut) {
    return '$shortcut pega el original';
  }

  @override
  String get quickPasteDefaultSkipped =>
      'Transformación predeterminada omitida';

  @override
  String get quickPasteDefaultSkippedMessage =>
      'Se usó el original en su lugar.';

  @override
  String get quickPastePasteManually =>
      'El contenido está en el portapapeles. Pégalo manualmente.';

  @override
  String get quickPastePaste => 'Pegar';

  @override
  String get rulesLoadFailed =>
      'No se pudieron cargar las reglas de contenido.';

  @override
  String get rulesNewRule => 'Nueva regla';

  @override
  String get rulesEditRule => 'Editar regla';

  @override
  String get rulesUntitled => 'Regla sin nombre';

  @override
  String get rulesPrivacyDefaultsTitle => 'Valores de privacidad';

  @override
  String get rulesPrivacyDefaultsDescription =>
      'Los ajustes de privacidad se aplican antes que cualquier regla.';

  @override
  String get rulesPrivacyDefaultsKeywords =>
      'privacidad oculto temporal apps excluidas integradas credenciales contraseñas tarjetas';

  @override
  String get rulesOpenPrivacy => 'Abrir Privacidad';

  @override
  String get rulesMyRulesTitle => 'Mis reglas';

  @override
  String get rulesMyRulesDescription =>
      'Se ejecutan en orden sobre las copias nuevas.';

  @override
  String get rulesMyRulesKeywords =>
      'reglas automatización etiqueta omitir secreto caducar eliminar patrón regex';

  @override
  String get rulesEmptyTitle => 'Aún no hay reglas';

  @override
  String get rulesEmptyMessage =>
      'Las reglas etiquetan, protegen, caducan u omiten copias nuevas.';

  @override
  String get rulesDeletedTitle => 'Eliminadas recientemente';

  @override
  String get rulesDeletedDescription => 'Se pueden restaurar durante 30 días.';

  @override
  String rulesDeletedAt(String time) {
    return 'Eliminada: $time';
  }

  @override
  String get rulesRestore => 'Restaurar';

  @override
  String get rulesDeletedToast => 'Regla eliminada';

  @override
  String get rulesDeletedToastMessage => 'Puedes restaurarla durante 30 días.';

  @override
  String get rulesUndo => 'Deshacer';

  @override
  String rulesConflictMessage(String rule, String device) {
    return 'Una edición simultánea de «$rule» desde $device se sustituyó por una más reciente.';
  }

  @override
  String get rulesAnotherDevice => 'otro dispositivo';

  @override
  String get rulesRestoreConflict => 'Usar esa versión';

  @override
  String get rulesSkipLogTitle => 'Omitidos recientemente';

  @override
  String get rulesSkipLogDescription =>
      'Copias que tus reglas no guardaron en este dispositivo.';

  @override
  String get rulesSkipLogKeywords => 'omitidos ignorados no guardados registro';

  @override
  String get rulesSkipLogShow => 'Mostrar';

  @override
  String get rulesSkipLogEmpty => 'No se omitió ninguna copia.';

  @override
  String get rulesUnknownApp => 'App desconocida';

  @override
  String get rulesMatchesEverything => 'Cualquier copia';

  @override
  String rulesSummary(String conditions, String actions) {
    return '$conditions → $actions';
  }

  @override
  String rulesSummaryStop(String summary) {
    return '$summary y detener';
  }

  @override
  String rulesConditionJoin(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {'any': ' o ', 'other': ' y '});
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
    return 'Se ejecuta en $devices';
  }

  @override
  String rulesNewFromDevice(String device) {
    return 'Nueva de $device';
  }

  @override
  String get rulesNeedsUpdateBadge => 'Requiere actualización';

  @override
  String get rulesUpdateRequired =>
      'Actualiza CopyPaste para ejecutar esta regla.';

  @override
  String get rulesErrorBadge => 'Error';

  @override
  String rulesErrorHint(String problem) {
    return '$problem Hasta corregirlo, las coincidencias se protegen.';
  }

  @override
  String get rulesReorder => 'Arrastra para reordenar';

  @override
  String get rulesMoreActions => 'Más acciones';

  @override
  String get rulesEdit => 'Editar';

  @override
  String rulesMove(String direction) {
    String _temp0 = intl.Intl.selectLogic(direction, {
      'up': 'Subir',
      'down': 'Bajar',
      'top': 'Mover al principio',
      'other': 'Mover al final',
    });
    return '$_temp0';
  }

  @override
  String get rulesApplyToItems => 'Aplicar a elementos existentes…';

  @override
  String get rulesResetBuiltin => 'Restablecer valores predeterminados';

  @override
  String get rulesNameLabel => 'Nombre';

  @override
  String get rulesNamePlaceholder => 'Por ejemplo, Enlaces de trabajo';

  @override
  String get rulesWhenTitle => 'Cuándo';

  @override
  String rulesMatchMode(String mode) {
    String _temp0 = intl.Intl.selectLogic(mode, {
      'any': 'Se cumple alguna condición',
      'other': 'Se cumplen todas las condiciones',
    });
    return '$_temp0';
  }

  @override
  String get rulesNoConditions =>
      'Sin condiciones: esta regla se aplica a todas las copias.';

  @override
  String get rulesAddCondition => 'Añadir condición';

  @override
  String get rulesRemoveCondition => 'Quitar condición';

  @override
  String rulesConditionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'content_text': 'Texto',
      'content_type': 'Tipo de contenido',
      'detected_kind': 'Tipo detectado',
      'source_app': 'App de origen',
      'size': 'Tamaño',
      'privacy_marker': 'Marca de privacidad',
      'capture_origin': 'Origen',
      'other': 'Condición desconocida',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'contiene',
      'equals': 'es',
      'starts_with': 'empieza por',
      'ends_with': 'termina en',
      'matches_regex': 'coincide con el patrón',
      'other': 'coincide',
    });
    return '$_temp0';
  }

  @override
  String rulesTextOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'contains': 'no contiene',
      'equals': 'no es',
      'starts_with': 'no empieza por',
      'ends_with': 'no termina en',
      'matches_regex': 'no coincide con el patrón',
      'other': 'no coincide',
    });
    return '$_temp0';
  }

  @override
  String rulesListOperator(String negate) {
    String _temp0 = intl.Intl.selectLogic(negate, {
      'true': 'no es',
      'other': 'es',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperator(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'es menor que',
      'other': 'es mayor que',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeOperatorNegated(String op) {
    String _temp0 = intl.Intl.selectLogic(op, {
      'less_than': 'no es menor que',
      'other': 'no es mayor que',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeUnit(String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'líneas',
      'bytes': 'bytes',
      'other': 'caracteres',
    });
    return '$_temp0';
  }

  @override
  String rulesSizeValue(int count, String unit) {
    String _temp0 = intl.Intl.selectLogic(unit, {
      'lines': 'líneas',
      'bytes': 'bytes',
      'other': 'caracteres',
    });
    return '$count $_temp0';
  }

  @override
  String rulesContentKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'plain_text': 'Texto sin formato',
      'rich_text': 'Texto enriquecido',
      'image': 'Imagen',
      'files': 'Archivos',
      'color': 'Color',
      'link': 'Enlace',
      'other': 'Otro',
    });
    return '$_temp0';
  }

  @override
  String rulesDetectedKind(String kind) {
    String _temp0 = intl.Intl.selectLogic(kind, {
      'credential': 'Contraseña o secreto',
      'url': 'Dirección web',
      'email': 'Correo electrónico',
      'phone': 'Número de teléfono',
      'code': 'Código',
      'color': 'Valor de color',
      'json': 'JSON',
      'file_path': 'Ruta de archivo',
      'number': 'Número',
      'one_time_code': 'Código de un solo uso',
      'payment_card': 'Número de tarjeta',
      'other': 'Otro',
    });
    return '$_temp0';
  }

  @override
  String rulesPrivacyMarker(String marker) {
    String _temp0 = intl.Intl.selectLogic(marker, {
      'concealed': 'Oculto',
      'transient': 'Temporal',
      'auto_generated': 'Generado automáticamente',
      'excluded_from_history': 'Excluido del historial',
      'other': 'Otro',
    });
    return '$_temp0';
  }

  @override
  String rulesCaptureOrigin(String origin) {
    String _temp0 = intl.Intl.selectLogic(origin, {
      'local_copy': 'Copiado en este dispositivo',
      'received': 'Recibido de otro dispositivo',
      'created_in_app': 'Creado en CopyPaste',
      'other': 'Otro',
    });
    return '$_temp0';
  }

  @override
  String get rulesValuePlaceholder => 'Texto que buscar';

  @override
  String get rulesPatternPlaceholder => 'Patrón, por ejemplo ^ticket-[0-9]+';

  @override
  String get rulesPatternHint =>
      'No se admiten búsquedas alrededor ni referencias inversas.';

  @override
  String get rulesCaseSensitive => 'Distinguir mayúsculas';

  @override
  String get rulesChooseValues => 'Elegir';

  @override
  String get rulesUnknownConditionNotice =>
      'Esta condición proviene de una versión más reciente de CopyPaste.';

  @override
  String get rulesSourceAppNotice =>
      'En Android solo coinciden las copias hechas con el teclado de recortes.';

  @override
  String get rulesPrivacyMarkerNotice =>
      'No todos los sistemas informan todas las marcas.';

  @override
  String get rulesThenTitle => 'Entonces';

  @override
  String get rulesNoActions => 'Añade al menos una acción.';

  @override
  String get rulesAddAction => 'Añadir acción';

  @override
  String get rulesRemoveAction => 'Quitar acción';

  @override
  String rulesActionType(String type) {
    String _temp0 = intl.Intl.selectLogic(type, {
      'dont_save': 'No guardar',
      'mark_secret': 'Marcar como secreto',
      'delete_after': 'Eliminar después de',
      'add_tags': 'Añadir etiquetas',
      'transform': 'Transformar',
      'other': 'Acción desconocida',
    });
    return '$_temp0';
  }

  @override
  String rulesActionBlock(String reason) {
    String _temp0 = intl.Intl.selectLogic(reason, {
      'dont_save_alone':
          '«No guardar» no se puede combinar con otras acciones.',
      'present': 'Esta regla ya tiene esta acción.',
      'other': 'Una regla puede tener hasta 8 acciones.',
    });
    return '$_temp0';
  }

  @override
  String get rulesUnknownActionNotice =>
      'Esta acción proviene de una versión más reciente de CopyPaste.';

  @override
  String get rulesDontSaveWarning =>
      'Las copias que coincidan nunca se guardan, indexan ni sincronizan.';

  @override
  String rulesMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count minutos',
      one: '1 minuto',
    );
    return '$_temp0';
  }

  @override
  String rulesHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count horas',
      one: '1 hora',
    );
    return '$_temp0';
  }

  @override
  String rulesDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count días',
      one: '1 día',
    );
    return '$_temp0';
  }

  @override
  String get rulesClearClipboardAtExpiry =>
      'Vaciar también el portapapeles si aún contiene esta copia.';

  @override
  String get rulesNoTags => 'Crea etiquetas en el Historial para usarlas aquí.';

  @override
  String get rulesNoTransforms => 'No hay transformaciones disponibles.';

  @override
  String get rulesReplaceClipboard =>
      'Sustituir también el portapapeles por el resultado';

  @override
  String get rulesOptionsTitle => 'Opciones';

  @override
  String get rulesEnabledSwitch => 'Activada';

  @override
  String get rulesStopProcessing =>
      'No procesar las reglas siguientes si esta coincide';

  @override
  String get rulesRunsOnLabel => 'Se ejecuta en';

  @override
  String get rulesAllDevices => 'Todos los dispositivos';

  @override
  String get rulesChosenDevices => 'Dispositivos elegidos';

  @override
  String get rulesTestTitle => 'Probar';

  @override
  String get rulesTestDescription => 'Prueba la regla con texto de ejemplo.';

  @override
  String get rulesSamplePlaceholder => 'Pega o escribe texto de ejemplo';

  @override
  String get rulesTestFullSet => 'Probar junto con todas las reglas';

  @override
  String get rulesRunTest => 'Ejecutar prueba';

  @override
  String get rulesTestFailed => 'No se pudo ejecutar la prueba.';

  @override
  String get rulesPreviewHistory => 'Vista previa con el historial';

  @override
  String get rulesSaveFailed => 'No se pudo guardar la regla.';

  @override
  String get rulesReadOnlyNotice =>
      'Creada con un CopyPaste más reciente. Actualiza para editarla o ejecutarla.';

  @override
  String rulesProblem(String code) {
    String _temp0 = intl.Intl.selectLogic(code, {
      'invalid_definition': 'No se pudo leer la regla.',
      'unsupported_schema':
          'Esta regla requiere una versión más reciente de CopyPaste.',
      'unknown_condition_type':
          'Esta condición requiere una versión más reciente de CopyPaste.',
      'unknown_action_type':
          'Esta acción requiere una versión más reciente de CopyPaste.',
      'too_many_conditions': 'Una regla puede tener hasta 20 condiciones.',
      'too_many_actions': 'Una regla puede tener hasta 8 acciones.',
      'too_many_rules': 'Solo se ejecutan las primeras 200 reglas.',
      'no_actions': 'Añade al menos una acción.',
      'conflicting_actions':
          '«No guardar» no se puede combinar con otras acciones.',
      'duplicate_action': 'Cada acción solo se puede usar una vez.',
      'empty_value': 'Introduce un valor.',
      'empty_list': 'Elige al menos un valor.',
      'too_many_values': 'Elige hasta 64 valores.',
      'invalid_value': 'Este valor no es válido.',
      'pattern_too_long': 'Los patrones pueden tener hasta 2000 caracteres.',
      'unsupported_look_around':
          'No se admiten búsquedas anticipadas ni retrospectivas. Usa dos condiciones con «Se cumplen todas las condiciones».',
      'unsupported_backreference': 'No se admiten referencias inversas.',
      'pattern_too_large':
          'Este patrón es demasiado complejo. Simplifica sus repeticiones.',
      'invalid_pattern': 'Este patrón no es válido.',
      'invalid_duration': 'Elige una duración de 1 minuto a 90 días.',
      'invalid_tags': 'Elige al menos una etiqueta.',
      'invalid_transform': 'Elige una transformación.',
      'other': 'Esta regla tiene un problema.',
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
      'matched': 'coincide',
      'not_matched': 'no coincide',
      'unknown': 'desconocido',
      'skipped': 'no ejecutada',
      'other': 'error',
    });
    return '$_temp0';
  }

  @override
  String rulesTraceCondition(int number) {
    return 'Condición $number';
  }

  @override
  String get rulesTraceNoRules => 'No se evaluó ninguna regla.';

  @override
  String get rulesTraceStopped => 'Las reglas siguientes no se ejecutaron.';

  @override
  String get rulesTraceUnknownReason =>
      'Se desconoce para esta copia, así que no coincide.';

  @override
  String get rulesTraceRuleError =>
      'Esta regla no pudo ejecutarse, así que las coincidencias se protegen.';

  @override
  String get rulesTraceOutcomeTitle => 'Resultado';

  @override
  String get rulesTraceNoEffect => 'Ninguna regla cambia esta copia.';

  @override
  String get rulesTraceSkipped => 'No se guarda';

  @override
  String get rulesTraceSecret => 'Se guarda como secreto';

  @override
  String rulesTraceExpires(String duration) {
    return 'Se elimina después de $duration';
  }

  @override
  String rulesTraceTags(String tags) {
    return 'Etiquetada: $tags';
  }

  @override
  String rulesTraceTransforms(String names) {
    return 'Transformada con $names';
  }

  @override
  String get rulesTraceOutput => 'Texto transformado';

  @override
  String get rulesPreviewTitle => 'Aplicar a elementos existentes';

  @override
  String rulesPreviewRunning(int count) {
    return 'Revisando los últimos $count clips…';
  }

  @override
  String get rulesPreviewNone =>
      'Ningún clip reciente coincide con esta regla.';

  @override
  String rulesPreviewMatched(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips coinciden',
      one: '1 clip coincide',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count se eliminarían',
      one: '1 se eliminaría',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewSecret(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count se marcarían como secretos',
      one: '1 se marcaría como secreto',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewExpiring(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count tendrían una hora de eliminación',
      one: '1 tendría una hora de eliminación',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTagged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count se etiquetarían',
      one: '1 se etiquetaría',
    );
    return '$_temp0';
  }

  @override
  String rulesPreviewTruncated(int count) {
    return 'Solo se revisaron los últimos $count clips.';
  }

  @override
  String rulesPreviewMore(int count) {
    return 'y $count más';
  }

  @override
  String get rulesPreviewNote => 'Nada cambia hasta que apliques.';

  @override
  String get rulesSecretItem => 'Elemento secreto';

  @override
  String rulesSecretItemFrom(String app) {
    return 'Elemento secreto de $app';
  }

  @override
  String get rulesPreviewCancelled =>
      'Se detuvo la vista previa. No cambió nada.';

  @override
  String get rulesPreviewFailed => 'No se pudo ejecutar la vista previa.';

  @override
  String rulesApplyDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '¿Eliminar $count clips?',
      one: '¿Eliminar 1 clip?',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyDeleteMessage =>
      'Los clips que coinciden se eliminan en todos tus dispositivos. No se puede deshacer.';

  @override
  String get rulesApplyDeleteConfirm => 'Eliminar y aplicar';

  @override
  String rulesApplyProgress(int done, int total) {
    return 'Aplicando: $done de $total';
  }

  @override
  String rulesApplyChanged(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips modificados',
      one: '1 clip modificado',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyDeleted(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clips eliminados',
      one: '1 clip eliminado',
    );
    return '$_temp0';
  }

  @override
  String rulesApplyFailed(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'No se pudieron modificar $count clips',
      one: 'No se pudo modificar 1 clip',
    );
    return '$_temp0';
  }

  @override
  String get rulesApplyCancelled =>
      'Detenido. Los cambios hechos hasta ahora se mantienen.';

  @override
  String get historyActionEdit => 'Editar';

  @override
  String get historyActionAddToQueue => 'Añadir a Paste Queue';

  @override
  String get historyActionSendTo => 'Enviar a…';

  @override
  String get historySendDialogTitle => 'Enviar al dispositivo';

  @override
  String get historySendDescription =>
      'Envía ahora, aunque la sincronización automática esté en pausa.';

  @override
  String get historySendNoDevices =>
      'Ningún dispositivo vinculado se ha sincronizado todavía.';

  @override
  String get historySendAction => 'Enviar';

  @override
  String get historySentTitle => 'Enviado';

  @override
  String historySentTo(String devices) {
    return 'Enviado a $devices.';
  }

  @override
  String get historySendFailedTitle => 'No enviado';

  @override
  String historySendBlocked(String device) {
    return 'Los ajustes de sincronización de $device no permiten este recorte.';
  }

  @override
  String historySendFailed(String device) {
    return '$device no pudo recibir el recorte. Comprueba que esté en línea y vuelve a intentarlo.';
  }

  @override
  String get historySendUnavailable => 'El envío no está disponible ahora.';

  @override
  String get historyActionTransform => 'Transformar…';

  @override
  String get historyActionExplain => '¿Por qué está aquí?';

  @override
  String get historyActionKeep => 'Conservar';

  @override
  String get historyNewClip => 'Nuevo recorte';

  @override
  String get historySavedTitle => 'Guardado';

  @override
  String get historyEditSaved =>
      'El texto editado es un nuevo recorte al principio del historial.';

  @override
  String get historyNewClipSaved => 'Se guardó tu texto.';

  @override
  String historyQueueAddedInOrder(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se añadieron $count recortes a la cola.',
      one: 'Se añadió 1 recorte a la cola.',
    );
    return '$_temp0';
  }

  @override
  String get historyQueueAddFailed =>
      'No se pudo añadir a Paste Queue. Comprueba que esté activada.';

  @override
  String get historyKeepFailed => 'No se pudo conservar este recorte.';

  @override
  String get historyExplainFailed => 'No se pudo cargar la explicación.';

  @override
  String get historyDeletesSoon => 'Se elimina en menos de un minuto';

  @override
  String historyDeletesInMinutes(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se elimina en $count min',
      one: 'Se elimina en 1 min',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInHours(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se elimina en $count h',
      one: 'Se elimina en 1 h',
    );
    return '$_temp0';
  }

  @override
  String historyDeletesInDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se elimina en $count días',
      one: 'Se elimina en 1 día',
    );
    return '$_temp0';
  }

  @override
  String get historyProvenanceManual => 'Manual';

  @override
  String get historyProvenanceEdited => 'Editado';

  @override
  String get historyProvenanceTransformed => 'Transformado';

  @override
  String get historyProvenanceModule => 'Módulo';

  @override
  String historyRuleTag(String name) {
    return '$name (auto)';
  }

  @override
  String get historyMetadataExpiry => 'Eliminación';

  @override
  String get historyMetadataRuleTags => 'Etiquetas automáticas';

  @override
  String modulesEnableTitle(String module) {
    return '¿Activar $module?';
  }

  @override
  String get modulesEnableAccess => 'Podrá:';

  @override
  String modulesCapability(String capability) {
    String _temp0 = intl.Intl.selectLogic(capability, {
      'history_read': 'Leer tu historial del portapapeles',
      'clipboard_write': 'Copiar en tu portapapeles',
      'library_write': 'Guardar fragmentos y fijar elementos',
      'snippets': 'Leer tus fragmentos',
      'synced_records': 'Sincronizar sus datos entre tus dispositivos',
      'local_state': 'Guardar sus datos en este dispositivo',
      'prompts': 'Pedirte que confirmes acciones',
      'endpoints': 'Aceptar conexiones de apps locales',
      'other': 'Usar un servicio de CopyPaste',
    });
    return '$_temp0';
  }

  @override
  String modulesPromptFrom(String module) {
    return 'De $module';
  }

  @override
  String get modulesUnknownModule => 'Módulo desconocido';

  @override
  String get modulesPromptAnswerFailed => 'Tu respuesta no se envió.';

  @override
  String get modulesPromptNotificationBody => 'Abre CopyPaste para responder.';

  @override
  String get modulesUnknownDevice => 'Dispositivo desconocido';

  @override
  String get modulesChooseDevices => 'Elegir dispositivos';

  @override
  String get modulesAddApp => 'Añadir app';

  @override
  String modulesRemoveApp(String app) {
    return 'Quitar $app';
  }

  @override
  String get modulesNoApps => 'No hay apps elegidas';

  @override
  String get settingsSectionDiagnostics => 'Diagnóstico';

  @override
  String get settingsSectionDiagnosticsSummary =>
      'Registros de ejecución para resolver problemas. El contenido del portapapeles nunca se registra.';

  @override
  String get diagnosticsSearchPlaceholder => 'Buscar en los registros';

  @override
  String get diagnosticsAllLevels => 'Todos los niveles';

  @override
  String get diagnosticsAllProcesses => 'Todos los procesos';

  @override
  String get diagnosticsLevelError => 'Error';

  @override
  String get diagnosticsLevelWarning => 'Advertencia';

  @override
  String get diagnosticsLevelInfo => 'Información';

  @override
  String get diagnosticsLevelDebug => 'Depuración';

  @override
  String get diagnosticsLevelTrace => 'Traza';

  @override
  String get diagnosticsProcessApp => 'Aplicación';

  @override
  String get diagnosticsProcessDaemon => 'Servicio en segundo plano';

  @override
  String get diagnosticsRefresh => 'Actualizar';

  @override
  String get diagnosticsExport => 'Exportar';

  @override
  String get diagnosticsExportFileType => 'Archivo de texto';

  @override
  String get diagnosticsExported => 'Diagnóstico exportado';

  @override
  String get diagnosticsExportedMessage =>
      'El archivo de registro está listo para enviarlo al soporte.';

  @override
  String get diagnosticsExportFailed => 'Error al exportar';

  @override
  String get diagnosticsExportFailedMessage =>
      'No se pudo guardar el archivo de registro.';

  @override
  String get diagnosticsLoadFailed => 'No se pudieron cargar los registros';

  @override
  String get diagnosticsEmpty => 'No hay entradas de registro';

  @override
  String get diagnosticsLogLevelTitle => 'Nivel de registro';

  @override
  String get diagnosticsLogLevelDescription =>
      'Cuánto detalle se registra en este dispositivo. Los registros nunca incluyen el contenido del portapapeles.';

  @override
  String get onboardingAndroidCaptureMethod => 'Método de detección';

  @override
  String get onboardingAndroidLogTrigger =>
      'Registro del sistema (recomendado)';

  @override
  String get onboardingAndroidLogTriggerDescription =>
      'Instantáneo, sin gasto extra de batería. Requiere acceso con Shizuku o ADB una vez.';

  @override
  String get onboardingAndroidPolling => 'Sondeo';

  @override
  String get onboardingAndroidPollingDescription =>
      'Comprueba cada segundo mientras la pantalla está encendida. Solo requiere acceso de superposición; gasta más batería.';

  @override
  String get onboardingAndroidOverlay => 'Mostrar sobre otras apps';

  @override
  String get onboardingAndroidOverlayDescription =>
      'Permite que una ventana invisible de 1 píxel lea el portapapeles.';

  @override
  String get onboardingAndroidOverlaySettingsFailed =>
      'No se pudieron abrir los ajustes de superposición.';

  @override
  String get settingsBackgroundSyncTitle => 'Sincronización en segundo plano';

  @override
  String get settingsBackgroundSyncDescription =>
      'Sigue sincronizando con la app cerrada. Muestra una notificación.';

  @override
  String get settingsBackgroundSyncKeywords =>
      'segundo plano persistente servicio notificación batería keep alive persistent service notification battery';

  @override
  String get settingsBackgroundSyncStoppedTitle =>
      'La sincronización en segundo plano no se está ejecutando';

  @override
  String get settingsBackgroundSyncStoppedMessage =>
      'Permite las notificaciones para que Android mantenga la sincronización.';

  @override
  String get settingsBackgroundSyncResume => 'Reanudar sincronización';

  @override
  String get quickAccessSendShortcutTitle =>
      'Acceso directo «Enviar portapapeles»';

  @override
  String get quickAccessSendShortcutDescription =>
      'Guarda el portapapeles actual y lo envía a tus dispositivos.';

  @override
  String get quickAccessTransformShortcutTitle =>
      'Acceso directo «Transformar portapapeles»';

  @override
  String get quickAccessTransformShortcutDescription =>
      'Abre las transformaciones del portapapeles actual.';

  @override
  String get quickAccessAddToHomeScreen => 'Añadir a la pantalla de inicio';

  @override
  String get quickAccessShortcutKeywords =>
      'acceso directo pantalla de inicio launcher icono shortcut home screen launcher icon';

  @override
  String fileSetCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count archivos',
      one: '$count archivo',
    );
    return '$_temp0';
  }

  @override
  String get fileSetShowFolder => 'Mostrar carpeta';

  @override
  String get historyFilesSaved => 'Archivos guardados.';

  @override
  String get transfersResume => 'Reanudar';

  @override
  String get settingsStartInBackgroundTitle => 'Iniciar en segundo plano';

  @override
  String get settingsStartInBackgroundDescription =>
      'Cuando CopyPaste se inicia al iniciar sesión, la ventana permanece cerrada.';

  @override
  String get settingsWindowGlassTitle => 'Efecto de cristal';

  @override
  String get settingsWindowGlassDescription =>
      'Desenfoca el escritorio detrás de la ventana. Se desactiva si el sistema reduce la transparencia.';

  @override
  String get settingsRestoreClipboardTitle =>
      'Restaurar el portapapeles al iniciar';

  @override
  String get settingsRestoreClipboardDescription =>
      'Al iniciar, pone el último recorte en un portapapeles vacío. Los recortes secretos se omiten.';

  @override
  String get settingsStorageUsageTitle => 'Uso del almacenamiento';

  @override
  String get settingsStorageUsageDescription =>
      'Espacio que CopyPaste usa en este dispositivo.';

  @override
  String get settingsStorageUsageUnavailable =>
      'No se pudo leer el uso del almacenamiento.';

  @override
  String get settingsStorageHistory => 'Historial';

  @override
  String get settingsStorageFiles => 'Archivos';

  @override
  String get settingsStorageCaches => 'Cachés';

  @override
  String get settingsStorageUpgradeBackups => 'Copias de actualización';

  @override
  String get settingsStorageOther => 'Registros, módulos y ajustes';

  @override
  String get settingsStorageTotal => 'Total';

  @override
  String get settingsClearCacheTitle => 'Borrar caché';

  @override
  String get settingsClearCacheDescription =>
      'Elimina vistas previas y archivos temporales. El historial se conserva.';

  @override
  String get settingsClearCacheAction => 'Borrar';

  @override
  String get settingsClearCacheDoneTitle => 'Caché borrada';

  @override
  String get settingsClearCacheDoneMessage =>
      'Las vistas previas se recrean cuando hacen falta.';

  @override
  String get settingsClearCacheFailed => 'No se pudo borrar la caché.';

  @override
  String get settingsDataFolderTitle => 'Carpeta de datos';

  @override
  String get settingsDataFolderDescription =>
      'Muestra dónde se guardan el historial, las copias y los módulos.';

  @override
  String get settingsLogsFolderTitle => 'Carpeta de registros';

  @override
  String get settingsLogsFolderDescription =>
      'Muestra los registros de diagnóstico de este dispositivo.';

  @override
  String get settingsOpenFolderAction => 'Abrir';

  @override
  String get settingsOpenFolderFailed => 'No se pudo abrir la carpeta.';

  @override
  String get settingsConfirmDeleteTitle => 'Confirmar antes de eliminar';

  @override
  String get settingsConfirmDeleteDescription =>
      'Preguntar antes de eliminar un recorte. Eliminar varios siempre pregunta.';

  @override
  String get settingsUpgradeBackupsTitle => 'Copias de actualización';

  @override
  String get settingsUpgradeBackupsDescription =>
      'Copias cifradas hechas antes de actualizar los datos. Se guardan las 5 últimas.';

  @override
  String get settingsUpgradeBackupsEmpty =>
      'Aún no hay copias de actualización.';

  @override
  String settingsUpgradeBackupEntry(String date, String size) {
    return '$date · $size';
  }

  @override
  String get settingsUpgradeBackupRestoreAction => 'Restaurar';

  @override
  String get settingsUpgradeBackupRestoreTitle => '¿Restaurar esta copia?';

  @override
  String settingsUpgradeBackupRestoreMessage(String date) {
    return 'El historial se sustituye por la copia del $date. Los recortes añadidos desde entonces se eliminan.';
  }

  @override
  String get settingsDownloadUpdatesTitle =>
      'Descargar actualizaciones en segundo plano';

  @override
  String get settingsDownloadUpdatesDescription =>
      'Descarga y verifica la actualización encontrada para instalarla al instante. Espera en redes de uso medido y en modo de bajo consumo.';

  @override
  String settingsUpdateDownloadedReady(String available) {
    return 'CopyPaste $available está descargado y listo para instalar.';
  }

  @override
  String get devicesSpaceTitle => 'Tu espacio';

  @override
  String devicesSpaceSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dispositivos',
      one: 'Solo este dispositivo',
    );
    return '$_temp0';
  }

  @override
  String get devicesSpaceKeyPending =>
      'Los dispositivos sin conexión recibirán la nueva clave cuando se conecten.';

  @override
  String get devicesSpaceWaitingForKey => 'Necesita clave nueva';

  @override
  String get devicesSpaceRemovedTitle => 'Sin espacio';

  @override
  String get devicesSpaceRemovedMessage =>
      'Este dispositivo se eliminó o salió. Su historial se queda aquí. Únete a un espacio para volver a sincronizar.';

  @override
  String get devicesSpaceJoin => 'Unirse a un espacio';

  @override
  String get devicesRemoveFromSpace => 'Quitar del espacio';

  @override
  String devicesRemoveDialogTitle(String device) {
    return '¿Quitar $device?';
  }

  @override
  String get devicesRemoveDialogMessage =>
      'Se quitará de todos tus dispositivos y no podrá leer nada sincronizado a partir de ahora.';

  @override
  String get devicesRemoveNewPassphrase =>
      'Nueva frase de sincronización (opcional)';

  @override
  String get devicesRemoveNewPassphraseHint =>
      'El dispositivo quitado conoce la frase actual. Sin una nueva, añade dispositivos solo por invitación.';

  @override
  String get devicesRemoveSignOutOthers =>
      'Cerrar sesión de la cuenta en la nube en otros dispositivos';

  @override
  String get devicesPassphraseTooShort => 'Usa al menos 12 caracteres.';

  @override
  String get devicesLeaveSpace => 'Salir del espacio';

  @override
  String get devicesLeaveDialogTitle => '¿Salir de este espacio?';

  @override
  String get devicesLeaveDialogMessage =>
      'Tus otros dispositivos quitarán este. Su historial se queda aquí.';

  @override
  String get devicesLeave => 'Salir';

  @override
  String get onboardingSpaceTitle => 'Tus dispositivos';

  @override
  String get onboardingSpaceSubtitle =>
      'Sincroniza el portapapeles entre tus propios dispositivos. Todo sigue cifrado.';

  @override
  String get onboardingSpaceCreateTitle => 'Crear un espacio';

  @override
  String get onboardingSpaceCreateDescription =>
      'Es mi primer dispositivo con CopyPaste.';

  @override
  String get onboardingSpaceJoinTitle => 'Unirse a un espacio';

  @override
  String get onboardingSpaceJoinDescription =>
      'Ya uso CopyPaste en otro dispositivo.';

  @override
  String get onboardingSpaceLater => 'Más tarde';

  @override
  String get onboardingReadySpaceCreated =>
      'Tu espacio está listo. Añade tus otros dispositivos ahora o más tarde en Dispositivos.';

  @override
  String get onboardingReadyJoin =>
      'Ahora confirma el código que aparece en tu otro dispositivo.';

  @override
  String get onboardingAddDeviceNow => 'Añadir dispositivo';

  @override
  String get onboardingJoinNow => 'Unirse ahora';

  @override
  String get onboardingBadgeWelcome => 'Bienvenida';

  @override
  String get onboardingBadgeSetup => 'Configuración';

  @override
  String get onboardingBadgeDevices => 'Dispositivos';

  @override
  String get onboardingBadgeReady => 'Listo';

  @override
  String get onboardingPairJoinTitle => 'Únete a tu espacio';

  @override
  String get onboardingPairJoinSubtitle =>
      'Introduce el código o escanea el código QR de tu otro dispositivo. Después comprueba que ambos dispositivos muestran el mismo código de verificación.';

  @override
  String get onboardingPairAddDeviceSubtitle =>
      'En tu otro dispositivo, abre CopyPaste y únete con este código QR o código. Después comprueba que ambos dispositivos muestran el mismo código de verificación.';

  @override
  String get onboardingPairSkip => 'Omitir por ahora';

  @override
  String get onboardingPairInterruptedTitle => 'La vinculación no terminó';

  @override
  String get onboardingPairInterruptedMessage =>
      'Inténtalo de nuevo o continúa y vincula más tarde en Dispositivos.';

  @override
  String onboardingReadyPaired(String device) {
    return 'Vinculado con $device. Tu portapapeles ya se sincroniza entre ambos.';
  }

  @override
  String get onboardingReadyPairedUnnamed =>
      'Tus dispositivos están vinculados. Tu portapapeles ya se sincroniza entre ellos.';

  @override
  String get onboardingReadyPairLater =>
      'Puedes vincular tus dispositivos en cualquier momento en Dispositivos.';

  @override
  String get settingsSectionShortcuts => 'Atajos';

  @override
  String get settingsSectionShortcutsSummary =>
      'Atajos de teclado globales. Desactiva los que no uses.';

  @override
  String get settingsShortcutEnabledLabel => 'Desactivar este atajo';

  @override
  String get settingsWelcomeTourTitle => 'Recorrido de bienvenida';

  @override
  String get settingsWelcomeTourDescription =>
      'Repite la configuración inicial.';

  @override
  String get settingsWelcomeTourAction => 'Mostrar de nuevo';

  @override
  String get modulesDetails => 'Detalles';

  @override
  String get modulesDetailsFeatures => 'Qué añade';

  @override
  String get modulesDetailsSearch =>
      'Busca recortes por significado con un modelo local.';

  @override
  String get modulesDetailsPermissions => 'Permisos';

  @override
  String get modulesDetailsPermissionsPending =>
      'Se muestran tras instalar. Los confirmas al activar el módulo.';

  @override
  String get modulesDetailsNoPermissions => 'No usa servicios de CopyPaste.';

  @override
  String get modulesDetailsInformation => 'Información';

  @override
  String get modulesDetailsVersion => 'Versión';

  @override
  String get modulesDetailsSize => 'Tamaño';

  @override
  String get modulesDetailsRequirements => 'Requisitos';

  @override
  String get modulesDetailsStatus => 'Estado';

  @override
  String get errorUnexpected => 'Algo salió mal. Inténtalo de nuevo.';

  @override
  String get errorRuntimeUnavailable =>
      'CopyPaste no se está ejecutando en segundo plano. Reinicia la app.';

  @override
  String get errorRuntimeStartFailed =>
      'CopyPaste no pudo iniciar su servicio en segundo plano. Reinicia la app.';

  @override
  String get errorRuntimeStarting =>
      'CopyPaste todavía se está iniciando. Inténtalo en un momento.';

  @override
  String get errorRuntimeTimeout =>
      'CopyPaste no respondió a tiempo. Inténtalo de nuevo.';

  @override
  String get errorRuntimeMismatch =>
      'CopyPaste se ha actualizado. Reinicia la app para terminar.';

  @override
  String get errorPeerUnreachable =>
      'No se pudo contactar con el otro dispositivo. Comprueba que esté conectado.';

  @override
  String get errorPeerVersion =>
      'El otro dispositivo usa una versión incompatible de CopyPaste. Actualiza ambos dispositivos.';

  @override
  String get errorRateLimited =>
      'Demasiados intentos. Espera un momento e inténtalo de nuevo.';

  @override
  String devicesSyncFailedWith(String devices) {
    return 'No se pudo sincronizar con $devices.';
  }

  @override
  String get transfersUnavailableTitle =>
      'No se pudieron cargar las transferencias';

  @override
  String get historyTagsLoadFailedTitle =>
      'No se pudieron cargar las etiquetas';

  @override
  String get settingsEraseAllDataTitle => 'Eliminar todos los datos';

  @override
  String get settingsEraseAllDataDescription =>
      'Elimina el historial, los ajustes y la clave de cifrado, y cierra CopyPaste.';

  @override
  String get settingsEraseAllDataAction => 'Eliminar todo';

  @override
  String get settingsEraseAllDataConfirmTitle =>
      '¿Eliminar todos los datos de CopyPaste?';

  @override
  String get settingsEraseAllDataConfirmMessage =>
      'Se eliminarán el historial, los archivos, los ajustes y la clave de cifrado de este dispositivo, y CopyPaste se cerrará. Los dispositivos vinculados y las copias en la nube se conservan. Esta acción no se puede deshacer.';

  @override
  String get settingsEraseAllDataFailed =>
      'No se pudieron eliminar algunos datos de CopyPaste.';
}
