import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../platform/permissions/linux_integration.dart';
import '../../../shared/setup_setting_row.dart';
import '../controller/linux_onboarding_setup.dart';

class LinuxSetupStep extends StatelessWidget {
  const LinuxSetupStep({super.key, required this.setup});

  final LinuxOnboardingSetup setup;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final status = setup.status;
    if (status == null) {
      return Text(l10n.onboardingLinuxChecking);
    }
    final isWayland = status.session == LinuxDesktopSession.wayland;
    final companionReady =
        status.companion == LinuxCompanionState.active &&
        status.clipboard &&
        (status.capabilities.isEmpty ||
            status.capability(LinuxCapabilityKind.sourceAttribution).state ==
                LinuxCapabilityState.available);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(l10n.onboardingLinuxTitle, style: Theme.of(context).typography.h1),
        const Gap(AppSpacing.sm),
        Text(l10n.onboardingLinuxSubtitle).muted(),
        const Gap(AppSpacing.xxl),
        Card(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SetupSettingRow(
                icon: LucideIcons.monitor,
                title: l10n.onboardingLinuxSession,
                description: switch (status.session) {
                  LinuxDesktopSession.x11 => l10n.onboardingLinuxX11Detected,
                  LinuxDesktopSession.wayland =>
                    l10n.onboardingLinuxWaylandDetected,
                  LinuxDesktopSession.unsupported =>
                    l10n.onboardingLinuxSessionUnsupported,
                },
                statusIcon: status.session == LinuxDesktopSession.unsupported
                    ? LucideIcons.circleAlert
                    : LucideIcons.circleCheck,
              ),
              if (isWayland) ...[
                const Divider(),
                SetupSettingRow(
                  icon: LucideIcons.puzzle,
                  title: l10n.onboardingLinuxCompanion,
                  description: companionReady
                      ? l10n.onboardingLinuxCompanionReady
                      : status
                                .capability(
                                  LinuxCapabilityKind.clipboardCapture,
                                )
                                .state ==
                            LinuxCapabilityState.needsCompanion
                      ? l10n.onboardingLinuxCompanionCapturePaused
                      : setup.captureReady
                      ? l10n.onboardingLinuxCompanionOptional
                      : l10n.onboardingLinuxCompanionInstall,
                  statusIcon: companionReady ? LucideIcons.circleCheck : null,
                  action: companionReady
                      ? null
                      : Button.secondary(
                          onPressed: setup.busy
                              ? null
                              : setup.openCompanionSetup,
                          child: Text(l10n.onboardingLinuxOpenSetup),
                        ),
                ),
                const Divider(),
                SetupSettingRow(
                  icon: LucideIcons.keyboard,
                  title: l10n.onboardingLinuxKeyboardControl,
                  description: switch (status.remoteDesktop) {
                    LinuxRemoteDesktopState.active =>
                      l10n.onboardingLinuxKeyboardAllowed,
                    LinuxRemoteDesktopState.consentRequired =>
                      l10n.onboardingLinuxKeyboardOptional,
                    LinuxRemoteDesktopState.unsupported =>
                      l10n.onboardingLinuxKeyboardUnsupported,
                  },
                  statusIcon:
                      status.remoteDesktop == LinuxRemoteDesktopState.active
                      ? LucideIcons.circleCheck
                      : null,
                  action:
                      status.remoteDesktop ==
                          LinuxRemoteDesktopState.consentRequired
                      ? Button.secondary(
                          onPressed: setup.busy
                              ? null
                              : setup.requestRemoteDesktop,
                          child: Text(l10n.onboardingLinuxAllowKeyboard),
                        )
                      : null,
                ),
              ],
              const Divider(),
              SetupSettingRow(
                icon: LucideIcons.clipboard,
                title: 'Quick Paste',
                description: status.quickPaste
                    ? l10n.onboardingLinuxQuickPasteReady
                    : l10n.onboardingLinuxQuickPastePending,
                statusIcon: status.quickPaste
                    ? LucideIcons.circleCheck
                    : LucideIcons.circleAlert,
              ),
              const Divider(),
              SetupSettingRow(
                icon: LucideIcons.power,
                title: l10n.onboardingStartAtLogin,
                description: l10n.onboardingLinuxStartAtLoginDescription,
                action: Checkbox(
                  state: status.startAtLogin
                      ? CheckboxState.checked
                      : CheckboxState.unchecked,
                  onChanged: setup.busy
                      ? null
                      : (state) => setup.setStartAtLogin(
                          state == CheckboxState.checked,
                        ),
                  trailing: Text(l10n.commonEnable),
                ),
              ),
              const Divider(),
              SetupSettingRow(
                icon: LucideIcons.link,
                title: l10n.onboardingLinuxLinks,
                description: status.uriRegistered
                    ? l10n.onboardingLinuxLinksRegistered
                    : l10n.onboardingLinuxLinksUnregistered,
                statusIcon: status.uriRegistered
                    ? LucideIcons.circleCheck
                    : null,
                action: status.uriRegistered
                    ? null
                    : Button.secondary(
                        onPressed: setup.busy
                            ? null
                            : setup.registerCopypasteUri,
                        child: Text(l10n.onboardingLinuxRegisterLinks),
                      ),
              ),
              const Divider(),
              SetupSettingRow(
                icon: LucideIcons.cameraOff,
                title: l10n.onboardingLinuxScreenshotBlocking,
                description: l10n.onboardingLinuxScreenshotBlockingUnavailable,
                statusIcon: LucideIcons.circleAlert,
              ),
            ],
          ),
        ),
        const Gap(AppSpacing.lg),
        Align(
          alignment: Alignment.centerLeft,
          child: Button.ghost(
            onPressed: setup.busy ? null : setup.refresh,
            child: Text(l10n.onboardingLinuxCheckAgain),
          ),
        ),
      ],
    );
  }
}
