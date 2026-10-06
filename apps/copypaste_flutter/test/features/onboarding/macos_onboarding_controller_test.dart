import 'dart:async';

import 'package:copypaste_flutter/features/onboarding/controller/macos_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/macos_onboarding_store.dart';
import 'package:copypaste_flutter/platform/macos/macos_setup_gateway.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test(
    'missing Login Item records remain available for registration',
    () async {
      final setup = _FakeMacosSetupGateway(
        status: MacosLoginItemStatus.notFound,
      );
      final controller = _controller(setup: setup);
      addTearDown(controller.dispose);
      await controller.initialize();

      expect(controller.launchAtLoginAvailable, isTrue);
      expect(controller.launchAtLogin, isTrue);
      await controller.refreshSystemState();
      expect(controller.launchAtLogin, isTrue);
      await controller.continueFromSetup();

      expect(setup.launchAtLoginValues, [true]);
      expect(controller.loginItemStatus, MacosLoginItemStatus.enabled);
      expect(controller.noticeMessage, isNull);
    },
  );

  test('switch applies changes before Continue and survives refresh', () async {
    final setup = _FakeMacosSetupGateway(status: MacosLoginItemStatus.enabled);
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();

    await controller.setLaunchAtLogin(false);
    expect(setup.launchAtLoginValues, [false]);
    expect(controller.launchAtLogin, isFalse);
    await controller.refreshSystemState();
    expect(controller.launchAtLogin, isFalse);

    await controller.setLaunchAtLogin(true);
    expect(setup.launchAtLoginValues, [false, true]);
    expect(controller.launchAtLogin, isTrue);
    expect(controller.step, MacosOnboardingStep.setup);
  });

  test('completed onboarding reflects a missing Login Item record', () async {
    final controller = _controller(
      store: MemoryMacosOnboardingStore(complete: true),
      setup: _FakeMacosSetupGateway(status: MacosLoginItemStatus.notFound),
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    expect(controller.launchAtLogin, isFalse);
    expect(controller.launchAtLoginAvailable, isTrue);
    await controller.setLaunchAtLogin(true);
    expect(controller.launchAtLogin, isTrue);
  });

  test('refresh reflects a Login Item disabled outside CopyPaste', () async {
    final setup = _FakeMacosSetupGateway(status: MacosLoginItemStatus.enabled);
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    setup.status = MacosLoginItemStatus.notRegistered;

    await controller.refreshSystemState();

    expect(controller.launchAtLogin, isFalse);
  });

  test('failed switch update restores the confirmed system state', () async {
    final setup = _FakeMacosSetupGateway(
      status: MacosLoginItemStatus.notFound,
      failLaunchAtLoginUpdate: true,
    );
    final controller = _controller(
      store: MemoryMacosOnboardingStore(complete: true),
      setup: setup,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    await controller.setLaunchAtLogin(true);

    expect(controller.launchAtLogin, isFalse);
    expect(controller.busy, isFalse);
    expect(controller.noticeMessage, contains('could not be updated'));
  });

  test(
    'switch reports required approval and locks concurrent actions',
    () async {
      final setup = _FakeMacosSetupGateway();
      final controller = _controller(
        store: MemoryMacosOnboardingStore(complete: true),
        setup: setup,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      final update = Completer<MacosLoginItemStatus>();
      setup.pendingUpdate = update.future;

      final enabling = controller.setLaunchAtLogin(true);
      expect(controller.busy, isTrue);
      expect(await controller.continueFromSetup(), isFalse);
      await controller.setLaunchAtLogin(false);
      expect(setup.launchAtLoginValues, [true]);
      update.complete(MacosLoginItemStatus.requiresApproval);
      await enabling;

      expect(controller.busy, isFalse);
      expect(controller.launchAtLogin, isTrue);
      expect(controller.loginItemNeedsAttention, isTrue);
      expect(controller.noticeMessage, contains('requires approval'));
    },
  );

  test(
    'fresh onboarding starts at Welcome with Start at login enabled',
    () async {
      final controller = _controller();
      addTearDown(controller.dispose);

      await controller.initialize();

      expect(controller.initialized, isTrue);
      expect(controller.complete, isFalse);
      expect(controller.step, MacosOnboardingStep.welcome);
      expect(controller.launchAtLogin, isTrue);
    },
  );

  test('Accessibility can still be enabled from Setup', () async {
    final setup = _FakeMacosSetupGateway();
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();

    setup.accessibility = true;
    await controller.requestAccessibility();

    expect(controller.accessibilityGranted, isTrue);
  });

  test('development builds do not block on Start at login', () async {
    final setup = _FakeMacosSetupGateway(
      status: MacosLoginItemStatus.developmentUnavailable,
    );
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();

    expect(controller.launchAtLogin, isFalse);
    expect(controller.launchAtLoginAvailable, isFalse);
    expect(await controller.continueFromSetup(), isTrue);
    expect(controller.step, MacosOnboardingStep.sync);
  });

  test(
    'Accessibility is optional while Start at login still applies',
    () async {
      final setup = _FakeMacosSetupGateway();
      final controller = _controller(setup: setup);
      addTearDown(controller.dispose);
      await controller.initialize();
      controller.showSetup();

      expect(controller.canContinueSetup, isTrue);
      expect(await controller.continueFromSetup(), isTrue);
      expect(controller.step, MacosOnboardingStep.sync);
      expect(setup.launchAtLoginValues, [true]);
    },
  );

  test(
    'completed onboarding stays complete when Accessibility is revoked',
    () async {
      final store = MemoryMacosOnboardingStore(complete: true);
      final controller = _controller(store: store);
      addTearDown(controller.dispose);

      await controller.initialize();

      expect(controller.complete, isTrue);
    },
  );

  test('finish persists completion after the Sync step', () async {
    final store = MemoryMacosOnboardingStore();
    final setup = _FakeMacosSetupGateway(accessibility: true);
    final controller = _controller(store: store, setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();
    await controller.continueFromSetup();

    expect(await controller.finish(), isTrue);
    expect(store.complete, isTrue);
    expect(controller.complete, isTrue);
  });

  test('Login Item approval is reported without blocking onboarding', () async {
    final setup = _FakeMacosSetupGateway(
      accessibility: true,
      statusAfterUpdate: MacosLoginItemStatus.requiresApproval,
    );
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();

    expect(await controller.continueFromSetup(), isTrue);
    expect(controller.step, MacosOnboardingStep.sync);
    expect(controller.errorMessage, isNull);
    expect(controller.noticeMessage, contains('requires approval'));
  });

  test('Login Item update errors do not block onboarding', () async {
    final setup = _FakeMacosSetupGateway(
      accessibility: true,
      failLaunchAtLoginUpdate: true,
    );
    final controller = _controller(setup: setup);
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();

    expect(await controller.continueFromSetup(), isTrue);
    expect(controller.step, MacosOnboardingStep.sync);
    expect(controller.errorMessage, isNull);
    expect(controller.noticeMessage, contains('continue without it'));
  });
}

MacosOnboardingController _controller({
  MacosOnboardingStore? store,
  _FakeMacosSetupGateway? setup,
}) {
  return MacosOnboardingController(
    store: store ?? MemoryMacosOnboardingStore(),
    setup: setup ?? _FakeMacosSetupGateway(),
  );
}

class _FakeMacosSetupGateway implements MacosSetupGateway {
  _FakeMacosSetupGateway({
    this.accessibility = false,
    this.status = MacosLoginItemStatus.notRegistered,
    this.statusAfterUpdate = MacosLoginItemStatus.enabled,
    this.failLaunchAtLoginUpdate = false,
  });

  bool accessibility;
  MacosLoginItemStatus status;
  MacosLoginItemStatus statusAfterUpdate;
  bool failLaunchAtLoginUpdate;
  final List<bool> launchAtLoginValues = [];
  int openSettingsCalls = 0;
  Future<MacosLoginItemStatus>? pendingUpdate;

  @override
  Future<bool> accessibilityGranted() async => accessibility;

  @override
  Future<MacosLoginItemStatus> loginItemStatus() async => status;

  @override
  Future<void> openLoginItemsSettings() async {
    openSettingsCalls += 1;
  }

  @override
  Future<bool> requestAccessibility() async => accessibility;

  @override
  Future<MacosLoginItemStatus> setLaunchAtLogin(bool enabled) async {
    if (failLaunchAtLoginUpdate) {
      throw StateError('Login Item update failed.');
    }
    launchAtLoginValues.add(enabled);
    if (pendingUpdate case final update?) return status = await update;
    if (status == MacosLoginItemStatus.developmentUnavailable) return status;
    status = enabled ? statusAfterUpdate : MacosLoginItemStatus.notRegistered;
    return status;
  }
}
