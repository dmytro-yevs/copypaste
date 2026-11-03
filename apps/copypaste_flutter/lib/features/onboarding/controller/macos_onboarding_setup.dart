import '../../../platform/macos/macos_setup_gateway.dart';
import '../repository/onboarding_store.dart';
import 'onboarding_setup.dart';

/// Accessibility and Login Item setup for macOS.
class MacosOnboardingSetup extends OnboardingSetup {
  MacosOnboardingSetup({
    required OnboardingStore store,
    required MacosSetupGateway setup,
  }) : _store = store,
       _setup = setup;

  final OnboardingStore _store;
  final MacosSetupGateway _setup;

  MacosLoginItemStatus _loginItemStatus = MacosLoginItemStatus.notRegistered;
  bool _accessibilityGranted = false;
  bool _launchAtLogin = true;
  bool _loginDefaultPending = false;
  String? _noticeMessage;

  @override
  String get platformLabel => 'macOS';
  MacosLoginItemStatus get loginItemStatus => _loginItemStatus;
  bool get accessibilityGranted => _accessibilityGranted;
  bool get launchAtLogin => _launchAtLogin;
  bool get launchAtLoginAvailable =>
      _loginItemStatus != MacosLoginItemStatus.developmentUnavailable &&
      _loginItemStatus != MacosLoginItemStatus.unavailable;
  bool get loginItemNeedsAttention =>
      _launchAtLogin &&
      launchAtLoginAvailable &&
      _loginItemStatus != MacosLoginItemStatus.enabled;
  @override
  String? get noticeMessage => _noticeMessage;
  @override
  bool get noticeOpensSystemSettings => loginItemNeedsAttention;

  @override
  Future<OnboardingResume> restore() async {
    busy = true;
    notifyIfActive();
    try {
      final wasComplete = await _store.isComplete();
      _loginDefaultPending = !wasComplete;
      await _refreshSystemState();
      errorMessage = null;
      return wasComplete ? OnboardingResume.complete : OnboardingResume.welcome;
    } catch (_) {
      errorMessage = 'CopyPaste could not read the macOS setup state.';
      return OnboardingResume.welcome;
    } finally {
      busy = false;
      notifyIfActive();
    }
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

  @override
  Future<void> refresh() async {
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

  @override
  Future<bool> apply() async {
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
    busy = false;
    notifyIfActive();
    return true;
  }

  @override
  Future<void> openSystemSettings() async {
    try {
      await _setup.openLoginItemsSettings();
    } catch (_) {
      errorMessage = 'Login Items settings could not be opened.';
      notifyIfActive();
    }
  }

  @override
  Future<void> persistCompletion() => _store.markComplete();

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
