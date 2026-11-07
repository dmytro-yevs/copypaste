import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import 'onboarding_setup.dart';

enum OnboardingStep { welcome, setup, ready }

/// Welcome → platform setup → ready flow shared by every platform.
class OnboardingController extends ChangeNotifier {
  OnboardingController({required OnboardingSetup setup}) : _setup = setup {
    _setup.addListener(notifyIfActive);
  }

  final OnboardingSetup _setup;
  OnboardingStep _step = OnboardingStep.welcome;
  bool _initialized = false;
  bool _busy = false;
  bool _complete = false;
  bool _disposed = false;
  bool _attached = false;
  bool _foreground = true;
  bool _monitoring = false;
  String? _errorMessage;

  OnboardingSetup get setup => _setup;
  OnboardingStep get step => _step;
  bool get initialized => _initialized;
  bool get complete => _complete;
  bool get busy => _busy || _setup.busy;
  String? get errorMessage => _errorMessage ?? _setup.errorMessage;
  String? get noticeMessage => _setup.noticeMessage;
  bool get canGoBack => _step == OnboardingStep.setup && !busy;
  bool get canContinue => switch (_step) {
    OnboardingStep.setup => _setup.canContinue && !_busy,
    OnboardingStep.welcome || OnboardingStep.ready => !busy,
  };

  Future<void> initialize() async {
    if (_initialized || _disposed) return;
    _busy = true;
    notifyIfActive();
    try {
      final resume = await _setup.restore();
      _complete = resume == OnboardingResume.complete;
      _step = resume == OnboardingResume.setup
          ? OnboardingStep.setup
          : OnboardingStep.welcome;
    } catch (_) {
      _errorMessage = AppStrings.current.onboardingStateReadFailed(
        _setup.platformLabel,
      );
    } finally {
      _initialized = true;
      _busy = false;
      _syncMonitoring();
      notifyIfActive();
    }
  }

  Future<bool> showSetup() async {
    if (busy || _complete || _disposed || _step != OnboardingStep.welcome) {
      return false;
    }
    await _enterSetup();
    return true;
  }

  void showPreviousStep() {
    if (busy || _complete || _disposed) return;
    switch (_step) {
      case OnboardingStep.welcome:
        return;
      case OnboardingStep.setup:
        _step = OnboardingStep.welcome;
        _setup.leave();
        _syncMonitoring();
        notifyIfActive();
      case OnboardingStep.ready:
        unawaited(_enterSetup());
    }
  }

  Future<bool> continueFromSetup() async {
    if (busy ||
        _complete ||
        _disposed ||
        _step != OnboardingStep.setup ||
        !_setup.canContinue) {
      return false;
    }
    final advanced = await _setup.apply();
    if (!advanced || _disposed || _step != OnboardingStep.setup) return false;
    _step = OnboardingStep.ready;
    _setup.leave();
    _syncMonitoring();
    notifyIfActive();
    return true;
  }

  Future<bool> finish() async {
    if (_disposed || busy || _step != OnboardingStep.ready) return false;
    _busy = true;
    _errorMessage = null;
    notifyIfActive();
    try {
      await _setup.persistCompletion();
      if (_disposed) return false;
      _complete = true;
      return true;
    } catch (_) {
      if (!_disposed) {
        _errorMessage = AppStrings.current.onboardingSaveProgressFailed;
      }
      return false;
    } finally {
      _busy = false;
      notifyIfActive();
    }
  }

  /// Returns completed onboarding to the setup step, when supported.
  Future<bool> reopenSetup() async {
    if (busy || _disposed || !_setup.reopenable) return false;
    if (!await _setup.reopen() || _disposed) return false;
    _complete = false;
    _step = OnboardingStep.setup;
    _syncMonitoring();
    notifyIfActive();
    return true;
  }

  /// Tracks whether the onboarding screen is mounted.
  void setAttached(bool attached) {
    _attached = attached;
    _syncMonitoring();
  }

  /// Follows the app lifecycle; resuming refreshes platforms without streams.
  void setForeground(bool foreground) {
    final resumed = foreground && !_foreground;
    _foreground = foreground;
    _syncMonitoring();
    if (resumed && _setupVisible && !_setup.monitorsChanges) {
      unawaited(_setup.refresh());
    }
  }

  bool get _setupVisible =>
      !_disposed &&
      _initialized &&
      _attached &&
      _foreground &&
      !_complete &&
      _step == OnboardingStep.setup;

  Future<void> _enterSetup() async {
    _step = OnboardingStep.setup;
    notifyIfActive();
    await _setup.enter();
    _syncMonitoring();
  }

  void _syncMonitoring() {
    if (!_setup.monitorsChanges) return;
    final active = _setupVisible;
    if (active == _monitoring) return;
    _monitoring = active;
    _setup.setMonitoring(active);
  }

  @protected
  void notifyIfActive() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _setup.removeListener(notifyIfActive);
    if (_monitoring) _setup.setMonitoring(false);
    _setup.dispose();
    super.dispose();
  }
}
