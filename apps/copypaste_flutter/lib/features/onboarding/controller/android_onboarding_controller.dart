import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../platform/android/android_capture_setup_gateway.dart';
import '../../../platform/clipboard/clipboard_writer.dart';
import '../repository/android_onboarding_store.dart';

enum AndroidOnboardingStep { welcome, capture, sync }

class AndroidOnboardingController extends ChangeNotifier {
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
  bool _complete = false;
  bool _busy = false;
  bool _verifying = false;
  bool _verified = false;
  bool _disposed = false;
  int? _verificationBaseline;
  String? _errorMessage;
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
  bool get complete => _complete;
  bool get busy => _busy;
  bool get verifying => _verifying;
  bool get verified => _verified;
  String? get errorMessage => _errorMessage;
  String get adbCommandText => _setupState?.adbCommands.join('\n') ?? '';

  Future<bool> copyAdbCommands() async {
    if (adbCommandText.isEmpty) return false;
    try {
      await _clipboard.writeText(adbCommandText);
      return true;
    } catch (_) {
      _errorMessage = 'Setup commands could not be copied.';
      _notify();
      return false;
    }
  }

  bool get canContinueCapture =>
      !_busy && (_selectedMode == AndroidCaptureMode.limited || _verified);

  /// Observe Android only while the setup screen is visible and resumed.
  void setMonitoring(bool enabled) {
    if (_disposed) return;
    if (!enabled) {
      unawaited(_stateSubscription?.cancel());
      _stateSubscription = null;
      return;
    }
    if (_stateSubscription != null) return;
    _stateSubscription = _setup.changes.listen(
      (state) {
        _acceptState(state);
        _notify();
        _applyAuthorizedSetup();
      },
      onError: (Object error) {
        _errorMessage = 'Android setup monitoring is unavailable.';
        _notify();
      },
    );
    unawaited(refresh());
  }

  Future<void> initialize() async {
    if (_initialized) return;
    _busy = true;
    _notify();
    try {
      final progress = await _store.read();
      _method = progress.method;
      _complete = progress.complete;
      _step = progress.captureStarted
          ? AndroidOnboardingStep.capture
          : AndroidOnboardingStep.welcome;
      _verificationBaseline = progress.verificationBaseline;
      _verifying = _verificationBaseline != null;
      _acceptState(await _setup.state());
      _selectedMode = progress.complete || mode == AndroidCaptureMode.full
          ? mode
          : progress.mode;
      if (_complete && !await _setup.setForegroundCaptureEnabled(true)) {
        _errorMessage = 'Clipboard intake could not be enabled.';
      } else {
        _errorMessage = null;
      }
    } catch (_) {
      _errorMessage = 'CopyPaste could not read Android setup state.';
    } finally {
      _initialized = true;
      _busy = false;
      _notify();
    }
  }

  void showCapture() {
    if (_busy || _step != AndroidOnboardingStep.welcome) return;
    _step = AndroidOnboardingStep.capture;
    unawaited(_saveCaptureProgress());
    _notify();
    _applyAuthorizedSetup();
  }

  void showPreviousStep() {
    if (_busy) return;
    _step = switch (_step) {
      AndroidOnboardingStep.welcome => AndroidOnboardingStep.welcome,
      AndroidOnboardingStep.capture => AndroidOnboardingStep.welcome,
      AndroidOnboardingStep.sync => AndroidOnboardingStep.capture,
    };
    _notify();
  }

  Future<void> selectMode(AndroidCaptureMode mode) async {
    if (_busy || _selectedMode == mode) return;
    if (mode == AndroidCaptureMode.limited &&
        (_setupState?.captureEnabled ?? false)) {
      await _runStateAction(_setup.stopCapture);
      if (_setupState?.captureEnabled ?? true) {
        _errorMessage = 'Background capture could not be stopped.';
        _notify();
        return;
      }
    }
    _selectedMode = mode;
    _errorMessage = null;
    _notify();
    await _saveChoice();
    _applyAuthorizedSetup();
  }

  Future<void> selectMethod(AndroidCaptureSetupMethod method) async {
    if (_busy || _method == method) return;
    _method = method;
    _errorMessage = null;
    _notify();
    await _saveChoice();
    _applyAuthorizedSetup();
  }

  Future<void> refresh() async {
    if (_disposed) return;
    try {
      _acceptState(await _setup.state());
      _notify();
      _applyAuthorizedSetup();
    } catch (_) {
      _errorMessage = 'Android capture state could not be refreshed.';
      _notify();
    }
  }

  Future<void> requestNotifications() =>
      _runStateAction(_setup.requestNotifications);

  Future<void> requestBatteryExemption() async {
    if (_busy) return;
    try {
      if (!await _setup.requestBatteryExemption()) {
        _errorMessage = 'Battery settings could not be opened.';
        _notify();
      }
    } catch (_) {
      _errorMessage = 'Battery settings could not be opened.';
      _notify();
    }
  }

  Future<void> openShizuku() async {
    if (_busy) return;
    try {
      if (!await _setup.openShizuku()) {
        _errorMessage = 'Shizuku could not be opened.';
        _notify();
      }
    } catch (_) {
      _errorMessage = 'Shizuku could not be opened.';
      _notify();
    }
  }

  Future<void> applyShizukuGrants() async {
    if (_busy || _disposed) return;
    _automaticGrantsAttempted = true;
    _grantError = false;
    await _runStateAction(_setup.applyShizukuGrants);
    if (_setupState?.privilegedGrants != true && _errorMessage == null) {
      _grantError = true;
      _errorMessage = _setupState?.shizuku.permission == true
          ? 'CopyPaste is allowed in Shizuku, but Android capture grants could not be applied.'
          : 'Allow CopyPaste in Shizuku to apply capture grants.';
      _notify();
    }
  }

  void _applyAuthorizedSetup() {
    final state = _setupState;
    if (_disposed ||
        _busy ||
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
    if (_disposed) return;
    _setupState = state;
    if (state.privilegedGrants && _grantError) {
      _grantError = false;
      _errorMessage = null;
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
    if (_busy) return;
    _busy = true;
    _errorMessage = null;
    _notify();
    try {
      final before = await _setup.state();
      if (!before.privilegedGrants) {
        _errorMessage = 'Grant background clipboard access first.';
        return;
      }
      if (!before.notificationGranted) {
        _errorMessage =
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
        _errorMessage = 'Background capture could not be started.';
        return;
      }
    } catch (_) {
      _errorMessage = 'Background capture could not be started.';
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<void> continueFromCapture() async {
    if (!canContinueCapture) return;
    _busy = true;
    _errorMessage = null;
    _notify();
    try {
      if (!await _setup.setForegroundCaptureEnabled(true)) {
        _errorMessage = 'Clipboard intake could not be enabled.';
        return;
      }
      // Capture setup is complete before the optional pairing screen. A process
      // restart must not require the user to repeat verified permissions.
      await _store.markComplete(mode: _selectedMode, method: _method);
      _step = AndroidOnboardingStep.sync;
    } catch (_) {
      _errorMessage = 'CopyPaste could not save onboarding progress.';
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<bool> finish() async {
    if (_busy || _step != AndroidOnboardingStep.sync) return false;
    _busy = true;
    _notify();
    try {
      await _store.markComplete(mode: _selectedMode, method: _method);
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

  Future<bool> reopenCaptureSetup() async {
    if (_busy) return false;
    _busy = true;
    _errorMessage = null;
    _notify();
    try {
      _acceptState(await _setup.state());
      await _store.writeCaptureProgress();
      _complete = false;
      _step = AndroidOnboardingStep.capture;
      _selectedMode = mode;
      _automaticGrantsAttempted = false;
      return true;
    } catch (_) {
      _errorMessage = 'Android capture setup could not be opened.';
      return false;
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<void> _runStateAction(
    Future<AndroidCaptureSetupState> Function() action,
  ) async {
    if (_busy) return;
    _busy = true;
    _errorMessage = null;
    _notify();
    try {
      _acceptState(await action());
    } catch (_) {
      _errorMessage = 'Android capture state could not be refreshed.';
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<void> _saveChoice() async {
    try {
      await _store.writeChoice(mode: _selectedMode, method: _method);
    } catch (_) {
      _errorMessage = 'Android setup choice could not be saved.';
      _notify();
    }
  }

  Future<void> _saveCaptureProgress() async {
    try {
      await _store.writeCaptureProgress();
    } catch (_) {
      _errorMessage = 'CopyPaste could not save onboarding progress.';
      _notify();
    }
  }

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_stateSubscription?.cancel());
    super.dispose();
  }
}
