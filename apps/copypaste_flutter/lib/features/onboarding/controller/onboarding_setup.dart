import 'package:flutter/foundation.dart';

/// Where a restored onboarding flow continues.
enum OnboardingResume { welcome, setup, complete }

/// Platform-specific middle step hosted by `OnboardingController`.
///
/// Platforms that publish setup changes override [monitorsChanges] and
/// [setMonitoring]; the others are refreshed when the app resumes.
abstract class OnboardingSetup extends ChangeNotifier {
  bool _busy = false;
  bool _disposed = false;
  String? _errorMessage;

  /// Platform name shown in the step header.
  String get platformLabel;
  bool get busy => _busy;
  String? get errorMessage => _errorMessage;

  /// Optional setup result shown without blocking onboarding.
  String? get noticeMessage => null;

  /// Whether the notice can be resolved from system settings.
  bool get noticeOpensSystemSettings => false;

  /// Setup step action label when continuing skips optional features.
  String? get continueLabel => null;

  bool get canContinue => !busy;
  bool get monitorsChanges => false;

  /// Whether completed onboarding can reopen this setup step.
  bool get reopenable => false;

  @protected
  bool get disposed => _disposed;
  @protected
  set busy(bool value) => _busy = value;
  @protected
  set errorMessage(String? value) => _errorMessage = value;
  @protected
  void notifyIfActive() {
    if (!_disposed) notifyListeners();
  }

  /// Restores persisted progress and reads the current system state.
  Future<OnboardingResume> restore();

  /// Called whenever the flow navigates to this step.
  Future<void> enter() async {}

  /// Called whenever the flow navigates away from this step.
  void leave() {}

  /// Applies setup choices; returns whether the flow may advance.
  Future<bool> apply();

  /// Persists completion after the final step.
  Future<void> persistCompletion();

  /// Re-reads system state that may have changed outside CopyPaste.
  Future<void> refresh();

  /// Starts or stops the platform change stream while setup is visible.
  void setMonitoring(bool enabled) {}

  /// Prepares completed onboarding to show this step again.
  Future<bool> reopen() async => false;

  Future<void> openSystemSettings() async {}

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
