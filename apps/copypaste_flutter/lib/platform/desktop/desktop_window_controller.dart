import 'dart:async';

import 'package:flutter/foundation.dart';

import 'desktop_window_geometry.dart';

export 'desktop_window_geometry.dart';

enum DesktopWindowSetupIssue {
  windowUnavailable,
  trayUnavailable,
  quitUnavailable,
}

extension DesktopWindowSetupIssueMessage on DesktopWindowSetupIssue {
  String get message => switch (this) {
    DesktopWindowSetupIssue.windowUnavailable =>
      'Desktop window controls are unavailable. CopyPaste will stay visible.',
    DesktopWindowSetupIssue.trayUnavailable =>
      'The system tray is unavailable. Keep CopyPaste open, then retry or quit.',
    DesktopWindowSetupIssue.quitUnavailable =>
      'CopyPaste could not quit. Please try again.',
  };
}

abstract interface class DesktopWindowHost {
  Future<void> initializeWindow(DesktopWindowBounds initialBounds);

  Future<void> setCloseHandler(Future<void> Function() onCloseRequested);

  Future<DesktopWorkArea> workAreaFor(DesktopWindowBounds bounds);

  Future<void> setBounds(DesktopWindowBounds bounds);

  Future<void> setMinimumSize(DesktopWindowSize size);

  Future<DesktopWindowBounds> getBounds();

  Future<void> initializeTray({
    required Future<void> Function() onOpenRequested,
    required Future<void> Function() onCaptureRequested,
    required Future<void> Function() onSettingsRequested,
    required Future<void> Function() onQuitRequested,
  });

  Future<void> updateCaptureMenu({
    required bool available,
    required bool paused,
  });

  Future<void> hide();

  Future<void> showAndFocus();

  Future<void> quit();

  void setBoundsChangedHandler(Future<void> Function() onBoundsChanged);

  void setVisibilityChangedHandler(ValueChanged<bool>? onVisibilityChanged);

  Future<void> dispose();
}

class DesktopWindowController {
  DesktopWindowController({
    required DesktopWindowHost host,
    required DesktopWindowGeometryStore geometryStore,
  }) : _host = host,
       _geometryStore = geometryStore;

  final DesktopWindowHost _host;
  final DesktopWindowGeometryStore _geometryStore;
  final ValueNotifier<DesktopWindowSetupIssue?> setupIssue =
      ValueNotifier<DesktopWindowSetupIssue?>(null);
  final ValueNotifier<bool> _isUnifiedTitleBarReady = ValueNotifier<bool>(
    false,
  );

  ValueListenable<bool> get isUnifiedTitleBarReady => _isUnifiedTitleBarReady;

  final ValueNotifier<bool> _isVisible = ValueNotifier(true);
  ValueListenable<bool> get isVisible => _isVisible;

  bool _trayReady = false;
  bool _isQuitting = false;
  bool _windowReady = false;
  bool _disposed = false;
  bool _persistenceReady = false;
  Future<void> Function()? _beforeQuit;
  VoidCallback? _openSettings;
  Future<void> Function()? _toggleCapture;
  bool _captureAvailable = false;
  bool _capturePaused = true;
  Future<void>? _initialization;
  Future<void>? _disposal;
  Future<void> _boundsWrite = Future<void>.value();

  Future<void> initialize() {
    if (_disposed) {
      return Future<void>.error(
        StateError('Desktop window controller is disposed.'),
      );
    }
    final initialization = _initialization;
    if (initialization != null) {
      return initialization;
    }
    if (_trayReady) {
      return Future<void>.value();
    }
    late final Future<void> nextInitialization;
    nextInitialization = _initialize().whenComplete(() {
      if (identical(_initialization, nextInitialization)) {
        _initialization = null;
      }
    });
    _initialization = nextInitialization;
    return nextInitialization;
  }

  Future<void> retry() {
    return switch (setupIssue.value) {
      DesktopWindowSetupIssue.quitUnavailable => quit(),
      DesktopWindowSetupIssue.windowUnavailable => _retryWindowAccess(),
      _ => initialize(),
    };
  }

  Future<void> _retryWindowAccess() async {
    if (!_windowReady) {
      await initialize();
      return;
    }
    if (!_trayReady) {
      await initialize();
      if (!_trayReady) {
        return;
      }
    }
    await showFromTrayOrDock();
  }

  Future<void> _initialize() async {
    if (_disposed) {
      return;
    }
    _host.setBoundsChangedHandler(_saveCurrentBounds);
    _host.setVisibilityChangedHandler(_visibilityChanged);

    if (!_windowReady) {
      try {
        await _host.initializeWindow(desktopWindowInitialBounds);
        if (_disposed) {
          return;
        }
        _isUnifiedTitleBarReady.value = true;
        await _host.setCloseHandler(handleCloseRequested);
        if (_disposed) {
          return;
        }
        _windowReady = true;
      } catch (_) {
        _setIssue(DesktopWindowSetupIssue.windowUnavailable);
        return;
      }

      await _restoreBounds();
      if (_disposed) {
        return;
      }
      _persistenceReady = true;
    }

    try {
      await _host.initializeTray(
        onOpenRequested: showFromTrayOrDock,
        onCaptureRequested: _toggleCaptureFromTray,
        onSettingsRequested: _openSettingsFromTray,
        onQuitRequested: quit,
      );
      if (_disposed) {
        return;
      }
      _trayReady = true;
      await _host.updateCaptureMenu(
        available: _captureAvailable,
        paused: _capturePaused,
      );
      _clearIssue();
    } catch (_) {
      _setIssue(DesktopWindowSetupIssue.trayUnavailable);
    }
  }

  Future<void> handleCloseRequested() async {
    if (_isQuitting) {
      return;
    }

    await _saveCurrentBounds();
    if (_trayReady) {
      try {
        await _host.hide();
        _visibilityChanged(false);
      } catch (_) {
        _setIssue(DesktopWindowSetupIssue.windowUnavailable);
      }
      return;
    }

    _setIssue(DesktopWindowSetupIssue.trayUnavailable);
    try {
      await _host.showAndFocus();
    } catch (_) {
      _setIssue(DesktopWindowSetupIssue.windowUnavailable);
    }
  }

  Future<void> showFromTrayOrDock() async {
    try {
      _visibilityChanged(true);
      await _host.showAndFocus();
      _clearIssue();
    } catch (_) {
      _setIssue(DesktopWindowSetupIssue.windowUnavailable);
    }
  }

  void _visibilityChanged(bool visible) {
    if (!_disposed) _isVisible.value = visible;
  }

  Future<void> _openSettingsFromTray() async {
    await showFromTrayOrDock();
    _openSettings?.call();
  }

  Future<void> _toggleCaptureFromTray() async {
    await _toggleCapture?.call();
  }

  Future<void> quit() async {
    if (_isQuitting) {
      return;
    }

    _isQuitting = true;
    try {
      await _saveCurrentBounds();
      await _beforeQuit?.call();
      await _host.quit();
    } catch (_) {
      _setIssue(DesktopWindowSetupIssue.quitUnavailable);
    } finally {
      if (!_disposed) {
        _isQuitting = false;
      }
    }
  }

  /// Registers application-owned work that must finish before destroying the window.
  void setBeforeQuit(Future<void> Function()? callback) {
    _beforeQuit = callback;
  }

  void setOpenSettings(VoidCallback? callback) {
    _openSettings = callback;
  }

  void setCaptureToggle(Future<void> Function()? callback) {
    _toggleCapture = callback;
  }

  Future<void> updateCaptureState({
    required bool available,
    required bool paused,
  }) async {
    _captureAvailable = available;
    _capturePaused = paused;
    if (_trayReady && !_disposed) {
      await _host.updateCaptureMenu(available: available, paused: paused);
    }
  }

  Future<void> _restoreBounds() async {
    try {
      final savedBounds = await _geometryStore.read();
      if (_disposed) {
        return;
      }
      final placement = savedBounds ?? desktopWindowInitialBounds;
      final workArea = await _host.workAreaFor(placement);
      if (_disposed) {
        return;
      }
      await _host.setMinimumSize(minimumSizeFor(workArea));
      await _host.setBounds(
        restoreDesktopWindowBounds(
          workArea: workArea,
          savedBounds: savedBounds,
        ),
      );
    } catch (_) {
      // The initial native window remains visible when persistence is absent.
    }
  }

  Future<void> _saveCurrentBounds() async {
    if (!_persistenceReady || _disposed) {
      return;
    }
    _boundsWrite = _boundsWrite.catchError((_) {}).then((_) async {
      if (!_persistenceReady || _disposed) {
        return;
      }
      try {
        final bounds = await _host.getBounds();
        if (_persistenceReady && !_disposed) {
          await _geometryStore.write(bounds);
        }
      } catch (_) {
        // Geometry persistence is optional and must not disrupt window controls.
      }
    });
    await _boundsWrite;
  }

  void _setIssue(DesktopWindowSetupIssue issue) {
    if (_disposed) {
      return;
    }
    setupIssue.value = issue;
  }

  void _clearIssue() {
    if (_disposed) {
      return;
    }
    setupIssue.value = null;
  }

  Future<void> dispose() async {
    final disposal = _disposal;
    if (disposal != null) {
      return disposal;
    }
    if (_disposed) {
      return;
    }
    _disposed = true;
    _trayReady = false;
    _isUnifiedTitleBarReady.value = false;
    _persistenceReady = false;
    late final Future<void> nextDisposal;
    nextDisposal = _dispose().whenComplete(() {
      if (identical(_disposal, nextDisposal)) {
        _disposal = null;
      }
    });
    _disposal = nextDisposal;
    return nextDisposal;
  }

  Future<void> _dispose() async {
    try {
      await _initialization?.catchError((_) {});
      await _boundsWrite.catchError((_) {});
      await _host.dispose();
    } finally {
      setupIssue.dispose();
      _isUnifiedTitleBarReady.dispose();
      _isVisible.dispose();
    }
  }
}
