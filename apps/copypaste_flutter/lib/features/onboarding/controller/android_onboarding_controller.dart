import 'package:flutter/foundation.dart';

import '../../../platform/android/android_capture_setup_gateway.dart';
import '../repository/android_onboarding_store.dart';

enum AndroidOnboardingStep { welcome, capture, sync }

class AndroidOnboardingController extends ChangeNotifier {
  AndroidOnboardingController({
    required AndroidOnboardingStore store,
    required AndroidCaptureSetupGateway setup,
  }) : _store = store,
       _setup = setup;

  final AndroidOnboardingStore _store;
  final AndroidCaptureSetupGateway _setup;

  AndroidOnboardingStep _step = AndroidOnboardingStep.welcome;
  AndroidCaptureMode _mode = AndroidCaptureMode.full;
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

  AndroidOnboardingStep get step => _step;
  AndroidCaptureMode get mode => _mode;
  AndroidCaptureSetupMethod get method => _method;
  AndroidCaptureSetupState? get setupState => _setupState;
  bool get initialized => _initialized;
  bool get complete => _complete;
  bool get busy => _busy;
  bool get verifying => _verifying;
  bool get verified => _verified;
  String? get errorMessage => _errorMessage;

  bool get canContinueCapture =>
      !_busy && (_mode == AndroidCaptureMode.limited || _verified);

  Future<void> initialize() async {
    if (_initialized) return;
    _busy = true;
    _notify();
    try {
      final progress = await _store.read();
      _mode = progress.mode;
      _method = progress.method;
      _setupState = await _setup.state();
      _complete = progress.complete;
      if (_complete && !await _setup.setForegroundCaptureEnabled(true)) {
        _complete = false;
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
    _notify();
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
    if (_busy || _mode == mode) return;
    if (mode == AndroidCaptureMode.limited &&
        (_setupState?.captureEnabled ?? false)) {
      await _runStateAction(_setup.stopCapture);
      if (_setupState?.captureEnabled ?? true) {
        _errorMessage = 'Background capture could not be stopped.';
        _notify();
        return;
      }
    }
    _mode = mode;
    _errorMessage = null;
    _notify();
    await _saveChoice();
  }

  Future<void> selectMethod(AndroidCaptureSetupMethod method) async {
    if (_busy || _method == method) return;
    _method = method;
    _errorMessage = null;
    _notify();
    await _saveChoice();
  }

  Future<void> refresh() => _runStateAction(_setup.state);

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

  Future<void> applyShizukuGrants() =>
      _runStateAction(_setup.applyShizukuGrants);

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
      _verificationBaseline = before.lastCaptureAtMs;
      _setupState = await _setup.startCapture();
      if (!_setupState!.captureEnabled) {
        _errorMessage = 'Background capture could not be started.';
        return;
      }
      _verifying = true;
      _verified = false;
    } catch (_) {
      _errorMessage = 'Background capture could not be started.';
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<void> continueFromCapture() async {
    if (!canContinueCapture) return;
    if (!await _setup.setForegroundCaptureEnabled(true)) {
      _errorMessage = 'Clipboard intake could not be enabled.';
      _notify();
      return;
    }
    _step = AndroidOnboardingStep.sync;
    await _saveChoice();
    _notify();
  }

  Future<bool> finish() async {
    if (_busy || _step != AndroidOnboardingStep.sync) return false;
    _busy = true;
    _notify();
    try {
      await _store.markComplete(mode: _mode, method: _method);
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
      await _store.resetCompletion();
      _setupState = await _setup.state();
      _complete = false;
      _step = AndroidOnboardingStep.capture;
      _verificationBaseline = null;
      _verifying = false;
      _verified = false;
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
      _setupState = await action();
      final baseline = _verificationBaseline;
      if (_verifying &&
          baseline != null &&
          _setupState!.serviceRunning &&
          _setupState!.lastCaptureAtMs > baseline) {
        _verified = true;
        _verifying = false;
      }
    } catch (_) {
      _errorMessage = 'Android capture state could not be refreshed.';
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<void> _saveChoice() async {
    try {
      await _store.writeChoice(mode: _mode, method: _method);
    } catch (_) {
      _errorMessage = 'Android setup choice could not be saved.';
      _notify();
    }
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
