import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../../../app/shell/macos_window_header.dart';
import '../../../platform/macos/macos_setup_gateway.dart';
import '../controller/macos_onboarding_controller.dart';

class MacosOnboardingScreen extends StatefulWidget {
  const MacosOnboardingScreen({
    super.key,
    required this.controller,
    required this.onPairDevice,
    required this.onOpenHistory,
    this.unifiedTitleBar = false,
  });

  final MacosOnboardingController controller;
  final Future<void> Function() onPairDevice;
  final Future<void> Function() onOpenHistory;
  final bool unifiedTitleBar;

  @override
  State<MacosOnboardingScreen> createState() => _MacosOnboardingScreenState();
}

class _MacosOnboardingScreenState extends State<MacosOnboardingScreen>
    with WidgetsBindingObserver {
  MacosOnboardingController get controller => widget.controller;

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
        controller.step == MacosOnboardingStep.setup) {
      unawaited(controller.refreshSystemState());
    }
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, child) {
        return Scaffold(
          headers: widget.unifiedTitleBar
              ? const [MacosWindowHeader(title: Text('Set up CopyPaste'))]
              : const [AppBar(title: Text('Set up CopyPaste')), Divider()],
          footers: [
            const Divider(),
            _OnboardingFooter(
              controller: controller,
              onPairDevice: widget.onPairDevice,
              onOpenHistory: widget.onOpenHistory,
            ),
          ],
          loadingProgressIndeterminate: controller.busy,
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(AppSpacing.xxl),
            child: Align(
              alignment: Alignment.topCenter,
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 680),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Progress(
                      progress: controller.step.index + 1,
                      min: 0,
                      max: MacosOnboardingStep.values.length.toDouble(),
                      disableAnimation: MediaQuery.disableAnimationsOf(context),
                    ),
                    const Gap(AppSpacing.xxl),
                    switch (controller.step) {
                      MacosOnboardingStep.welcome => const _WelcomeStep(),
                      MacosOnboardingStep.setup => _SetupStep(
                        controller: controller,
                      ),
                      MacosOnboardingStep.sync => const _SyncStep(),
                    },
                    if (controller.errorMessage case final message?) ...[
                      const Gap(AppSpacing.lg),
                      Alert.destructive(
                        leading: const Icon(LucideIcons.circleAlert),
                        title: const Text('Setup needs attention'),
                        content: Text(message),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

class _WelcomeStep extends StatelessWidget {
  const _WelcomeStep();

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Icon(LucideIcons.clipboard, size: AppIconSize.state),
        const Gap(AppSpacing.xl),
        Text('Welcome to CopyPaste', style: Theme.of(context).typography.h2),
        const Gap(AppSpacing.sm),
        const Text(
          'Keep clipboard history close, open it from anywhere, and pair your devices when you are ready.',
        ).muted(),
      ],
    );
  }
}

class _SetupStep extends StatelessWidget {
  const _SetupStep({required this.controller});

  final MacosOnboardingController controller;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Allow CopyPaste to work', style: Theme.of(context).typography.h2),
        const Gap(AppSpacing.sm),
        const Text(
          'Accessibility enables auto-paste. Starting at login keeps clipboard history available after you sign in.',
        ).muted(),
        const Gap(AppSpacing.xl),
        Card(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _SetupHeading(
                icon: LucideIcons.accessibility,
                title: 'Accessibility',
                status: controller.accessibilityGranted
                    ? 'Enabled'
                    : 'Optional',
                granted: controller.accessibilityGranted,
              ),
              const Gap(AppSpacing.sm),
              const Text(
                'Allow Accessibility to paste a selected clip into the app you were using. Without it, CopyPaste still copies the clip.',
              ).muted().textSmall(),
              if (!controller.accessibilityGranted) ...[
                const Gap(AppSpacing.lg),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Button.primary(
                    onPressed: controller.busy
                        ? null
                        : controller.requestAccessibility,
                    leading: const Icon(LucideIcons.externalLink),
                    child: const Text('Enable Accessibility'),
                  ),
                ),
              ],
            ],
          ),
        ),
        const Gap(AppSpacing.md),
        Card(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              const Icon(LucideIcons.power),
              const Gap(AppSpacing.md),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text('Start at login').medium(),
                    const Gap(AppSpacing.xs),
                    Text(
                      controller.loginItemStatus ==
                              MacosLoginItemStatus.developmentUnavailable
                          ? 'Available in the installed CopyPaste app.'
                          : 'Open CopyPaste automatically when you sign in to this Mac.',
                    ).muted().textSmall(),
                  ],
                ),
              ),
              const Gap(AppSpacing.lg),
              Semantics(
                label: 'Start CopyPaste at login',
                toggled: controller.launchAtLogin,
                child: Switch(
                  value: controller.launchAtLogin,
                  onChanged:
                      controller.busy || !controller.launchAtLoginAvailable
                      ? null
                      : controller.setLaunchAtLogin,
                ),
              ),
            ],
          ),
        ),
        if (controller.loginItemStatus ==
            MacosLoginItemStatus.requiresApproval) ...[
          const Gap(AppSpacing.md),
          Alert(
            leading: const Icon(LucideIcons.info),
            title: const Text('Login Item approval is required'),
            content: const Text(
              'Allow CopyPaste in System Settings, then return here.',
            ),
            trailing: Button.secondary(
              onPressed: controller.openLoginItemsSettings,
              child: const Text('Open Settings'),
            ),
          ),
        ],
      ],
    );
  }
}

class _SetupHeading extends StatelessWidget {
  const _SetupHeading({
    required this.icon,
    required this.title,
    required this.status,
    required this.granted,
  });

  final IconData icon;
  final String title;
  final String status;
  final bool granted;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Icon(icon),
        const Gap(AppSpacing.md),
        Expanded(child: Text(title).medium()),
        Icon(
          granted ? LucideIcons.circleCheck : LucideIcons.circleAlert,
          size: AppIconSize.sm,
        ),
        const Gap(AppSpacing.xs),
        Text(status).textSmall(),
      ],
    );
  }
}

class _SyncStep extends StatelessWidget {
  const _SyncStep();

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Icon(LucideIcons.badgeCheck, size: AppIconSize.state),
        const Gap(AppSpacing.xl),
        Text('CopyPaste is ready', style: Theme.of(context).typography.h2),
        const Gap(AppSpacing.sm),
        const Text(
          'Pair another device to sync, or open your clipboard history now.',
        ).muted(),
      ],
    );
  }
}

class _OnboardingFooter extends StatelessWidget {
  const _OnboardingFooter({
    required this.controller,
    required this.onPairDevice,
    required this.onOpenHistory,
  });

  final MacosOnboardingController controller;
  final Future<void> Function() onPairDevice;
  final Future<void> Function() onOpenHistory;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(AppSpacing.lg),
      child: Align(
        alignment: Alignment.center,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 680),
          child: Row(
            children: [
              if (controller.step != MacosOnboardingStep.welcome)
                Button.ghost(
                  onPressed: controller.busy
                      ? null
                      : controller.showPreviousStep,
                  leading: const Icon(LucideIcons.arrowLeft),
                  child: const Text('Back'),
                ),
              const Spacer(),
              switch (controller.step) {
                MacosOnboardingStep.welcome => Button.primary(
                  onPressed: controller.busy ? null : controller.showSetup,
                  trailing: const Icon(LucideIcons.arrowRight),
                  child: const Text('Continue'),
                ),
                MacosOnboardingStep.setup => Button.primary(
                  onPressed: controller.canContinueSetup
                      ? controller.continueFromSetup
                      : null,
                  trailing: const Icon(LucideIcons.arrowRight),
                  child: const Text('Continue'),
                ),
                MacosOnboardingStep.sync => Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Button.ghost(
                      onPressed: controller.busy
                          ? null
                          : () => _finish(onOpenHistory),
                      child: const Text('Open History'),
                    ),
                    const Gap(AppSpacing.sm),
                    Button.primary(
                      onPressed: controller.busy
                          ? null
                          : () => _finish(onPairDevice),
                      leading: const Icon(LucideIcons.laptop),
                      child: const Text('Pair a device'),
                    ),
                  ],
                ),
              },
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _finish(Future<void> Function() destination) async {
    if (await controller.finish()) {
      await destination();
    }
  }
}
