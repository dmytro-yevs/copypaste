import '../../../platform/startup/launch_at_login.dart';
import '../../../l10n/l10n.dart';
import '../repository/onboarding_store.dart';
import 'onboarding_setup.dart';

/// Start-at-login setup for Windows.
class WindowsOnboardingSetup extends OnboardingSetup {
  WindowsOnboardingSetup({
    required OnboardingStore store,
    required LaunchAtLoginGateway startup,
  }) : _store = store,
       _startup = startup;

  static String get _disabledInTaskManager =>
      AppStrings.current.onboardingWindowsTaskManagerDisabled;

  final OnboardingStore _store;
  final LaunchAtLoginGateway _startup;
  LaunchAtLoginStatus _startupStatus = LaunchAtLoginStatus.unavailable;
  bool _launchAtLogin = false;
  bool _defaultPending = false;
  String? _noticeMessage;

  @override
  String get platformLabel => 'Windows';
  LaunchAtLoginStatus get startupStatus => _startupStatus;
  bool get launchAtLogin => _launchAtLogin;
  bool get launchAtLoginAvailable =>
      _startupStatus != LaunchAtLoginStatus.unavailable;

  /// Registered, but Task Manager keeps Windows from launching CopyPaste.
  bool get disabledInTaskManager =>
      _startupStatus == LaunchAtLoginStatus.requiresApproval;
  @override
  String? get noticeMessage => _noticeMessage;
  bool get _registered =>
      _startupStatus == LaunchAtLoginStatus.enabled ||
      _startupStatus == LaunchAtLoginStatus.requiresApproval;

  @override
  Future<OnboardingResume> restore() async {
    busy = true;
    notifyIfActive();
    try {
      // Completed installations keep their existing startup choice. Only new
      // onboarding offers the same opt-out default as macOS.
      if (await _store.isComplete()) return OnboardingResume.complete;
      try {
        _startupStatus = await _startup.read();
        _launchAtLogin = launchAtLoginAvailable;
        _defaultPending = !_registered;
      } catch (_) {
        _noticeMessage =
            AppStrings.current.onboardingWindowsStartAtLoginUnavailableNotice;
      }
      return OnboardingResume.welcome;
    } catch (_) {
      errorMessage = AppStrings.current.onboardingWindowsStateReadFailed;
      return OnboardingResume.welcome;
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> setLaunchAtLogin(bool enabled) async {
    if (busy ||
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

  @override
  Future<void> refresh() async {
    if (busy || disposed) return;
    busy = true;
    notifyIfActive();
    try {
      _startupStatus = await _startup.read();
      if (!_defaultPending || _registered) {
        _defaultPending = false;
        _launchAtLogin = _registered;
      } else {
        _launchAtLogin = launchAtLoginAvailable;
      }
      if (!disabledInTaskManager && _noticeMessage == _disabledInTaskManager) {
        _noticeMessage = null;
      }
    } catch (_) {
      // Keep the last confirmed state; startup remains optional.
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  @override
  Future<bool> apply() async {
    if (busy || disposed) return false;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      if (launchAtLoginAvailable) await _applyStartup(_launchAtLogin);
      return !disposed;
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  @override
  Future<void> openSystemSettings() async {
    try {
      await _startup.openSystemSettings();
    } catch (_) {
      errorMessage = AppStrings.current.onboardingWindowsStartupSettingsFailed;
      notifyIfActive();
    }
  }

  @override
  Future<void> persistCompletion() => _store.markComplete();

  Future<void> _applyStartup(bool enabled) async {
    _defaultPending = false;
    if (enabled == _registered) {
      _launchAtLogin = enabled;
      if (enabled && disabledInTaskManager) {
        _noticeMessage = _disabledInTaskManager;
      }
      return;
    }
    try {
      _startupStatus = await _startup.setEnabled(enabled);
      _launchAtLogin = _registered;
      if (_registered != enabled) {
        _noticeMessage =
            AppStrings.current.onboardingWindowsStartAtLoginChangeLater;
      } else if (enabled && disabledInTaskManager) {
        _noticeMessage = _disabledInTaskManager;
      }
    } catch (_) {
      _launchAtLogin = _registered;
      _noticeMessage = AppStrings.current.onboardingStartAtLoginUpdateFailed;
    }
  }
}
