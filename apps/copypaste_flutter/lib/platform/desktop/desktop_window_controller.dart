import 'dart:async';
import 'dart:io';

import 'package:copypaste_flutter/l10n/l10n.dart';
import 'package:flutter/foundation.dart';

import '../../features/tray/models/tray_menu_model.dart';
import '../capture/capture_control_state.dart';

import 'desktop_window_geometry.dart';
import 'desktop_visibility_controller.dart';
import 'desktop_visibility_preferences.dart';

export 'desktop_window_geometry.dart';
export '../capture/capture_control_state.dart';
export '../../features/tray/models/tray_menu_model.dart';

enum DesktopWindowSetupIssue {
  windowUnavailable,
  trayUnavailable,
  quitUnavailable,
}

extension DesktopWindowSetupIssueMessage on DesktopWindowSetupIssue {
  String get message => switch (this) {
    DesktopWindowSetupIssue.windowUnavailable =>
      AppStrings.current.platformWindowControlsUnavailable,
    DesktopWindowSetupIssue.trayUnavailable =>
      AppStrings.current.platformTrayUnavailable,
    DesktopWindowSetupIssue.quitUnavailable =>
      AppStrings.current.platformQuitFailed,
  };
}

abstract interface class DesktopWindowHost implements DesktopVisibilityHost {
  Future<void> initializeWindow(
    DesktopWindowBounds initialBounds, {
    bool showInTaskbar = true,
  });

  Future<void> setCloseHandler(Future<void> Function() onCloseRequested);

  Future<DesktopWorkArea> workAreaFor(DesktopWindowBounds bounds);

  Future<void> setBounds(DesktopWindowBounds bounds);

  Future<void> setMinimumSize(DesktopWindowSize size);

  Future<DesktopWindowBounds> getBounds();

  Future<void> initializeTray({
    bool visible = true,
    required List<TrayMenuEntry> menu,
    required TrayMenuEvents events,
  });

  /// Shows [menu]; rows keep their native items when their keys persist.
  Future<void> updateTrayMenu(List<TrayMenuEntry> menu);

  /// Opens Quick Paste anchored to the status icon.
  Future<void> openQuickPaste();

  Future<void> hide();

  Future<void> showAndFocus();

  Future<void> quit();

  Future<bool> isFullScreen();

  Future<void> setFullScreen(bool enabled);

  void setBoundsChangedHandler(Future<void> Function() onBoundsChanged);

  void setVisibilityChangedHandler(ValueChanged<bool>? onVisibilityChanged);

  Future<void> dispose();
}

/// What the native tray menu reports to its owner.
class TrayMenuEvents {
  const TrayMenuEvents({
    required this.onAction,
    required this.onOpened,
    required this.onClosed,
  });

  final Future<void> Function(TrayAction action) onAction;
  final VoidCallback onOpened;
  final VoidCallback onClosed;
}

/// macOS draws the app header in the native title bar once its window is ready.
bool usesUnifiedMacosTitleBar(
  DesktopWindowController? window, {
  required bool windowReady,
}) => Platform.isMacOS && window != null && windowReady;

class DesktopWindowController implements TrayMenuSurface {
  bool _changingFullScreen = false;

  Future<void> toggleFullScreen() => _changeFullScreen();

  Future<void> exitFullScreen() => _changeFullScreen(enabled: false);

  Future<void> _changeFullScreen({bool? enabled}) async {
    if (_disposed || _isQuitting || _changingFullScreen) return;
    _changingFullScreen = true;
    try {
      final current = await _host.isFullScreen();
      final next = enabled ?? !current;
      if (!_disposed && !_isQuitting && next != current) {
        await _host.setFullScreen(next);
      }
    } catch (_) {
      _setIssue(DesktopWindowSetupIssue.windowUnavailable);
    } finally {
      _changingFullScreen = false;
    }
  }

  DesktopWindowController({
    required DesktopWindowHost host,
    required DesktopWindowGeometryStore geometryStore,
    DesktopVisibilityPreferencesStore? visibilityStore,
    DateTime Function()? clock,
  }) : _host = host,
       _geometryStore = geometryStore,
       _clock = clock ?? DateTime.now,
       visibility = DesktopVisibilityController(
         host: host,
         store: visibilityStore ?? MemoryDesktopVisibilityStore(),
       );

  final DesktopVisibilityController visibility;
  bool _visibilityLoaded = false;

  final DesktopWindowHost _host;
  final DesktopWindowGeometryStore _geometryStore;
  final DateTime Function() _clock;
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
  TrayMenuState _trayState = const TrayMenuState();
  TrayMenuDelegate? _trayDelegate;
  Timer? _resumeTimer;
  Future<void> _trayUpdate = Future<void>.value();
  Future<void>? _initialization;
  Future<void>? _disposal;
  Future<void>? _taskbarRestoration;
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
        if (!_visibilityLoaded) {
          await visibility.load();
          _visibilityLoaded = true;
        }
        await _host.initializeWindow(
          desktopWindowInitialBounds,
          showInTaskbar: visibility.preferences.showInTaskbar,
        );
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
        visible: visibility.preferences.showInTray,
        menu: buildTrayMenu(_trayState, AppStrings.current),
        events: TrayMenuEvents(
          onAction: _trayAction,
          onOpened: _trayMenuOpened,
          onClosed: _trayMenuClosed,
        ),
      );
      if (_disposed) {
        return;
      }
      _trayReady = true;
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

  @override
  Future<void> showFromTrayOrDock() async {
    try {
      _visibilityChanged(true);
      await _host.showAndFocus();
      await _restoreTaskbarVisibility();
      _clearIssue();
    } catch (_) {
      _setIssue(DesktopWindowSetupIssue.windowUnavailable);
    }
  }

  void _visibilityChanged(bool visible) {
    if (!_disposed) _isVisible.value = visible;
    if (visible && _windowReady && !_disposed) {
      unawaited(
        _restoreTaskbarVisibility().catchError((Object _) {
          _setIssue(DesktopWindowSetupIssue.windowUnavailable);
        }),
      );
    }
  }

  Future<void> _restoreTaskbarVisibility() {
    final pending = _taskbarRestoration;
    if (pending != null) return pending;
    late final Future<void> operation;
    operation =
        (() async {
          await visibility.settle();
          if (_disposed) return;
          await _host.setTaskbarVisible(visibility.preferences.showInTaskbar);
        })().whenComplete(() {
          if (identical(_taskbarRestoration, operation)) {
            _taskbarRestoration = null;
          }
        });
    _taskbarRestoration = operation;
    return operation;
  }

  Future<void> _openSettingsFromTray() async {
    await showFromTrayOrDock();
    _openSettings?.call();
  }

  Future<void> _toggleCaptureFromTray() async {
    await _clearPause();
    await _toggleCapture?.call();
  }

  Future<void> _trayAction(TrayAction action) async {
    if (_disposed) return;
    final work = switch (action) {
      TrayCommandAction(command: TrayCommand.openApp) => showFromTrayOrDock(),
      TrayCommandAction(command: TrayCommand.openQuickPaste) =>
        _host.openQuickPaste(),
      TrayCommandAction(command: TrayCommand.toggleCapture) =>
        _toggleCaptureFromTray(),
      TrayCommandAction(command: TrayCommand.openSettings) =>
        _openSettingsFromTray(),
      TrayCommandAction(command: TrayCommand.quit) => quit(),
      TrayPauseAction(:final duration) => pauseCapture(duration),
      _ => _trayDelegate?.trayActionSelected(action) ?? Future<void>.value(),
    };
    try {
      await work;
    } catch (_) {
      // Each action reports its own failure; a menu has nowhere to show one.
    }
  }

  void _trayMenuOpened() {
    if (_disposed) return;
    // A monotonic timer can fall behind the wall clock across system sleep.
    unawaited(_resumeIfDue());
    // Labels follow the interface language, which can change while closed.
    unawaited(updateTrayMenu((state) => state));
    _trayDelegate?.trayMenuOpened();
  }

  void _trayMenuClosed() {
    if (!_disposed) _trayDelegate?.trayMenuClosed();
  }

  /// Pauses capture until resumed, or for [duration] and then resumes it.
  Future<void> pauseCapture(Duration? duration) async {
    final toggle = _toggleCapture;
    if (_disposed ||
        toggle == null ||
        _trayState.capture != CaptureControlState.running) {
      return;
    }
    await _clearPause();
    await toggle();
    if (duration == null || _disposed) return;
    final until = _clock().add(duration);
    await updateTrayMenu((state) => state.copyWith(pausedUntil: until));
    _armResume(until);
  }

  void _armResume(DateTime until) {
    _resumeTimer?.cancel();
    final wait = until.difference(_clock());
    _resumeTimer = Timer(wait.isNegative ? Duration.zero : wait, () {
      unawaited(_resumeIfDue());
    });
  }

  Future<void> _resumeIfDue() async {
    final until = _trayState.pausedUntil;
    if (until == null || _disposed) return;
    if (_clock().isBefore(until)) {
      _armResume(until);
      return;
    }
    await _clearPause();
    if (_trayState.capture == CaptureControlState.paused) {
      await _toggleCapture?.call();
    }
  }

  Future<void> _clearPause() async {
    _resumeTimer?.cancel();
    _resumeTimer = null;
    if (_trayState.pausedUntil != null) {
      await updateTrayMenu((state) => state.copyWith(pausedUntil: null));
    }
  }

  @override
  TrayMenuState get trayState => _trayState;

  @override
  void setTrayMenuDelegate(TrayMenuDelegate? delegate) {
    _trayDelegate = delegate;
  }

  /// Applies [update] and shows the result once the tray is ready.
  @override
  Future<void> updateTrayMenu(
    TrayMenuState Function(TrayMenuState state) update,
  ) {
    if (_disposed) return Future<void>.value();
    _trayState = update(_trayState);
    _trayUpdate = _trayUpdate.catchError((Object _) {}).then((_) async {
      if (!_trayReady || _disposed) return;
      await _host.updateTrayMenu(buildTrayMenu(_trayState, AppStrings.current));
    });
    return _trayUpdate;
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

  Future<void> updateCaptureState({required CaptureControlState state}) async {
    if (state != CaptureControlState.paused) {
      // Resumed or stopped elsewhere: a timed pause no longer applies.
      _resumeTimer?.cancel();
      _resumeTimer = null;
    }
    await updateTrayMenu(
      (current) => current.copyWith(
        capture: state,
        pausedUntil: state == CaptureControlState.paused
            ? current.pausedUntil
            : null,
      ),
    );
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
        // Fullscreen is a presentation state, not the user's normal geometry.
        if (await _host.isFullScreen()) return;
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
    _resumeTimer?.cancel();
    _resumeTimer = null;
    _trayDelegate = null;
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
      await _trayUpdate.catchError((_) {});
      await visibility.settle();
      await _taskbarRestoration?.catchError((_) {});
      await _host.dispose();
    } finally {
      visibility.dispose();
      setupIssue.dispose();
      _isUnifiedTitleBarReady.dispose();
      _isVisible.dispose();
    }
  }
}
