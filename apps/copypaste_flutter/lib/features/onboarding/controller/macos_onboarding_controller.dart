import 'onboarding_controller.dart';

import '../../../platform/macos/macos_setup_gateway.dart';
import '../repository/onboarding_store.dart';

enum MacosOnboardingStep { welcome, setup, sync }

class MacosOnboardingController extends OnboardingController {
  MacosOnboardingController({
    required OnboardingStore store,
    required MacosSetupGateway setup,
  }) : _store = store,
       _setup = setup;

  final OnboardingStore _store;
  final MacosSetupGateway _setup;

  MacosOnboardingStep _step = MacosOnboardingStep.welcome;
  MacosLoginItemStatus _loginItemStatus = MacosLoginItemStatus.notRegistered;
  bool _initialized = false;
  bool _accessibilityGranted = false;
  bool _launchAtLogin = true;
  bool _loginDefaultPending = false;
  String? _noticeMessage;

  MacosOnboardingStep get step => _step;
  MacosLoginItemStatus get loginItemStatus => _loginItemStatus;
  bool get initialized => _initialized;
  bool get accessibilityGranted => _accessibilityGranted;
  bool get launchAtLogin => _launchAtLogin;
  bool get launchAtLoginAvailable =>
      _loginItemStatus != MacosLoginItemStatus.developmentUnavailable &&
      _loginItemStatus != MacosLoginItemStatus.unavailable;
  bool get loginItemNeedsAttention =>
      _launchAtLogin &&
      launchAtLoginAvailable &&
      _loginItemStatus != MacosLoginItemStatus.enabled;
  String? get noticeMessage => _noticeMessage;

  bool get canContinueSetup => !busy;

  Future<void> initialize() async {
    if (_initialized) return;
    busy = true;
    notifyIfActive();
    try {
      final wasComplete = await _store.isComplete();
      _loginDefaultPending = !wasComplete;
      await _refreshSystemState();
      complete = wasComplete;
      errorMessage = null;
    } catch (_) {
      errorMessage = 'CopyPaste could not read the macOS setup state.';
    } finally {
      _initialized = true;
      busy = false;
      notifyIfActive();
    }
  }

  void showSetup() {
    if (busy || _step != MacosOnboardingStep.welcome) return;
    _step = MacosOnboardingStep.setup;
    notifyIfActive();
  }

  void showPreviousStep() {
    if (busy) return;
    _step = switch (_step) {
      MacosOnboardingStep.welcome => MacosOnboardingStep.welcome,
      MacosOnboardingStep.setup => MacosOnboardingStep.welcome,
      MacosOnboardingStep.sync => MacosOnboardingStep.setup,
    };
    notifyIfActive();
  }

  Future<void> setLaunchAtLogin(bool enabled) async {
    if (busy || !launchAtLoginAvailable || _launchAtLogin == enabled) return;
    busy = true;
    errorMessage = null;
    _noticeMessage = null;
    notifyIfActive();
    try {
      await _updateLaunchAtLogin(enabled);
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> requestAccessibility() async {
    if (busy || _accessibilityGranted) return;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      _accessibilityGranted = await _setup.requestAccessibility();
    } catch (_) {
      errorMessage = 'Accessibility permission could not be requested.';
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> refreshSystemState() async {
    if (busy) return;
    busy = true;
    notifyIfActive();
    try {
      await _refreshSystemState();
      errorMessage = null;
    } catch (_) {
      errorMessage = 'CopyPaste could not refresh the macOS setup state.';
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<bool> continueFromSetup() async {
    if (busy) return false;
    busy = true;
    errorMessage = null;
    _noticeMessage = null;
    notifyIfActive();
    try {
      _accessibilityGranted = await _setup.accessibilityGranted();
    } catch (_) {
      _addNotice('Accessibility status could not be refreshed.');
    }
    await _updateLaunchAtLogin(_launchAtLogin);
    _step = MacosOnboardingStep.sync;
    busy = false;
    notifyIfActive();
    return true;
  }

  Future<void> openLoginItemsSettings() async {
    try {
      await _setup.openLoginItemsSettings();
    } catch (_) {
      errorMessage = 'Login Items settings could not be opened.';
      notifyIfActive();
    }
  }

  Future<bool> finish() => finishOnboarding(
    canFinish: _step == MacosOnboardingStep.sync,
    persist: _store.markComplete,
  );

  bool get _loginItemRegistered =>
      _loginItemStatus == MacosLoginItemStatus.enabled ||
      _loginItemStatus == MacosLoginItemStatus.requiresApproval;

  Future<void> _refreshSystemState() async {
    _accessibilityGranted = await _setup.accessibilityGranted();
    _loginItemStatus = await _setup.loginItemStatus();
    if (_loginItemRegistered) _loginDefaultPending = false;
    _launchAtLogin =
        _loginItemRegistered ||
        (_loginDefaultPending && launchAtLoginAvailable);
  }

  Future<void> _updateLaunchAtLogin(bool enabled) async {
    _loginDefaultPending = false;
    try {
      _loginItemStatus = await _setup.setLaunchAtLogin(enabled);
      _launchAtLogin = _loginItemRegistered;
      if (enabled && _loginItemStatus != MacosLoginItemStatus.enabled) {
        _addNotice(
          _loginItemStatus == MacosLoginItemStatus.requiresApproval
              ? 'Start at login requires approval in System Settings.'
              : 'Start at login could not be enabled. You can continue without it.',
        );
      } else if (!enabled && _loginItemRegistered) {
        _addNotice('Start at login could not be disabled.');
      }
    } catch (_) {
      // Restore the last confirmed system state after a failed update.
      _launchAtLogin = _loginItemRegistered;
      _addNotice(
        'Start at login could not be updated. You can continue without it.',
      );
    }
  }

  void _addNotice(String message) {
    _noticeMessage = switch (_noticeMessage) {
      null => message,
      final current => '$current $message',
    };
  }
}
