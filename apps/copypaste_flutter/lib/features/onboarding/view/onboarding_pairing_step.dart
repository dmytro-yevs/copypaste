import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_icon_frame.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../../shared/state_view.dart';
import '../../devices/devices.dart';
import '../controller/onboarding_controller.dart';

/// The Devices pairing ceremony inside the space step: entering a code or
/// scanning a QR to join, or showing an invitation to add a device.
class OnboardingPairingStep extends StatelessWidget {
  const OnboardingPairingStep({
    super.key,
    required this.kind,
    required this.joinMethod,
    required this.devices,
    required this.onJoinMethod,
    required this.onClose,
  });

  final OnboardingPairingKind kind;
  final OnboardingJoinMethod joinMethod;
  final DevicesController devices;
  final ValueChanged<OnboardingJoinMethod> onJoinMethod;
  final Future<void> Function() onClose;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final joining = kind == OnboardingPairingKind.join;
    final mode = devices.pairingEntryMode;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          joining ? l10n.onboardingPairJoinTitle : l10n.onboardingAddDeviceNow,
          style: Theme.of(context).typography.h3,
        ),
        const Gap(AppSpacing.sm),
        Text(
          joining
              ? l10n.onboardingPairJoinSubtitle
              : l10n.onboardingPairAddDeviceSubtitle,
        ).muted(),
        const Gap(AppSpacing.xxl),
        // The method is fixed once a ceremony has started.
        if (joining && devices.pairing == null) ...[
          Tabs(
            key: const ValueKey<String>('onboarding-join-method'),
            index: joinMethod.index,
            onChanged: (index) {
              if (devices.canChangePairingMode) {
                onJoinMethod(OnboardingJoinMethod.values[index]);
              }
            },
            children: [
              TabItem(child: Text(l10n.devicesEnterCode)),
              TabItem(child: Text(l10n.devicesScanQr)),
            ],
          ),
          const Gap(AppSpacing.lg),
        ],
        if (mode == null)
          StateView.loading(
            message: joining
                ? l10n.pairingConnectingDevice
                : l10n.pairingCreatingInvitation,
          )
        else
          PairingInspector(
            key: ValueKey<PairingEntryMode>(mode),
            controller: devices,
            inDrawer: false,
            fillsHeight: false,
            onClose: onClose,
          ),
      ],
    );
  }
}

/// The device paired during the tour, shown on the ready step.
class OnboardingPairedDevice extends StatelessWidget {
  const OnboardingPairedDevice({super.key, required this.name});

  final String? name;

  @override
  Widget build(BuildContext context) => Card(
    key: const ValueKey<String>('onboarding-paired-device'),
    child: Row(
      children: [
        const AppIconFrame(
          size: AppIconSize.lg,
          child: Icon(LucideIcons.monitorSmartphone, size: AppIconSize.md),
        ),
        const Gap(AppSpacing.md),
        Expanded(
          child: Text(
            name ?? context.l10n.pairingOtherDevice,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ).medium(),
        ),
        const Gap(AppSpacing.md),
        SecondaryBadge(child: Text(context.l10n.pairingStepPaired)),
      ],
    ),
  );
}

/// What to do after a ceremony failed or could not start.
class OnboardingPairingNotice extends StatelessWidget {
  const OnboardingPairingNotice({super.key, this.issue});

  final String? issue;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Alert(
      key: const ValueKey<String>('onboarding-pairing-notice'),
      leading: const AppIconFrame(child: Icon(LucideIcons.circleAlert)),
      title: Text(l10n.onboardingPairInterruptedTitle),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (issue case final issue?) ...[
            Text(issue),
            const Gap(AppSpacing.xxs),
          ],
          Text(l10n.onboardingPairInterruptedMessage),
        ],
      ),
    );
  }
}
