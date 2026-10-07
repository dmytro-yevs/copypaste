import 'dart:async';

import 'package:shadcn_flutter/shadcn_flutter.dart';

import '../../../app/theme/app_overlays.dart';
import '../../../app/theme/app_toast.dart';
import '../../../app/theme/app_tokens.dart';
import '../../../shared/state_view.dart';
import '../controller/history_ocr_controller.dart';
import '../models/history_models.dart';

Future<void> showHistoryOcrDialog(
  BuildContext context, {
  required HistoryOcrController controller,
  required HistoryClip clip,
}) async {
  if (!controller.canRun) return;
  unawaited(controller.recognize(clip));
  await AppOverlays.showDialog<void>(
    context,
    builder: (dialogContext) => AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        final text = controller.text;
        final content = controller.busy
            ? const StateView.loading(message: 'Recognizing image text.')
            : controller.errorMessage != null
            ? StateView.error(
                title: 'OCR failed',
                message: controller.errorMessage,
                actionLabel: controller.canRun ? 'Try again' : null,
                onAction: controller.canRun
                    ? () => unawaited(controller.recognize(clip))
                    : null,
              )
            : text == null || text.trim().isEmpty
            ? const StateView.empty(title: 'No text found')
            : SingleChildScrollView(child: SelectableText(text));
        return AppOverlays.alertDialog(
          icon: LucideIcons.scanText,
          title: const Text('Image text'),
          content: ConstrainedBox(
            constraints: BoxConstraints(
              maxHeight:
                  MediaQuery.sizeOf(context).height *
                  AppOverlaySize.dialogContentHeightFactor,
            ),
            child: content,
          ),
          actions: [
            Button.secondary(
              onPressed: () => Navigator.pop(dialogContext),
              child: const Text('Close'),
            ),
            Button.primary(
              key: const ValueKey('history-ocr-copy'),
              onPressed: controller.busy || text == null || text.trim().isEmpty
                  ? null
                  : () async {
                      final copied = await controller.copyText();
                      if (!context.mounted) return;
                      AppToast.show(
                        context,
                        title: copied ? 'Copied' : 'Copy failed',
                        message: copied
                            ? 'Recognized text copied.'
                            : 'Text could not be copied. Try again.',
                        tone: copied
                            ? AppToastTone.success
                            : AppToastTone.error,
                      );
                    },
              leading: const Icon(LucideIcons.copy),
              child: const Text('Copy'),
            ),
          ],
        );
      },
    ),
  );
}
