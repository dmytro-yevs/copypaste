import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';

import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_theme.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../l10n/l10n.dart';
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
      return AppOverlays.drawerDismissal(
        onDismiss: () {
          if (!controller.busy) unawaited(closeDrawer(context));
        },
        child: Focus(
          autofocus: true,
          skipTraversal: true,
          child: ConstrainedBox(
            key: const ValueKey('sms-access-setup-drawer'),
            constraints: AppOverlays.drawerContentConstraints(context),
            child: Scaffold(
              headers: [
                AppBar(
                  title: Text(
                    context.l10n.modulesSmsAccessTitle,
                    key: const ValueKey('sms-access-setup-title'),
                  ),
                  leading: const [
                    AppIconFrame(
                      child: Icon(
                        LucideIcons.messageSquare,
                        key: ValueKey('sms-access-setup-icon'),
                      ),
                    ),
                  ],
                  trailing: [
                    Tooltip(
                      tooltip: (context) => TooltipContainer(
                        child: Text(context.l10n.modulesCloseSmsAccess),
                      ),
                      child: Semantics(
                        label: context.l10n.modulesCloseSmsAccess,
                        button: true,
                        child: Button.ghost(
                          key: const ValueKey('sms-access-setup-close'),
                          style: AppTheme.controlButtonStyle(
                            const ButtonStyle.ghostIcon(),
                          ),
                          onPressed: controller.busy
                              ? null
                              : () => closeDrawer(context),
                          child: const AppIconFrame(child: Icon(LucideIcons.x)),
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
                    style: AppTheme.actionButtonStyle(
                      const ButtonStyle.primary(),
                    ),
                    key: const ValueKey('sms-access-setup-done'),
                    alignment: AppTheme.moduleSettingsActionAlignment,
                    onPressed: controller.busy
                        ? null
                        : () => closeDrawer(context),
                    child: Text(context.l10n.commonDone),
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
                      StateView.loading(
                        message: context.l10n.modulesSmsChecking,
                      )
                    else if (state != null) ...[
                      Text(
                        state.granted
                            ? context.l10n.modulesSmsReady
                            : context.l10n.modulesSmsAllowAccess,
                      ),
                      const Gap(AppSpacing.lg),
                      Card(
                        child: SetupSettingRow(
                          icon: LucideIcons.bell,
                          title: context.l10n.modulesSmsNotification,
                          description:
                              context.l10n.modulesSmsNotificationDescription,
                          statusIcon: state.notificationGranted
                              ? LucideIcons.circleCheck
                              : null,
                          action: state.notificationGranted
                              ? null
                              : Button.ghost(
                                  style: AppTheme.actionButtonStyle(
                                    const ButtonStyle.ghost(),
                                  ),
                                  onPressed: controller.busy
                                      ? null
                                      : controller.requestNotifications,
                                  child: Text(context.l10n.sharedAllow),
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
                        applyAccessLabel: context.l10n.modulesSmsApplyAccess,
                        applyAccessDescription:
                            context.l10n.modulesSmsApplyAccessDescription,
                        onOpenShizuku: controller.openShizuku,
                        onApplyAccess: controller.applyAccess,
                        onCopyCommands: controller.copyAdbCommands,
                      ),
                    ],
                    if (controller.errorMessage case final error?) ...[
                      const Gap(AppSpacing.md),
                      StateView.error(
                        title: context.l10n.modulesSmsSetupFailed,
                        message: error,
                        actionLabel: state == null
                            ? context.l10n.commonTryAgain
                            : null,
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
