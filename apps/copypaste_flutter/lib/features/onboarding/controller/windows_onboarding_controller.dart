import 'onboarding_controller.dart';

import '../repository/onboarding_store.dart';
import '../../../platform/startup/launch_at_login.dart';

enum WindowsOnboardingStep { welcome, setup, sync }

class WindowsOnboardingController extends OnboardingController {
  WindowsOnboardingController({
    required OnboardingStore store,
    LaunchAtLoginGateway? startup,
  }) : _store = store,
       _startup =
           startup ?? MethodChannelLaunchAtLoginGateway(platform: 'windows');

  final OnboardingStore _store;
  final LaunchAtLoginGateway _startup;
  WindowsOnboardingStep _step = WindowsOnboardingStep.welcome;
  bool _initialized = false;
  LaunchAtLoginStatus _startupStatus = LaunchAtLoginStatus.unavailable;
  bool _launchAtLogin = false;
  String? _noticeMessage;

  WindowsOnboardingStep get step => _step;
  bool get launchAtLogin => _launchAtLogin;
  bool get launchAtLoginAvailable =>
      _startupStatus != LaunchAtLoginStatus.unavailable;
  String? get noticeMessage => _noticeMessage;
  bool get _registered =>
      _startupStatus == LaunchAtLoginStatus.enabled ||
      _startupStatus == LaunchAtLoginStatus.requiresApproval;

  Future<void> initialize() async {
    if (_initialized) return;
    busy = true;
    notifyIfActive();
    try {
      complete = await _store.isComplete();
      // Completed installations keep their existing startup choice. Only new
      // onboarding offers the same opt-out default as macOS.
      if (!complete) {
        try {
          _startupStatus = await _startup.read();
          _launchAtLogin = launchAtLoginAvailable;
        } catch (_) {
          _noticeMessage =
              'Start at login is unavailable. You can continue without it.';
        }
      }
    } catch (_) {
      errorMessage = 'CopyPaste could not read Windows setup state.';
    } finally {
      _initialized = true;
      busy = false;
      notifyIfActive();
    }
  }

  void showSetup() {
    if (busy ||
        complete ||
        disposed ||
        _step != WindowsOnboardingStep.welcome) {
      return;
    }
    _step = WindowsOnboardingStep.setup;
    notifyIfActive();
  }

  void showPreviousStep() {
    if (busy || complete || disposed) return;
    _step = switch (_step) {
      WindowsOnboardingStep.welcome => WindowsOnboardingStep.welcome,
      WindowsOnboardingStep.setup => WindowsOnboardingStep.welcome,
      WindowsOnboardingStep.sync => WindowsOnboardingStep.setup,
    };
    notifyIfActive();
  }

  Future<void> setLaunchAtLogin(bool enabled) async {
    if (busy ||
        complete ||
        disposed ||
        !launchAtLoginAvailable ||
        enabled == _launchAtLogin) {
      return;
    }
    busy = true;
    _noticeMessage = null;
    notifyIfActive();
    try {
      await _applyStartup(enabled);
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<bool> continueFromSetup() async {
    if (busy || complete || disposed || _step != WindowsOnboardingStep.setup) {
      return false;
    }
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      if (launchAtLoginAvailable) await _applyStartup(_launchAtLogin);
      if (disposed) return false;
      _step = WindowsOnboardingStep.sync;
      return true;
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> _applyStartup(bool enabled) async {
    if (enabled == _registered) {
      _launchAtLogin = enabled;
      return;
    }
    try {
      _startupStatus = await _startup.setEnabled(enabled);
      _launchAtLogin = _registered;
      if (_registered != enabled ||
          _startupStatus == LaunchAtLoginStatus.requiresApproval) {
        _noticeMessage =
            'Start at login could not be updated. You can change it later in Settings.';
      }
    } catch (_) {
      _launchAtLogin = _registered;
      _noticeMessage =
          'Start at login could not be updated. You can continue without it.';
    }
  }

  Future<bool> finish() => finishOnboarding(
    canFinish: _step == WindowsOnboardingStep.sync,
    persist: _store.markComplete,
  );
}
