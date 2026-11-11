import 'package:copypaste_flutter/app/theme/app_icon_frame.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/shell/header_fit.dart';
import '../../../app/theme/app_theme.dart';

import '../../../app/theme/app_motion.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../platform/capture/capture_control_state.dart';
import '../controller/settings_controller.dart';
import '../models/settings_models.dart';

/// Capture status cell: one control that pauses or resumes capture, showing
/// the state glyph and word while the header has room and the action glyph
/// in every stage.
class CaptureHeaderAction extends StatelessWidget {
  const CaptureHeaderAction({super.key, required this.controller});

  final SettingsController controller;

  @override
  Widget build(BuildContext context) {
    final worded = HeaderFit.stageOf(context).showsStatusText;
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        final capture = controller.capture;
        final state = capture?.controlState ?? CaptureControlState.unavailable;
        final stopped = state == CaptureControlState.stopped;
        final problemColor = stopped
            ? AppTheme.statusColor(context, AppStatusTone.error)
            : null;
        final onPressed =
            capture == null || !controller.canStart(SettingsOperation.capture)
            ? null
            : controller.toggleCapture;
        return Tooltip(
          showDuration: AppMotion.resolve(context, AppMotion.standard),
          tooltip: (context) => TooltipContainer(
            child: Text('${state.actionLabel} · ${state.label}'),
          ),
          child: Button.ghost(
            key: const ValueKey<String>('capture-header-toggle'),
            style: AppTheme.headerStatusCellStyle,
            onPressed: onPressed,
            child: Semantics(
              label: state.actionLabel,
              value: state.label,
              child: ExcludeSemantics(
                child: SizedBox(
                  height: AppTheme.controlHeight(context),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      HeaderCollapsible(
                        until: HeaderStage.compact,
                        child: Padding(
                          padding: const EdgeInsetsDirectional.only(
                            end: AppSpacing.sm,
                          ),
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              AppIconFrame(
                                child: Icon(
                                  key: const ValueKey<String>(
                                    'capture-header-state',
                                  ),
                                  switch (state) {
                                    CaptureControlState.running =>
                                      LucideIcons.circleDot,
                                    CaptureControlState.paused =>
                                      LucideIcons.circlePause,
                                    CaptureControlState.stopped =>
                                      LucideIcons.circleAlert,
                                    CaptureControlState.unavailable =>
                                      LucideIcons.circleOff,
                                  },
                                  color: problemColor,
                                ),
                              ),
                              const SizedBox(width: AppSpacing.sm),
                              Text(state.label, maxLines: 1),
                            ],
                          ),
                        ),
                      ),
                      // Without the state word the action glyph also reports
                      // a stopped capture.
                      AppIconFrame(
                        child: Icon(
                          stopped && !worded
                              ? LucideIcons.circleAlert
                              : state.canResume
                              ? LucideIcons.play
                              : LucideIcons.pause,
                          color: worded ? null : problemColor,
                        ),
                      ),
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
}
