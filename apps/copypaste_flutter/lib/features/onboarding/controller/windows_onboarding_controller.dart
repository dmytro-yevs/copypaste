import 'onboarding_controller.dart';

import '../repository/onboarding_store.dart';

enum WindowsOnboardingStep { welcome, sync }

class WindowsOnboardingController extends OnboardingController {
  WindowsOnboardingController({required OnboardingStore store})
    : _store = store;

  final OnboardingStore _store;
  WindowsOnboardingStep _step = WindowsOnboardingStep.welcome;
  bool _initialized = false;

  WindowsOnboardingStep get step => _step;

  Future<void> initialize() async {
    if (_initialized) return;
    busy = true;
    notifyIfActive();
    try {
      complete = await _store.isComplete();
    } catch (_) {
      errorMessage = 'CopyPaste could not read Windows setup state.';
    } finally {
      _initialized = true;
      busy = false;
      notifyIfActive();
    }
  }

  void showReady() {
    if (busy) return;
    _step = WindowsOnboardingStep.sync;
    notifyIfActive();
  }

  Future<bool> finish() => finishOnboarding(
    canFinish: _step == WindowsOnboardingStep.sync,
    persist: _store.markComplete,
  );
}
