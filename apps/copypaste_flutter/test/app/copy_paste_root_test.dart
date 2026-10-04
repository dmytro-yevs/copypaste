import 'dart:io';

import 'package:copypaste_flutter/main.dart';
import 'package:copypaste_flutter/app/shell/macos_window_header.dart';
import 'package:copypaste_flutter/features/onboarding/controller/android_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/controller/macos_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/android_onboarding_store.dart';
import 'package:copypaste_flutter/features/onboarding/repository/macos_onboarding_store.dart';
import 'package:copypaste_flutter/platform/android/android_capture_setup_gateway.dart';
import 'package:copypaste_flutter/platform/desktop/desktop_window_controller.dart';
import 'package:copypaste_flutter/platform/macos/macos_setup_gateway.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';
import 'package:window_manager/window_manager.dart';

void main() {
  testWidgets('shows the injected macOS onboarding before the app shell', (
    tester,
  ) async {
    final onboarding = MacosOnboardingController(
      store: MemoryMacosOnboardingStore(),
      setup: _GrantedMacosSetupGateway(),
    );
    addTearDown(onboarding.dispose);

    await tester.pumpWidget(
      CopyPasteRoot(
        runtimeEnabled: false,
        macosOnboardingController: onboarding,
      ),
    );
    await tester.pump();

    expect(find.text('Welcome to CopyPaste'), findsOneWidget);
    expect(find.text('History runtime is unavailable'), findsNothing);
  });

  testWidgets('shows the injected Android onboarding before the app shell', (
    tester,
  ) async {
    final onboarding = AndroidOnboardingController(
      store: MemoryAndroidOnboardingStore(),
      setup: _AndroidSetupGateway(),
    );
    final macosOnboarding = _completedOnboarding();
    addTearDown(onboarding.dispose);
    addTearDown(macosOnboarding.dispose);

    await tester.pumpWidget(
      CopyPasteRoot(
        runtimeEnabled: false,
        androidOnboardingController: onboarding,
        macosOnboardingController: macosOnboarding,
      ),
    );
    await tester.pump();

    expect(find.text('Welcome to CopyPaste'), findsOneWidget);
    expect(find.text('History runtime is unavailable'), findsNothing);
  });

  testWidgets('opens Settings from the desktop tray', (tester) async {
    final host = _FakeDesktopWindowHost();
    final onboarding = _completedOnboarding();
    addTearDown(onboarding.dispose);
    final desktopWindow = DesktopWindowController(
      host: host,
      geometryStore: const _NoopGeometryStore(),
    );

    await tester.pumpWidget(
      CopyPasteRoot(
        desktopWindow: desktopWindow,
        macosOnboardingController: onboarding,
        runtimeEnabled: false,
      ),
    );
    await desktopWindow.initialize();
    await host.requestSettings();
    await tester.pump();

    expect(find.text('Settings runtime is unavailable'), findsOneWidget);
  });

  testWidgets('uses a standard header until native setup recovers', (
    tester,
  ) async {
    final host = _FakeDesktopWindowHost(windowInitializationFails: true);
    final onboarding = _completedOnboarding();
    addTearDown(onboarding.dispose);
    final desktopWindow = DesktopWindowController(
      host: host,
      geometryStore: const _NoopGeometryStore(),
    );

    await tester.pumpWidget(
      CopyPasteRoot(
        desktopWindow: desktopWindow,
        macosOnboardingController: onboarding,
        runtimeEnabled: false,
      ),
    );
    await tester.pump();
    await desktopWindow.initialize();
    await tester.pump();

    expect(find.text('Window setup needs attention'), findsOneWidget);
    expect(find.byType(MacosWindowHeader), findsNothing);
    expect(find.byType(DragToMoveArea), findsNothing);
    expect(find.byType(AppBar), findsOneWidget);

    host.windowInitializationFails = false;
    await tester.tap(find.text('Try again'));
    await tester.pumpAndSettle();

    expect(find.text('History runtime is unavailable'), findsOneWidget);
    expect(find.byType(MacosWindowHeader), findsOneWidget);
    expect(find.byType(DragToMoveArea), findsOneWidget);
  }, skip: !Platform.isMacOS);

  testWidgets('updates the onboarding header when native setup becomes ready', (
    tester,
  ) async {
    final host = _FakeDesktopWindowHost();
    final onboarding = MacosOnboardingController(
      store: MemoryMacosOnboardingStore(),
      setup: _GrantedMacosSetupGateway(),
    );
    addTearDown(onboarding.dispose);
    final desktopWindow = DesktopWindowController(
      host: host,
      geometryStore: const _NoopGeometryStore(),
    );

    await tester.pumpWidget(
      CopyPasteRoot(
        desktopWindow: desktopWindow,
        macosOnboardingController: onboarding,
        runtimeEnabled: false,
      ),
    );
    await tester.pump();

    expect(find.text('Welcome to CopyPaste'), findsOneWidget);
    expect(find.byType(MacosWindowHeader), findsNothing);
    expect(find.byType(DragToMoveArea), findsNothing);

    await desktopWindow.initialize();
    await tester.pump();

    expect(find.byType(MacosWindowHeader), findsOneWidget);
    expect(find.byType(DragToMoveArea), findsOneWidget);
  }, skip: !Platform.isMacOS);

  testWidgets('disposes its owned desktop window during teardown', (
    tester,
  ) async {
    final host = _FakeDesktopWindowHost();
    final onboarding = _completedOnboarding();
    addTearDown(onboarding.dispose);
    final desktopWindow = DesktopWindowController(
      host: host,
      geometryStore: const _NoopGeometryStore(),
    );

    await tester.pumpWidget(
      CopyPasteRoot(
        desktopWindow: desktopWindow,
        macosOnboardingController: onboarding,
        runtimeEnabled: false,
      ),
    );

    await tester.pumpWidget(const SizedBox());
    await tester.pump();

    expect(host.disposeCalls, 1);
  });
}

MacosOnboardingController _completedOnboarding() {
  return MacosOnboardingController(
    store: MemoryMacosOnboardingStore(complete: true),
    setup: _GrantedMacosSetupGateway(),
  );
}

class _GrantedMacosSetupGateway implements MacosSetupGateway {
  @override
  Future<bool> accessibilityGranted() async => true;

  @override
  Future<MacosLoginItemStatus> loginItemStatus() async =>
      MacosLoginItemStatus.enabled;

  @override
  Future<void> openLoginItemsSettings() async {}

  @override
  Future<bool> requestAccessibility() async => true;

  @override
  Future<MacosLoginItemStatus> setLaunchAtLogin(bool enabled) async => enabled
      ? MacosLoginItemStatus.enabled
      : MacosLoginItemStatus.notRegistered;
}

class _AndroidSetupGateway implements AndroidCaptureSetupGateway {
  static const stateValue = AndroidCaptureSetupState(
    packageName: 'com.copypaste.app',
    privilegedGrants: false,
    notificationGranted: false,
    batteryExempt: false,
    captureEnabled: false,
    serviceRunning: false,
    lastCaptureAtMs: 0,
    shizuku: AndroidShizukuState(
      supported: true,
      installed: false,
      running: false,
      permission: false,
    ),
    adbCommands: [],
  );

  @override
  Future<AndroidCaptureSetupState> applyShizukuGrants() async => stateValue;

  @override
  Future<bool> openShizuku() async => true;

  @override
  Future<bool> requestBatteryExemption() async => true;

  @override
  Future<AndroidCaptureSetupState> requestNotifications() async => stateValue;

  @override
  Future<bool> setForegroundCaptureEnabled(bool enabled) async => true;

  @override
  Future<AndroidCaptureSetupState> startCapture() async => stateValue;

  @override
  Future<AndroidCaptureSetupState> state() async => stateValue;

  @override
  Future<AndroidCaptureSetupState> stopCapture() async => stateValue;
}

class _NoopGeometryStore implements DesktopWindowGeometryStore {
  const _NoopGeometryStore();

  @override
  Future<DesktopWindowBounds?> read() async => null;

  @override
  Future<void> write(DesktopWindowBounds bounds) async {}
}

class _FakeDesktopWindowHost implements DesktopWindowHost {
  _FakeDesktopWindowHost({this.windowInitializationFails = false});

  int disposeCalls = 0;
  bool windowInitializationFails;
  Future<void> Function()? settingsHandler;

  @override
  Future<void> dispose() async {
    disposeCalls += 1;
  }

  @override
  Future<DesktopWindowBounds> getBounds() async => desktopWindowInitialBounds;

  @override
  Future<void> hide() async {}

  @override
  Future<void> initializeTray({
    required Future<void> Function() onOpenRequested,
    required Future<void> Function() onCaptureRequested,
    required Future<void> Function() onSettingsRequested,
    required Future<void> Function() onQuitRequested,
  }) async {
    settingsHandler = onSettingsRequested;
  }

  @override
  Future<void> updateCaptureMenu({
    required bool available,
    required bool paused,
  }) async {}

  @override
  Future<void> initializeWindow(DesktopWindowBounds initialBounds) async {
    if (windowInitializationFails) {
      throw StateError('Native window setup failed.');
    }
  }

  @override
  Future<void> quit() async {}

  @override
  void setBoundsChangedHandler(Future<void> Function() onBoundsChanged) {}

  @override
  Future<void> setBounds(DesktopWindowBounds bounds) async {}

  @override
  Future<void> setCloseHandler(
    Future<void> Function() onCloseRequested,
  ) async {}

  @override
  Future<void> setMinimumSize(DesktopWindowSize size) async {}

  @override
  Future<void> showAndFocus() async {}

  @override
  Future<DesktopWorkArea> workAreaFor(DesktopWindowBounds bounds) async {
    return const DesktopWorkArea(left: 0, top: 0, width: 1440, height: 900);
  }

  Future<void> requestSettings() async {
    await settingsHandler?.call();
  }
}
