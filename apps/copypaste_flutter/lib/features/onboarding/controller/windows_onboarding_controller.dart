import 'package:flutter/foundation.dart';

import '../repository/windows_onboarding_store.dart';

enum WindowsOnboardingStep { welcome, sync }

class WindowsOnboardingController extends ChangeNotifier {
  WindowsOnboardingController({required WindowsOnboardingStore store})
    : _store = store;

  final WindowsOnboardingStore _store;
  WindowsOnboardingStep _step = WindowsOnboardingStep.welcome;
  bool _initialized = false;
  bool _complete = false;
  bool _busy = false;
  bool _disposed = false;
  String? _errorMessage;

  WindowsOnboardingStep get step => _step;
  bool get complete => _complete;
  bool get busy => _busy;
  String? get errorMessage => _errorMessage;

  Future<void> initialize() async {
    if (_initialized) return;
    _busy = true;
    _notify();
    try {
      _complete = await _store.isComplete();
    } catch (_) {
      _errorMessage = 'CopyPaste could not read Windows setup state.';
    } finally {
      _initialized = true;
      _busy = false;
      _notify();
    }
  }

  void showReady() {
    if (_busy) return;
    _step = WindowsOnboardingStep.sync;
    _notify();
  }

  Future<bool> finish() async {
    if (_busy || _step != WindowsOnboardingStep.sync) return false;
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

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
