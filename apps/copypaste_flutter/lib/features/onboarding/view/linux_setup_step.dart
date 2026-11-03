import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../platform/permissions/linux_integration.dart';
import '../../../shared/setup_setting_row.dart';
import '../controller/linux_onboarding_setup.dart';

class LinuxSetupStep extends StatelessWidget {
  const LinuxSetupStep({super.key, required this.setup});

  final LinuxOnboardingSetup setup;

  @override
  Widget build(BuildContext context) {
    final status = setup.status;
    if (status == null) {
      return const Text('Checking your Linux desktop session.');
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
                      : status
                                .capability(
                                  LinuxCapabilityKind.clipboardCapture,
                                )
                                .state ==
                            LinuxCapabilityState.needsCompanion
                      ? 'Clipboard capture is paused by source app exclusions. Enable the signed companion to resume it.'
                      : status.clipboard &&
                            status
                                    .capability(
                                      LinuxCapabilityKind.sourceAttribution,
                                    )
                                    .state ==
                                LinuxCapabilityState.unsupported
                      ? 'Basic clipboard access is ready. Source app exclusions and Quick Paste need the signed companion.'
                      : 'Install and enable the signed companion session package, then return here.',
                  statusIcon: companionReady ? LucideIcons.circleCheck : null,
                  action: companionReady
                      ? null
                      : Button.secondary(
                          onPressed: setup.busy
                              ? null
                              : setup.openCompanionSetup,
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
                          onPressed: setup.busy
                              ? null
                              : setup.requestRemoteDesktop,
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
                  onChanged: setup.busy
                      ? null
                      : (state) => setup.setStartAtLogin(
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
                        onPressed: setup.busy
                            ? null
                            : setup.registerCopypasteUri,
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
            onPressed: setup.busy ? null : setup.refresh,
            child: const Text('Check again'),
          ),
        ),
      ],
    );
  }
}
