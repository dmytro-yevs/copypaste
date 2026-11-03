import '../../../platform/permissions/linux_integration.dart';
import '../../../platform/startup/launch_at_login.dart';
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

  bool get integrationReady {
    final status = _status;
    if (status == null) return false;
    final captureReady =
        status.capabilities.isEmpty ||
        status.capability(LinuxCapabilityKind.clipboardCapture).state ==
            LinuxCapabilityState.available;
    return switch (status.session) {
      LinuxDesktopSession.x11 =>
        status.clipboard && captureReady && status.quickPaste,
      LinuxDesktopSession.wayland =>
        status.clipboard && captureReady && status.quickPaste,
      LinuxDesktopSession.unsupported => false,
    };
  }

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
        errorMessage = 'CopyPaste could not read Linux integration status.';
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
      errorMessage = 'Desktop integration setup could not be opened.';
    }
    await _refresh();
  });

  Future<void> requestRemoteDesktop() => _run(() async {
    if (!await _integration.requestRemoteDesktop()) {
      errorMessage = 'Keyboard control permission was not granted.';
    }
    await _refresh();
  });

  Future<void> setStartAtLogin(bool enabled) => _run(() async {
    try {
      await _startup.setEnabled(enabled);
    } catch (_) {
      errorMessage = 'Start at login could not be updated.';
    }
    await _refresh();
  });

  Future<void> registerCopypasteUri() => _run(() async {
    if (!await _integration.registerCopypasteUri()) {
      errorMessage = 'CopyPaste links could not be registered.';
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
    if (integrationReady) return true;
    errorMessage = _integrationRequirement();
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
      errorMessage = 'Linux integration status could not be refreshed.';
    } finally {
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> _refresh() async {
    _status = await _integration.status();
  }

  String _integrationRequirement() => switch (_status?.session) {
    LinuxDesktopSession.x11 when _status?.clipboard != true =>
      'Clipboard access is unavailable in this X11 session.',
    LinuxDesktopSession.x11 =>
      'Quick Paste input is unavailable in this X11 session.',
    LinuxDesktopSession.wayland =>
      'Enable the signed GNOME or KDE companion and allow keyboard control.',
    LinuxDesktopSession.unsupported ||
    null => 'CopyPaste requires an X11 or Wayland desktop session.',
  };
}
