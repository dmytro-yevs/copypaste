import 'dart:async';

import '../../../platform/android/android_capture_setup_gateway.dart';
import '../../../platform/clipboard/clipboard_writer.dart';
import '../repository/android_onboarding_store.dart';
import 'onboarding_setup.dart';

/// Full or Limited clipboard capture setup for Android.
class AndroidOnboardingSetup extends OnboardingSetup {
  AndroidOnboardingSetup({
    required AndroidOnboardingStore store,
    required AndroidCaptureSetupGateway setup,
    ClipboardWriter clipboard = const SystemClipboardWriter(),
  }) : _store = store,
       _setup = setup,
       _clipboard = clipboard;

  static const _stateReadFailure =
      'CopyPaste could not read Android setup state.';
  static const _stateRefreshFailure =
      'Android capture state could not be refreshed.';

  final AndroidOnboardingStore _store;
  final AndroidCaptureSetupGateway _setup;
  final ClipboardWriter _clipboard;

  bool _current = false;
  AndroidCaptureMode _selectedMode = AndroidCaptureMode.full;
  AndroidCaptureSetupMethod _method = AndroidCaptureSetupMethod.shizuku;
  AndroidCaptureSetupState? _setupState;
  bool _verifying = false;
  bool _verified = false;
  int? _verificationBaseline;
  StreamSubscription<AndroidCaptureSetupState>? _stateSubscription;
  bool _automaticGrantsAttempted = false;
  bool _grantError = false;

  @override
  String get platformLabel => 'Android';
  @override
  bool get monitorsChanges => true;
  @override
  bool get reopenable => true;
  AndroidCaptureMode get mode => _setupState?.backgroundCaptureRunning == true
      ? AndroidCaptureMode.full
      : AndroidCaptureMode.limited;
  AndroidCaptureMode get selectedMode => _selectedMode;
  AndroidCaptureSetupMethod get method => _method;
  AndroidCaptureSetupState? get setupState => _setupState;
  bool get verifying => _verifying;
  bool get verified => _verified;
  String get adbCommandText => _setupState?.adbCommands.join('\n') ?? '';

  Future<bool> copyAdbCommands() async {
    if (adbCommandText.isEmpty) return false;
    try {
      await _clipboard.writeText(adbCommandText);
      return true;
    } catch (_) {
      errorMessage = 'Setup commands could not be copied.';
      notifyIfActive();
      return false;
    }
  }

  @override
  bool get canContinue =>
      !busy && (_selectedMode == AndroidCaptureMode.limited || _verified);

  /// Observe Android only while the setup step is visible and resumed.
  @override
  void setMonitoring(bool enabled) {
    if (disposed) return;
    if (!enabled) {
      unawaited(_stateSubscription?.cancel());
      _stateSubscription = null;
      return;
    }
    if (_stateSubscription != null) return;
    _stateSubscription = _setup.changes.listen(
      (state) {
        _acceptState(state);
        notifyIfActive();
        _applyAuthorizedSetup();
      },
      onError: (Object error) {
        errorMessage = 'Android setup monitoring is unavailable.';
        notifyIfActive();
      },
    );
    unawaited(refresh());
  }

  @override
  Future<OnboardingResume> restore() async {
    busy = true;
    notifyIfActive();
    // Saved progress decides the resume step; a later failure must not reset it.
    var resume = OnboardingResume.welcome;
    try {
      final progress = await _store.read();
      _method = progress.method;
      _current = !progress.complete && progress.captureStarted;
      resume = progress.complete
          ? OnboardingResume.complete
          : _current
          ? OnboardingResume.setup
          : OnboardingResume.welcome;
      _verificationBaseline = progress.verificationBaseline;
      _verifying = _verificationBaseline != null;
      _acceptState(await _setup.state());
      _selectedMode = progress.complete || mode == AndroidCaptureMode.full
          ? mode
          : progress.mode;
      if (progress.complete &&
          !await _setup.setForegroundCaptureEnabled(true)) {
        errorMessage = 'Clipboard intake could not be enabled.';
      } else {
        errorMessage = null;
      }
    } catch (_) {
      errorMessage = _stateReadFailure;
    } finally {
      busy = false;
      notifyIfActive();
    }
    return resume;
  }

  @override
  Future<void> enter() async {
    _current = true;
    unawaited(_saveCaptureProgress());
    _applyAuthorizedSetup();
  }

  @override
  void leave() => _current = false;

  Future<void> selectMode(AndroidCaptureMode mode) async {
    if (busy || _selectedMode == mode) return;
    if (mode == AndroidCaptureMode.limited &&
        (_setupState?.captureEnabled ?? false)) {
      await _runStateAction(_setup.stopCapture);
      if (_setupState?.captureEnabled ?? true) {
        errorMessage = 'Background capture could not be stopped.';
        notifyIfActive();
        return;
      }
    }
    _selectedMode = mode;
    errorMessage = null;
    notifyIfActive();
    await _saveChoice();
    _applyAuthorizedSetup();
  }

  Future<void> selectMethod(AndroidCaptureSetupMethod method) async {
    if (busy || _method == method) return;
    _method = method;
    errorMessage = null;
    notifyIfActive();
    await _saveChoice();
    _applyAuthorizedSetup();
  }

  @override
  Future<void> refresh() async {
    if (disposed) return;
    try {
      _acceptState(await _setup.state());
      notifyIfActive();
      _applyAuthorizedSetup();
    } catch (_) {
      errorMessage = _stateRefreshFailure;
      notifyIfActive();
    }
  }

  Future<void> requestNotifications() =>
      _runStateAction(_setup.requestNotifications);

  Future<void> requestBatteryExemption() async {
    if (busy) return;
    try {
      if (!await _setup.requestBatteryExemption()) {
        errorMessage = 'Battery settings could not be opened.';
        notifyIfActive();
      }
    } catch (_) {
      errorMessage = 'Battery settings could not be opened.';
      notifyIfActive();
    }
  }

  Future<void> openShizuku() async {
    if (busy) return;
    try {
      if (!await _setup.openShizuku()) {
        errorMessage = 'Shizuku could not be opened.';
        notifyIfActive();
      }
    } catch (_) {
      errorMessage = 'Shizuku could not be opened.';
      notifyIfActive();
    }
  }

  Future<void> applyShizukuGrants() async {
    if (busy || disposed) return;
    _automaticGrantsAttempted = true;
    _grantError = false;
    await _runStateAction(_setup.applyShizukuGrants);
    if (_setupState?.privilegedGrants != true && errorMessage == null) {
      _grantError = true;
      errorMessage = _setupState?.shizuku.permission == true
          ? 'CopyPaste is allowed in Shizuku, but Android capture grants could not be applied.'
          : 'Allow CopyPaste in Shizuku to apply capture grants.';
      notifyIfActive();
    }
  }

  void _applyAuthorizedSetup() {
    final state = _setupState;
    if (disposed ||
        busy ||
        _automaticGrantsAttempted ||
        !_current ||
        _selectedMode != AndroidCaptureMode.full ||
        _method != AndroidCaptureSetupMethod.shizuku ||
        state == null ||
        state.privilegedGrants ||
        !state.shizuku.running ||
        !state.shizuku.permission) {
      return;
    }
    unawaited(applyShizukuGrants());
  }

  void _acceptState(AndroidCaptureSetupState state) {
    if (disposed) return;
    _setupState = state;
    if (errorMessage == _stateReadFailure ||
        errorMessage == _stateRefreshFailure) {
      errorMessage = null;
    }
    if (state.privilegedGrants && _grantError) {
      _grantError = false;
      errorMessage = null;
    }
    if (!state.shizuku.running || !state.shizuku.permission) {
      _automaticGrantsAttempted = false;
    }
    final baseline = _verificationBaseline;
    _verified =
        state.backgroundCaptureRunning &&
        state.lastCaptureAtMs > (baseline ?? 0);
    _verifying =
        !_verified &&
        baseline != null &&
        state.privilegedGrants &&
        state.notificationGranted &&
        state.captureEnabled;
    if (state.backgroundCaptureRunning) {
      _selectedMode = AndroidCaptureMode.full;
    }
  }

  Future<void> beginVerification() async {
    if (busy) return;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      final before = await _setup.state();
      if (!before.privilegedGrants) {
        errorMessage = 'Grant background clipboard access first.';
        return;
      }
      if (!before.notificationGranted) {
        errorMessage =
            'Allow notifications before starting background capture.';
        return;
      }
      _verificationBaseline = before.observedAtMs > before.lastCaptureAtMs
          ? before.observedAtMs
          : before.lastCaptureAtMs;
      await _store.writeCaptureProgress(
        verificationBaseline: _verificationBaseline,
      );
      _acceptState(await _setup.startCapture());
      if (!_setupState!.captureEnabled) {
        errorMessage = 'Background capture could not be started.';
        return;
      }
    } catch (_) {
      errorMessage = 'Background capture could not be started.';
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  @override
  Future<bool> apply() async {
    if (!canContinue) return false;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      if (!await _setup.setForegroundCaptureEnabled(true)) {
        errorMessage = 'Clipboard intake could not be enabled.';
        return false;
      }
      // Capture setup is complete before the optional pairing screen. A process
      // restart must not require the user to repeat verified permissions.
      await _store.markComplete(mode: _selectedMode, method: _method);
      return true;
    } catch (_) {
      errorMessage = 'CopyPaste could not save onboarding progress.';
      return false;
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  @override
  Future<void> persistCompletion() =>
      _store.markComplete(mode: _selectedMode, method: _method);

  @override
  Future<bool> reopen() async {
    if (busy) return false;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      _acceptState(await _setup.state());
      await _store.writeCaptureProgress();
      _current = true;
      _selectedMode = mode;
      _automaticGrantsAttempted = false;
      return true;
    } catch (_) {
      errorMessage = 'Android capture setup could not be opened.';
      return false;
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> _runStateAction(
    Future<AndroidCaptureSetupState> Function() action,
  ) async {
    if (busy) return;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      _acceptState(await action());
    } catch (_) {
      errorMessage = _stateRefreshFailure;
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> _saveChoice() async {
    try {
      await _store.writeChoice(mode: _selectedMode, method: _method);
    } catch (_) {
      errorMessage = 'Android setup choice could not be saved.';
      notifyIfActive();
    }
  }

  Future<void> _saveCaptureProgress() async {
    try {
      await _store.writeCaptureProgress();
    } catch (_) {
      errorMessage = 'CopyPaste could not save onboarding progress.';
      notifyIfActive();
    }
  }

  @override
  void dispose() {
    unawaited(_stateSubscription?.cancel());
    super.dispose();
  }
}
