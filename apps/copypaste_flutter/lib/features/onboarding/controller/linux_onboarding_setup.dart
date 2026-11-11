import '../../../platform/permissions/linux_integration.dart';
import '../../../platform/startup/launch_at_login.dart';
import '../../../l10n/l10n.dart';
import '../repository/onboarding_store.dart';
import 'onboarding_setup.dart';

/// Verified desktop integration setup for Linux.
class LinuxOnboardingSetup extends OnboardingSetup {
  LinuxOnboardingSetup({
    required OnboardingStore store,
    required LinuxIntegrationPort integration,
    required LaunchAtLoginGateway startup,
  }) : _store = store,
       _integration = integration,
       _startup = startup;

  final OnboardingStore _store;
  final LinuxIntegrationPort _integration;
  final LaunchAtLoginGateway _startup;

  LinuxIntegrationStatus? _status;

  @override
  String get platformLabel => 'Linux';
  LinuxIntegrationStatus? get status => _status;

  /// Whether clipboard capture works, which is all onboarding requires.
  bool get captureReady {
    final status = _status;
    if (status == null ||
        status.session == LinuxDesktopSession.unsupported ||
        !status.clipboard) {
      return false;
    }
    return status.capabilities.isEmpty ||
        status.capability(LinuxCapabilityKind.clipboardCapture).state ==
            LinuxCapabilityState.available;
  }

  /// Whether optional Quick Paste input is also ready.
  bool get quickPasteReady => captureReady && _status!.quickPaste;

  @override
  String? get continueLabel => captureReady && !quickPasteReady
      ? AppStrings.current.onboardingLinuxContinueBasic
      : null;

  @override
  Future<OnboardingResume> restore() async {
    busy = true;
    notifyIfActive();
    try {
      final complete = await _store.isComplete();
      try {
        await _refresh();
        errorMessage = null;
      } catch (_) {
        errorMessage = AppStrings.current.onboardingLinuxStatusReadFailed;
      }
      return complete ? OnboardingResume.complete : OnboardingResume.welcome;
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  @override
  Future<void> enter() => _run(_refresh);

  @override
  Future<void> refresh() => _run(_refresh);

  Future<void> openCompanionSetup() => _run(() async {
    if (!await _integration.openCompanionSetup()) {
      errorMessage = AppStrings.current.onboardingLinuxSetupOpenFailed;
    }
    await _refresh();
  });

  Future<void> requestRemoteDesktop() => _run(() async {
    if (!await _integration.requestRemoteDesktop()) {
      errorMessage = AppStrings.current.onboardingLinuxKeyboardDenied;
    }
    await _refresh();
  });

  Future<void> setStartAtLogin(bool enabled) => _run(() async {
    try {
      await _startup.setEnabled(enabled);
    } catch (_) {
      errorMessage = AppStrings.current.onboardingLinuxStartAtLoginFailed;
    }
    await _refresh();
  });

  Future<void> registerCopypasteUri() => _run(() async {
    if (!await _integration.registerCopypasteUri()) {
      errorMessage = AppStrings.current.onboardingLinuxLinksFailed;
    }
    await _refresh();
  });

  @override
  Future<bool> apply() async {
    if (busy) return false;
    var refreshed = false;
    await _run(() async {
      await _refresh();
      refreshed = true;
    });
    // A failed probe must not turn a previously ready snapshot into current
    // permission to advance. Preserve the refresh error and allow a retry.
    if (!refreshed || disposed || busy) return false;
    if (captureReady) return true;
    errorMessage = _captureRequirement();
    notifyIfActive();
    return false;
  }

  @override
  Future<void> persistCompletion() => _store.markComplete();

  Future<void> _run(Future<void> Function() action) async {
    if (busy) return;
    busy = true;
    errorMessage = null;
    notifyIfActive();
    try {
      await action();
    } catch (_) {
      errorMessage = AppStrings.current.onboardingLinuxStatusRefreshFailed;
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> _refresh() async {
    _status = await _integration.status();
  }

  String _captureRequirement() {
    final status = _status;
    final strings = AppStrings.current;
    return switch (status?.session) {
      LinuxDesktopSession.unsupported ||
      null => strings.onboardingLinuxSessionRequired,
      _
          when status!.clipboard &&
              status.capability(LinuxCapabilityKind.clipboardCapture).state ==
                  LinuxCapabilityState.needsCompanion =>
        strings.onboardingLinuxCompanionCapturePaused,
      LinuxDesktopSession.x11 => strings.onboardingLinuxX11ClipboardUnavailable,
      LinuxDesktopSession.wayland =>
        strings.onboardingLinuxWaylandClipboardUnavailable,
    };
  }
}
