import 'package:copypaste_flutter/features/onboarding/controller/macos_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/macos_onboarding_store.dart';
import 'package:copypaste_flutter/platform/macos/macos_setup_gateway.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
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
    if (status == MacosLoginItemStatus.developmentUnavailable) return status;
    status = enabled ? statusAfterUpdate : MacosLoginItemStatus.notRegistered;
    return status;
  }
}
