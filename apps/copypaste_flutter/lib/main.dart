import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'app/app.dart';
import 'app/navigation/navigation.dart';
import 'app/shell/macos_window_header.dart';
import 'app/theme/app_motion.dart';
import 'app/theme/app_theme.dart';
import 'features/devices/devices.dart';
import 'features/devices/flutter_rust_devices_gateway.dart';
import 'features/history/controller/history_controller.dart';
import 'features/history/repository/file_selector_history_file_downloader.dart';
import 'features/history/repository/runtime_history_repository.dart';
import 'features/onboarding/controller/android_onboarding_controller.dart';
import 'features/onboarding/controller/macos_onboarding_controller.dart';
import 'features/onboarding/repository/android_onboarding_store.dart';
import 'features/onboarding/repository/macos_onboarding_store.dart';
import 'features/onboarding/view/android_onboarding_screen.dart';
import 'features/onboarding/view/macos_onboarding_screen.dart';
import 'features/quick_paste/quick_paste_app.dart';
import 'features/quick_paste/quick_paste_controller.dart';
import 'features/settings/controller/quick_paste_settings_controller.dart';
import 'features/settings/controller/settings_controller.dart';
import 'features/settings/repository/file_selector_settings_file_picker.dart';
import 'features/settings/repository/quick_paste_preferences_store.dart';
import 'features/settings/repository/runtime_settings_repository.dart';
import 'features/update/update.dart';
import 'generated/frb_generated.dart';
import 'generated/api.dart' as runtime;
import 'platform/desktop/desktop_window_bootstrap.dart';
import 'platform/desktop/desktop_window_controller.dart';
import 'platform/desktop/global_shortcut.dart';
import 'platform/desktop/quick_paste_host.dart';
import 'platform/android/android_capture_setup_gateway.dart';
import 'platform/macos/macos_setup_gateway.dart';
import 'platform/pairing/pairing_presentation.dart';
import 'platform/update/app_update_platform.dart';
import 'shared/state_view.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  AppMotion.configureLibrary();
  await RustLib.init();
  _runtimeLifecycleChannel.setMethodCallHandler(_handleNativeTermination);
  final desktopWindow = await initializeDesktopWindow();
  runApp(
    CopyPasteRoot(
      desktopWindow: desktopWindow,
      appUpdateController: AppUpdateController(
        repository: GitHubAppUpdateRepository(
          temporaryDirectory: getTemporaryDirectory,
        ),
        platform: MethodChannelAppUpdatePlatform(),
      ),
    ),
  );
}

@pragma('vm:entry-point')
Future<void> quickPasteMain() async {
  WidgetsFlutterBinding.ensureInitialized();
  AppMotion.configureLibrary();
  await RustLib.init();
  final repository = RuntimeHistoryRepository();
  runApp(
    QuickPasteApp(
      controller: QuickPasteController(
        repository: repository,
        disposeRepository: repository.dispose,
        preferencesStore: SharedPreferencesQuickPastePreferencesStore(),
        host: MethodChannelQuickPasteContextHost(),
      ),
    ),
  );
}

const _runtimeLifecycleChannel = MethodChannel(
  'com.copypaste.app/runtime_lifecycle',
);
const _pairingLinksChannel = MethodChannel('com.copypaste.app/pairing_links');

Future<Object?> _handleNativeTermination(MethodCall call) async {
  if (call.method != 'prepareForTermination') {
    throw MissingPluginException(
      'Unsupported lifecycle method: ${call.method}',
    );
  }
  if (Platform.isMacOS) {
    await runtime.stopIsolatedDesktopRuntime();
  }
  return true;
}

Future<void> _startDebugRuntime() async {
  if (Platform.isAndroid) {
    await runtime.runtimeStatus();
    return;
  }
  if (!Platform.isMacOS) return;
  final daemon = File(
    '${File(Platform.resolvedExecutable).parent.path}/copypaste-daemon',
  );
  if (!await daemon.exists()) {
    throw StateError('The CopyPaste runtime helper is unavailable.');
  }
  final supportDirectory = await getApplicationSupportDirectory();
  final dataDir = Directory('${supportDirectory.path}/development-runtime');
  await runtime.startIsolatedDesktopRuntime(
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
    this.macosOnboardingController,
    this.androidOnboardingController,
    this.appUpdateController,
    this.runtimeEnabled = true,
  });

  final DesktopWindowController? desktopWindow;
  final MacosOnboardingController? macosOnboardingController;
  final AndroidOnboardingController? androidOnboardingController;
  final AppUpdateController? appUpdateController;
  final bool runtimeEnabled;

  @override
  State<CopyPasteRoot> createState() => _CopyPasteRootState();
}

class _CopyPasteRootState extends State<CopyPasteRoot> {
  late final AppNavigationController _navigation = AppNavigationController();
  late final DesktopWindowController? _desktopWindow = widget.desktopWindow;
  late final AppUpdateController? _appUpdateController =
      widget.appUpdateController;
  RuntimeHistoryRepository? _historyRepository;
  HistoryController? _historyController;
  FlutterRustDevicesGateway? _devicesGateway;
  DevicesController? _devicesController;
  QuickPasteSettingsController? _quickPasteSettings;
  SettingsController? _settingsController;
  MacosOnboardingController? _macosOnboarding;
  AndroidOnboardingController? _androidOnboarding;
  bool _ownsMacosOnboarding = false;
  bool _ownsAndroidOnboarding = false;
  bool _macosOnboardingReady = false;
  bool _androidOnboardingReady = false;
  _RuntimeState _runtimeState = _RuntimeState.idle;
  String? _runtimeFailureMessage;
  String? _pendingPairingUri;

  bool _usesUnifiedMacosTitleBar(bool windowReady) =>
      Platform.isMacOS && _desktopWindow != null && windowReady;

  @override
  void initState() {
    super.initState();
    _pairingLinksChannel.setMethodCallHandler(_handlePairingLinkCall);
    unawaited(_takePendingPairingLink());
    _desktopWindow?.setBeforeQuit(_prepareForTermination);
    _desktopWindow?.setOpenSettings(() {
      _navigation.selectDestination(AppDestination.settings);
    });
    _configureMacosOnboarding();
    _configureAndroidOnboarding();
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
      oldWidget.macosOnboardingController == widget.macosOnboardingController,
      'CopyPasteRoot cannot replace its macOS onboarding controller.',
    );
    assert(
      oldWidget.androidOnboardingController ==
          widget.androidOnboardingController,
      'CopyPasteRoot cannot replace its Android onboarding controller.',
    );
    assert(
      oldWidget.appUpdateController == widget.appUpdateController,
      'CopyPasteRoot cannot replace its process-wide update controller.',
    );
  }

  @override
  void dispose() {
    _pairingLinksChannel.setMethodCallHandler(null);
    _desktopWindow?.setOpenSettings(null);
    if (_ownsMacosOnboarding) {
      _macosOnboarding?.dispose();
    }
    if (_ownsAndroidOnboarding) {
      _androidOnboarding?.dispose();
    }
    _navigation.dispose();
    _appUpdateController?.dispose();
    unawaited(_prepareForTermination());
    _disposeDesktopWindow(_desktopWindow);
    super.dispose();
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
      await _startDebugRuntime();
      if (!mounted) {
        await runtime.stopIsolatedDesktopRuntime();
        return;
      }
      setState(() {
        _historyRepository = RuntimeHistoryRepository();
        _historyController = HistoryController(
          _historyRepository!,
          fileDownloader: const FileSelectorHistoryFileDownloader(),
        );
        _devicesGateway = FlutterRustDevicesGateway();
        _devicesController = DevicesController(
          gateway: _devicesGateway!,
          captureProtection: MethodChannelPairingCaptureProtection(),
        );
        _settingsController = SettingsController(
          repository: RuntimeSettingsRepository(),
          filePicker: const FileSelectorSettingsFilePicker(),
        );
      });
      final settingsController = _settingsController!;
      settingsController.addListener(_syncCaptureControls);
      _desktopWindow?.setCaptureToggle(() async {
        await settingsController.toggleCapture();
      });
      await Future.wait([
        _initializeQuickPaste(),
        settingsController.initialize(),
      ]);
      _syncCaptureControls();
      if (!mounted) return;
      setState(() => _runtimeState = _RuntimeState.ready);
      unawaited(_activatePendingPairingLink());
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _runtimeState = _RuntimeState.failed;
        _runtimeFailureMessage = error is runtime.RuntimeError
            ? error.message
            : 'CopyPaste could not start its runtime.';
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
        !(_macosOnboarding?.complete ?? true) ||
        !(_androidOnboarding?.complete ?? true)) {
      return;
    }
    _pendingPairingUri = null;
    _navigation.selectDestination(AppDestination.devices);
    await controller.joinPairingUri(uri);
  }

  Future<void> _retryRuntime() async {
    await _stopRuntime();
    await _startRuntime();
  }

  /// Releases the daemon's app-parent pipe before native window destruction.
  /// Feature Watch cleanup is best-effort during process termination so it
  /// cannot delay the native termination reply after the daemon is gone.
  Future<void> _prepareForTermination() async {
    final historyController = _historyController;
    final devicesController = _devicesController;
    final historyRepository = _historyRepository;
    final devicesGateway = _devicesGateway;
    final quickPasteSettings = _quickPasteSettings;
    final settingsController = _settingsController;
    _historyController = null;
    _devicesController = null;
    _historyRepository = null;
    _devicesGateway = null;
    _quickPasteSettings = null;
    _settingsController = null;
    historyController?.dispose();
    devicesController?.dispose();
    quickPasteSettings?.dispose();
    settingsController?.removeListener(_syncCaptureControls);
    settingsController?.dispose();
    _desktopWindow?.setCaptureToggle(null);
    unawaited(
      _desktopWindow?.updateCaptureState(available: false, paused: true),
    );
    if (widget.runtimeEnabled && Platform.isMacOS) {
      await runtime.stopIsolatedDesktopRuntime();
    }
    if (historyRepository != null) {
      unawaited(historyRepository.dispose().catchError((Object _) {}));
    }
    if (devicesGateway != null) {
      unawaited(devicesGateway.dispose().catchError((Object _) {}));
    }
  }

  Future<void> _stopRuntime() async {
    final historyController = _historyController;
    final devicesController = _devicesController;
    final historyRepository = _historyRepository;
    final devicesGateway = _devicesGateway;
    final quickPasteSettings = _quickPasteSettings;
    final settingsController = _settingsController;
    _historyController = null;
    _devicesController = null;
    _historyRepository = null;
    _devicesGateway = null;
    _quickPasteSettings = null;
    _settingsController = null;
    historyController?.dispose();
    devicesController?.dispose();
    quickPasteSettings?.dispose();
    settingsController?.removeListener(_syncCaptureControls);
    settingsController?.dispose();
    _desktopWindow?.setCaptureToggle(null);
    unawaited(
      _desktopWindow?.updateCaptureState(available: false, paused: true),
    );
    await historyRepository?.dispose();
    await devicesGateway?.dispose();
    if (widget.runtimeEnabled && Platform.isMacOS) {
      await runtime.stopIsolatedDesktopRuntime();
    }
  }

  void _disposeDesktopWindow(DesktopWindowController? desktopWindow) {
    if (desktopWindow == null) return;
    unawaited(desktopWindow.dispose().catchError((Object _) {}));
  }

  void _configureMacosOnboarding() {
    final injected = widget.macosOnboardingController;
    if (injected != null) {
      _macosOnboarding = injected;
    } else if (Platform.isMacOS) {
      _macosOnboarding = MacosOnboardingController(
        store: SharedPreferencesMacosOnboardingStore(),
        setup: MethodChannelMacosSetupGateway(),
      );
      _ownsMacosOnboarding = true;
    }
    final controller = _macosOnboarding;
    if (controller == null) {
      _macosOnboardingReady = true;
      return;
    }
    unawaited(_initializeMacosOnboarding(controller));
  }

  Future<void> _initializeMacosOnboarding(
    MacosOnboardingController controller,
  ) async {
    await controller.initialize();
    if (!mounted) return;
    setState(() => _macosOnboardingReady = true);
  }

  void _configureAndroidOnboarding() {
    final injected = widget.androidOnboardingController;
    if (injected != null) {
      _androidOnboarding = injected;
    } else if (Platform.isAndroid) {
      _androidOnboarding = AndroidOnboardingController(
        store: SharedPreferencesAndroidOnboardingStore(),
        setup: MethodChannelAndroidCaptureSetupGateway(),
      );
      _ownsAndroidOnboarding = true;
    }
    final controller = _androidOnboarding;
    if (controller == null) {
      _androidOnboardingReady = true;
      return;
    }
    unawaited(_initializeAndroidOnboarding(controller));
  }

  Future<void> _initializeAndroidOnboarding(
    AndroidOnboardingController controller,
  ) async {
    await controller.initialize();
    if (!mounted) return;
    setState(() => _androidOnboardingReady = true);
  }

  Future<void> _finishMacosOnboarding(AppDestination destination) async {
    _navigation.selectDestination(destination);
    if (!mounted) return;
    setState(() {});
    await _activatePendingPairingLink();
  }

  Future<void> _finishAndroidOnboarding(AppDestination destination) async {
    _navigation.selectDestination(destination);
    if (!mounted) return;
    setState(() {});
    await _activatePendingPairingLink();
  }

  Future<void> _openAndroidCaptureSetup() async {
    final controller = _androidOnboarding;
    if (controller == null || !await controller.reopenCaptureSetup()) return;
    if (mounted) setState(() {});
  }

  Future<void> _initializeQuickPaste() async {
    if (!Platform.isMacOS && !Platform.isWindows) return;
    final host = MethodChannelQuickPasteWindowHost();
    host.setOpenSettingsHandler(() {
      _navigation.selectDestination(AppDestination.settings);
    });
    final controller = QuickPasteSettingsController(
      store: SharedPreferencesQuickPastePreferencesStore(),
      registrar: HotKeyManagerDesktopShortcutRegistrar(),
      windowHost: host,
    );
    _quickPasteSettings = controller;
    await controller.initialize();
  }

  void _syncCaptureControls() {
    final capture = _settingsController?.capture;
    unawaited(
      _desktopWindow?.updateCaptureState(
        available: capture != null,
        paused: capture?.paused ?? true,
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
    final unifiedTitleBar = _usesUnifiedMacosTitleBar(windowReady);
    if (_runtimeState != _RuntimeState.ready ||
        !_macosOnboardingReady ||
        !_androidOnboardingReady) {
      return ShadcnApp(
        title: 'CopyPaste',
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: AppTheme.mode,
        builder: AppTheme.builder,
        home: Scaffold(
          headers: unifiedTitleBar
              ? const [MacosWindowHeader(title: Text('CopyPaste'))]
              : const [AppBar(title: Text('CopyPaste')), Divider()],
          child: _runtimeState == _RuntimeState.failed
              ? StateView.error(
                  title: 'Runtime needs attention',
                  message: _runtimeFailureMessage,
                  actionLabel: 'Retry',
                  onAction: _retryRuntime,
                )
              : const StateView.loading(message: 'Preparing CopyPaste.'),
        ),
      );
    }
    final onboarding = _macosOnboarding;
    if (onboarding != null && !onboarding.complete) {
      return ShadcnApp(
        title: 'Set up CopyPaste',
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: AppTheme.mode,
        builder: AppTheme.builder,
        home: MacosOnboardingScreen(
          controller: onboarding,
          onPairDevice: () => _finishMacosOnboarding(AppDestination.devices),
          onOpenHistory: () => _finishMacosOnboarding(AppDestination.history),
          unifiedTitleBar: unifiedTitleBar,
        ),
      );
    }
    final androidOnboarding = _androidOnboarding;
    if (androidOnboarding != null && !androidOnboarding.complete) {
      return ShadcnApp(
        title: 'Set up CopyPaste',
        theme: AppTheme.light,
        darkTheme: AppTheme.dark,
        themeMode: AppTheme.mode,
        builder: AppTheme.builder,
        home: AndroidOnboardingScreen(
          controller: androidOnboarding,
          onPairDevice: () => _finishAndroidOnboarding(AppDestination.devices),
          onOpenHistory: () => _finishAndroidOnboarding(AppDestination.history),
        ),
      );
    }
    return CopyPasteApp(
      navigation: _navigation,
      historyController: _historyController,
      devicesController: _devicesController,
      quickPasteSettings: _quickPasteSettings,
      settingsController: _settingsController,
      appUpdateController: _appUpdateController,
      desktopWindow: _desktopWindow,
      onOpenAndroidCaptureSetup: _androidOnboarding == null
          ? null
          : _openAndroidCaptureSetup,
    );
  }
}
