import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../platform/permissions/linux_integration.dart';
import '../../../shared/setup_setting_row.dart';
import '../controller/linux_onboarding_controller.dart';
import 'onboarding_intro.dart';
import 'onboarding_scaffold.dart';

class LinuxOnboardingScreen extends StatefulWidget {
  const LinuxOnboardingScreen({
    super.key,
    required this.controller,
    required this.onFinished,
  });

  final LinuxOnboardingController controller;
  final Future<void> Function() onFinished;

  @override
  State<LinuxOnboardingScreen> createState() => _LinuxOnboardingScreenState();
}

class _LinuxOnboardingScreenState extends State<LinuxOnboardingScreen>
    with WidgetsBindingObserver {
  LinuxOnboardingController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed &&
        controller.step == LinuxOnboardingStep.integration) {
      unawaited(controller.refreshIntegration());
    }
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: controller,
    builder: (context, child) => OnboardingScaffold(
      stepIndex: controller.step.index,
      stepCount: LinuxOnboardingStep.values.length,
      platform: 'Linux',
      busy: controller.busy,
      errorMessage: controller.errorMessage,
      onBack:
          controller.step == LinuxOnboardingStep.integration && !controller.busy
          ? controller.showPreviousStep
          : null,
      action: Button.primary(
        onPressed: controller.busy
            ? null
            : switch (controller.step) {
                LinuxOnboardingStep.welcome => controller.showIntegration,
                LinuxOnboardingStep.integration =>
                  controller.continueFromIntegration,
                LinuxOnboardingStep.sync => _finish,
              },
        child: Text(
          controller.step == LinuxOnboardingStep.sync
              ? 'Get started'
              : 'Continue',
        ),
      ),
      child: switch (controller.step) {
        LinuxOnboardingStep.welcome => const OnboardingIntro.welcome(),
        LinuxOnboardingStep.integration => _IntegrationSetup(
          controller: controller,
        ),
        LinuxOnboardingStep.sync => const OnboardingIntro.ready(),
      },
    ),
  );

  Future<void> _finish() async {
    if (await controller.finish()) await widget.onFinished();
  }
}

class _IntegrationSetup extends StatelessWidget {
  const _IntegrationSetup({required this.controller});

  final LinuxOnboardingController controller;

  @override
  Widget build(BuildContext context) {
    final status = controller.status;
    if (status == null) {
      return const Text('Checking your Linux desktop session.');
    }
    final isWayland = status.session == LinuxDesktopSession.wayland;
    final companionReady =
        status.companion == LinuxCompanionState.active && status.clipboard;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Set up your Linux desktop',
          style: Theme.of(context).typography.h1,
        ),
        const Gap(AppSpacing.sm),
        const Text(
          'CopyPaste needs verified desktop integration for clipboard attribution and Quick Paste.',
        ).muted(),
        const Gap(AppSpacing.xxl),
        Card(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SetupSettingRow(
                icon: LucideIcons.monitor,
                title: 'Desktop session',
                description: switch (status.session) {
                  LinuxDesktopSession.x11 => 'X11 session detected.',
                  LinuxDesktopSession.wayland => 'Wayland session detected.',
                  LinuxDesktopSession.unsupported =>
                    'No supported X11 or Wayland desktop session was detected.',
                },
                statusIcon: status.session == LinuxDesktopSession.unsupported
                    ? LucideIcons.circleAlert
                    : LucideIcons.circleCheck,
              ),
              if (isWayland) ...[
                const Divider(),
                SetupSettingRow(
                  icon: LucideIcons.puzzle,
                  title: 'GNOME or KDE companion',
                  description: companionReady
                      ? 'The authenticated Clipboard v2 bridge is ready.'
                      : 'Install and enable the signed companion session package, then return here.',
                  statusIcon: companionReady ? LucideIcons.circleCheck : null,
                  action: companionReady
                      ? null
                      : Button.secondary(
                          onPressed: controller.busy
                              ? null
                              : controller.openCompanionSetup,
                          child: const Text('Open setup'),
                        ),
                ),
                const Divider(),
                SetupSettingRow(
                  icon: LucideIcons.keyboard,
                  title: 'Keyboard control',
                  description: switch (status.remoteDesktop) {
                    LinuxRemoteDesktopState.active =>
                      'Keyboard control is allowed for Quick Paste.',
                    LinuxRemoteDesktopState.consentRequired =>
                      'Allow keyboard control so Quick Paste can return to the previous app.',
                    LinuxRemoteDesktopState.unsupported =>
                      'Keyboard control is unavailable in this desktop session.',
                  },
                  statusIcon:
                      status.remoteDesktop == LinuxRemoteDesktopState.active
                      ? LucideIcons.circleCheck
                      : null,
                  action:
                      status.remoteDesktop ==
                          LinuxRemoteDesktopState.consentRequired
                      ? Button.secondary(
                          onPressed: controller.busy
                              ? null
                              : controller.requestRemoteDesktop,
                          child: const Text('Allow keyboard control'),
                        )
                      : null,
                ),
              ],
              const Divider(),
              SetupSettingRow(
                icon: LucideIcons.clipboard,
                title: 'Quick Paste',
                description: status.quickPaste
                    ? 'Quick Paste is ready.'
                    : 'Quick Paste will be ready after the required desktop integration is active.',
                statusIcon: status.quickPaste
                    ? LucideIcons.circleCheck
                    : LucideIcons.circleAlert,
              ),
              const Divider(),
              SetupSettingRow(
                icon: LucideIcons.power,
                title: 'Start at login',
                description: 'Open CopyPaste when you sign in.',
                action: Checkbox(
                  state: status.startAtLogin
                      ? CheckboxState.checked
                      : CheckboxState.unchecked,
                  onChanged: controller.busy
                      ? null
                      : (state) => controller.setStartAtLogin(
                          state == CheckboxState.checked,
                        ),
                  trailing: const Text('Enable'),
                ),
              ),
              const Divider(),
              SetupSettingRow(
                icon: LucideIcons.link,
                title: 'CopyPaste links',
                description: status.uriRegistered
                    ? 'CopyPaste links open in this app.'
                    : 'Register CopyPaste links for pairing invitations.',
                statusIcon: status.uriRegistered
                    ? LucideIcons.circleCheck
                    : null,
                action: status.uriRegistered
                    ? null
                    : Button.secondary(
                        onPressed: controller.busy
                            ? null
                            : controller.registerCopypasteUri,
                        child: const Text('Register links'),
                      ),
              ),
              const Divider(),
              const SetupSettingRow(
                icon: LucideIcons.cameraOff,
                title: 'Screenshot blocking',
                description:
                    'Screenshot blocking is unavailable on Linux. Protected stored data remains encrypted.',
                statusIcon: LucideIcons.circleAlert,
              ),
            ],
          ),
        ),
        const Gap(AppSpacing.lg),
        Align(
          alignment: Alignment.centerLeft,
          child: Button.ghost(
            onPressed: controller.busy ? null : controller.refreshIntegration,
            child: const Text('Check again'),
          ),
        ),
      ],
    );
  }
}
