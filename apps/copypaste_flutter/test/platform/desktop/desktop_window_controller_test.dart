import 'dart:async';

import 'package:copypaste_flutter/platform/desktop/desktop_window_controller.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('DesktopWindowController', () {
    test('reports the window as unavailable before setup starts', () {
      final controller = DesktopWindowController(
        host: _FakeDesktopWindowHost(),
        geometryStore: _FakeGeometryStore(),
      );

      expect(controller.isUnifiedTitleBarReady.value, isFalse);
    });

    test('reports the window as ready after native setup succeeds', () async {
      final controller = DesktopWindowController(
        host: _FakeDesktopWindowHost(),
        geometryStore: _FakeGeometryStore(),
      );

      await controller.initialize();

      expect(controller.isUnifiedTitleBarReady.value, isTrue);
    });

    test(
      'keeps the unified title bar unavailable when native setup fails',
      () async {
        final controller = DesktopWindowController(
          host: _FakeDesktopWindowHost(failedWindowInitializations: 1),
          geometryStore: _FakeGeometryStore(),
        );

        await controller.initialize();

        expect(controller.isUnifiedTitleBarReady.value, isFalse);
      },
    );

    test(
      'keeps the unified title bar ready when close handler setup fails',
      () async {
        final controller = DesktopWindowController(
          host: _FakeDesktopWindowHost(failedCloseHandlerInitializations: 1),
          geometryStore: _FakeGeometryStore(),
        );

        await controller.initialize();

        expect(controller.isUnifiedTitleBarReady.value, isTrue);
        expect(
          controller.setupIssue.value,
          DesktopWindowSetupIssue.windowUnavailable,
        );
      },
    );

    test(
      'reports the window as ready after a successful setup retry',
      () async {
        final controller = DesktopWindowController(
          host: _FakeDesktopWindowHost(failedCloseHandlerInitializations: 1),
          geometryStore: _FakeGeometryStore(),
        );

        await controller.initialize();
        expect(controller.isUnifiedTitleBarReady.value, isTrue);
        expect(
          controller.setupIssue.value,
          DesktopWindowSetupIssue.windowUnavailable,
        );
        await controller.retry();

        expect(controller.isUnifiedTitleBarReady.value, isTrue);
        expect(controller.setupIssue.value, isNull);
      },
    );

    test('hides after an intercepted close when the tray is ready', () async {
      final host = _FakeDesktopWindowHost();
      final geometryStore = _FakeGeometryStore();
      final controller = DesktopWindowController(
        host: host,
        geometryStore: geometryStore,
      );

      await controller.initialize();
      await host.requestClose();

      expect(host.hideCalls, 1);
      expect(geometryStore.lastWrittenBounds, host.bounds);
    });

    test('runs application shutdown before destroying the window', () async {
      final host = _FakeDesktopWindowHost();
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );
      var shutdownCompleted = false;
      controller.setBeforeQuit(() async {
        expect(host.quitCalls, 0);
        shutdownCompleted = true;
      });

      await controller.quit();

      expect(shutdownCompleted, isTrue);
      expect(host.quitCalls, 1);
    });

    test('keeps the app visible when tray setup fails', () async {
      final host = _FakeDesktopWindowHost(failedTrayInitializations: 1);
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );

      await controller.retry();
      await host.requestClose();

      expect(
        controller.setupIssue.value,
        DesktopWindowSetupIssue.trayUnavailable,
      );
      expect(host.closeHandler, isNotNull);
      expect(host.hideCalls, 0);
      expect(host.showAndFocusCalls, 1);
    });

    test('retries tray setup without duplicating window setup', () async {
      final host = _FakeDesktopWindowHost(failedTrayInitializations: 1);
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );

      await controller.initialize();
      await controller.retry();
      await host.requestClose();

      expect(host.initializeWindowCalls, 1);
      expect(host.setCloseHandlerCalls, 1);
      expect(host.initializeTrayCalls, 2);
      expect(host.hideCalls, 1);
      expect(controller.setupIssue.value, isNull);
    });

    test('retries window access after the tray is already ready', () async {
      final host = _FakeDesktopWindowHost(failedShowAndFocusAttempts: 1);
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );

      await controller.initialize();
      await host.requestOpen();
      await controller.retry();

      expect(host.initializeWindowCalls, 1);
      expect(host.initializeTrayCalls, 1);
      expect(host.showAndFocusCalls, 2);
      expect(controller.setupIssue.value, isNull);
    });

    test('restores the tray before retrying window access', () async {
      final host = _FakeDesktopWindowHost(
        failedTrayInitializations: 1,
        failedShowAndFocusAttempts: 1,
      );
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );

      await controller.initialize();
      await host.requestClose();
      expect(
        controller.setupIssue.value,
        DesktopWindowSetupIssue.windowUnavailable,
      );

      await controller.retry();

      expect(host.initializeTrayCalls, 2);
      expect(host.showAndFocusCalls, 2);
      expect(controller.setupIssue.value, isNull);
    });

    test(
      'retries window initialization after the first setup failure',
      () async {
        final host = _FakeDesktopWindowHost(failedWindowInitializations: 1);
        final controller = DesktopWindowController(
          host: host,
          geometryStore: _FakeGeometryStore(),
        );

        await controller.initialize();
        expect(
          controller.setupIssue.value,
          DesktopWindowSetupIssue.windowUnavailable,
        );

        await controller.retry();

        expect(host.initializeWindowCalls, 2);
        expect(host.initializeTrayCalls, 1);
        expect(controller.setupIssue.value, isNull);
      },
    );

    test('shows and focuses the window from the tray or Dock', () async {
      final host = _FakeDesktopWindowHost();
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );

      await controller.initialize();
      await controller.showFromTrayOrDock();

      expect(host.showAndFocusCalls, 1);
    });

    test('opens the window on Settings from the tray', () async {
      final host = _FakeDesktopWindowHost();
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );
      var settingsOpenCalls = 0;
      controller.setOpenSettings(() {
        settingsOpenCalls += 1;
      });

      await controller.initialize();
      await host.requestSettings();

      expect(host.showAndFocusCalls, 1);
      expect(settingsOpenCalls, 1);
    });

    test('updates and invokes the capture action from the tray', () async {
      final host = _FakeDesktopWindowHost();
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );
      var toggleCalls = 0;
      controller.setCaptureToggle(() async {
        toggleCalls += 1;
      });

      await controller.initialize();
      await controller.updateCaptureState(available: true, paused: false);
      await host.requestCapture();

      expect(host.captureAvailable, isTrue);
      expect(host.capturePaused, isFalse);
      expect(toggleCalls, 1);
    });

    test('persists geometry and exits from the explicit quit action', () async {
      final host = _FakeDesktopWindowHost();
      final geometryStore = _FakeGeometryStore();
      final controller = DesktopWindowController(
        host: host,
        geometryStore: geometryStore,
      );

      await controller.initialize();
      await controller.quit();

      expect(host.quitCalls, 1);
      expect(geometryStore.lastWrittenBounds, host.bounds);
    });

    test(
      'allows a quit retry after the host rejects the first attempt',
      () async {
        final host = _FakeDesktopWindowHost(failedQuitAttempts: 1);
        final controller = DesktopWindowController(
          host: host,
          geometryStore: _FakeGeometryStore(),
        );

        await controller.initialize();
        await controller.quit();
        await controller.retry();

        expect(host.quitCalls, 2);
        expect(
          controller.setupIssue.value,
          DesktopWindowSetupIssue.quitUnavailable,
        );
      },
    );

    test('disposes host resources and ignores a repeated dispose', () async {
      final host = _FakeDesktopWindowHost();
      final controller = DesktopWindowController(
        host: host,
        geometryStore: _FakeGeometryStore(),
      );

      await controller.initialize();
      await controller.dispose();
      await controller.dispose();

      expect(host.disposeCalls, 1);
    });

    test(
      'disposal waits for setup and prevents late tray initialization',
      () async {
        final windowInitializationStarted = Completer<void>();
        final windowInitializationGate = Completer<void>();
        final host = _FakeDesktopWindowHost(
          windowInitializationStarted: windowInitializationStarted,
          windowInitializationGate: windowInitializationGate,
        );
        final controller = DesktopWindowController(
          host: host,
          geometryStore: _FakeGeometryStore(),
        );

        final initialization = controller.initialize();
        await windowInitializationStarted.future;
        final disposal = controller.dispose();

        expect(host.disposeCalls, 0);
        windowInitializationGate.complete();
        await Future.wait(<Future<void>>[initialization, disposal]);

        expect(host.initializeTrayCalls, 0);
        expect(host.disposeCalls, 1);
      },
    );

    test('disposal prevents a pending geometry read from writing', () async {
      final boundsReadStarted = Completer<void>();
      final boundsReadGate = Completer<void>();
      final host = _FakeDesktopWindowHost(
        boundsReadStarted: boundsReadStarted,
        boundsReadGate: boundsReadGate,
      );
      final geometryStore = _FakeGeometryStore();
      final controller = DesktopWindowController(
        host: host,
        geometryStore: geometryStore,
      );

      await controller.initialize();
      final save = host.requestBoundsChanged();
      await boundsReadStarted.future;
      final disposal = controller.dispose();
      boundsReadGate.complete();
      await Future.wait(<Future<void>>[save, disposal]);

      expect(geometryStore.lastWrittenBounds, isNull);
      expect(host.disposeCalls, 1);
    });

    test(
      'does not replace saved geometry with startup bounds events',
      () async {
        const savedBounds = DesktopWindowBounds(
          left: 240,
          top: 120,
          width: 900,
          height: 700,
        );
        final host = _FakeDesktopWindowHost(
          emitsBoundsDuringWindowInitialization: true,
        );
        final geometryStore = _FakeGeometryStore()..storedBounds = savedBounds;
        final controller = DesktopWindowController(
          host: host,
          geometryStore: geometryStore,
        );

        await controller.initialize();

        expect(host.bounds.left, savedBounds.left);
        expect(host.bounds.top, savedBounds.top);
        expect(host.bounds.width, savedBounds.width);
        expect(host.bounds.height, savedBounds.height);
        expect(geometryStore.lastWrittenBounds, isNull);
      },
    );

    test(
      'uses the available work area when applying the minimum size',
      () async {
        final host = _FakeDesktopWindowHost(
          workArea: const DesktopWorkArea(
            left: 0,
            top: 0,
            width: 320,
            height: 450,
          ),
        );
        final controller = DesktopWindowController(
          host: host,
          geometryStore: _FakeGeometryStore(),
        );

        await controller.initialize();

        expect(host.minimumSize?.width, 320);
        expect(host.minimumSize?.height, 450);
        expect(host.bounds.width, 320);
        expect(host.bounds.height, 450);
      },
    );
  });

  group('restoreDesktopWindowBounds', () {
    test('clamps saved bounds to the available monitor work area', () {
      final restored = restoreDesktopWindowBounds(
        workArea: const DesktopWorkArea(
          left: 100,
          top: 50,
          width: 800,
          height: 600,
        ),
        savedBounds: const DesktopWindowBounds(
          left: -400,
          top: -300,
          width: 2000,
          height: 2000,
        ),
      );

      expect(restored.left, 100);
      expect(restored.top, 50);
      expect(restored.width, 800);
      expect(restored.height, 600);
    });
  });
}

class _FakeDesktopWindowHost implements DesktopWindowHost {
  _FakeDesktopWindowHost({
    this.failedTrayInitializations = 0,
    this.failedQuitAttempts = 0,
    this.failedShowAndFocusAttempts = 0,
    this.failedWindowInitializations = 0,
    this.failedCloseHandlerInitializations = 0,
    this.emitsBoundsDuringWindowInitialization = false,
    this.windowInitializationStarted,
    this.windowInitializationGate,
    this.boundsReadStarted,
    this.boundsReadGate,
    DesktopWorkArea? workArea,
  }) : workArea =
           workArea ??
           const DesktopWorkArea(left: 0, top: 0, width: 1920, height: 1080);

  int failedTrayInitializations;
  int failedQuitAttempts;
  int failedShowAndFocusAttempts;
  int failedWindowInitializations;
  int failedCloseHandlerInitializations;
  final bool emitsBoundsDuringWindowInitialization;
  final Completer<void>? windowInitializationStarted;
  final Completer<void>? windowInitializationGate;
  final Completer<void>? boundsReadStarted;
  final Completer<void>? boundsReadGate;
  final DesktopWorkArea workArea;
  DesktopWindowBounds bounds = desktopWindowInitialBounds;
  Future<void> Function()? closeHandler;
  Future<void> Function()? boundsChangedHandler;
  Future<void> Function()? openHandler;
  Future<void> Function()? captureHandler;
  Future<void> Function()? settingsHandler;
  int hideCalls = 0;
  int showAndFocusCalls = 0;
  int quitCalls = 0;
  int initializeWindowCalls = 0;
  int setCloseHandlerCalls = 0;
  int initializeTrayCalls = 0;
  int disposeCalls = 0;
  DesktopWindowSize? minimumSize;

  @override
  Future<void> dispose() async {
    disposeCalls += 1;
    closeHandler = null;
    boundsChangedHandler = null;
    openHandler = null;
    captureHandler = null;
    settingsHandler = null;
  }

  @override
  Future<DesktopWindowBounds> getBounds() async {
    boundsReadStarted?.complete();
    await boundsReadGate?.future;
    return bounds;
  }

  @override
  Future<void> hide() async {
    hideCalls += 1;
  }

  @override
  Future<void> initializeTray({
    required Future<void> Function() onOpenRequested,
    required Future<void> Function() onCaptureRequested,
    required Future<void> Function() onSettingsRequested,
    required Future<void> Function() onQuitRequested,
  }) async {
    initializeTrayCalls += 1;
    if (failedTrayInitializations > 0) {
      failedTrayInitializations -= 1;
      throw StateError('Tray setup failed.');
    }
    openHandler = onOpenRequested;
    captureHandler = onCaptureRequested;
    settingsHandler = onSettingsRequested;
  }

  bool captureAvailable = false;
  bool capturePaused = true;

  @override
  Future<void> updateCaptureMenu({
    required bool available,
    required bool paused,
  }) async {
    captureAvailable = available;
    capturePaused = paused;
  }

  @override
  Future<void> initializeWindow(DesktopWindowBounds initialBounds) async {
    initializeWindowCalls += 1;
    windowInitializationStarted?.complete();
    await windowInitializationGate?.future;
    if (failedWindowInitializations > 0) {
      failedWindowInitializations -= 1;
      throw StateError('Window setup failed.');
    }
    bounds = initialBounds;
    if (emitsBoundsDuringWindowInitialization) {
      await boundsChangedHandler?.call();
    }
  }

  @override
  Future<void> quit() async {
    quitCalls += 1;
    if (failedQuitAttempts > 0) {
      failedQuitAttempts -= 1;
      throw StateError('Quit failed.');
    }
  }

  @override
  void setBoundsChangedHandler(Future<void> Function() onBoundsChanged) {
    boundsChangedHandler = onBoundsChanged;
  }

  @override
  Future<void> setBounds(DesktopWindowBounds value) async {
    bounds = value;
  }

  @override
  Future<void> setMinimumSize(DesktopWindowSize size) async {
    minimumSize = size;
  }

  @override
  Future<void> setCloseHandler(Future<void> Function() onCloseRequested) async {
    setCloseHandlerCalls += 1;
    if (failedCloseHandlerInitializations > 0) {
      failedCloseHandlerInitializations -= 1;
      throw StateError('Close handler setup failed.');
    }
    closeHandler = onCloseRequested;
  }

  @override
  Future<void> showAndFocus() async {
    showAndFocusCalls += 1;
    if (failedShowAndFocusAttempts > 0) {
      failedShowAndFocusAttempts -= 1;
      throw StateError('Window focus failed.');
    }
  }

  @override
  Future<DesktopWorkArea> workAreaFor(DesktopWindowBounds bounds) async {
    return workArea;
  }

  Future<void> requestClose() async {
    await closeHandler?.call();
  }

  Future<void> requestBoundsChanged() async {
    await boundsChangedHandler?.call();
  }

  Future<void> requestOpen() async {
    await openHandler?.call();
  }

  Future<void> requestCapture() async {
    await captureHandler?.call();
  }

  Future<void> requestSettings() async {
    await settingsHandler?.call();
  }
}

class _FakeGeometryStore implements DesktopWindowGeometryStore {
  DesktopWindowBounds? storedBounds;
  DesktopWindowBounds? lastWrittenBounds;

  @override
  Future<DesktopWindowBounds?> read() async => storedBounds;

  @override
  Future<void> write(DesktopWindowBounds bounds) async {
    lastWrittenBounds = bounds;
  }
}
