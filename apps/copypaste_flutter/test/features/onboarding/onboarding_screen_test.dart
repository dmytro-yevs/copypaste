import 'package:copypaste_flutter/features/onboarding/controller/macos_onboarding_setup.dart';
import 'package:copypaste_flutter/features/onboarding/controller/onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/onboarding_store.dart';
import 'package:copypaste_flutter/features/onboarding/view/onboarding_screen.dart';
import 'package:copypaste_flutter/app/shell/macos_window_header.dart';
import 'package:copypaste_flutter/platform/macos/macos_setup_gateway.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'onboarding_test_support.dart';

void main() {
  testWidgets('missing Login Item record can be enabled from the switch', (
    tester,
  ) async {
    final setup = _ScreenMacosSetupGateway()
      ..status = MacosLoginItemStatus.notFound;
    final controller = macosOnboarding(
      store: MemoryOnboardingStore(),
      setup: setup,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.showSetup();
    final macos = controller.setup as MacosOnboardingSetup;
    await macos.setLaunchAtLogin(false);
    setup.status = MacosLoginItemStatus.notFound;
    await macos.refresh();
    await tester.pumpWidget(
      ShadcnApp(
        home: OnboardingScreen(controller: controller, onFinished: () async {}),
      ),
    );

    expect(tester.widget<Switch>(find.byType(Switch)).value, isFalse);
    expect(tester.widget<Switch>(find.byType(Switch)).onChanged, isNotNull);
    await tester.ensureVisible(find.byType(Switch));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(Switch));
    await tester.pumpAndSettle();

    expect(setup.status, MacosLoginItemStatus.enabled);
    expect(tester.widget<Switch>(find.byType(Switch)).value, isTrue);
    expect(controller.step, OnboardingStep.setup);
  });

  testWidgets('runs Welcome, combined Setup, and Sync in order', (
    tester,
  ) async {
    final setup = _ScreenMacosSetupGateway();
    final controller = macosOnboarding(
      store: MemoryOnboardingStore(),
      setup: setup,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    var finishCalls = 0;

    await tester.pumpWidget(
      ShadcnApp(
        home: OnboardingScreen(
          controller: controller,
          onFinished: () async {
            finishCalls += 1;
          },
        ),
      ),
    );

    expect(find.text('Welcome to CopyPaste'), findsOneWidget);
    await tester.tap(find.widgetWithText(Button, 'Continue'));
    await tester.pump();

    expect(find.text('Accessibility'), findsOneWidget);
    expect(find.text('Optional for auto-paste'), findsOneWidget);
    expect(find.text('Start at login'), findsOneWidget);
    expect(tester.widget<Switch>(find.byType(Switch)).value, isTrue);
    expect(
      tester.widget<Button>(find.widgetWithText(Button, 'Continue')).onPressed,
      isNotNull,
    );
    await tester.tap(find.widgetWithText(Button, 'Continue'));
    await tester.pump();
    await tester.pump();

    expect(find.text('CopyPaste is ready'), findsOneWidget);
    await tester.tap(find.widgetWithText(Button, 'Get started'));
    await tester.pump();
    await tester.pump();

    expect(finishCalls, 1);
    expect(find.byType(Button), findsOneWidget);
    expect(controller.complete, isTrue);
  });

  testWidgets('fits the minimum desktop size with enlarged text', (
    tester,
  ) async {
    addTearDown(() => tester.binding.setSurfaceSize(null));
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await tester.binding.setSurfaceSize(const Size(360, 480));
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    final setup = _ScreenMacosSetupGateway(accessibility: true);
    final controller = macosOnboarding(
      store: MemoryOnboardingStore(),
      setup: setup,
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    await tester.pumpWidget(
      ShadcnApp(
        home: OnboardingScreen(controller: controller, onFinished: () async {}),
      ),
    );
    expect(tester.takeException(), isNull);

    controller.showSetup();
    await tester.pump();
    expect(tester.takeException(), isNull);

    await controller.continueFromSetup();
    await tester.pump();
    expect(tester.takeException(), isNull);
  });

  testWidgets('shows pending Login Item approval without blocking Sync', (
    tester,
  ) async {
    final setup = _ScreenMacosSetupGateway(
      accessibility: true,
      statusAfterUpdate: MacosLoginItemStatus.requiresApproval,
    );
    final controller = macosOnboarding(
      store: MemoryOnboardingStore(),
      setup: setup,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showSetup();

    await tester.pumpWidget(
      ShadcnApp(
        home: OnboardingScreen(controller: controller, onFinished: () async {}),
      ),
    );
    await tester.tap(find.widgetWithText(Button, 'Continue'));
    await tester.pumpAndSettle();

    expect(find.text('CopyPaste is ready'), findsOneWidget);
    expect(find.text('Optional setup needs attention'), findsOneWidget);
    expect(find.widgetWithText(Button, 'Open Settings'), findsNothing);
    expect(find.widgetWithText(Button, 'Get started'), findsOneWidget);
  });

  testWidgets('uses the unified header when macOS owns the title bar', (
    tester,
  ) async {
    final controller = macosOnboarding(
      store: MemoryOnboardingStore(),
      setup: _ScreenMacosSetupGateway(),
    );
    addTearDown(controller.dispose);
    await controller.initialize();

    await tester.pumpWidget(
      ShadcnApp(
        home: OnboardingScreen(
          controller: controller,
          onFinished: () async {},
          unifiedTitleBar: true,
        ),
      ),
    );

    expect(find.byType(MacosWindowHeader), findsOneWidget);
    expect(find.byType(AppBar), findsOneWidget);
    expect(find.text('Set up CopyPaste'), findsOneWidget);
  });
}

class _ScreenMacosSetupGateway implements MacosSetupGateway {
  _ScreenMacosSetupGateway({
    this.accessibility = false,
    this.statusAfterUpdate = MacosLoginItemStatus.enabled,
  });

  bool accessibility;
  MacosLoginItemStatus status = MacosLoginItemStatus.notRegistered;
  MacosLoginItemStatus statusAfterUpdate;

  @override
  Future<bool> accessibilityGranted() async => accessibility;

  @override
  Future<MacosLoginItemStatus> loginItemStatus() async => status;

  @override
  Future<void> openLoginItemsSettings() async {}

  @override
  Future<bool> requestAccessibility() async => accessibility;

  @override
  Future<MacosLoginItemStatus> setLaunchAtLogin(bool enabled) async {
    status = enabled ? statusAfterUpdate : MacosLoginItemStatus.notRegistered;
    return status;
  }
}
