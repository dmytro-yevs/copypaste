import 'onboarding_controller.dart';

import '../../../platform/permissions/linux_integration.dart';
import '../repository/onboarding_store.dart';

enum LinuxOnboardingStep { welcome, integration, sync }

class LinuxOnboardingController extends OnboardingController {
  LinuxOnboardingController({
    required OnboardingStore store,
    required LinuxIntegrationPort integration,
  }) : _store = store,
       _integration = integration;

  final OnboardingStore _store;
  final LinuxIntegrationPort _integration;

  LinuxOnboardingStep _step = LinuxOnboardingStep.welcome;
  LinuxIntegrationStatus? _status;
  bool _initialized = false;

  LinuxOnboardingStep get step => _step;
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

  Future<void> initialize() async {
    if (_initialized) return;
    busy = true;
    notifyIfActive();
    try {
      complete = await _store.isComplete();
      await _refresh();
      errorMessage = null;
    } catch (_) {
      errorMessage = 'CopyPaste could not read Linux integration status.';
    } finally {
      _initialized = true;
      busy = false;
      notifyIfActive();
    }
  }

  Future<void> showIntegration() async {
    if (busy || _step != LinuxOnboardingStep.welcome) return;
    _step = LinuxOnboardingStep.integration;
    notifyIfActive();
    await _run(_refresh);
  }

  void showPreviousStep() {
    if (busy) return;
    _step = switch (_step) {
      LinuxOnboardingStep.welcome => LinuxOnboardingStep.welcome,
      LinuxOnboardingStep.integration => LinuxOnboardingStep.welcome,
      LinuxOnboardingStep.sync => LinuxOnboardingStep.integration,
    };
    notifyIfActive();
  }

  Future<void> refreshIntegration() => _run(_refresh);

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
    if (!await _integration.setStartAtLogin(enabled)) {
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

  Future<void> continueFromIntegration() async {
    if (busy) return;
    await _run(_refresh);
    if (busy || integrationReady) {
      if (integrationReady) {
        _step = LinuxOnboardingStep.sync;
        notifyIfActive();
      }
      return;
    }
    errorMessage = _integrationRequirement();
    notifyIfActive();
  }

  Future<bool> finish() => finishOnboarding(
    canFinish: _step == LinuxOnboardingStep.sync,
    persist: _store.markComplete,
  );

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
