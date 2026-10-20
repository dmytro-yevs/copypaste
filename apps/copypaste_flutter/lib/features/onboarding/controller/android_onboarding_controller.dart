import 'dart:async';

import 'onboarding_controller.dart';

import '../../../platform/android/android_capture_setup_gateway.dart';
import '../../../platform/clipboard/clipboard_writer.dart';
import '../repository/android_onboarding_store.dart';

enum AndroidOnboardingStep { welcome, capture, sync }

class AndroidOnboardingController extends OnboardingController {
  AndroidOnboardingController({
    required AndroidOnboardingStore store,
    required AndroidCaptureSetupGateway setup,
    ClipboardWriter clipboard = const SystemClipboardWriter(),
  }) : _store = store,
       _setup = setup,
       _clipboard = clipboard;

  final AndroidOnboardingStore _store;
  final AndroidCaptureSetupGateway _setup;
  final ClipboardWriter _clipboard;

  AndroidOnboardingStep _step = AndroidOnboardingStep.welcome;
  AndroidCaptureMode _selectedMode = AndroidCaptureMode.full;
  AndroidCaptureSetupMethod _method = AndroidCaptureSetupMethod.shizuku;
  AndroidCaptureSetupState? _setupState;
  bool _initialized = false;
  bool _verifying = false;
  bool _verified = false;
  int? _verificationBaseline;
  StreamSubscription<AndroidCaptureSetupState>? _stateSubscription;
  bool _automaticGrantsAttempted = false;
  bool _grantError = false;

  AndroidOnboardingStep get step => _step;
  AndroidCaptureMode get mode => _setupState?.backgroundCaptureRunning == true
      ? AndroidCaptureMode.full
      : AndroidCaptureMode.limited;
  AndroidCaptureMode get selectedMode => _selectedMode;
  AndroidCaptureSetupMethod get method => _method;
  AndroidCaptureSetupState? get setupState => _setupState;
  bool get initialized => _initialized;
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

  bool get canContinueCapture =>
      !busy && (_selectedMode == AndroidCaptureMode.limited || _verified);

  /// Observe Android only while the setup screen is visible and resumed.
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

  Future<void> initialize() async {
    if (_initialized) return;
    busy = true;
    notifyIfActive();
    try {
      final progress = await _store.read();
      _method = progress.method;
      complete = progress.complete;
      _step = progress.captureStarted
          ? AndroidOnboardingStep.capture
          : AndroidOnboardingStep.welcome;
      _verificationBaseline = progress.verificationBaseline;
      _verifying = _verificationBaseline != null;
      _acceptState(await _setup.state());
      _selectedMode = progress.complete || mode == AndroidCaptureMode.full
          ? mode
          : progress.mode;
      if (complete && !await _setup.setForegroundCaptureEnabled(true)) {
        errorMessage = 'Clipboard intake could not be enabled.';
      } else {
        errorMessage = null;
      }
    } catch (_) {
      errorMessage = 'CopyPaste could not read Android setup state.';
    } finally {
      _initialized = true;
      busy = false;
      notifyIfActive();
    }
  }

  void showCapture() {
    if (busy || _step != AndroidOnboardingStep.welcome) return;
    _step = AndroidOnboardingStep.capture;
    unawaited(_saveCaptureProgress());
    notifyIfActive();
    _applyAuthorizedSetup();
  }

  void showPreviousStep() {
    if (busy) return;
    _step = switch (_step) {
      AndroidOnboardingStep.welcome => AndroidOnboardingStep.welcome,
      AndroidOnboardingStep.capture => AndroidOnboardingStep.welcome,
      AndroidOnboardingStep.sync => AndroidOnboardingStep.capture,
    };
    notifyIfActive();
  }

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

  Future<void> refresh() async {
    if (disposed) return;
    try {
      _acceptState(await _setup.state());
      notifyIfActive();
      _applyAuthorizedSetup();
    } catch (_) {
      errorMessage = 'Android capture state could not be refreshed.';
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
        _step != AndroidOnboardingStep.capture ||
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

  Future<void> continueFromCapture() async {
    if (!canContinueCapture) return;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      if (!await _setup.setForegroundCaptureEnabled(true)) {
        errorMessage = 'Clipboard intake could not be enabled.';
        return;
      }
      // Capture setup is complete before the optional pairing screen. A process
      // restart must not require the user to repeat verified permissions.
      await _store.markComplete(mode: _selectedMode, method: _method);
      _step = AndroidOnboardingStep.sync;
    } catch (_) {
      errorMessage = 'CopyPaste could not save onboarding progress.';
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<bool> finish() => finishOnboarding(
    canFinish: _step == AndroidOnboardingStep.sync,
    persist: () => _store.markComplete(mode: _selectedMode, method: _method),
  );

  Future<bool> reopenCaptureSetup() async {
    if (busy) return false;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      _acceptState(await _setup.state());
      await _store.writeCaptureProgress();
      complete = false;
      _step = AndroidOnboardingStep.capture;
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
      errorMessage = 'Android capture state could not be refreshed.';
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
