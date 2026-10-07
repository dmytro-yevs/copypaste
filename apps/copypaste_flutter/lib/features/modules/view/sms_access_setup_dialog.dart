import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/android_access_setup.dart';
import '../../../shared/setup_setting_row.dart';
import '../../../shared/state_view.dart';
import '../controller/sms_access_setup_controller.dart';

class SmsAccessSetupDialog extends StatefulWidget {
  const SmsAccessSetupDialog({super.key, required this.controller});

  final SmsAccessSetupController controller;

  @override
  State<SmsAccessSetupDialog> createState() => _SmsAccessSetupDialogState();
}

class _SmsAccessSetupDialogState extends State<SmsAccessSetupDialog>
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
      return AppOverlays.alertDialog(
        icon: LucideIcons.messageSquare,
        title: const Text('SMS access'),
        content: ConstrainedBox(
          constraints: BoxConstraints(
            maxHeight:
                MediaQuery.sizeOf(context).height *
                AppOverlaySize.dialogContentHeightFactor,
          ),
          child: SingleChildScrollView(
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
        actions: [
          Button.ghost(
            onPressed: controller.busy ? null : () => Navigator.pop(context),
            child: const Text('Done'),
          ),
        ],
      );
    },
  );
}
