import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../l10n/l10n.dart';
import 'onboarding_pairing.dart';
import 'onboarding_setup.dart';

export 'onboarding_pairing.dart';

enum OnboardingStep { welcome, setup, space, ready }

/// How this device starts: its own space, joining one, or neither for now.
enum OnboardingSpaceChoice { create, join, later }

/// Welcome → platform setup → space → ready flow shared by every platform.
/// The space step is offered only where a sync runtime exists.
class OnboardingController extends ChangeNotifier {
  OnboardingController({
    required OnboardingSetup setup,
    this.offersSpace = false,
  }) : _setup = setup {
    _setup.addListener(notifyIfActive);
  }

  final OnboardingSetup _setup;
  final bool offersSpace;
  OnboardingStep _step = OnboardingStep.welcome;
  OnboardingSpaceChoice _spaceChoice = OnboardingSpaceChoice.later;
  bool _initialized = false;
  bool _busy = false;
  bool _complete = false;
  bool _disposed = false;
  bool _attached = false;
  bool _foreground = true;
  bool _monitoring = false;
  String? _errorMessage;
  OnboardingPairing? _pairing;
  OnboardingPairingKind? _pairingKind;
  OnboardingStep _pairingOrigin = OnboardingStep.space;
  bool _pairingOpening = false;
  bool _paired = false;
  String? _pairedDeviceName;
  bool _pairingInterrupted = false;
  String? _pairingIssue;

  OnboardingSetup get setup => _setup;
  OnboardingStep get step => _step;
  OnboardingSpaceChoice get spaceChoice => _spaceChoice;
  bool get initialized => _initialized;
  bool get complete => _complete;
  bool get busy => _busy || _pairingOpening || _setup.busy;
  String? get errorMessage => _errorMessage ?? _setup.errorMessage;

  /// The setup notice; on the setup step, without what that step shows itself.
  String? get noticeMessage => _step == OnboardingStep.setup
      ? _setup.stepNoticeMessage
      : _setup.noticeMessage;

  /// Whether pairing can run inside the tour instead of in Devices.
  bool get pairsInline => _pairing != null;

  /// The ceremony shown in the space step, while one is open.
  OnboardingPairingKind? get pairingKind => _pairingKind;
  OnboardingJoinMethod get joinMethod =>
      _pairing?.joinMethod ?? OnboardingJoinMethod.code;

  /// Whether a device was paired during this tour.
  bool get paired => _paired;
  String? get pairedDeviceName => _pairedDeviceName;

  /// Set after a ceremony failed or could not start, until the next choice.
  bool get pairingInterrupted => _pairingInterrupted;
  String? get pairingIssue => _pairingIssue;

  bool get canGoBack =>
      !busy &&
      (_step == OnboardingStep.setup ||
          _step == OnboardingStep.space ||
          (_step == OnboardingStep.ready && offersSpace));
  bool get canContinue => switch (_step) {
    OnboardingStep.setup => _setup.canContinue && !_busy,
    OnboardingStep.welcome ||
    OnboardingStep.space ||
    OnboardingStep.ready => !busy,
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
      case OnboardingStep.space when _pairingKind != null:
        unawaited(leavePairing());
      case OnboardingStep.space:
        unawaited(_enterSetup());
      case OnboardingStep.ready when offersSpace:
        _step = OnboardingStep.space;
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
    _step = offersSpace ? OnboardingStep.space : OnboardingStep.ready;
    _setup.leave();
    _syncMonitoring();
    notifyIfActive();
    return true;
  }

  /// Records how this device starts. Joining opens the pairing ceremony in
  /// the tour when [pairsInline]; otherwise it is handed to Devices.
  void chooseSpace(OnboardingSpaceChoice choice) {
    if (busy ||
        _complete ||
        _disposed ||
        _step != OnboardingStep.space ||
        _pairingKind != null) {
      return;
    }
    if (choice == OnboardingSpaceChoice.join && _pairing != null) {
      unawaited(startPairing(OnboardingPairingKind.join));
      return;
    }
    _clearPairingIssue();
    _spaceChoice = choice;
    _step = OnboardingStep.ready;
    notifyIfActive();
  }

  /// Connects the Devices pairing ceremony once the sync runtime is ready,
  /// or disconnects it when the runtime stops.
  void attachPairing(OnboardingPairing? pairing) {
    if (identical(pairing, _pairing) || _disposed) return;
    _pairing?.removeListener(_pairingChanged);
    _pairing = pairing;
    pairing?.addListener(_pairingChanged);
    if (_pairingKind != null) {
      // The previous ceremony ended with its owner.
      _pairingOpening = false;
      _returnFromPairing(interrupted: true);
    }
    notifyIfActive();
  }

  /// Opens the ceremony in the space step: joining from the space step, or
  /// adding a device from the ready step.
  Future<bool> startPairing(OnboardingPairingKind kind) async {
    final pairing = _pairing;
    if (pairing == null ||
        busy ||
        _complete ||
        _disposed ||
        _pairingKind != null ||
        (_step != OnboardingStep.space && _step != OnboardingStep.ready)) {
      return false;
    }
    _clearPairingIssue();
    _pairingOrigin = _step;
    _pairingKind = kind;
    _pairingOpening = true;
    _step = OnboardingStep.space;
    notifyIfActive();
    final opened = await pairing.open(kind);
    if (_disposed || _pairingKind != kind || !identical(_pairing, pairing)) {
      return false;
    }
    _pairingOpening = false;
    if (!opened) {
      _returnFromPairing(interrupted: true, issue: pairing.errorMessage);
      notifyIfActive();
      return false;
    }
    _pairingChanged();
    return _pairingKind == kind;
  }

  /// Switches between code entry and QR scanning while joining.
  Future<void> useJoinMethod(OnboardingJoinMethod method) async {
    final pairing = _pairing;
    if (pairing == null ||
        _pairingKind != OnboardingPairingKind.join ||
        _pairingOpening ||
        method == joinMethod) {
      return;
    }
    await pairing.useJoinMethod(method);
    if (!_disposed && identical(_pairing, pairing)) _pairingChanged();
  }

  /// Leaves the ceremony for the step it was opened from. A failed ceremony
  /// leaves a notice there so the next step stays clear.
  Future<void> leavePairing() async {
    final pairing = _pairing;
    if (_pairingKind == null || _pairingOpening || pairing == null) return;
    final failed = pairing.phase == OnboardingPairingPhase.failed;
    _returnFromPairing(interrupted: failed);
    notifyIfActive();
    await pairing.close();
  }

  /// Ends the ceremony and moves on to the ready step without pairing.
  Future<void> skipPairing() async {
    final pairing = _pairing;
    if (_pairingKind == null || _pairingOpening || pairing == null) return;
    final origin = _pairingOrigin;
    _returnFromPairing(interrupted: false);
    if (origin == OnboardingStep.space) {
      _spaceChoice = OnboardingSpaceChoice.later;
      _step = OnboardingStep.ready;
    }
    notifyIfActive();
    await pairing.close();
  }

  void _pairingChanged() {
    final pairing = _pairing;
    final kind = _pairingKind;
    if (pairing == null || kind == null || _pairingOpening || _disposed) {
      return;
    }
    switch (pairing.phase) {
      case OnboardingPairingPhase.paired:
        _paired = true;
        _pairedDeviceName = pairing.peerName;
        _pairingKind = null;
        if (kind == OnboardingPairingKind.join) {
          _spaceChoice = OnboardingSpaceChoice.join;
        }
        _step = OnboardingStep.ready;
        notifyIfActive();
        // Releases the protected surface the ceremony no longer needs.
        unawaited(pairing.close());
      case OnboardingPairingPhase.idle:
        // Closed outside the tour, such as by a scanner that failed to open.
        final issue = pairing.errorMessage;
        _returnFromPairing(interrupted: issue != null, issue: issue);
        notifyIfActive();
      case OnboardingPairingPhase.inProgress || OnboardingPairingPhase.failed:
        notifyIfActive();
    }
  }

  void _returnFromPairing({required bool interrupted, String? issue}) {
    _pairingKind = null;
    _step = _pairingOrigin;
    _pairingInterrupted = interrupted;
    _pairingIssue = interrupted ? issue : null;
  }

  void _clearPairingIssue() {
    _pairingInterrupted = false;
    _pairingIssue = null;
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

  /// Shows the whole tour again from the welcome step. Completion stays
  /// stored, so leaving the tour midway does not bring it back at launch.
  bool replay() {
    if (busy || _disposed || !_initialized) return false;
    if (_pairingKind != null) unawaited(_pairing?.close());
    _pairingKind = null;
    _paired = false;
    _pairedDeviceName = null;
    _clearPairingIssue();
    _spaceChoice = OnboardingSpaceChoice.later;
    _complete = false;
    _errorMessage = null;
    _step = OnboardingStep.welcome;
    _syncMonitoring();
    notifyIfActive();
    return true;
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
    _pairing?.removeListener(_pairingChanged);
    if (_pairingKind != null) unawaited(_pairing?.close());
    _setup.removeListener(notifyIfActive);
    if (_monitoring) _setup.setMonitoring(false);
    _setup.dispose();
    super.dispose();
  }
}
