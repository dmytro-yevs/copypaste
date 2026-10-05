import 'dart:async';

import 'package:copypaste_flutter/features/onboarding/controller/android_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/android_onboarding_store.dart';
import 'package:copypaste_flutter/platform/android/android_capture_setup_gateway.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test(
    'a queued older clipboard save cannot verify a new capture attempt',
    () async {
      final setup = _FakeAndroidCaptureSetupGateway(
        current: _state(
          privilegedGrants: true,
          notificationGranted: true,
          lastCaptureAtMs: 10,
          observedAtMs: 20,
        ),
      );
      final controller = _controller(setup: setup);
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.showCapture();
      await controller.beginVerification();
      setup.current = _state(
        privilegedGrants: true,
        notificationGranted: true,
        captureEnabled: true,
        serviceRunning: true,
        lastCaptureAtMs: 15,
      );
      await controller.refresh();
      expect(controller.verified, isFalse);
      setup.current = _state(
        privilegedGrants: true,
        notificationGranted: true,
        captureEnabled: true,
        serviceRunning: true,
        lastCaptureAtMs: 21,
      );
      await controller.refresh();
      expect(controller.verified, isTrue);
    },
  );
  test('observes external permission grants and applies setup once', () async {
    final setup = _FakeAndroidCaptureSetupGateway();
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    addTearDown(setup.events.close);
    await controller.initialize();
    controller.showCapture();
    controller.setMonitoring(true);
    await Future<void>.delayed(Duration.zero);

    setup.current = _state(shizukuPermission: true);
    setup.events.add(setup.current);
    await Future<void>.delayed(Duration.zero);
    expect(setup.applyCalls, 1);
    expect(controller.setupState!.shizuku.permission, isTrue);
    expect(
      controller.errorMessage,
      contains('capture grants could not be applied'),
    );

    setup.events.add(setup.current);
    await Future<void>.delayed(Duration.zero);
    expect(setup.applyCalls, 1);
    controller.setMonitoring(false);
    expect(setup.events.hasListener, isFalse);
  });

  test(
    'new background receipt verifies automatically and revocation blocks it',
    () async {
      final setup = _FakeAndroidCaptureSetupGateway(
        current: _state(
          privilegedGrants: true,
          notificationGranted: true,
          lastCaptureAtMs: 10,
        ),
      );
      final controller = _controller(setup: setup);
      addTearDown(controller.dispose);
      addTearDown(setup.events.close);
      await controller.initialize();
      controller.showCapture();
      controller.setMonitoring(true);
      await Future<void>.delayed(Duration.zero);
      await controller.beginVerification();

      setup.events.add(
        _state(
          privilegedGrants: true,
          notificationGranted: true,
          captureEnabled: true,
          serviceRunning: true,
          lastCaptureAtMs: 11,
        ),
      );
      await Future<void>.delayed(Duration.zero);
      expect(controller.verified, isTrue);
      expect(controller.canContinueCapture, isTrue);

      setup.events.add(_state(lastCaptureAtMs: 11));
      await Future<void>.delayed(Duration.zero);
      expect(controller.verified, isFalse);
      expect(controller.canContinueCapture, isFalse);
    },
  );
  test(
    'fresh Android onboarding defaults to Full capture with Shizuku',
    () async {
      final controller = _controller();
      addTearDown(controller.dispose);

      await controller.initialize();

      expect(controller.step, AndroidOnboardingStep.welcome);
      expect(controller.mode, AndroidCaptureMode.full);
      expect(controller.method, AndroidCaptureSetupMethod.shizuku);
      expect(controller.canContinueCapture, isFalse);
    },
  );

  test('Limited mode can continue without privileged grants', () async {
    final store = MemoryAndroidOnboardingStore();
    final controller = _controller(store: store);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();

    await controller.selectMode(AndroidCaptureMode.limited);
    await controller.continueFromCapture();

    expect(controller.step, AndroidOnboardingStep.sync);
    expect(await controller.finish(), isTrue);
    expect(store.complete, isTrue);
    expect(store.mode, AndroidCaptureMode.limited);
  });

  test('Full mode requires a new capture from another app', () async {
    final setup = _FakeAndroidCaptureSetupGateway(
      current: _state(
        privilegedGrants: true,
        notificationGranted: true,
        lastCaptureAtMs: 10,
      ),
    );
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();

    await controller.beginVerification();
    expect(controller.verifying, isTrue);
    expect(controller.verified, isFalse);
    expect(controller.canContinueCapture, isFalse);

    setup.current = _state(
      privilegedGrants: true,
      notificationGranted: true,
      captureEnabled: true,
      serviceRunning: true,
      lastCaptureAtMs: 11,
    );
    await controller.refresh();

    expect(controller.verified, isTrue);
    expect(controller.canContinueCapture, isTrue);
  });

  test('Full mode remains blocked when only an older capture exists', () async {
    final setup = _FakeAndroidCaptureSetupGateway(
      current: _state(
        privilegedGrants: true,
        notificationGranted: true,
        lastCaptureAtMs: 10,
      ),
    );
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showCapture();

    await controller.beginVerification();
    setup.current = _state(
      privilegedGrants: true,
      notificationGranted: true,
      captureEnabled: true,
      serviceRunning: true,
      lastCaptureAtMs: 10,
    );
    await controller.refresh();

    expect(controller.verified, isFalse);
    expect(controller.canContinueCapture, isFalse);
  });

  test('completed Limited setup can reopen Full capture setup', () async {
    final store = MemoryAndroidOnboardingStore(
      complete: true,
      mode: AndroidCaptureMode.limited,
    );
    final controller = _controller(store: store);
    addTearDown(controller.dispose);
    await controller.initialize();

    expect(await controller.reopenCaptureSetup(), isTrue);

    expect(controller.complete, isFalse);
    expect(controller.step, AndroidOnboardingStep.capture);
    expect(store.complete, isFalse);
  });
}

AndroidOnboardingController _controller({
  AndroidOnboardingStore? store,
  _FakeAndroidCaptureSetupGateway? setup,
}) => AndroidOnboardingController(
  store: store ?? MemoryAndroidOnboardingStore(),
  setup: setup ?? _FakeAndroidCaptureSetupGateway(),
);

AndroidCaptureSetupState _state({
  bool privilegedGrants = false,
  bool notificationGranted = false,
  bool batteryExempt = false,
  bool captureEnabled = false,
  bool serviceRunning = false,
  int lastCaptureAtMs = 0,
  int observedAtMs = 0,
  bool shizukuPermission = false,
}) => AndroidCaptureSetupState(
  packageName: 'com.copypaste.app',
  privilegedGrants: privilegedGrants,
  notificationGranted: notificationGranted,
  batteryExempt: batteryExempt,
  captureEnabled: captureEnabled,
  serviceRunning: serviceRunning,
  lastCaptureAtMs: lastCaptureAtMs,
  observedAtMs: observedAtMs,
  shizuku: AndroidShizukuState(
    supported: true,
    installed: true,
    running: true,
    permission: shizukuPermission,
  ),
  adbCommands: const [
    'adb shell pm grant com.copypaste.app android.permission.READ_LOGS',
    'adb shell cmd appops set com.copypaste.app SYSTEM_ALERT_WINDOW allow',
    'adb shell cmd appops set com.copypaste.app RUN_IN_BACKGROUND allow',
    'adb shell cmd appops set com.copypaste.app RUN_ANY_IN_BACKGROUND allow',
    'adb shell am set-inactive com.copypaste.app false',
    'adb shell am set-standby-bucket com.copypaste.app active',
  ],
);

class _FakeAndroidCaptureSetupGateway implements AndroidCaptureSetupGateway {
  _FakeAndroidCaptureSetupGateway({AndroidCaptureSetupState? current})
    : current = current ?? _state();

  AndroidCaptureSetupState current;
  final events = StreamController<AndroidCaptureSetupState>.broadcast();
  int applyCalls = 0;

  @override
  Stream<AndroidCaptureSetupState> get changes => events.stream;

  @override
  Future<AndroidCaptureSetupState> applyShizukuGrants() async {
    applyCalls++;
    return current;
  }

  @override
  Future<bool> openShizuku() async => true;

  @override
  Future<bool> requestBatteryExemption() async => true;

  @override
  Future<AndroidCaptureSetupState> requestNotifications() async => current;

  @override
  Future<bool> setForegroundCaptureEnabled(bool enabled) async => true;

  @override
  Future<AndroidCaptureSetupState> startCapture() async {
    current = AndroidCaptureSetupState(
      packageName: current.packageName,
      privilegedGrants: current.privilegedGrants,
      notificationGranted: current.notificationGranted,
      batteryExempt: current.batteryExempt,
      captureEnabled: true,
      serviceRunning: current.serviceRunning,
      lastCaptureAtMs: current.lastCaptureAtMs,
      shizuku: current.shizuku,
      adbCommands: current.adbCommands,
    );
    return current;
  }

  @override
  Future<AndroidCaptureSetupState> state() async => current;

  @override
  Future<AndroidCaptureSetupState> stopCapture() async => current;
}
