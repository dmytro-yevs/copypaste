import 'package:flutter/foundation.dart';

import '../../../platform/permissions/linux_integration.dart';
import '../repository/linux_onboarding_store.dart';

enum LinuxOnboardingStep { welcome, integration, sync }

class LinuxOnboardingController extends ChangeNotifier {
  LinuxOnboardingController({
    required LinuxOnboardingStore store,
    required LinuxIntegrationPort integration,
  }) : _store = store,
       _integration = integration;

  final LinuxOnboardingStore _store;
  final LinuxIntegrationPort _integration;

  LinuxOnboardingStep _step = LinuxOnboardingStep.welcome;
  LinuxIntegrationStatus? _status;
  bool _initialized = false;
  bool _complete = false;
  bool _busy = false;
  bool _disposed = false;
  String? _errorMessage;

  LinuxOnboardingStep get step => _step;
  LinuxIntegrationStatus? get status => _status;
  bool get complete => _complete;
  bool get busy => _busy;
  String? get errorMessage => _errorMessage;

  bool get integrationReady {
    final status = _status;
    if (status == null) return false;
    return switch (status.session) {
      LinuxDesktopSession.x11 => status.quickPaste,
      LinuxDesktopSession.wayland => status.clipboard && status.quickPaste,
      LinuxDesktopSession.unsupported => false,
    };
  }

  Future<void> initialize() async {
    if (_initialized) return;
    _busy = true;
    _notify();
    try {
      _complete = await _store.isComplete();
      await _refresh();
      _errorMessage = null;
    } catch (_) {
      _errorMessage = 'CopyPaste could not read Linux integration status.';
    } finally {
      _initialized = true;
      _busy = false;
      _notify();
    }
  }

  void showIntegration() {
    if (_busy || _step != LinuxOnboardingStep.welcome) return;
    _step = LinuxOnboardingStep.integration;
    _notify();
  }

  void showPreviousStep() {
    if (_busy) return;
    _step = switch (_step) {
      LinuxOnboardingStep.welcome => LinuxOnboardingStep.welcome,
      LinuxOnboardingStep.integration => LinuxOnboardingStep.welcome,
      LinuxOnboardingStep.sync => LinuxOnboardingStep.integration,
    };
    _notify();
  }

  Future<void> refreshIntegration() => _run(_refresh);

  Future<void> openCompanionSetup() => _run(() async {
    if (!await _integration.openCompanionSetup()) {
      _errorMessage = 'Desktop integration setup could not be opened.';
    }
    await _refresh();
  });

  Future<void> requestRemoteDesktop() => _run(() async {
    if (!await _integration.requestRemoteDesktop()) {
      _errorMessage = 'Keyboard control permission was not granted.';
    }
    await _refresh();
  });

  Future<void> setStartAtLogin(bool enabled) => _run(() async {
    if (!await _integration.setStartAtLogin(enabled)) {
      _errorMessage = 'Start at login could not be updated.';
    }
    await _refresh();
  });

  Future<void> registerCopypasteUri() => _run(() async {
    if (!await _integration.registerCopypasteUri()) {
      _errorMessage = 'CopyPaste links could not be registered.';
    }
    await _refresh();
  });

  Future<void> continueFromIntegration() async {
    if (_busy) return;
    await _run(_refresh);
    if (_busy || integrationReady) {
      if (integrationReady) {
        _step = LinuxOnboardingStep.sync;
        _notify();
      }
      return;
    }
    _errorMessage = _integrationRequirement();
    _notify();
  }

  Future<bool> finish() async {
    if (_busy || _step != LinuxOnboardingStep.sync) return false;
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

  Future<void> _run(Future<void> Function() action) async {
    if (_busy) return;
    _busy = true;
    _errorMessage = null;
    _notify();
    try {
      await action();
    } catch (_) {
      _errorMessage = 'Linux integration status could not be refreshed.';
    } finally {
      _busy = false;
      _notify();
    }
  }

  Future<void> _refresh() async {
    _status = await _integration.status();
  }

  String _integrationRequirement() => switch (_status?.session) {
    LinuxDesktopSession.x11 =>
      'Quick Paste input is unavailable in this X11 session.',
    LinuxDesktopSession.wayland =>
      'Enable the signed GNOME or KDE companion and allow keyboard control.',
    LinuxDesktopSession.unsupported ||
    null => 'CopyPaste requires an X11 or Wayland desktop session.',
  };

  void _notify() {
    if (!_disposed) notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
