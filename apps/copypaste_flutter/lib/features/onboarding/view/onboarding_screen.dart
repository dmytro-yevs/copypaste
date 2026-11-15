import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
import '../../devices/devices_controller.dart';
import '../../../shared/failure_toasts.dart';
import '../controller/android_onboarding_setup.dart';
import '../controller/linux_onboarding_setup.dart';
import '../controller/macos_onboarding_setup.dart';
import '../controller/onboarding_controller.dart';
import '../controller/onboarding_setup.dart';
import '../controller/windows_onboarding_setup.dart';
import 'android_setup_step.dart';
import 'linux_setup_step.dart';
import 'macos_setup_step.dart';
import 'onboarding_intro.dart';
import 'onboarding_pairing_step.dart';
import 'onboarding_scaffold.dart';
import 'onboarding_space_step.dart';
import 'windows_setup_step.dart';

/// What to open after onboarding when the space step asked for pairing.
enum OnboardingSpaceIntent { addDevice, joinSpace }

/// Welcome, platform setup, space and ready steps for every platform.
class OnboardingScreen extends StatefulWidget {
  const OnboardingScreen({
    super.key,
    required this.controller,
    required this.onFinished,
    this.onSpaceIntent,
    this.devices,
    this.unifiedTitleBar = false,
  });

  final OnboardingController controller;
  final Future<void> Function() onFinished;

  /// Called after [onFinished] when the user chose to pair now but the tour
  /// could not pair inline.
  final void Function(OnboardingSpaceIntent intent)? onSpaceIntent;

  /// Renders the inline pairing ceremony; the same controller backs the
  /// tour's [OnboardingController.attachPairing].
  final DevicesController? devices;
  final bool unifiedTitleBar;

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

class _OnboardingScreenState extends State<OnboardingScreen>
    with WidgetsBindingObserver {
  OnboardingController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    controller.setAttached(true);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    controller.setAttached(false);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    controller.setForeground(state == AppLifecycleState.resumed);
  }

  @override
  Widget build(BuildContext context) => FailureToasts(
    listenable: controller,
    failure: () => controller.errorMessage,
    title: (context) => context.l10n.onboardingSetupErrorTitle,
    child: AnimatedBuilder(
      animation: controller,
      builder: (context, child) {
        final step = controller.step;
        final setup = controller.setup;
        // Devices is a step only where this device offers to join a space.
        final steps = [
          for (final each in OnboardingStep.values)
            if (each != OnboardingStep.space || controller.offersSpace) each,
        ];
        final devices = widget.devices;
        final pairingKind = devices == null ? null : controller.pairingKind;
        final decisionInFlight =
            pairingKind != null && devices!.decisionInFlight;
        return OnboardingScaffold(
          steps: [
            for (final each in steps)
              switch (each) {
                OnboardingStep.welcome => context.l10n.onboardingBadgeWelcome,
                OnboardingStep.setup => context.l10n.onboardingBadgeSetup,
                OnboardingStep.space => context.l10n.onboardingBadgeDevices,
                OnboardingStep.ready => context.l10n.onboardingBadgeReady,
              },
          ],
          stepIndex: steps.indexOf(step).clamp(0, steps.length - 1),
          unifiedTitleBar: widget.unifiedTitleBar,
          busy: controller.busy,
          onBack: controller.canGoBack && !decisionInFlight
              ? controller.showPreviousStep
              : null,
          action: switch (step) {
            OnboardingStep.space when pairingKind != null => Button.secondary(
              key: const ValueKey<String>('onboarding-pairing-skip'),
              onPressed: controller.canContinue && !decisionInFlight
                  ? controller.skipPairing
                  : null,
              child: Text(context.l10n.onboardingPairSkip),
            ),
            OnboardingStep.space => Button.secondary(
              onPressed: controller.canContinue
                  ? () => controller.chooseSpace(OnboardingSpaceChoice.later)
                  : null,
              child: Text(context.l10n.onboardingSpaceLater),
            ),
            _ => Button.primary(
              onPressed: controller.canContinue ? _next : null,
              child: Text(switch (step) {
                OnboardingStep.ready when _handsOffJoin =>
                  context.l10n.onboardingJoinNow,
                OnboardingStep.ready ||
                OnboardingStep.space => context.l10n.onboardingGetStarted,
                OnboardingStep.setup =>
                  setup.continueLabel ?? context.l10n.commonContinue,
                OnboardingStep.welcome => context.l10n.commonContinue,
              }),
            ),
          },
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              switch (step) {
                OnboardingStep.welcome => const OnboardingIntro.welcome(),
                OnboardingStep.setup => _SetupStep(setup: setup),
                OnboardingStep.space when pairingKind != null =>
                  OnboardingPairingStep(
                    kind: pairingKind,
                    joinMethod: controller.joinMethod,
                    devices: devices!,
                    onJoinMethod: controller.useJoinMethod,
                    onClose: controller.leavePairing,
                  ),
                OnboardingStep.space => OnboardingSpaceStep(
                  onChoose: controller.canContinue
                      ? controller.chooseSpace
                      : null,
                ),
                OnboardingStep.ready => OnboardingIntro.ready(
                  message: _readyMessage(context),
                  action: _readyAction(context),
                ),
              },
              if (controller.pairingInterrupted && pairingKind == null) ...[
                const Gap(AppSpacing.lg),
                OnboardingPairingNotice(issue: controller.pairingIssue),
              ],
              if (controller.noticeMessage case final message?) ...[
                const Gap(AppSpacing.lg),
                Alert(
                  title: Text(context.l10n.onboardingOptionalSetupTitle),
                  content: Text(message),
                  trailing:
                      step == OnboardingStep.setup &&
                          setup.noticeOpensSystemSettings
                      ? Button.secondary(
                          onPressed: setup.openSystemSettings,
                          child: Text(
                            context.l10n.onboardingOpenSystemSettings,
                          ),
                        )
                      : null,
                ),
              ],
            ],
          ),
        );
      },
    ),
  );

  /// Joining could not run in the tour, so Devices takes over after it.
  bool get _handsOffJoin =>
      controller.spaceChoice == OnboardingSpaceChoice.join &&
      !controller.paired;

  bool get _pairsInline => controller.pairsInline && widget.devices != null;

  String? _readyMessage(BuildContext context) {
    final l10n = context.l10n;
    if (controller.paired) {
      return switch (controller.pairedDeviceName) {
        final name? => l10n.onboardingReadyPaired(name),
        null => l10n.onboardingReadyPairedUnnamed,
      };
    }
    return switch (controller.spaceChoice) {
      OnboardingSpaceChoice.create => l10n.onboardingReadySpaceCreated,
      OnboardingSpaceChoice.join => l10n.onboardingReadyJoin,
      OnboardingSpaceChoice.later when controller.offersSpace =>
        l10n.onboardingReadyPairLater,
      OnboardingSpaceChoice.later => null,
    };
  }

  Widget? _readyAction(BuildContext context) {
    final addDevice = controller.spaceChoice == OnboardingSpaceChoice.create
        ? Button.secondary(
            key: const ValueKey<String>('onboarding-add-device'),
            onPressed: controller.canContinue
                ? () => _pairsInline
                      ? controller.startPairing(OnboardingPairingKind.addDevice)
                      : _finish(OnboardingSpaceIntent.addDevice)
                : null,
            leading: const Icon(LucideIcons.plus),
            child: Text(context.l10n.onboardingAddDeviceNow),
          )
        : null;
    if (!controller.paired) return addDevice;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        OnboardingPairedDevice(name: controller.pairedDeviceName),
        if (addDevice != null) ...[
          const Gap(AppSpacing.lg),
          Align(alignment: AlignmentDirectional.centerStart, child: addDevice),
        ],
      ],
    );
  }

  Future<void> _next() async {
    switch (controller.step) {
      case OnboardingStep.welcome:
        await controller.showSetup();
      case OnboardingStep.setup:
        await controller.continueFromSetup();
      case OnboardingStep.space:
        controller.chooseSpace(OnboardingSpaceChoice.later);
      case OnboardingStep.ready:
        await _finish(_handsOffJoin ? OnboardingSpaceIntent.joinSpace : null);
    }
  }

  Future<void> _finish(OnboardingSpaceIntent? intent) async {
    if (!await controller.finish()) return;
    await widget.onFinished();
    if (intent != null) widget.onSpaceIntent?.call(intent);
  }
}

class _SetupStep extends StatelessWidget {
  const _SetupStep({required this.setup});

  final OnboardingSetup setup;

  @override
  Widget build(BuildContext context) => switch (setup) {
    final MacosOnboardingSetup setup => MacosSetupStep(setup: setup),
    final WindowsOnboardingSetup setup => WindowsSetupStep(setup: setup),
    final LinuxOnboardingSetup setup => LinuxSetupStep(setup: setup),
    final AndroidOnboardingSetup setup => AndroidSetupStep(setup: setup),
    _ => throw UnsupportedError('No setup view for ${setup.platformLabel}.'),
  };
}
