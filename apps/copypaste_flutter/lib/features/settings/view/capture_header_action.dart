import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_theme.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../platform/capture/capture_control_state.dart';
import '../controller/settings_controller.dart';

class CaptureHeaderAction extends StatelessWidget {
  const CaptureHeaderAction({super.key, required this.controller});

  final SettingsController controller;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        final capture = controller.capture;
        final state = capture?.controlState ?? CaptureControlState.unavailable;
        return Tooltip(
          showDuration: AppMotion.resolve(context, AppMotion.standard),
          tooltip: (context) => TooltipContainer(
            child: Text('${state.actionLabel} · ${state.label}'),
          ),
          child: Button.secondary(
            key: const ValueKey<String>('capture-header-toggle'),
            style: AppTheme.controlButtonStyle(
              const ButtonStyle.secondaryIcon(),
            ),
            onPressed: capture == null || controller.busy
                ? null
                : controller.toggleCapture,
            child: Semantics(
              label: state.actionLabel,
              child: Icon(
                state == CaptureControlState.stopped
                    ? LucideIcons.circleAlert
                    : state.canResume
                    ? LucideIcons.play
                    : LucideIcons.pause,
                color: state == CaptureControlState.stopped
                    ? AppTheme.statusColor(context, AppStatusTone.error)
                    : null,
              ),
            ),
          ),
        );
      },
    );
  }
}
