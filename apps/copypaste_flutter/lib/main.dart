import 'dart:async';
import 'dart:io';

import 'app/localization/app_language.dart';
import 'l10n/l10n.dart';

import 'platform/lifecycle/application_restarter.dart';

import 'features/modules/controller/modules_controller.dart';
import 'features/modules/controller/module_app_update_preparer.dart';
import 'features/modules/repository/module_update_stager.dart';
import 'features/modules/repository/runtime_modules_repository.dart';
import 'features/modules/repository/file_selector_module_input_picker.dart';
import 'features/modules/repository/github_module_marketplace_repository.dart';
import 'platform/modules/module_marketplace_platform.dart';
import 'platform/modules/android_module_access.dart';

import 'features/transfers/file_operations_controller.dart';
import 'features/transfers/runtime_file_operations_repository.dart';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app/app.dart';
import 'app/app_features.dart';
import 'app/ui_memory_controller.dart';
import 'app/resource_policy.dart';
import 'platform/lifecycle/application_visibility.dart';
import 'app/navigation/navigation.dart';
import 'app/navigation/settings_presentation_lifecycle.dart';
import 'app/shell/macos_window_header.dart';
import 'app/theme/app_motion.dart';
import 'app/theme/app_theme.dart';
import 'features/ai_tools/controller/ai_approval_controller.dart';
import 'features/ai_tools/controller/ai_tools_controller.dart';
import 'features/ai_tools/repository/ai_approval_notifier.dart';
import 'features/ai_tools/repository/runtime_ai_tools_repository.dart';
import 'features/content_rules/controller/content_rules_controller.dart';
import 'features/content_rules/repository/runtime_content_rule_repository.dart';
import 'features/devices/devices.dart';
import 'features/notifications/notifications_controller.dart';
import 'platform/notifications/pairing_notification_port.dart';
import 'features/devices/flutter_rust_devices_gateway.dart';
import 'features/history/controller/history_controller.dart';
import 'features/history/controller/history_ocr_controller.dart';
import 'features/history/repository/file_selector_history_file_downloader.dart';
import 'platform/files/history_file_picker.dart';
import 'platform/files/temporary_artifacts.dart';
import 'features/history/repository/runtime_history_repository.dart';
import 'features/history/repository/temporary_history_image_input.dart';
import 'features/onboarding/controller/onboarding_controller.dart';
import 'features/onboarding/controller/platform_onboarding_setup.dart';
import 'features/onboarding/view/onboarding_screen.dart';
import 'features/paste_queue/controller/paste_queue_controller.dart';
import 'features/paste_queue/controller/paste_queue_shortcuts_controller.dart';
import 'features/paste_queue/repository/paste_queue_shortcuts_store.dart';
import 'features/paste_queue/repository/runtime_paste_queue_repository.dart';
import 'features/paste_queue/repository/system_paste_queue_notifier.dart';
import 'features/quick_access/controller/quick_access_controller.dart';
import 'features/quick_paste/quick_paste_app.dart';
import 'features/quick_paste/quick_paste_controller.dart';
import 'features/quick_paste/repository/runtime_quick_paste_delivery_repository.dart';
import 'features/snippets/controller/concealed_snippet_access.dart';
import 'features/snippets/controller/snippets_controller.dart';
import 'features/snippets/repository/runtime_snippet_repository.dart';
import 'features/transforms/controller/transforms_settings_controller.dart';
import 'features/transforms/repository/runtime_transform_repository.dart';
import 'features/transforms/repository/transform_recents_store.dart';
import 'features/tray/controller/tray_menu_controller.dart';
import 'platform/security/device_authenticator.dart';
import 'features/settings/controller/quick_paste_settings_controller.dart';
import 'features/settings/controller/settings_navigation_state.dart';
import 'features/settings/controller/settings_controller.dart';
import 'features/settings/controller/launch_at_login_controller.dart';
import 'platform/startup/launch_at_login.dart';
import 'features/settings/repository/file_selector_settings_file_picker.dart';
import 'features/settings/repository/quick_paste_preferences_store.dart';
import 'features/settings/repository/runtime_settings_repository.dart';
import 'features/update/update.dart';
import 'generated/frb_generated.dart';
import 'generated/api.dart' as runtime;
import 'platform/desktop/desktop_window_bootstrap.dart';
import 'platform/desktop/desktop_window_controller.dart';
import 'platform/desktop/global_shortcut.dart';
import 'platform/android/android_quick_access_gateway.dart';
import 'platform/desktop/quick_paste_host.dart';
import 'platform/pairing/pairing_presentation.dart';
import 'platform/permissions/linux_integration.dart';
import 'platform/update/app_update_platform.dart';
import 'shared/state_view.dart';

Future<void> main([List<String> arguments = const []]) async {
  if (Platform.isLinux && arguments.contains('--copypaste-quick-paste')) {
    await quickPasteMain();
    return;
  }
  WidgetsFlutterBinding.ensureInitialized();
  await AppResourcePolicy.configure();
  AppMotion.configureLibrary();
  await RustLib.init();
  final desktopWindow = await initializeDesktopWindow();
  final language = await _initializeLanguage();
  runApp(
    CopyPasteRoot(
      desktopWindow: desktopWindow,
      language: language,
      appUpdateController: AppUpdateController(
        repository: GitHubAppUpdateRepository(
          temporaryDirectory: getTemporaryDirectory,
        ),
        platform: MethodChannelAppUpdatePlatform(),
        restart: const ApplicationRestarter().restart,
        prepareModules: ModuleAppUpdatePreparer(
          installed: RuntimeModulesRepository(),
          marketplace: (version) => GitHubModuleMarketplaceRepository(
            temporaryDirectory: getTemporaryDirectory,
            currentTarget: () =>
                ModuleMarketplacePlatform().currentTarget(appVersion: version),
          ),
          stager: RuntimeModuleUpdateStager(),
        ).prepare,
      ),
    ),
  );
}

@pragma('vm:entry-point')
Future<void> quickPasteMain() async {
  WidgetsFlutterBinding.ensureInitialized();
  await AppResourcePolicy.configure();
  AppMotion.configureLibrary();
  await RustLib.init();
  final host = MethodChannelQuickPasteContextHost();
  if (Platform.isLinux) {
    await runtime.attachDesktopRuntime(endpoint: await host.runtimeEndpoint());
  }
  QuickPasteController createController() {
    final repository = RuntimeHistoryRepository();
    final modules = ModulesController(
      repository: RuntimeModulesRepository(),
      marketplace: GitHubModuleMarketplaceRepository(
        temporaryDirectory: getTemporaryDirectory,
        currentTarget: ModuleMarketplacePlatform().currentTarget,
      ),
    );
    return QuickPasteController(
      repository: repository,
      fileOperations: FileOperationsController(
        const RuntimeFileOperationsRepository(),
      ),
      disposeRepository: () async {
        modules.dispose();
        await repository.dispose();
      },
      preferencesStore: SharedPreferencesQuickPastePreferencesStore(),
      host: host,
      fileDownloader: const FileSelectorHistoryFileDownloader(),
      ocr: HistoryOcrController(
        copyText: repository.copyText,
        modules: modules,
        imageInput: TemporaryHistoryImageInput(repository: repository),
      ),
      initializeInspector: modules.initialize,
      snippets: RuntimeSnippetRepository(),
      transforms: RuntimeTransformRepository(),
      transformRecents: SharedPreferencesTransformRecentsStore(),
      delivery: const RuntimeQuickPasteDeliveryRepository(),
      authenticator: MethodChannelDeviceAuthenticator(),
      createQueue: (hideWindow) {
        final queue = PasteQueueController(
          repository: RuntimePasteQueueRepository(),
          pasteHost: host,
          requestAccessibility: host.requestAccessibility,
          hideWindow: hideWindow,
        );
        unawaited(
          SharedPreferencesPasteQueueShortcutsStore().read().then(
            (preferences) => queue.pasteNextStyle = preferences.pasteNextStyle,
            onError: (Object _) {},
          ),
        );
        return queue;
      },
    );
  }

  final language = await _initializeLanguage();
  runApp(QuickPasteApp(controller: createController(), language: language));
}

Future<AppLanguageController> _initializeLanguage() async {
  final language = AppLanguageController(
    store: SharedPreferencesAppLanguageStore(),
    platform: const MethodChannelAppLanguagePlatform(),
  );
  await language.initialize();
  return language;
}

const _pairingLinksChannel = MethodChannel('com.copypaste.app/pairing_links');

Future<void> _startRuntimeProcess() async {
  if (Platform.isAndroid) {
    await runtime.runtimeStatus();
    return;
  }
  if (!Platform.isMacOS && !Platform.isWindows && !Platform.isLinux) return;
  final daemonName = Platform.isWindows
      ? 'copypaste-daemon.exe'
      : 'copypaste-daemon';
  final daemon = File(
    '${File(Platform.resolvedExecutable).parent.path}/$daemonName',
  );
  if (!await daemon.exists()) {
    throw StateError('The CopyPaste runtime helper is unavailable.');
  }
  final supportDirectory = await getApplicationSupportDirectory();
  final runtimeDirectory = kDebugMode ? 'development-runtime' : 'runtime';
  final dataDir = Directory('${supportDirectory.path}/$runtimeDirectory');
  await runtime.startDesktopRuntime(
    daemonExecutable: daemon.path,
    dataDir: dataDir.path,
  );
}

enum _RuntimeState { idle, starting, ready, failed }

/// Owns process-wide application state and the immutable desktop controller.
class CopyPasteRoot extends StatefulWidget {
  const CopyPasteRoot({
    super.key,
    this.desktopWindow,
    this.onboardingController,
    this.appUpdateController,
    this.runtimeEnabled = true,
    this.language,
  });

  final DesktopWindowController? desktopWindow;
  final AppLanguageController? language;
  final OnboardingController? onboardingController;
  final AppUpdateController? appUpdateController;
  final bool runtimeEnabled;

  @override
  State<CopyPasteRoot> createState() => _CopyPasteRootState();
}

class _CopyPasteRootState extends State<CopyPasteRoot> {
  final _startupGateway = platformLaunchAtLoginGateway();
  late final _launchAtLogin = LaunchAtLoginController(_startupGateway);
  late final AppNavigationController _navigation = AppNavigationController();
  late final DesktopWindowController? _desktopWindow = widget.desktopWindow;
  late final AppUpdateController? _appUpdateController =
      widget.appUpdateController;
  RuntimeHistoryRepository? _historyRepository;
  HistoryController? _historyController;
  FlutterRustDevicesGateway? _devicesGateway;
  DevicesController? _devicesController;
  NotificationsController? _notifications;
  QuickPasteSettingsController? _quickPasteSettings;
  MethodChannelQuickPasteWindowHost? _quickPasteHost;
  DesktopShortcutRegistrar? _shortcutRegistrar;
  AppFeatures? _features;
  SettingsController? _settingsController;
  ModulesController? _modulesController;
  OnboardingController? _onboarding;
  bool _ownsOnboarding = false;
  bool _onboardingReady = false;
  _RuntimeState _runtimeState = _RuntimeState.idle;
  String? _runtimeFailureMessage;
  String? _pendingPairingUri;
  final _rootNavigatorKey = GlobalKey<NavigatorState>();
  final _temporaryArtifacts = TemporaryArtifactCleanup();
  late final ApplicationVisibility _visibility;
  late final SettingsPresentationLifecycle _settingsPresentation;
  late final UiMemoryController _uiMemory;

  @override
  void initState() {
    super.initState();
    widget.language?.addListener(_languageChanged);
    unawaited(_launchAtLogin.refresh());
    if (widget.runtimeEnabled) unawaited(_temporaryArtifacts.run());
    _visibility = ApplicationVisibility(
      windowVisible: _desktopWindow?.isVisible,
    );
    _visibility.addListener(_settingsVisibilityChanged);
    _settingsPresentation = SettingsPresentationLifecycle(
      navigation: _navigation,
      visible: _visibility,
      setActive: (active) => _settingsController?.setPresentationActive(active),
    );
    _uiMemory = UiMemoryController(
      visible: _visibility,
      canSuspend: _canSuspendPresentation,
    )..addListener(_memoryStateChanged);
    _pairingLinksChannel.setMethodCallHandler(_handlePairingLinkCall);
    unawaited(_takePendingPairingLink());
    if (!Platform.isMacOS) {
      _desktopWindow?.setBeforeQuit(_prepareForTermination);
    }
    _desktopWindow?.setOpenSettings(() {
      _navigation.selectDestination(AppDestination.settings);
    });
    _configureOnboarding();
    unawaited(_appUpdateController?.initialize());
    if (widget.runtimeEnabled) {
      unawaited(_startRuntime());
    } else {
      _runtimeState = _RuntimeState.ready;
    }
  }

  @override
  void didUpdateWidget(covariant CopyPasteRoot oldWidget) {
    super.didUpdateWidget(oldWidget);
    assert(
      oldWidget.desktopWindow == widget.desktopWindow,
      'CopyPasteRoot cannot replace its process-wide desktop window controller.',
    );
    assert(
      oldWidget.onboardingController == widget.onboardingController,
      'CopyPasteRoot cannot replace its onboarding controller.',
    );
    assert(
      oldWidget.appUpdateController == widget.appUpdateController,
      'CopyPasteRoot cannot replace its process-wide update controller.',
    );
  }

  @override
  void dispose() {
    widget.language?.removeListener(_languageChanged);
    _uiMemory.removeListener(_memoryStateChanged);
    _uiMemory.dispose();
    _visibility.removeListener(_settingsVisibilityChanged);
    _settingsPresentation.dispose();
    _visibility.dispose();
    _launchAtLogin.dispose();
    _pairingLinksChannel.setMethodCallHandler(null);
    _desktopWindow?.setOpenSettings(null);
    if (_ownsOnboarding) _onboarding?.dispose();
    _navigation.dispose();
    _appUpdateController?.dispose();
    unawaited(_prepareForTermination());
    _disposeDesktopWindow(_desktopWindow);
    super.dispose();
  }

  bool _canSuspendPresentation() =>
      _runtimeState == _RuntimeState.ready &&
      _onboardingReady &&
      (_onboarding?.complete ?? true) &&
      (_historyController?.canSuspend ?? true) &&
      !(_features?.busy ?? false) &&
      _devicesController?.pairingEntryMode == null &&
      !(_devicesController?.systemScanInFlight ?? false) &&
      !(_devicesController?.actionInFlight ?? false) &&
      !(_settingsController?.busy ?? false) &&
      !(_modulesController?.busy ?? false) &&
      !(_appUpdateController?.busy ?? false) &&
      !_navigation.bottomOverlayOpen &&
      !(_navigation.navigatorKey.currentState?.canPop() ?? false) &&
      !(_rootNavigatorKey.currentState?.canPop() ?? false);

  void _settingsVisibilityChanged() {
    if (widget.runtimeEnabled && _visibility.value) {
      unawaited(_temporaryArtifacts.run());
    }
  }

  void _languageChanged() {
    if (mounted) setState(() {});
  }

  void _memoryStateChanged() {
    if (!mounted) return;
    if (_uiMemory.suspended) {
      _historyController?.suspend();
    } else {
      unawaited(AppResourcePolicy.resumePresentation());
    }
    setState(() {});
    if (_uiMemory.suspended) {
      // Dispose image listeners before releasing the shared caches.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && _uiMemory.suspended) {
          unawaited(AppResourcePolicy.suspendPresentation());
        }
      });
      // Hidden/paused lifecycle states disable ordinary frames. One warm-up
      // frame disposes presentation now instead of retaining it until reopening.
      WidgetsBinding.instance.scheduleWarmUpFrame();
    }
  }

  Future<void> _startRuntime() async {
    if (!widget.runtimeEnabled || _runtimeState == _RuntimeState.starting) {
      return;
    }
    setState(() {
      _runtimeState = _RuntimeState.starting;
      _runtimeFailureMessage = null;
    });
    try {
      await _startRuntimeProcess();
      if (Platform.isLinux && mounted) {
        final endpoint = await runtime.desktopRuntimeEndpoint();
        await MethodChannelLinuxIntegrationPort().publishRuntimeEndpoint(
          endpoint,
        );
      }
      if (!mounted) {
        if (Platform.isMacOS || Platform.isWindows || Platform.isLinux) {
          await runtime.stopDesktopRuntime();
        }
        return;
      }
      setState(() {
        _historyRepository = RuntimeHistoryRepository();
        _devicesGateway = FlutterRustDevicesGateway();
        _devicesController = DevicesController(
          gateway: _devicesGateway!,
          captureProtection: MethodChannelPairingCaptureProtection(),
        );
        _modulesController = ModulesController(
          access: Platform.isAndroid ? const AndroidModuleAccess() : null,
          repository: RuntimeModulesRepository(),
          marketplace: GitHubModuleMarketplaceRepository(
            temporaryDirectory: getTemporaryDirectory,
            currentTarget: ModuleMarketplacePlatform().currentTarget,
          ),
          inputPicker: const FileSelectorModuleInputPicker(),
          restart: () async {
            if (Platform.isAndroid) {
              await _prepareForTermination();
              await const ApplicationRestarter().restart();
            } else {
              await _stopRuntime();
              await _startRuntime();
            }
          },
        );
        _historyController = HistoryController(
          _historyRepository!,
          includeGroupCounts: true,
          fileOperations: FileOperationsController(
            const RuntimeFileOperationsRepository(),
          ),
          fileDownloader: const FileSelectorHistoryFileDownloader(),
          filePicker: const SystemHistoryFilePicker(),
          ocr: HistoryOcrController(
            copyText: _historyRepository!.copyText,
            modules: _modulesController!,
            imageInput: TemporaryHistoryImageInput(
              repository: _historyRepository!,
            ),
          ),
        );
        _settingsController = SettingsController(
          repository: RuntimeSettingsRepository(),
          filePicker: const FileSelectorSettingsFilePicker(),
          backgroundCaptureStatusRequired: _desktopWindow != null,
        );
        _createQuickPaste();
        _features = _createFeatures();
      });
      final settingsController = _settingsController!;
      final features = _features!;
      _settingsVisibilityChanged();
      _settingsPresentation.refresh();
      settingsController.addListener(_syncCaptureControls);
      _desktopWindow?.setCaptureToggle(() async {
        await settingsController.toggleCapture();
      });
      await Future.wait([
        _quickPasteSettings?.initialize() ?? Future<void>.value(),
        settingsController.initialize(),
        _modulesController!.initialize(),
      ]);
      _syncCaptureControls();
      if (!mounted) return;
      // After Quick Paste, which registers its shortcut first.
      unawaited(features.start().catchError((Object _) {}));
      _notifications = NotificationsController(
        devices: _devicesController!,
        platform: PlatformPairingNotificationPort(),
        visible: _visibility,
        openPairing: _openPairingFromNotification,
      );
      setState(() => _runtimeState = _RuntimeState.ready);
      if (_visibility.value) {
        unawaited(_notifications!.enableSystemNotifications());
      }
      unawaited(_devicesController!.start());
      unawaited(_activatePendingPairingLink());
    } catch (error) {
      if (!mounted) {
        if (Platform.isMacOS || Platform.isWindows || Platform.isLinux) {
          await runtime.stopDesktopRuntime();
        }
        return;
      }
      setState(() {
        _runtimeState = _RuntimeState.failed;
        _runtimeFailureMessage = error is runtime.RuntimeError
            ? error.message
            : AppStrings.current.appRuntimeStartFailed;
      });
    }
  }

  Future<Object?> _handlePairingLinkCall(MethodCall call) async {
    if (call.method != 'openPairingUri') {
      throw MissingPluginException(
        'Unsupported pairing link method: ${call.method}',
      );
    }
    final uri = call.arguments as String?;
    if (uri == null || uri.isEmpty) return false;
    _pendingPairingUri = uri;
    await _activatePendingPairingLink();
    return true;
  }

  Future<void> _takePendingPairingLink() async {
    try {
      final uri = await _pairingLinksChannel.invokeMethod<String>(
        'takePendingUri',
      );
      if (uri == null || uri.isEmpty) return;
      _pendingPairingUri = uri;
      await _activatePendingPairingLink();
    } on MissingPluginException {
      // Tests and unsupported hosts have no native URI registration.
    } on PlatformException {
      // Native validation keeps an invalid URI out of the pairing controller.
    }
  }

  Future<void> _activatePendingPairingLink() async {
    final controller = _devicesController;
    final uri = _pendingPairingUri;
    if (controller == null ||
        uri == null ||
        _runtimeState != _RuntimeState.ready ||
        !(_onboarding?.complete ?? true)) {
      return;
    }
    _pendingPairingUri = null;
    _navigation.selectDestination(AppDestination.devices);
    await controller.joinPairingUri(uri);
  }

  void _openPairingFromNotification() {
    if (!mounted || _runtimeState != _RuntimeState.ready) return;
    _navigation.selectDestination(AppDestination.devices);
    if (!_visibility.value) unawaited(_desktopWindow?.showFromTrayOrDock());
  }

  Future<void> _retryRuntime() async {
    await _stopRuntime();
    await _startRuntime();
  }

  /// Releases feature state during widget disposal and before Windows Quit.
  /// macOS committed process exit closes the owned daemon's app-parent pipe.
  Future<void> _prepareForTermination() async {
    final connections = _releaseFeatureState();
    await _stopDesktopRuntime();
    unawaited(connections.history?.dispose().catchError((Object _) {}));
    unawaited(connections.devices?.dispose().catchError((Object _) {}));
  }

  Future<void> _stopRuntime() async {
    final connections = _releaseFeatureState();
    await connections.history?.dispose();
    await connections.devices?.dispose();
    await _stopDesktopRuntime();
  }

  /// Disposes feature controllers and hands back the runtime connections,
  /// which each teardown path closes in its own order.
  ({RuntimeHistoryRepository? history, FlutterRustDevicesGateway? devices})
  _releaseFeatureState() {
    final historyController = _historyController;
    final devicesController = _devicesController;
    final historyRepository = _historyRepository;
    final devicesGateway = _devicesGateway;
    final quickPasteSettings = _quickPasteSettings;
    final settingsController = _settingsController;
    final features = _features;
    _historyController = null;
    _devicesController = null;
    _historyRepository = null;
    _devicesGateway = null;
    _quickPasteSettings = null;
    _quickPasteHost = null;
    _shortcutRegistrar = null;
    _features = null;
    _settingsController = null;
    unawaited(features?.dispose().catchError((Object _) {}));
    historyController?.dispose();
    _modulesController?.dispose();
    _modulesController = null;
    _notifications?.dispose();
    _notifications = null;
    devicesController?.dispose();
    quickPasteSettings?.removeListener(_syncTrayShortcut);
    quickPasteSettings?.dispose();
    settingsController?.removeListener(_syncCaptureControls);
    settingsController?.dispose();
    _desktopWindow?.setCaptureToggle(null);
    unawaited(
      _desktopWindow?.updateCaptureState(
        state: CaptureControlState.unavailable,
      ),
    );
    return (history: historyRepository, devices: devicesGateway);
  }

  Future<void> _stopDesktopRuntime() async {
    if (widget.runtimeEnabled &&
        (Platform.isMacOS || Platform.isWindows || Platform.isLinux)) {
      await runtime.stopDesktopRuntime();
    }
  }

  void _disposeDesktopWindow(DesktopWindowController? desktopWindow) {
    if (desktopWindow == null) return;
    unawaited(desktopWindow.dispose().catchError((Object _) {}));
  }

  void _configureOnboarding() {
    final injected = widget.onboardingController;
    if (injected != null) {
      _onboarding = injected;
    } else if (platformOnboardingSetup(startup: _startupGateway)
        case final setup?) {
      _onboarding = OnboardingController(setup: setup);
      _ownsOnboarding = true;
    }
    final controller = _onboarding;
    if (controller == null) {
      _onboardingReady = true;
      return;
    }
    unawaited(_initializeOnboarding(controller));
  }

  Future<void> _initializeOnboarding(OnboardingController controller) async {
    await controller.initialize();
    if (mounted) setState(() => _onboardingReady = true);
  }

  Future<void> _finishOnboarding() async {
    await _launchAtLogin.refresh();
    if (!mounted) return;
    _navigation.selectDestination(AppDestination.history);
    setState(() {});
    await _activatePendingPairingLink();
  }

  Future<void> _reopenOnboardingSetup() async {
    final controller = _onboarding;
    if (controller == null || !await controller.reopenSetup()) return;
    if (mounted) setState(() {});
  }

  void _openSettings(SettingsSectionId section) {
    _settingsController?.showSection(section);
    _navigation.selectDestination(AppDestination.settings);
  }

  /// Creates the Quick Paste window host and the shortcut registrar the
  /// paste queue shares; the caller initializes the settings controller.
  void _createQuickPaste() {
    if (!Platform.isMacOS && !Platform.isWindows && !Platform.isLinux) return;
    final host = _quickPasteHost = MethodChannelQuickPasteWindowHost();
    host.setFindInHistoryHandler(_navigation.findInHistory);
    host.setOpenMainHandler(() {
      _navigation.selectDestination(AppDestination.history);
    });
    host.setOpenSettingsHandler(() {
      _navigation.selectDestination(AppDestination.settings);
    });
    // One registrar binds every Linux portal shortcut in one session.
    final registrar = _shortcutRegistrar = Platform.isLinux
        ? LinuxPortalDesktopShortcutRegistrar()
        : HotKeyManagerDesktopShortcutRegistrar();
    _quickPasteSettings = QuickPasteSettingsController(
      store: SharedPreferencesQuickPastePreferencesStore(),
      registrar: registrar,
      windowHost: host,
      linuxIntegration: Platform.isLinux
          ? MethodChannelLinuxIntegrationPort()
          : null,
    )..addListener(_syncTrayShortcut);
  }

  AppFeatures _createFeatures() {
    final desktop = Platform.isMacOS || Platform.isWindows || Platform.isLinux;
    final authenticator = MethodChannelDeviceAuthenticator();
    final aiTools = RuntimeAiToolsRepository();
    final host = _quickPasteHost;
    final registrar = _shortcutRegistrar;
    final desktopWindow = _desktopWindow;
    // Without hideWindow, deliveries from this window only copy; the global
    // shortcuts paste into the foreground application.
    final pasteQueue = PasteQueueController(
      repository: RuntimePasteQueueRepository(),
      pasteHost: host ?? const _ClipboardOnlyPasteHost(),
      notifier: desktop ? SystemPasteQueueNotifier() : null,
      requestAccessibility: host?.requestAccessibility,
    );
    return AppFeatures(
      pasteQueue: pasteQueue,
      pasteQueueShortcuts: registrar == null
          ? null
          : PasteQueueShortcutsController(
              queue: pasteQueue,
              registrar: registrar,
            ),
      snippets: SnippetsController(
        repository: RuntimeSnippetRepository(),
        access: ConcealedSnippetAccess(authenticator: authenticator),
      ),
      transforms: TransformsSettingsController(
        repository: RuntimeTransformRepository(),
        platform: TransformsSettingsController.platformName(
          defaultTargetPlatform,
        ),
      ),
      contentRules: ContentRulesController(
        repository: RuntimeContentRuleRepository(),
      ),
      aiTools: AiToolsController(repository: aiTools, serving: desktop),
      aiApproval: desktop
          ? AiApprovalController(
              repository: aiTools,
              notifier: SystemAiApprovalNotifier(),
              visible: _visibility,
              showWindow: () => unawaited(_desktopWindow?.showFromTrayOrDock()),
            )
          : null,
      quickAccess: Platform.isAndroid
          ? QuickAccessController(
              const MethodChannelAndroidQuickAccessGateway(),
            )
          : null,
      tray: desktopWindow == null
          ? null
          : TrayMenuController(
              surface: desktopWindow,
              authenticator: authenticator,
            ),
      openSettings: _openSettings,
      capturePaused: () => _settingsController?.capture?.paused ?? false,
    );
  }

  void _syncTrayShortcut() {
    final settings = _quickPasteSettings;
    _features?.showQuickPasteShortcut(
      settings?.registeredShortcut,
      description: settings?.registeredShortcutDescription,
    );
  }

  void _syncCaptureControls() {
    final capture = _settingsController?.capture;
    unawaited(
      _desktopWindow?.updateCaptureState(
        state: capture?.controlState ?? CaptureControlState.unavailable,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final desktopWindow = _desktopWindow;
    if (desktopWindow == null) {
      return _buildApplication(windowReady: false);
    }
    return ValueListenableBuilder<bool>(
      valueListenable: desktopWindow.isUnifiedTitleBarReady,
      builder: (context, windowReady, child) =>
          _buildApplication(windowReady: windowReady),
    );
  }

  Widget _buildApplication({required bool windowReady}) {
    // Keep the native accessibility root stable while releasing presentation.
    // Replacing the entire semantics tree with an empty render object leaves
    // macOS accessibility clients with references to the retired subtree.
    return Semantics(
      key: const ValueKey('application-semantics'),
      container: true,
      explicitChildNodes: true,
      label: 'CopyPaste',
      textDirection: TextDirection.ltr,
      child: _buildPresentation(windowReady: windowReady),
    );
  }

  Widget _buildPresentation({required bool windowReady}) {
    if (_uiMemory.suspended) return const SizedBox.expand();
    final unifiedTitleBar = usesUnifiedMacosTitleBar(
      _desktopWindow,
      windowReady: windowReady,
    );
    if (!_onboardingReady) {
      return ShadcnApp(
        title: 'CopyPaste',
        debugShowCheckedModeBanner: false,
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: AppTheme.mode,
        builder: AppTheme.builder,
        locale: widget.language?.locale,
        localizationsDelegates: AppLocalization.delegates,
        supportedLocales: AppLocalization.supportedLocales,
        localeListResolutionCallback: AppLocalization.resolve,
        home: AppStringsLocale(
          child: Scaffold(
            headers: unifiedTitleBar
                ? const [MacosWindowHeader(title: Text('CopyPaste'))]
                : const [AppBar(title: Text('CopyPaste')), Divider()],
            child: Builder(
              builder: (context) =>
                  StateView.loading(message: context.l10n.appPreparing),
            ),
          ),
        ),
      );
    }
    final onboarding = _onboarding;
    if (onboarding != null && !onboarding.complete) {
      return ShadcnApp(
        onGenerateTitle: (context) => context.l10n.onboardingTitle,
        debugShowCheckedModeBanner: false,
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: AppTheme.mode,
        builder: AppTheme.builder,
        locale: widget.language?.locale,
        localizationsDelegates: AppLocalization.delegates,
        supportedLocales: AppLocalization.supportedLocales,
        localeListResolutionCallback: AppLocalization.resolve,
        home: AppStringsLocale(
          child: OnboardingScreen(
            controller: onboarding,
            onFinished: _finishOnboarding,
            unifiedTitleBar: unifiedTitleBar,
          ),
        ),
      );
    }
    return CopyPasteApp(
      navigatorKey: _rootNavigatorKey,
      navigation: _navigation,
      language: widget.language,
      historyController: _historyController,
      devicesController: _devicesController,
      features: _features,
      notifications: _notifications,
      quickPasteSettings: _quickPasteSettings,
      settingsController: _settingsController,
      launchAtLogin: _launchAtLogin,
      modulesController: _modulesController,
      appUpdateController: _appUpdateController,
      desktopWindow: _desktopWindow,
      runtimeUnavailableMessage: switch (_runtimeState) {
        _RuntimeState.starting => AppStrings.current.appRuntimeWaiting,
        _RuntimeState.failed => _runtimeFailureMessage,
        _RuntimeState.idle || _RuntimeState.ready => null,
      },
      onRetryRuntime: _runtimeState == _RuntimeState.failed
          ? _retryRuntime
          : null,
      onOpenAndroidCaptureSetup: _onboarding?.setup.reopenable == true
          ? _reopenOnboardingSetup
          : null,
    );
  }
}

/// Android puts queue entries on the clipboard; its keyboard inserts them.
class _ClipboardOnlyPasteHost implements ForegroundPasteHost {
  const _ClipboardOnlyPasteHost();

  @override
  Future<PasteOutcome> pasteForeground({bool plainText = false}) async =>
      PasteOutcome.unsupported;

  @override
  Future<bool> moveCaretLeft(int count) async => false;

  @override
  Future<bool> accessibilityGranted() async => false;

  @override
  Future<bool?> secureFieldFocused() async => null;
}
