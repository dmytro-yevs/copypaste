import 'package:flutter/foundation.dart';

import '../../../platform/macos/macos_setup_gateway.dart';
import '../repository/macos_onboarding_store.dart';

enum MacosOnboardingStep { welcome, setup, sync }

class MacosOnboardingController extends ChangeNotifier {
  MacosOnboardingController({
    required MacosOnboardingStore store,
    required MacosSetupGateway setup,
  }) : _store = store,
       _setup = setup;

  final MacosOnboardingStore _store;
  final MacosSetupGateway _setup;

  MacosOnboardingStep _step = MacosOnboardingStep.welcome;
  MacosLoginItemStatus _loginItemStatus = MacosLoginItemStatus.notRegistered;
  bool _initialized = false;
  bool _complete = false;
  bool _busy = false;
  bool _accessibilityGranted = false;
  bool _launchAtLogin = true;
  bool _loginDefaultPending = false;
  bool _disposed = false;
  String? _errorMessage;
  String? _noticeMessage;

  MacosOnboardingStep get step => _step;
  MacosLoginItemStatus get loginItemStatus => _loginItemStatus;
  bool get initialized => _initialized;
  bool get complete => _complete;
  bool get busy => _busy;
  bool get accessibilityGranted => _accessibilityGranted;
  bool get launchAtLogin => _launchAtLogin;
  bool get launchAtLoginAvailable =>
      _loginItemStatus != MacosLoginItemStatus.developmentUnavailable &&
      _loginItemStatus != MacosLoginItemStatus.unavailable;
  bool get loginItemNeedsAttention =>
      _launchAtLogin &&
      launchAtLoginAvailable &&
      _loginItemStatus != MacosLoginItemStatus.enabled;
  String? get errorMessage => _errorMessage;
  String? get noticeMessage => _noticeMessage;

  bool get canContinueSetup => !_busy;

  Future<void> initialize() async {
    if (_initialized) return;
    _busy = true;
    _notify();
    try {
      final wasComplete = await _store.isComplete();
      _loginDefaultPending = !wasComplete;
      await _refreshSystemState();
      _complete = wasComplete;
      _errorMessage = null;
    } catch (_) {
      _errorMessage = 'CopyPaste could not read the macOS setup state.';
    } finally {
      _initialized = true;
      _busy = false;
      _notify();
    }
  }

  void showSetup() {
    if (_busy || _step != MacosOnboardingStep.welcome) return;
    _step = MacosOnboardingStep.setup;
    _notify();
  }

  void showPreviousStep() {
    if (_busy) return;
    _step = switch (_step) {
      MacosOnboardingStep.welcome => MacosOnboardingStep.welcome,
      MacosOnboardingStep.setup => MacosOnboardingStep.welcome,
      MacosOnboardingStep.sync => MacosOnboardingStep.setup,
    };
    _notify();
  }

  Future<void> setLaunchAtLogin(bool enabled) async {
    if (_busy || !launchAtLoginAvailable || _launchAtLogin == enabled) return;
    _busy = true;
    _errorMessage = null;
    _noticeMessage = null;
    _notify();
    try {
      await _updateLaunchAtLogin(enabled);
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<void> requestAccessibility() async {
    if (_busy || _accessibilityGranted) return;
    _busy = true;
    _errorMessage = null;
    _notify();
    try {
      _accessibilityGranted = await _setup.requestAccessibility();
    } catch (_) {
      _errorMessage = 'Accessibility permission could not be requested.';
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<void> refreshSystemState() async {
    if (_busy) return;
    _busy = true;
    _notify();
    try {
      await _refreshSystemState();
      _errorMessage = null;
    } catch (_) {
      _errorMessage = 'CopyPaste could not refresh the macOS setup state.';
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<bool> continueFromSetup() async {
    if (_busy) return false;
    _busy = true;
    _errorMessage = null;
    _noticeMessage = null;
    _notify();
    try {
      _accessibilityGranted = await _setup.accessibilityGranted();
    } catch (_) {
      _addNotice('Accessibility status could not be refreshed.');
    }
    await _updateLaunchAtLogin(_launchAtLogin);
    _step = MacosOnboardingStep.sync;
    _busy = false;
    _notify();
    return true;
  }

  Future<void> openLoginItemsSettings() async {
    try {
      await _setup.openLoginItemsSettings();
    } catch (_) {
      _errorMessage = 'Login Items settings could not be opened.';
      _notify();
    }
  }

  Future<bool> finish() async {
    if (_busy || _step != MacosOnboardingStep.sync) return false;
    _busy = true;
    _errorMessage = null;
    _notify();
    try {
      await _store.markComplete();
      _complete = true;
      return true;
    } catch (_) {
      _errorMessage = 'CopyPaste could not save onboarding progress.';
      return false;
    } finally {
      _busy = false;
      _notify();
    }
  }

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

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
