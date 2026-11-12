// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Portuguese (`pt`).
class AppLocalizationsPt extends AppLocalizations {
  AppLocalizationsPt([String locale = 'pt']) : super(locale);

  @override
  String get commonCancel => 'Cancelar';

  @override
  String get commonClose => 'Fechar';

  @override
  String get commonDone => 'Concluído';

  @override
  String get commonRetry => 'Tentar novamente';

  @override
  String get commonTryAgain => 'Tentar novamente';

  @override
  String get commonCopy => 'Copiar';

  @override
  String get commonDelete => 'Excluir';

  @override
  String get commonSave => 'Salvar';

  @override
  String get commonContinue => 'Continuar';

  @override
  String get commonBack => 'Voltar';

  @override
  String get commonOpenSettings => 'Abrir configurações';

  @override
  String get commonLoading => 'Carregando';

  @override
  String get commonApply => 'Aplicar';

  @override
  String get commonClear => 'Limpar';

  @override
  String get commonPin => 'Fixar';

  @override
  String get commonEnable => 'Ativar';

  @override
  String get settingsLanguageTitle => 'Idioma';

  @override
  String get settingsLanguageDescription =>
      'Escolha o idioma da interface do CopyPaste.';

  @override
  String get settingsLanguageSystem => 'Padrão do sistema';

  @override
  String get settingsLanguageKeywords =>
      'idioma localidade tradução interface language locale translation interface';

  @override
  String get settingsRuntimeLaunchAtLoginUpdateFailed =>
      'Não foi possível atualizar a abertura ao iniciar sessão. Tente novamente.';

  @override
  String get settingsRuntimeStartupSettingsOpenFailed =>
      'Não foi possível abrir as configurações de inicialização do sistema.';

  @override
  String get settingsRuntimeRefreshFailed =>
      'Não foi possível atualizar as configurações. Tente novamente.';

  @override
  String get settingsRuntimeLoadFailed =>
      'Não foi possível carregar as configurações.';

  @override
  String get settingsRuntimeAppAlreadyExcluded =>
      'Esse app já está sendo ignorado.';

  @override
  String get settingsRuntimeChangeFailed =>
      'Não foi possível concluir a alteração solicitada.';

  @override
  String get settingsRuntimeCapturePermissionsIncomplete =>
      'Conceda todas as permissões de captura da área de transferência antes de retomar a captura.';

  @override
  String get settingsRuntimeCaptureStopUnconfirmed =>
      'A captura da área de transferência está pausada. O serviço de captura do Android não confirmou que parou.';

  @override
  String get settingsRuntimeNotificationPermissionRequired =>
      'É necessária a permissão de notificações para ativar as notificações de captura.';

  @override
  String get settingsRuntimeSavedSettingsUnreadable =>
      'O CopyPaste não conseguiu ler as configurações salvas. Valores de proteção estão ativos; revise e salve suas preferências para corrigi-las.';

  @override
  String get settingsRuntimeSettingsRecovered =>
      'O CopyPaste recuperou configurações ilegíveis com valores de proteção. Revise e salve suas preferências para corrigi-las.';

  @override
  String get settingsRuntimeExcludedAppIdInvalid =>
      'Os identificadores de app não podem estar vazios e devem ter no máximo 256 bytes.';

  @override
  String get settingsRuntimeExcludedAppsLimit =>
      'Você pode ignorar até 256 apps.';

  @override
  String get settingsRuntimeAutoPasteUpdateFailed =>
      'Não foi possível atualizar a colagem automática.';

  @override
  String get settingsRuntimeShortcutRegisterFailed =>
      'Não foi possível registrar esse atalho.';

  @override
  String get settingsRuntimeShortcutPauseFailed =>
      'Não foi possível pausar o atalho atual.';

  @override
  String get settingsRuntimeShortcutRestoreFailed =>
      'Não foi possível restaurar o atalho.';

  @override
  String get settingsRuntimeQuickPasteOpenFailed =>
      'Não foi possível abrir o Quick Paste.';

  @override
  String get settingsRuntimeAccessibilityRequestFailed =>
      'Não foi possível solicitar a permissão de Acessibilidade.';

  @override
  String get settingsRuntimeLinuxIntegrationRefreshFailed =>
      'Não foi possível atualizar o status da integração com o Linux.';

  @override
  String get settingsRuntimeClipboardInputNotGranted =>
      'A permissão de entrada da área de transferência não foi concedida.';

  @override
  String get settingsRuntimeClipboardInputRequestFailed =>
      'Não foi possível solicitar a permissão de entrada da área de transferência.';

  @override
  String get settingsRuntimeDesktopIntegrationSetupOpenFailed =>
      'Não foi possível abrir a configuração da integração com a área de trabalho.';

  @override
  String get settingsRuntimeTextHistoryFileType =>
      'Histórico de texto do CopyPaste';

  @override
  String get settingsRuntimeEncryptedBackupFileType =>
      'Backup criptografado do CopyPaste';

  @override
  String get settingsRuntimeNotificationImage => 'Imagem';

  @override
  String settingsRuntimeNotificationImageSize(int width, int height) {
    return 'Imagem · $width × $height';
  }

  @override
  String get settingsRuntimeNotificationFile => 'Arquivo';

  @override
  String get updateRestoreInstallationFailed =>
      'O CopyPaste não pôde restaurar a instalação da atualização.';

  @override
  String get updateLinuxAutomaticUnsupported =>
      'Esta instalação do Linux não pode ser atualizada automaticamente.';

  @override
  String get updateMacosPackageMissing =>
      'Esta versão não tem um pacote de atualização assinado para macOS.';

  @override
  String get updateInstalledVersionInvalid =>
      'A versão instalada do CopyPaste é inválida.';

  @override
  String get updateCheckUnreachable =>
      'O CopyPaste não conseguiu acessar o GitHub para verificar atualizações.';

  @override
  String get updateCheckTimedOut => 'A verificação de atualizações expirou.';

  @override
  String get updateCheckDeviceFailed =>
      'O CopyPaste não pôde verificar atualizações neste dispositivo.';

  @override
  String get updateCheckFailed =>
      'O CopyPaste não pôde verificar atualizações.';

  @override
  String get updateDownloadFailed =>
      'O CopyPaste não pôde baixar a atualização.';

  @override
  String get updateDownloadTimedOut => 'O download da atualização expirou.';

  @override
  String get updateInstallFailed =>
      'O CopyPaste não pôde instalar a atualização.';

  @override
  String get updateContinueInAndroidInstaller =>
      'Continue no instalador do sistema Android.';

  @override
  String get updateContinueInPackageManager =>
      'Continue no gerenciador de pacotes do sistema.';

  @override
  String get updateInstallerStarting => 'O instalador está sendo iniciado.';

  @override
  String get updatePermissionRequiredAndroid =>
      'Permita que o CopyPaste instale apps nas configurações do Android e continue.';

  @override
  String get updatePermissionRequiredLinux =>
      'Autentique-se no gerenciador de pacotes do sistema para instalar esta atualização.';

  @override
  String get updatePermissionRequired =>
      'Permita que o CopyPaste instale esta atualização e continue.';

  @override
  String get updateRestartRequired =>
      'A atualização foi instalada. Reinicie o CopyPaste para usá-la.';

  @override
  String get updateRestarting => 'Reiniciando o CopyPaste.';

  @override
  String get updateRestartFailed =>
      'O CopyPaste não pôde reiniciar. Tente novamente.';

  @override
  String get updateReleasePageOpenFailed =>
      'O CopyPaste não pôde abrir a página da versão.';

  @override
  String get updatePackageInvalid =>
      'A atualização baixada não é um pacote válido do CopyPaste.';

  @override
  String get updateSignatureInvalid =>
      'A atualização baixada tem uma assinatura inválida.';

  @override
  String get updateDowngradeRefused =>
      'O pacote baixado não é mais recente que esta versão.';

  @override
  String get updateHomebrewRequired =>
      'Instale o CopyPaste com o Homebrew para atualizá-lo aqui.';

  @override
  String get updateInstallationCancelled =>
      'A instalação da atualização foi cancelada.';

  @override
  String get updateInstallationBlocked =>
      'O Android bloqueou a instalação da atualização.';

  @override
  String get updateInstallationConflict =>
      'A atualização entra em conflito com o app instalado.';

  @override
  String get updateInstallationIncompatible =>
      'A atualização é incompatível com este dispositivo.';

  @override
  String get updateInstallationStorage =>
      'Libere espaço de armazenamento para instalar a atualização.';

  @override
  String get updateInstallationInterrupted =>
      'A instalação da atualização foi interrompida. Tente novamente.';

  @override
  String get updateInstallationBusy =>
      'Já há uma instalação de atualização em andamento.';

  @override
  String get updatePackageDetailsInvalid =>
      'Os detalhes do pacote de atualização são inválidos.';

  @override
  String get updateIntegrityCheckFailed =>
      'A atualização baixada não passou na verificação de integridade.';

  @override
  String get updateInstallerLaunchFailed =>
      'O CopyPaste não pôde iniciar o instalador do sistema.';

  @override
  String get updateInstallLocationTranslocated =>
      'Mova o CopyPaste para a pasta Aplicativos e abra-o de lá para atualizar.';

  @override
  String get updateInstallLocationUnwritable =>
      'O CopyPaste não pode se substituir na pasta atual. Use a página da versão para atualizar.';

  @override
  String get updateReplacementBlocked =>
      'O macOS bloqueou a substituição do CopyPaste. Permita isso em Gerenciamento de Apps nos Ajustes do Sistema ou use a página da versão.';

  @override
  String get updatePackageNotDownloadable =>
      'Esta plataforma não usa um pacote de atualização para download.';

  @override
  String get updatePackageSizeInvalid =>
      'O tamanho do pacote de atualização é inválido.';

  @override
  String get updateUpdaterSignatureInvalid =>
      'A atualização baixada tem uma assinatura de atualizador inválida.';

  @override
  String get updatePackageUrlUntrusted =>
      'O URL do pacote de atualização não é confiável.';

  @override
  String get updateDownloadLeftGitHub =>
      'O download da atualização saiu do GitHub.';

  @override
  String get updateDownloadTooManyRedirects =>
      'O download da atualização usou redirecionamentos demais.';

  @override
  String get updatePackageTooLarge =>
      'O pacote de atualização é grande demais.';

  @override
  String get updateSignatureSizeInvalid =>
      'O tamanho da assinatura do atualizador é inválido.';

  @override
  String get updateMetadataInvalid =>
      'O GitHub retornou metadados de atualização inválidos.';

  @override
  String get updateMetadataTooLarge =>
      'Os metadados da atualização são grandes demais.';

  @override
  String get quickPasteActionFailedTitle => 'Falha na ação';

  @override
  String get quickPasteOpenAppSemantics => 'Abrir o CopyPaste';

  @override
  String get quickPasteSearchPlaceholder => 'Digite para buscar…';

  @override
  String get quickPasteLoadingHistory =>
      'Carregando o histórico da área de transferência.';

  @override
  String get quickPasteHistoryUnavailable => 'O histórico não está disponível';

  @override
  String get quickPasteNoClipsFound => 'Nenhum clipe encontrado';

  @override
  String get quickPasteLoadMore => 'Carregar mais';

  @override
  String get quickPasteTransferFailed => 'Falhou';

  @override
  String get quickPasteClipUnavailable => 'Clipe indisponível';

  @override
  String get quickPasteFileFallbackName => 'Arquivo';

  @override
  String get quickPastePathUnavailable => 'Caminho indisponível';

  @override
  String get quickPastePreferences => 'Preferências…';

  @override
  String get quickPasteAbout => 'Sobre';

  @override
  String get quickPasteQuit => 'Encerrar';

  @override
  String get quickPasteClearHistoryTitle => 'Limpar o histórico?';

  @override
  String get quickPasteClearHistoryMessage =>
      'Os clipes fixados continuarão no histórico.';

  @override
  String get quickPasteClearHistoryFailed =>
      'Não foi possível excluir o histórico.';

  @override
  String get quickPasteAboutDescription =>
      'Histórico privado da área de transferência no macOS, Android, Windows e Linux.';

  @override
  String get transfersTitle => 'Downloads e transferências';

  @override
  String transfersActiveTooltip(int count, String written, String total) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count transferências ativas',
      many: '$count transferências ativas',
      one: '$count transferência ativa',
    );
    return '$_temp0 · $written / $total';
  }

  @override
  String transfersActiveCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ativas',
      many: '$count ativas',
      one: '$count ativa',
    );
    return '$_temp0';
  }

  @override
  String get transfersShowLatest => 'Mais recentes';

  @override
  String get transfersShowEarlier => 'Anteriores';

  @override
  String get transfersClearCompletedHistory =>
      'Limpar transferências concluídas';

  @override
  String get transfersErrorTitle => 'Transferências';

  @override
  String get transfersEmptyTitle => 'Nenhuma transferência';

  @override
  String get transfersEmptyMessage =>
      'Importações de arquivos, downloads e transferências enviadas aparecem aqui.';

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
  String get transfersRemoveFromHistory => 'Remover do histórico';

  @override
  String get transfersCancelDownload => 'Cancelar download';

  @override
  String get transfersShowFile => 'Mostrar arquivo';

  @override
  String get transfersPhaseSending => 'Enviando';

  @override
  String get transfersPhaseDownloading => 'Baixando';

  @override
  String get transfersPhaseAdding => 'Adicionando';

  @override
  String get transfersPhaseAdded => 'Adicionado';

  @override
  String get transfersPhaseSent => 'Enviado';

  @override
  String get transfersPhaseCompleted => 'Concluído';

  @override
  String get transfersPhaseCancelled => 'Cancelado';

  @override
  String get transfersPhaseInterrupted => 'Interrompido';

  @override
  String get transfersPhaseFailed => 'Falhou';

  @override
  String get transfersUpdatesUnavailable =>
      'As atualizações das transferências não estão disponíveis. Reabra para atualizar.';

  @override
  String get transfersLoadFailed =>
      'Não foi possível carregar as transferências. Tente novamente.';

  @override
  String get transfersUpdatesLoadFailed =>
      'Não foi possível carregar o estado das transferências.';

  @override
  String get transfersRetryFailed =>
      'Não foi possível tentar novamente este arquivo.';

  @override
  String get transfersClearHistoryFailed =>
      'Não foi possível limpar o histórico de transferências.';

  @override
  String get transfersRemoveRecordFailed =>
      'Não foi possível remover este registro do histórico.';

  @override
  String get transfersCancelFailed =>
      'Não foi possível cancelar a transferência. Ela ainda pode estar em andamento.';

  @override
  String get transfersSavedFileUnavailable =>
      'O arquivo salvo não está disponível. Ele pode ter sido movido ou excluído.';

  @override
  String notificationsTooltip(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Notificações · $count não lidas',
      many: 'Notificações · $count não lidas',
      one: 'Notificações · $count não lida',
    );
    return '$_temp0';
  }

  @override
  String get notificationsTitle => 'Notificações';

  @override
  String get notificationsClearRead => 'Limpar notificações lidas';

  @override
  String get notificationsEmptyTitle => 'Nenhuma notificação';

  @override
  String get notificationsEmptyMessage =>
      'Solicitações de conexão de dispositivos e seus resultados aparecem aqui.';

  @override
  String get notificationsSystemAlertTitle => 'Notificações do sistema';

  @override
  String get notificationsReview => 'Revisar';

  @override
  String get notificationsConnectionRequest => 'Solicitação de conexão';

  @override
  String get notificationsConfirmConnection => 'Confirmar conexão';

  @override
  String get notificationsDeviceConnected => 'Dispositivo conectado';

  @override
  String get notificationsConnectionRejected => 'Conexão recusada';

  @override
  String get notificationsConnectionCancelled => 'Conexão cancelada';

  @override
  String get notificationsConnectionRequestExpired =>
      'Solicitação de conexão expirada';

  @override
  String get notificationsConnectionFailed => 'Falha na conexão';

  @override
  String get notificationsNearbyDevice => 'Dispositivo próximo';

  @override
  String get notificationsPermissionDenied =>
      'Permita as notificações do sistema nas configurações do sistema operacional para receber solicitações em segundo plano.';

  @override
  String get notificationsSystemUnavailable =>
      'As notificações do sistema não estão disponíveis. As solicitações de conexão continuam aqui.';

  @override
  String get platformCaptureStateUnavailable => 'Indisponível';

  @override
  String get platformCaptureStatePaused => 'Pausada';

  @override
  String get platformCaptureStateRunning => 'Capturando';

  @override
  String get platformCaptureStateStopped => 'Parada';

  @override
  String get platformCaptureActionUnavailable => 'Captura indisponível';

  @override
  String get platformCaptureActionResume => 'Retomar captura';

  @override
  String get platformCaptureActionPause => 'Pausar captura';

  @override
  String get platformCaptureActionResumeStopped => 'Retomar captura (parada)';

  @override
  String get platformTrayOpen => 'Abrir';

  @override
  String get platformTraySettings => 'Configurações';

  @override
  String get platformTrayQuit => 'Encerrar';

  @override
  String get platformWindowControlsUnavailable =>
      'Os controles de janela da área de trabalho não estão disponíveis. O CopyPaste continuará visível.';

  @override
  String get platformTrayUnavailable =>
      'A bandeja do sistema não está disponível. Mantenha o CopyPaste aberto e tente novamente ou encerre.';

  @override
  String get platformQuitFailed =>
      'Não foi possível encerrar o CopyPaste. Tente novamente.';

  @override
  String get platformVisibilityChangeFailed =>
      'Não foi possível alterar a visibilidade do app. Tente novamente.';

  @override
  String get platformCaptureNotificationTitle => 'Área de transferência salva';

  @override
  String get platformCaptureNotificationBody =>
      'Um novo item foi adicionado ao CopyPaste.';

  @override
  String get platformPairingNotificationTitle => 'Solicitação de conexão';

  @override
  String platformPairingNotificationBody(String peerName) {
    return '$peerName quer se conectar. Abra o CopyPaste para comparar o código e confirmar.';
  }

  @override
  String get platformPairingNotificationChannelName =>
      'Solicitações de conexão de dispositivos';

  @override
  String get platformPairingNotificationChannelDescription =>
      'Solicitações para conectar dispositivos próximos';

  @override
  String get platformNotificationDefaultAction => 'Abrir';

  @override
  String get platformFilesSelectionFailed =>
      'Não foi possível selecionar os arquivos.';

  @override
  String get platformFilesOnlyRegular =>
      'Somente arquivos comuns podem ser importados.';

  @override
  String get platformModulesUnavailable =>
      'Os módulos não estão disponíveis neste dispositivo.';

  @override
  String get platformSmsAccessConfigureFailed =>
      'Não foi possível configurar o acesso a SMS.';

  @override
  String get platformSmsAccessVerifyFailed =>
      'Não foi possível verificar o acesso a SMS.';

  @override
  String get platformUpdatesUnavailableHere =>
      'As atualizações do app não estão disponíveis aqui.';

  @override
  String get platformUpdateVersionUnavailable =>
      'A versão instalada não está disponível.';

  @override
  String get platformUpdatesUnavailableInstallation =>
      'As atualizações do app não estão disponíveis nesta instalação.';

  @override
  String get platformUpdateLinuxUnsupported =>
      'Esta instalação do Linux não pode ser atualizada automaticamente.';

  @override
  String get platformUpdateLinuxArchitectureMismatch =>
      'Esta atualização do Linux não corresponde à arquitetura em execução.';

  @override
  String get platformUpdateMacosUnsupported =>
      'Esta instalação do macOS não pode ser atualizada automaticamente.';

  @override
  String get platformUpdateStartFailed =>
      'O CopyPaste não pôde iniciar a atualização.';

  @override
  String get platformGlobalShortcutDescription => 'Abrir o Quick Paste';

  @override
  String get settingsSectionGeneral => 'Geral';

  @override
  String get settingsSectionGeneralSummary => 'Idioma e visibilidade do app.';

  @override
  String get settingsSectionClipboard => 'Área de transferência';

  @override
  String get settingsSectionClipboardSummary =>
      'Captura e limites do histórico.';

  @override
  String get settingsSectionPrivacy => 'Privacidade';

  @override
  String get settingsSectionPrivacySummary =>
      'Apps ignorados e proteção da tela.';

  @override
  String get settingsSectionQuickPasteSummary =>
      'Atalho e comportamento da colagem automática.';

  @override
  String get settingsSectionSync => 'Sincronização';

  @override
  String get settingsSectionSyncSummary =>
      'Sincronização e descoberta de dispositivos próximos.';

  @override
  String get settingsSectionNotifications => 'Notificações';

  @override
  String get settingsSectionNotificationsSummary =>
      'Notificações e sons de captura.';

  @override
  String get settingsSectionData => 'Dados';

  @override
  String get settingsSectionDataSummary => 'Exportação, backup e restauração.';

  @override
  String get settingsSectionModules => 'Módulos';

  @override
  String get settingsSectionModulesSummary =>
      'Instale e gerencie módulos opcionais.';

  @override
  String get settingsSectionRules => 'Regras de conteúdo';

  @override
  String get settingsSectionRulesSummary =>
      'Marque, ignore, proteja ou expire itens automaticamente.';

  @override
  String get settingsSectionQuickAccess => 'Acesso rápido';

  @override
  String get settingsSectionQuickAccessSummary =>
      'Teclado, blocos e widget da tela inicial.';

  @override
  String get settingsSectionSnippets => 'Trechos';

  @override
  String get settingsSectionSnippetsSummary =>
      'Textos e modelos reutilizáveis.';

  @override
  String get settingsSectionTransforms => 'Transformações de texto';

  @override
  String get settingsSectionTransformsSummary =>
      'Favoritos, sequências e transformações padrão ao colar.';

  @override
  String get settingsSectionAiTools => 'Ferramentas de IA';

  @override
  String get settingsSectionAiToolsSummary =>
      'Conecte assistentes de IA e controle o que eles podem ver.';

  @override
  String get pasteQueueSettingsTitle => 'Paste Queue';

  @override
  String get pasteQueueSettingsSummary =>
      'Reúna várias cópias e cole-as em ordem.';

  @override
  String pasteQueuePasted(int position, int total) {
    return 'Colado $position de $total';
  }

  @override
  String pasteQueueCopied(int position, int total) {
    return 'O item $position de $total está na área de transferência';
  }

  @override
  String pasteQueuePastedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count itens colados',
      one: '1 item colado',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueCopiedAll(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count itens estão na área de transferência',
      one: '1 item está na área de transferência',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueFinished => 'Fila concluída';

  @override
  String pasteQueueAdded(int total) {
    String _temp0 = intl.Intl.pluralLogic(
      total,
      locale: localeName,
      other: 'Adicionado, $total na fila',
      one: 'Adicionado, 1 na fila',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueNothingToPaste => 'Fila vazia, nada foi colado';

  @override
  String get pasteQueueMixed =>
      'Colar tudo funciona com filas só de texto ou só de arquivos';

  @override
  String get pasteQueueSeveralFiles =>
      'Ainda não é possível colar vários arquivos de uma vez. Cole-os um por um.';

  @override
  String pasteQueueUnavailable(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count itens não estão mais disponíveis',
      one: '1 item não está mais disponível',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCopyOnlyTitle => 'Colar próximo apenas copia';

  @override
  String get pasteQueueAccessibilityMessage =>
      'Permita Acessibilidade para colar automaticamente. Até lá, o item fica na área de transferência para você colar.';

  @override
  String get pasteQueuePasteUnsupported =>
      'Não foi possível colar automaticamente. O item está na área de transferência; cole-o com o atalho habitual.';

  @override
  String get pasteQueueTargetElevated =>
      'Não foi possível colar em um app executado como administrador. O item continua na fila e está na área de transferência.';

  @override
  String get pasteQueueNoTarget =>
      'Nenhum app para colar. O item continua na fila e está na área de transferência.';

  @override
  String get pasteQueueCollectingStarted => 'Coletando cópias';

  @override
  String get pasteQueueCollectingStopped => 'Coleta interrompida';

  @override
  String get pasteQueueFailed =>
      'Não foi possível concluir a ação da Paste Queue';

  @override
  String pasteQueueSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pendentes',
      one: '1 pendente',
    );
    return '$_temp0';
  }

  @override
  String pasteQueueSummaryCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pendentes · Coletando',
      one: '1 pendente · Coletando',
    );
    return '$_temp0';
  }

  @override
  String get pasteQueueCollectingLabel => 'Coletando';

  @override
  String get pasteQueueFinishedSummary => 'Concluída';

  @override
  String pasteQueueFull(int max) {
    return 'Fila cheia ($max)';
  }

  @override
  String get pasteQueueFullMessage =>
      'Novas cópias não são mais adicionadas. Nada foi removido.';

  @override
  String get pasteQueueEmptyTitle => 'A Paste Queue está vazia';

  @override
  String get pasteQueueEmptyMessage =>
      'Colete cópias para colá-las em ordem ou adicione itens do histórico.';

  @override
  String get pasteQueueCollectingEmptyTitle => 'Copie itens para adicioná-los';

  @override
  String get pasteQueueCollectingEmptyMessage =>
      'Cada cópia entra na fila até você parar a coleta.';

  @override
  String get pasteQueueStartCollecting => 'Começar a coletar';

  @override
  String get pasteQueueStopCollecting => 'Parar de coletar';

  @override
  String get pasteQueueLoading => 'Carregando a fila…';

  @override
  String get pasteQueueLoadFailed => 'Não foi possível carregar a fila';

  @override
  String get pasteQueuePasteNext => 'Colar próximo';

  @override
  String get pasteQueueCopyNext => 'Copiar próximo';

  @override
  String get pasteQueuePasteAll => 'Colar tudo';

  @override
  String get pasteQueueCopyAll => 'Copiar tudo';

  @override
  String get pasteQueueReverse => 'Inverter ordem';

  @override
  String get pasteQueueStepBack => 'Voltar um passo';

  @override
  String get pasteQueueSkip => 'Pular';

  @override
  String get pasteQueueRemovePasted => 'Remover colados';

  @override
  String get pasteQueueCleared => 'Fila limpa';

  @override
  String get pasteQueueUndo => 'Desfazer';

  @override
  String get pasteQueueStartFresh => 'Começar do zero';

  @override
  String get pasteQueueAppend => 'Acrescentar';

  @override
  String get pasteQueueRestartTitle => 'Começar uma nova coleta?';

  @override
  String get pasteQueueRestartMessage =>
      'Começar do zero remove os itens colados. Acrescentar os mantém.';

  @override
  String pasteQueueStale(String time) {
    return 'Fila de $time';
  }

  @override
  String get pasteQueueStatusPasted => 'Colado';

  @override
  String get pasteQueueStatusSkipped => 'Pulado';

  @override
  String get pasteQueueStatusUnavailable => 'Não disponível';

  @override
  String get pasteQueueEdited => 'Editado';

  @override
  String get pasteQueueSecret => 'Secreto';

  @override
  String get pasteQueueKindImage => 'Imagem';

  @override
  String get pasteQueueKindFiles => 'Arquivos';

  @override
  String pasteQueueRenderedAt(String time) {
    return 'Gerado às $time';
  }

  @override
  String pasteQueueRowLabel(String position, String content) {
    return 'Item $position: $content';
  }

  @override
  String pasteQueueMoved(int position) {
    return 'Movido para a posição $position';
  }

  @override
  String get pasteQueueMoveUp => 'Mover para cima';

  @override
  String get pasteQueueMoveDown => 'Mover para baixo';

  @override
  String get pasteQueueMoveToTop => 'Mover para o início';

  @override
  String get pasteQueueMoveToBottom => 'Mover para o fim';

  @override
  String get pasteQueueReorder => 'Arraste para reordenar';

  @override
  String get pasteQueueMoreActions => 'Mais ações';

  @override
  String get pasteQueuePasteNow => 'Colar agora';

  @override
  String get pasteQueueEdit => 'Editar texto';

  @override
  String get pasteQueueUseOriginal => 'Usar o texto original';

  @override
  String get pasteQueueRemove => 'Remover da fila';

  @override
  String get pasteQueueEditTitle => 'Editar texto da fila';

  @override
  String get pasteQueueEditMessage =>
      'As alterações valem só para esta fila. O histórico mantém o original.';

  @override
  String get pasteQueueShortcutPromptTitle => 'Definir atalhos';

  @override
  String get pasteQueueShortcutPromptMessage =>
      'Escolha atalhos para coletar e colar sem abrir o CopyPaste.';

  @override
  String get pasteQueueShortcutToggleCollecting => 'Iniciar ou parar a coleta';

  @override
  String get pasteQueueShortcutPasteNextPlain =>
      'Colar próximo como texto simples';

  @override
  String get pasteQueueShortcutDescription =>
      'Um atalho global que funciona em qualquer app.';

  @override
  String get pasteQueueShortcutHeldByApp =>
      'Outro atalho do CopyPaste usa esta combinação. O anterior foi mantido.';

  @override
  String get pasteQueueShortcutInUse =>
      'Outro app usa esta combinação. A anterior foi mantida; escolha outra.';

  @override
  String get pasteQueueShortcutRejected =>
      'O sistema recusou esta combinação. A anterior foi mantida.';

  @override
  String get pasteQueueShortcutAltGr =>
      'Ctrl+Alt digita caracteres em muitos layouts de teclado (AltGr). Escolha outra combinação.';

  @override
  String get pasteQueueShortcutUnavailable =>
      'Atalhos globais não estão disponíveis nesta sessão. Use a fila pela janela ou pelo Quick Paste.';

  @override
  String get pasteQueueShortcutFailed =>
      'Não foi possível definir este atalho.';

  @override
  String get pasteQueueEnableTitle => 'Ativar a Paste Queue';

  @override
  String get pasteQueueEnableDescription =>
      'Desativada, os atalhos da fila são liberados e nada é coletado.';

  @override
  String get pasteQueuePasteNextStyleTitle => 'Colar próximo faz';

  @override
  String get pasteQueuePasteNextStyleDescription =>
      'Colar no app em primeiro plano requer permissão para enviar teclas. Só área de transferência deixa a colagem com você.';

  @override
  String get pasteQueuePasteIntoApp => 'Colar no app em primeiro plano';

  @override
  String get pasteQueueClipboardOnly => 'Só na área de transferência';

  @override
  String get pasteQueueInsertTitle => 'Novos itens vão para';

  @override
  String get pasteQueueInsertDescription =>
      'Onde os itens coletados e adicionados entram na fila.';

  @override
  String get pasteQueueInsertEnd => 'Fim (primeiro a entrar, primeiro a sair)';

  @override
  String get pasteQueueInsertStart =>
      'Início (último a entrar, primeiro a sair)';

  @override
  String get pasteQueueEndBehaviorTitle => 'Depois do último item';

  @override
  String get pasteQueueEndBehaviorDescription =>
      'O que Colar próximo faz depois que todos os itens foram colados.';

  @override
  String get pasteQueueEndStop => 'Parar';

  @override
  String get pasteQueueEndLoop => 'Recomeçar';

  @override
  String get pasteQueueEndKeepLast => 'Continuar colando o último';

  @override
  String get pasteQueueSeparatorTitle => 'Separador de Colar tudo';

  @override
  String get pasteQueueSeparatorDescription =>
      'Colocado entre os itens ao colar todos de uma vez.';

  @override
  String get pasteQueueSeparatorNewLine => 'Nova linha';

  @override
  String get pasteQueueSeparatorBlankLine => 'Linha em branco';

  @override
  String get pasteQueueSeparatorTab => 'Tabulação';

  @override
  String get pasteQueueSeparatorSpace => 'Espaço';

  @override
  String get pasteQueueSeparatorComma => 'Vírgula';

  @override
  String get pasteQueueSeparatorCommaSpace => 'Vírgula e espaço';

  @override
  String get pasteQueueSeparatorNone => 'Nenhum';

  @override
  String get pasteQueueSeparatorCustom => 'Personalizado';

  @override
  String get pasteQueueCustomSeparatorPlaceholder =>
      'Até 32 caracteres; \\n e \\t permitidos';

  @override
  String get pasteQueuePlainTextTitle => 'Sempre colar como texto simples';

  @override
  String get pasteQueuePlainTextDescription =>
      'Remove a formatação dos itens da fila.';

  @override
  String get pasteQueueIdleStopTitle => 'Parar de coletar após';

  @override
  String get pasteQueueIdleStopDescription =>
      'A coleta é desligada quando nada é copiado por esse tempo. Os itens continuam.';

  @override
  String pasteQueueIdleMinutes(int minutes) {
    return '$minutes min';
  }

  @override
  String get pasteQueueIdleNever => 'Nunca';

  @override
  String get pasteQueueMaxEntriesTitle => 'Limite de tamanho da fila';

  @override
  String get pasteQueueMaxEntriesDescription =>
      'Quando cheia, nenhum item é adicionado. Nada é descartado.';

  @override
  String get pasteQueueRestoreTitle =>
      'Restaurar a área de transferência anterior ao terminar';

  @override
  String get pasteQueueRestoreDescription =>
      'Devolve o que você tinha copiado antes de a fila começar a colar.';

  @override
  String get pasteQueueSettingsKeywords =>
      'paste queue fila coletar ordem atalho separador';

  @override
  String get settingsSectionAbout => 'Sobre';

  @override
  String get settingsSectionAboutSummary => 'Versão e atualizações do app.';

  @override
  String get settingsGeneralDescription =>
      'Idioma, inicialização automática e visibilidade do app.';

  @override
  String get settingsModulesDescription =>
      'Recursos opcionais para o CopyPaste.';

  @override
  String get settingsClipboardDescription =>
      'Capture as alterações da área de transferência e gerencie os limites do histórico.';

  @override
  String get settingsDataDescription =>
      'Exporte, faça backup e restaure o histórico da área de transferência.';

  @override
  String get settingsSyncDescription =>
      'Controle a sincronização com dispositivos pareados.';

  @override
  String get settingsPrivacyDescription =>
      'Controle quais apps são capturados e proteja o conteúdo da área de transferência.';

  @override
  String get settingsNotificationsDescription =>
      'Escolha notificações e sons para os clipes capturados.';

  @override
  String get settingsAboutDescription =>
      'Versão do CopyPaste e atualizações do app.';

  @override
  String get settingsQuickPasteDescription =>
      'Abra o histórico da área de transferência de qualquer lugar sem trocar de janela.';

  @override
  String get settingsLoadingMessage => 'Carregando configurações.';

  @override
  String get settingsUnavailableTitle =>
      'As configurações não estão disponíveis';

  @override
  String get settingsNoResultsTitle => 'Nenhuma configuração encontrada';

  @override
  String get settingsNoResultsMessage => 'Tente outra busca.';

  @override
  String get settingsAttentionTitle => 'As configurações precisam de atenção';

  @override
  String get settingsSearchPlaceholder => 'Buscar configurações';

  @override
  String get settingsBackToSettings => 'Voltar para Configurações';

  @override
  String get settingsStartOnBootTitle => 'Iniciar com o dispositivo';

  @override
  String get settingsStartAtLoginTitle => 'Abrir ao iniciar sessão';

  @override
  String get settingsStartupSearchDescription =>
      'Iniciar o CopyPaste automaticamente.';

  @override
  String get settingsStartupKeywords =>
      'inicialização sessão iniciar abrir automaticamente startup login boot launch automatically autostart';

  @override
  String get settingsStartupWindowsApproval =>
      'Desativado no Gerenciador de Tarefas. Ative o CopyPaste em Configurações → Aplicativos → Inicialização.';

  @override
  String get settingsStartupMacApproval =>
      'Permita o CopyPaste em Ajustes do Sistema → Geral → Itens de Início.';

  @override
  String get settingsStartupUnavailable =>
      'A inicialização automática não está disponível nesta instalação.';

  @override
  String get settingsStartupAndroidDescription =>
      'Retomar os serviços em segundo plano ativados após reiniciar o dispositivo.';

  @override
  String get settingsStartupDesktopDescription =>
      'Abrir o CopyPaste automaticamente ao iniciar sessão.';

  @override
  String get settingsShowInDock => 'Mostrar no Dock';

  @override
  String get settingsShowInTaskbar => 'Mostrar na barra de tarefas';

  @override
  String get settingsTaskbarVisibilityDescription =>
      'Mostrar o ícone do app para alternar entre janelas.';

  @override
  String get settingsTaskbarVisibilityKeywords =>
      'dock barra tarefas ícone app visibilidade dock taskbar application icon visibility';

  @override
  String get settingsShowInMenuBar => 'Mostrar na barra de menus';

  @override
  String get settingsShowInSystemTray => 'Mostrar na bandeja do sistema';

  @override
  String get settingsTrayVisibilityDescription =>
      'Mostrar acesso rápido ao CopyPaste e aos controles de captura.';

  @override
  String get settingsTrayVisibilityKeywords =>
      'bandeja barra menus status ícone visibilidade tray menu bar menubar status icon visibility';

  @override
  String get settingsVisibilityErrorTitle =>
      'Não foi possível alterar a visibilidade';

  @override
  String get settingsModulesSearchDescription =>
      'Instale, atualize e remova módulos opcionais.';

  @override
  String get settingsModulesKeywords =>
      'recursos comandos extensões features commands extensions';

  @override
  String get settingsAndroidCaptureTitle =>
      'Captura em segundo plano no Android';

  @override
  String get settingsAndroidCaptureSearchDescription =>
      'Captura completa ou limitada com configuração via Shizuku ou ADB.';

  @override
  String get settingsAndroidCaptureKeywords =>
      'segundo plano permissões configuração background permissions setup';

  @override
  String get settingsAndroidCaptureDescription =>
      'Escolha a Captura completa ou limitada e gerencie a configuração única via Shizuku ou ADB.';

  @override
  String get settingsOpenSetup => 'Abrir configuração';

  @override
  String get settingsSaveScreenshotsTitle => 'Salvar capturas de tela';

  @override
  String get settingsSaveScreenshotsSearchDescription =>
      'Salvar automaticamente novas capturas de tela do Android no Histórico.';

  @override
  String get settingsSaveScreenshotsKeywords =>
      'imagens fotos captura permissões images photos capture permissions';

  @override
  String get settingsSaveScreenshotsDescription =>
      'Adicionar automaticamente novas capturas de tela ao Histórico.';

  @override
  String get settingsScreenshotAccessTitle => 'Acesso às capturas de tela';

  @override
  String get settingsScreenshotAccessDescription =>
      'Permita o acesso a todas as fotos e às notificações para salvar novas capturas de tela em segundo plano.';

  @override
  String get settingsAllowAccess => 'Permitir acesso';

  @override
  String get settingsScreenshotCaptureStoppedTitle =>
      'O salvamento de capturas de tela não está em execução';

  @override
  String get settingsScreenshotCaptureStoppedMessage =>
      'Novas capturas de tela não estão sendo salvas. Retome a captura para iniciar o serviço novamente.';

  @override
  String get settingsResumeCapture => 'Retomar captura';

  @override
  String get settingsClipboardCaptureTitle =>
      'Captura da área de transferência';

  @override
  String get settingsClipboardCaptureSearchDescription =>
      'Pausar ou retomar a captura da área de transferência.';

  @override
  String get settingsCaptureStateUnavailable => 'Estado indisponível';

  @override
  String get settingsCaptureStoppedTitle => 'A captura não está em execução';

  @override
  String get settingsCaptureStoppedMessage =>
      'No momento, o CopyPaste não está recebendo alterações da área de transferência neste dispositivo.';

  @override
  String get settingsRetentionTitle => 'Retenção';

  @override
  String get settingsRetentionDescription =>
      'Remover automaticamente os itens antigos não fixados do histórico.';

  @override
  String get settingsRetentionForever => 'Manter para sempre';

  @override
  String get settingsStorageQuotaTitle => 'Cota de armazenamento';

  @override
  String get settingsStorageQuotaDescription =>
      'Armazenamento local máximo usado pelo histórico não fixado.';

  @override
  String get settingsStorageQuotaKeywords =>
      'disco espaço limite disk space limit';

  @override
  String get settingsMaxFileSizeTitle => 'Tamanho máximo de arquivo';

  @override
  String get settingsMaxFileSizeSearchDescription =>
      'Limite para adicionar e baixar arquivos.';

  @override
  String get settingsMaxFileSizeKeywords =>
      'importar baixar transferência tamanho limite import download transfer size limit';

  @override
  String get settingsMaxFileSizeDescription =>
      'Limite para adicionar e baixar arquivos. O conteúdo dos arquivos só é transferido quando você escolhe Baixar.';

  @override
  String get settingsHistoryFilesTitle => 'Arquivos do histórico';

  @override
  String get settingsHistoryFilesSearchDescription =>
      'Exportar o histórico, criar backups ou restaurar um backup.';

  @override
  String get settingsHistoryFilesKeywords =>
      'texto criptografado backup dados text encrypted backup data';

  @override
  String get settingsExportTextTitle => 'Exportar histórico de texto';

  @override
  String get settingsExportTextDescription =>
      'Salve os clipes de texto em um arquivo portátil.';

  @override
  String get settingsExportAction => 'Exportar';

  @override
  String get settingsBackupTitle => 'Backup criptografado';

  @override
  String get settingsBackupDescription =>
      'Salve todo o histórico local deste dispositivo.';

  @override
  String get settingsCreateBackupAction => 'Criar backup';

  @override
  String get settingsRestoreTitle => 'Restaurar backup';

  @override
  String get settingsRestoreDescription =>
      'Substitua o histórico local por um backup criptografado.';

  @override
  String get settingsRestoreAction => 'Restaurar';

  @override
  String get settingsRestoreDialogTitle => 'Substituir o histórico local?';

  @override
  String get settingsRestoreDialogMessage =>
      'O backup criptografado selecionado substituirá o histórico da área de transferência neste dispositivo. As configurações atuais e a identidade do dispositivo serão mantidas.';

  @override
  String get settingsRestoreFailed => 'Não foi possível restaurar o backup.';

  @override
  String get settingsSyncSwitchTitle => 'Sincronização';

  @override
  String get settingsSyncSwitchDescription =>
      'Chave geral da sincronização com dispositivos pareados.';

  @override
  String get settingsSyncSearchDescription =>
      'Sincronização com dispositivos pareados.';

  @override
  String get settingsInstantClipboardTitle =>
      'Área de transferência instantânea';

  @override
  String get settingsInstantClipboardSearchDescription =>
      'Copiar automaticamente novos clipes de outros dispositivos.';

  @override
  String get settingsInstantClipboardKeywords =>
      'automático copiar colar sincronização recebido automatic copy paste sync received';

  @override
  String get settingsInstantClipboardDescription =>
      'Copiar automaticamente para esta área de transferência os clipes mais recentes de outros dispositivos.';

  @override
  String get settingsLanVisibilityTitle => 'Visibilidade na rede local';

  @override
  String get settingsLanVisibilityDescription =>
      'Permitir que dispositivos próximos encontrem este dispositivo.';

  @override
  String get settingsLanVisibilityKeywords =>
      'rede local descoberta LAN local network discovery';

  @override
  String get settingsExcludedAppsTitle => 'Apps ignorados';

  @override
  String get settingsExcludedAppsSearchDescription =>
      'Não capturar automaticamente de apps ignorados.';

  @override
  String get settingsExcludedAppsKeywords =>
      'privacidade app identificadores ignorados privacy app identifiers';

  @override
  String get settingsExcludedAppsMacDescription =>
      'A captura automática é ignorada durante a atividade destes apps. Cópias em segundo plano podem não ser ignoradas.';

  @override
  String get settingsExcludedAppsDesktopDescription =>
      'A captura automática é ignorada quando o proprietário da área de transferência é identificado. A captura fica pausada quando há apps ignorados e a origem não pode ser identificada.';

  @override
  String get settingsExcludedAppsAndroidDescription =>
      'O Android suspende a captura automática enquanto houver apps ignorados, pois não consegue identificar os apps de origem.';

  @override
  String get settingsExcludedAppsUnsupportedDescription =>
      'A lista de apps ignorados é compatível com macOS, Windows, Android e Linux.';

  @override
  String get settingsExcludedAppsAdd => 'Adicionar';

  @override
  String get settingsExcludedAppsEmpty => 'Nenhum app está sendo ignorado.';

  @override
  String settingsExcludedAppsRemove(String app) {
    return 'Remover $app';
  }

  @override
  String get settingsExcludeDialogTitle => 'Ignorar app';

  @override
  String get settingsExcludeDialogAction => 'Ignorar';

  @override
  String get settingsExcludeDialogPlaceholder => 'Identificador do app';

  @override
  String get settingsExcludeFailed => 'Não foi possível ignorar o app.';

  @override
  String get settingsSkipSecretTitle => 'Ignorar conteúdo confidencial';

  @override
  String get settingsSkipSecretDescription =>
      'Manter fora do Histórico os segredos marcados pelo app de origem.';

  @override
  String get settingsSkipSecretKeywords =>
      'segredo senha sensível oculto spoiler secret password sensitive concealed spoiler';

  @override
  String get settingsSkipTransientTitle => 'Ignorar conteúdo temporário';

  @override
  String get settingsSkipTransientDescription =>
      'Manter fora do Histórico as cópias temporárias marcadas pelo app de origem.';

  @override
  String get settingsSkipTransientKeywords =>
      'transitório temporário privacidade transient temporary privacy';

  @override
  String get settingsBlockScreenshotsTitle => 'Bloquear capturas de tela';

  @override
  String get settingsBlockScreenshotsDescription =>
      'Impedir capturas e gravações da tela do CopyPaste.';

  @override
  String get settingsBlockScreenshotsUnsupported =>
      'O bloqueio de capturas de tela não está disponível nesta plataforma.';

  @override
  String get settingsBlockScreenshotsKeywords =>
      'privacidade tela captura proteção pareamento qr segurança código privacy screen capture protection pairing qr security code';

  @override
  String get settingsNotifyOnCopyTitle => 'Notificar ao copiar';

  @override
  String get settingsNotifyOnCopyDescription =>
      'Mostrar uma notificação após uma captura em segundo plano.';

  @override
  String get settingsNotificationPreviewTitle =>
      'Mostrar conteúdo da área de transferência';

  @override
  String get settingsNotificationPreviewDescription =>
      'Incluir uma prévia do clipe nas notificações de cópia.';

  @override
  String get settingsNotificationPreviewKeywords =>
      'notificação prévia texto imagem privacidade notification preview text image privacy';

  @override
  String get settingsSoundOnCopyTitle => 'Som ao copiar';

  @override
  String get settingsSoundOnCopyDescription =>
      'Reproduzir o retorno do sistema após uma captura bem-sucedida.';

  @override
  String get settingsVersionTitle => 'Versão';

  @override
  String get settingsVersionDescription => 'Versão instalada do CopyPaste.';

  @override
  String get settingsVersionUnavailable => 'Indisponível';

  @override
  String get settingsUpdatesTitle => 'Atualizações do app';

  @override
  String get settingsUpdatesSearchDescription =>
      'Verificar e instalar atualizações do CopyPaste.';

  @override
  String get settingsUpdatesKeywords =>
      'versão github lançamento version github release';

  @override
  String get settingsUpdateIdle =>
      'Verifique se há uma versão mais recente do CopyPaste.';

  @override
  String get settingsUpdateChecking => 'Verificando atualizações.';

  @override
  String get settingsUpdateUpToDate => 'O CopyPaste está atualizado.';

  @override
  String settingsUpdateVersionUpToDate(String version) {
    return 'A versão $version está atualizada.';
  }

  @override
  String settingsUpdateAvailable(String available, String current) {
    return 'A versão $available está disponível. Este dispositivo tem a versão $current.';
  }

  @override
  String settingsUpdateDownloading(String version, int percent) {
    return 'Baixando a versão $version · $percent%.';
  }

  @override
  String get settingsUpdatePreparingModules =>
      'Preparando as atualizações dos módulos instalados.';

  @override
  String settingsUpdateInstalling(String version) {
    return 'Instalando a versão $version.';
  }

  @override
  String get settingsUpdateNeedsAttention =>
      'As atualizações do app precisam de atenção.';

  @override
  String get settingsUpdateNowAction => 'Atualizar agora';

  @override
  String get settingsUpdateOpenReleaseAction => 'Abrir página da versão';

  @override
  String get settingsUpdateCheckingAction => 'Verificando';

  @override
  String get settingsUpdateDownloadingAction => 'Baixando';

  @override
  String get settingsUpdateInstallingAction => 'Instalando';

  @override
  String get settingsUpdateRestartAction => 'Reiniciar o CopyPaste';

  @override
  String get settingsUpdateCheckAgainAction => 'Verificar novamente';

  @override
  String get settingsUpdateNoneTitle => 'Nenhuma atualização disponível';

  @override
  String get settingsUpdateLatest => 'O CopyPaste está na versão mais recente.';

  @override
  String settingsUpdateVersionLatest(String version) {
    return 'O CopyPaste $version é a versão mais recente.';
  }

  @override
  String get settingsUpdateAvailableTitle => 'Atualização disponível';

  @override
  String get settingsUpdateReady =>
      'Uma versão mais recente do CopyPaste está pronta para instalar.';

  @override
  String settingsUpdateVersionReady(String version) {
    return 'O CopyPaste $version está pronto para instalar.';
  }

  @override
  String get settingsUpdateUnavailableTitle => 'Atualização indisponível';

  @override
  String get settingsUpdateUnavailableMessage =>
      'Esta instalação não pode ser atualizada automaticamente.';

  @override
  String get settingsUpdateCheckFailedTitle =>
      'Falha ao verificar atualizações';

  @override
  String get settingsUpdateCheckFailedMessage =>
      'O CopyPaste não pôde verificar atualizações.';

  @override
  String get settingsQuickPasteSetupTitle => 'Configurar o Quick Paste';

  @override
  String get settingsQuickPasteSetupMessage =>
      'Ative o complemento assinado para GNOME ou KDE e permita o controle do teclado para voltar ao app anterior.';

  @override
  String get settingsQuickPasteDesktopIntegration =>
      'Integração com a área de trabalho';

  @override
  String get settingsQuickPasteAllowKeyboard => 'Permitir controle do teclado';

  @override
  String get settingsQuickPasteCheckAgain => 'Verificar novamente';

  @override
  String get settingsQuickPasteLoading =>
      'Carregando as configurações do Quick Paste.';

  @override
  String get settingsQuickPasteUnavailableTitle =>
      'O Quick Paste não está disponível';

  @override
  String get settingsQuickPasteUnavailableMessage =>
      'Este dispositivo não oferece a janela do Quick Paste.';

  @override
  String get settingsQuickPasteShortcutTitle => 'Abrir o Quick Paste';

  @override
  String get settingsQuickPasteShortcutSearchDescription =>
      'Configurar o atalho global do Quick Paste.';

  @override
  String get settingsQuickPasteShortcutKeywords =>
      'teclado atalho tecla keyboard hotkey';

  @override
  String get settingsQuickPasteShortcutDescription =>
      'O atalho funciona enquanto o CopyPaste está em execução em segundo plano.';

  @override
  String settingsQuickPasteRegisteredShortcut(String shortcut) {
    return 'Atalho registrado: $shortcut';
  }

  @override
  String get settingsAutoPasteTitle => 'Colar automaticamente';

  @override
  String get settingsAutoPasteSearchDescription =>
      'Colar o clipe selecionado no app anterior.';

  @override
  String get settingsAutoPasteKeywords =>
      'acessibilidade automático accessibility automatic';

  @override
  String get settingsAutoPasteDescription =>
      'Ao selecionar um clipe, o CopyPaste volta ao app anterior e cola o clipe.';

  @override
  String get settingsAccessibilityRequiredTitle =>
      'É necessária a permissão de Acessibilidade';

  @override
  String get settingsAccessibilityRequiredMessage =>
      'Até que a permissão seja concedida, selecionar um clipe apenas o copia, sem colar.';

  @override
  String get settingsQuickPasteAttentionTitle =>
      'O Quick Paste precisa de atenção';

  @override
  String get settingsShortcutRecording => 'Pressione o atalho…';

  @override
  String get settingsShortcutNotSet => 'Não definido';

  @override
  String get settingsShortcutReset => 'Redefinir atalho';

  @override
  String settingsRetentionDays(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dias',
      many: '$count dias',
      one: '$count dia',
    );
    return '$_temp0';
  }

  @override
  String get devicesLoading => 'Carregando dispositivos…';

  @override
  String get devicesUnavailableTitle => 'Os dispositivos não estão disponíveis';

  @override
  String get devicesNearbyTitle => 'Dispositivos próximos';

  @override
  String get devicesScan => 'Procurar';

  @override
  String get devicesScanning => 'Procurando…';

  @override
  String get devicesNoneNearby => 'Nenhum dispositivo próximo';

  @override
  String get devicesThisDevice => 'Este dispositivo';

  @override
  String get devicesTrusted => 'Confiável';

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
  String get devicesRenameDialogTitle => 'Renomear este dispositivo';

  @override
  String get devicesNamePlaceholder => 'Nome do dispositivo';

  @override
  String get devicesRenameFailed =>
      'Não foi possível salvar o nome do dispositivo.';

  @override
  String devicesRevokeDialogTitle(String device) {
    return 'Revogar $device?';
  }

  @override
  String devicesUnpairDialogTitle(String device) {
    return 'Desparear $device?';
  }

  @override
  String get devicesRevokeDialogMessage =>
      'Isso remove o dispositivo do seu grupo e bloqueia este pareamento permanentemente. Os outros dispositivos aplicam a remoção quando se reconectarem.';

  @override
  String get devicesUnpairDialogMessage =>
      'Isso remove o dispositivo do seu grupo. Os outros dispositivos aplicam a remoção quando se reconectarem. Para adicioná-lo novamente, será necessária uma nova confirmação.';

  @override
  String get devicesRevoke => 'Revogar';

  @override
  String get devicesUnpair => 'Desparear';

  @override
  String get devicesActionFailedMessage =>
      'Não foi possível concluir a ação no dispositivo.';

  @override
  String get devicesCloseDetails => 'Fechar detalhes do dispositivo';

  @override
  String get devicesDetailType => 'Tipo de dispositivo';

  @override
  String get devicesDetailOperatingSystem => 'Sistema operacional';

  @override
  String get devicesDetailModel => 'Modelo';

  @override
  String get devicesDetailAppVersion => 'Versão do CopyPaste';

  @override
  String get devicesDetailProtocolVersion => 'Versão do protocolo';

  @override
  String get devicesDetailStatus => 'Status';

  @override
  String get devicesDetailPing => 'Ping';

  @override
  String get devicesDetailMeasured => 'Medido em';

  @override
  String get devicesDetailLastSeen => 'Visto por último';

  @override
  String get devicesDetailLanEndpoint => 'Endereço na rede local';

  @override
  String get devicesDetailDeviceId => 'ID do dispositivo';

  @override
  String get devicesDetailPairingId => 'ID do pareamento';

  @override
  String get devicesDetailProfileProvenance => 'Origem do perfil';

  @override
  String get devicesDetailProfileTrust => 'Confiabilidade do perfil';

  @override
  String get devicesDetailProfileUpdated => 'Perfil atualizado em';

  @override
  String get devicesValueUnavailable => 'Indisponível';

  @override
  String get devicesRenameDevice => 'Renomear dispositivo';

  @override
  String get devicesSyncNow => 'Sincronizar agora';

  @override
  String get devicesRevokePairing => 'Revogar pareamento';

  @override
  String get devicesLocal => 'Local';

  @override
  String get devicesTrustAuthenticated => 'Autenticado';

  @override
  String get devicesTrustUnverified => 'Não verificado';

  @override
  String get devicesProvenanceSelfReported => 'Informado pelo dispositivo';

  @override
  String get devicesProvenanceObserved => 'Observado';

  @override
  String get devicesProvenanceMeasured => 'Medido';

  @override
  String get devicesConnectionOnline => 'On-line';

  @override
  String get devicesConnectionOffline => 'Off-line';

  @override
  String get devicesConnectionVisible => 'Visível';

  @override
  String get devicesConnectionNotSeen => 'Não visto';

  @override
  String get devicesConnectionUnknown => 'Desconhecido';

  @override
  String get devicesPeerAvailable => 'Disponível';

  @override
  String get devicesPeerVisibleNow => 'Visível agora';

  @override
  String get devicesPeerStatusUnknown => 'Status desconhecido';

  @override
  String get devicesPingUnavailable => '— ms';

  @override
  String get devicesPingUnderOneMillisecond => '<1 ms';

  @override
  String devicesPingMilliseconds(int milliseconds) {
    return '$milliseconds ms';
  }

  @override
  String get devicesClassDesktop => 'Computador de mesa';

  @override
  String get devicesClassLaptop => 'Notebook';

  @override
  String get devicesClassPhone => 'Celular';

  @override
  String get devicesClassTablet => 'Tablet';

  @override
  String get devicesClassUnknown => 'Dispositivo';

  @override
  String get devicesUnknownOs => 'SO desconhecido';

  @override
  String get devicesAddDevice => 'Adicionar dispositivo';

  @override
  String get devicesScanQr => 'Escanear QR';

  @override
  String get devicesEnterCode => 'Inserir código';

  @override
  String get devicesScannerUnavailableTitle => 'Leitor indisponível';

  @override
  String get devicesActionFailedTitle => 'Falha na ação';

  @override
  String get devicesRefreshFailedTitle => 'Falha ao atualizar os dispositivos';

  @override
  String get devicesNearbyPairingUnavailable =>
      'O pareamento com dispositivos próximos não está disponível.';

  @override
  String get devicesNearbyPairingUnavailableUseAddDevice =>
      'O pareamento com dispositivos próximos não está disponível. Use Adicionar dispositivo para mostrar um código de pareamento.';

  @override
  String get pairingTitle => 'Pareamento';

  @override
  String get pairingPairDevice => 'Parear um dispositivo';

  @override
  String get pairingScanQrCode => 'Escanear código QR';

  @override
  String get pairingEnterPairingCode => 'Inserir código de pareamento';

  @override
  String get pairingConnectNearbyDevice => 'Conectar dispositivo próximo';

  @override
  String get pairingSendingRequest => 'Enviando solicitação de conexão…';

  @override
  String get pairingEmptyTitle => 'Escolha uma ação de pareamento';

  @override
  String get pairingEmptyMessage => 'Os controles de pareamento aparecem aqui.';

  @override
  String get pairingCreatingInvitation => 'Criando um convite de pareamento.';

  @override
  String get pairingCode => 'Código de pareamento';

  @override
  String get pairingCodeHint =>
      'Insira o código de 8 caracteres mostrado no outro dispositivo.';

  @override
  String get pairingDeviceAddress => 'Endereço do dispositivo';

  @override
  String get pairingDeviceAddressHint =>
      'Use o endereço host:port informado pelo outro dispositivo.';

  @override
  String get pairingCheckDetailsTitle => 'Verifique os dados de pareamento';

  @override
  String get pairingActionFailed => 'Falha na ação de pareamento';

  @override
  String get pairingConnectingDevice => 'Conectando dispositivo…';

  @override
  String get pairingQrScannerTitle => 'Leitor de QR';

  @override
  String get pairingCameraPreviewPlaceholder =>
      'A pré-visualização da câmera aparece aqui.';

  @override
  String get pairingScannerNeedsAttention => 'O leitor precisa de atenção';

  @override
  String get pairingCodeIncomplete =>
      'Insira o código de pareamento completo de 8 caracteres.';

  @override
  String get pairingAddressMissing =>
      'Insira o endereço host:port do dispositivo.';

  @override
  String get pairingWaitingForOtherDevice => 'Aguardando o outro dispositivo';

  @override
  String get pairingInviteInstructions =>
      'No outro dispositivo, escolha Adicionar dispositivo → Inserir código ou escaneie o código QR.';

  @override
  String get pairingDetailsTitle => 'Detalhes do pareamento';

  @override
  String get pairingAddress => 'Endereço';

  @override
  String get pairingVerificationCode => 'Código de verificação';

  @override
  String get pairingVerificationCodeHint =>
      'Confirme apenas se este código for igual nos dois dispositivos.';

  @override
  String get pairingReject => 'Recusar';

  @override
  String get pairingConfirm => 'Confirmar';

  @override
  String get pairingAccept => 'Aceitar';

  @override
  String get pairingJoinDevice => 'Conectar ao dispositivo';

  @override
  String get pairingStartScanner => 'Iniciar leitor';

  @override
  String get pairingActionNotStarted =>
      'Não foi possível iniciar a ação de pareamento.';

  @override
  String get pairingStateHandshaking => 'Verificando dispositivo';

  @override
  String get pairingStateAwaitingConfirmation =>
      'Confirme nos dois dispositivos';

  @override
  String get pairingStateConfirmed => 'Dispositivo pareado';

  @override
  String get pairingStateRejected => 'Pareamento recusado';

  @override
  String get pairingStateCancelled => 'Pareamento cancelado';

  @override
  String get pairingStateTimedOut => 'O pareamento expirou';

  @override
  String get pairingStateFailed => 'Falha no pareamento';

  @override
  String get pairingHandshakingDescription =>
      'Os dispositivos estão estabelecendo um canal seguro.';

  @override
  String get pairingAwaitingConfirmationDescription =>
      'Compare o código de verificação protegido e confirme nos dois dispositivos.';

  @override
  String get pairingConfirmedDescription =>
      'O dispositivo está conectado. A participação no seu grupo será sincronizada automaticamente.';

  @override
  String get pairingNoDeviceAdded => 'Nenhum dispositivo foi adicionado.';

  @override
  String get pairingTimedOutDescription =>
      'Inicie uma nova tentativa de pareamento para continuar.';

  @override
  String get pairingIdleDescription =>
      'Inicie uma tentativa de pareamento para conectar um dispositivo.';

  @override
  String get pairingProtectedUnavailableTitle =>
      'O pareamento protegido não está disponível';

  @override
  String get pairingProtectedUnavailableMessage =>
      'Esta sessão de pareamento precisa ser aberta em uma janela protegida dedicada.';

  @override
  String get pairingCancel => 'Cancelar pareamento';

  @override
  String get pairingOrEnterCode => 'Ou insira um código de pareamento';

  @override
  String get pairingJoinSecurely => 'Conectar com segurança';

  @override
  String get pairingProtectedWaitingDescription =>
      'Revele um código QR, escaneie um ou insira um código de pareamento nesta janela protegida.';

  @override
  String pairingExpiresInSeconds(int seconds) {
    return 'Expira em $seconds s';
  }

  @override
  String get pairingInvalidQr =>
      'Escaneie um código QR de pareamento do CopyPaste.';

  @override
  String get pairingGoogleScannerUnavailable =>
      'O leitor do Google não está disponível. Insira o código de pareamento.';

  @override
  String get pairingScannerCouldNotOpen =>
      'Não foi possível abrir o leitor. Insira o código de pareamento.';

  @override
  String get pairingSecurePresentationUnavailable =>
      'A exibição segura do pareamento não está disponível.';

  @override
  String get pairingVerificationCodeRequired =>
      'Revele e compare o código de verificação primeiro.';

  @override
  String get syncStatusUnavailable => 'Estado da sincronização indisponível';

  @override
  String get syncStatusOff => 'Sincronização desativada';

  @override
  String get syncStatusWaitingForSync => 'Aguardando sincronização';

  @override
  String get syncStatusSyncing => 'Sincronizando';

  @override
  String get syncStatusUpToDate => 'Atualizado';

  @override
  String get syncStatusInterrupted => 'Sincronização interrompida';

  @override
  String get syncStatusPartiallySynchronized => 'Parcialmente sincronizado';

  @override
  String get syncStatusRetrying => 'Tentando novamente';

  @override
  String get syncStatusWaitingToSync => 'Aguardando para sincronizar';

  @override
  String get syncStatusWaitingForDevice => 'Aguardando o dispositivo';

  @override
  String get syncStatusNoPairedDevices => 'Nenhum dispositivo pareado';

  @override
  String get syncStatusDevicesOffline => 'Dispositivos off-line';

  @override
  String syncStatusWithActivityLogUnavailable(String status) {
    return '$status · registro de atividades indisponível';
  }

  @override
  String syncStatusOffWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Sincronização desativada · $count problemas',
      many: 'Sincronização desativada · $count problemas',
      one: 'Sincronização desativada · $count problema',
    );
    return '$_temp0';
  }

  @override
  String syncStatusWithOfflineDevices(String status, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$status · $count off-line',
      many: '$status · $count off-line',
      one: '$status · $count off-line',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesNeedAttention(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count dispositivos precisam de atenção',
      many: '$count dispositivos precisam de atenção',
      one: '$count dispositivo precisa de atenção',
    );
    return '$_temp0';
  }

  @override
  String syncStatusDevicesOfflineWithIssues(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Dispositivos off-line · $count problemas',
      many: 'Dispositivos off-line · $count problemas',
      one: 'Dispositivos off-line · $count problema',
    );
    return '$_temp0';
  }

  @override
  String get syncConnectionChecking => 'Verificando conexão';

  @override
  String get syncConnectionUnknown => 'Conexão desconhecida';

  @override
  String get syncLogDeviceOffline => 'O dispositivo ficou off-line.';

  @override
  String get syncLogConnectionRestored => 'Conexão restabelecida.';

  @override
  String get syncLogProblemDetected => 'Problema de sincronização detectado.';

  @override
  String get syncLogProblemResolved => 'Problema de sincronização resolvido.';

  @override
  String get syncLogFailed => 'Falha na sincronização.';

  @override
  String get syncLogRecovered => 'Sincronização restabelecida.';

  @override
  String syncLogClipsTransferred(int sent, int received) {
    String _temp0 = intl.Intl.pluralLogic(
      sent,
      locale: localeName,
      other: '$sent clipes',
      many: '$sent clipes',
      one: '$sent clipe',
    );
    String _temp1 = intl.Intl.pluralLogic(
      received,
      locale: localeName,
      other: '$received clipes',
      many: '$received clipes',
      one: '$received clipe',
    );
    return 'Enviados: $_temp0 · Recebidos: $_temp1';
  }

  @override
  String syncHeaderSemanticsLabel(String status) {
    return 'Sincronização: $status';
  }

  @override
  String get syncDetailsTitle => 'Sincronização';

  @override
  String get syncDetailsClose => 'Fechar detalhes da sincronização';

  @override
  String get syncNoPairedDevicesMessage =>
      'Pareie um dispositivo para sincronizar o histórico da área de transferência.';

  @override
  String get syncActivityLogUnavailableTitle =>
      'Registro de atividades indisponível';

  @override
  String get syncActivityLogUnavailableMessage =>
      'Não foi possível ler ou salvar a atividade de sincronização.';

  @override
  String get syncDetailsDevicesHeading => 'Dispositivos';

  @override
  String get syncDetailsActivityHeading => 'Atividade';

  @override
  String get syncDetailsNoRecentProblems =>
      'Nenhum problema recente de sincronização';

  @override
  String get syncDetailsUnknownPeer => 'Dispositivo pareado';

  @override
  String get syncPeerConnection => 'Conexão';

  @override
  String get syncPeerLastCheckStarted => 'Início da última verificação';

  @override
  String get syncPeerItemsOverSizeLimit => 'Itens acima do limite de tamanho';

  @override
  String syncPeerLastSeen(String time) {
    return 'Visto por último: $time';
  }

  @override
  String syncPeerLastSynced(String time) {
    return 'Última sincronização: $time';
  }

  @override
  String get syncPeerLastSuccessfulSync => 'Última sincronização bem-sucedida';

  @override
  String get syncPeerConnectionDetails => 'Detalhes da conexão';

  @override
  String syncPeerConnectionDetailsFor(String device) {
    return 'Detalhes da conexão de $device';
  }

  @override
  String get historyKindText => 'Texto';

  @override
  String get historyKindLink => 'Link';

  @override
  String get historyKindEmail => 'E-mail';

  @override
  String get historyKindColor => 'Cor';

  @override
  String get historyKindPhone => 'Telefone';

  @override
  String get historyKindCode => 'Código';

  @override
  String get historyKindPath => 'Caminho';

  @override
  String get historyKindCredentials => 'Credenciais';

  @override
  String get historyKindImage => 'Imagem';

  @override
  String get historyKindFile => 'Arquivo';

  @override
  String get historyKindOther => 'Outro';

  @override
  String get historySortNewest => 'Mais recentes primeiro';

  @override
  String get historySortOldest => 'Mais antigos primeiro';

  @override
  String get historySortRelevance => 'Mais relevantes';

  @override
  String get historyErrorUpdatesUnavailable =>
      'As atualizações do histórico estão temporariamente indisponíveis.';

  @override
  String get historyErrorBulkDelete =>
      'Não foi possível excluir alguns clipes selecionados. Tente novamente.';

  @override
  String get historyErrorBulkPin =>
      'Não foi possível atualizar a fixação de alguns clipes selecionados. Tente novamente.';

  @override
  String get historyErrorRefresh =>
      'Não foi possível atualizar o histórico. Tente novamente.';

  @override
  String get historyErrorLoad =>
      'Não foi possível carregar o histórico. Tente novamente.';

  @override
  String get historyErrorLoadMore =>
      'Não foi possível carregar mais itens do histórico.';

  @override
  String get historyErrorLoadEarlier =>
      'Não foi possível carregar o histórico anterior.';

  @override
  String get historyErrorOpenClip => 'Não foi possível abrir este clipe.';

  @override
  String get historyErrorCopyPlainText =>
      'Não foi possível copiar o texto simples.';

  @override
  String get historyErrorCopyClip => 'Não foi possível copiar este clipe.';

  @override
  String get historyErrorCopyText => 'Não foi possível copiar este texto.';

  @override
  String get historyErrorPrepareFile =>
      'Não foi possível baixar este arquivo. Verifique o dispositivo de origem e tente novamente.';

  @override
  String get historyErrorDownloadFile =>
      'Não foi possível baixar este arquivo.';

  @override
  String get historyErrorSelectFiles =>
      'Não foi possível selecionar os arquivos. Tente novamente.';

  @override
  String get historyErrorReadSelectedFile =>
      'Não foi possível ler o arquivo selecionado. Verifique o acesso e o armazenamento disponível.';

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
          'Não foi possível importar $failed de $total arquivos. Primeiro: $name. $reason',
      many:
          'Não foi possível importar $failed de $total arquivos. Primeiro: $name. $reason',
      one:
          'Não foi possível importar $failed de $total arquivo. Primeiro: $name. $reason',
    );
    return '$_temp0';
  }

  @override
  String get historyErrorSecrecyUpdate =>
      'Não foi possível atualizar o sigilo do clipe.';

  @override
  String get historyErrorPinUpdate => 'Não foi possível atualizar a fixação.';

  @override
  String get historyErrorPinnedOrderSave =>
      'Não foi possível salvar a ordem dos clipes fixados. Tente novamente.';

  @override
  String get historyErrorPinnedOrderRefresh =>
      'A ordem foi salva, mas não foi possível atualizar o histórico.';

  @override
  String get historyErrorDeleteClip => 'Não foi possível excluir este clipe.';

  @override
  String get historyErrorDeleteAll => 'Não foi possível excluir o histórico.';

  @override
  String get historyErrorBulkDeleteFallback =>
      'Não foi possível excluir os clipes selecionados.';

  @override
  String get historyOcrErrorUnavailable =>
      'O OCR não está disponível. Verifique o módulo em Configurações e tente novamente.';

  @override
  String get historyOcrErrorStart =>
      'Não foi possível iniciar o OCR. Tente novamente.';

  @override
  String get historyOcrErrorRecognize =>
      'Não foi possível reconhecer o texto da imagem. Tente novamente.';

  @override
  String get historyOcrRecognizing => 'Reconhecendo o texto da imagem.';

  @override
  String get historyOcrFailedTitle => 'Falha no OCR';

  @override
  String get historyOcrNoText => 'Nenhum texto encontrado';

  @override
  String get historyOcrTitle => 'Texto da imagem';

  @override
  String get historyOcrTextCopied => 'Texto reconhecido copiado.';

  @override
  String get historyOcrCopyError =>
      'Não foi possível copiar o texto. Tente novamente.';

  @override
  String get historyTagsErrorLoad =>
      'Não foi possível carregar as etiquetas. Tente novamente.';

  @override
  String get historyTagsErrorSave =>
      'Não foi possível salvar a alteração da etiqueta. Use um nome exclusivo de até 64 caracteres e selecione clipes fixados.';

  @override
  String get historyTags => 'Etiquetas';

  @override
  String get historyTagsUpdateErrorTitle =>
      'Não foi possível atualizar as etiquetas';

  @override
  String get historyTagsEmpty => 'Nenhuma etiqueta ainda';

  @override
  String get historyTagRename => 'Renomear';

  @override
  String get historyTagActions => 'Ações da etiqueta';

  @override
  String get historyTagNamePlaceholder => 'Nome da etiqueta';

  @override
  String get historyCreateTag => 'Criar etiqueta';

  @override
  String get historySaveTag => 'Salvar etiqueta';

  @override
  String get historyTagsFilterAll => 'Todas as etiquetas';

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
  String get historyManageTags => 'Gerenciar etiquetas';

  @override
  String get historyTagsFilterSemantics =>
      'Filtrar clipes fixados por etiquetas';

  @override
  String get historyDownloadErrorCancelled =>
      'O download do arquivo foi cancelado.';

  @override
  String get historyDownloadErrorServiceUnavailable =>
      'O serviço de download não está disponível. Reabra o CopyPaste e tente novamente.';

  @override
  String get historyDownloadErrorServiceTimeout =>
      'O serviço de download não se conectou. Tente novamente.';

  @override
  String get historyFileErrorAccess =>
      'Não foi possível acessar o arquivo selecionado.';

  @override
  String get historySpoilerRevealSemantics =>
      'Conteúdo confidencial. Revelar spoiler';

  @override
  String get historyActionFailedTitle => 'Falha na ação';

  @override
  String get historyLoadingMessage =>
      'Carregando o histórico da área de transferência.';

  @override
  String get historyUnavailableTitle => 'O histórico não está disponível';

  @override
  String get historyEmptyTitle => 'Nenhum clipe encontrado';

  @override
  String get historyEmptyMessage =>
      'Os clipes capturados e o histórico correspondente aparecem aqui.';

  @override
  String get historyAttentionTitle => 'O histórico precisa de atenção';

  @override
  String get historyLoadMore => 'Carregar mais';

  @override
  String get historySectionPinned => 'Fixados';

  @override
  String get historySectionToday => 'Hoje';

  @override
  String get historySectionYesterday => 'Ontem';

  @override
  String historySkippedRowsTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count itens da área de transferência foram ignorados',
      many: '$count itens da área de transferência foram ignorados',
      one: '$count item da área de transferência foi ignorado',
    );
    return '$_temp0';
  }

  @override
  String get historySkippedRowsMessage =>
      'Alguns clipes armazenados não puderam ser lidos e não aparecem nesta lista.';

  @override
  String get historySearch => 'Buscar no histórico';

  @override
  String get historyResetFilters => 'Redefinir filtros';

  @override
  String get historyImportFiles => 'Importar arquivos';

  @override
  String get historySelectClips => 'Selecionar clipes';

  @override
  String get historyCloseSearch => 'Fechar busca';

  @override
  String get historyFilterAllClips => 'Todos os clipes';

  @override
  String get historyFilterAllDevices => 'Todos os dispositivos';

  @override
  String get historyFilterAllApps => 'Todos os apps';

  @override
  String get historyPinClip => 'Fixar clipe';

  @override
  String get historyUnpinClip => 'Desafixar clipe';

  @override
  String get historyReorderPinnedClip => 'Reordenar clipe fixado';

  @override
  String get historyReorderPinnedHint =>
      'Arraste para mover. Alt + Seta para cima ou Seta para baixo move uma posição.';

  @override
  String get historyClipUnavailable => 'Clipe indisponível';

  @override
  String historyClipKindSemantics(String kind) {
    return 'Tipo de clipe: $kind';
  }

  @override
  String get historyInspectorEmptyTitle => 'Selecione um clipe';

  @override
  String get historyInspectorEmptyMessage =>
      'O conteúdo completo e as ações aparecem aqui.';

  @override
  String get historyNoPreview => 'Nenhuma prévia disponível.';

  @override
  String get historyCloseClipDetails => 'Fechar detalhes do clipe';

  @override
  String get historyCopyOptions => 'Opções de cópia';

  @override
  String get historyCopyPlainText => 'Copiar texto simples';

  @override
  String get historyManageClipTags => 'Gerenciar etiquetas do clipe';

  @override
  String get historyPinnedButton => 'Fixado';

  @override
  String get historyAddSpoiler => 'Adicionar spoiler';

  @override
  String get historyRemoveSpoiler => 'Remover spoiler';

  @override
  String get historyUpdateFailedTitle => 'Falha na atualização';

  @override
  String get historyRecognizeImageText => 'Reconhecer texto da imagem';

  @override
  String get historyCopiedTitle => 'Copiado';

  @override
  String get historyCopyFailedTitle => 'Falha ao copiar';

  @override
  String get historyPlainTextCopied => 'Texto simples copiado.';

  @override
  String get historyClipCopied => 'Clipe copiado.';

  @override
  String get historyDownloadedTitle => 'Baixado';

  @override
  String get historyFileSaved => 'Arquivo salvo.';

  @override
  String get historyDownload => 'Baixar';

  @override
  String get historyDownloading => 'Baixando…';

  @override
  String historyDownloadingPercent(int percent) {
    return 'Baixando $percent%';
  }

  @override
  String historyDownloadProgressPercent(int percent) {
    return '$percent%';
  }

  @override
  String get historyDownloadCancelling => 'Cancelando…';

  @override
  String get historyDownloadCancellingTooltip => 'Cancelando o download';

  @override
  String historyDownloadingCancelTooltip(int percent) {
    return 'Baixando $percent% · Clique para cancelar';
  }

  @override
  String get historyDownloadFailedRetryTooltip =>
      'Falha no download · Clique para tentar novamente';

  @override
  String historyCancelDownloadSemantics(int percent) {
    return 'Cancelar download, $percent% concluído';
  }

  @override
  String get historyRetryDownload => 'Tentar baixar novamente';

  @override
  String get historyDownloadFailedTitle => 'Falha no download';

  @override
  String get historyShowFile => 'Mostrar arquivo';

  @override
  String get historyMetadataPrivacy => 'Privacidade';

  @override
  String get historyMetadataConfidential => 'Confidencial';

  @override
  String get historyMetadataCaptured => 'Capturado em';

  @override
  String get historyMetadataObservedApp => 'App observado';

  @override
  String get historyMetadataDevice => 'Dispositivo';

  @override
  String get historyMetadataClipType => 'Tipo de clipe';

  @override
  String get historyMetadataType => 'Tipo';

  @override
  String get historyMetadataPath => 'Caminho';

  @override
  String get historyMetadataFile => 'Arquivo';

  @override
  String get historyMetadataResolution => 'Resolução';

  @override
  String get historyMetadataSize => 'Tamanho';

  @override
  String get historyMetadataFiles => 'Arquivos';

  @override
  String get historyMetadataSync => 'Sincronização';

  @override
  String get historyMetadataTooLargeToSync => 'Grande demais para sincronizar';

  @override
  String historyBulkDeleteTitle(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Excluir $count clipes selecionados?',
      many: 'Excluir $count clipes selecionados?',
      one: 'Excluir $count clipe selecionado?',
    );
    return '$_temp0';
  }

  @override
  String get historyDeleteClipTitle => 'Excluir este clipe?';

  @override
  String get historyBulkTagTooltip => 'Etiquetar clipes fixados selecionados';

  @override
  String get historyBulkExit => 'Sair da seleção';

  @override
  String get historyBulkPin => 'Fixar selecionados';

  @override
  String get historyBulkUnpin => 'Desafixar selecionados';

  @override
  String get historyBulkDelete => 'Excluir selecionados';

  @override
  String historyBulkSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count selecionados',
      many: '$count selecionados',
      one: '$count selecionado',
    );
    return '$_temp0';
  }

  @override
  String get historyDateFilter => 'Filtrar por data';

  @override
  String get historyDatePresetToday => 'Hoje';

  @override
  String get historyDatePresetYesterday => 'Ontem';

  @override
  String historyDatePresetLastDays(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Últimos $days dias',
      many: 'Últimos $days dias',
      one: 'Último $days dia',
    );
    return '$_temp0';
  }

  @override
  String get historyDatePresetAllTime => 'Todo o período';

  @override
  String get appRuntimeStartFailed =>
      'O CopyPaste não pôde iniciar seu ambiente de execução.';

  @override
  String get appPreparing => 'Preparando o CopyPaste.';

  @override
  String get appRuntimeWaiting =>
      'O CopyPaste está aguardando acesso ao histórico protegido. As partes disponíveis do app continuam utilizáveis.';

  @override
  String get appHistoryUnavailableTitle =>
      'O ambiente de execução do histórico não está disponível';

  @override
  String get appHistoryUnavailableMessage =>
      'Inicie o ambiente de execução do app antes de abrir o histórico.';

  @override
  String get appDevicesUnavailableTitle =>
      'O ambiente de execução dos dispositivos não está disponível';

  @override
  String get appDevicesUnavailableMessage =>
      'Inicie o ambiente de execução do app antes de abrir os dispositivos.';

  @override
  String get appSettingsUnavailableTitle =>
      'O ambiente de execução das configurações não está disponível';

  @override
  String get appSettingsUnavailableMessage =>
      'Inicie o ambiente de execução do app antes de abrir as configurações.';

  @override
  String get appQuit => 'Encerrar o CopyPaste';

  @override
  String get appWindowSetupTitle =>
      'A configuração da janela precisa de atenção';

  @override
  String get appOpenHistory => 'Abrir Histórico';

  @override
  String get appDestinationHistory => 'Histórico';

  @override
  String get appDestinationDevices => 'Dispositivos';

  @override
  String get appDestinationSettings => 'Configurações';

  @override
  String appDestinationSelected(String destination) {
    return '$destination, selecionado';
  }

  @override
  String get appActiveFilter => 'Filtro ativo';

  @override
  String get appActionFailed => 'Falha na ação';

  @override
  String get sharedAndroidVersionRequired =>
      'É necessário o Android 11 ou mais recente';

  @override
  String get sharedUseAdbTab => 'Use a aba ADB neste dispositivo.';

  @override
  String get sharedOneTimeAccessApplied => 'Acesso único aplicado';

  @override
  String get sharedInstallShizuku => 'Instalar o Shizuku';

  @override
  String get sharedPairAndStartShizuku => 'Parear e iniciar o Shizuku';

  @override
  String get sharedAllowCopyPaste => 'Permitir o CopyPaste';

  @override
  String get sharedShizukuNoLongerNeeded => 'O Shizuku não é mais necessário.';

  @override
  String get sharedGetShizukuDescription =>
      'Obtenha o Shizuku para aplicar o acesso único.';

  @override
  String get sharedShizukuWirelessDebugging =>
      'Use a Depuração por Wi-Fi no Shizuku e depois volte.';

  @override
  String get sharedApproveInShizuku => 'Aprove o CopyPaste uma vez no Shizuku.';

  @override
  String get sharedGetShizuku => 'Obter o Shizuku';

  @override
  String get sharedOpenShizuku => 'Abrir o Shizuku';

  @override
  String get sharedRunOnComputer => 'Execute no seu computador';

  @override
  String get sharedCopyAllCommands => 'Copiar todos os comandos';

  @override
  String get sharedEnableUsbDebugging =>
      'Ative a Depuração USB e conecte este celular.';

  @override
  String get sharedAllow => 'Permitir';

  @override
  String get sharedSetupCommandsCopyFailed =>
      'Não foi possível copiar os comandos de configuração.';

  @override
  String get sharedShizukuOpenFailed => 'Não foi possível abrir o Shizuku.';

  @override
  String sharedSizeBytes(String size) {
    return '$size B';
  }

  @override
  String get onboardingTitle => 'Configurar o CopyPaste';

  @override
  String get onboardingWelcomeTitle => 'Boas-vindas ao CopyPaste';

  @override
  String get onboardingWelcomeSubtitle =>
      'Sua área de transferência, pronta quando você precisar.';

  @override
  String get onboardingReadyTitle => 'O CopyPaste está pronto';

  @override
  String get onboardingGetStarted => 'Começar';

  @override
  String get onboardingOptionalSetupTitle =>
      'A configuração opcional precisa de atenção';

  @override
  String get onboardingOpenSystemSettings => 'Abrir configurações do sistema';

  @override
  String onboardingStepProgress(int step, int count, String platform) {
    return 'ETAPA $step DE $count · $platform';
  }

  @override
  String get onboardingSetupErrorTitle => 'A configuração precisa de atenção';

  @override
  String onboardingStateReadFailed(String platform) {
    return 'O CopyPaste não pôde ler o estado da configuração do $platform.';
  }

  @override
  String get onboardingSaveProgressFailed =>
      'O CopyPaste não pôde salvar o progresso da configuração inicial.';

  @override
  String get onboardingStartAtLogin => 'Abrir ao iniciar sessão';

  @override
  String get onboardingStartAtLoginDescription =>
      'Abrir o CopyPaste quando você iniciar sessão';

  @override
  String get onboardingStartAtLoginSemantics =>
      'Abrir o CopyPaste ao iniciar sessão';

  @override
  String get onboardingStartAtLoginEnableFailed =>
      'Não foi possível ativar a abertura ao iniciar sessão. Você pode continuar sem ela.';

  @override
  String get onboardingStartAtLoginUpdateFailed =>
      'Não foi possível atualizar a abertura ao iniciar sessão. Você pode continuar sem ela.';

  @override
  String get onboardingMacosTitle => 'Configure seu Mac';

  @override
  String get onboardingAccessibility => 'Acessibilidade';

  @override
  String get onboardingAutoPasteEnabled => 'Colagem automática ativada';

  @override
  String get onboardingAutoPasteOptional =>
      'Opcional para a colagem automática';

  @override
  String get onboardingMacosStartAtLoginUnavailable =>
      'Disponível no app CopyPaste instalado.';

  @override
  String get onboardingLoginItemApprovalTitle =>
      'É necessário aprovar o Item de Início';

  @override
  String get onboardingLoginItemApprovalMessage =>
      'Permita o CopyPaste em Ajustes do Sistema e depois volte.';

  @override
  String get onboardingMacosStateReadFailed =>
      'O CopyPaste não pôde ler o estado da configuração do macOS.';

  @override
  String get onboardingMacosAccessibilityRequestFailed =>
      'Não foi possível solicitar a permissão de Acessibilidade.';

  @override
  String get onboardingMacosStateRefreshFailed =>
      'O CopyPaste não pôde atualizar o estado da configuração do macOS.';

  @override
  String get onboardingMacosAccessibilityRefreshFailed =>
      'Não foi possível atualizar o status da Acessibilidade.';

  @override
  String get onboardingMacosLoginItemsOpenFailed =>
      'Não foi possível abrir os ajustes de Itens de Início.';

  @override
  String get onboardingMacosStartAtLoginApproval =>
      'A abertura ao iniciar sessão precisa de aprovação em Ajustes do Sistema.';

  @override
  String get onboardingMacosStartAtLoginDisableFailed =>
      'Não foi possível desativar a abertura ao iniciar sessão.';

  @override
  String get onboardingWindowsTitle => 'Configure seu PC';

  @override
  String get onboardingWindowsStartAtLoginUnavailable =>
      'Indisponível nesta instalação. Você pode continuar sem isso.';

  @override
  String get onboardingTaskManagerDisabledTitle =>
      'Desativado no Gerenciador de Tarefas';

  @override
  String get onboardingTaskManagerDisabledMessage =>
      'Ative o CopyPaste nos aplicativos de inicialização do Windows e depois volte.';

  @override
  String get onboardingWindowsTaskManagerDisabled =>
      'A abertura ao iniciar sessão está desativada no Gerenciador de Tarefas. Você pode ativá-la depois nas Configurações do Windows.';

  @override
  String get onboardingWindowsStartAtLoginUnavailableNotice =>
      'A abertura ao iniciar sessão não está disponível. Você pode continuar sem ela.';

  @override
  String get onboardingWindowsStateReadFailed =>
      'O CopyPaste não pôde ler o estado da configuração do Windows.';

  @override
  String get onboardingWindowsStartupSettingsFailed =>
      'Não foi possível abrir as configurações de inicialização do Windows.';

  @override
  String get onboardingWindowsStartAtLoginChangeLater =>
      'Não foi possível atualizar a abertura ao iniciar sessão. Você pode alterá-la depois em Configurações.';

  @override
  String get onboardingLinuxChecking =>
      'Verificando sua sessão da área de trabalho Linux.';

  @override
  String get onboardingLinuxTitle => 'Configure sua área de trabalho Linux';

  @override
  String get onboardingLinuxSubtitle =>
      'O CopyPaste precisa de uma integração verificada com a área de trabalho para identificar a origem do conteúdo copiado e para o Quick Paste.';

  @override
  String get onboardingLinuxSession => 'Sessão da área de trabalho';

  @override
  String get onboardingLinuxX11Detected => 'Sessão X11 detectada.';

  @override
  String get onboardingLinuxWaylandDetected => 'Sessão Wayland detectada.';

  @override
  String get onboardingLinuxSessionUnsupported =>
      'Nenhuma sessão de área de trabalho X11 ou Wayland compatível foi detectada.';

  @override
  String get onboardingLinuxCompanion => 'Complemento para GNOME ou KDE';

  @override
  String get onboardingLinuxCompanionReady =>
      'A ponte autenticada Clipboard v2 está pronta.';

  @override
  String get onboardingLinuxCompanionCapturePaused =>
      'A captura da área de transferência está pausada por causa dos apps ignorados. Ative o complemento assinado para retomá-la.';

  @override
  String get onboardingLinuxCompanionInstall =>
      'Instale e ative o pacote de sessão do complemento assinado e depois volte aqui.';

  @override
  String get onboardingLinuxOpenSetup => 'Abrir configuração';

  @override
  String get onboardingLinuxKeyboardControl => 'Controle do teclado';

  @override
  String get onboardingLinuxKeyboardAllowed =>
      'O controle do teclado está permitido para o Quick Paste.';

  @override
  String get onboardingLinuxKeyboardUnsupported =>
      'O controle do teclado não está disponível nesta sessão da área de trabalho.';

  @override
  String get onboardingLinuxAllowKeyboard => 'Permitir controle do teclado';

  @override
  String get onboardingLinuxQuickPasteReady => 'O Quick Paste está pronto.';

  @override
  String get onboardingLinuxQuickPastePending =>
      'O Quick Paste ficará pronto quando a integração necessária com a área de trabalho estiver ativa.';

  @override
  String get onboardingLinuxStartAtLoginDescription =>
      'Abrir o CopyPaste quando você iniciar sessão.';

  @override
  String get onboardingLinuxLinks => 'Links do CopyPaste';

  @override
  String get onboardingLinuxLinksRegistered =>
      'Os links do CopyPaste abrem neste app.';

  @override
  String get onboardingLinuxLinksUnregistered =>
      'Registre os links do CopyPaste para convites de pareamento.';

  @override
  String get onboardingLinuxRegisterLinks => 'Registrar links';

  @override
  String get onboardingLinuxScreenshotBlocking =>
      'Bloqueio de capturas de tela';

  @override
  String get onboardingLinuxScreenshotBlockingUnavailable =>
      'O bloqueio de capturas de tela não está disponível no Linux. Os dados armazenados protegidos continuam criptografados.';

  @override
  String get onboardingLinuxCheckAgain => 'Verificar novamente';

  @override
  String get onboardingLinuxStatusReadFailed =>
      'O CopyPaste não pôde ler o status da integração com o Linux.';

  @override
  String get onboardingLinuxSetupOpenFailed =>
      'Não foi possível abrir a configuração da integração com a área de trabalho.';

  @override
  String get onboardingLinuxKeyboardDenied =>
      'A permissão de controle do teclado não foi concedida.';

  @override
  String get onboardingLinuxStartAtLoginFailed =>
      'Não foi possível atualizar a abertura ao iniciar sessão.';

  @override
  String get onboardingLinuxLinksFailed =>
      'Não foi possível registrar os links do CopyPaste.';

  @override
  String get onboardingLinuxStatusRefreshFailed =>
      'Não foi possível atualizar o status da integração com o Linux.';

  @override
  String get onboardingLinuxX11ClipboardUnavailable =>
      'O acesso à área de transferência não está disponível nesta sessão X11.';

  @override
  String get onboardingLinuxSessionRequired =>
      'O CopyPaste requer uma sessão de área de trabalho X11 ou Wayland.';

  @override
  String get onboardingAndroidFullCapture => 'Captura completa';

  @override
  String get onboardingAndroidBackgroundCapture => 'Captura em segundo plano';

  @override
  String get onboardingAndroidFullCaptureDescription =>
      'Configure uma vez e depois verifique com uma cópia real.';

  @override
  String get onboardingAndroidLimitedCapture => 'Captura limitada';

  @override
  String get onboardingAndroidLimitedCaptureDescription =>
      'Compartilhe com o CopyPaste ou volte a ele após copiar.';

  @override
  String get onboardingAndroidNotification => 'Notificação de captura';

  @override
  String get onboardingAndroidNotificationDescription =>
      'Necessária enquanto a captura estiver em execução';

  @override
  String get onboardingAndroidBackgroundActivity =>
      'Atividade em segundo plano';

  @override
  String get onboardingAndroidBackgroundActivityDescription =>
      'Recomendado para reduzir perdas de captura';

  @override
  String get onboardingAndroidOpen => 'Abrir';

  @override
  String get onboardingAndroidApplyAccess => 'Aplicar acesso de captura';

  @override
  String get onboardingAndroidApplyAccessDescription =>
      'Aplique o acesso de captura uma única vez.';

  @override
  String get onboardingAndroidFullCaptureLater =>
      'Você pode configurar a Captura completa depois.';

  @override
  String get onboardingAndroidCaptureWorking =>
      'A captura em segundo plano está funcionando';

  @override
  String get onboardingAndroidVerifyCopy => 'Verifique com uma cópia real';

  @override
  String get onboardingAndroidCopyReached =>
      'Uma nova cópia chegou ao seu Histórico local.';

  @override
  String get onboardingAndroidCopyInstruction =>
      'Copie algo em outro app e depois volte aqui.';

  @override
  String get onboardingAndroidStartCapture => 'Iniciar captura';

  @override
  String get onboardingAndroidStateReadFailed =>
      'O CopyPaste não pôde ler o estado da configuração do Android.';

  @override
  String get onboardingAndroidStateRefreshFailed =>
      'Não foi possível atualizar o estado da captura do Android.';

  @override
  String get onboardingAndroidMonitoringUnavailable =>
      'O monitoramento da configuração do Android não está disponível.';

  @override
  String get onboardingAndroidIntakeFailed =>
      'Não foi possível ativar o recebimento da área de transferência.';

  @override
  String get onboardingAndroidStopCaptureFailed =>
      'Não foi possível parar a captura em segundo plano.';

  @override
  String get onboardingAndroidBatterySettingsFailed =>
      'Não foi possível abrir as configurações de bateria.';

  @override
  String get onboardingAndroidGrantsFailed =>
      'O CopyPaste está permitido no Shizuku, mas não foi possível aplicar as permissões de captura do Android.';

  @override
  String get onboardingAndroidAllowInShizuku =>
      'Permita o CopyPaste no Shizuku para aplicar as permissões de captura.';

  @override
  String get onboardingAndroidGrantAccessFirst =>
      'Conceda primeiro o acesso à área de transferência em segundo plano.';

  @override
  String get onboardingAndroidAllowNotificationsFirst =>
      'Permita as notificações antes de iniciar a captura em segundo plano.';

  @override
  String get onboardingAndroidStartCaptureFailed =>
      'Não foi possível iniciar a captura em segundo plano.';

  @override
  String get onboardingAndroidSetupOpenFailed =>
      'Não foi possível abrir a configuração de captura do Android.';

  @override
  String get onboardingAndroidChoiceSaveFailed =>
      'Não foi possível salvar a escolha de configuração do Android.';

  @override
  String get modulesMarketplaceTab => 'Loja';

  @override
  String get modulesInstalledTab => 'Instalados';

  @override
  String get modulesRefresh => 'Atualizar';

  @override
  String get modulesSearchPlaceholder => 'Buscar módulos';

  @override
  String get modulesOperationFailed => 'Falha na operação do módulo';

  @override
  String get modulesLoading => 'Carregando módulos.';

  @override
  String get modulesUnavailableTitle => 'Os módulos não estão disponíveis';

  @override
  String get modulesUnavailableMessage =>
      'Tente carregar os módulos novamente.';

  @override
  String get modulesNoneInstalled => 'Nenhum módulo instalado';

  @override
  String get modulesNoMatches => 'Nenhum módulo correspondente';

  @override
  String get modulesChooseFromMarketplace => 'Escolha um módulo na loja.';

  @override
  String get modulesTryDifferentSearch => 'Tente outra busca.';

  @override
  String get modulesMarketplaceLoading => 'Carregando a loja.';

  @override
  String get modulesMarketplaceUnavailableTitle => 'A loja não está disponível';

  @override
  String get modulesMarketplaceUnavailableMessage =>
      'Tente carregar a loja novamente.';

  @override
  String get modulesNoneAvailable => 'Nenhum módulo disponível';

  @override
  String get modulesNoneAvailableMessage =>
      'Os módulos aparecerão aqui quando forem publicados.';

  @override
  String modulesEnableModule(String module) {
    return 'Ativar $module';
  }

  @override
  String get modulesInstall => 'Instalar';

  @override
  String modulesUpdateTo(String version) {
    return 'Atualizar para $version';
  }

  @override
  String get modulesRemoveModule => 'Remover módulo';

  @override
  String get modulesModuleSettings => 'Configurações do módulo';

  @override
  String modulesSettingsTitle(String module) {
    return 'Configurações de $module';
  }

  @override
  String get modulesDownloadingModel => 'Baixando o modelo de idioma…';

  @override
  String get modulesRun => 'Executar';

  @override
  String modulesRemoveTitle(String module) {
    return 'Remover $module?';
  }

  @override
  String get modulesRemoveMessage =>
      'O módulo e suas configurações locais serão removidos.';

  @override
  String get modulesRemove => 'Remover';

  @override
  String get modulesRemoveFailed => 'Não foi possível remover o módulo.';

  @override
  String get modulesCloseSettings => 'Fechar configurações do módulo';

  @override
  String get modulesRemoved => 'Módulo removido';

  @override
  String get modulesNoSettingsTitle => 'Nenhuma configuração adicional';

  @override
  String get modulesNoSettingsMessage =>
      'Este módulo não precisa de configuração extra.';

  @override
  String get modulesSetUpSmsAccess => 'Configurar acesso a SMS';

  @override
  String get modulesRestartApp => 'Reiniciar o CopyPaste';

  @override
  String get modulesSmsAccessTitle => 'Acesso a SMS';

  @override
  String get modulesCloseSmsAccess => 'Fechar acesso a SMS';

  @override
  String get modulesSmsChecking => 'Verificando o acesso a SMS.';

  @override
  String get modulesSmsReady =>
      'O acesso está pronto. Ative o SMS Codes para começar a copiar novos códigos.';

  @override
  String get modulesSmsAllowAccess =>
      'Permita o acesso para copiar códigos de novas mensagens SMS.';

  @override
  String get modulesSmsNotification => 'Notificação de SMS';

  @override
  String get modulesSmsNotificationDescription =>
      'Necessária enquanto o SMS Codes estiver em execução';

  @override
  String get modulesSmsApplyAccess => 'Aplicar acesso a SMS';

  @override
  String get modulesSmsApplyAccessDescription =>
      'Aplique o acesso a SMS uma única vez.';

  @override
  String get modulesSmsSetupFailed => 'Falha na configuração de SMS';

  @override
  String get modulesFileSelectionFailed => 'Falha na seleção do arquivo';

  @override
  String get modulesChooseLanguages => 'Escolher idiomas';

  @override
  String get modulesChooseFile => 'Escolher arquivo';

  @override
  String modulesModelReady(String model) {
    return 'O modelo $model está pronto';
  }

  @override
  String modulesModelDownload(String model, String size) {
    return 'Modelo $model · download de $size MiB';
  }

  @override
  String get modulesPlatformsUnknown => 'Plataformas: desconhecidas';

  @override
  String modulesPlatforms(String platforms) {
    return 'Plataformas: $platforms';
  }

  @override
  String get modulesCatalogSignatureInvalid =>
      'A assinatura do catálogo de módulos é inválida.';

  @override
  String get modulesMarketplaceConnectFailed =>
      'Não foi possível conectar à loja de módulos.';

  @override
  String get modulesMarketplaceTimeout => 'A loja de módulos não respondeu.';

  @override
  String get modulesIncompatible =>
      'Este módulo não é compatível com este dispositivo.';

  @override
  String get modulesPackageSizeInvalid =>
      'O tamanho do pacote do módulo é inválido.';

  @override
  String get modulesDownloadInterrupted =>
      'O download do módulo foi interrompido.';

  @override
  String get modulesDownloadTimeout => 'O download do módulo não respondeu.';

  @override
  String get modulesPackageUrlUntrusted =>
      'O URL do pacote do módulo não é confiável.';

  @override
  String get modulesDownloadLeftGithub =>
      'O download do módulo saiu do GitHub.';

  @override
  String get modulesDownloadTooManyRedirects =>
      'O download do módulo usou redirecionamentos demais.';

  @override
  String get modulesDownloadTooLarge => 'O download do módulo é grande demais.';

  @override
  String get modulesPackageIntegrityFailed =>
      'O pacote do módulo não passou na verificação de integridade.';

  @override
  String get modulesDownloadUnavailable =>
      'O download deste módulo não está disponível no momento.';

  @override
  String get modulesMarketplaceUnavailableRetry =>
      'A loja de módulos não está disponível. Tente novamente.';

  @override
  String get modulesCatalogInvalid => 'O catálogo de módulos é inválido.';

  @override
  String get modulesThisSystem => 'este sistema';

  @override
  String modulesSystemRequirement(String system, String version) {
    return '$system $version ou mais recente';
  }

  @override
  String get modulesNotAvailableForDevice =>
      'Indisponível para este dispositivo.';

  @override
  String get modulesAppVersionUnverified =>
      'Não foi possível verificar a versão instalada do CopyPaste.';

  @override
  String modulesAppVersionUnsupported(String version) {
    return 'Este módulo não é compatível com o CopyPaste $version.';
  }

  @override
  String modulesRequires(String requirement) {
    return 'Requer $requirement.';
  }

  @override
  String get modulesMarketplaceLoadFailed =>
      'Não foi possível carregar a loja de módulos.';

  @override
  String get modulesSmsSetupRequired =>
      'Configure o acesso a SMS antes de ativar este módulo.';

  @override
  String get modulesSmsMonitoringStillEnabled =>
      'Não foi possível iniciar o monitoramento de SMS. O módulo continua ativado; desative-o novamente para tentar outra vez.';

  @override
  String get modulesSmsMonitoringDisabled =>
      'Não foi possível iniciar o monitoramento de SMS. O módulo foi desativado.';

  @override
  String get modulesSmsMonitoringUnconfirmed =>
      'Não foi possível iniciar o monitoramento de SMS. Não foi possível confirmar se o módulo está ativado.';

  @override
  String get modulesSmsMonitoringFailed =>
      'Não foi possível iniciar o monitoramento de SMS.';

  @override
  String get modulesSmsAndroidOnly =>
      'O acesso a SMS está disponível apenas no Android.';

  @override
  String get modulesSmsMonitoringNeedsSetup =>
      'O monitoramento de SMS precisa ser configurado.';

  @override
  String get modulesOperationIncomplete =>
      'Não foi possível concluir a operação do módulo.';

  @override
  String get modulesUnsupportedEvent => 'O módulo usa um evento incompatível.';

  @override
  String get modulesUnsupportedResult =>
      'O módulo retornou um resultado incompatível.';

  @override
  String get modulesUnsupportedField => 'O módulo usa um campo incompatível.';

  @override
  String get modulesResponseInvalid => 'A resposta do módulo é inválida.';

  @override
  String get modulesFileSelectionUnavailable =>
      'A seleção de arquivos não está disponível.';

  @override
  String get modulesFileSelectFailed =>
      'Não foi possível selecionar o arquivo.';

  @override
  String modulesFileNotAccepted(String field) {
    return '$field não é um arquivo aceito.';
  }

  @override
  String get modulesSmsVerifyFailed =>
      'Não foi possível verificar o acesso a SMS.';

  @override
  String get modulesSmsAllowNotifications =>
      'Permita as notificações para manter o SMS Codes ativo em segundo plano.';

  @override
  String get modulesSmsNotGranted => 'O acesso a SMS não foi concedido.';

  @override
  String get modulesSmsConfigureFailed =>
      'Não foi possível configurar o acesso a SMS.';

  @override
  String get onboardingLinuxContinueBasic => 'Continuar com a captura básica';

  @override
  String get onboardingLinuxCompanionOptional =>
      'Opcional. Necessário para o Quick Paste e para os apps ignorados.';

  @override
  String get onboardingLinuxKeyboardOptional =>
      'Opcional. Necessário para que o Quick Paste volte ao app anterior.';

  @override
  String get onboardingLinuxWaylandClipboardUnavailable =>
      'O acesso à área de transferência não está disponível nesta sessão Wayland. Ative o complemento assinado para GNOME ou KDE e verifique novamente.';

  @override
  String get historyMoreActions => 'Mais ações';

  @override
  String get historyTagsAction => 'Etiquetas…';

  @override
  String deviceAuthReasonUnlockSecrets(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'desbloquear itens confidenciais',
      'other': 'Desbloquear itens confidenciais',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonUseConcealedSnippet(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'usar um snippet oculto',
      'other': 'Usar um snippet oculto',
    });
    return '$_temp0';
  }

  @override
  String deviceAuthReasonCopySecret(String platform) {
    String _temp0 = intl.Intl.selectLogic(platform, {
      'macos': 'copiar um item confidencial',
      'other': 'Copiar um item confidencial',
    });
    return '$_temp0';
  }

  @override
  String get deviceAuthUnavailable => 'Indisponível neste sistema';

  @override
  String get deviceAuthNotConfigured =>
      'Primeiro configure uma senha, PIN ou biometria do dispositivo';

  @override
  String get commonOn => 'Ativado';

  @override
  String get commonOff => 'Desativado';

  @override
  String get settingsQuickPasteStartFailed =>
      'Não foi possível iniciar o Quick Paste, então o atalho dele não funciona.';

  @override
  String settingsQuickPasteShortcutInUse(String shortcut) {
    return '$shortcut já é usado por outro app. Escolha outro atalho.';
  }

  @override
  String get devicesEmptyTitle => 'Nenhum outro dispositivo pareado ainda';

  @override
  String get devicesEmptyMessage =>
      'Pareie outro dispositivo para manter o histórico da área de transferência sincronizado entre eles. Os dois dispositivos precisam estar na mesma rede, e cada pareamento é confirmado nos dois.';

  @override
  String get pairingStateNotCompleted => 'Pareamento não concluído';

  @override
  String get pairingStepInvite => 'Convite';

  @override
  String get pairingStepScan => 'Leitura';

  @override
  String get pairingStepCode => 'Código';

  @override
  String get pairingStepRequest => 'Solicitação';

  @override
  String get pairingStepConnect => 'Conexão';

  @override
  String get pairingStepConfirm => 'Confirmação';

  @override
  String get pairingStepPaired => 'Pareado';

  @override
  String get pairingOtherDevice => 'Outro dispositivo';

  @override
  String get pairingDecisionNotConfirmed => 'Ainda não confirmado';

  @override
  String get pairingDecisionSending => 'Enviando sua decisão…';

  @override
  String get pairingDecisionConfirmed => 'Confirmado';

  @override
  String get pairingDecisionRejected => 'Recusado';

  @override
  String get pairingDecisionWaiting => 'Aguardando confirmação';

  @override
  String get pairingDecisionUnknown => 'Resposta desconhecida';

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
      other: '$count linhas',
      many: '$count linhas',
      one: '$count linha',
    );
    return '$_temp0';
  }

  @override
  String get appNavigationLabel => 'Navegação principal';

  @override
  String sharedStepPosition(int step, int count, String name) {
    return '$step / $count · $name';
  }

  @override
  String sharedStepSemantics(int step, int count, String name) {
    return 'Etapa $step de $count: $name';
  }

  @override
  String get modulesPhaseDownload => 'Download';

  @override
  String get modulesPhaseInstall => 'Instalação';

  @override
  String modulesInstallingModule(String module) {
    return 'Instalando $module';
  }

  @override
  String modulesUpdatingModule(String module) {
    return 'Atualizando $module';
  }

  @override
  String get commonDismiss => 'Dispensar';

  @override
  String get settingsRetentionConfirmTitle =>
      'Remover o histórico mais antigo?';

  @override
  String settingsRetentionConfirmMessage(String limit) {
    return 'Novo limite: $limit. Os itens não fixados mais antigos são removidos deste dispositivo agora e não podem ser recuperados. Os itens fixados são mantidos.';
  }

  @override
  String get settingsStorageQuotaConfirmTitle =>
      'Reduzir a cota de armazenamento?';

  @override
  String settingsStorageQuotaConfirmMessage(String size) {
    return 'Nova cota: $size. Se o histórico não fixado ocupar mais, os itens mais antigos são removidos agora e não podem ser recuperados. Os itens fixados são mantidos.';
  }

  @override
  String get settingsLimitConfirmAction => 'Aplicar limite';

  @override
  String get settingsHideEverywhereTitle =>
      'Ocultar o CopyPaste completamente?';

  @override
  String get settingsHideEverywhereMessage =>
      'Com as duas opções desativadas, o CopyPaste não tem nenhum ícone visível. Para abri-lo de novo, inicie o CopyPaste novamente.';

  @override
  String settingsHideEverywhereShortcutMessage(String shortcut) {
    return 'Com as duas opções desativadas, o CopyPaste não tem nenhum ícone visível. Para abri-lo de novo, pressione $shortcut ou inicie o CopyPaste novamente.';
  }

  @override
  String get settingsHideEverywhereAction => 'Ocultar';

  @override
  String get settingsExportDoneTitle => 'Histórico de texto exportado';

  @override
  String get settingsBackupDoneTitle => 'Backup criado';

  @override
  String get settingsRestoreDoneTitle => 'Histórico restaurado';

  @override
  String settingsExportedTextClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clipes de texto exportados',
      many: '$count clipes de texto exportados',
      one: '$count clipe de texto exportado',
    );
    return '$_temp0';
  }

  @override
  String settingsExportUnreadableClips(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count não puderam ser lidos',
      many: '$count não puderam ser lidos',
      one: '$count não pôde ser lido',
    );
    return '$_temp0';
  }

  @override
  String settingsOutcomeDetail(String outcome, String detail) {
    return '$outcome · $detail';
  }

  @override
  String settingsBackupDoneMessage(String size) {
    return 'Backup criptografado salvo · $size';
  }

  @override
  String get settingsRestoreDoneMessage =>
      'O histórico da área de transferência deste dispositivo agora corresponde ao backup.';

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
  String get storageUsageOverQuota => 'Cota excedida';

  @override
  String storageUsageOldestClip(String date) {
    return 'Clipe mais antigo: $date';
  }

  @override
  String storageUsageDetail(String usage, String detail) {
    return '$usage · $detail';
  }

  @override
  String storageHeaderSemanticsLabel(String usage) {
    return 'Armazenamento: $usage';
  }

  @override
  String settingsStorageQuotaConfirmRemovesMessage(String size, String used) {
    return 'Nova cota: $size. O histórico não fixado ocupa $used, então os clipes mais antigos são removidos agora até caber. Eles não podem ser recuperados. Os itens fixados são mantidos.';
  }

  @override
  String historySectionClipCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clipes',
      many: '$count clipes',
      one: '$count clipe',
    );
    return '$_temp0';
  }

  @override
  String get trayOpenApp => 'Abrir o CopyPaste';

  @override
  String get trayOpenQuickPaste => 'Abrir o Quick Paste';

  @override
  String trayOpenQuickPasteWithShortcut(String shortcut) {
    return 'Abrir o Quick Paste ($shortcut)';
  }

  @override
  String get trayNewSnippet => 'Novo trecho';

  @override
  String get trayRecent => 'Recentes';

  @override
  String get trayRecentLoading => 'Carregando…';

  @override
  String get trayRecentFailed => 'Não foi possível carregar os clipes';

  @override
  String get trayRecentEmpty => 'Ainda não há clipes';

  @override
  String trayRecentHidden(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count clipes recentes ocultos',
      one: '1 clipe recente oculto',
    );
    return '$_temp0';
  }

  @override
  String get trayRecentSecret => 'Secreto';

  @override
  String get trayCopyLatestFromOtherDevice =>
      'Copiar o mais recente de outro dispositivo';

  @override
  String get trayCaptureNow => 'Capturar agora';

  @override
  String trayPausedUntil(String time) {
    return 'Pausado até $time';
  }

  @override
  String get trayPauseFor => 'Pausar por…';

  @override
  String get trayPauseFor15Minutes => '15 minutos';

  @override
  String get trayPauseForOneHour => '1 hora';

  @override
  String get trayPauseUntilResumed => 'Até eu retomar';

  @override
  String trayPasteQueue(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Paste Queue: $count itens',
      one: 'Paste Queue: 1 item',
    );
    return '$_temp0';
  }

  @override
  String trayPasteQueueCollecting(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Coletando: $count itens',
      one: 'Coletando: 1 item',
    );
    return '$_temp0';
  }

  @override
  String trayRecentlySkipped(int count) {
    return 'Ignorados recentemente ($count)';
  }

  @override
  String get trayOpenContentRules => 'Abrir regras de conteúdo';

  @override
  String get trayRecentSettingsTitle => 'Barra de menus e bandeja';

  @override
  String get trayRecentSettingsDescription =>
      'O que o menu da barra de menus ou da bandeja mostra em Recentes.';

  @override
  String get trayRecentCountTitle => 'Clipes recentes';

  @override
  String get trayRecentCountDescription =>
      'Quantos dos clipes mais novos o menu mostra.';

  @override
  String get trayRecentPreviewTitle => 'Prévia dos clipes recentes';

  @override
  String get trayRecentPreviewDescription =>
      'O sistema guarda o texto dos menus fora da criptografia do CopyPaste. Mostre menos para manter o conteúdo dos clipes fora dele. Segredos são sempre mascarados.';

  @override
  String get trayRecentPreviewFull => 'Prévia completa';

  @override
  String get trayRecentPreviewTitles => 'Somente tipo e origem';

  @override
  String get trayRecentPreviewHidden => 'Oculto';

  @override
  String get trayRecentSettingsKeywords =>
      'bandeja barra de menus recentes prévia privacidade quantidade';

  @override
  String get transformsPaletteTitle => 'Transformar texto';

  @override
  String get transformsFilterPlaceholder => 'Filtrar transformações';

  @override
  String get transformsNoMatches => 'Nenhuma transformação corresponde';

  @override
  String get transformsGroupResults => 'Resultados';

  @override
  String get transformsGroupSuggested => 'Sugeridas';

  @override
  String get transformsGroupRecent => 'Recentes';

  @override
  String get transformsGroupFavorites => 'Favoritas';

  @override
  String get transformsGroupPipelines => 'Pipelines';

  @override
  String get transformsCategoryStructure => 'JSON, YAML, CSV e XML';

  @override
  String get transformsCategoryEncode => 'Codificar e decodificar';

  @override
  String get transformsCategoryCase => 'Maiúsculas e minúsculas';

  @override
  String get transformsCategoryHash => 'Hashes';

  @override
  String get transformsCategoryLines => 'Linhas e espaços';

  @override
  String get transformsCategoryClean => 'Limpar';

  @override
  String get transformsCategoryUrl => 'Links';

  @override
  String get transformsCategoryMarkup => 'Texto formatado e Markdown';

  @override
  String get transformsCategoryDev => 'Desenvolvedor';

  @override
  String get transformsCategoryOther => 'Outras';

  @override
  String get transformsChainEmpty => 'Nenhuma etapa ainda';

  @override
  String transformsStepLabel(int step, String name) {
    return '$step · $name';
  }

  @override
  String transformsThenLabel(String name) {
    return 'Depois: $name';
  }

  @override
  String get transformsStepEnabled => 'Executar esta etapa';

  @override
  String get transformsMoveEarlier => 'Mover para cima';

  @override
  String get transformsMoveLater => 'Mover para baixo';

  @override
  String get transformsRemoveStep => 'Remover etapa';

  @override
  String get transformsSavePipeline => 'Salvar como pipeline';

  @override
  String get transformsSavePipelineTitle => 'Salvar pipeline';

  @override
  String get transformsSavePipelineFailed =>
      'Não foi possível salvar o pipeline.';

  @override
  String get transformsPipelineNameLabel => 'Nome';

  @override
  String get transformsPipelineDescriptionLabel => 'Descrição (opcional)';

  @override
  String get transformsPipelineStepsLabel => 'Etapas';

  @override
  String get transformsPipelineNoSteps => 'Adicione pelo menos uma etapa.';

  @override
  String get transformsPipelineNameRequired => 'Digite um nome.';

  @override
  String get transformsPipelineNameTooLong => 'Use no máximo 80 caracteres.';

  @override
  String get transformsPipelineNameTaken => 'Outro pipeline já tem este nome.';

  @override
  String get transformsPipelineSavedTitle => 'Pipeline salvo';

  @override
  String transformsPipelineSavedMessage(String name) {
    return '$name está disponível em todas as paletas de transformação.';
  }

  @override
  String transformsPipelineSteps(int count) {
    return 'Etapas: $count';
  }

  @override
  String transformsPipelineCopyName(String name, int index) {
    return '$name ($index)';
  }

  @override
  String get transformsRemovedPipeline => 'Pipeline excluído';

  @override
  String get transformsNeedsNewerVersion =>
      'Requer uma versão mais recente do CopyPaste';

  @override
  String get transformsLoadFailed =>
      'Não foi possível carregar as transformações.';

  @override
  String get transformsNotTextTitle => 'As transformações funcionam com texto';

  @override
  String get transformsNotTextMessage =>
      'Este item é uma imagem, um arquivo ou uma cor. Escolha um item de texto ou copie um texto.';

  @override
  String get transformsNothingTitle => 'Nada para transformar';

  @override
  String get transformsNothingMessage =>
      'Copie um texto ou escolha um item do histórico.';

  @override
  String get transformsPickTransform => 'Escolha uma transformação';

  @override
  String get transformsPickTransformHint =>
      'Tab adiciona como etapa. Enter aplica o resultado.';

  @override
  String get transformsPreviewing => 'Atualizando prévia…';

  @override
  String transformsCounts(int lines, int words, int chars) {
    return 'Linhas: $lines · Palavras: $words · Caracteres: $chars';
  }

  @override
  String get transformsNoChanges => 'Sem alterações';

  @override
  String transformsTruncated(String size) {
    return 'Mostrando os primeiros 64 KiB de $size';
  }

  @override
  String get transformsRandom => 'Aleatório';

  @override
  String get transformsNotRun => 'Não executada';

  @override
  String get transformsSensitiveHidden =>
      'O resultado é confidencial e está oculto.';

  @override
  String get transformsReveal => 'Mostrar';

  @override
  String get transformsPaneOutput => 'Saída';

  @override
  String get transformsPaneInput => 'Entrada';

  @override
  String get transformsPaneChanges => 'Alterações';

  @override
  String get transformsCannotTransform =>
      'Não é possível transformar este texto';

  @override
  String transformsStepFailed(int step, String name, String message) {
    return 'Etapa $step · $name: $message';
  }

  @override
  String get transformsUseHexOutput => 'Mostrar em hexadecimal';

  @override
  String get transformsActionPaste => 'Colar';

  @override
  String get transformsActionSave => 'Salvar como novo item';

  @override
  String get transformsCopiedMessage =>
      'O resultado está na área de transferência e no histórico.';

  @override
  String get transformsSavedTitle => 'Salvo';

  @override
  String get transformsSavedMessage =>
      'O resultado é um novo item do histórico. A área de transferência não mudou.';

  @override
  String get transformsPasteFailedTitle => 'Não foi possível colar';

  @override
  String get transformsPasteFailedMessage =>
      'O resultado está na área de transferência. Cole manualmente.';

  @override
  String transformsErrorAt(String message, int line, int column) {
    return '$message na linha $line, coluna $column';
  }

  @override
  String get transformsErrorBinary =>
      'Os dados decodificados são binários, não texto';

  @override
  String get transformsErrorTooLarge => 'Grande demais para transformar';

  @override
  String get transformsErrorInvalidParam =>
      'Uma configuração desta etapa não é válida';

  @override
  String get transformsErrorCancelled => 'Cancelado';

  @override
  String get transformsErrorTooExpensive => 'Padrão custoso demais';

  @override
  String get transformsErrorPattern => 'O padrão não é válido';

  @override
  String get transformsErrorTimestampUnit =>
      'Escolha se o número está em segundos ou milissegundos';

  @override
  String get transformsErrorExternalEntity =>
      'Entidades externas não são permitidas';

  @override
  String get transformsErrorJwtEncrypted =>
      'Tokens criptografados não podem ser decodificados';

  @override
  String get transformsErrorJson => 'JSON inválido';

  @override
  String get transformsErrorYaml => 'YAML inválido';

  @override
  String get transformsErrorXml => 'XML inválido';

  @override
  String get transformsErrorCsv => 'CSV inválido';

  @override
  String get transformsErrorBase64 => 'Base64 inválido';

  @override
  String get transformsErrorHex => 'Hexadecimal inválido';

  @override
  String get transformsErrorEscape => 'Sequência de escape inválida';

  @override
  String get transformsErrorUrl => 'Codificação percentual malformada';

  @override
  String get transformsErrorJwt => 'Não é um JWT válido';

  @override
  String get transformsErrorDate =>
      'Data ou carimbo de data/hora não reconhecido';

  @override
  String get transformsErrorGeneric =>
      'Esta etapa não consegue transformar sua entrada';

  @override
  String get transformsWarningSignedLink => 'Link assinado mantido inalterado';

  @override
  String get transformsWarningExpired => 'Expirado';

  @override
  String get transformsWarningNotYetValid => 'Ainda não é válido';

  @override
  String get transformsWarningSignatureNotVerified =>
      'Assinatura não verificada';

  @override
  String get transformsWarningDuplicateKeys => 'Chaves duplicadas';

  @override
  String get transformsWarningLenientJson => 'JSON fora do padrão aceito';

  @override
  String get transformsWarningDoubleEncoded =>
      'O JSON estava codificado duas vezes';

  @override
  String get transformsWarningEscapes =>
      'Sequências de escape desconhecidas mantidas como escritas';

  @override
  String get transformsWarningLocalZone => 'Fuso horário local presumido';

  @override
  String get transformsWarningNoMatches => 'Nenhuma correspondência';

  @override
  String get transformsWarningGeneric => 'Verifique o resultado';

  @override
  String get transformsRestoreTitle =>
      'Restaurar a área de transferência após colar';

  @override
  String get transformsRestoreDescription =>
      'Após colar um resultado transformado, a área de transferência anterior volta, a menos que outra coisa tenha sido copiada.';

  @override
  String get transformsDefaultTitle => 'Transformação padrão ao colar';

  @override
  String get transformsDefaultDescription =>
      'O Quick Paste aplica este pipeline ao texto e o mostra antes de colar. Se falhar, cola o original.';

  @override
  String get transformsNoDefault => 'Nenhuma';

  @override
  String get transformsAppDefaultsTitle => 'Padrões por app';

  @override
  String get transformsAppDefaultsDescription =>
      'Substitua o padrão de um app pelo ID do pacote, executável ou nome do pacote. Entradas de outra plataforma sincronizam, mas não têm efeito aqui.';

  @override
  String get transformsAppIdentifierPlaceholder => 'Identificador do app';

  @override
  String transformsOtherPlatform(String platform) {
    return 'somente em $platform';
  }

  @override
  String get transformsAdd => 'Adicionar';

  @override
  String get transformsRemove => 'Remover';

  @override
  String get transformsFavoritesTitle => 'Favoritas';

  @override
  String get transformsFavoritesDescription =>
      'As favoritas aparecem primeiro em todas as paletas. As nove primeiras rodam com Mod+1 a Mod+9.';

  @override
  String transformsFavoriteShortcut(int number) {
    return 'Mod+$number na paleta';
  }

  @override
  String get transformsAddFavorite => 'Adicionar favorita';

  @override
  String get transformsPipelinesTitle => 'Pipelines';

  @override
  String get transformsPipelinesDescription =>
      'Cadeias salvas de transformações com suas configurações. Elas sincronizam com seus outros dispositivos.';

  @override
  String get transformsNewPipeline => 'Novo pipeline';

  @override
  String get transformsEditPipeline => 'Editar pipeline';

  @override
  String get transformsDuplicatePipeline => 'Duplicar';

  @override
  String get transformsDeletePipeline => 'Excluir';

  @override
  String transformsDeletePipelineTitle(String name) {
    return 'Excluir $name?';
  }

  @override
  String get transformsDeletePipelineMessage =>
      'Favoritas e padrões de colagem que o usam voltam para nenhum em todos os dispositivos.';

  @override
  String get transformsDeletePipelineFailed =>
      'Não foi possível excluir o pipeline.';

  @override
  String get transformsAddStep => 'Adicionar etapa';

  @override
  String get transformsHiddenTitle => 'Transformações ocultas';

  @override
  String get transformsHiddenDescription =>
      'Transformações ocultas não aparecem nas paletas nem nas sugestões. Pipelines que as usam continuam funcionando.';

  @override
  String get transformsHideTransform => 'Ocultar uma transformação';

  @override
  String get transformsShow => 'Mostrar';

  @override
  String get transformsSuggestionsTitle => 'Sugestões';

  @override
  String get transformsSuggestionsDescription =>
      'Ao abrir uma paleta, os primeiros 64 KiB do texto são verificados quanto aos tipos abaixo para sugerir até três transformações. Nada roda em segundo plano.';

  @override
  String get transformsDetectorJson => 'JSON';

  @override
  String get transformsDetectorJwt => 'JSON Web Tokens';

  @override
  String get transformsDetectorBase64 => 'Base64';

  @override
  String get transformsDetectorTrackingUrl =>
      'Links com parâmetros de rastreamento';

  @override
  String get transformsDetectorTimestamp => 'Carimbos de data/hora';

  @override
  String get transformsDetectorHtml => 'HTML';

  @override
  String get transformsDetectorMarkdown => 'Markdown';

  @override
  String get transformsTrackingTitle => 'Parâmetros de rastreamento';

  @override
  String get transformsTrackingDescription =>
      'A lista integrada vem com o app e só muda com atualizações. Adicione nomes para remover ou manter; escreva nome@domínio para limitar a um domínio.';

  @override
  String get transformsTrackingRemoveLabel => 'Remover também';

  @override
  String get transformsTrackingAllowLabel => 'Sempre manter';

  @override
  String get transformsTrackingPlaceholder => 'nome ou nome@domínio e Enter';
}
