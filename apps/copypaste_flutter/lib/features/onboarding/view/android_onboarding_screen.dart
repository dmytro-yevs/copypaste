import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_tokens.dart';
import '../controller/android_onboarding_controller.dart';
import '../repository/android_onboarding_store.dart';

class AndroidOnboardingScreen extends StatefulWidget {
  const AndroidOnboardingScreen({
    super.key,
    required this.controller,
    required this.onPairDevice,
    required this.onOpenHistory,
  });

  final AndroidOnboardingController controller;
  final Future<void> Function() onPairDevice;
  final Future<void> Function() onOpenHistory;

  @override
  State<AndroidOnboardingScreen> createState() =>
      _AndroidOnboardingScreenState();
}

class _AndroidOnboardingScreenState extends State<AndroidOnboardingScreen>
    with WidgetsBindingObserver {
  AndroidOnboardingController get controller => widget.controller;

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
        controller.step == AndroidOnboardingStep.capture) {
      unawaited(controller.refresh());
    }
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, child) {
        return Scaffold(
          headers: [
            const AppBar(title: Text('Set up CopyPaste')),
            const Divider(),
          ],
          footers: [
            const Divider(),
            _Footer(
              controller: controller,
              onPairDevice: widget.onPairDevice,
              onOpenHistory: widget.onOpenHistory,
            ),
          ],
          loadingProgressIndeterminate: controller.busy,
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(AppSpacing.xl),
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
                      max: AndroidOnboardingStep.values.length.toDouble(),
                      disableAnimation: MediaQuery.disableAnimationsOf(context),
                    ),
                    const Gap(AppSpacing.xl),
                    switch (controller.step) {
                      AndroidOnboardingStep.welcome => const _Welcome(),
                      AndroidOnboardingStep.capture => _CaptureSetup(
                        controller: controller,
                      ),
                      AndroidOnboardingStep.sync => const _Sync(),
                    },
                    if (controller.errorMessage case final message?) ...[
                      const Gap(AppSpacing.md),
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

class _Welcome extends StatelessWidget {
  const _Welcome();

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      const Icon(LucideIcons.smartphone, size: AppIconSize.state),
      const Gap(AppSpacing.xl),
      Text('Welcome to CopyPaste', style: Theme.of(context).typography.h2),
      const Gap(AppSpacing.sm),
      const Text(
        'Save clipboard history on this phone and pair it with your other devices.',
      ).muted(),
    ],
  );
}

class _CaptureSetup extends StatelessWidget {
  const _CaptureSetup({required this.controller});

  final AndroidOnboardingController controller;

  @override
  Widget build(BuildContext context) {
    final state = controller.setupState;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Background capture', style: Theme.of(context).typography.h2),
        const Gap(AppSpacing.sm),
        const Text(
          'Choose whether CopyPaste may save copies made while another app is open.',
        ).muted(),
        const Gap(AppSpacing.lg),
        RadioGroup<AndroidCaptureMode>(
          value: controller.mode,
          onChanged: (mode) => unawaited(controller.selectMode(mode)),
          child: Column(
            children: [
              RadioCard<AndroidCaptureMode>(
                value: AndroidCaptureMode.full,
                child: const _Choice(
                  title: 'Full background capture',
                  description:
                      'One-time Shizuku or ADB setup. Copies from other apps are saved automatically.',
                ),
              ),
              const Gap(AppSpacing.sm),
              RadioCard<AndroidCaptureMode>(
                value: AndroidCaptureMode.limited,
                child: const _Choice(
                  title: 'Limited mode',
                  description:
                      'Use Share to CopyPaste, or return here after copying.',
                ),
              ),
            ],
          ),
        ),
        if (controller.mode == AndroidCaptureMode.full) ...[
          const Gap(AppSpacing.lg),
          const Alert(
            leading: Icon(LucideIcons.shieldCheck),
            title: Text('What the one-time setup allows'),
            content: Text(
              'CopyPaste reads only ClipboardService events needed to detect a blocked background read, then briefly focuses a 1×1 overlay to read the clipboard. Shizuku is not used after setup.',
            ),
          ),
          const Gap(AppSpacing.md),
          _PermissionCard(
            icon: LucideIcons.bell,
            title: 'Capture notification',
            description: 'Required while background capture is running.',
            granted: state?.notificationGranted ?? false,
            actionLabel: 'Allow',
            onAction: controller.requestNotifications,
          ),
          const Gap(AppSpacing.sm),
          _PermissionCard(
            icon: LucideIcons.batteryCharging,
            title: 'Background activity',
            description:
                'Recommended to reduce capture loss on battery-managed devices.',
            granted: state?.batteryExempt ?? false,
            actionLabel: 'Open settings',
            onAction: controller.requestBatteryExemption,
          ),
          const Gap(AppSpacing.lg),
          Tabs(
            index: controller.method.index,
            expand: true,
            onChanged: (index) => unawaited(
              controller.selectMethod(AndroidCaptureSetupMethod.values[index]),
            ),
            children: const [
              TabItem(child: Text('Shizuku')),
              TabItem(child: Text('ADB')),
            ],
          ),
          const Gap(AppSpacing.md),
          if (controller.method == AndroidCaptureSetupMethod.shizuku)
            _ShizukuSetup(controller: controller)
          else
            _AdbSetup(controller: controller),
          const Gap(AppSpacing.md),
          _Verification(controller: controller),
        ] else ...[
          const Gap(AppSpacing.lg),
          const Alert(
            leading: Icon(LucideIcons.info),
            title: Text('Limited mode'),
            content: Text(
              'Background capture stays off. Share items to CopyPaste or return to the app after copying. You can set up Full mode later.',
            ),
          ),
        ],
      ],
    );
  }
}

class _Choice extends StatelessWidget {
  const _Choice({required this.title, required this.description});

  final String title;
  final String description;

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Text(title).medium(),
      const Gap(AppSpacing.xs),
      Text(description).muted().textSmall(),
    ],
  );
}

class _PermissionCard extends StatelessWidget {
  const _PermissionCard({
    required this.icon,
    required this.title,
    required this.description,
    required this.granted,
    required this.actionLabel,
    required this.onAction,
  });

  final IconData icon;
  final String title;
  final String description;
  final bool granted;
  final String actionLabel;
  final Future<void> Function() onAction;

  @override
  Widget build(BuildContext context) => Card(
    child: LayoutBuilder(
      builder: (context, constraints) {
        final details = Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon),
            const Gap(AppSpacing.md),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title).medium(),
                  const Gap(AppSpacing.xs),
                  Text(description).muted().textSmall(),
                ],
              ),
            ),
          ],
        );
        final action = granted
            ? const Icon(LucideIcons.circleCheck)
            : Button.secondary(onPressed: onAction, child: Text(actionLabel));
        if (constraints.maxWidth < 420) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              details,
              const Gap(AppSpacing.md),
              Align(alignment: Alignment.centerLeft, child: action),
            ],
          );
        }
        return Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Expanded(child: details),
            const Gap(AppSpacing.md),
            action,
          ],
        );
      },
    ),
  );
}

class _ShizukuSetup extends StatelessWidget {
  const _ShizukuSetup({required this.controller});

  final AndroidOnboardingController controller;

  @override
  Widget build(BuildContext context) {
    final state = controller.setupState;
    final shizuku = state?.shizuku;
    if (shizuku == null) return const SizedBox.shrink();
    if (!shizuku.supported) {
      return const Alert(
        leading: Icon(LucideIcons.info),
        title: Text('Android 11 or newer is required'),
        content: Text('Use the ADB tab on this device.'),
      );
    }
    final ready = state!.privilegedGrants;
    final title = ready
        ? 'One-time grants applied'
        : !shizuku.installed
        ? 'Install Shizuku'
        : !shizuku.running
        ? 'Pair and start Shizuku'
        : 'Allow CopyPaste';
    final description = ready
        ? 'Shizuku is no longer required for capture.'
        : !shizuku.installed
        ? 'Install Shizuku from its official download page.'
        : !shizuku.running
        ? 'Use Wireless debugging pairing in Shizuku, then return here.'
        : 'Approve CopyPaste once so it can apply the six setup commands.';
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(title).medium(),
          const Gap(AppSpacing.xs),
          Text(description).muted().textSmall(),
          if (!ready) ...[
            const Gap(AppSpacing.md),
            Align(
              alignment: Alignment.centerLeft,
              child: Button.primary(
                onPressed: !shizuku.installed || !shizuku.running
                    ? controller.openShizuku
                    : controller.applyShizukuGrants,
                leading: Icon(
                  !shizuku.installed || !shizuku.running
                      ? LucideIcons.externalLink
                      : LucideIcons.shieldCheck,
                ),
                child: Text(
                  !shizuku.installed
                      ? 'Get Shizuku'
                      : !shizuku.running
                      ? 'Open Shizuku'
                      : 'Allow CopyPaste',
                ),
              ),
            ),
          ],
          const Gap(AppSpacing.sm),
          Align(
            alignment: Alignment.centerLeft,
            child: Button.ghost(
              onPressed: controller.refresh,
              child: const Text('Check again'),
            ),
          ),
        ],
      ),
    );
  }
}

class _AdbSetup extends StatelessWidget {
  const _AdbSetup({required this.controller});

  final AndroidOnboardingController controller;

  @override
  Widget build(BuildContext context) {
    final commands = controller.setupState?.adbCommands ?? const <String>[];
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Text('Run these commands on your computer').medium(),
          const Gap(AppSpacing.xs),
          const Text(
            'Enable USB debugging, connect this phone, accept the debugging prompt, then run every command in order.',
          ).muted().textSmall(),
          const Gap(AppSpacing.md),
          for (var index = 0; index < commands.length; index++) ...[
            _Command(command: commands[index], number: index + 1),
            if (index != commands.length - 1) const Gap(AppSpacing.sm),
          ],
          const Gap(AppSpacing.md),
          Align(
            alignment: Alignment.centerLeft,
            child: Button.primary(
              onPressed: controller.refresh,
              child: const Text('Check access'),
            ),
          ),
        ],
      ),
    );
  }
}

class _Command extends StatefulWidget {
  const _Command({required this.command, required this.number});

  final String command;
  final int number;

  @override
  State<_Command> createState() => _CommandState();
}

class _CommandState extends State<_Command> {
  bool copied = false;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      Expanded(child: SelectableText(widget.command).textSmall()),
      const Gap(AppSpacing.sm),
      Button.fixed(
        style: const ButtonStyle.fixedIcon(),
        onPressed: () async {
          await Clipboard.setData(ClipboardData(text: widget.command));
          if (mounted) setState(() => copied = true);
        },
        child: Icon(copied ? LucideIcons.copyCheck : LucideIcons.copy),
      ),
    ],
  );
}

class _Verification extends StatelessWidget {
  const _Verification({required this.controller});

  final AndroidOnboardingController controller;

  @override
  Widget build(BuildContext context) {
    final state = controller.setupState;
    if (controller.verified) {
      return const Alert(
        leading: Icon(LucideIcons.circleCheck),
        title: Text('Background capture is working'),
        content: Text('A copy made in another app reached your local History.'),
      );
    }
    return Card(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Text('Verify with a real copy').medium(),
          const Gap(AppSpacing.xs),
          Text(
            controller.verifying
                ? 'Leave CopyPaste, copy text in another app, then return here.'
                : 'Start capture, copy text in another app, then return. Setup is complete only after the copy reaches History.',
          ).muted().textSmall(),
          const Gap(AppSpacing.md),
          Align(
            alignment: Alignment.centerLeft,
            child: Button.primary(
              onPressed:
                  state?.privilegedGrants == true &&
                      state?.notificationGranted == true
                  ? controller.verifying
                        ? controller.refresh
                        : controller.beginVerification
                  : null,
              leading: const Icon(LucideIcons.clipboardCheck),
              child: Text(
                controller.verifying ? 'Check capture' : 'Start capture',
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _Sync extends StatelessWidget {
  const _Sync();

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      const Icon(LucideIcons.badgeCheck, size: AppIconSize.state),
      const Gap(AppSpacing.xl),
      Text('CopyPaste is ready', style: Theme.of(context).typography.h2),
      const Gap(AppSpacing.sm),
      const Text(
        'Pair another device, or open your clipboard History.',
      ).muted(),
    ],
  );
}

class _Footer extends StatelessWidget {
  const _Footer({
    required this.controller,
    required this.onPairDevice,
    required this.onOpenHistory,
  });

  final AndroidOnboardingController controller;
  final Future<void> Function() onPairDevice;
  final Future<void> Function() onOpenHistory;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.all(AppSpacing.lg),
    child: LayoutBuilder(
      builder: (context, constraints) {
        final back = controller.step == AndroidOnboardingStep.welcome
            ? null
            : Button.ghost(
                onPressed: controller.busy ? null : controller.showPreviousStep,
                leading: const Icon(LucideIcons.arrowLeft),
                child: const Text('Back'),
              );
        final actions = switch (controller.step) {
          AndroidOnboardingStep.welcome => <Widget>[
            Button.primary(
              onPressed: controller.busy ? null : controller.showCapture,
              child: const Text('Continue'),
            ),
          ],
          AndroidOnboardingStep.capture => <Widget>[
            Button.primary(
              onPressed: controller.canContinueCapture
                  ? controller.continueFromCapture
                  : null,
              child: const Text('Continue'),
            ),
          ],
          AndroidOnboardingStep.sync => <Widget>[
            Button.ghost(
              onPressed: controller.busy ? null : () => _finish(onOpenHistory),
              child: const Text('Open History'),
            ),
            Button.primary(
              onPressed: controller.busy ? null : () => _finish(onPairDevice),
              child: const Text('Pair a device'),
            ),
          ],
        };
        if (constraints.maxWidth < 440) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              for (var index = 0; index < actions.length; index++) ...[
                actions[index],
                if (index != actions.length - 1) const Gap(AppSpacing.sm),
              ],
              if (back != null) ...[const Gap(AppSpacing.sm), back],
            ],
          );
        }
        return Row(
          children: [
            if (back case final Widget button) button,
            const Spacer(),
            for (var index = 0; index < actions.length; index++) ...[
              actions[index],
              if (index != actions.length - 1) const Gap(AppSpacing.sm),
            ],
          ],
        );
      },
    ),
  );

  Future<void> _finish(Future<void> Function() destination) async {
    if (await controller.finish()) await destination();
  }
}
