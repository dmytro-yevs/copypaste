import 'package:flutter/foundation.dart';

/// Shared completion lifecycle; each platform owns its setup steps.
abstract class OnboardingController extends ChangeNotifier {
  bool _busy = false;
  bool _complete = false;
  bool _disposed = false;
  String? _errorMessage;
  bool get busy => _busy;
  bool get complete => _complete;
  String? get errorMessage => _errorMessage;
  @protected
  bool get disposed => _disposed;
  @protected
  set busy(bool value) => _busy = value;
  @protected
  set complete(bool value) => _complete = value;
  @protected
  set errorMessage(String? value) => _errorMessage = value;
  @protected
  void notifyIfActive() {
    if (!_disposed) notifyListeners();
  }

  @protected
  Future<bool> finishOnboarding({
    required bool canFinish,
    required Future<void> Function() persist,
  }) async {
    if (_disposed || _busy || !canFinish) return false;
    _busy = true;
    _errorMessage = null;
    notifyIfActive();
    try {
      await persist();
      if (_disposed) return false;
      _complete = true;
      return true;
    } catch (_) {
      if (!_disposed) {
        _errorMessage = 'CopyPaste could not save onboarding progress.';
      }
      return false;
    } finally {
      _busy = false;
      notifyIfActive();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
