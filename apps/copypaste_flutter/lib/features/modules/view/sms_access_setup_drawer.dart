import 'dart:async';

import 'package:flutter/services.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/android_access_setup.dart';
import '../../../shared/setup_setting_row.dart';
import '../../../shared/state_view.dart';
import '../controller/sms_access_setup_controller.dart';

class SmsAccessSetupDrawer extends StatefulWidget {
  const SmsAccessSetupDrawer({super.key, required this.controller});

  final SmsAccessSetupController controller;

  @override
  State<SmsAccessSetupDrawer> createState() => _SmsAccessSetupDrawerState();
}

class _SmsAccessSetupDrawerState extends State<SmsAccessSetupDrawer>
    with WidgetsBindingObserver {
  SmsAccessSetupController get controller => widget.controller;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    controller.setMonitoring(true);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    controller.setMonitoring(false);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    controller.setMonitoring(state == AppLifecycleState.resumed);
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: controller,
    builder: (context, _) {
      final state = controller.state;
      return CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.escape): () {
            if (!controller.busy) unawaited(closeDrawer(context));
          },
        },
        child: Focus(
          autofocus: true,
          child: ConstrainedBox(
            key: const ValueKey('sms-access-setup-drawer'),
            constraints: AppOverlays.drawerContentConstraints(context),
            child: Scaffold(
              headers: [
                AppBar(
                  title: const Text(
                    'SMS access',
                    key: ValueKey('sms-access-setup-title'),
                  ),
                  leading: const [
                    Icon(
                      LucideIcons.messageSquare,
                      key: ValueKey('sms-access-setup-icon'),
                    ),
                  ],
                  trailing: [
                    Tooltip(
                      tooltip: (context) => const TooltipContainer(
                        child: Text('Close SMS access'),
                      ),
                      child: Semantics(
                        label: 'Close SMS access',
                        button: true,
                        child: Button.ghost(
                          key: const ValueKey('sms-access-setup-close'),
                          style: AppTheme.controlButtonStyle(
                            const ButtonStyle.ghostIcon(),
                          ),
                          onPressed: controller.busy
                              ? null
                              : () => closeDrawer(context),
                          child: const Icon(LucideIcons.x),
                        ),
                      ),
                    ),
                  ],
                ),
                const Divider(),
              ],
              footers: [
                const Divider(),
                Padding(
                  padding: const EdgeInsets.all(AppSpacing.lg),
                  child: Button.primary(
                    key: const ValueKey('sms-access-setup-done'),
                    alignment: AppTheme.moduleSettingsActionAlignment,
                    onPressed: controller.busy
                        ? null
                        : () => closeDrawer(context),
                    child: const Text('Done'),
                  ),
                ),
              ],
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(AppSpacing.lg),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (controller.loading)
                      const StateView.loading(message: 'Checking SMS access.')
                    else if (state != null) ...[
                      Text(
                        state.granted
                            ? 'Access is ready. Enable SMS Codes to start copying new codes.'
                            : 'Allow access to copy codes from new SMS messages.',
                      ),
                      const Gap(AppSpacing.lg),
                      Card(
                        child: SetupSettingRow(
                          icon: LucideIcons.bell,
                          title: 'SMS notification',
                          description: 'Required while SMS Codes runs',
                          statusIcon: state.notificationGranted
                              ? LucideIcons.circleCheck
                              : null,
                          action: state.notificationGranted
                              ? null
                              : Button.ghost(
                                  onPressed: controller.busy
                                      ? null
                                      : controller.requestNotifications,
                                  child: const Text('Allow'),
                                ),
                        ),
                      ),
                      const Gap(AppSpacing.lg),
                      AndroidAccessSetup(
                        methodIndex: controller.methodIndex,
                        onMethodChanged: controller.selectMethod,
                        shizuku: state.shizuku,
                        granted: state.smsGranted,
                        busy: controller.busy,
                        adbCommands: state.adbCommands,
                        applyAccessLabel: 'Apply SMS access',
                        applyAccessDescription: 'Apply one-time SMS access.',
                        onOpenShizuku: controller.openShizuku,
                        onApplyAccess: controller.applyAccess,
                        onCopyCommands: controller.copyAdbCommands,
                      ),
                    ],
                    if (controller.errorMessage case final error?) ...[
                      const Gap(AppSpacing.md),
                      StateView.error(
                        title: 'SMS setup failed',
                        message: error,
                        actionLabel: state == null ? 'Try again' : null,
                        onAction: state == null ? controller.refresh : null,
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        ),
      );
    },
  );
}
