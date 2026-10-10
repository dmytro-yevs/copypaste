import 'package:copypaste_flutter/features/onboarding/controller/linux_onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/repository/linux_onboarding_store.dart';
import 'package:copypaste_flutter/features/onboarding/view/linux_onboarding_screen.dart';
import 'package:copypaste_flutter/platform/permissions/linux_integration.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

void main() {
  test(
    'Wayland setup requires the verified clipboard bridge and input consent',
    () async {
      final integration = _Integration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.wayland,
          globalShortcuts: true,
          remoteDesktop: LinuxRemoteDesktopState.consentRequired,
          companion: LinuxCompanionState.unavailable,
          clipboard: false,
          quickPaste: false,
          screenshotProtection: false,
        ),
      );
      final controller = LinuxOnboardingController(
        store: MemoryLinuxOnboardingStore(),
        integration: integration,
      );
      addTearDown(controller.dispose);

      await controller.initialize();
      controller.showIntegration();
      await controller.continueFromIntegration();
      expect(controller.step, LinuxOnboardingStep.integration);
      expect(
        controller.errorMessage,
        contains('signed GNOME or KDE companion'),
      );

      integration.statusValue = const LinuxIntegrationStatus(
        session: LinuxDesktopSession.wayland,
        globalShortcuts: true,
        remoteDesktop: LinuxRemoteDesktopState.active,
        companion: LinuxCompanionState.active,
        clipboard: true,
        quickPaste: true,
        screenshotProtection: false,
      );
      await controller.continueFromIntegration();
      expect(controller.step, LinuxOnboardingStep.sync);
    },
  );

  testWidgets('Linux setup states the screenshot protection exception', (
    tester,
  ) async {
    final controller = LinuxOnboardingController(
      store: MemoryLinuxOnboardingStore(),
      integration: _Integration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.wayland,
          globalShortcuts: true,
          remoteDesktop: LinuxRemoteDesktopState.consentRequired,
          companion: LinuxCompanionState.unavailable,
          clipboard: false,
          quickPaste: false,
          screenshotProtection: false,
        ),
      ),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    controller.showIntegration();

    await tester.pumpWidget(
      ShadcnApp(
        home: LinuxOnboardingScreen(
          controller: controller,
          onFinished: () async {},
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Screenshot blocking'), findsOneWidget);
    expect(
      find.textContaining('Screenshot blocking is unavailable on Linux.'),
      findsOneWidget,
    );
    expect(find.widgetWithText(Button, 'Open setup'), findsOneWidget);
  });
}

class _Integration implements LinuxIntegrationPort {
  _Integration(this.statusValue);

  LinuxIntegrationStatus statusValue;

  @override
  Future<bool> openCompanionSetup() async => true;

  @override
  Future<bool> requestRemoteDesktop() async => true;

  @override
  Future<LinuxIntegrationStatus> status() async => statusValue;
}
