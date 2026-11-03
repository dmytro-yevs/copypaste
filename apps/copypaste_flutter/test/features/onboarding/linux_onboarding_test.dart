import 'package:copypaste_flutter/features/onboarding/controller/onboarding_controller.dart';
import 'package:copypaste_flutter/features/onboarding/controller/linux_onboarding_setup.dart';
import 'package:copypaste_flutter/features/onboarding/repository/onboarding_store.dart';
import 'package:copypaste_flutter/features/onboarding/view/onboarding_screen.dart';
import 'package:copypaste_flutter/platform/permissions/linux_integration.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import 'onboarding_test_support.dart';

void main() {
  test('failed readiness refresh cannot advance using stale success', () async {
    final integration = _Integration(
      const LinuxIntegrationStatus(
        session: LinuxDesktopSession.x11,
        globalShortcuts: true,
        remoteDesktop: LinuxRemoteDesktopState.unsupported,
        companion: LinuxCompanionState.unavailable,
        clipboard: true,
        quickPaste: true,
        screenshotProtection: false,
      ),
    );
    final controller = linuxOnboarding(
      store: MemoryOnboardingStore(),
      integration: integration,
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.showSetup();
    expect(controller.linux.integrationReady, isTrue);

    integration.failStatus = true;
    await controller.continueFromSetup();
    expect(controller.step, OnboardingStep.setup);
    expect(controller.errorMessage, contains('could not be refreshed'));
    expect(await controller.finish(), isFalse);

    integration.failStatus = false;
    await controller.continueFromSetup();
    expect(controller.step, OnboardingStep.ready);
    expect(controller.errorMessage, isNull);
  });

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
          startAtLogin: false,
          uriRegistered: false,
        ),
      );
      final controller = linuxOnboarding(
        store: MemoryOnboardingStore(),
        integration: integration,
      );
      addTearDown(controller.dispose);

      await controller.initialize();
      await controller.showSetup();
      await controller.continueFromSetup();
      expect(controller.step, OnboardingStep.setup);
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
        startAtLogin: false,
        uriRegistered: false,
      );
      await controller.continueFromSetup();
      expect(controller.step, OnboardingStep.ready);
    },
  );

  test(
    'X11 setup requires clipboard availability before Quick Paste',
    () async {
      final integration = _Integration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.x11,
          globalShortcuts: true,
          remoteDesktop: LinuxRemoteDesktopState.unsupported,
          companion: LinuxCompanionState.unavailable,
          clipboard: false,
          quickPaste: true,
          screenshotProtection: false,
        ),
      );
      final controller = linuxOnboarding(
        store: MemoryOnboardingStore(),
        integration: integration,
      );
      addTearDown(controller.dispose);

      await controller.initialize();
      await controller.showSetup();
      await controller.continueFromSetup();

      expect(controller.step, OnboardingStep.setup);
      expect(
        controller.errorMessage,
        contains('Clipboard access is unavailable'),
      );

      integration.statusValue = const LinuxIntegrationStatus(
        session: LinuxDesktopSession.x11,
        globalShortcuts: true,
        remoteDesktop: LinuxRemoteDesktopState.unsupported,
        companion: LinuxCompanionState.unavailable,
        clipboard: true,
        quickPaste: true,
        screenshotProtection: false,
      );
      await controller.continueFromSetup();

      expect(controller.step, OnboardingStep.ready);
    },
  );

  test(
    'Wayland setup keeps clipboard and Quick Paste readiness separate',
    () async {
      final integration = _Integration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.wayland,
          globalShortcuts: true,
          remoteDesktop: LinuxRemoteDesktopState.active,
          companion: LinuxCompanionState.active,
          clipboard: true,
          quickPaste: false,
          screenshotProtection: false,
        ),
      );
      final controller = linuxOnboarding(
        store: MemoryOnboardingStore(),
        integration: integration,
      );
      addTearDown(controller.dispose);

      await controller.initialize();
      expect(controller.linux.integrationReady, isFalse);

      integration.statusValue = const LinuxIntegrationStatus(
        session: LinuxDesktopSession.wayland,
        globalShortcuts: true,
        remoteDesktop: LinuxRemoteDesktopState.active,
        companion: LinuxCompanionState.active,
        clipboard: false,
        quickPaste: true,
        screenshotProtection: false,
      );
      await controller.linux.refresh();

      expect(controller.linux.integrationReady, isFalse);
    },
  );

  test(
    'source exclusions cannot pass setup while capture needs a companion',
    () async {
      final integration = _Integration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.wayland,
          globalShortcuts: true,
          remoteDesktop: LinuxRemoteDesktopState.active,
          companion: LinuxCompanionState.active,
          clipboard: true,
          quickPaste: true,
          screenshotProtection: false,
          capabilities: {
            LinuxCapabilityKind.clipboardCapture: LinuxCapabilityStatus(
              state: LinuxCapabilityState.needsCompanion,
              backend: 'linux-wayland-data-control',
              guarantee: LinuxCapabilityGuarantee.none,
            ),
          },
        ),
      );
      final controller = linuxOnboarding(
        store: MemoryOnboardingStore(),
        integration: integration,
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      expect(controller.linux.integrationReady, isFalse);
    },
  );

  test('refreshes Linux readiness when integration setup opens', () async {
    final integration = _Integration(
      const LinuxIntegrationStatus(
        session: LinuxDesktopSession.wayland,
        globalShortcuts: true,
        remoteDesktop: LinuxRemoteDesktopState.consentRequired,
        companion: LinuxCompanionState.active,
        clipboard: false,
        quickPaste: false,
        screenshotProtection: false,
      ),
    );
    final controller = linuxOnboarding(
      store: MemoryOnboardingStore(),
      integration: integration,
    );
    addTearDown(controller.dispose);

    await controller.initialize();
    expect(controller.linux.integrationReady, isFalse);

    integration.statusValue = const LinuxIntegrationStatus(
      session: LinuxDesktopSession.wayland,
      globalShortcuts: true,
      remoteDesktop: LinuxRemoteDesktopState.active,
      companion: LinuxCompanionState.active,
      clipboard: true,
      quickPaste: true,
      screenshotProtection: false,
    );
    await controller.showSetup();

    expect(controller.step, OnboardingStep.setup);
    expect(controller.linux.integrationReady, isTrue);
  });

  testWidgets('Linux setup states the screenshot protection exception', (
    tester,
  ) async {
    final controller = linuxOnboarding(
      store: MemoryOnboardingStore(),
      integration: _Integration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.wayland,
          globalShortcuts: true,
          remoteDesktop: LinuxRemoteDesktopState.consentRequired,
          companion: LinuxCompanionState.unavailable,
          clipboard: false,
          quickPaste: false,
          screenshotProtection: false,
          startAtLogin: false,
          uriRegistered: false,
        ),
      ),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.showSetup();

    await tester.pumpWidget(
      ShadcnApp(
        home: OnboardingScreen(controller: controller, onFinished: () async {}),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Screenshot blocking'), findsOneWidget);
    expect(
      find.textContaining('Screenshot blocking is unavailable on Linux.'),
      findsOneWidget,
    );
    expect(find.widgetWithText(Button, 'Open setup'), findsOneWidget);
    expect(find.widgetWithText(Button, 'Register links'), findsOneWidget);
  });

  testWidgets('Wayland data control shows the source policy limitation', (
    tester,
  ) async {
    final controller = linuxOnboarding(
      store: MemoryOnboardingStore(),
      integration: _Integration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.wayland,
          globalShortcuts: false,
          remoteDesktop: LinuxRemoteDesktopState.unsupported,
          companion: LinuxCompanionState.unavailable,
          clipboard: true,
          quickPaste: false,
          screenshotProtection: false,
          capabilities: {
            LinuxCapabilityKind.sourceAttribution: LinuxCapabilityStatus(
              state: LinuxCapabilityState.unsupported,
              backend: 'daemonClipboard',
              guarantee: LinuxCapabilityGuarantee.none,
            ),
          },
        ),
      ),
    );
    addTearDown(controller.dispose);
    await controller.initialize();
    await controller.showSetup();

    await tester.pumpWidget(
      ShadcnApp(
        home: OnboardingScreen(controller: controller, onFinished: () async {}),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.textContaining('Basic clipboard access is ready.'),
      findsOneWidget,
    );
    expect(
      find.textContaining('Source app exclusions and Quick Paste'),
      findsOneWidget,
    );
  });

  test(
    'updates optional startup and URI integration through the typed port',
    () async {
      final integration = _Integration(
        const LinuxIntegrationStatus(
          session: LinuxDesktopSession.x11,
          globalShortcuts: true,
          remoteDesktop: LinuxRemoteDesktopState.unsupported,
          companion: LinuxCompanionState.unavailable,
          clipboard: true,
          quickPaste: true,
          screenshotProtection: false,
        ),
      );
      final controller = linuxOnboarding(
        store: MemoryOnboardingStore(),
        integration: integration,
      );
      addTearDown(controller.dispose);
      await controller.initialize();

      await controller.linux.setStartAtLogin(true);
      await controller.linux.registerCopypasteUri();

      expect(integration.startAtLoginUpdates, [true]);
      expect(integration.uriRegistrations, 1);
    },
  );
}

class _Integration implements LinuxIntegrationPort {
  _Integration(this.statusValue);

  LinuxIntegrationStatus statusValue;
  final List<bool> startAtLoginUpdates = <bool>[];
  int uriRegistrations = 0;
  bool failStatus = false;

  @override
  Future<bool> openCompanionSetup() async => true;

  @override
  Future<bool> requestRemoteDesktop() async => true;

  @override
  Future<bool> registerCopypasteUri() async {
    uriRegistrations += 1;
    return true;
  }

  @override
  Future<bool> startAtLogin() async => statusValue.startAtLogin;

  @override
  Future<bool> setStartAtLogin(bool enabled) async {
    startAtLoginUpdates.add(enabled);
    return true;
  }

  @override
  Future<LinuxIntegrationStatus> status() async {
    if (failStatus) throw StateError('Desktop disconnected');
    return statusValue;
  }
}

extension on OnboardingController {
  LinuxOnboardingSetup get linux => setup as LinuxOnboardingSetup;
}
