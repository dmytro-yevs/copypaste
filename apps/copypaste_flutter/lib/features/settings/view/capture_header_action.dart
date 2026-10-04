import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_motion.dart';
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
        final paused = capture?.paused ?? true;
        final label = paused ? 'Resume capture' : 'Pause capture';
        return Tooltip(
          showDuration: AppMotion.resolve(context, AppMotion.standard),
          tooltip: (context) => TooltipContainer(
            child: Text('$label · ${capture?.label ?? 'Unavailable'}'),
          ),
          child: Button.secondary(
            key: const ValueKey<String>('capture-header-toggle'),
            style: const ButtonStyle.secondaryIcon(),
            onPressed: capture == null || controller.busy
                ? null
                : controller.toggleCapture,
            child: Icon(paused ? LucideIcons.play : LucideIcons.pause),
          ),
        );
      },
    );
  }
}
